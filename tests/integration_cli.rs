use std::path::Path;
use std::process::Command;

fn cargo_run(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed")
}

/// Path helper: resolve relative to CARGO_MANIFEST_DIR
fn fixture(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures/{}", manifest_dir, name)
}

#[test]
#[ignore] // RED: binary compile subcommand not implemented yet — remove when Plan 02 is done
fn test_compile_end_to_end() {
    let output = cargo_run(&[
        "compile",
        "--template",
        &fixture("sample.gtpl"),
        "--include-path",
        &format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")),
        "--properties",
        &fixture("sample.props.yaml"),
        "-D",
        "primary_db.name=my-bucket",
    ]);
    assert!(
        output.status.success(),
        "compile failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore] // RED: binary validate subcommand not implemented yet — remove when Plan 02 is done
fn test_validate_valid_files() {
    let output = cargo_run(&[
        "validate",
        &fixture("sample.gmod"),
        &fixture("sample.gtpl"),
    ]);
    assert!(
        output.status.success(),
        "validate failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore] // RED: binary validate subcommand not implemented yet — remove when Plan 02 is done
fn test_validate_all_errors_reported() {
    let output = cargo_run(&["validate", &fixture("invalid.gmod")]);
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "expected error messages in stderr, got nothing"
    );
}

#[test]
#[ignore] // RED: binary init subcommand not implemented yet — remove when Plan 03 is done
fn test_init_module_writes_file() {
    let out_path = "/tmp/test_scaffold_graydr.gmod";
    // Clean up from a previous run if present
    let _ = std::fs::remove_file(out_path);

    let output = cargo_run(&["init", "module", "--output", out_path]);
    assert!(
        output.status.success(),
        "init module failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        Path::new(out_path).exists(),
        "expected output file to be written at {}",
        out_path
    );
    let contents = std::fs::read_to_string(out_path).expect("could not read output file");
    assert!(!contents.is_empty(), "expected non-empty scaffold file");
}

#[test]
#[ignore] // RED: binary init subcommand not implemented yet — remove when Plan 03 is done
fn test_init_template_writes_file() {
    let out_path = "/tmp/test_scaffold_graydr.gtpl";
    let _ = std::fs::remove_file(out_path);

    let output = cargo_run(&["init", "template", "--output", out_path]);
    assert!(
        output.status.success(),
        "init template failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        Path::new(out_path).exists(),
        "expected output file to be written at {}",
        out_path
    );
    let contents = std::fs::read_to_string(out_path).expect("could not read output file");
    assert!(!contents.is_empty(), "expected non-empty scaffold file");
}

#[test]
#[ignore] // RED: binary version subcommand not implemented yet — remove when Plan 02 is done
fn test_version_output() {
    let output = cargo_run(&["version"]);
    assert!(
        output.status.success(),
        "version failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0.2.0"),
        "expected version string '0.2.0' in stdout, got: {}",
        stdout
    );
}

#[test]
#[ignore] // RED: binary compile subcommand not implemented yet — remove when Plan 02 is done
fn test_multi_properties_merge() {
    // sample.props.yaml has environment=production
    // sample.props.override.yaml has environment=staging (should win — later-takes-precedence)
    let output = cargo_run(&[
        "compile",
        "--template",
        &fixture("sample.gtpl"),
        "--include-path",
        &format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")),
        "--properties",
        &fixture("sample.props.yaml"),
        "--properties",
        &fixture("sample.props.override.yaml"),
        "-D",
        "primary_db.name=my-bucket",
    ]);
    // The compile output should contain "staging" (from override) not "production" (from base)
    // This assertion turns GREEN only after Plan 02 implements the compile handler.
    assert!(
        output.status.success(),
        "compile failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The merged context should reflect the override value wins; exact assertion depends on
    // how the compile handler surfaces the resolved values in its output.
    // For now assert the process exits 0 and produces some output.
    assert!(
        !stdout.is_empty() || !stderr.is_empty(),
        "expected some output from compile"
    );
}
