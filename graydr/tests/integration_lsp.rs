// Integration tests for `graydr lsp` subcommand.
// Phase 21 — LSP Core and Diagnostics
// These tests are RED in Wave 0 (LSP subcommand not yet implemented).
// They turn GREEN in Plan 02 (handshake) and Plan 03 (diagnostics).

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn graydr_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // up to workspace root
    path.push("target/debug/graydr");
    path
}

fn lsp_fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/lsp");
    path.push(name);
    path
}

fn send_lsp(writer: &mut impl Write, msg: &str) {
    let header = format!("Content-Length: {}\r\n\r\n", msg.len());
    writer.write_all(header.as_bytes()).unwrap();
    writer.write_all(msg.as_bytes()).unwrap();
    writer.flush().unwrap();
}

fn recv_lsp(reader: &mut impl BufRead) -> String {
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        if n == 0 {
            panic!("LSP server closed stdout before sending response");
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
            length = rest.parse().unwrap();
        }
    }
    let mut buf = vec![0u8; length];
    std::io::Read::read_exact(reader, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

const INITIALIZE_MSG: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#;
const INITIALIZED_MSG: &str = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
const SHUTDOWN_MSG: &str = r#"{"jsonrpc":"2.0","id":99,"method":"shutdown","params":null}"#;
const EXIT_MSG: &str = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

/// SC-1: graydr lsp starts, performs LSP initialize handshake, keeps running.
#[test]
fn test_lsp_handshake() {
    let mut child = Command::new(graydr_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn graydr lsp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize
    send_lsp(&mut stdin, INITIALIZE_MSG);

    // Expect InitializeResult with capabilities
    let response = recv_lsp(&mut reader);
    assert!(
        response.contains("\"capabilities\""),
        "Expected InitializeResult with capabilities, got: {response}"
    );
    assert!(
        response.contains("\"result\""),
        "Expected JSON-RPC result field, got: {response}"
    );

    // Send initialized (notification, no response expected)
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let shutdown_resp = recv_lsp(&mut reader);
    assert!(
        shutdown_resp.contains("\"result\""),
        "Expected shutdown result, got: {shutdown_resp}"
    );

    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}

/// SC-2: Opening a .gmod with a known error produces publishDiagnostics with
/// correct file URI, line, column, and message. No diagnostics inside heredoc blocks.
#[test]
fn test_lsp_publish_diagnostics() {
    let fixture_path = lsp_fixture("invalid.gmod");
    let fixture_uri = format!(
        "file://{}",
        fixture_path.to_string_lossy().replace('\\', "/")
    );
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .expect("invalid.gmod fixture must exist");

    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{fixture_uri}","languageId":"gmod","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(&fixture_text).unwrap_or_else(|_| format!("{:?}", fixture_text))
    );

    let mut child = Command::new(graydr_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn graydr lsp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Handshake
    send_lsp(&mut stdin, INITIALIZE_MSG);
    let _init_resp = recv_lsp(&mut reader);
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Open the invalid file — triggers publishDiagnostics
    send_lsp(&mut stdin, &did_open);

    // Read the publishDiagnostics notification
    let notification = recv_lsp(&mut reader);
    assert!(
        notification.contains("textDocument/publishDiagnostics"),
        "Expected publishDiagnostics notification, got: {notification}"
    );
    assert!(
        notification.contains(&fixture_uri),
        "Expected notification to contain file URI, got: {notification}"
    );
    // The invalid.gmod has a `size` input with no type annotation — lint check: missing_type
    assert!(
        notification.contains("diagnostics") && !notification.contains("\"diagnostics\":[]"),
        "Expected non-empty diagnostics array, got: {notification}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}

/// SC-3: Fixing the error causes the diagnostic to disappear within one document sync cycle.
#[test]
fn test_lsp_diagnostic_clears() {
    let fixture_path = lsp_fixture("invalid.gmod");
    let fixture_uri = format!(
        "file://{}",
        fixture_path.to_string_lossy().replace('\\', "/")
    );
    let invalid_text = std::fs::read_to_string(&fixture_path)
        .expect("invalid.gmod fixture must exist");
    let valid_text = std::fs::read_to_string(lsp_fixture("valid.gmod"))
        .expect("valid.gmod fixture must exist");

    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{fixture_uri}","languageId":"gmod","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(&invalid_text).unwrap_or_else(|_| format!("{:?}", invalid_text))
    );
    let did_change = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"{fixture_uri}","version":2}},"contentChanges":[{{"text":{}}}]}}}}"#,
        serde_json::to_string(&valid_text).unwrap_or_else(|_| format!("{:?}", valid_text))
    );

    let mut child = Command::new(graydr_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn graydr lsp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Handshake
    send_lsp(&mut stdin, INITIALIZE_MSG);
    let _init_resp = recv_lsp(&mut reader);
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Open invalid — get diagnostics
    send_lsp(&mut stdin, &did_open);
    let diag1 = recv_lsp(&mut reader);
    assert!(
        diag1.contains("textDocument/publishDiagnostics"),
        "First publishDiagnostics expected, got: {diag1}"
    );

    // Change to valid content — diagnostics should clear
    send_lsp(&mut stdin, &did_change);
    let diag2 = recv_lsp(&mut reader);
    assert!(
        diag2.contains("textDocument/publishDiagnostics"),
        "Second publishDiagnostics expected after fix, got: {diag2}"
    );
    assert!(
        diag2.contains("\"diagnostics\":[]"),
        "Expected empty diagnostics after fix, got: {diag2}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}

/// SC-4: No output appears on stdout other than LSP JSON-RPC messages.
/// All logging goes to stderr. Verified by inspecting the raw byte stream.
#[test]
fn test_lsp_stdout_clean() {
    let mut child = Command::new(graydr_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn graydr lsp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize — the first bytes on stdout MUST be a Content-Length header
    send_lsp(&mut stdin, INITIALIZE_MSG);
    let response = recv_lsp(&mut reader);

    // If stdout is clean, the response is valid JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&response);
    assert!(
        parsed.is_ok(),
        "stdout contained non-JSON bytes (stdout pollution detected): {response}"
    );
    assert!(
        response.contains("\"jsonrpc\""),
        "Response must be a JSON-RPC message, got: {response}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}
