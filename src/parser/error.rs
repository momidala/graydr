use crate::ast::span::Span;
use thiserror::Error;

/// All errors that can occur while parsing graydr files.
///
/// Every variant except `HclParse` carries a `Span` so error messages include
/// `file:line:col` via `Span`'s `Display` implementation.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Wraps an hcl-edit parse failure (no span available — hcl-edit provides
    /// its own error message with position).
    #[error("HCL parse error in {file}: {source}")]
    HclParse {
        file: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// A required block (e.g. `metadata`) was absent from the file.
    #[error("{span}: missing required block '{block}' in {file_type}")]
    MissingRequiredBlock {
        span: Span,
        block: &'static str,
        file_type: &'static str,
    },

    /// An unrecognised block name was found at a position that expects a known block.
    #[error("{span}: unknown block '{name}'")]
    UnknownBlock { span: Span, name: String },

    /// A `case` block's label was not a quoted string variable name.
    #[error("{span}: case block label must be a quoted string variable name")]
    InvalidCaseLabel { span: Span },

    /// The top-level block type was not `module`, `template`, or `fragment`.
    #[error("{span}: expected block type 'module', 'template', or 'fragment', found '{found}'")]
    UnexpectedBlockType { span: Span, found: String },

    /// A block that requires a quoted string label was missing one.
    #[error("{span}: block '{block}' requires a quoted string label")]
    MissingLabel { span: Span, block: String },
}
