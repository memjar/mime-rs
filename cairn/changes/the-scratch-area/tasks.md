---
cairn: tasks
change: the-scratch-area
---

- [ ] Rebuild the cycle harness and the differential run from the proposal; baseline cycles of the current commit and of mail-parser
- [ ] One scratch area per parse (tasks, path, delimiters, dash lines), the public coroutine API unchanged
- [ ] `MimeCst::entities` with one allocation, pre-order and no recursion kept
- [ ] The dash-line searcher built once per caller, measured on `edit` too
- [ ] `MimeMediaType::new` and `raw_params` with fewer instructions per byte, checked against the lexer
- [ ] Charset labels compared without trimming first
- [ ] Suite, clippy, bare core build, docs, fmt, differential run
- [ ] Quiet-machine criterion run; README.md and benches/README.md updated
- [ ] Log entry, change archived, handoff for the next pass
