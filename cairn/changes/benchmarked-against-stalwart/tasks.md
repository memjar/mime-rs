---
cairn: tasks
change: benchmarked-against-stalwart
---

- [ ] Phase 0: port the corpus selection from mail-parser's benches/support/corpus.rs, with attribution
- [ ] Phase 0: benches/compare.rs with the rivals only, answer checks before timing
- [ ] Phase 0: codec micro-benchmarks, mail-parser's kernels bench and the base64 comparison for D4
- [ ] Phase 1 onwards: try the levers of the proposal in order, keeping each only when its workload moves
- [ ] Phase 0: baseline tables in benches/README.md
- [ ] Phase 1: `structure` and `round-trip` for this crate; revisit the gates with these numbers
- [ ] Phase 2: `headers`; address and date reader micro-benchmarks
- [ ] Phase 3: `full`
- [ ] Phase 4: `build` against mail-builder 1.0 and lettre
- [ ] Phase 5: `edit` against parse plus rebuild
- [ ] Counting-allocator report per workload
- [ ] Phase 7: final tables, optional cross-language ballpark
- [ ] Write the log entry, archive this change
