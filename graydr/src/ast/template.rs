use crate::ast::common::{Spanned, SpannedVariable};
use crate::ast::module::{MetadataBlock, OutputMapping};
use crate::ast::span::Span;

/// Top-level definition for a `.gtpl` template file.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDefinition {
    pub span: Span,
    pub name: Spanned<String>,
    pub metadata: Spanned<MetadataBlock>,
    pub parameters: Spanned<ParametersBlock>,
    pub resources: Vec<Spanned<ResourceInstance>>,
    pub outputs: Spanned<OutputsBlock>,
}

/// `parameters { group_name { param = {} ... } ... }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ParametersBlock {
    pub span: Span,
    pub groups: Vec<Spanned<ParameterGroup>>,
}

/// A named group of parameters (e.g. `primary_db { provider = {} region = {} }`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterGroup {
    pub span: Span,
    pub name: Spanned<String>,
    pub params: Vec<Spanned<ParameterDecl>>,
}

/// A single parameter declaration within a group.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDecl {
    pub span: Span,
    pub name: Spanned<String>,
    pub variables: Vec<SpannedVariable>,
}

/// A `resource "name" { module = "..." inputs { ... } }` block in a template.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceInstance {
    pub span: Span,
    pub name: Spanned<String>,
    pub module_ref: Spanned<String>,
    pub inputs: Vec<Spanned<InputBinding>>,
    pub depends_on: Vec<Spanned<String>>,
}

/// A key = value binding within a resource `inputs { }` block.
///
/// `variables` captures `$var` references found in the value string.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBinding {
    pub span: Span,
    pub key: Spanned<String>,
    pub value: Spanned<String>,
    pub variables: Vec<SpannedVariable>,
}

/// `outputs { name = "..." ... }` block at the template level.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputsBlock {
    pub span: Span,
    pub mappings: Vec<Spanned<OutputMapping>>,
}
