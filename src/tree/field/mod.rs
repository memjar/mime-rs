//! # Field readers
//!
//! The grammars turning a raw header field value into its decoded form,
//! each liberal: a value that does not fit reads as the closest thing it
//! means, never as an error. [`value`] dispatches from a form to its
//! reader.

pub mod address;
pub mod content_type;
pub mod date;
pub(crate) mod lex;
pub mod list;
pub mod value;
