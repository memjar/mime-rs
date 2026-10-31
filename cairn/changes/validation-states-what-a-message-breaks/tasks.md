---
cairn: tasks
change: validation-states-what-a-message-breaks
---

- [ ] `MimeValidateError` with entity, field, rule, RFC section and severity
- [ ] Message-level rules: field counts, Sender, Resent- blocks, trace placement
- [ ] Field rules: strict grammar per form, obsolete syntax, encoded word placement and length, raw 8-bit bytes
- [ ] Line rules: 998 and 78, bare CR and LF, NUL
- [ ] MIME rules: MIME-Version, boundaries, composite encodings, declared versus actual content, required parameters, Content-ID uniqueness
- [ ] Parse anomalies reported as violations
- [ ] `MimeValid<T>` and its conversion back into a tree
- [ ] `MimeTransport`
- [ ] tests/validate.rs: every rule with a message breaking it and one keeping it
- [ ] Corpus classification with asserted counts per directory
- [ ] The builder's property test now also asserts every built message validates
- [ ] Fold the delta into cairn/spec/conformance.md, write the log entry, archive this change
