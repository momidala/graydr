use crate::ast::span::Span;

/// Generic wrapper carrying a value alongside its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

/// A graydr variable reference — the `$name` sigil is stripped; `name` holds
/// just the identifier (e.g. `"provider"` from `$provider`).
#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
}

/// A `Spanned<Variable>` — the canonical type for variable references in the AST.
pub type SpannedVariable = Spanned<Variable>;

/// A literal value (string, boolean, or number).
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Bool(bool),
    Number(f64),
}
