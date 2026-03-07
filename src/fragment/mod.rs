use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FragmentError {
    #[error("circular fragment include detected; cycle: {}", cycle.join(" -> "))]
    CircularInclude { cycle: Vec<String> },
    #[error("fragment file not found: '{path}' (included from {included_from})")]
    FileNotFound { path: String, included_from: String },
    #[error("fragment parse error in '{file}': {source}")]
    ParseError {
        file: String,
        #[source]
        source: crate::parser::error::ParseError,
    },
    #[error("registry resolution deferred for '{coordinate}' — registry not available in community tier")]
    RegistryResolutionDeferred { coordinate: String },
}

/// Maps byte ranges in the expanded string back to source file positions.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub expanded_start: usize,
    pub expanded_end: usize,
    pub source_file: String,
    pub source_line_offset: u32,
}

#[derive(Debug, Clone)]
pub struct SourceMap {
    pub entries: Vec<SourceEntry>,
    pub root_file: String,
}

impl SourceMap {
    pub fn new(root_file: String) -> Self {
        todo!()
    }

    /// Translate byte offset in expanded string → (file, line).
    pub fn resolve(&self, expanded_offset: usize) -> (&str, u32) {
        todo!()
    }
}

/// Expand all `include "..."` directives in `code` inline, producing the
/// expanded string and a SourceMap for error position translation.
///
/// `source_file` — the file this code string came from (for error spans).
/// `include_path` — base directory for resolving relative fragment paths.
/// `call_stack`   — ordered Vec of canonical paths currently on the recursion
///                  stack; pass `&mut Vec::new()` at top level.
pub fn expand_includes(
    code: &str,
    source_file: &str,
    include_path: &Path,
    call_stack: &mut Vec<String>,
) -> Result<(String, SourceMap), FragmentError> {
    todo!()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_include_inlines_content() {
        todo!()
    }

    #[test]
    fn test_registry_coordinate_deferred() {
        todo!()
    }

    #[test]
    fn test_circular_include_error() {
        todo!()
    }

    #[test]
    fn test_source_map_resolves_fragment_position() {
        todo!()
    }

    #[test]
    fn test_file_path_resolution() {
        todo!()
    }

    #[test]
    fn test_registry_coordinate_not_file_error() {
        todo!()
    }

    #[test]
    fn test_diamond_include_not_cycle() {
        todo!()
    }
}
