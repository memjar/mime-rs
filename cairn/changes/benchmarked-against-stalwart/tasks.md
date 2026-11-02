---
cairn: tasks
change: benchmarked-against-stalwart
---

- [ ] Phase 0: port the corpus selection from mail-parser's benches/support/corpus.rs, with attribution
- [x] Phase 0: benches/compare.rs with the rivals, answer checks before timing
- [ ] Phase 0: codec micro-benchmarks, mail-parser's kernels bench and the base64 comparison for D4
- [x] Phase 1 onwards: try the levers of the proposal in order, keeping each only when its workload moves (lazy structure landed)
- [x] Phase 0: baseline tables in benches/README.md
- [ ] Phase 1: `structure` and `round-trip` for this crate; revisit the gates with these numbers (`structure` done, 1.37x since the-steps-inlined; no `round-trip` workload yet)
- [ ] Phase 2: `headers`; address and date reader micro-benchmarks (`headers` done, 1.20x and 2.78x lazy since the-steps-inlined; no micro-benchmarks yet)
- [x] Phase 3: `full` (0.75x, 1.05x since the-steps-inlined)
- [x] Phase 4: `build` against mail-builder 1.0 (lettre left out: a heavy dependency for a sanity line); gate missed, see the phase 4 log; met since the SIMD codecs (1.35x on 2026-11-02)
- [x] Phase 5: `edit` against parse plus rebuild, gate met
- [ ] Counting-allocator report per workload
- [ ] Phase 7: final tables, optional cross-language ballpark
- [ ] Write the log entry, archive this change
