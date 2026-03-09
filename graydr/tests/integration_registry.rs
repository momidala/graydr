// Phase 9: Community Registry integration tests
// All tests are #[ignore] — Wave 0 RED stubs; remove #[ignore] as implementation completes

#[test]
fn test_publish_command_exists() {
    // Verify graydr publish --help is accessible (subcommand exists in CLI).
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_graydr"))
        .args(["publish", "--help"])
        .output()
        .expect("failed to run graydr");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--module"),
        "publish --help should show --module flag; got: {}",
        stdout
    );
    assert!(
        stdout.contains("--registry"),
        "publish --help should show --registry flag; got: {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_compile_with_registry_coordinate_fetches_module() {
    // graydr compile resolves include "org/name@1.0.0" from registry
    todo!()
}

#[test]
#[ignore]
fn test_compile_with_retired_module_errors() {
    // graydr compile with a retired module coordinate produces hard error
    todo!()
}

#[test]
#[ignore]
fn test_cached_module_used_on_second_compile() {
    // second compile re-uses cache; no network call
    todo!()
}
