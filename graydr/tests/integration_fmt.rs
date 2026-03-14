// Integration tests for the `graydr fmt` subcommand.
//
// Sweep result (2026-03-14, Plan 03):
//   All 10 v1.2 reference modules and web-app-stack.gtpl passed `graydr fmt --check`
//   with exit 0 after fixing is_heredoc_attr to treat multi-line object-valued
//   attributes as alignment-run breakers (single-line and empty objects remain aligned).

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

/// All 10 v1.2 reference modules and the example template must pass `fmt --check`.
///
/// This is the phase gate: if any module would be reformatted, either the formatter
/// rules need adjusting or the module is not canonical (per RESEARCH.md Pitfall 3).
#[test]
fn test_fmt_sweep_reference_modules() {
    // CARGO_MANIFEST_DIR is graydr/graydr; workspace root is one level up.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .expect("could not find workspace root");

    let modules = [
        "modules/network/network.gmod",
        "modules/relational_db/relational_db.gmod",
        "modules/object_storage/object_storage.gmod",
        "modules/cache/cache.gmod",
        "modules/load_balancer/load_balancer.gmod",
        "modules/dns/dns.gmod",
        "modules/container_registry/container_registry.gmod",
        "modules/kubernetes/kubernetes.gmod",
        "modules/secret_manager/secret_manager.gmod",
        "modules/queue/queue.gmod",
        "examples/web-app-stack.gtpl",
    ];

    let mut args: Vec<String> = vec!["fmt".to_string(), "--check".to_string()];
    for m in &modules {
        args.push(workspace_root.join(m).to_string_lossy().into_owned());
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(&arg_refs)
        .current_dir(manifest_dir)
        .output()
        .expect("cargo run failed");

    assert!(
        output.status.success(),
        "reference module sweep failed — some modules would be reformatted:\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
