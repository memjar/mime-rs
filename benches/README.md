# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0 and mail-builder 1.0. The parse workloads are mail-parser's own three, over the 153 vendored fixtures under tests/corpus, as written (about 108 KiB); `build` writes one message of a text body, an HTML body, two attachments and an inline image (about 110 KiB). Measured on 2026-11-02, rustc 1.99, an AMD Ryzen AI Max+ 395 (Zen 5), the bench pinned to one core. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | rival | ratio | gate |
| --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 1,132 MiB/s | 829 MiB/s | 1.37x | 1.0x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 971 MiB/s | 812 MiB/s | 1.20x | 0.8x |
| `headers` at structure depth 0, against mail-parser's full parse | 2.20 GiB/s | 812 MiB/s | 2.78x | |
| `headers` at structure depth 0, against mail-parser's `parse_headers` | 2.20 GiB/s | 2.07 GiB/s | 1.06x | |
| `full`: parse, then every leaf decoded, text transcoded | 783 MiB/s | 744 MiB/s | 1.05x | 0.7x |
| `build`: the same message written to bytes, against mail-builder 1.0 | 6.72 GiB/s | 4.91 GiB/s | 1.37x | 1.0x |
| `edit`: the Subject of a message carrying a 2 MiB attachment changed and written out, against mail-parser's parse and mail-builder's rebuild | 74 µs | 387 µs | 5.24x | 5x |
| `edit` at structure depth 0 | 39 µs | 387 µs | 10.0x | |

Every gate is met, and mime-rs is ahead on every row. Against the morning of 2026-11-02 (the-tree-built-in-place): `structure` 1.21x to 1.37x, `headers` 1.07x to 1.20x, `headers` at depth 0 0.97x to 1.06x, `full` 0.98x to 1.05x; `build` and `edit` did not move (their code did not change; `edit` swings 5% between runs, as its 2 MiB copies are memory-bound). The `full` and `edit` rows come from a second run at a lower machine load.


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
- The parse steps are inlined into the coroutine's `resume` (2026-11-02, the-steps-inlined): each step used to return its request through memory, written by narrow stores and read back whole, and the load waited on the stores (a failed store-to-load forwarding). The same for `Delimiter::at`. 3% to 5% fewer cycles on `structure`, `headers` and `full`.
- The root's task starts in the coroutine state instead of a task vector, which a single-part message no longer allocates (2% to 3% fewer cycles on every parse workload).
- The parse reads a Content-Type value once: the type, subtype and boundary in one lexer pass, and not at all when the type opens with a letter other than `m` (neither a multipart nor a message), the common leaf. The media type lexing is one function, shared by the four readers that each had a copy.

## What the profiles show now

Cycles and instructions per corpus pass (`perf stat`, user space, medians of 7 interleaved runs), mime-rs then mail-parser 1.0: `structure` 431k against 623k, `headers` 522k against 634k, `headers` at depth 0 220k against 248k, `full` 625k against 705k. Instructions: 1,857k, 2,049k, 907k, 2,467k (2,023k, 2,221k, 956k, 2,618k before).

Callgrind on `structure`: the allocator is about 20% of the instructions, 40% of the 1,554 allocations per pass being scratch (the answer buffer, the task, path and delimiter vectors, the copy of the dash lines) and `entities()` adding 306 more; reading Content-Type (the media type, the field name compares, the boundary) about 20% with the bench's own `content_type()` calls; the dash-line search 6.5%, a third of it building the memmem searcher. In cycles, `resume` still loses about a tenth of `structure` and `lazy` to two copies stalling the same way: the whole coroutine state at the top of `resume` (its unused bytes and the padding after `Task::digest`), and the root entity moved out at the end (its anomaly byte just written).

Tried on 2026-11-02 and dropped, no gain beyond the noise: a dash-line search by `memchr` on `-` (4% slower, the corpus has dash-heavy text), a newline walk for bodies under 4 KiB (tried twice, 1% at most for more instructions), the lexer's loops rewritten over `position` or over a local cursor (4% more instructions), a word-at-a-time field name compare (fewer instructions, 3% to 6% more cycles on `headers`), the quoted-string scan over `memchr2`, `#[inline]` hints (ignored), the root entity moved out field by field (4% slower on `lazy`), `resume` inlined into the memchr answerer (fewer instructions, 6% more cycles), the pending multipart split moved out of the state enum, with or without `Task::digest` as a word (fewer instructions, cycles flat), `Task::digest` as a word alone (1%, not worth the type), a byte search in `charset::clean_label` (0.2%).

## Measuring

Criterion's wall-clock medians move by 2% to 3% between builds of the same code (function layout) and with the load of the machine. To compare two versions of the code, count cycles instead: a harness running one workload a fixed number of times, built with `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-loops=64`, run twice under `perf stat -e cycles:u,instructions:u` at two iteration counts (200 and 1200 passes), the difference divided by the passes. One binary repeats within 1%, but two builds of nearly the same code still differ by up to 1.5% from layout alone, so keep both binaries and compare the medians of 5 to 7 interleaved runs, and read the instruction count beside the cycles. Check a parser change against the previous parser with a differential run (both trees compared after random edits of the corpus), not the tests alone.

## Levers left, in order

The handoff for the next pass, with the measuring harnesses, is [cairn/changes/the-scratch-area](../cairn/changes/the-scratch-area/proposal.md).

1. The two stalling copies left in `resume`: the coroutine state copied whole on every resume, and the root moved out at the end.
2. Fewer allocations per parse: the answer buffer, the copy of the dash lines and the task, path and delimiter vectors could share one scratch area; `entities()` allocates its result and its stack.
3. A dash-line searcher built once per caller rather than once per parse.
4. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.


## Reading the numbers

The parse corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back; mail-builder writes from its own builder, mime-rs from a decoded model any reader can produce.
