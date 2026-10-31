---
cairn: tasks
change: header-fields-are-read-by-form
---

- [x] `MimeBase64::decode` and the quoted-printable decoder (in-house, see the phase 0 log): line-by-line decoding with a filtering fallback, Q mode, problems reported
- [x] `MimeCharset` over `encoding_rs::Encoding::for_label` behind `encoding`, UTF-8 fallback, as vcard's `charset`
- [x] Encoded words: tokenizer, B and Q, adjacent-word joining, literal fallback
- [x] Unstructured text form
- [x] Address list form, flat and grouped, with the obsolete syntax
- [x] Message id list form
- [x] Date form, obsolete zones and years, `to_unix_seconds` and `to_rfc3339`
- [x] URL list form (RFC 2369)
- [x] Content-Type and Content-Disposition forms with RFC 2231 continuations and charsets
- [x] Transfer encoding and comma list forms
- [x] `MimeHeaderForm::parse` for standalone values and `read_as` on any field
- [x] `base_subject` (as `value::subject::base_subject`)
- [x] Decoded model types and `MimeHeaderKind` with `ALL`, `FromStr` and `Deref`
- [x] Markers, lenses (read side) and `MimeHeaderSpec` with the kind dispatch
- [x] `MimeCst::decode` for header blocks; `encode` landed with phase 4 as `Mime::to_bytes`
- [x] tests/vectors.rs: every Stalwart field vector, divergences read from the divergence file
- [x] tests/headers.rs: decoded header values of every fixture against the frozen answers
- [x] tests/coverage.rs: every kind parses from its own spelling, every kind's spec answers for it, every form reads back once written
- [x] Linearity tests for encoded word runs, RFC 2231 sections, address lists and folded runs (tests/linearity.rs)
- [x] fuzz/fuzz_targets/fields.rs: every form over arbitrary bytes, never panics, read then write then read is stable
- [x] Headers gate of benchmarked-against-stalwart met
- [x] Fold the delta into cairn/spec/header-fields.md, decoded-model.md and content-encodings.md, write the log entry, archive this change
