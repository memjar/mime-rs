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

        if empty_line(input, i + 1, end).is_some() {
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

/// The end of the empty line starting at `at`, within `end`: a line holding
/// nothing but blanks, carriage returns and form feeds before its line feed.
/// Asked only where a line continues no field, so it never starts with a
/// blank.
// NOTE: wider than `\n` and `\r\n`, as mail-parser reads it: a message whose
// CRLF line endings were converted again ends its empty line in `\r\r\n`.
pub(crate) fn empty_line(input: &[u8], at: usize, end: usize) -> Option<usize> {
    // NOTE: the first byte alone settles a line that starts a field, the
    // common case; the rare scan stays out of line, so the answerers that
    // ask this at every field end still inline into their caller.
    match *input.get(at)? {
        b'\n' if at < end => Some(at + 1),
        b'\r' | b'\x0c' => blank_line_end(input, at, end),
        _ => None,
    }
}

/// [`empty_line`] for a line starting with a carriage return or a form feed.
#[cold]
#[inline(never)]
fn blank_line_end(input: &[u8], at: usize, end: usize) -> Option<usize> {
    let line = input.get(at..end.min(input.len()))?;
    let lf = line
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\x0c'))?;
    (line[lf] == b'\n').then_some(at + lf + 1)
}
