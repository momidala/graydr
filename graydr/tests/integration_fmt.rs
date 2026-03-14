// Integration tests for the `graydr fmt` subcommand.
//
// These tests are marked #[ignore] because they require the `graydr fmt` CLI
// subcommand which is not wired until Plan 03.
//
// Remove #[ignore] after Plan 03 wires graydr fmt subcommand.

use std::process::Command;

fn cargo_run(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed")
}

fn fixture(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures/fmt/{}", manifest_dir, name)
}

/// A file that is already canonically formatted must exit 0 under --check.
#[test]
#[ignore]
// Remove #[ignore] after Plan 03 wires graydr fmt subcommand
fn test_fmt_check_already_formatted() {
    let output = cargo_run(&["fmt", "--check", &fixture("already_formatted.gmod")]);
    assert!(
        output.status.success(),
        "expected exit 0 for already-formatted file, got:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A file with uncanonical formatting must exit non-zero under --check.
#[test]
#[ignore]
// Remove #[ignore] after Plan 03 wires graydr fmt subcommand
fn test_fmt_check_needs_formatting() {
    let output = cargo_run(&["fmt", "--check", &fixture("basic.gmod")]);
    assert!(
        !output.status.success(),
        "expected non-zero exit for unformatted file, got exit 0:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Formatting a file in-place must change its contents and exit 0.
#[test]
#[ignore]
// Remove #[ignore] after Plan 03 wires graydr fmt subcommand
fn test_fmt_inplace() {
    use std::fs;

    // Copy basic.gmod to a temp file
    let original_path = fixture("basic.gmod");
    let original_content = fs::read_to_string(&original_path).expect("could not read basic.gmod");

    let tmp = std::env::temp_dir().join("graydr_test_fmt_inplace.gmod");
    fs::write(&tmp, &original_content).expect("could not write temp file");

    let output = cargo_run(&["fmt", tmp.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "expected exit 0 for in-place fmt, got:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let formatted_content = fs::read_to_string(&tmp).expect("could not read formatted temp file");
    assert_ne!(
        original_content, formatted_content,
        "formatter did not change file contents — expected canonical formatting to differ from input"
    );

    // Clean up
    let _ = fs::remove_file(&tmp);
}
