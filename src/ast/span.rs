use std::sync::Arc;

/// Source position spanning from start to end (1-indexed line and column).
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub file: Arc<str>,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.start_line, self.start_col)
    }
}

/// Convert a byte offset in `source` to 1-indexed (line, col).
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Convert an hcl-edit byte-offset range (from `hcl::edit::Span::span()`) to a graydr `Span`.
///
/// hcl-edit exposes spans as `Option<Range<usize>>` via the `hcl::edit::repr::Span` trait.
/// Call `.span()` on any hcl-edit node, unwrap the `Option`, and pass the `Range<usize>` here.
pub fn hcl_range_to_graydr(source: &str, range: std::ops::Range<usize>, file: &Arc<str>) -> Span {
    let (start_line, start_col) = byte_offset_to_line_col(source, range.start);
    let (end_line, end_col) = byte_offset_to_line_col(source, range.end);
    Span {
        file: file.clone(),
        start_line,
        start_col,
        end_line,
        end_col,
    }
}
