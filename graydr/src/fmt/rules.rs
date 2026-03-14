use hcl_edit::template::HeredocTemplate;
use hcl_edit::visit_mut::VisitMut;

/// Visitor that traverses an HCL body and applies formatting rules.
///
/// The `visit_heredoc_template_mut` method is intentionally a no-op: heredoc
/// content (delimiters, indentation, body bytes) must be preserved byte-for-byte
/// because the `<<-` strip semantics depend on the closing delimiter's indentation.
pub struct FormatVisitor;

impl VisitMut for FormatVisitor {
    /// No-op override — heredoc nodes are treated as opaque and must not be
    /// modified by the formatter. Do NOT call the default super implementation.
    fn visit_heredoc_template_mut(&mut self, _node: &mut HeredocTemplate) {}
}
