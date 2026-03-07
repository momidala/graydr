use std::sync::Arc;

use hcl::edit::repr::Span as HclSpan;
use hcl::edit::structure::{Block, Body};

use crate::ast::common::Spanned;
use crate::ast::module::{MetadataBlock, OutputMapping};
use crate::ast::span::{hcl_range_to_graydr, Span};
use crate::ast::template::{
    InputBinding, OutputsBlock, ParameterDecl, ParameterGroup, ParametersBlock, ResourceInstance,
    TemplateDefinition,
};
use crate::parser::error::ParseError;
use crate::parser::variable::scan_variables;

// Known block type names inside a `template` block — anything else is treated as a resource block.
const KNOWN_BLOCKS: &[&str] = &["metadata", "parameters", "outputs"];

/// Parse a `.gtpl` source file into a [`TemplateDefinition`].
///
/// Uses `hcl::edit::parser::parse_body()` to preserve source spans on every node.
/// Returns a [`ParseError`] whose `Display` includes `file:line:col`.
pub fn parse_template_file(source: &str, file: &str) -> Result<TemplateDefinition, ParseError> {
    let file_arc: Arc<str> = Arc::from(file);

    let body: Body =
        hcl::edit::parser::parse_body(source).map_err(|e| ParseError::HclParse {
            file: file.to_string(),
            source: Box::new(e),
        })?;

    // Find the top-level `template "name" { ... }` block.
    let template_block = body
        .blocks()
        .find(|b| b.ident.as_str() == "template")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: dummy_span(&file_arc),
            block: "template",
            file_type: ".gtpl",
        })?;

    let tpl_span = block_span(source, template_block, &file_arc);

    // Extract the quoted-string label as the template name.
    let name_label = template_block
        .labels
        .first()
        .map(|l| l.as_str().to_owned())
        .ok_or_else(|| ParseError::MissingLabel {
            span: tpl_span.clone(),
            block: "template".to_string(),
        })?;

    let name = Spanned {
        value: name_label,
        span: tpl_span.clone(),
    };

    let inner_body = &template_block.body;

    // --- metadata block ---
    let metadata = parse_metadata(source, inner_body, &file_arc)?;

    // --- parameters block ---
    let parameters = parse_parameters(source, inner_body, &file_arc)?;

    // --- resource blocks ---
    let resources = parse_resources(source, inner_body, &file_arc)?;

    // --- outputs block ---
    let outputs = parse_outputs(source, inner_body, &file_arc)?;

    Ok(TemplateDefinition {
        span: tpl_span,
        name,
        metadata,
        parameters,
        resources,
        outputs,
    })
}

fn dummy_span(file: &Arc<str>) -> Span {
    Span {
        file: file.clone(),
        start_line: 1,
        start_col: 1,
        end_line: 1,
        end_col: 1,
    }
}

fn block_span(source: &str, block: &Block, file: &Arc<str>) -> Span {
    hcl_range_to_graydr(source, block.span().unwrap_or(0..0), file)
}

fn parse_metadata(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<MetadataBlock>, ParseError> {
    let block = body
        .blocks()
        .find(|b| b.ident.as_str() == "metadata")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: dummy_span(file),
            block: "metadata",
            file_type: ".gtpl",
        })?;

    let span = block_span(source, block, file);
    Ok(Spanned {
        value: MetadataBlock { span: span.clone() },
        span,
    })
}

fn parse_parameters(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<ParametersBlock>, ParseError> {
    let block = body
        .blocks()
        .find(|b| b.ident.as_str() == "parameters")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: dummy_span(file),
            block: "parameters",
            file_type: ".gtpl",
        })?;

    let span = block_span(source, block, file);

    let mut groups: Vec<Spanned<ParameterGroup>> = Vec::new();

    for group_block in block.body.blocks() {
        let group_span = block_span(source, group_block, file);
        let group_name = group_block.ident.as_str().to_owned();
        let group_name_spanned = Spanned {
            value: group_name,
            span: group_span.clone(),
        };

        let mut params: Vec<Spanned<ParameterDecl>> = Vec::new();

        for attr in group_block.body.attributes() {
            let attr_span = hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
            let param_name = attr.key.as_str().to_owned();
            let value_str = expr_to_string(&attr.value);
            let variables = scan_variables(&value_str, &attr_span);

            params.push(Spanned {
                value: ParameterDecl {
                    span: attr_span.clone(),
                    name: Spanned {
                        value: param_name,
                        span: attr_span.clone(),
                    },
                    variables,
                },
                span: attr_span,
            });
        }

        groups.push(Spanned {
            value: ParameterGroup {
                span: group_span.clone(),
                name: group_name_spanned,
                params,
            },
            span: group_span,
        });
    }

    Ok(Spanned {
        value: ParametersBlock {
            span: span.clone(),
            groups,
        },
        span,
    })
}

fn parse_resources(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Vec<Spanned<ResourceInstance>>, ParseError> {
    let mut resources = Vec::new();

    for block in body.blocks() {
        let type_name = block.ident.as_str();
        if KNOWN_BLOCKS.contains(&type_name) {
            continue;
        }
        // Only `resource "name" { ... }` blocks are resource instances.
        if type_name != "resource" {
            continue;
        }

        let res_span = block_span(source, block, file);

        // The first label is the resource instance name.
        let instance_name = block
            .labels
            .first()
            .map(|l| l.as_str().to_owned())
            .ok_or_else(|| ParseError::MissingLabel {
                span: res_span.clone(),
                block: "resource".to_string(),
            })?;

        let name_spanned = Spanned {
            value: instance_name,
            span: res_span.clone(),
        };

        // `module = "..."` attribute.
        let module_ref = block
            .body
            .attributes()
            .find(|a| a.key.as_str() == "module")
            .map(|a| {
                let a_span = hcl_range_to_graydr(source, a.span().unwrap_or(0..0), file);
                Spanned {
                    value: expr_to_string(&a.value),
                    span: a_span,
                }
            })
            .unwrap_or_else(|| Spanned {
                value: String::new(),
                span: res_span.clone(),
            });

        // `inputs { key = value ... }` block.
        let mut inputs: Vec<Spanned<InputBinding>> = Vec::new();
        if let Some(inputs_block) =
            block.body.blocks().find(|b| b.ident.as_str() == "inputs")
        {
            for attr in inputs_block.body.attributes() {
                let attr_span =
                    hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
                let key = attr.key.as_str().to_owned();
                let value_str = expr_to_string(&attr.value);
                let variables = scan_variables(&value_str, &attr_span);

                inputs.push(Spanned {
                    value: InputBinding {
                        span: attr_span.clone(),
                        key: Spanned {
                            value: key,
                            span: attr_span.clone(),
                        },
                        value: Spanned {
                            value: value_str,
                            span: attr_span.clone(),
                        },
                        variables,
                    },
                    span: attr_span,
                });
            }
        }

        // `depends_on = [...]` attribute — collect as string list.
        let depends_on: Vec<Spanned<String>> = block
            .body
            .attributes()
            .find(|a| a.key.as_str() == "depends_on")
            .map(|a| {
                let a_span = hcl_range_to_graydr(source, a.span().unwrap_or(0..0), file);
                extract_string_list(&a.value, &a_span)
            })
            .unwrap_or_default();

        resources.push(Spanned {
            value: ResourceInstance {
                span: res_span.clone(),
                name: name_spanned,
                module_ref,
                inputs,
                depends_on,
            },
            span: res_span,
        });
    }

    Ok(resources)
}

fn parse_outputs(
    source: &str,
    body: &Body,
    file: &Arc<str>,
) -> Result<Spanned<OutputsBlock>, ParseError> {
    let block = body
        .blocks()
        .find(|b| b.ident.as_str() == "outputs")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: dummy_span(file),
            block: "outputs",
            file_type: ".gtpl",
        })?;

    let span = block_span(source, block, file);

    let mut mappings: Vec<Spanned<OutputMapping>> = Vec::new();
    for attr in block.body.attributes() {
        let attr_span = hcl_range_to_graydr(source, attr.span().unwrap_or(0..0), file);
        let name = attr.key.as_str().to_owned();
        let template_str = expr_to_string(&attr.value);

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

    Ok(Spanned {
        value: OutputsBlock {
            span: span.clone(),
            mappings,
        },
        span,
    })
}

/// Convert an hcl_edit Expression to its raw string representation.
///
/// For quoted string literals, the surrounding double-quotes are stripped.
fn expr_to_string(expr: &hcl::edit::expr::Expression) -> String {
    let raw = expr.to_string();
    // Strip surrounding quotes from string literals.
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        raw[1..raw.len() - 1].to_owned()
    } else {
        raw
    }
}

/// Extract a list of string values from an Expression (e.g. `["a", "b"]`).
fn extract_string_list(
    expr: &hcl::edit::expr::Expression,
    base_span: &Span,
) -> Vec<Spanned<String>> {
    use hcl::edit::expr::Expression;
    match expr {
        Expression::Array(arr) => arr
            .iter()
            .map(|e| {
                let s = expr_to_string(e);
                Spanned {
                    value: s,
                    span: base_span.clone(),
                }
            })
            .collect(),
        _ => vec![Spanned {
            value: expr_to_string(expr),
            span: base_span.clone(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_GTPL_SOURCE: &str = r#"
template "data-platform" {
  metadata {
    description = "Test template"
    version     = "0.1.0"
  }

  parameters {
    primary_db {
      provider = {}
      region   = {}
    }
  }

  resource "main_storage" {
    module = "storage"
    inputs {
      bucket_name = "$primary_db.name"
      region      = "$primary_db.region"
    }
  }

  outputs {
    storage_url = "${main_storage.bucket_url}"
  }
}
"#;

    const GTPL_MISSING_METADATA: &str = r#"
template "no-metadata" {
  parameters {
    db {
      region = {}
    }
  }
  resource "store" {
    module = "storage"
    inputs {}
  }
  outputs {
    url = "http://example.com"
  }
}
"#;

    #[test]
    fn test_parse_valid_template_returns_ok() {
        let result = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl");
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result.err());
    }

    #[test]
    fn test_template_name_is_correct() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        assert_eq!(tpl.name.value, "data-platform");
    }

    #[test]
    fn test_template_spans_carry_file() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        assert_eq!(tpl.span.file.as_ref(), "test.gtpl");
        assert!(tpl.span.start_line > 0, "start_line must be > 0");
    }

    #[test]
    fn test_missing_metadata_returns_error() {
        let result = parse_template_file(GTPL_MISSING_METADATA, "test.gtpl");
        assert!(result.is_err(), "Expected Err for missing metadata");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ParseError::MissingRequiredBlock { block: "metadata", .. }),
            "Expected MissingRequiredBlock for metadata, got: {:?}",
            err
        );
    }

    #[test]
    fn test_resource_instance_name() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        assert!(!tpl.resources.is_empty(), "Expected at least one resource");
        assert_eq!(tpl.resources[0].value.name.value, "main_storage");
    }

    #[test]
    fn test_input_binding_variables_scanned() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        let res = &tpl.resources[0].value;
        // Find region binding: "$primary_db.region" — scanner stops at the dot,
        // so the variable name is "primary_db".
        let region_binding = res
            .inputs
            .iter()
            .find(|i| i.value.key.value == "region")
            .expect("region input binding must exist");
        let vars: Vec<&str> = region_binding
            .value
            .variables
            .iter()
            .map(|v| v.value.name.as_str())
            .collect();
        assert!(
            vars.contains(&"primary_db"),
            "Expected 'primary_db' variable in region binding, got: {:?}",
            vars
        );
    }

    #[test]
    fn test_outputs_parsed() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        let outputs = &tpl.outputs.value;
        assert!(!outputs.mappings.is_empty(), "Expected at least one output mapping");
        assert_eq!(outputs.mappings[0].value.name.value, "storage_url");
    }

    #[test]
    fn test_error_display_contains_file_line_col() {
        let result = parse_template_file(GTPL_MISSING_METADATA, "test.gtpl");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("test.gtpl"),
            "error must contain filename, got: {}",
            msg
        );
        assert!(
            msg.chars().any(|c| c.is_ascii_digit()),
            "error must contain a line number, got: {}",
            msg
        );
    }

    #[test]
    fn test_parameters_groups_parsed() {
        let tpl = parse_template_file(VALID_GTPL_SOURCE, "test.gtpl").unwrap();
        assert!(
            !tpl.parameters.value.groups.is_empty(),
            "Expected at least one parameter group"
        );
        assert_eq!(tpl.parameters.value.groups[0].value.name.value, "primary_db");
    }
}
