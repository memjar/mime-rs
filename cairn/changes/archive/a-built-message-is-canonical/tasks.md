---
cairn: tasks
change: a-built-message-is-canonical
---

- [x] Base64 and quoted-printable encoders with their known-answer and round-trip tests
- [x] Encoded word writer: B or Q, 75-character words, whole characters only
- [x] Phrase writer: atoms, quoted strings, encoded words
- [x] Folder: 76-character target, 998 hard limit, fold at existing whitespace
- [x] RFC 2231 parameter writer, with the legacy `name` option
- [x] Transfer encoding choice and canonical line endings for text
- [x] Flowed encoder
- [x] Boundary generator checked against enclosed content
- [x] `MimeBuilder` and `MimeBuildOptions`, with the mail-builder layout
- [x] `MimeMessageId::new`
- [x] `Mime::to_bytes`, the writer of whole messages
- [x] Property test: an arbitrary model under arbitrary options builds, reads back as built and rebuilds to the same bytes (tests/build.rs); validation asserted once phase 6 lands
- [x] Differential test: mail-parser 1.0 reads every built message with the structure and subject the model states, and the scenario tests with its names, attachments and nested bodies
- [x] Folding test: the rules from two starting columns, mixed scripts
- [x] fuzz/fuzz_targets/write.rs: the model of arbitrary bytes, written, rebuilds to the same bytes; findings kept in tests/build.rs
- [ ] Build gate of benchmarked-against-stalwart met (0.65x; the profile is in the log entry, the lever is the base64-codec crate)
- [x] Fold the delta into cairn/spec/building.md, write the log entry, archive this change
