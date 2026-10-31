---
cairn: tasks
change: an-edit-touches-only-its-bytes
---

- [ ] Field set, insert and remove on the tree, with placement and line ending rules
- [ ] Lens cursors for every marker, write side
- [ ] Address-list item edits keeping the other items' bytes
- [ ] Leaf content replacement and transfer re-encoding
- [ ] Part insert, remove and move; boundary regeneration on collision
- [ ] Wrap into and unwrap from a multipart
- [ ] `downgrade_7bit` with its report
- [ ] `fill_required`
- [ ] Sealing and `unwrap_protected`
- [ ] tests/edit.rs: every scenario of the delta over corpus fixtures, untouched entities compared byte for byte
- [ ] Ownership test: after each kind of edit, every leaf outside it still borrows the input
- [ ] Property test: a random sequence of edits keeps every untouched field and entity byte-identical, and the result reparses to what the edits state
- [ ] fuzz/fuzz_targets/edit.rs: arbitrary edits on arbitrary inputs, the property test's oracles
- [ ] Edit gate of benchmarked-against-stalwart met
- [ ] Fold the delta into cairn/spec/editing.md, write the log entry, archive this change
