//! # Search answerers
//!
//! The answers to the parse coroutine's search requests, as plain
//! functions, each appending positions to a buffer the caller owns.
//!
//! [`scalar`] is the reference, plain loops written to be obviously right;
//! [`memchr`] gives the same answers through the memchr crate. Any other
//! answerer, a SIMD one included, only has to agree with [`scalar`]:
//!
//! - field ends: every `\n` in the range whose next byte is neither SP nor
//!   HTAB (or that ends the range), stopping after the first one followed by
//!   an empty line;
//! - dash lines: every position in the range starting `--` at the start of a
//!   line (the range start counting as one);
//! - find: every non-overlapping occurrence of a needle in the range,
//!   leftmost first.

pub mod memchr;
pub mod scalar;
