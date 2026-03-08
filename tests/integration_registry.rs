// Phase 9: Community Registry integration tests
// All tests are #[ignore] — Wave 0 RED stubs; remove #[ignore] as implementation completes

#[test]
#[ignore]
fn test_publish_command_exists() {
    // graydr publish --module foo.gmod --registry http://localhost exits with
    // a meaningful error (not "unknown subcommand") when registry is unreachable
    todo!()
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
