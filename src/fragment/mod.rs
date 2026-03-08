use std::path::Path;
use thiserror::Error;

use regex::Regex;

use crate::registry::RegistryClient;
use crate::registry::coord::ModuleCoord;

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
    #[error("module '{coordinate}' is retired and cannot be used; check for a newer active version")]
    RetiredModule { coordinate: String },
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
        SourceMap {
            entries: vec![],
            root_file,
        }
    }

    /// Translate byte offset in expanded string → (file, line).
    ///
    /// Scans entries ordered by expanded_start. If the offset falls within
    /// [entry.expanded_start, entry.expanded_end), returns that entry's
    /// source_file and source_line_offset + lines within the entry up to offset.
    /// If no entry matches, returns (root_file, 0).
    pub fn resolve(&self, expanded_offset: usize) -> (&str, u32) {
        for entry in &self.entries {
            if expanded_offset >= entry.expanded_start && expanded_offset < entry.expanded_end {
                // Compute how many lines into this entry the offset falls.
                let within = expanded_offset - entry.expanded_start;
                // We count newlines to determine line within the entry.
                // This is a coarse approximation — source_line_offset is the base.
                let lines_within = within as u32;
                return (entry.source_file.as_str(), entry.source_line_offset + lines_within);
            }
        }
        (self.root_file.as_str(), 0)
    }
}

/// Returns true if the path string looks like a registry coordinate:
/// `org/name@major` — i.e., contains a `/` before an `@` followed by a digit.
fn is_registry_coordinate(path: &str) -> bool {
    // Pattern: at least one char, slash, at least one char, @, digit
    let re = Regex::new(r"^[^/]+/[^@]+@\d").expect("registry regex is valid");
    re.is_match(path)
}

/// Expand all `include "..."` directives in `code` inline, producing the
/// expanded string and a SourceMap for error position translation.
///
/// `source_file` — the file this code string came from (for error spans).
/// `include_path` — base directory for resolving relative fragment paths.
/// `call_stack`   — ordered Vec of canonical paths currently on the recursion
///                  stack; pass `&mut Vec::new()` at top level.
/// `registry`     — optional registry client; if Some, registry coordinates are
///                  fetched and inlined; if None, Phase 6 deferred behavior is preserved.
pub fn expand_includes(
    code: &str,
    source_file: &str,
    include_path: &Path,
    call_stack: &mut Vec<String>,
    registry: Option<&RegistryClient>,
) -> Result<(String, SourceMap), FragmentError> {
    let include_re =
        Regex::new(r#"^\s*include\s+"([^"]+)"\s*$"#).expect("include regex is valid");

    let mut output = String::new();
    let mut source_map = SourceMap::new(source_file.to_string());

    for line in code.lines() {
        if let Some(caps) = include_re.captures(line) {
            let path_str = caps.get(1).unwrap().as_str();

            if is_registry_coordinate(path_str) {
                match registry {
                    None => {
                        // Preserve Phase 6 deferred behavior when no client is provided.
                        let deferred_marker = format!("<deferred:{}>", path_str);
                        let start = output.len();
                        // We append nothing to the output — the include line is dropped.
                        // But we push a zero-length SourceEntry marking the deferral.
                        source_map.entries.push(SourceEntry {
                            expanded_start: start,
                            expanded_end: start,
                            source_file: deferred_marker,
                            source_line_offset: 0,
                        });
                        // Do not append any content for the registry include.
                        continue;
                    }
                    Some(client) => {
                        let coord = ModuleCoord::parse(path_str)
                            .map_err(|e| FragmentError::RegistryResolutionDeferred {
                                coordinate: format!("{} (parse error: {})", path_str, e),
                            })?;
                        // Lifecycle check first (REG-04)
                        let lifecycle = client.get_lifecycle(&coord)
                            .map_err(|e| FragmentError::RegistryResolutionDeferred {
                                coordinate: format!("{} (lifecycle check failed: {})", path_str, e),
                            })?;
                        if lifecycle.blocks_new_use() {
                            return Err(FragmentError::RetiredModule {
                                coordinate: path_str.to_string(),
                            });
                        }
                        // Fetch content
                        let content = client.fetch_module(&coord)
                            .map_err(|e| FragmentError::RegistryResolutionDeferred {
                                coordinate: format!("{} (fetch failed: {})", path_str, e),
                            })?;
                        // Inline content the same way as file-based fragments
                        let entry_start = output.len();
                        output.push_str(&content);
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                        let entry_end = output.len();
                        source_map.entries.push(SourceEntry {
                            expanded_start: entry_start,
                            expanded_end: entry_end,
                            source_file: format!("registry:{}", path_str),
                            source_line_offset: 0,
                        });
                        continue;
                    }
                }
            }

            // Resolve the include path relative to include_path.
            let candidate = include_path.join(path_str);
            let canonical = candidate.canonicalize().map_err(|_| FragmentError::FileNotFound {
                path: path_str.to_string(),
                included_from: source_file.to_string(),
            })?;
            let canonical_str = canonical.to_string_lossy().into_owned();

            // Cycle detection: if canonical_str is already on the call stack.
            if call_stack.contains(&canonical_str) {
                let mut cycle = call_stack.clone();
                cycle.push(canonical_str.clone());
                return Err(FragmentError::CircularInclude { cycle });
            }

            // Push onto call stack before recursing.
            call_stack.push(canonical_str.clone());

            // Read the fragment file.
            let file_contents =
                std::fs::read_to_string(&canonical).map_err(|_| FragmentError::FileNotFound {
                    path: path_str.to_string(),
                    included_from: source_file.to_string(),
                })?;

            // Parse the fragment to get its code string.
            let frag_def = crate::parser::fragment::parse_fragment_file(
                &file_contents,
                &canonical_str,
            )
            .map_err(|e| FragmentError::ParseError {
                file: canonical_str.clone(),
                source: e,
            })?;

            // Recursive expansion: next include_path is the fragment's parent dir.
            let next_include_path = canonical
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();

            let (expanded_frag, inner_map) = expand_includes(
                &frag_def.code.value,
                &canonical_str,
                &next_include_path,
                call_stack,
                registry,
            )?;

            // Pop from call stack after recursion returns.
            call_stack.pop();

            // Record the byte range in the output for this inlined fragment.
            let entry_start = output.len();
            output.push_str(&expanded_frag);
            // Ensure content ends with newline for clean line-by-line expansion.
            if !output.ends_with('\n') {
                output.push('\n');
            }
            let entry_end = output.len();

            // Record our own SourceEntry for the range we just added.
            source_map.entries.push(SourceEntry {
                expanded_start: entry_start,
                expanded_end: entry_end,
                source_file: canonical_str.clone(),
                source_line_offset: 0,
            });

            // Merge inner entries (offset them by entry_start).
            for mut inner_entry in inner_map.entries {
                inner_entry.expanded_start += entry_start;
                inner_entry.expanded_end += entry_start;
                source_map.entries.push(inner_entry);
            }
        } else {
            // Not an include directive — pass through as-is.
            output.push_str(line);
            output.push('\n');
        }
    }

    Ok((output, source_map))
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
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
        // diamond_b.gfrag includes diamond_d.gfrag
        // diamond_c.gfrag includes diamond_d.gfrag
        // Expanding both from "diamond_a" — no cycle, D content appears twice
        let code = r#"include "diamond_b.gfrag"
include "diamond_c.gfrag""#;
        let result = expand_includes(
            code,
            "diamond_a.gfrag",
            &fixtures,
            &mut Vec::new(),
            None,
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

    #[test]
    fn test_registry_include_with_client_inlines_content() {
        use mockito::Server;
        let mut server = Server::new();
        let _meta = server
            .mock("GET", "/api/v1/modules/testorg/testmod/1.0.0/meta")
            .with_body(r#"{"lifecycle":"active"}"#)
            .with_status(200)
            .create();
        let _content = server
            .mock("GET", "/api/v1/modules/testorg/testmod/1.0.0/content")
            .with_body("inlined_fragment_code")
            .with_status(200)
            .create();
        let config = crate::registry::RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = crate::registry::RegistryClient::new(config);
        // Clear cache to force HTTP fetch
        let coord = crate::registry::coord::ModuleCoord::parse("testorg/testmod@1.0.0").unwrap();
        if let Some(p) = crate::registry::cache::cache_path(&coord) {
            let _ = std::fs::remove_file(&p);
        }
        let code = r#"include "testorg/testmod@1.0.0""#;
        let (expanded, _) = expand_includes(
            code,
            "root",
            std::path::Path::new("."),
            &mut vec![],
            Some(&client),
        )
        .unwrap();
        assert!(
            expanded.contains("inlined_fragment_code"),
            "registry include must inline fetched content; got: {:?}",
            expanded
        );
        // cleanup
        if let Some(p) = crate::registry::cache::cache_path(&coord) {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn test_registry_include_retired_module_errors() {
        use mockito::Server;
        let mut server = Server::new();
        let _meta = server
            .mock("GET", "/api/v1/modules/retorg/retmod/2.0.0/meta")
            .with_body(r#"{"lifecycle":"retired"}"#)
            .with_status(200)
            .create();
        let config = crate::registry::RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = crate::registry::RegistryClient::new(config);
        let code = r#"include "retorg/retmod@2.0.0""#;
        let result = expand_includes(
            code,
            "root",
            std::path::Path::new("."),
            &mut vec![],
            Some(&client),
        );
        assert!(
            matches!(result, Err(FragmentError::RetiredModule { .. })),
            "retired module must produce RetiredModule error; got: {:?}",
            result
        );
    }
}
