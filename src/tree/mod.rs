//! # Syntax tree
//!
//! Everything byte-faithful: the tree a message parses into, the coroutine
//! parsing it, the search answerers that coroutine relies on, and the
//! readers turning raw field values into decoded ones.
//!
//! The hub is [`cst::MimeCst`]: plain structs with public fields
//! ([`entity`], [`header`], [`body`]) whose every leaf borrows the input
//! until an edit replaces it, so serializing an unedited message gives back
//! its bytes and an edit owns only what it writes.
//!
//! [`parse::MimeParse`] builds it as an I/O-free coroutine over the answers
//! of [`scan`]; [`anomaly`] names what it worked around. This whole layer
//! is gated behind the `parser` feature, so the decoded model can be
//! depended on without it.

pub mod anomaly;
pub mod body;
pub mod coroutine;
pub mod cst;
pub mod entity;
pub mod field;
pub mod header;
pub mod lists;
pub mod parse;
pub mod scan;
