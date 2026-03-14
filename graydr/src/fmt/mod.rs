pub mod rules;

use rules::FormatVisitor;
use hcl_edit::visit_mut::VisitMut;

/// Format an HCL source string and return the canonically-formatted output.
///
/// The formatter operates at the `hcl_edit::Body` layer to preserve comments
/// and whitespace decorations. Heredoc nodes are passed through unmodified.
pub fn format_source(src: &str) -> anyhow::Result<String> {
    let mut body = hcl_edit::parser::parse_body(src)?;
    let mut visitor = FormatVisitor::new();
    visitor.visit_body_mut(&mut body);
    Ok(body.to_string())
}

/// Format an HCL file in-place, or check whether it needs formatting.
///
/// - `check = false`: write canonically-formatted content back to `path`.
/// - `check = true`: do NOT write; just return whether the file would change.
///
/// Returns `Ok(true)` when the file content changed (or would change), `Ok(false)`
/// when it was already canonical.
pub fn format_file(path: &std::path::Path, check: bool) -> anyhow::Result<bool> {
    let original = std::fs::read_to_string(path)?;
    let formatted = format_source(&original)?;
    let changed = original != formatted;
    if changed && !check {
        std::fs::write(path, &formatted)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_idempotent(src: &str) {
        let first = format_source(src).expect("first pass failed");
        let second = format_source(&first).expect("second pass failed");
        assert_eq!(
            first, second,
            "formatter is not idempotent:\nfirst:\n{}\nsecond:\n{}",
            first, second
        );
    }

    #[test]
    fn test_format_source_empty() {
        assert_eq!(format_source("").unwrap(), "");
    }

    #[test]
    fn test_format_source_roundtrip() {
        let src = r#"module "example" {
  metadata {
    name    = "example"
    version = "1.0.0"
  }
}
"#;
        let formatted = format_source(src).unwrap();
        // Must be parseable after formatting — no parse error
        hcl_edit::parser::parse_body(&formatted).expect("formatted output failed to parse");
    }

    #[test]
    fn test_heredoc_preserved() {
        let src = "code = <<-EOT\n  content line\nEOT\n";
        let formatted = format_source(src).unwrap();
        // Extract bytes between <<-EOT\n and \nEOT in both versions
        let extract_heredoc_body = |s: &str| -> String {
            // Find the first newline after <<-EOT, then everything up to \nEOT
            if let (Some(start), Some(end)) = (s.find("<<-EOT\n"), s.rfind("\nEOT")) {
                let inner_start = start + "<<-EOT\n".len();
                s[inner_start..end].to_string()
            } else {
                panic!("heredoc markers not found in: {}", s)
            }
        };
        let original_body = extract_heredoc_body(src);
        let formatted_body = extract_heredoc_body(&formatted);
        assert_eq!(
            original_body, formatted_body,
            "heredoc content was not preserved byte-identical"
        );
    }

    #[test]
    fn test_comments_preserved() {
        let src = "// comment\nname = \"value\"\n";
        let formatted = format_source(src).unwrap();
        assert!(
            formatted.contains("// comment"),
            "comment line was lost in output:\n{}",
            formatted
        );
    }

    #[test]
    fn test_idempotent_empty() {
        assert_idempotent("");
    }
}
