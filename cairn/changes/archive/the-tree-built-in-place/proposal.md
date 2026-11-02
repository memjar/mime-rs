---
cairn: change
id: the-tree-built-in-place
status: landed
created: 2026-11-02
---

# The tree built in place

A speed pass over the parse and the leaf readers, asked by the user on 2026-11-02 after the SIMD codecs landed: run the benchmarks, put them in the README as base64-codec and quoted-printable-codec do, then find a way to improve them. Part of [benchmarked-against-stalwart](../../benchmarked-against-stalwart/proposal.md).

## Why

On the morning's run `structure` sat at 0.97x mail-parser 1.0, `headers` at 0.91x and `full` at 0.73x. The profiles were flat: no hot loop, the time spread over the parse's bookkeeping, about 13% in malloc and free, and copies of whole structs (header blocks, flat entities, tasks) assembled field by field and moved through the coroutine.

## What

No behaviour changes: the same tree, byte for byte, the same anomalies, the same answers to every reader.

- The parser writes each entity into its final slot in the tree, reached from the root by the positions of a short path (a task carries its position in its parent; tasks run depth first, so the path down to a task's parent is the head of the current one). The flat list of entities and the fold assembling them at the end are gone; a multipart allocates its parts once, with default entities filled in place.
- `raw_param` returns a `Cow` borrowed from the value, skips the parameters it does not want without copying them, and reads several names in one pass (`raw_params`); a multipart's boundary is borrowed from the input whenever its field is.
- One delimiter buffer per parse, reused by every multipart; starting capacities for the parse's vectors, the answer buffer and `entities`.
- A text leaf runs core's UTF-8 validation before the lossy decoder, reads its Content-Type and Content-Transfer-Encoding in one walk of the header block and its charset, `format` and `delsp` in one parameter pass; `MimeMediaType::param` converts its value without a second copy.
- `rfc2047::contains_word_start` scans for `=` instead of comparing every two-byte window.
- The README gains a Benchmarks section; benches/README.md is rerun and gains the measuring method.

## Measuring

Criterion medians move by 2% to 3% between builds of the same code and with the machine's load, which other sessions shared during this pass. Decisions were taken on `perf stat` cycles of a fixed-count harness built with aligned functions and loops (difference of two iteration counts), which repeat within 1%. Every parser step was checked against the previous parser by a differential run: the corpus and 300 to 500 rounds of random edits of it, at three structure depths with tight entity and depth bounds, the two trees compared whole and expanded; a deliberate break in the old copy was caught.

## Risks

- `current()` indexes the tree by the path and reaches `unreachable!` on a leaf, as the edit module's `at_mut` does: the path is only ever built from parts and messages the parse itself created.
