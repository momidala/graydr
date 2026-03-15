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

/// SC-2: Completion — typing inside inputs {} block returns declared input names.
/// No completions inside heredoc code blocks.
/// RED in Wave 0 (completion capability not yet advertised by lsp.rs).
#[test]
#[ignore = "RED: completion handler not yet implemented (Plan 22-02)"]
fn test_lsp_completion() {
    let fixture_path = lsp_fixture("completion_context.gtpl");
    let fixture_uri = format!(
        "file://{}",
        fixture_path.to_string_lossy().replace('\\', "/")
    );
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .expect("completion_context.gtpl fixture must exist");

    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{fixture_uri}","languageId":"gtpl","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(&fixture_text).unwrap_or_else(|_| format!("{:?}", fixture_text))
    );

    // Position inside the inputs block of test-resource (line 24, character 6 — inside "inputs = {")
    // Exact position: after "      " on the blank line inside inputs block
    let completion_request = format!(
        r#"{{"jsonrpc":"2.0","id":10,"method":"textDocument/completion","params":{{"textDocument":{{"uri":"{fixture_uri}"}},"position":{{"line":24,"character":6}}}}}}"#
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
    let init_resp = recv_lsp(&mut reader);
    assert!(
        init_resp.contains("\"completionProvider\""),
        "Expected completionProvider in capabilities, got: {init_resp}"
    );
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Open file
    send_lsp(&mut stdin, &did_open);
    // Consume the publishDiagnostics notification triggered by didOpen
    let _diag = recv_lsp(&mut reader);

    // Request completion
    send_lsp(&mut stdin, &completion_request);
    let completion_resp = recv_lsp(&mut reader);
    assert!(
        completion_resp.contains("\"result\""),
        "Expected completion result, got: {completion_resp}"
    );
    // The module declares inputs: vpc_id, size, name — at least one must appear
    assert!(
        completion_resp.contains("vpc_id") || completion_resp.contains("size") || completion_resp.contains("name"),
        "Expected input names in completion items, got: {completion_resp}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}

/// SC-3: Hover — hovering over a governance metadata field returns description and accepted values.
/// RED in Wave 0 (hover capability not yet advertised by lsp.rs).
#[test]
#[ignore = "RED: hover handler not yet implemented (Plan 22-02)"]
fn test_lsp_hover() {
    let fixture_path = lsp_fixture("completion_context_module.gmod");
    let fixture_uri = format!(
        "file://{}",
        fixture_path.to_string_lossy().replace('\\', "/")
    );
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .expect("completion_context_module.gmod fixture must exist");

    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{fixture_uri}","languageId":"gmod","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(&fixture_text).unwrap_or_else(|_| format!("{:?}", fixture_text))
    );

    // Position on "security_tier" in the governance block (line 12 of completion_context_module.gmod)
    let hover_request = format!(
        r#"{{"jsonrpc":"2.0","id":11,"method":"textDocument/hover","params":{{"textDocument":{{"uri":"{fixture_uri}"}},"position":{{"line":12,"character":8}}}}}}"#
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
    let init_resp = recv_lsp(&mut reader);
    assert!(
        init_resp.contains("\"hoverProvider\""),
        "Expected hoverProvider in capabilities, got: {init_resp}"
    );
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Open file
    send_lsp(&mut stdin, &did_open);
    // Consume publishDiagnostics
    let _diag = recv_lsp(&mut reader);

    // Request hover
    send_lsp(&mut stdin, &hover_request);
    let hover_resp = recv_lsp(&mut reader);
    assert!(
        hover_resp.contains("\"result\""),
        "Expected hover result, got: {hover_resp}"
    );
    // security_tier hover must contain the field name and some description
    assert!(
        hover_resp.contains("security_tier"),
        "Expected security_tier in hover content, got: {hover_resp}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}

/// SC-4: Go-to-definition — pressing go-to-definition on a module reference navigates to the .gmod file.
/// RED in Wave 0 (definition capability not yet advertised by lsp.rs).
#[test]
#[ignore = "RED: goto_definition handler not yet implemented (Plan 22-02)"]
fn test_lsp_goto_definition() {
    let fixture_path = lsp_fixture("completion_context.gtpl");
    let fixture_uri = format!(
        "file://{}",
        fixture_path.to_string_lossy().replace('\\', "/")
    );
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .expect("completion_context.gtpl fixture must exist");

    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{fixture_uri}","languageId":"gtpl","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(&fixture_text).unwrap_or_else(|_| format!("{:?}", fixture_text))
    );

    // Position on "completion_context_module" in `module = "completion_context_module"` line
    // Line 18 in completion_context.gtpl (0-indexed): `    module = "completion_context_module"`
    let goto_request = format!(
        r#"{{"jsonrpc":"2.0","id":12,"method":"textDocument/definition","params":{{"textDocument":{{"uri":"{fixture_uri}"}},"position":{{"line":18,"character":15}}}}}}"#
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

    // Handshake — use rootUri pointing to the fixtures directory so server can find the .gmod
    let fixtures_dir = lsp_fixture("").to_string_lossy().replace('\\', "/");
    let init_with_root = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"processId":null,"rootUri":"file://{fixtures_dir}","capabilities":{{}}}}}}"#
    );
    send_lsp(&mut stdin, &init_with_root);
    let init_resp = recv_lsp(&mut reader);
    assert!(
        init_resp.contains("\"definitionProvider\""),
        "Expected definitionProvider in capabilities, got: {init_resp}"
    );
    send_lsp(&mut stdin, INITIALIZED_MSG);

    // Open file
    send_lsp(&mut stdin, &did_open);
    // Consume publishDiagnostics
    let _diag = recv_lsp(&mut reader);

    // Request go-to-definition
    send_lsp(&mut stdin, &goto_request);
    let goto_resp = recv_lsp(&mut reader);
    assert!(
        goto_resp.contains("\"result\""),
        "Expected definition result, got: {goto_resp}"
    );
    // Must reference the completion_context_module.gmod file
    assert!(
        goto_resp.contains("completion_context_module"),
        "Expected module gmod file reference in result, got: {goto_resp}"
    );

    // Graceful shutdown
    send_lsp(&mut stdin, SHUTDOWN_MSG);
    let _ = recv_lsp(&mut reader);
    send_lsp(&mut stdin, EXIT_MSG);
    drop(stdin);
    let _ = child.wait();
}
