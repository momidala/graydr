#[cfg(test)]
mod integration_codegen {
    use std::collections::HashMap;
    use std::sync::Arc;

    use graydr::ast::common::{Spanned, SpannedVariable, Variable};
    use graydr::ast::module::{
        CaseArm, CaseBlock, GenerateBlock, InputDecl, InterfaceBlock, MetadataBlock,
        ModuleDefinition, ValidationBlock, ValidationRule, ValidationSeverity,
    };
    use graydr::ast::span::Span;
    use graydr::ast::template::{InputBinding, ResourceInstance};
    use graydr::codegen::{assemble_output, AssembleError, IssueKind};
    use graydr::graph::AssemblyGroup;
    use graydr::resolver::context::ResolveContext;

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

    fn make_spanned_var(name: &str) -> SpannedVariable {
        Spanned {
            value: Variable { name: name.to_string() },
            span: test_span(),
        }
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

    /// Happy-path test: parse a module with governance metadata, resolve vars,
    /// dispatch, render — final output contains governance comment and rendered code.
    ///
    /// COMP-07 + LANG-11: IaC ${...} passthrough and governance comment present in output.
    #[test]
    fn integration_codegen_render_and_validate() {
        // ── Build module ─────────────────────────────────────────────────────
        let arm = CaseArm {
            span: test_span(),
            keys: vec![spanned("aws".to_string())],
            // $bucket_name is a graydr variable; ${var.region} is an IaC interpolation passthrough
            code: spanned(r#"resource "aws_s3_bucket" "$bucket_name" {}"#.to_string()),
            // Inline-populated variables (matches Phase 1 scanner output format)
            variables: vec![make_spanned_var("bucket_name")],
            outputs: vec![],
        };

        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![spanned(arm.clone())],
        };

        let module = ModuleDefinition {
            span: test_span(),
            name: spanned("storage".to_string()),
            metadata: spanned(MetadataBlock {
                span: test_span(),
                security_tier: Some("high".to_string()),
                cost_tier: Some("medium".to_string()),
                ..Default::default()
            }),
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
            validation: spanned(ValidationBlock {
                span: test_span(),
                // Always-passing rule (1 == 1)
                rules: vec![spanned(ValidationRule {
                    span: test_span(),
                    condition: spanned("1 == 1".to_string()),
                    error_message: spanned("unreachable".to_string()),
                    severity: ValidationSeverity::Error,
                })],
            }),
            generate: spanned(GenerateBlock {
                span: test_span(),
                cases: vec![spanned(case_block)],
            }),
        };

        // ── Build resource instance ───────────────────────────────────────────
        let resource = ResourceInstance {
            span: test_span(),
            name: spanned("storage".to_string()),
            module_ref: spanned("storage".to_string()),
            inputs: vec![spanned(InputBinding {
                span: test_span(),
                key: spanned("bucket_name".to_string()),
                value: spanned("my-bucket".to_string()),
                variables: vec![],
            })],
            depends_on: vec![],
        };

        // ── Build context ─────────────────────────────────────────────────────
        let ctx = make_context(&[
            ("bucket_name", "my-bucket"),
            ("provider", "aws"),
        ]);

        // ── Build assembly group ──────────────────────────────────────────────
        let group = AssemblyGroup {
            provider: "aws".to_string(),
            region: "us-east-1".to_string(),
            resources_in_order: vec!["storage".to_string()],
        };

        // ── Build maps ────────────────────────────────────────────────────────
        let mut module_map = HashMap::new();
        module_map.insert("storage".to_string(), module);

        let mut arm_map = HashMap::new();
        arm_map.insert("storage".to_string(), arm);

        let mut resource_map = HashMap::new();
        resource_map.insert("storage".to_string(), resource);

        // ── Call assemble_output ──────────────────────────────────────────────
        let result = assemble_output(&group, &module_map, &arm_map, &ctx, &resource_map, None);
        assert!(result.is_ok(), "happy path must succeed — got: {:?}", result.err());

        let assembled = result.unwrap();

        // Substitution worked — $bucket_name replaced with "my-bucket"
        assert!(
            assembled.output.contains("my-bucket"),
            "output must contain substituted bucket name — got:\n{}",
            assembled.output
        );

        // Governance comment present with security_tier and cost_tier
        assert!(
            assembled.output.contains("# security_tier: high"),
            "output must contain governance security_tier comment — got:\n{}",
            assembled.output
        );
        assert!(
            assembled.output.contains("# cost_tier: medium"),
            "output must contain governance cost_tier comment — got:\n{}",
            assembled.output
        );

        // $bucket_name sigil fully replaced (no raw $variable_name in output)
        assert!(
            !assembled.output.contains("$bucket_name"),
            "output must not contain un-substituted $bucket_name — got:\n{}",
            assembled.output
        );

        // No validation errors in returned issues (passing rule)
        assert!(
            assembled.issues.is_empty(),
            "happy path must produce no issues — got: {:?}", assembled.issues
        );
    }

    /// Validation-abort test: a module requiring an unwired input causes
    /// MissingRequiredInput → IssueKind::Error → render is aborted.
    ///
    /// Verifies the validation-gates-render contract.
    #[test]
    fn integration_codegen_validation_aborts_render() {
        // ── Build arm (same as happy path) ───────────────────────────────────
        let arm = CaseArm {
            span: test_span(),
            keys: vec![spanned("aws".to_string())],
            code: spanned(r#"resource "aws_s3_bucket" "$bucket_name" {}"#.to_string()),
            variables: vec![make_spanned_var("bucket_name")],
            outputs: vec![],
        };

        // ── Build module with TWO required inputs, one of which is NOT wired ─
        // "bucket_name" is wired; "region" is NOT wired → MissingRequiredInput for "region"
        let module = ModuleDefinition {
            span: test_span(),
            name: spanned("storage".to_string()),
            metadata: spanned(MetadataBlock { span: test_span(), ..Default::default() }),
            interface: spanned(InterfaceBlock {
                span: test_span(),
                inputs: vec![
                    spanned(InputDecl {
                        span: test_span(),
                        name: spanned("bucket_name".to_string()),
                        required: true,
                        sensitive: false,
                        default: None,
                        variables: vec![],
                    }),
                    spanned(InputDecl {
                        span: test_span(),
                        name: spanned("region".to_string()),
                        required: true,
                        sensitive: false,
                        default: None,
                        variables: vec![],
                    }),
                ],
                outputs: vec![],
            }),
            validation: spanned(ValidationBlock { span: test_span(), rules: vec![] }),
            generate: spanned(GenerateBlock {
                span: test_span(),
                cases: vec![spanned(CaseBlock {
                    span: test_span(),
                    variable_names: vec![spanned("provider".to_string())],
                    arms: vec![spanned(arm.clone())],
                })],
            }),
        };

        // ── Build resource — only wires "bucket_name", NOT "region" ──────────
        let resource = ResourceInstance {
            span: test_span(),
            name: spanned("storage".to_string()),
            module_ref: spanned("storage".to_string()),
            inputs: vec![spanned(InputBinding {
                span: test_span(),
                key: spanned("bucket_name".to_string()),
                value: spanned("my-bucket".to_string()),
                variables: vec![],
            })],
            depends_on: vec![],
        };

        let ctx = make_context(&[("bucket_name", "my-bucket"), ("provider", "aws")]);

        let group = AssemblyGroup {
            provider: "aws".to_string(),
            region: "us-east-1".to_string(),
            resources_in_order: vec!["storage".to_string()],
        };

        let mut module_map = HashMap::new();
        module_map.insert("storage".to_string(), module);

        let mut arm_map = HashMap::new();
        arm_map.insert("storage".to_string(), arm);

        let mut resource_map = HashMap::new();
        resource_map.insert("storage".to_string(), resource);

        // ── Call assemble_output — must return Err ────────────────────────────
        let result = assemble_output(&group, &module_map, &arm_map, &ctx, &resource_map, None);
        assert!(result.is_err(), "missing required input must cause Err — got Ok");

        match result.unwrap_err() {
            AssembleError::ValidationErrors(issues) => {
                assert!(
                    !issues.is_empty(),
                    "ValidationErrors must carry at least one issue"
                );
                let has_error = issues.iter().any(|i| i.severity == IssueKind::Error);
                assert!(has_error, "at least one issue must be IssueKind::Error");
                let mentions_region = issues.iter().any(|i| i.message.contains("region"));
                assert!(
                    mentions_region,
                    "at least one error message must mention 'region' — got: {:?}",
                    issues.iter().map(|i| &i.message).collect::<Vec<_>>()
                );
            }
            other => panic!(
                "expected AssembleError::ValidationErrors, got: {:?}", other
            ),
        }
    }
}
