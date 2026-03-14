use hcl_edit::expr::Expression;
use hcl_edit::prelude::Decorate;
use hcl_edit::structure::{AttributeMut, Block, Body, Structure};
use hcl_edit::template::HeredocTemplate;
use hcl_edit::visit_mut::VisitMut;

/// Visitor that traverses an HCL body and applies formatting rules.
///
/// Rules applied:
/// - Attribute keys are indented 2 spaces per nesting level
/// - In a run of consecutive attributes (no blank line or block between them),
///   `=` signs are column-aligned to the longest key in the run
/// - Heredoc attributes break alignment runs and are left byte-identical
/// - Exactly one blank line between block entries and adjacent entries at the
///   same nesting level; no blank lines in attribute-only bodies
///
/// The `visit_heredoc_template_mut` method is intentionally a no-op: heredoc
/// content (delimiters, indentation, body bytes) must be preserved byte-for-byte
/// because the `<<-` strip semantics depend on the closing delimiter's indentation.
pub struct FormatVisitor {
    indent_level: usize,
}

impl FormatVisitor {
    pub fn new() -> Self {
        Self { indent_level: 0 }
    }

    /// Post-process a body's attributes to align `=` signs within consecutive
    /// attribute runs.
    ///
    /// A "run" is a maximal sequence of consecutive `Structure::Attribute` entries
    /// where no blank line, block, or heredoc attribute interrupts the sequence.
    /// Blank lines are counted as LEADING `\n` chars in the structure decor prefix
    /// (not inline comment newlines).
    fn align_attribute_runs(body: &mut Body) {
        let n = body.len();
        let mut i = 0;

        while i < n {
            let structure = body.get(i).unwrap();
            if !structure.is_attribute() || is_heredoc_attr(structure) {
                i += 1;
                continue;
            }

            // Gather the run starting at i.
            let run_start = i;
            let mut run_end = i;

            let mut j = i + 1;
            while j < n {
                let s = body.get(j).unwrap();
                if s.is_block() || is_heredoc_attr(s) || has_leading_blank_line(s) {
                    break;
                }
                run_end = j;
                j += 1;
            }

            // Find max key length in this run.
            let max_key_len = (run_start..=run_end)
                .map(|idx| {
                    body.get(idx)
                        .and_then(|s| s.as_attribute())
                        .map(|a| a.key.as_str().len())
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);

            // Apply alignment: set key decor suffix to pad to max_key_len.
            for idx in run_start..=run_end {
                if let Some(attr) = body.get_mut(idx).and_then(|s| s.as_attribute_mut()) {
                    let key_len = attr.key.as_str().len();
                    let padding = max_key_len - key_len + 1;
                    attr.key.decor_mut().set_suffix(" ".repeat(padding));
                }
            }

            i = run_end + 1;
        }
    }

    /// Normalize blank lines between entries in a body.
    ///
    /// Rule: reduce more than one consecutive blank line to exactly one.
    /// Never add blank lines where none existed — that would break idempotency
    /// for canonical files where blank lines between certain block groups are
    /// intentionally omitted (e.g., parameter definitions in `inputs {}`).
    ///
    /// Comment lines in the prefix are preserved — only the leading `\n` count
    /// (the true blank lines before any content) is capped at one.
    fn normalize_blank_lines(body: &mut Body) {
        let n = body.len();
        if n < 2 {
            return;
        }

        for i in 1..n {
            if let Some(curr) = body.get_mut(i) {
                let decor = curr.decor_mut();
                let current_prefix = decor.prefix().map(|p| &**p).unwrap_or("").to_string();

                // Count LEADING \n chars (true blank lines) — not \n within comments.
                let leading_count = count_leading_newlines(&current_prefix);
                let rest = &current_prefix[leading_count..];

                // Only reduce; never add.
                if leading_count > 1 {
                    decor.set_prefix(format!("\n{}", rest));
                }
            }
        }
    }
}

/// Count the number of `\n` characters at the very start of the string
/// (before any non-newline character). These represent true blank lines.
fn count_leading_newlines(s: &str) -> usize {
    s.chars().take_while(|&c| c == '\n').count()
}

/// Replace the trailing indent portion of a prefix (the part after the last `\n`)
/// while preserving everything before the last `\n` (including comment lines).
///
/// If there is no `\n`, the entire prefix is replaced with the new indent.
/// This ensures comment lines are not lost when re-indenting.
fn replace_last_line_indent(prefix: &str, indent: &str) -> String {
    if prefix.is_empty() {
        return indent.to_string();
    }
    match prefix.rfind('\n') {
        Some(pos) => {
            // Keep everything up to and including the last \n, then set new indent.
            format!("{}\n{}", &prefix[..pos], indent)
        }
        None => {
            // No newline — the whole prefix is just whitespace (indent for first entry).
            indent.to_string()
        }
    }
}

/// Returns true if the structure is an attribute that breaks an alignment run.
///
/// Alignment runs are broken by:
/// - Heredoc template values (multi-line, content must be preserved byte-identical)
/// - Multi-line object literals (e.g., `name = {\n  required = true\n}`) where the
///   object body spans multiple lines. Single-line objects (`= { a = 1, b = 2 }`)
///   and empty objects (`= {}`) remain in runs and are aligned with their siblings.
///
/// Multi-line detection: if the first item in the object's body has a decor prefix
/// containing a newline, the object was written across multiple lines in the source.
fn is_heredoc_attr(s: &Structure) -> bool {
    match s {
        Structure::Attribute(attr) => match &attr.value {
            Expression::HeredocTemplate(_) => true,
            Expression::Object(obj) => {
                // Check if the first item's prefix contains a newline —
                // indicating the object spans multiple lines.
                obj.iter().next().map_or(false, |(key, _value)| {
                    key.decor()
                        .prefix()
                        .map(|p| p.contains('\n'))
                        .unwrap_or(false)
                })
            }
            _ => false,
        },
        _ => false,
    }
}

/// Returns true if the structure's decor prefix has at least one leading blank line
/// (i.e., the prefix starts with `\n`).
fn has_leading_blank_line(s: &Structure) -> bool {
    let prefix = match s {
        Structure::Attribute(attr) => attr.decor().prefix().map(|p| &**p).unwrap_or(""),
        Structure::Block(block) => block.decor().prefix().map(|p| &**p).unwrap_or(""),
    };
    count_leading_newlines(prefix) > 0
}

impl VisitMut for FormatVisitor {
    /// Override visit_body_mut to apply post-processing after recursive descent.
    fn visit_body_mut(&mut self, node: &mut Body) {
        // Recurse into all structures first.
        hcl_edit::visit_mut::visit_body_mut(self, node);

        // Post-process: align attribute runs and normalize blank lines.
        Self::align_attribute_runs(node);
        Self::normalize_blank_lines(node);
    }

    /// Override visit_block_mut to track indent level during recursion.
    fn visit_block_mut(&mut self, node: &mut Block) {
        self.indent_level += 1;
        hcl_edit::visit_mut::visit_block_mut(self, node);
        self.indent_level -= 1;
    }

    /// Override visit_attr_mut to apply indentation and spacing around `=`.
    ///
    /// In hcl-edit, the indentation whitespace is stored in the **structure-level**
    /// decor prefix (set by the body parser from the `ws` before the structure),
    /// NOT in the key decor prefix. The structure prefix may contain leading blank
    /// lines (`\n`) and inline comment lines — we preserve those and only replace
    /// the trailing whitespace (the actual indent before the key on the last line).
    fn visit_attr_mut(&mut self, mut node: AttributeMut) {
        let indent = "  ".repeat(self.indent_level);

        // Read the structure-level decor prefix (contains indent + optional leading newlines
        // for blank lines + optional comment lines before the attribute).
        let existing_prefix = node.decor().prefix().map(|p| &**p).unwrap_or("").to_string();

        // Replace only the trailing indent (after last \n), preserving comments and blank lines.
        let new_prefix = replace_last_line_indent(&existing_prefix, &indent);
        node.decor_mut().set_prefix(new_prefix);

        // Key decor suffix = space(s) before =. Set to single space now;
        // the alignment pass in align_attribute_runs will adjust this.
        node.key_decor_mut().set_suffix(" ");

        // Value decor prefix = space after =.
        node.value_mut().decor_mut().set_prefix(" ");

        // Recurse into the value expression (no-op for heredocs due to override below).
        hcl_edit::visit_mut::visit_attr_mut(self, node);
    }

    /// No-op override — heredoc nodes are treated as opaque and must not be
    /// modified by the formatter. Do NOT call the default super implementation.
    fn visit_heredoc_template_mut(&mut self, _node: &mut HeredocTemplate) {}
}
