---
cairn: tasks
change: validation-states-what-a-message-breaks
---

- [x] `MimeValidateError` with entity, field, rule, RFC section and severity
- [x] Message-level rules: field counts, Sender, Resent- blocks, trace placement
- [x] Field rules: values in their form, obsolete dates, encoded word placement and length, raw 8-bit bytes
- [x] Line rules: 998 and 78, stray CR and LF, NUL, LF line endings
- [x] MIME rules: MIME-Version, boundaries, composite encodings, declared versus actual content, required parameters, Content-ID uniqueness
- [x] Parse anomalies reported as violations
- [x] `MimeValid<T>` and its conversion back
- [x] `MimeTransport`
- [x] tests/validate.rs: every rule with a message breaking it and one keeping it
- [x] Corpus classification with asserted counts per directory
- [x] The builder's property test now also asserts the writer adds no violation, and a complete built message carries no problem
- [x] Fold the delta into cairn/spec/conformance.md, write the log entry, archive this change
