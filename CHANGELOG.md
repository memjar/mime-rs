# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `mime::Mime`, the decoded model of a message: its fields read in their RFC 8621 forms and its body as text, bytes, parts or a nested message.

- Added `header::MimeHeaderKind`, the closed set of 40 known fields with their `spec::MimeHeaderSpec`, and the field markers of `header::marker` keying typed access.

- Added `header::spec::MimeHeaderForm`, the forms a field reads in, and `MimeHeaderForm::parse` for a value standing alone.

- Added `tree::cst::MimeCst`, the byte-faithful syntax tree, every leaf borrowed from the input until an edit replaces it.

- Added `tree::parse::MimeParse`, the I/O-free parse coroutine yielding bulk byte searches, with the `tree::scan::scalar` and `tree::scan::memchr` answerers.

- Added `MimeParseOptions::structure_depth` and `MimeCst::expand`, a parse stopping at the header block and its body split later.

- Added the field readers of `tree::field`: unstructured text, addresses, message identifiers, dates, URLs, content types, dispositions, comma lists, transfer encodings.

- Added the leaf accessors of `tree::entity::MimeEntity`: `bytes`, `decoded`, `decoded_checked`, `decoded_len`, `text`, `text_checked`, `text_prefix` and `nested`.

- Added `tree::lists::MimeBodyLists`, the RFC 8621 text body, HTML body and attachment lists.

- Added the codecs of `codec`: base64, quoted-printable, RFC 2047 encoded words, RFC 2231 parameters, RFC 3676 flowed text and charsets.

- Added SIMD base64 through base64-codec: leaves and encoded words decode under the `simd_cpu::SimdCpu` token the application installs once with `SimdCpu::install_detected`, the build target's baseline when none is, and `MimeBuildOptions::cpu` overrides it for a build.

- Added SIMD quoted-printable through quoted-printable-codec, for bodies and Q encoded words, under the same token. Decoding now also reports a lowercase hex digit, a raw 8-bit byte and a bare CR (kept as before), reads an `=` ending the input as a clean soft break, and keeps as data the blanks past 76 before a line break.

- Added `value::subject::base_subject`, the RFC 5256 base subject.

- Added `Mime::to_bytes` and `build::MimeBuildOptions`, the deterministic writer of any model.

- Added `builder::MimeBuilder`, the strict builder with mail-builder's layout.

- Added `value::message_id::MimeMessageId`, a checked message identifier.

- Added the field edits of `tree::edit::field`: `set`, `insert`, `remove_all` and the address item edits.

- Added the content edits `MimeCst::replace_content` and `MimeCst::replace_text`.

- Added the structural edits `MimeCst::insert_part`, `remove_part`, `move_part`, `wrap`, `unwrap` and `attach`.

- Added `MimeCst::downgrade_7bit` and `MimeCst::fill_required`.

- Added sealing of signed and encrypted content, with `MimeCst::protector` and `MimeCst::unwrap_protected`.

- Added `Mime::check`, `Mime::validate`, `MimeCst::check` and `MimeCst::validate`, the validator, with `validator::MimeValid` and `validator::MimeTransport`.
