use crate::ast::common::{Spanned, SpannedVariable};
use crate::ast::span::Span;

/// Top-level definition for a `.gfrag` fragment file.
///
/// Fragment inclusion/cycle detection is deferred to Phase 6.
/// Phase 1 only needs this type to compile; no parsing implementation yet.
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentDefinition {
    pub span: Span,
    pub name: Spanned<String>,
    pub code: Spanned<String>,
    pub variables: Vec<SpannedVariable>,
}
