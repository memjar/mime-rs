---
cairn: tasks
change: the-tree-built-in-place
---

- [x] Baseline run of the whole bench, profiles (perf, callgrind), allocation counts
- [x] Allocation cuts: borrowed parameter values, parts allocated once, starting capacities
- [x] Text leaves: UTF-8 fast path, one header walk, one parameter pass
- [x] The tree built in place, differential run against the previous parser
- [x] Borrowed boundary, one delimiter buffer per parse, word-start scan
- [x] Tried and dropped: dash search by `-`, newline walk for small bodies, lexer loops over `position`, word-at-a-time name compare, inlined `Delimiter::at`, a reserved or overwritten field push
- [x] Tests, clippy, bare core build, docs; README Benchmarks section, benches/README.md rerun
- [x] Log entry, change archived
