use std::sync::Arc;

use hcl::edit::repr::Span as HclSpan;
use hcl::edit::structure::Body;

use crate::ast::common::Spanned;
use crate::ast::fragment::FragmentDefinition;
use crate::ast::span::{hcl_range_to_graydr, Span};
use crate::parser::error::ParseError;
use crate::parser::variable::scan_variables;

/// Parse a `.gfrag` source file into a [`FragmentDefinition`].
///
/// Expects a top-level `fragment "name" { code = <<-EOT ... EOT }` block.
/// Uses `hcl::edit::parser::parse_body()` to preserve source spans.
pub fn parse_fragment_file(source: &str, file: &str) -> Result<FragmentDefinition, ParseError> {
    if source.trim().is_empty() {
        return Err(ParseError::MissingRequiredBlock {
            span: Span {
                file: Arc::from(file),
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
            block: "fragment",
            file_type: ".gfrag",
        });
    }

    let file_arc: Arc<str> = Arc::from(file);

    let body: Body =
        hcl::edit::parser::parse_body(source).map_err(|e| ParseError::HclParse {
            file: file.to_string(),
            source: Box::new(e),
        })?;

    // Find the top-level `fragment "name" { ... }` block.
    let frag_block = body
        .blocks()
        .find(|b| b.ident.as_str() == "fragment")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: Span {
                file: file_arc.clone(),
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
            block: "fragment",
            file_type: ".gfrag",
        })?;

    let frag_span = hcl_range_to_graydr(source, frag_block.span().unwrap_or(0..0), &file_arc);

    // Extract the fragment name from the first label.
    let name_label = frag_block
        .labels
        .first()
        .map(|l| l.as_str().to_owned())
        .ok_or_else(|| ParseError::MissingLabel {
            span: frag_span.clone(),
            block: "fragment".to_string(),
        })?;

    let name = Spanned {
        value: name_label,
        span: frag_span.clone(),
    };

    // Extract the `code` attribute (heredoc or string).
    let code_attr = frag_block
        .body
        .attributes()
        .find(|a| a.key.as_str() == "code")
        .ok_or_else(|| ParseError::MissingRequiredBlock {
            span: frag_span.clone(),
            block: "code",
            file_type: ".gfrag",
        })?;

    let code_span =
        hcl_range_to_graydr(source, code_attr.span().unwrap_or(0..0), &file_arc);
    let code_str = expr_to_string(&code_attr.value);

    // Scan $variable_name references in the code content.
    let variables = scan_variables(&code_str, &code_span);

    let code = Spanned {
        value: code_str,
        span: code_span,
    };

    Ok(FragmentDefinition {
        span: frag_span,
        name,
        code,
        variables,
    })
}

/// Convert an hcl_edit Expression to its raw string content.
///
/// For quoted string literals the surrounding double-quotes are stripped.
/// For heredocs the raw content is returned (hcl-edit includes the EOT markers
/// in the Display output; we strip them for the code field).
fn expr_to_string(expr: &hcl::edit::expr::Expression) -> String {
    use hcl::edit::expr::Expression;
    match expr {
        Expression::HeredocTemplate(heredoc) => {
            // The raw content is accessible via the template's inner content.
            // hcl-edit stores the heredoc body as a RawString.
            heredoc.template.to_string()
        }
        _ => {
            let raw = expr.to_string();
            if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
                raw[1..raw.len() - 1].to_owned()
            } else {
                raw
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_GFRAG_SOURCE: &str = r#"
fragment "aws-bucket" {
  code = <<-EOT
    resource "aws_s3_bucket" "$bucket_name" {
      bucket = "$bucket_name"
      region = "${var.region}"
    }
  EOT
}
"#;

    const GFRAG_NO_CODE: &str = r#"
fragment "no-code" {
  description = "Missing code attribute"
}
"#;

    #[test]
    fn test_parse_minimal_fragment_returns_ok() {
        let result = parse_fragment_file(MINIMAL_GFRAG_SOURCE, "test.gfrag");
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result.err());
    }

    #[test]
    fn test_fragment_name_is_correct() {
        let frag = parse_fragment_file(MINIMAL_GFRAG_SOURCE, "test.gfrag").unwrap();
        assert_eq!(frag.name.value, "aws-bucket");
    }

    #[test]
    fn test_fragment_span_carries_file() {
        let frag = parse_fragment_file(MINIMAL_GFRAG_SOURCE, "test.gfrag").unwrap();
        assert_eq!(frag.span.file.as_ref(), "test.gfrag");
        assert!(frag.span.start_line > 0, "start_line must be > 0");
    }

    #[test]
    fn test_fragment_variables_scanned() {
        let frag = parse_fragment_file(MINIMAL_GFRAG_SOURCE, "test.gfrag").unwrap();
        let names: Vec<&str> = frag.variables.iter().map(|v| v.value.name.as_str()).collect();
        // $bucket_name appears twice; $region does not (${var.region} is IaC interpolation).
        assert!(
            names.contains(&"bucket_name"),
            "Expected 'bucket_name' in variables, got: {:?}",
            names
        );
        // IaC ${var.region} must not appear.
        assert!(
            !names.contains(&"var"),
            "IaC ${{var.region}} must not produce a 'var' variable, got: {:?}",
            names
        );
    }

    #[test]
    fn test_empty_source_returns_error() {
        let result = parse_fragment_file("", "empty.gfrag");
        assert!(result.is_err(), "Expected Err for empty source");
    }

    #[test]
    fn test_missing_code_returns_error() {
        let result = parse_fragment_file(GFRAG_NO_CODE, "test.gfrag");
        assert!(result.is_err(), "Expected Err for missing code attribute");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("test.gfrag"),
            "error must contain filename, got: {}",
            msg
        );
    }

    #[test]
    fn test_no_fragment_block_returns_error() {
        let bad_source = r#"module "storage" { metadata {} }"#;
        let result = parse_fragment_file(bad_source, "test.gfrag");
        assert!(
            result.is_err(),
            "Expected Err for source with no fragment block"
        );
    }
}
