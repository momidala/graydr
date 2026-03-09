//! Integration tests for Phase 6: Module Fragments
//! These tests verify the full expand_includes() pipeline end-to-end.

use std::path::PathBuf;
use graydr::fragment::{expand_includes, FragmentError};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

#[test]
fn test_expand_includes_end_to_end() {
    let fixtures = fixtures_dir();

    // A code string that includes sample.gfrag and has a trailing line.
    let code = "include \"sample.gfrag\"\nsome_other_line = true";

    let (expanded, source_map) = expand_includes(
        code,
        "root.gmod",
        &fixtures,
        &mut Vec::new(),
        None,
    )
    .expect("expand_includes with sample.gfrag must succeed");

    // The expanded output must contain fragment content.
    assert!(
        expanded.contains("aws_s3_bucket"),
        "expanded output must contain fragment content 'aws_s3_bucket'; got: {:?}",
        expanded
    );

    // The raw include directive must NOT appear in the output.
    assert!(
        !expanded.contains("include \"sample.gfrag\""),
        "expanded output must not contain raw include directive; got: {:?}",
        expanded
    );

    // The SourceMap must have at least one entry referencing sample.gfrag.
    let has_sample_entry = source_map.entries.iter().any(|e| e.source_file.contains("sample.gfrag"));
    assert!(
        has_sample_entry,
        "SourceMap must have at least one entry with source_file containing 'sample.gfrag'; entries: {:?}",
        source_map.entries.iter().map(|e| &e.source_file).collect::<Vec<_>>()
    );
}

#[test]
fn test_circular_include_hard_error() {
    let fixtures = fixtures_dir();

    // cycle_a.gfrag includes cycle_b.gfrag which includes cycle_a.gfrag.
    let code = "include \"cycle_a.gfrag\"";

    let result = expand_includes(
        code,
        "root.gmod",
        &fixtures,
        &mut Vec::new(),
        None,
    );

    assert!(result.is_err(), "circular include must return Err; got Ok");

    match result.unwrap_err() {
        FragmentError::CircularInclude { cycle } => {
            let has_cycle_a = cycle.iter().any(|p| p.contains("cycle_a"));
            let has_cycle_b = cycle.iter().any(|p| p.contains("cycle_b"));
            assert!(
                has_cycle_a,
                "cycle Vec must contain cycle_a; got: {:?}",
                cycle
            );
            assert!(
                has_cycle_b,
                "cycle Vec must contain cycle_b; got: {:?}",
                cycle
            );
        }
        other => panic!("expected FragmentError::CircularInclude, got: {:?}", other),
    }
}

#[test]
fn test_source_map_fragment_position() {
    let fixtures = fixtures_dir();

    // A code string that includes sample.gfrag.
    let code = "include \"sample.gfrag\"\nafter line";

    let (expanded, source_map) = expand_includes(
        code,
        "root.gmod",
        &fixtures,
        &mut Vec::new(),
        None,
    )
    .expect("expand_includes must succeed");

    assert!(
        !source_map.entries.is_empty(),
        "SourceMap must have at least one entry; expanded: {:?}",
        expanded
    );

    // Find the first entry whose source_file references sample.gfrag.
    let frag_entry = source_map.entries.iter().find(|e| e.source_file.contains("sample.gfrag"))
        .expect("SourceMap must have an entry for sample.gfrag");

    // Pick an offset inside the fragment range.
    let mid_offset = frag_entry.expanded_start + 1;

    let (resolved_file, resolved_line) = source_map.resolve(mid_offset);

    assert!(
        resolved_file.contains("sample.gfrag"),
        "resolve() must return sample.gfrag for an offset inside the fragment; got: {:?}",
        resolved_file
    );
    assert!(
        resolved_line >= 1,
        "resolved line must be >= 1; got: {}",
        resolved_line
    );
}
