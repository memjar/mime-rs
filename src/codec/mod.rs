//! # Content codecs
//!
//! The transfer encodings and character sets content arrives in, decoded
//! only when asked for: [`base64`] (behind the `base64` feature),
//! [`quoted_printable`], [`rfc2047`] encoded words, [`rfc2231`] parameter
//! values and [`charset`] transcoding (behind the `encoding` feature).
//!
//! None of them needs the syntax tree, so a caller holding raw bytes can use
//! them alone.

#[cfg(feature = "base64")]
#[cfg_attr(docsrs, doc(cfg(feature = "base64")))]
pub mod base64;
pub mod charset;
pub mod flowed;
pub mod problems;
pub mod quoted_printable;
pub mod rfc2047;
pub mod rfc2231;
