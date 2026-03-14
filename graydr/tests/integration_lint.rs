// Integration tests for `graydr lint` CLI.
// These tests are #[ignore]d until Plan 03 wires the graydr lint subcommand.
// Remove #[ignore] after Plan 03 is complete.

use std::path::PathBuf;
use std::process::Command;

fn graydr_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // up to workspace root
    path.push("target/debug/graydr");
    path
}

fn fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/lint");
    path.push(name);
    path
}

#[test]
#[ignore]
fn test_lint_warnings_exit_zero() {
    // graydr lint (no --strict) exits 0 even when warnings are present
    let output = Command::new(graydr_bin())
        .args(["lint", &fixture("missing_type.gmod").to_string_lossy()])
        .output()
        .expect("failed to run graydr");
    assert_eq!(output.status.code(), Some(0), "expected exit 0 for warnings without --strict");
}

#[test]
#[ignore]
fn test_lint_strict_exit_code() {
    // graydr lint --strict exits non-zero when warnings are present
    let output = Command::new(graydr_bin())
        .args(["lint", "--strict", &fixture("missing_type.gmod").to_string_lossy()])
        .output()
        .expect("failed to run graydr");
    assert_ne!(output.status.code(), Some(0), "expected non-zero exit for warnings with --strict");
}

#[test]
#[ignore]
fn test_lint_clean_module_exits_zero() {
    // graydr lint --strict exits 0 for a clean module
    let output = Command::new(graydr_bin())
        .args(["lint", "--strict", &fixture("clean.gmod").to_string_lossy()])
        .output()
        .expect("failed to run graydr");
    assert_eq!(output.status.code(), Some(0), "expected exit 0 for clean module with --strict");
}

#[test]
#[ignore]
fn test_lint_sweep_reference_modules() {
    // graydr lint against all v1.2 reference modules exits 0 (no errors; warnings are ok)
    // Reference modules lack type annotations — they will produce missing-type warnings.
    // Without --strict, exit code must be 0.
    let modules_dir = {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.push("modules");
        p
    };
    let gmod_files: Vec<String> = std::fs::read_dir(&modules_dir)
        .expect("modules dir missing")
        .filter_map(|e| e.ok())
        .flat_map(|e| {
            let path = e.path();
            std::fs::read_dir(&path)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|f| f.ok())
                .map(|f| f.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "gmod"))
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(!gmod_files.is_empty(), "no .gmod files found in modules/");
    let mut cmd = Command::new(graydr_bin());
    cmd.arg("lint");
    for f in &gmod_files {
        cmd.arg(f);
    }
    let output = cmd.output().expect("failed to run graydr");
    assert_eq!(
        output.status.code(),
        Some(0),
        "graydr lint (no --strict) must exit 0 against reference modules. stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
