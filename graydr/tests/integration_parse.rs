//! Integration tests for Phase 1 AST parsers.

use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn parse_module_from_fixture() {
    let source = std::fs::read_to_string(fixture_path("sample.gmod"))
        .expect("fixture file must exist");
    let result = graydr::parser::module::parse_module_file(&source, "sample.gmod");
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    let module = result.unwrap();
    assert_eq!(module.name.value, "storage");
    assert!(module.span.start_line >= 1);
    assert!(!module.span.file.is_empty());
    // Verify all required blocks were parsed.
    assert!(module.metadata.span.start_line >= 1);
    assert!(module.interface.span.start_line >= 1);
    assert!(module.validation.span.start_line >= 1);
    assert!(module.generate.span.start_line >= 1);
    // Verify the case block.
    assert!(!module.generate.value.cases.is_empty(), "generate must have at least one case");
    assert_eq!(module.generate.value.cases[0].value.variable_names[0].value, "provider");
}

#[test]
fn parse_template_from_fixture() {
    let source = std::fs::read_to_string(fixture_path("sample.gtpl"))
        .expect("fixture file must exist");
    let result = graydr::parser::template::parse_template_file(&source, "sample.gtpl");
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    let template = result.unwrap();
    assert_eq!(template.name.value, "data-platform");
    assert!(template.span.start_line >= 1);
    assert!(!template.span.file.is_empty());
    // Verify at least one resource instance.
    assert!(!template.resources.is_empty(), "template must have at least one resource");
    let resource = &template.resources[0].value;
    assert_eq!(resource.name.value, "main_storage");
    // Verify parameters block.
    assert!(!template.parameters.value.groups.is_empty(), "template must have parameter groups");
}

#[test]
fn parse_module_variables_not_iac_interpolation() {
    // Verifies $bucket_name → Variable node, ${var.region} → opaque text (not parsed as var).
    // This tests the critical distinction required by LANG-10.
    let source = std::fs::read_to_string(fixture_path("sample.gmod"))
        .expect("fixture file must exist");
    let module = graydr::parser::module::parse_module_file(&source, "sample.gmod")
        .expect("fixture must parse successfully");

    let cases = &module.generate.value.cases;
    assert!(!cases.is_empty(), "generate must have at least one case");

    let aws_arm = cases[0]
        .value
        .arms
        .iter()
        .find(|a| a.value.keys[0].value == "aws")
        .expect("aws arm must exist in sample.gmod");

    let var_names: Vec<&str> = aws_arm
        .value
        .variables
        .iter()
        .map(|v| v.value.name.as_str())
        .collect();

    // $bucket_name in code block must produce a Variable node.
    assert!(
        var_names.contains(&"bucket_name"),
        "bucket_name must be a Variable node — got: {:?}",
        var_names
    );

    // ${var.region} is IaC-native interpolation — must NOT produce a 'var' Variable node.
    assert!(
        !var_names.contains(&"var"),
        "IaC interpolation ${{var.region}} must not produce a Variable node — got: {:?}",
        var_names
    );
}

#[test]
fn parse_error_includes_source_position() {
    // A .gmod file with an unknown top-level block should error with position.
    let bad_source = "module \"broken\" {\n  metadata {}\n  interface {}\n  validation {}\n  generate {}\n  size_configs {}\n}";
    let result = graydr::parser::module::parse_module_file(bad_source, "broken.gmod");
    assert!(result.is_err(), "expected parse error for unknown block");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("broken.gmod"),
        "error must contain filename, got: {}",
        err_msg
    );
    assert!(
        err_msg.chars().any(|c| c.is_ascii_digit()),
        "error must contain a line number, got: {}",
        err_msg
    );
}
