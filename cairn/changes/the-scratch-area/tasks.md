---
cairn: tasks
change: the-scratch-area
---

- [ ] Rebuild the cycle harness (with `edit`), the interleaved A/B driver, the stall counts and the differential run (noisy answerer and field readers included) from the proposal; baseline cycles of the current commit and of mail-parser
- [ ] The dash-line searcher built once per caller (an answerer outliving a parse is the user's call), measured on `edit` too
- [ ] One scratch area per parse (answer buffer, dash lines, tasks, delimiters), the public coroutine API unchanged
- [ ] The copies still stalling in `resume` (the multipart's `Box::new`, the root at completion), each confirmed by an A/B
- [ ] The From and Subject readers' allocations, measured in cycles
- [ ] Suite, clippy, bare core build, docs, fmt, differential run
- [ ] Criterion run, interleaved with the previous tree's bench under load; README.md and benches/README.md updated
- [ ] Log entry, change archived, handoff for the next pass
