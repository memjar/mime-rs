#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]

//! # mime-rs
//!
//! A parser, validator, editor and builder of Internet messages (RFC 5322
//! and MIME), liberal in what it reads and strict in what it writes.
//!
//! ## Layers
//!
//! The crate is `no_std` and free of `unsafe`. The decoded model
//! ([`mime`], [`header`], [`value`]) needs nothing: a [`Mime`] is a message
//! as pure data, its fields read in their RFC 8621 forms, [`header::spec`]
//! stating what the RFCs allow of each field and [`header::marker`] keying
//! typed access by field name.
//!
//! The content codecs ([`codec`]) decode on demand: quoted-printable,
//! RFC 2047 encoded words, RFC 2231 parameters and RFC 3676 flowed text
//! always, base64 behind `base64`, charsets behind `encoding`.
//!
//! The writer ([`build`]) turns any model into canonical bytes with no
//! clock and no randomness, [`builder`] composes a model strictly, and the
//! [`validator`] lists every violation and advisory of a model and mints
//! the [`MimeValid`](validator::MimeValid) proof.
//!
//! The syntax tree (`tree`, behind `parser`) is byte-faithful, every leaf
//! borrowing the input. An I/O-free coroutine parses it, handing its byte
//! searches to the caller; it is read lazily, edited in place without
//! copying what an edit does not name, and validated as written.
//!
//! ## Features
//!
//! | feature | default | pulls | gives |
//! | --- | --- | --- | --- |
//! | `parser` | yes | memchr | the syntax tree, its readers, edits and wire validation |
//! | `base64` | yes | base64 | base64 bodies and B encoded words |
//! | `encoding` | yes | encoding_rs | charsets beyond UTF-8 and US-ASCII |
//!
//! With every feature off the crate pulls in no dependency.
//!
//! ## Example
//!
//! ```rust
//! # #[cfg(all(feature = "parser", feature = "base64", feature = "encoding"))] {
//! use mime::{
//!     build::MimeBuildOptions,
//!     header::marker::{FROM, SUBJECT},
//!     tree::cst::MimeCst,
//! };
//!
//! let raw = concat!(
//!     "From: Alice <alice@example.com>\r\n",
//!     "Subject: Draft\r\n",
//!     "  number one\r\n",
//!     "\r\n",
//!     "Hello\r\n",
//! );
//!
//! let mut cst = MimeCst::parse(raw.as_bytes());
//! let from = cst.root.header::<FROM>().unwrap();
//! assert_eq!(from.first().unwrap().address.as_deref(), Some("alice@example.com"));
//! assert_eq!(cst.root.header::<SUBJECT>().as_deref(), Some("Draft number one"));
//!
//! // NOTE: an edit rewrites the field it names and nothing else.
//! let opts = MimeBuildOptions::default();
//! cst.header_mut(0).unwrap().set::<SUBJECT>("Café".into(), &opts).unwrap();
//!
//! let bytes = cst.to_bytes();
//! assert!(bytes.starts_with(b"From: Alice <alice@example.com>\r\nSubject: =?utf-8?"));
//! assert!(bytes.ends_with(b"\r\n\r\nHello\r\n"));
//! # }
//! ```

extern crate alloc;

pub mod build;
pub mod builder;
pub mod codec;
pub mod header;
pub mod mime;
pub mod validator;
pub mod value;

#[cfg(feature = "parser")]
#[cfg_attr(docsrs, doc(cfg(feature = "parser")))]
pub mod tree;

#[cfg(doc)]
use crate::mime::Mime;
