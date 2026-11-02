# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0 and mail-builder 1.0. The parse workloads are mail-parser's own three, over the 153 vendored fixtures under tests/corpus, as written (about 108 KiB); `build` writes one message of a text body, an HTML body, two attachments and an inline image (about 110 KiB). Measured on 2026-11-02, rustc 1.99, an AMD Ryzen AI Max+ 395 (Zen 5), the bench pinned to one core. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | rival | ratio | gate |
| --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 992 MiB/s | 822 MiB/s | 1.21x | 1.0x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 860 MiB/s | 802 MiB/s | 1.07x | 0.8x |
| `headers` at structure depth 0, against mail-parser's full parse | 2.05 GiB/s | 802 MiB/s | 2.61x | |
| `headers` at structure depth 0, against mail-parser's `parse_headers` | 2.05 GiB/s | 2.11 GiB/s | 0.97x | |
| `full`: parse, then every leaf decoded, text transcoded | 722 MiB/s | 735 MiB/s | 0.98x | 0.7x |
| `build`: the same message written to bytes, against mail-builder 1.0 | 6.69 GiB/s | 4.97 GiB/s | 1.35x | 1.0x |
| `edit`: the Subject of a message carrying a 2 MiB attachment changed and written out, against mail-parser's parse and mail-builder's rebuild | 74 µs | 391 µs | 5.27x | 5x |
| `edit` at structure depth 0 | 42 µs | 391 µs | 9.36x | |

Every gate is met. Against the 2026-10-31 run: `structure` 0.97x to 1.21x, `headers` 0.90x to 1.07x, `full` 0.75x to 0.98x, `build` 0.61x to 1.35x (the SIMD codecs), mail-parser's own rows within 1%.

## What moved the numbers

- The Content-Type of an entity is read as a borrowed `MimeMediaType`, type and subtype in place, one parameter read on its own when asked.
- The parse reads a boundary with a single-parameter scan, borrowed from the input, and the media type without allocating.
- An ordinary field is split with one `memchr2`; an answer that passes its check is used as it came.
- The tree is compact: a header field is one leaf holding its whole line, cut by three offsets (40 bytes), anomalies are a bitset, and a multipart body is boxed, so an entity is 80 bytes and a part 104.
- The tree is built in place: each entity is written into its slot, reached from the root by a short path of part positions, rather than collected flat and folded at the end (2026-11-02, 5% to 11% fewer cycles).
- Allocations per corpus pass went from 2,841 plus 519 reallocations to 2,155 plus 119 (2026-11-02): no copy of every parameter value while looking for one, the parts allocated once with their entities, one delimiter buffer per parse, starting capacities for the parse's own vectors.
- A text leaf is transcoded after core's UTF-8 validation, whose ASCII fast path the lossy decoder lacks, and reads its charset, `format` and `delsp` in one parameter pass and its two content fields in one header walk. With the allocation cuts above, the first step of 2026-11-02, `full` went 29% faster.
- Lazy structure (D13): `MimeParseOptions::structure_depth` stops the parse at the header block, which no full parse can match on `headers`.
- The writer emits base64 straight into the output, writes a multipart's parts straight into the output whenever a generated boundary cannot collide, checks text for `7bit` with core's word-at-a-time scans, and reserves the output once; base64 and quoted-printable run on the SIMD codecs.

## What the profiles show now

Cycles and instructions per corpus pass (`perf stat`, user space), mime-rs then mail-parser 1.0: `structure` 479k against 622k, `headers` 568k against 640k to 669k, `headers` at depth 0 226k against 250k, `full` 680k against 705k. The instruction count is what to cut: the parse retires about 4.2 instructions a cycle.

Callgrind on `structure`: the allocator is about 18% of the instructions (glibc's `free` path most of it), reading Content-Type (the media type, the field name compares, the boundary) about 12%, building the memmem searcher for the dash lines about 840 instructions per multipart message.

Tried on 2026-11-02 and dropped, no gain beyond the noise: a dash-line search by `memchr` on `-` (4% slower, the corpus has dash-heavy text), a newline walk for bodies under 4 KiB, the lexer's loops rewritten over `position`, a word-at-a-time field name compare (fewer instructions, 3% to 6% more cycles on `headers`), inlining `Delimiter::at`.

## Measuring

Criterion's wall-clock medians move by 2% to 3% between builds of the same code (function layout) and with the load of the machine. To compare two versions of the code, count cycles instead: a harness running one workload a fixed number of times, built with `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-loops=64`, run twice under `perf stat -e cycles:u,instructions:u` at two iteration counts (200 and 1200 passes), the difference divided by the passes. That repeats within 1% (3% on the depth 0 row). Check a parser change against the previous parser with a differential run (both trees compared after random edits of the corpus), not the tests alone.

## Levers left, in order

The handoff for the next pass, with the measuring harnesses, is [cairn/changes/the-scratch-area](../cairn/changes/the-scratch-area/proposal.md).

1. Fewer allocations per parse: the answer buffer, the copy of the dash lines and the task and path vectors could share one scratch area; `entities()` allocates its result and its stack.
2. A dash-line searcher built once per caller rather than once per parse.
3. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.

## Reading the numbers

The parse corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back; mail-builder writes from its own builder, mime-rs from a decoded model any reader can produce.
