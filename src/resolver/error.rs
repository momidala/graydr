use thiserror::Error;
use crate::ast::span::Span;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("{span}: unresolved variable '${name}'")]
    UnresolvedVariable { span: Span, name: String },

    #[error("{span}: required input '{input}' of module '{module}' is not wired in template")]
    MissingRequiredInput { span: Span, module: String, input: String },

    #[error("{span}: module '{module}' has no input named '{input}'")]
    UnknownInput { span: Span, module: String, input: String },

    #[error("{span}: validation rule '{rule}' failed: {message}")]
    ValidationFailed { span: Span, rule: String, message: String },

    #[error("{span}: invalid condition expression in rule '{rule}': {reason}")]
    InvalidCondition { span: Span, rule: String, reason: String },

    #[error("failed to load properties file '{path}': {reason}")]
    PropertiesLoadError { path: String, reason: String },
}
