use thiserror::Error;
use crate::ast::module::{CaseArm, MetadataBlock, ModuleDefinition, ValidationSeverity};
use crate::ast::template::ResourceInstance;
use crate::ast::span::Span;
use crate::resolver::context::ResolveContext;

/// Errors that can occur during code generation / rendering.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("{span}: unresolved variable during rendering: {reason}")]
    UnresolvedVariable { span: Span, reason: String },
    #[error("{span}: Tera render error: {reason}")]
    TeraRender { span: Span, reason: String },
}

/// Severity of a validation issue produced by the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum IssueKind {
    Error,
    Warning,
    Info,
}

/// A single issue emitted by the validation pipeline or code-generation step.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub span: Span,
    pub message: String,
    pub severity: IssueKind,
}

/// Render the code template for one case arm, substituting `$variable_name`
/// references via `ctx` before handing the result to Tera.
///
/// IaC `${...}` interpolation sequences are left untouched.
pub fn render_code_block(arm: &CaseArm, ctx: &ResolveContext) -> Result<String, CodegenError> {
    todo!("Wave 1: pre-substitution then Tera::one_off")
}

/// Run the full validation pipeline for a module + resource pair.
///
/// Collects ALL validation issues (does not fail-fast). Returns every
/// issue found, including semantic errors from input binding checks and
/// rule evaluation failures.
pub fn run_validation_pipeline(
    module: &ModuleDefinition,
    resource: &ResourceInstance,
    ctx: &ResolveContext,
) -> Vec<ValidationIssue> {
    todo!("Wave 1: collect-all validation pipeline")
}

/// Format a governance comment block from the module's metadata.
///
/// Returns `Some(String)` if at least one governance field is set;
/// `None` if no governance fields are present (no comment emitted).
pub fn format_governance_comment(meta: &MetadataBlock) -> Option<String> {
    todo!("Wave 1: governance comment block")
}

/// Assemble the final output string from rendered code, validation results,
/// and the governance comment block.
///
/// Exact signature is finalized in Plan 03 Task 2 once render_code_block is
/// implemented. Body is `todo!()` to avoid committing to parameters prematurely.
pub fn assemble_output() -> String {
    todo!("Wave 2: wire validation + render + governance comment")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::ast::span::Span;
    use crate::ast::common::Spanned;
    use crate::ast::module::MetadataBlock;

    fn test_span() -> Span {
        Span {
            file: Arc::from("test.gmod"),
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 10,
        }
    }

    fn spanned<T>(value: T) -> Spanned<T> {
        Spanned { value, span: test_span() }
    }

    // COMP-07: IaC passthrough — `${var.region}` must survive rendering untouched.
    #[test]
    fn test_iac_interpolation_passthrough() {
        todo!("Wave 1: render_code_block leaves ${{var.region}} untouched")
    }

    // COMP-07: Collect-all pipeline — two failing rules → 2 issues, not 1.
    #[test]
    fn test_pipeline_collects_all_errors() {
        todo!("Wave 1: run_validation_pipeline returns all errors, not fail-fast")
    }

    // COMP-07: Custom module validation rule produces ValidationIssue same format as built-ins.
    #[test]
    fn test_custom_rules_in_pipeline() {
        todo!("Wave 1: custom rules produce ValidationIssue in standard format")
    }

    // COMP-07: Semantic error carries a non-zero span (file:line:col).
    #[test]
    fn test_semantic_error_has_span() {
        todo!("Wave 1: MissingRequiredInput from pipeline carries non-zero span")
    }

    // LANG-11: Governance comment output — security_tier="high" → Some(String) containing key.
    #[test]
    fn test_governance_comment_output() {
        todo!("Wave 1: format_governance_comment returns Some with security_tier: high")
    }

    // LANG-11: All six governance fields appear in the comment block when set.
    #[test]
    fn test_all_governance_fields_in_output() {
        todo!("Wave 1: all six governance fields appear in comment block")
    }
}
