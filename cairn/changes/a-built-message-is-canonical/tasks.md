---
cairn: tasks
change: a-built-message-is-canonical
---

- [ ] `MimeBase64::encode` and `MimeQuotedPrintable::encode` with their known-answer tests
- [ ] Encoded word writer: B or Q, 75-character words, whole characters only
- [ ] Phrase writer: atoms, quoted strings, encoded words
- [ ] Folder: 78-character target, 998 hard limit, fold at existing whitespace
- [ ] RFC 2231 parameter writer, with the legacy `name` option
- [ ] Transfer encoding choice and canonical line endings for text
- [ ] `MimeFlowed::encode`
- [ ] Boundary generator checked against enclosed content
- [ ] `MimeBuilder` and `MimeBuildOptions`, with the mail-builder layout
- [ ] `MimeMessageId::new`
- [ ] `encode` of whole messages into `MimeCst`
- [ ] Property test: an arbitrary model builds, reparses and decodes to itself, and validates once phase 6 lands
- [ ] Differential test: mail-parser 1.0 reads every built message with the structure and decoded values the model states
- [ ] Folding test: every rule above from any starting column, every field form
- [ ] fuzz/fuzz_targets/build.rs: arbitrary models, never panics, the property test's oracles
- [ ] Build gate of benchmarked-against-stalwart met
- [ ] Fold the delta into cairn/spec/building.md and conformance.md, write the log entry, archive this change
