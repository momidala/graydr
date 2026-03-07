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
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }

    #[test]
    fn test_include_inlines_content() {
        let fixtures = fixtures_dir();
        let code = r#"include "sample.gfrag"
extra line"#;
        let (expanded, source_map) = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        )
        .expect("expand_includes should succeed");

        // The expanded output must contain the fragment's code content.
        assert!(
            expanded.contains("aws_s3_bucket"),
            "expanded output should contain fragment content; got: {:?}",
            expanded
        );
        // The extra line must still be present.
        assert!(
            expanded.contains("extra line"),
            "expanded output should preserve non-include lines; got: {:?}",
            expanded
        );
        // The SourceMap must have at least one entry for the inlined range.
        assert!(
            !source_map.entries.is_empty(),
            "SourceMap should have at least one entry after inlining"
        );
    }

    #[test]
    fn test_registry_coordinate_deferred() {
        let fixtures = fixtures_dir();
        let code = r#"include "org/name@1.0""#;
        let result = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        );
        // Must not return FileNotFound — registry coords are deferred, not errored.
        match result {
            Ok(_) => { /* acceptable — deferred silently */ }
            Err(FragmentError::RegistryResolutionDeferred { .. }) => { /* also acceptable */ }
            Err(FragmentError::FileNotFound { .. }) => {
                panic!("registry coordinate must NOT produce FileNotFound")
            }
            Err(e) => panic!("unexpected error for registry coordinate: {:?}", e),
        }
    }

    #[test]
    fn test_circular_include_error() {
        let fixtures = fixtures_dir();
        // cycle_a.gfrag includes cycle_b.gfrag which includes cycle_a.gfrag
        let code = r#"include "cycle_a.gfrag""#;
        let result = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        );
        match result {
            Err(FragmentError::CircularInclude { cycle }) => {
                // The cycle must mention both files.
                let cycle_str = cycle.join(" -> ");
                assert!(
                    cycle.iter().any(|p| p.contains("cycle_a")),
                    "cycle should contain cycle_a; got: {:?}",
                    cycle_str
                );
                assert!(
                    cycle.iter().any(|p| p.contains("cycle_b")),
                    "cycle should contain cycle_b; got: {:?}",
                    cycle_str
                );
            }
            other => panic!("expected CircularInclude error, got: {:?}", other),
        }
    }

    #[test]
    fn test_source_map_resolves_fragment_position() {
        let fixtures = fixtures_dir();
        let code = r#"include "sample.gfrag"
after line"#;
        let (expanded, source_map) = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        )
        .expect("expand_includes should succeed");

        // Find an offset that is inside the inlined fragment content.
        // The first entry's expanded_start is inside the fragment.
        assert!(
            !source_map.entries.is_empty(),
            "SourceMap must have entries"
        );
        let entry = &source_map.entries[0];
        let mid_offset = entry.expanded_start + 1;
        let (resolved_file, _line) = source_map.resolve(mid_offset);
        // The resolved file must be the fragment file, not root.gfrag.
        assert!(
            resolved_file.contains("sample"),
            "resolve() should point to fragment file; got: {:?}",
            resolved_file
        );
    }

    #[test]
    fn test_file_path_resolution() {
        let fixtures = fixtures_dir();
        let code = r#"include "sample.gfrag""#;
        let result = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        );
        assert!(
            result.is_ok(),
            "include of sample.gfrag with correct include_path should succeed; got: {:?}",
            result.err()
        );
        let (expanded, _) = result.unwrap();
        assert!(
            expanded.contains("aws_s3_bucket"),
            "expanded output should contain sample.gfrag content; got: {:?}",
            expanded
        );
    }

    #[test]
    fn test_registry_coordinate_not_file_error() {
        let fixtures = fixtures_dir();
        let code = r#"include "org/name@1.0""#;
        let result = expand_includes(
            code,
            "root.gfrag",
            &fixtures,
            &mut Vec::new(),
        );
        // Must never produce FileNotFound for a registry coordinate.
        assert!(
            !matches!(result, Err(FragmentError::FileNotFound { .. })),
            "registry coordinate must NOT produce FileNotFound error"
        );
    }

    #[test]
    fn test_diamond_include_not_cycle() {
        let fixtures = fixtures_dir();
        // diamond_a.gfrag includes both diamond_b.gfrag and diamond_c.gfrag
        // both of which include diamond_d.gfrag — no cycle, D appears twice
        let code = r#"include "diamond_b.gfrag"
include "diamond_c.gfrag""#;
        let result = expand_includes(
            code,
            "diamond_a.gfrag",
            &fixtures,
            &mut Vec::new(),
        );
        // Must NOT return CircularInclude.
        assert!(
            !matches!(result, Err(FragmentError::CircularInclude { .. })),
            "diamond-shaped includes must not produce a cycle error"
        );
        // D content must appear twice.
        if let Ok((expanded, _)) = result {
            let count = expanded.matches("shared-queue").count();
            assert_eq!(
                count, 2,
                "D's content should appear twice in diamond expansion; got: {}",
                count
            );
        }
    }
}
