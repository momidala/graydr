use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use crate::ast::module::{CaseArm, MetadataBlock, ModuleDefinition, ValidationSeverity};
use crate::ast::template::ResourceInstance;
use crate::ast::span::Span;
use crate::resolver::context::ResolveContext;
use crate::resolver::error::ResolveError;
use crate::graph::AssemblyGroup;

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

/// Perform variable substitution and Tera rendering on an arbitrary code string,
/// using the variable declarations from `arm` for resolution.
///
/// This is the core rendering logic shared by both `render_code_block` (which
/// uses `arm.code.value`) and the fragment-expanded path in `assemble_output`
/// (which may provide a different code string after `expand_includes`).
fn render_raw_code(code: &str, arm: &CaseArm, ctx: &ResolveContext) -> Result<String, CodegenError> {
    // Sort longest-first to prevent shorter name matching inside longer name
    let mut vars: Vec<_> = arm.variables.iter().collect();
    vars.sort_by(|a, b| b.value.name.len().cmp(&a.value.name.len()));

    let mut rendered = code.to_string();
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

/// Render the code template for one case arm, substituting `$variable_name`
/// references via `ctx` before handing the result to Tera.
///
/// IaC `${...}` interpolation sequences are left untouched.
pub fn render_code_block(arm: &CaseArm, ctx: &ResolveContext) -> Result<String, CodegenError> {
    render_raw_code(&arm.code.value, arm, ctx)
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

/// The successful result of `assemble_output`.
#[derive(Debug)]
pub struct AssembleResult {
    /// Combined IaC output string (governance comment prepended if metadata present).
    pub output: String,
    /// Warnings and infos from the validation pipeline (errors are never present here —
    /// they abort rendering and are returned as `AssembleError::ValidationErrors`).
    pub issues: Vec<ValidationIssue>,
}

/// Error returned by `assemble_output`.
#[derive(Debug, Error)]
pub enum AssembleError {
    /// One or more `IssueKind::Error` issues — rendering was aborted.
    #[error("validation errors prevented rendering")]
    ValidationErrors(Vec<ValidationIssue>),
    /// A Tera or variable-substitution failure during rendering.
    #[error("render error: {0}")]
    RenderError(#[from] CodegenError),
    /// Fragment expansion failed (circular include, file not found, etc.).
    #[error("fragment expansion error: {0}")]
    FragmentExpansion(#[from] crate::fragment::FragmentError),
}

/// Assemble the final IaC output for one `AssemblyGroup`.
///
/// # Pipeline
///
/// 1. Run `run_validation_pipeline` for every resource in topo order,
///    accumulating ALL issues across all resources.
/// 2. If any accumulated issue has `IssueKind::Error`, abort and return
///    `Err(AssembleError::ValidationErrors(...))` — rendering is skipped.
/// 3. For each resource, optionally run `expand_includes` on the arm's code
///    (when `include_path` is `Some`), then render via `render_raw_code`.
///    Registry-coordinate deferred includes are surfaced as `Warning` issues.
/// 4. Prepend a governance comment from the first resource's module metadata
///    (if any governance fields are set). Using the first module's metadata is
///    correct for the community tier: a single template typically uses one module;
///    multi-module templates surface metadata from the first resource's module.
/// 5. Return `Ok(AssembleResult)` with the combined output and non-error issues.
///
/// # Parameters
///
/// `include_path` — when `Some`, enables the fragment pre-pass: every `include`
/// directive in each arm's code is expanded before Tera rendering. Pass `None`
/// to skip the pre-pass (identical behaviour to Phase 5).
pub fn assemble_output(
    group: &AssemblyGroup,
    module_map: &HashMap<String, ModuleDefinition>,
    arm_map: &HashMap<String, CaseArm>,
    ctx: &ResolveContext,
    resource_map: &HashMap<String, ResourceInstance>,
    include_path: Option<&Path>,
) -> Result<AssembleResult, AssembleError> {
    use std::sync::Arc;

    // ── Stage 1: validation across all resources (collect-all, no fail-fast) ──
    let mut all_issues: Vec<ValidationIssue> = Vec::new();

    for resource_name in &group.resources_in_order {
        if let (Some(module), Some(resource)) = (
            module_map.get(resource_name),
            resource_map.get(resource_name),
        ) {
            let issues = run_validation_pipeline(module, resource, ctx);
            all_issues.extend(issues);
        }
    }

    // ── Stage 2: abort if any Error-severity issue found ──────────────────────
    let has_errors = all_issues.iter().any(|i| i.severity == IssueKind::Error);
    if has_errors {
        let error_issues: Vec<ValidationIssue> = all_issues
            .into_iter()
            .filter(|i| i.severity == IssueKind::Error)
            .collect();
        return Err(AssembleError::ValidationErrors(error_issues));
    }

    // ── Stage 3: render each resource in topo order ───────────────────────────
    let mut rendered_blocks: Vec<String> = Vec::new();

    for resource_name in &group.resources_in_order {
        if let Some(arm) = arm_map.get(resource_name) {
            let code_to_render = if let Some(inc_path) = include_path {
                let source_file = arm.code.span.file.as_ref();
                let (expanded, source_map) = crate::fragment::expand_includes(
                    &arm.code.value,
                    source_file,
                    inc_path,
                    &mut Vec::new(),
                )?;

                // Collect deferred registry coordinates as Warning issues.
                // expand_includes marks deferred entries with source_file "<deferred:…>".
                for entry in &source_map.entries {
                    if entry.source_file.starts_with("<deferred:") {
                        // Extract coordinate from marker "<deferred:org/name@1>".
                        let coordinate = entry.source_file
                            .trim_start_matches("<deferred:")
                            .trim_end_matches('>');
                        all_issues.push(ValidationIssue {
                            span: Span {
                                file: Arc::from(source_file),
                                start_line: 0,
                                start_col: 0,
                                end_line: 0,
                                end_col: 0,
                            },
                            message: format!(
                                "registry coordinate '{}' deferred — registry not available in community tier",
                                coordinate
                            ),
                            severity: IssueKind::Warning,
                        });
                    }
                }

                expanded
            } else {
                arm.code.value.clone()
            };

            let rendered = render_raw_code(&code_to_render, arm, ctx)?;
            rendered_blocks.push(rendered);
        }
    }

    // ── Stage 4: governance comment from first resource's module ──────────────
    let governance_comment = group.resources_in_order
        .first()
        .and_then(|name| module_map.get(name))
        .and_then(|module| format_governance_comment(&module.metadata.value));

    // ── Stage 5: combine output ───────────────────────────────────────────────
    let code_output = rendered_blocks.join("\n");
    let output = match governance_comment {
        Some(comment) => format!("{}{}", comment, code_output),
        None => code_output,
    };

    // non-error issues (warnings, infos) pass through to caller
    let non_error_issues: Vec<ValidationIssue> = all_issues
        .into_iter()
        .filter(|i| i.severity != IssueKind::Error)
        .collect();

    Ok(AssembleResult {
        output,
        issues: non_error_issues,
    })
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

    // FRAG-01: assemble_output with include_path=None and no includes → no regression.
    #[test]
    fn test_assemble_output_none_include_path_no_regression() {
        use crate::graph::AssemblyGroup;
        // A simple arm with no include directives — None include_path must behave like Phase 5.
        let code = r#"resource "aws_s3_bucket" "$bucket_name" {}"#;
        let arm = make_case_arm(code, &["bucket_name"]);
        let ctx = make_context(&[("bucket_name", "my-bucket")]);

        let mut arm_map = HashMap::new();
        arm_map.insert("res1".to_string(), arm);

        let module = make_module_with_rules(vec![]);
        let mut module_map = HashMap::new();
        module_map.insert("res1".to_string(), module);

        let resource = make_resource(vec![]);
        let mut resource_map = HashMap::new();
        resource_map.insert("res1".to_string(), resource);

        let group = AssemblyGroup {
            provider: "aws".to_string(),
            region: "us-east-1".to_string(),
            resources_in_order: vec!["res1".to_string()],
        };

        let result = assemble_output(&group, &module_map, &arm_map, &ctx, &resource_map, None)
            .expect("assemble_output with no includes must succeed");
        assert!(
            result.output.contains("my-bucket"),
            "output must contain substituted variable value; got: {:?}",
            result.output
        );
    }

    // FRAG-03: AssembleError::FragmentExpansion variant wraps FragmentError.
    #[test]
    fn test_assemble_error_fragment_expansion_variant() {
        use crate::graph::AssemblyGroup;
        use std::path::PathBuf;

        // An arm whose code tries to include a non-existent file → FragmentExpansion error.
        let code = r#"include "nonexistent_file.gfrag""#;
        let arm = make_case_arm(code, &[]);
        let ctx = make_context(&[]);

        let mut arm_map = HashMap::new();
        arm_map.insert("res1".to_string(), arm);

        let module = make_module_with_rules(vec![]);
        let mut module_map = HashMap::new();
        module_map.insert("res1".to_string(), module);

        let resource = make_resource(vec![]);
        let mut resource_map = HashMap::new();
        resource_map.insert("res1".to_string(), resource);

        let group = AssemblyGroup {
            provider: "aws".to_string(),
            region: "us-east-1".to_string(),
            resources_in_order: vec!["res1".to_string()],
        };

        // Use a temp dir as include_path so the file won't be found.
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures");
        let result = assemble_output(
            &group, &module_map, &arm_map, &ctx, &resource_map,
            Some(&PathBuf::from("/nonexistent_include_path")),
        );
        // Must fail — include path doesn't contain "nonexistent_file.gfrag".
        assert!(result.is_err(), "assemble_output must fail when include file is not found");
        let err = result.unwrap_err();
        assert!(
            matches!(err, AssembleError::FragmentExpansion(_)),
            "error must be AssembleError::FragmentExpansion; got: {:?}",
            err
        );
        let _ = tmp; // suppress unused warning
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
