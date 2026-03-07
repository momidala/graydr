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
    todo!("implemented in plan 02-03")
}

pub fn evaluate_validation_rule(
    _rule: &ValidationRule,
    _context: &ResolveContext,
) -> RuleOutcome {
    todo!("implemented in plan 02-03")
}
