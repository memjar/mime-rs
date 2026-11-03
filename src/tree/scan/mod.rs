//! # Search answerers
//!
//! The answers to the parse coroutine's search requests, as plain
//! functions, each appending positions to a buffer the caller owns.
//!
//! [`scalar`] is the reference, plain loops written to be obviously right;
//! [`memchr`] gives the same answers through the memchr crate. Any other
//! answerer, a SIMD one included, only has to agree with [`scalar`], whose
//! functions state each search's contract.

pub mod memchr;
pub mod scalar;
