//! Integration tests for Phase 1 AST parsers.
//! Wave 0: These tests are EXPECTED TO FAIL until Wave 3 parser implementations are complete.

use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn parse_module_from_fixture() {
    // Wave 3: graydr::parser::module::parse_module_file() not yet implemented
    // When implemented: should parse sample.gmod into ModuleDefinition with all blocks
    let source = std::fs::read_to_string(fixture_path("sample.gmod"))
        .expect("fixture file must exist");
    // Uncomment when Wave 3 plan 01-03 is complete:
    // let result = graydr::parser::module::parse_module_file(&source, "sample.gmod");
    // assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    // let module = result.unwrap();
    // assert_eq!(module.name.value, "storage");
    // assert!(module.span.start_line == 1);
    let _ = source;
    todo!("Wave 3: implement parse_module_file in src/parser/module.rs")
}

#[test]
fn parse_template_from_fixture() {
    // Wave 3: graydr::parser::template::parse_template_file() not yet implemented
    let source = std::fs::read_to_string(fixture_path("sample.gtpl"))
        .expect("fixture file must exist");
    let _ = source;
    todo!("Wave 3: implement parse_template_file in src/parser/template.rs")
}

#[test]
fn parse_module_variables_not_iac_interpolation() {
    // Wave 3: verifies $bucket_name → Variable node, ${var.region} → opaque text
    // This test documents the critical distinction required by LANG-10
    todo!("Wave 3: implement parse_module_file — then verify variable scan result")
}

#[test]
fn parse_error_includes_source_position() {
    // Wave 3: malformed input should produce ParseError with file:line:col
    todo!("Wave 3: implement parser then verify error format")
}
