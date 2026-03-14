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

    fn fixture(name: &str) -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::fs::read_to_string(format!("{}/tests/fixtures/fmt/{}", manifest_dir, name))
            .unwrap_or_else(|_| panic!("fixture {} not found", name))
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

    // ── Fixture-based tests ──────────────────────────────────────────────────

    /// Formatting an already-canonical file must produce byte-identical output.
    #[test]
    fn test_already_formatted_no_change() {
        let src = fixture("already_formatted.gmod");
        let out = format_source(&src).unwrap();
        assert_eq!(
            out, src,
            "already_formatted.gmod changed after formatting:\n---\n{}\n---",
            out
        );
    }

    /// The bytes between <<-CFN and its closing CFN line must survive formatting
    /// byte-identical.
    #[test]
    fn test_heredoc_preserved_against_fixture() {
        let src = fixture("heredoc.gmod");
        let out = format_source(&src).unwrap();

        let extract_cfn_body = |s: &str| -> String {
            let marker_open = "<<-CFN\n";
            let marker_close = "\n        CFN";
            if let (Some(start), Some(end)) = (s.find(marker_open), s.rfind(marker_close)) {
                s[start + marker_open.len()..end].to_string()
            } else {
                // Try alternate close pattern (indented differently after formatting)
                // Fall back to scanning lines.
                let mut lines = s.lines();
                let mut in_heredoc = false;
                let mut body_lines: Vec<&str> = Vec::new();
                for line in lines.by_ref() {
                    if line.contains("<<-CFN") {
                        in_heredoc = true;
                        continue;
                    }
                    if in_heredoc {
                        if line.trim() == "CFN" {
                            break;
                        }
                        body_lines.push(line);
                    }
                }
                body_lines.join("\n")
            }
        };

        let original_body = extract_cfn_body(&src);
        let formatted_body = extract_cfn_body(&out);
        assert_eq!(
            original_body, formatted_body,
            "heredoc CFN content changed after formatting.\nOriginal:\n{}\nFormatted:\n{}",
            original_body, formatted_body
        );
    }

    /// Every comment line (`//`) from the input must appear in the output.
    #[test]
    fn test_comments_preserved_against_fixture() {
        let src = fixture("comments.gmod");
        let out = format_source(&src).unwrap();

        let comment_lines: Vec<&str> = src
            .lines()
            .filter(|l| l.trim().starts_with("//"))
            .collect();

        for comment in &comment_lines {
            let trimmed = comment.trim();
            assert!(
                out.lines().any(|l| l.trim() == trimmed),
                "comment line lost in output: {:?}\nFull output:\n{}",
                comment,
                out
            );
        }
    }

    /// format_source(format_source(x)) == format_source(x) for all four fixtures.
    #[test]
    fn test_idempotent_all_fixtures() {
        for name in &["basic.gmod", "heredoc.gmod", "comments.gmod", "already_formatted.gmod"] {
            let src = fixture(name);
            assert_idempotent(&src);
        }
    }

    /// After formatting basic.gmod, the `=` signs in the metadata block must be aligned.
    #[test]
    fn test_basic_alignment() {
        let src = fixture("basic.gmod");
        let out = format_source(&src).unwrap();

        // The metadata block contains name (4), version (7), description (11).
        // After alignment: max key is "description" (11 chars).
        // "name" (4) gets 8 spaces padding → "name        = "
        // "version" (7) gets 5 spaces padding → "version     = "
        // "description" (11) gets 1 space padding → "description = "
        assert!(
            out.contains("description ="),
            "Expected aligned '=' for description key.\nFormatted output:\n{}",
            out
        );
        // name should have more padding than description
        assert!(
            out.contains("name ") && out.lines().any(|l| {
                let trimmed = l.trim_start();
                trimmed.starts_with("name") && trimmed.contains("=")
                    && {
                        // Check that there are multiple spaces before =
                        let after_key = &trimmed["name".len()..];
                        after_key.starts_with("  ") // at least 2 spaces before =
                    }
            }),
            "Expected 'name' key to have padding before '='.\nFormatted output:\n{}",
            out
        );
    }
}
