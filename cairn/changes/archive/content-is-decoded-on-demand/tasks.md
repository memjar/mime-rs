---
cairn: tasks
change: content-is-decoded-on-demand
---

- [x] `bytes`, `quoted_printable`, `base64`, `decoded`, `decoded_len`, `text`, `text_prefix` and their checked variants on leaves
- [x] `MimeDecodeProblems`: malformed transfer encoding, unknown transfer encoding, unknown charset, malformed charset data
- [x] Charset selection for a text part: BOM, parameter, UTF-8 fallback
- [x] Flowed decoder and its RFC 3676 cases
- [x] `MimeBodyLists` over the parseStructure algorithm, `has_attachment`
- [x] `nested` for encoded `message/rfc822` leaves
- [x] `decode` fills leaf content: `MimeBody::Text` and `MimeBody::Binary`, encoded nested messages parsed
- [x] tests/decoded.rs: decoded content and body lists of every fixture against the frozen answers, divergences read from the divergence file
- [x] Flowed cases as unit tests in src/codec/flowed.rs: soft breaks, DelSp, quote depth, the signature, space-stuffing, a wide character at the margin
- [ ] Ignored sweep: every leaf of the fetched corpora decodes without panicking and agrees with mail-parser 1.0 (waits for the fetch script, phase 7)
- [x] fuzz/fuzz_targets/decode.rs: transfer codecs, charsets and flowed text over arbitrary bytes, never panics, encoders exact
- [x] Full gate of benchmarked-against-stalwart met
- [x] Fold the delta into cairn/spec/content-decoding.md and decoded-model.md, write the log entry, archive this change
