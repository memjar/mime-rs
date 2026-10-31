//! # Scalar answerer
//!
//! The reference answers to the parse coroutine's searches, one byte at a
//! time. Every other answerer must return exactly what these return.

use alloc::vec::Vec;
use core::ops::Range;

/// Appends every field end of the header block starting the range: each
/// `\n` whose next byte is neither SP nor HTAB, or that ends the range,
/// stopping after the first one followed by an empty line.
pub fn field_ends(input: &[u8], range: Range<usize>, out: &mut Vec<usize>) {
    let end = range.end.min(input.len());

    for i in range.start..end {
        if input[i] != b'\n' {
            continue;
        }

        let next = input.get(i + 1).filter(|_| i + 1 < end);

        if matches!(next, Some(b' ' | b'\t')) {
            continue;
        }

        out.push(i);

        if is_empty_line(input, i + 1, end) {
            return;
        }
    }
}

/// Appends every line start in the range beginning with `--`, the range
/// start counting as a line start.
pub fn dash_lines(input: &[u8], range: Range<usize>, out: &mut Vec<usize>) {
    let end = range.end.min(input.len());

    for i in range.start..end {
        let line_start = i == range.start || input[i - 1] == b'\n';

        if line_start && i + 2 <= end && input[i] == b'-' && input[i + 1] == b'-' {
            out.push(i);
        }
    }
}

/// Appends every non-overlapping occurrence of `needle` in the range,
/// leftmost first.
pub fn find(input: &[u8], range: Range<usize>, needle: &[u8], out: &mut Vec<usize>) {
    let end = range.end.min(input.len());

    if needle.is_empty() {
        return;
    }

    let mut i = range.start;

    while i + needle.len() <= end {
        if &input[i..i + needle.len()] == needle {
            out.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

/// Whether an empty line (`\n` or `\r\n`) starts at `at`, within `end`.
pub(crate) fn is_empty_line(input: &[u8], at: usize, end: usize) -> bool {
    match input.get(at) {
        Some(b'\n') => at < end,
        Some(b'\r') => at + 1 < end && input.get(at + 1) == Some(&b'\n'),
        _ => false,
    }
}
