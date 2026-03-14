use hcl_edit::expr::Expression;
use hcl_edit::prelude::Decorate;
use hcl_edit::structure::{Body, Structure};
use hcl_edit::template::HeredocTemplate;
use hcl_edit::visit_mut::VisitMut;

/// Visitor that traverses an HCL body and applies formatting rules.
///
/// Rules applied:
/// - Attribute keys are indented 2 spaces per nesting level
/// - In a run of consecutive attributes (no blank line or block between them),
///   `=` signs are column-aligned to the longest key in the run
/// - Heredoc attributes break alignment runs and are left byte-identical
/// - Exactly one blank line between consecutive block entries at the same level
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
    /// where:
    /// - No blank line separates them (prefix of the first item after the start
    ///   does not contain an extra `\n`)
    /// - No `Block` entry or heredoc attribute interrupts the sequence
    ///
    /// A heredoc attribute breaks the run: it is not included in the aligned group
    /// before or after it.
    fn align_attribute_runs(body: &mut Body) {
        // We collect indices of attribute runs, then apply alignment.
        // A "run" is a Vec of indices of consecutive non-heredoc attributes
        // separated by no blank line between them and with no intervening block.
        let n = body.len();
        let mut i = 0;

        while i < n {
            // Find the start of an attribute run at index i.
            let structure = body.get(i).unwrap();
            if !structure.is_attribute() {
                i += 1;
                continue;
            }

            // Check if this attribute is a heredoc — if so, skip it (it breaks runs).
            if is_heredoc_attr(structure) {
                i += 1;
                continue;
            }

            // Gather the run starting at i.
            let run_start = i;
            let mut run_end = i; // inclusive

            let mut j = i + 1;
            while j < n {
                let s = body.get(j).unwrap();
                // A block breaks the run.
                if s.is_block() {
                    break;
                }
                // A heredoc attribute breaks the run.
                if is_heredoc_attr(s) {
                    break;
                }
                // A blank line (the prefix of this entry contains more than one \n after
                // stripping the indent whitespace) breaks the run.
                if has_blank_line_prefix(s) {
                    break;
                }
                run_end = j;
                j += 1;
            }

            // Now we have a run from run_start..=run_end.
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

            // Apply alignment: set key decor suffix to pad to max_key_len + 1 space before =
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

    /// Normalize blank lines between block entries in a body.
    ///
    /// Rules:
    /// - Between two consecutive `Block` entries: ensure exactly one blank line
    ///   (i.e., the second block's prefix contains exactly one extra `\n` beyond
    ///   the indent).
    /// - Between a `Block` and a following entry: ensure a blank line.
    /// - Between an entry and a following `Block`: ensure a blank line.
    /// - Do NOT add blank lines inside attribute-only blocks (inputs/outputs etc.).
    fn normalize_blank_lines(body: &mut Body) {
        let n = body.len();
        if n < 2 {
            return;
        }

        // Determine whether this body contains any blocks at all.
        let has_any_block = (0..n).any(|i| body.get(i).map(|s| s.is_block()).unwrap_or(false));
        // If there are no blocks, it's an attribute-only body — don't touch blank lines.
        if !has_any_block {
            return;
        }

        for i in 1..n {
            let prev_is_block = body.get(i - 1).map(|s| s.is_block()).unwrap_or(false);
            let curr_is_block = body.get(i).map(|s| s.is_block()).unwrap_or(false);

            let needs_blank = prev_is_block || curr_is_block;

            if let Some(curr) = body.get_mut(i) {
                let decor = curr.decor_mut();
                let current_prefix = decor
                    .prefix()
                    .map(|p| &**p)
                    .unwrap_or("")
                    .to_string();

                if needs_blank {
                    // Ensure the prefix has at least one blank line (\n\n).
                    // The prefix of the second item in "block\n\nblock" is "\n".
                    // We want exactly one blank line, so prefix should contain exactly one \n
                    // (the parser already puts the newline at end of previous line, so the
                    // prefix here is the whitespace/newlines between entries).
                    let newline_count = current_prefix.chars().filter(|&c| c == '\n').count();
                    if newline_count == 0 {
                        // No newline at all — add "\n" (one blank line after previous entry's \n)
                        let new_prefix = format!("\n{}", current_prefix);
                        decor.set_prefix(new_prefix);
                    } else if newline_count > 1 {
                        // Too many blank lines — reduce to exactly one blank line.
                        // Strip trailing newlines and rebuild with exactly one \n.
                        let stripped = current_prefix.trim_start_matches('\n');
                        let new_prefix = format!("\n{}", stripped);
                        decor.set_prefix(new_prefix);
                    }
                    // newline_count == 1 is already correct.
                } else {
                    // Between two non-block entries: remove extra blank lines.
                    let newline_count = current_prefix.chars().filter(|&c| c == '\n').count();
                    if newline_count > 1 {
                        let stripped = current_prefix.trim_start_matches('\n');
                        decor.set_prefix(stripped.to_string());
                    }
                }
            }
        }
    }
}

/// Returns true if the structure is an attribute whose value is a heredoc.
fn is_heredoc_attr(s: &Structure) -> bool {
    match s {
        Structure::Attribute(attr) => matches!(&attr.value, Expression::HeredocTemplate(_)),
        _ => false,
    }
}

/// Returns true if the structure's decor prefix contains a blank line
/// (more than one newline character).
fn has_blank_line_prefix(s: &Structure) -> bool {
    let prefix = match s {
        Structure::Attribute(attr) => attr
            .decor()
            .prefix()
            .map(|p| &**p)
            .unwrap_or(""),
        Structure::Block(block) => block
            .decor()
            .prefix()
            .map(|p| &**p)
            .unwrap_or(""),
    };
    prefix.chars().filter(|&c| c == '\n').count() > 1
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
    fn visit_block_mut(&mut self, node: &mut hcl_edit::structure::Block) {
        self.indent_level += 1;
        hcl_edit::visit_mut::visit_block_mut(self, node);
        self.indent_level -= 1;
    }

    /// Override visit_attr_mut to apply indentation and spacing around `=`.
    fn visit_attr_mut(&mut self, mut node: hcl_edit::structure::AttributeMut) {
        let indent = "  ".repeat(self.indent_level);

        // The key's prefix is the indentation (whitespace before the key on its line).
        // We need to preserve any comment lines in the existing prefix, only replacing
        // the final indentation portion (the last line of the prefix).
        let existing_prefix = node
            .key_decor_mut()
            .prefix()
            .map(|p| &**p)
            .unwrap_or("")
            .to_string();

        let new_prefix = replace_last_line_indent(&existing_prefix, &indent);
        node.key_decor_mut().set_prefix(new_prefix);

        // Set suffix to a single space (alignment pass will adjust this later).
        node.key_decor_mut().set_suffix(" ");

        // Set value prefix to a single space (after the `=`).
        node.value_mut().decor_mut().set_prefix(" ");

        // Recurse into the value expression (for non-heredoc values).
        hcl_edit::visit_mut::visit_attr_mut(self, node);
    }

    /// No-op override — heredoc nodes are treated as opaque and must not be
    /// modified by the formatter. Do NOT call the default super implementation.
    fn visit_heredoc_template_mut(&mut self, _node: &mut HeredocTemplate) {}
}

/// Replace the indentation on the last line of a prefix string while preserving
/// comment lines above it.
///
/// The "last line" of a prefix is the part after the final `\n` (or the whole
/// prefix if there is no `\n`). This last line is the indentation whitespace that
/// appears directly before the key on the same line.
///
/// Comment lines (lines containing `//`) are preserved verbatim.
fn replace_last_line_indent(prefix: &str, indent: &str) -> String {
    if prefix.is_empty() {
        return indent.to_string();
    }

    // Find the position of the last newline.
    match prefix.rfind('\n') {
        Some(pos) => {
            // Everything up to and including the last \n is kept (this includes comment lines).
            let before_last = &prefix[..=pos];
            // The part after the last \n is the old indent — replace with new.
            format!("{}{}", before_last, indent)
        }
        None => {
            // No newline: the entire prefix is on the same line as some prior content
            // (unusual). Just replace with indent.
            indent.to_string()
        }
    }
}
