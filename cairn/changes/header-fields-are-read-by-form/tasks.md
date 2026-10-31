---
cairn: tasks
change: header-fields-are-read-by-form
---

- [ ] `MimeBase64::decode` and `MimeQuotedPrintable::decode` over the base64 and quoted_printable crates, behind their features: line-by-line `decode_slice` with a filtering fallback, Q mode, problems reported
- [ ] `MimeCharset` over `encoding_rs::Encoding::for_label` behind `encoding`, UTF-8 fallback, as vcard's `charset`
- [ ] Encoded words: tokenizer, B and Q, adjacent-word joining, literal fallback
- [ ] Unstructured text form
- [ ] Address list form, flat and grouped, with the obsolete syntax
- [ ] Message id list form
- [ ] Date form, obsolete zones and years, `to_unix_seconds` and `to_rfc3339`
- [ ] URL list form (RFC 2369)
- [ ] Content-Type and Content-Disposition forms with RFC 2231 continuations and charsets
- [ ] Transfer encoding and comma list forms
- [ ] `MimeHeaderForm::parse` for standalone values and `read_as` on any field
- [ ] `MimeText::base_subject`
- [ ] Decoded model types and `MimeHeaderKind` with `ALL`, `FromStr` and `Deref`
- [ ] Markers, lenses (read side) and `MimeHeaderSpec` with the kind dispatch
- [ ] `MimeCst::decode` and `encode` for header blocks
- [ ] tests/vectors.rs: every Stalwart field vector, divergences read from the divergence file
- [ ] tests/headers.rs: decoded header values of every fixture against the frozen answers
- [ ] tests/coverage.rs: every kind parses from its own spelling, every marker's spec answers for its kind, every form decodes and encodes back
- [ ] Linearity tests for folded runs, RFC 2231 sections and encoded word runs
- [ ] fuzz/fuzz_targets/fields.rs: every form over arbitrary bytes, never panics, decode then encode then decode is stable
- [ ] Headers gate of benchmarked-against-stalwart met
- [ ] Fold the delta into cairn/spec/header-fields.md, decoded-model.md and content-encodings.md, write the log entry, archive this change
