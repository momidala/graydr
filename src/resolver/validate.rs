use std::collections::{HashMap, HashSet};
use evalexpr::{eval_boolean_with_context, HashMapContext, ContextWithMutableVariables};
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

/// Validate that a resource's input bindings match the module's declared interface.
///
/// Returns errors for:
/// - required inputs that are not wired in the resource
/// - wired keys that are not declared in the module interface
pub fn validate_module_inputs(
    resource: &ResourceInstance,
    module: &ModuleDefinition,
) -> Vec<ResolveError> {
    let module_name = &module.name.value;
    let declared_inputs: HashMap<&str, bool> = module
        .interface
        .value
        .inputs
        .iter()
        .map(|s| (s.value.name.value.as_str(), s.value.required))
        .collect();

    let wired_keys: HashSet<&str> = resource
        .inputs
        .iter()
        .map(|s| s.value.key.value.as_str())
        .collect();

    let mut errors = Vec::new();

    // Pass 1: check required inputs are wired
    for (input_name, required) in &declared_inputs {
        if *required && !wired_keys.contains(input_name) {
            errors.push(ResolveError::MissingRequiredInput {
                span: resource.span.clone(),
                module: module_name.clone(),
                input: input_name.to_string(),
            });
        }
    }

    // Pass 2: check wired keys are declared
    for binding in &resource.inputs {
        let key = binding.value.key.value.as_str();
        if !declared_inputs.contains_key(key) {
            errors.push(ResolveError::UnknownInput {
                span: binding.value.key.span.clone(),
                module: module_name.clone(),
                input: key.to_string(),
            });
        }
    }

    errors
}

/// Evaluate a single validation rule condition against the resolved context.
///
/// The `$` sigil is stripped from the condition string before evaluation,
/// since module authors write `$variable_name` but evalexpr expects bare identifiers.
pub fn evaluate_validation_rule(
    rule: &ValidationRule,
    context: &ResolveContext,
) -> RuleOutcome {
    // Build evalexpr context from resolved values
    let mut eval_ctx = HashMapContext::new();
    for (name, value) in context.all_values() {
        let _ = eval_ctx.set_value(name.to_string().into(), value.to_string().into());
    }

    // Strip $ sigil before evaluation
    let stripped = rule.condition.value.replace('$', "");

    match eval_boolean_with_context(&stripped, &eval_ctx) {
        Ok(true) => RuleOutcome::Passed,
        Ok(false) => RuleOutcome::Failed {
            message: rule.error_message.value.clone(),
            severity: rule.severity.clone(),
        },
        Err(e) => RuleOutcome::EvalError {
            reason: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use crate::ast::span::Span;
    use crate::ast::common::Spanned;
    use crate::ast::module::{
        InterfaceBlock, InputDecl, ValidationBlock,
        MetadataBlock, ModuleDefinition,
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
            metadata: spanned(MetadataBlock { span: test_span(), ..Default::default() }),
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
