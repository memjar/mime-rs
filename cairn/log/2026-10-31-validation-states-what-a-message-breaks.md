---
cairn: log
change: validation-states-what-a-message-breaks
landed: 2026-10-31
---

# Validation states what a message breaks

Landed the validator: `Mime::check` and `Mime::validate` on the model (no parser), `MimeCst::check` and `MimeCst::validate` on the tree with the wire rules, every problem a `MimeValidateError` with its `MimeValidateRule`, RFC section and `MimeSeverity`, the `MimeValid<T>` proof carrying the advisories, and `MimeTransport`. The rules shared by both walks (message fields, field values, Content-ID uniqueness, the RFC 1847, 2387 and 6522 structures) sit in `validator` and read either the model or the fields as written.

Decisions that moved from the plan: the strict field grammar is checked through what the readers make of a value (a known field reading as raw, a date naming no day, a mailbox with no full address, an identifier without `@`) and the obsolete syntax through dates alone, rather than a second, strict grammar per form; a message stored with LF line endings gets one advisory rather than a violation per line, since local stores keep messages that way; and the model check leaves MIME-Version and boundaries to the tree, since the writer adds them.

The builder's property test now asserts that a written message carries no violation its model's check does not state (30,000 cases clean), and the validator found two quoted-printable encoder bugs on its first run: a blank before a lone CR kept no column for the soft break, and an `F` or `.` pushed to the start of a line by a soft break was escaped but counted as one character, each line reaching 77 or 78 characters. Both are fixed, with tests failing on the old code. RFC 2231 sections are now sized to fit their line with the parameter name and charset, which a 79-character `filename*0*=` line had shown.

The vendored corpus classifies as 0 valid, 9 with advisories only and 144 with violations, these being test fixtures, mostly without Date or From.

Capabilities moved: conformance (ADDED).
