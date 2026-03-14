use crate::ast::common::{Literal, Spanned, SpannedVariable};
use crate::ast::span::Span;

/// Top-level definition for a `.gmod` module file.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDefinition {
    pub span: Span,
    pub name: Spanned<String>,
    pub metadata: Spanned<MetadataBlock>,
    pub interface: Spanned<InterfaceBlock>,
    pub validation: Spanned<ValidationBlock>,
    pub generate: Spanned<GenerateBlock>,
}

/// `metadata { ... }` block — governance fields added in Phase 5.
///
/// All governance fields are optional; unset fields are `None`.
/// `compliance_frameworks` is stored as a comma-joined string for the community tier
/// (no Vec needed until Phase 7 CLI inspection).
///
/// `Default` is manually implemented (Span has no Default) so callers can write
/// `MetadataBlock { span: my_span, ..Default::default() }` without repeating all fields.
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataBlock {
    pub span: Span,
    pub security_tier: Option<String>,
    pub compliance_frameworks: Option<String>,
    pub cost_tier: Option<String>,
    pub data_classification: Option<String>,
    pub disaster_recovery_tier: Option<String>,
    pub approval_required: Option<bool>,
}

impl Default for MetadataBlock {
    fn default() -> Self {
        Self {
            span: Span {
                file: std::sync::Arc::from(""),
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 0,
            },
            security_tier: None,
            compliance_frameworks: None,
            cost_tier: None,
            data_classification: None,
            disaster_recovery_tier: None,
            approval_required: None,
        }
    }
}

/// `interface { inputs { ... } outputs { ... } }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceBlock {
    pub span: Span,
    pub inputs: Vec<Spanned<InputDecl>>,
    pub outputs: Vec<Spanned<OutputDecl>>,
}

/// Declaration of a single input variable.
///
/// `variables` captures `$var` references found in the `default` value string.
/// `has_type` is true when the input object contains a `type` key (checked by the linter).
#[derive(Debug, Clone, PartialEq)]
pub struct InputDecl {
    pub span: Span,
    pub name: Spanned<String>,
    pub required: bool,
    pub sensitive: bool,
    pub has_type: bool,
    pub default: Option<Spanned<Literal>>,
    pub variables: Vec<SpannedVariable>,
}

/// Declaration of a single output.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputDecl {
    pub span: Span,
    pub name: Spanned<String>,
}

/// `validation { rule "name" { ... } ... }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationBlock {
    pub span: Span,
    pub rules: Vec<Spanned<ValidationRule>>,
}

/// A single validation rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationRule {
    pub span: Span,
    pub condition: Spanned<String>,
    pub error_message: Spanned<String>,
    pub severity: ValidationSeverity,
}

/// Severity level for validation rules.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

/// `generate { case "variable" { ... } ... }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateBlock {
    pub span: Span,
    pub cases: Vec<Spanned<CaseBlock>>,
}

/// A `case "variable_name" { ... }` dispatch block.
///
/// `variable_names` holds the string labels from `case "provider" "engine" { ... }`.
/// Single-variable case is the degenerate one-element case (backward-compatible).
/// Minimum one element is guaranteed by the parser (returns `InvalidCaseLabel` if empty).
#[derive(Debug, Clone, PartialEq)]
pub struct CaseBlock {
    pub span: Span,
    pub variable_names: Vec<Spanned<String>>,
    pub arms: Vec<Spanned<CaseArm>>,
}

/// A single arm within a case block — e.g. `aws { code = <<-EOT ... EOT  outputs { ... } }`.
///
/// `keys` holds the arm key values:
/// - Single-variable form `aws { ... }`: keys = `["aws"]` (from block ident)
/// - Multi-variable form `arm "aws" "aurora" { ... }`: keys = `["aws", "aurora"]` (from block labels)
/// `code` holds the raw heredoc/string content.
/// `variables` is the result of running `scan_variables` on the code content.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseArm {
    pub span: Span,
    pub keys: Vec<Spanned<String>>,
    pub code: Spanned<String>,
    pub variables: Vec<SpannedVariable>,
    pub outputs: Vec<Spanned<OutputMapping>>,
}

/// A name → template mapping in an `outputs { }` block.
///
/// `template` is the raw value string; IaC `${}` interpolation passes through opaque.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputMapping {
    pub span: Span,
    pub name: Spanned<String>,
    pub template: Spanned<String>,
}
