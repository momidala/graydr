use std::sync::Arc;

use hcl_edit::Span as HclSpan;
use hcl_edit::structure::Body;

use crate::ast::common::Spanned;
use crate::ast::module::{
    CaseArm, CaseBlock, GenerateBlock, InputDecl, InterfaceBlock, MetadataBlock, ModuleDefinition,
    OutputDecl, OutputMapping, ValidationBlock, ValidationRule, ValidationSeverity,
};
use crate::ast::span::{hcl_range_to_graydr, Span};
use crate::parser::error::ParseError;
use crate::parser::variable::scan_variables;

/// Parse a `.gmod` source string into a [`ModuleDefinition`].
///
/// Uses `hcl_edit::parser::parse_body()` to preserve source spans on every node.
/// Returns a [`ParseError`] whose `Display` includes `file:line:col`.
///
/// Expected top-level structure:
/// ```hcl
/// module "name" {
///   metadata { ... }
///   interface { inputs { ... } outputs { ... } }
///   validation { rule "name" { ... } }
///   generate { case "variable" { arm_key { code = <<-EOT ... EOT  outputs { ... } } } }
/// }
/// ```
pub fn parse_module_file(source: &str, file: &str) -> Result<ModuleDefinition, ParseError> {
    let file_arc: Arc<str> = Arc::from(file);

    let body: Body = hcl_edit::parser::parse_body(source).map_err(|e| ParseError::HclParse {
        file: file.to_string(),
        source: Box::new(e),
    })?;

    // Walk ALL top-level structures, find `module`, reject unknowns.
    let mut module_block_opt = None;
    for block in body.blocks() {
        match block.ident.as_str() {
            "module" => {
                module_block_opt = Some(block);
            }
            other => {
                let span = hcl_range_to_graydr(source, block.span().unwrap_or(0..0), &file_arc);
                return Err(ParseError::UnknownBlock {
                    span,
                    name: other.to_string(),
                });
            }
        }
    }

    let module_block = module_block_opt.ok_or_else(|| ParseError::MissingRequiredBlock {
        span: dummy_span(&file_arc),
        block: "module",
        file_type: ".gmod",
    })?;

    let module_span = hcl_range_to_graydr(source, module_block.span().unwrap_or(0..0), &file_arc);

    // Extract module name from first label.
    let module_name = module_block
        .labels
        .first()
        .map(|l| l.as_str().to_owned())
        .ok_or_else(|| ParseError::MissingLabel {
            span: module_span.clone(),
            block: "module".to_string(),
        })?;

    let name = Spanned {
        value: module_name,
        span: module_span.clone(),
    };

    let inner = &module_block.body;

    // Check for unknown top-level blocks inside `module { }`.
    for block in inner.blocks() {
        match block.ident.as_str() {
            "metadata" | "interface" | "validation" | "generate" => {}
            other => {
                let span =
                    hcl_range_to_graydr(source, block.span().unwrap_or(0..0), &file_arc);
                return Err(ParseError::UnknownBlock {
                    span,
                    name: other.to_string(),
                });
            }
        }
    }

    // Parse the four required blocks.
    let metadata = parse_metadata(source, inner, &file_arc)?;
    let interface = parse_interface(source, inner, &file_arc)?;
    let validation = parse_validation(source, inner, &file_arc)?;
    let generate = parse_generate(source, inner, &file_arc)?;

    Ok(ModuleDefinition {
        span: module_span,
        name,
        metadata,
        interface,
        validation,
        generate,
    })
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn dummy_span(file: &Arc<str>) -> Span {
    Span {
        file: file.clone(),
        start_line: 1,
        start_col: 1,
        end_line: 1,
        end_col: 1,
    }
}

/// Get a required block from `body` by name, or return `MissingRequiredBlock`.
fn require_block<'a>(
    _source: &str,
    body: &'a Body,
    name: &'static str,
    file_type: &'static str,
    file: &Arc<str>,
) -> Result<&'a hcl_edit::structure::Block, ParseError> {
    body.blocks()
        .find(|b| b.ident.as_str() == name)
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: dummy_span(file),
            block: name,
            file_type,
        })
}

/// Convert an hcl_edit `Expression` to its raw string content.
///
/// - `Expression::String` → strips surrounding quotes
/// - `Expression::HeredocTemplate` → renders the template content as a string
/// - Anything else → `Display` representation
fn expr_to_string_content(expr: &hcl_edit::expr::Expression) -> String {
    use hcl_edit::expr::Expression;
    match expr {
        Expression::String(s) => s.value().to_owned(),
        Expression::HeredocTemplate(h) => {
            // Render each element in the template to a string.
            template_to_string(&h.template)
        }
        other => other.to_string(),
    }
}

/// Render an hcl_edit `Template` to a string by concatenating its literal
/// and interpolation segments.
fn template_to_string(tpl: &hcl_edit::template::Template) -> String {
    use hcl_edit::template::Element;
    let mut out = String::new();
    for elem in tpl.iter() {
        match elem {
            Element::Literal(raw) => out.push_str(raw.as_str()),
            Element::Interpolation(interp) => {
                // Re-emit as `${...}` — pass through opaque to preserve IaC syntax.
                out.push_str(&format!("${{{}}}", interp.expr));
            }
            Element::Directive(_) => {
                // Directives not expected in code blocks; skip.
            }
        }
    }
    out
}

// ─── metadata ────────────────────────────────────────────────────────────────

fn parse_metadata(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<MetadataBlock>, ParseError> {
    let block = require_block(source, body, "metadata", ".gmod", file)?;
    let span = hcl_range_to_graydr(source, block.span().unwrap_or(0..0), file);

    let mut meta = MetadataBlock { span: span.clone(), ..Default::default() };

    for attr in block.body.attributes() {
        match attr.key.as_str() {
            "security_tier"          => meta.security_tier = Some(expr_to_string_content(&attr.value)),
            "compliance_frameworks"  => meta.compliance_frameworks = Some(expr_to_string_content(&attr.value)),
            "cost_tier"              => meta.cost_tier = Some(expr_to_string_content(&attr.value)),
            "data_classification"    => meta.data_classification = Some(expr_to_string_content(&attr.value)),
            "disaster_recovery_tier" => meta.disaster_recovery_tier = Some(expr_to_string_content(&attr.value)),
            "approval_required"      => meta.approval_required = attr.value.as_bool(),
            _ => {} // description, version, unknowns silently skipped
        }
    }

    Ok(Spanned { value: meta, span })
}

// ─── interface ────────────────────────────────────────────────────────────────

fn parse_interface(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<InterfaceBlock>, ParseError> {
    let block = require_block(source, body, "interface", ".gmod", file)?;
    let span = hcl_range_to_graydr(source, block.span().unwrap_or(0..0), file);

    let mut inputs: Vec<Spanned<InputDecl>> = Vec::new();
    let mut outputs: Vec<Spanned<OutputDecl>> = Vec::new();

    // `inputs { ... }` block — each attribute is an InputDecl.
    if let Some(inputs_block) = block.body.blocks().find(|b| b.ident.as_str() == "inputs") {
        for attr in inputs_block.body.attributes() {
            let attr_span = hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
            let name = attr.key.as_str().to_owned();

            // Value may be an object with `required`, `sensitive`, and `type` fields.
            let (required, sensitive, has_type) = extract_input_flags(&attr.value);

            // Check default value for $variable references.
            let default_str = match &attr.value {
                hcl_edit::expr::Expression::String(s) => Some(s.value().to_owned()),
                _ => None,
            };
            let variables = if let Some(ref s) = default_str {
                scan_variables(s, &attr_span)
            } else {
                Vec::new()
            };

            inputs.push(Spanned {
                value: InputDecl {
                    span: attr_span.clone(),
                    name: Spanned {
                        value: name,
                        span: attr_span.clone(),
                    },
                    required,
                    sensitive,
                    has_type,
                    default: None, // Phase 1: default parsing deferred
                    variables,
                },
                span: attr_span,
            });
        }
    }

    // `outputs { ... }` block — each attribute is an OutputDecl.
    if let Some(outputs_block) = block.body.blocks().find(|b| b.ident.as_str() == "outputs") {
        for attr in outputs_block.body.attributes() {
            let attr_span = hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
            let name = attr.key.as_str().to_owned();
            outputs.push(Spanned {
                value: OutputDecl {
                    span: attr_span.clone(),
                    name: Spanned {
                        value: name,
                        span: attr_span.clone(),
                    },
                },
                span: attr_span,
            });
        }
    }

    Ok(Spanned {
        value: InterfaceBlock {
            span: span.clone(),
            inputs,
            outputs,
        },
        span,
    })
}

/// Extract `required`, `sensitive`, and `has_type` boolean flags from an input object expression.
///
/// Returns `(required, sensitive, has_type)`.
/// `has_type` is true when the object contains a `type` key (regardless of value).
fn extract_input_flags(expr: &hcl_edit::expr::Expression) -> (bool, bool, bool) {
    use hcl_edit::expr::{Expression, ObjectKey};
    if let Expression::Object(obj) = expr {
        let required = obj
            .iter()
            .find(|(k, _)| match k {
                ObjectKey::Ident(id) => id.as_str() == "required",
                _ => false,
            })
            .and_then(|(_, v)| v.expr().as_bool())
            .unwrap_or(false);

        let sensitive = obj
            .iter()
            .find(|(k, _)| match k {
                ObjectKey::Ident(id) => id.as_str() == "sensitive",
                _ => false,
            })
            .and_then(|(_, v)| v.expr().as_bool())
            .unwrap_or(false);

        let has_type = obj.iter().any(|(k, _)| match k {
            ObjectKey::Ident(id) => id.as_str() == "type",
            _ => false,
        });

        (required, sensitive, has_type)
    } else {
        (false, false, false)
    }
}

// ─── validation ───────────────────────────────────────────────────────────────

fn parse_validation(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<ValidationBlock>, ParseError> {
    let block = require_block(source, body, "validation", ".gmod", file)?;
    let span = hcl_range_to_graydr(source, block.span().unwrap_or(0..0), file);

    let mut rules: Vec<Spanned<ValidationRule>> = Vec::new();

    for rule_block in block.body.blocks() {
        if rule_block.ident.as_str() != "rule" {
            continue;
        }
        let rule_span = hcl_range_to_graydr(source, rule_block.span().unwrap_or(0..0), file);

        let condition = rule_block
            .body
            .attributes()
            .find(|a| a.key.as_str() == "condition")
            .map(|a| {
                let s = hcl_range_to_graydr(source, a.span().unwrap_or(0..0), file);
                Spanned {
                    value: expr_to_string_content(&a.value),
                    span: s,
                }
            })
            .unwrap_or_else(|| Spanned {
                value: String::new(),
                span: rule_span.clone(),
            });

        let error_message = rule_block
            .body
            .attributes()
            .find(|a| a.key.as_str() == "error_message")
            .map(|a| {
                let s = hcl_range_to_graydr(source, a.span().unwrap_or(0..0), file);
                Spanned {
                    value: expr_to_string_content(&a.value),
                    span: s,
                }
            })
            .unwrap_or_else(|| Spanned {
                value: String::new(),
                span: rule_span.clone(),
            });

        let severity = rule_block
            .body
            .attributes()
            .find(|a| a.key.as_str() == "severity")
            .map(|a| parse_severity(&expr_to_string_content(&a.value)))
            .unwrap_or(ValidationSeverity::Error);

        rules.push(Spanned {
            value: ValidationRule {
                span: rule_span.clone(),
                condition,
                error_message,
                severity,
            },
            span: rule_span,
        });
    }

    Ok(Spanned {
        value: ValidationBlock {
            span: span.clone(),
            rules,
        },
        span,
    })
}

fn parse_severity(s: &str) -> ValidationSeverity {
    match s.to_lowercase().as_str() {
        "warning" => ValidationSeverity::Warning,
        "info" => ValidationSeverity::Info,
        _ => ValidationSeverity::Error,
    }
}

// ─── generate ─────────────────────────────────────────────────────────────────

fn parse_generate(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<GenerateBlock>, ParseError> {
    let block = require_block(source, body, "generate", ".gmod", file)?;
    let span = hcl_range_to_graydr(source, block.span().unwrap_or(0..0), file);

    let mut cases: Vec<Spanned<CaseBlock>> = Vec::new();

    for case_block in block.body.blocks() {
        if case_block.ident.as_str() != "case" {
            continue;
        }
        let case_span = hcl_range_to_graydr(source, case_block.span().unwrap_or(0..0), file);

        // Collect all case labels as variable names — e.g. `case "provider" "engine" { ... }`
        // produces variable_names = ["provider", "engine"]. Single-variable case produces a
        // one-element Vec (backward-compatible). Empty labels → ParseError.
        let variable_names: Vec<Spanned<String>> = case_block
            .labels
            .iter()
            .map(|l| Spanned {
                value: l.as_str().to_owned(),
                span: case_span.clone(),
            })
            .collect();
        if variable_names.is_empty() {
            return Err(ParseError::InvalidCaseLabel { span: case_span });
        }

        // Arms are sub-blocks within the case block body.
        // Syntax: `arm_key { code = <<-EOT ... EOT  outputs { ... } }`
        let mut arms: Vec<Spanned<CaseArm>> = Vec::new();

        for arm_block in case_block.body.blocks() {
            let arm_span =
                hcl_range_to_graydr(source, arm_block.span().unwrap_or(0..0), file);
            // Backward-compatible arm key extraction:
            // - Single-variable form `aws { ... }`: ident = "aws", labels empty → keys = ["aws"]
            // - Multi-variable form `arm "aws" "aurora" { ... }`: ident = "arm", labels non-empty
            //   → keys = ["aws", "aurora"] (labels carry the key values)
            let arm_keys: Vec<Spanned<String>> = if arm_block.labels.is_empty() {
                vec![Spanned {
                    value: arm_block.ident.as_str().to_owned(),
                    span: arm_span.clone(),
                }]
            } else {
                arm_block
                    .labels
                    .iter()
                    .map(|l| Spanned {
                        value: l.as_str().to_owned(),
                        span: arm_span.clone(),
                    })
                    .collect()
            };

            // Extract `code` attribute — may be a heredoc or string.
            let code_spanned = arm_block
                .body
                .attributes()
                .find(|a| a.key.as_str() == "code")
                .map(|a| {
                    let code_span =
                        hcl_range_to_graydr(source, a.span().unwrap_or(0..0), file);
                    let content = expr_to_string_content(&a.value);
                    Spanned {
                        value: content,
                        span: code_span,
                    }
                })
                .unwrap_or_else(|| Spanned {
                    value: String::new(),
                    span: arm_span.clone(),
                });

            // Scan code content for $variable_name references (exclude ${...} IaC interpolation).
            let variables = scan_variables(&code_spanned.value, &code_spanned.span);

            // Parse `outputs { ... }` sub-block.
            let output_mappings = parse_arm_outputs(source, &arm_block.body, file, &arm_span);

            arms.push(Spanned {
                value: CaseArm {
                    span: arm_span.clone(),
                    keys: arm_keys,
                    code: code_spanned,
                    variables,
                    outputs: output_mappings,
                },
                span: arm_span,
            });
        }

        cases.push(Spanned {
            value: CaseBlock {
                span: case_span.clone(),
                variable_names,
                arms,
            },
            span: case_span,
        });
    }

    Ok(Spanned {
        value: GenerateBlock {
            span: span.clone(),
            cases,
        },
        span,
    })
}

/// Parse the `outputs { key = "value" }` sub-block inside a case arm.
fn parse_arm_outputs(
    source: &str,
    body: &Body,
    file: &Arc<str>,
    fallback_span: &Span,
) -> Vec<Spanned<OutputMapping>> {
    let outputs_block = match body.blocks().find(|b| b.ident.as_str() == "outputs") {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut mappings = Vec::new();
    for attr in outputs_block.body.attributes() {
        let attr_span = hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
        let name = attr.key.as_str().to_owned();
        let template_str = expr_to_string_content(&attr.value);

        mappings.push(Spanned {
            value: OutputMapping {
                span: attr_span.clone(),
                name: Spanned {
                    value: name,
                    span: attr_span.clone(),
                },
                template: Spanned {
                    value: template_str,
                    span: attr_span.clone(),
                },
            },
            span: attr_span,
        });
    }
    let _ = fallback_span;
    mappings
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── valid module fixture ──────────────────────────────────────────────────

    const VALID_GMOD_SOURCE: &str = r#"module "storage" {
  metadata {
    description = "Cross-cloud object storage module"
    version     = "0.1.0"
  }

  interface {
    inputs {
      bucket_name = {
        required  = true
        sensitive = false
      }
    }
    outputs {
      bucket_url = {}
    }
  }

  validation {
    rule "bucket_name_length" {
      condition     = "$bucket_name != \"\""
      error_message = "bucket_name must not be empty"
      severity      = "error"
    }
  }

  generate {
    case "provider" {
      aws {
        code = <<-EOT
          resource "aws_s3_bucket" "$bucket_name" {
            bucket = "$bucket_name"
            region = "${var.region}"
          }
        EOT
        outputs {
          bucket_url = "${aws_s3_bucket.storage.bucket_regional_domain_name}"
        }
      }
      azure {
        code = <<-EOT
          resource "azurerm_storage_account" "$bucket_name" {
            name     = "$bucket_name"
            location = "$region"
          }
        EOT
        outputs {
          bucket_url = "${azurerm_storage_account.storage.primary_blob_endpoint}"
        }
      }
    }
  }
}"#;

    // ── missing generate block ────────────────────────────────────────────────

    const GMOD_MISSING_GENERATE: &str = r#"module "storage" {
  metadata {}
  interface {
    inputs {}
    outputs {}
  }
  validation {}
}"#;

    // ── unknown top-level block ───────────────────────────────────────────────

    const GMOD_WITH_SIZE_CONFIGS: &str = r#"module "storage" {
  metadata {}
  interface {
    inputs {}
    outputs {}
  }
  validation {}
  size_configs {}
  generate {
    case "provider" {
      aws { code = "x" }
    }
  }
}"#;

    // ── case block fixture ────────────────────────────────────────────────────

    const GMOD_WITH_CASE_BLOCK: &str = VALID_GMOD_SOURCE;

    // ── tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_valid_parse_returns_ok() {
        let result = parse_module_file(VALID_GMOD_SOURCE, "test.gmod");
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn test_module_name_is_storage() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        assert_eq!(m.name.value, "storage");
    }

    #[test]
    fn test_all_required_blocks_present() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        // Each block has a non-zero line span.
        assert!(m.metadata.span.start_line >= 1);
        assert!(m.interface.span.start_line >= 1);
        assert!(m.validation.span.start_line >= 1);
        assert!(m.generate.span.start_line >= 1);
    }

    #[test]
    fn test_spans_carry_file_name() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        assert_eq!(m.span.file.as_ref(), "test.gmod");
        assert!(m.span.start_line > 0, "start_line must be > 0");
        assert_eq!(m.metadata.span.file.as_ref(), "test.gmod");
    }

    #[test]
    fn test_unknown_block_returns_error_with_position() {
        let result = parse_module_file(GMOD_WITH_SIZE_CONFIGS, "test.gmod");
        assert!(result.is_err(), "Expected Err for unknown block");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("test.gmod"),
            "error must contain filename — got: {msg}"
        );
        assert!(
            msg.contains("size_configs"),
            "error must name the unknown block — got: {msg}"
        );
        // Must contain a digit (line number).
        assert!(
            msg.chars().any(|c| c.is_ascii_digit()),
            "error must contain a line number — got: {msg}"
        );
    }

    #[test]
    fn test_missing_generate_returns_error() {
        let result = parse_module_file(GMOD_MISSING_GENERATE, "test.gmod");
        assert!(result.is_err(), "Expected Err for missing generate block");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ParseError::MissingRequiredBlock { block: "generate", .. }),
            "Expected MissingRequiredBlock for 'generate', got: {:?}",
            err
        );
    }

    #[test]
    fn test_case_block_variable_name() {
        let m = parse_module_file(GMOD_WITH_CASE_BLOCK, "test.gmod").unwrap();
        let cases = &m.generate.value.cases;
        assert!(!cases.is_empty(), "generate must have at least one case block");
        assert_eq!(cases[0].value.variable_names[0].value, "provider");
    }

    #[test]
    fn test_case_arms_aws_and_azure_present() {
        let m = parse_module_file(GMOD_WITH_CASE_BLOCK, "test.gmod").unwrap();
        let arms = &m.generate.value.cases[0].value.arms;
        assert!(arms.len() >= 2, "expected aws and azure arms, got {}", arms.len());
        let keys: Vec<&str> = arms.iter().map(|a| a.value.keys[0].value.as_str()).collect();
        assert!(keys.contains(&"aws"), "aws arm missing — got: {keys:?}");
        assert!(keys.contains(&"azure"), "azure arm missing — got: {keys:?}");
    }

    #[test]
    fn test_aws_arm_code_contains_heredoc_content() {
        let m = parse_module_file(GMOD_WITH_CASE_BLOCK, "test.gmod").unwrap();
        let aws_arm = m.generate.value.cases[0]
            .value
            .arms
            .iter()
            .find(|a| a.value.keys[0].value == "aws")
            .expect("aws arm must exist");
        assert!(
            !aws_arm.value.code.value.is_empty(),
            "aws arm code must not be empty"
        );
        assert!(
            aws_arm.value.code.value.contains("aws_s3_bucket"),
            "aws arm code must contain 'aws_s3_bucket', got: {}",
            aws_arm.value.code.value
        );
    }

    #[test]
    fn test_aws_arm_variables_has_bucket_name_not_iac_interpolation() {
        let m = parse_module_file(GMOD_WITH_CASE_BLOCK, "test.gmod").unwrap();
        let aws_arm = m.generate.value.cases[0]
            .value
            .arms
            .iter()
            .find(|a| a.value.keys[0].value == "aws")
            .expect("aws arm must exist");

        let var_names: Vec<&str> = aws_arm
            .value
            .variables
            .iter()
            .map(|v| v.value.name.as_str())
            .collect();

        // $bucket_name must be a Variable node.
        assert!(
            var_names.contains(&"bucket_name"),
            "bucket_name must be a Variable node — got: {var_names:?}"
        );

        // ${var.region} must NOT produce a Variable node named "var".
        assert!(
            !var_names.contains(&"var"),
            "IaC interpolation ${{var.region}} must not produce a Variable node — got: {var_names:?}"
        );
    }

    #[test]
    fn test_all_spans_have_nonzero_lines() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        assert!(m.span.start_line > 0);
        assert!(m.metadata.span.start_line > 0);
        assert!(m.interface.span.start_line > 0);
        assert!(m.validation.span.start_line > 0);
        assert!(m.generate.span.start_line > 0);
        for case in &m.generate.value.cases {
            assert!(case.span.start_line > 0);
            for arm in &case.value.arms {
                assert!(arm.span.start_line > 0);
            }
        }
    }

    #[test]
    fn test_interface_inputs_parsed() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        let inputs = &m.interface.value.inputs;
        assert!(!inputs.is_empty(), "interface must have inputs");
        let bucket = inputs.iter().find(|i| i.value.name.value == "bucket_name");
        assert!(bucket.is_some(), "bucket_name input must exist");
        assert!(
            bucket.unwrap().value.required,
            "bucket_name must be required"
        );
    }

    #[test]
    fn test_interface_outputs_parsed() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        let outputs = &m.interface.value.outputs;
        assert!(!outputs.is_empty(), "interface must have outputs");
        assert_eq!(outputs[0].value.name.value, "bucket_url");
    }

    #[test]
    fn test_validation_rules_parsed() {
        let m = parse_module_file(VALID_GMOD_SOURCE, "test.gmod").unwrap();
        let rules = &m.validation.value.rules;
        assert!(!rules.is_empty(), "validation must have rules");
        assert_eq!(rules[0].value.severity, ValidationSeverity::Error);
    }

    /// Validation test: confirm hcl-edit parses `arm "aws" "aurora" { code = "x" }`
    /// with ident = "arm" and two labels ["aws", "aurora"].
    /// This MUST pass before any multi-variable parser changes are committed.
    #[test]
    fn test_multi_label_arm_syntax() {
        let src = r#"arm "aws" "aurora" { code = "x" }"#;
        let body = hcl_edit::parser::parse_body(src)
            .expect("hcl-edit must parse multi-label block syntax");
        let block = body
            .blocks()
            .next()
            .expect("body must contain exactly one block");
        assert_eq!(
            block.ident.as_str(),
            "arm",
            "block ident must be 'arm', got: {}",
            block.ident.as_str()
        );
        let labels: Vec<&str> = block.labels.iter().map(|l| l.as_str()).collect();
        assert_eq!(
            labels.len(),
            2,
            "must have exactly 2 labels, got: {labels:?}"
        );
        assert_eq!(labels[0], "aws", "first label must be 'aws', got: {}", labels[0]);
        assert_eq!(labels[1], "aurora", "second label must be 'aurora', got: {}", labels[1]);
    }

    /// Multi-variable case dispatch: `case "provider" "engine" { arm "aws" "aurora" { ... } }`
    /// must produce variable_names with 2 elements and keys with 2 elements.
    #[test]
    fn test_multi_variable_case_parses() {
        const GMOD_MULTI_VAR: &str = r#"module "storage" {
  metadata {}
  interface {
    inputs {}
    outputs {}
  }
  validation {}
  generate {
    case "provider" "engine" {
      arm "aws" "aurora" {
        code = "x"
      }
    }
  }
}"#;
        let m = parse_module_file(GMOD_MULTI_VAR, "test.gmod").unwrap();
        let cases = &m.generate.value.cases;
        assert_eq!(cases.len(), 1, "must have exactly one case block");
        let case = &cases[0].value;

        // variable_names must have two elements
        assert_eq!(
            case.variable_names.len(),
            2,
            "case must have 2 variable_names, got: {:?}",
            case.variable_names.iter().map(|v| &v.value).collect::<Vec<_>>()
        );
        assert_eq!(
            case.variable_names[0].value, "provider",
            "first variable_name must be 'provider'"
        );
        assert_eq!(
            case.variable_names[1].value, "engine",
            "second variable_name must be 'engine'"
        );

        // arm keys must have two elements
        assert_eq!(case.arms.len(), 1, "must have exactly one arm");
        let arm = &case.arms[0].value;
        assert_eq!(
            arm.keys.len(),
            2,
            "arm must have 2 keys, got: {:?}",
            arm.keys.iter().map(|k| &k.value).collect::<Vec<_>>()
        );
        assert_eq!(arm.keys[0].value, "aws", "first key must be 'aws'");
        assert_eq!(arm.keys[1].value, "aurora", "second key must be 'aurora'");
    }
}
