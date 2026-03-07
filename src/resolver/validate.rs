use crate::ast::module::ModuleDefinition;
use crate::ast::template::ResourceInstance;
use crate::resolver::error::ResolveError;
use crate::resolver::context::ResolveContext;
use crate::ast::module::{ValidationRule, ValidationSeverity};

pub enum RuleOutcome {
    Passed,
    Failed { message: String, severity: ValidationSeverity },
    EvalError { reason: String },
}

pub fn validate_module_inputs(
    _resource: &ResourceInstance,
    _module: &ModuleDefinition,
) -> Vec<ResolveError> {
    todo!("implemented in plan 02-04")
}

pub fn evaluate_validation_rule(
    _rule: &ValidationRule,
    _context: &ResolveContext,
) -> RuleOutcome {
    todo!("implemented in plan 02-04")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use crate::ast::span::Span;
    use crate::ast::common::Spanned;
    use crate::ast::module::{
        InterfaceBlock, InputDecl, OutputDecl, ValidationBlock,
        MetadataBlock, GenerateBlock, ModuleDefinition,
    };
    use crate::ast::template::{ResourceInstance, InputBinding};

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

    fn make_input_decl(name: &str, required: bool) -> Spanned<InputDecl> {
        spanned(InputDecl {
            span: test_span(),
            name: spanned(name.to_string()),
            required,
            sensitive: false,
            default: None,
            variables: vec![],
        })
    }

    fn make_module(inputs: Vec<Spanned<InputDecl>>) -> ModuleDefinition {
        ModuleDefinition {
            span: test_span(),
            name: spanned("storage".to_string()),
            metadata: spanned(MetadataBlock { span: test_span() }),
            interface: spanned(InterfaceBlock {
                span: test_span(),
                inputs,
                outputs: vec![],
            }),
            validation: spanned(ValidationBlock {
                span: test_span(),
                rules: vec![],
            }),
            generate: spanned(crate::ast::module::GenerateBlock {
                span: test_span(),
                cases: vec![],
            }),
        }
    }

    fn make_binding(key: &str) -> Spanned<InputBinding> {
        spanned(InputBinding {
            span: test_span(),
            key: spanned(key.to_string()),
            value: spanned("x".to_string()),
            variables: vec![],
        })
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

    // --- validate_module_inputs tests ---

    #[test]
    fn test_required_input_missing() {
        let module = make_module(vec![make_input_decl("name", true)]);
        let resource = make_resource(vec![]);
        let errors = validate_module_inputs(&resource, &module);
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            ResolveError::MissingRequiredInput { module, input, .. } => {
                assert_eq!(module, "storage");
                assert_eq!(input, "name");
            }
            other => panic!("expected MissingRequiredInput, got: {:?}", other),
        }
    }

    #[test]
    fn test_optional_input_unwired() {
        let module = make_module(vec![make_input_decl("name", false)]);
        let resource = make_resource(vec![]);
        let errors = validate_module_inputs(&resource, &module);
        assert!(errors.is_empty(), "optional unwired input should not produce an error");
    }

    #[test]
    fn test_unknown_input() {
        let module = make_module(vec![make_input_decl("name", false)]);
        let resource = make_resource(vec![
            make_binding("name"),
            make_binding("phantom"),
        ]);
        let errors = validate_module_inputs(&resource, &module);
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            ResolveError::UnknownInput { module, input, .. } => {
                assert_eq!(module, "storage");
                assert_eq!(input, "phantom");
            }
            other => panic!("expected UnknownInput, got: {:?}", other),
        }
    }

    #[test]
    fn test_valid_wiring() {
        let module = make_module(vec![
            make_input_decl("name", true),
            make_input_decl("region", false),
        ]);
        let resource = make_resource(vec![
            make_binding("name"),
            make_binding("region"),
        ]);
        let errors = validate_module_inputs(&resource, &module);
        assert!(errors.is_empty(), "valid wiring should not produce errors");
    }

    #[test]
    fn test_multiple_errors() {
        // module: required "name", optional "region"
        // resource: wires "ghost" (unknown) but not "name" (required)
        let module = make_module(vec![
            make_input_decl("name", true),
            make_input_decl("region", false),
        ]);
        let resource = make_resource(vec![make_binding("ghost")]);
        let errors = validate_module_inputs(&resource, &module);
        assert_eq!(errors.len(), 2, "expected 2 errors (missing required + unknown), got: {:?}", errors.len());
        let has_missing = errors.iter().any(|e| matches!(e, ResolveError::MissingRequiredInput { input, .. } if input == "name"));
        let has_unknown = errors.iter().any(|e| matches!(e, ResolveError::UnknownInput { input, .. } if input == "ghost"));
        assert!(has_missing, "expected MissingRequiredInput for 'name'");
        assert!(has_unknown, "expected UnknownInput for 'ghost'");
    }

    // --- evaluate_validation_rule tests ---

    #[test]
    fn test_rule_passes() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("provider == \"aws\"".to_string()),
            error_message: spanned("must use aws".to_string()),
            severity: ValidationSeverity::Error,
        };
        let ctx = make_context(&[("provider", "aws")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        assert!(matches!(outcome, RuleOutcome::Passed));
    }

    #[test]
    fn test_rule_error_severity() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("provider == \"aws\"".to_string()),
            error_message: spanned("must use aws".to_string()),
            severity: ValidationSeverity::Error,
        };
        let ctx = make_context(&[("provider", "azure")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        match outcome {
            RuleOutcome::Failed { severity, message } => {
                assert!(matches!(severity, ValidationSeverity::Error));
                assert_eq!(message, "must use aws");
            }
            _ => panic!("expected Failed with Error severity"),
        }
    }

    #[test]
    fn test_rule_warning_severity() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("provider == \"aws\"".to_string()),
            error_message: spanned("prefer aws".to_string()),
            severity: ValidationSeverity::Warning,
        };
        let ctx = make_context(&[("provider", "azure")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        assert!(matches!(outcome, RuleOutcome::Failed { severity: ValidationSeverity::Warning, .. }));
    }

    #[test]
    fn test_rule_dollar_sigil_stripped() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("$provider == \"aws\"".to_string()),
            error_message: spanned("must use aws".to_string()),
            severity: ValidationSeverity::Error,
        };
        let ctx = make_context(&[("provider", "aws")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        assert!(matches!(outcome, RuleOutcome::Passed), "$ sigil should be stripped before eval");
    }

    #[test]
    fn test_rule_malformed_condition() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("provider ===".to_string()),
            error_message: spanned("invalid".to_string()),
            severity: ValidationSeverity::Error,
        };
        let ctx = make_context(&[("provider", "aws")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        match outcome {
            RuleOutcome::EvalError { reason } => {
                assert!(!reason.is_empty(), "reason should be non-empty");
            }
            _ => panic!("expected EvalError for malformed condition"),
        }
    }

    #[test]
    fn test_rule_info_severity() {
        let rule = ValidationRule {
            span: test_span(),
            condition: spanned("environment == \"production\"".to_string()),
            error_message: spanned("not production".to_string()),
            severity: ValidationSeverity::Info,
        };
        let ctx = make_context(&[("environment", "staging")]);
        let outcome = evaluate_validation_rule(&rule, &ctx);
        assert!(matches!(outcome, RuleOutcome::Failed { severity: ValidationSeverity::Info, .. }));
    }
}
