use crate::ast::common::{SpannedVariable, Variable};
use crate::ast::span::{byte_offset_to_line_col, Span};
use crate::ast::common::Spanned;
use regex::Regex;
use std::sync::OnceLock;

/// Scan `content` for `$identifier` variable references, excluding IaC-native
/// `${...}` interpolation sequences.
///
/// Each match of `\$([a-zA-Z_][a-zA-Z0-9_]*)` is checked: if the byte
/// immediately after the match is `{`, the match is discarded (it is part of
/// a `${expr}` IaC expression and must pass through opaque).  Otherwise it
/// becomes a `SpannedVariable`.
///
/// Span computation — positions are absolute, derived from `base_span`:
/// - `byte_offset_to_line_col` returns 1-indexed (line, col) within `content`.
/// - Absolute line = `base_span.start_line + line_offset - 1`
/// - Absolute col (same line as base): `base_span.start_col + col_offset - 1`
/// - Absolute col (different line):    `col_offset`
/// - End col = start_col + name.len() as u32 + 1  (the `$` plus identifier)
pub fn scan_variables(content: &str, base_span: &Span) -> Vec<SpannedVariable> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // No lookahead available in the `regex` crate — post-filter instead.
        Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });

    let content_bytes = content.as_bytes();
    let mut result = Vec::new();

    for cap in re.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let name_match = cap.get(1).unwrap();

        // Post-filter: discard if the character immediately after the match is `{`.
        // That makes it IaC-native `${...}` interpolation — not a graydr variable.
        if content_bytes.get(full_match.end()) == Some(&b'{') {
            continue;
        }

        let name = name_match.as_str().to_string();
        let byte_start = full_match.start();

        let (line_offset, col_offset) = byte_offset_to_line_col(content, byte_start);

        let start_line = base_span.start_line + line_offset - 1;
        let start_col = if line_offset == 1 {
            base_span.start_col + col_offset - 1
        } else {
            col_offset
        };
        let end_col = start_col + name.len() as u32 + 1; // +1 for the `$` sigil

        let span = Span {
            file: base_span.file.clone(),
            start_line,
            start_col,
            end_line: start_line,
            end_col,
        };

        result.push(Spanned {
            value: Variable { name },
            span,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn base() -> Span {
        Span {
            file: Arc::from("test.gmod"),
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    #[test]
    fn test_mixed_vars_and_iac_interpolation() {
        let vars = scan_variables(
            "$provider is $region but not ${resource.attr}",
            &base(),
        );
        assert_eq!(vars.len(), 2, "expected exactly 2 variables, got {:?}", vars);
        assert_eq!(vars[0].value.name, "provider");
        assert_eq!(vars[1].value.name, "region");
    }

    #[test]
    fn test_no_vars_only_iac() {
        let vars = scan_variables("no vars here ${expr}", &base());
        assert_eq!(
            vars.len(),
            0,
            "expected no variables (only IaC interpolation), got {:?}",
            vars
        );
    }

    #[test]
    fn test_underscore_and_camel_case() {
        let vars = scan_variables("$_private and $CamelCase", &base());
        assert_eq!(vars.len(), 2, "expected 2 variables, got {:?}", vars);
        assert_eq!(vars[0].value.name, "_private");
        assert_eq!(vars[1].value.name, "CamelCase");
    }

    #[test]
    fn test_iac_dollar_brace_not_parsed() {
        // Terraform-style ${var.region} must not produce a Variable node.
        let vars = scan_variables("${var.region}", &base());
        assert_eq!(
            vars.len(),
            0,
            "IaC interpolation must not produce Variable nodes, got {:?}",
            vars
        );
    }

    #[test]
    fn test_error_display_format() {
        use crate::parser::error::ParseError;
        let span = Span {
            file: Arc::from("test.gmod"),
            start_line: 5,
            start_col: 3,
            end_line: 5,
            end_col: 15,
        };
        let err = ParseError::UnknownBlock {
            span,
            name: "size_configs".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("test.gmod:5:3"),
            "error must contain file:line:col — got: {msg}"
        );
        assert!(
            msg.contains("size_configs"),
            "error must name the unknown block — got: {msg}"
        );
    }

    #[test]
    fn test_variable_adjacent_to_iac() {
        // $bucket_name is a graydr var; ${aws_s3_bucket.$bucket_name...} contains
        // both a ${...} boundary AND a nested $bucket_name — the outer ${...} must
        // be excluded but the standalone $bucket_name references should be found.
        let content = r#"resource "aws_s3_bucket" "$bucket_name" {
  bucket = "$bucket_name"
  region = "${var.region}"
}"#;
        let vars = scan_variables(content, &base());
        // Expect: $bucket_name (line 1), $bucket_name (line 2)
        // NOT: ${var.region} (line 3 — IaC interpolation, filtered out)
        assert_eq!(
            vars.len(),
            2,
            "expected 2 vars ($bucket_name twice), got {:?}",
            vars
        );
        for v in &vars {
            assert_eq!(v.value.name, "bucket_name");
        }
    }
}
