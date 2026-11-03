//! # memchr answerer
//!
//! The parse coroutine's searches answered through the memchr crate, SIMD
//! wherever memchr has kernels for the target. Answers exactly as
//! [`scalar`](crate::tree::scan::scalar) does.

use alloc::vec::Vec;
use core::ops::Range;

use ::memchr::{memchr_iter, memmem};

use crate::tree::scan::scalar::empty_line;

/// See [`scalar::field_ends`](crate::tree::scan::scalar::field_ends).
// NOTE: hinted, since the empty line check grows it past the threshold for
// inlining into the answer loop.
#[inline]
pub fn field_ends(input: &[u8], range: Range<usize>, out: &mut Vec<usize>) {
    let end = range.end.min(input.len());
    let Some(hay) = input.get(range.start..end) else {
        return;
    };

    for at in memchr_iter(b'\n', hay) {
        let i = range.start + at;
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

/// See [`scalar::dash_lines`](crate::tree::scan::scalar::dash_lines).
pub fn dash_lines(input: &[u8], range: Range<usize>, out: &mut Vec<usize>) {
    let end = range.end.min(input.len());
    let Some(hay) = input.get(range.start..end) else {
        return;
    };

    if hay.starts_with(b"--") {
        out.push(range.start);
    }

    for at in memmem::find_iter(hay, b"\n--") {
        out.push(range.start + at + 1);
    }
}

/// See [`scalar::find`](crate::tree::scan::scalar::find).
pub fn find(input: &[u8], range: Range<usize>, needle: &[u8], out: &mut Vec<usize>) {
    let end = range.end.min(input.len());
    let Some(hay) = input.get(range.start..end) else {
        return;
    };

    if needle.is_empty() {
        return;
    }

    out.extend(memmem::find_iter(hay, needle).map(|at| range.start + at));
}
