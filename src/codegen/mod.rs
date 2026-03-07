use thiserror::Error;
use crate::ast::module::{CaseArm, MetadataBlock, ModuleDefinition, ValidationSeverity};
use crate::ast::template::ResourceInstance;
use crate::ast::span::Span;
use crate::resolver::context::ResolveContext;
use crate::resolver::error::ResolveError;

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

// ─── private helpers ──────────────────────────────────────────────────────────

fn severity_to_kind(s: &ValidationSeverity) -> IssueKind {
    match s {
        ValidationSeverity::Error => IssueKind::Error,
        ValidationSeverity::Warning => IssueKind::Warning,
        ValidationSeverity::Info => IssueKind::Info,
    }
}

fn resolve_error_to_issue(e: ResolveError, kind: IssueKind) -> ValidationIssue {
    use std::sync::Arc;
    let span = match &e {
        ResolveError::UnresolvedVariable { span, .. } => span.clone(),
        ResolveError::MissingRequiredInput { span, .. } => span.clone(),
        ResolveError::UnknownInput { span, .. } => span.clone(),
        _ => Span {
            file: Arc::from(""),
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
        },
    };
    ValidationIssue {
        span,
        message: e.to_string(),
        severity: kind,
    }
}

// ─── public API ───────────────────────────────────────────────────────────────

/// Render the code template for one case arm, substituting `$variable_name`
/// references via `ctx` before handing the result to Tera.
///
/// IaC `${...}` interpolation sequences are left untouched.
pub fn render_code_block(arm: &CaseArm, ctx: &ResolveContext) -> Result<String, CodegenError> {
    // Sort longest-first to prevent shorter name matching inside longer name
    let mut vars: Vec<_> = arm.variables.iter().collect();
    vars.sort_by(|a, b| b.value.name.len().cmp(&a.value.name.len()));

    let mut rendered = arm.code.value.clone();
    for var_ref in &vars {
        let value = ctx.resolve(&var_ref.value.name, &var_ref.span)
            .map_err(|e| CodegenError::UnresolvedVariable {
                span: var_ref.span.clone(),
                reason: e.to_string(),
            })?;
        rendered = rendered.replace(&format!("${}", var_ref.value.name), value);
    }

    // Empty Tera context — all graydr vars already substituted above.
    // Handles any legitimate {{ }} Tera expressions the module author placed in code.
    let tera_ctx = tera::Context::new();
    tera::Tera::one_off(&rendered, &tera_ctx, false)
        .map_err(|e| CodegenError::TeraRender {
            span: arm.code.span.clone(),
            reason: e.to_string(),
        })
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
    use crate::resolver::validate::{validate_module_inputs, evaluate_validation_rule, RuleOutcome};

    let mut issues: Vec<ValidationIssue> = Vec::new();

    // Stage 1: built-in semantic validation (collect all, never ?)
    for err in validate_module_inputs(resource, module) {
        issues.push(resolve_error_to_issue(err, IssueKind::Error));
    }

    // Stage 2: custom module validation rules
    for rule_sw in &module.validation.value.rules {
        match evaluate_validation_rule(&rule_sw.value, ctx) {
            RuleOutcome::Passed => {}
            RuleOutcome::Failed { message, severity } => {
                issues.push(ValidationIssue {
                    span: rule_sw.value.span.clone(),
                    message,
                    severity: severity_to_kind(&severity),
                });
            }
            RuleOutcome::EvalError { reason } => {
                issues.push(ValidationIssue {
                    span: rule_sw.value.span.clone(),
                    message: format!("validation rule eval error: {}", reason),
                    severity: IssueKind::Error,
                });
            }
        }
    }

    issues
}

/// Format a governance comment block from the module's metadata.
///
/// Returns `Some(String)` if at least one governance field is set;
/// `None` if no governance fields are present (no comment emitted).
pub fn format_governance_comment(meta: &MetadataBlock) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(ref v) = meta.security_tier        { lines.push(format!("# security_tier: {v}")); }
    if let Some(ref v) = meta.compliance_frameworks { lines.push(format!("# compliance_frameworks: {v}")); }
    if let Some(ref v) = meta.cost_tier             { lines.push(format!("# cost_tier: {v}")); }
    if let Some(ref v) = meta.data_classification   { lines.push(format!("# data_classification: {v}")); }
    if let Some(ref v) = meta.disaster_recovery_tier{ lines.push(format!("# disaster_recovery_tier: {v}")); }
    if let Some(v) = meta.approval_required          { lines.push(format!("# approval_required: {v}")); }

    if lines.is_empty() {
        None
    } else {
        Some(format!("# graydr governance metadata\n{}\n", lines.join("\n")))
    }
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
    use std::collections::HashMap;
    use crate::ast::span::Span;
    use crate::ast::common::Spanned;
    use crate::ast::module::{
        CaseArm, CaseBlock, GenerateBlock, InputDecl, InterfaceBlock, MetadataBlock,
        ModuleDefinition, OutputMapping, ValidationBlock, ValidationRule, ValidationSeverity,
    };
    use crate::ast::template::{InputBinding, ResourceInstance};
    use crate::resolver::context::ResolveContext;

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

    fn make_context(pairs: &[(&str, &str)]) -> ResolveContext {
        let mut cli_flags: HashMap<String, String> = HashMap::new();
        for (k, v) in pairs {
            cli_flags.insert(k.to_string(), v.to_string());
        }
        ResolveContext::build(
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            cli_flags,
        )
    }

    fn make_case_arm(code: &str, variable_names: &[&str]) -> CaseArm {
        use crate::ast::common::{SpannedVariable, Variable};
        let variables: Vec<SpannedVariable> = variable_names.iter().map(|name| {
            Spanned {
                value: Variable { name: name.to_string() },
                span: test_span(),
            }
        }).collect();
        CaseArm {
            span: test_span(),
            keys: vec![spanned("aws".to_string())],
            code: spanned(code.to_string()),
            variables,
            outputs: vec![],
        }
    }

    fn make_module_with_rules(rules: Vec<Spanned<ValidationRule>>) -> ModuleDefinition {
        ModuleDefinition {
            span: test_span(),
            name: spanned("storage".to_string()),
            metadata: spanned(MetadataBlock { span: test_span(), ..Default::default() }),
            interface: spanned(InterfaceBlock {
                span: test_span(),
                inputs: vec![],
                outputs: vec![],
            }),
            validation: spanned(ValidationBlock {
                span: test_span(),
                rules,
            }),
            generate: spanned(GenerateBlock {
                span: test_span(),
                cases: vec![],
            }),
        }
    }

    fn make_resource(inputs: Vec<Spanned<InputBinding>>) -> ResourceInstance {
        ResourceInstance {
            span: test_span(),
            name: spanned("my_resource".to_string()),
            module_ref: spanned("storage".to_string()),
            inputs,
            depends_on: vec![],
        }
    }

    fn make_rule(condition: &str, message: &str, severity: ValidationSeverity) -> Spanned<ValidationRule> {
        spanned(ValidationRule {
            span: test_span(),
            condition: spanned(condition.to_string()),
            error_message: spanned(message.to_string()),
            severity,
        })
    }

    // COMP-07: IaC passthrough — `${var.region}` must survive rendering untouched.
    #[test]
    fn test_iac_interpolation_passthrough() {
        // Code contains a bare $bucket_name graydr var AND an IaC ${var.region} sequence.
        // After render_code_block: $bucket_name → "my-bucket"; ${var.region} untouched.
        let code = r#"resource "aws_s3_bucket" "$bucket_name" {
  region = "${var.region}"
}"#;
        let arm = make_case_arm(code, &["bucket_name"]);
        let ctx = make_context(&[("bucket_name", "my-bucket")]);
        let result = render_code_block(&arm, &ctx).expect("render must succeed");
        assert!(
            result.contains("my-bucket"),
            "bucket_name must be substituted — got: {result}"
        );
        assert!(
            result.contains("${var.region}"),
            "IaC ${{var.region}} must pass through untouched — got: {result}"
        );
        assert!(
            !result.contains("$bucket_name"),
            "$bucket_name must be fully replaced — got: {result}"
        );
    }

    // COMP-07: Collect-all pipeline — two failing rules → 2 issues, not 1.
    #[test]
    fn test_pipeline_collects_all_errors() {
        let rule1 = make_rule("provider == \"aws\"", "must be aws", ValidationSeverity::Error);
        let rule2 = make_rule("region == \"us-east-1\"", "must be us-east-1", ValidationSeverity::Warning);
        let module = make_module_with_rules(vec![rule1, rule2]);
        let resource = make_resource(vec![]);
        // ctx resolves provider=azure, region=eu-west-1 → both rules fail
        let ctx = make_context(&[("provider", "azure"), ("region", "eu-west-1")]);
        let issues = run_validation_pipeline(&module, &resource, &ctx);
        assert_eq!(
            issues.len(), 2,
            "two failing rules must produce 2 issues, not fail-fast — got: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
        let has_error = issues.iter().any(|i| i.severity == IssueKind::Error);
        let has_warning = issues.iter().any(|i| i.severity == IssueKind::Warning);
        assert!(has_error, "must have at least one Error severity issue");
        assert!(has_warning, "must have at least one Warning severity issue");
    }

    // COMP-07: Custom module validation rule produces ValidationIssue same format as built-ins.
    #[test]
    fn test_custom_rules_in_pipeline() {
        let rule = make_rule("provider == \"gcp\"", "must use gcp", ValidationSeverity::Error);
        let module = make_module_with_rules(vec![rule]);
        let resource = make_resource(vec![]);
        let ctx = make_context(&[("provider", "aws")]); // rule will fail
        let issues = run_validation_pipeline(&module, &resource, &ctx);
        assert_eq!(issues.len(), 1, "one failing rule → one issue");
        assert_eq!(issues[0].message, "must use gcp");
        assert_eq!(issues[0].severity, IssueKind::Error);
    }

    // COMP-07: Semantic error carries a non-zero span (file:line:col).
    #[test]
    fn test_semantic_error_has_span() {
        use crate::ast::module::InputDecl;
        // Module requires "bucket_name"; resource wires nothing → MissingRequiredInput
        let module = ModuleDefinition {
            span: test_span(),
            name: spanned("storage".to_string()),
            metadata: spanned(MetadataBlock { span: test_span(), ..Default::default() }),
            interface: spanned(InterfaceBlock {
                span: test_span(),
                inputs: vec![spanned(InputDecl {
                    span: test_span(),
                    name: spanned("bucket_name".to_string()),
                    required: true,
                    sensitive: false,
                    default: None,
                    variables: vec![],
                })],
                outputs: vec![],
            }),
            validation: spanned(ValidationBlock { span: test_span(), rules: vec![] }),
            generate: spanned(GenerateBlock { span: test_span(), cases: vec![] }),
        };
        let resource = make_resource(vec![]);
        let ctx = make_context(&[]);
        let issues = run_validation_pipeline(&module, &resource, &ctx);
        assert_eq!(issues.len(), 1, "missing required input must produce one issue");
        // The span must have a non-zero file (from the resource span set in test_span())
        assert_eq!(
            issues[0].span.file.as_ref(), "test.gmod",
            "issue span must carry the source file name"
        );
        assert!(
            issues[0].span.start_line > 0,
            "issue span must have non-zero start_line — got: {}",
            issues[0].span.start_line
        );
    }

    // LANG-11: Governance comment output — security_tier="high" → Some(String) containing key.
    #[test]
    fn test_governance_comment_output() {
        let meta = MetadataBlock {
            span: test_span(),
            security_tier: Some("high".to_string()),
            ..Default::default()
        };
        let result = format_governance_comment(&meta);
        assert!(result.is_some(), "must return Some when security_tier is set");
        let comment = result.unwrap();
        assert!(
            comment.contains("# graydr governance metadata"),
            "must start with header — got: {comment}"
        );
        assert!(
            comment.contains("security_tier: high"),
            "must contain security_tier value — got: {comment}"
        );
    }

    // LANG-11: All six governance fields appear in the comment block when set.
    #[test]
    fn test_all_governance_fields_in_output() {
        let meta = MetadataBlock {
            span: test_span(),
            security_tier: Some("high".to_string()),
            compliance_frameworks: Some("SOC2,PCI".to_string()),
            cost_tier: Some("premium".to_string()),
            data_classification: Some("confidential".to_string()),
            disaster_recovery_tier: Some("tier1".to_string()),
            approval_required: Some(true),
        };
        let result = format_governance_comment(&meta).expect("must return Some when all fields set");
        assert!(result.contains("security_tier: high"), "missing security_tier");
        assert!(result.contains("compliance_frameworks: SOC2,PCI"), "missing compliance_frameworks");
        assert!(result.contains("cost_tier: premium"), "missing cost_tier");
        assert!(result.contains("data_classification: confidential"), "missing data_classification");
        assert!(result.contains("disaster_recovery_tier: tier1"), "missing disaster_recovery_tier");
        assert!(result.contains("approval_required: true"), "missing approval_required");

        // None case — must return None
        let empty_meta = MetadataBlock { span: test_span(), ..Default::default() };
        let none_result = format_governance_comment(&empty_meta);
        assert!(none_result.is_none(), "must return None when all fields are None");
    }
}
