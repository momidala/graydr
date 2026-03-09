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
fn test_compile_with_registry_coordinate_fetches_module() {
    // SRV-03: RegistryClient can publish then fetch a module via the real server
    // Uses mockito to simulate the registry server response (client-side test).
    // The actual server round-trip is covered by graydr-registry's own integration tests.
    // This test verifies the CLIENT SIDE of SRV-03:
    // fetch_module calls GET /api/v1/modules/{org}/{name}/{version}/content and returns content.

    use graydr::registry::{RegistryClient, RegistryConfig, ModuleCoord};

    let mut server = mockito::Server::new();
    let content = "module \"testmod\" { metadata { version = \"1.0.0\" } }";

    let _m = server
        .mock("GET", "/api/v1/modules/fetchorg/testmod/1.0.0/content")
        .with_status(200)
        .with_body(content)
        .create();

    let coord = ModuleCoord::parse("fetchorg/testmod@1.0.0").unwrap();
    // Clear cache to ensure a real HTTP call
    if let Some(p) = graydr::registry::cache::cache_path(&coord) {
        let _ = std::fs::remove_file(&p);
    }

    let config = RegistryConfig { base_url: server.url(), token: None };
    let client = RegistryClient::new(config);
    let result = client.fetch_module(&coord).unwrap();
    assert_eq!(result, content, "fetch_module must return server content");

    // Cleanup cache
    if let Some(p) = graydr::registry::cache::cache_path(&coord) {
        let _ = std::fs::remove_file(&p);
    }
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
