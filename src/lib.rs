#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]

//! # mime-rs
//!
//! A MIME message (RFC 5322 and MIME) parser, validator, editor and
//! builder.

extern crate alloc;

pub mod codec;
pub mod header;
pub mod mime;
pub mod value;

#[cfg(feature = "parser")]
#[cfg_attr(docsrs, doc(cfg(feature = "parser")))]
pub mod tree;
