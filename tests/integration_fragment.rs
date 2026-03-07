//! Integration tests for Phase 6: Module Fragments
//! These tests verify the full expand_includes() pipeline end-to-end.

use std::path::PathBuf;
use graydr::fragment::expand_includes;

#[test]
fn test_expand_includes_end_to_end() {
    // TODO (Wave 2): write a case arm code string that includes sample.gfrag,
    // call expand_includes, verify the output matches manually inlined content.
    todo!()
}

#[test]
fn test_circular_include_hard_error() {
    // TODO (Wave 2): call expand_includes with cycle_a.gfrag as root,
    // expect FragmentError::CircularInclude naming both cycle_a and cycle_b.
    todo!()
}

#[test]
fn test_source_map_fragment_position() {
    // TODO (Wave 2): verify SourceMap::resolve() returns fragment file path
    // and correct line for an offset inside inlined fragment content.
    todo!()
}
