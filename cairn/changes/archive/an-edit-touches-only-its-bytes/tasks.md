---
cairn: tasks
change: an-edit-touches-only-its-bytes
---

- [x] Field set, insert and remove on the tree, with placement and line ending rules
- [x] Typed field edits through the markers (`MimeHeaderBlock::set::<SUBJECT>`)
- [x] Address-list item edits keeping the other items' bytes
- [x] Leaf content replacement and transfer re-encoding
- [x] Part insert, remove and move; boundary regeneration on collision; enclosing boundaries avoided
- [x] Wrap into and unwrap from a multipart, attach
- [x] `downgrade_7bit` with its report
- [x] `fill_required`
- [x] Sealing and `unwrap_protected`
- [x] tests/edit.rs: every scenario of the delta, untouched bytes compared
- [x] Ownership test: after each kind of edit, every leaf outside it still borrows the input
- [x] Property test: a random sequence of edits over the corpus keeps every entity a root field edit does not name byte-identical, and the result reads back as edited; 30,000 cases clean
- [x] fuzz/fuzz_targets/edit.rs: arbitrary edits on arbitrary inputs, the property test's oracles
- [x] Edit gate of benchmarked-against-stalwart met (5.05x, 9.2x at structure depth 0)
- [x] Fold the delta into cairn/spec/editing.md, write the log entry, archive this change
