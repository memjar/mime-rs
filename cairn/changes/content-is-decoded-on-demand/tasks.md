---
cairn: tasks
change: content-is-decoded-on-demand
---

- [ ] `bytes`, `quoted_printable`, `base64`, `charset`, `decoded_len`, `text_prefix` and their checked variants on leaves
- [ ] `MimeDecodeProblems`: malformed transfer encoding, unknown transfer encoding, unknown charset, malformed charset data
- [ ] Charset selection for a text part: BOM, parameter, UTF-8 fallback
- [ ] `MimeFlowed` decoder and its RFC 3676 examples
- [ ] `MimeBodyLists` over the parseStructure algorithm, `has_attachment`
- [ ] `nested` for encoded `message/rfc822` leaves
- [ ] `decode` fills leaf content
- [ ] tests/bodies.rs: decoded content and body lists of every fixture against the frozen answers, divergences read from the divergence file
- [ ] tests/flowed.rs: the RFC 3676 examples, space-stuffing, `DelSp=yes`, quoted paragraphs
- [ ] Ignored sweep: every leaf of the fetched corpora decodes without panicking and agrees with mail-parser 1.0
- [ ] fuzz/fuzz_targets/decode.rs: transfer codecs, charsets and flowed text over arbitrary bytes, never panics
- [ ] Full gate of benchmarked-against-stalwart met
- [ ] Fold the delta into cairn/spec/content-decoding.md and decoded-model.md, write the log entry, archive this change
