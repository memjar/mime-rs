---
cairn: log
change: the-tree-built-in-place
landed: 2026-11-02
---

# The tree built in place

Landed a speed pass with no behaviour change. The parser writes every entity into its slot in the tree, reached by a path of part positions, instead of collecting flat entities and folding them at the end; parameter values are borrowed and read several at a time; a multipart's boundary is borrowed from the input; one delimiter buffer serves a whole parse; a text leaf validates UTF-8 with core before the lossy decoder and reads its content fields and parameters in one pass each.

Criterion on 2026-11-02 against the same morning: `structure` 132 to 106 µs (1.21x mail-parser 1.0, from 0.97x), `headers` 144 to 122 µs (1.07x, from 0.91x), `headers` at depth 0 59 to 50 µs, `full` 198 to 145 µs (0.98x, from 0.73x), `edit` 77 to 74 µs (5.27x), `build` 16.4 µs (1.35x mail-builder, the codecs' doing). Cycles per corpus pass against HEAD 2bcc406: `structure` 634k to 479k, `headers` 693k to 568k, depth 0 276k to 226k, `full` 880k to 680k. Allocations per pass: 2,841 plus 519 reallocations to 2,155 plus 119.

The README has a Benchmarks section; benches/README.md has the new table, the measuring method (cycles of an aligned fixed-count harness, criterion's medians moving 2% to 3% between builds) and what was tried and dropped.

Capabilities moved: none.
