# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0 and mail-builder 1.0. The parse workloads are mail-parser's own three, over the 153 vendored fixtures under tests/corpus, as written (about 108 KiB); `build` writes one message of a text body, an HTML body, two attachments and an inline image (about 110 KiB). Measured on 2026-11-02, rustc 1.99, an AMD Ryzen AI Max+ 395 (Zen 5), the bench pinned to one core. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | rival | ratio | gate |
| --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 1,257 MiB/s | 804 MiB/s | 1.56x | 1.0x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 1,071 MiB/s | 786 MiB/s | 1.36x | 0.8x |
| `headers` at structure depth 0, against mail-parser's full parse | 2.42 GiB/s | 786 MiB/s | 3.16x | |
| `headers` at structure depth 0, against mail-parser's `parse_headers` | 2.42 GiB/s | 2.05 GiB/s | 1.18x | |
| `full`: parse, then every leaf decoded, text transcoded | 863 MiB/s | 717 MiB/s | 1.20x | 0.7x |
| `build`: the same message written to bytes, against mail-builder 1.0 | 6.34 GiB/s | 4.85 GiB/s | 1.31x | 1.0x |
| `edit`: the Subject of a message carrying a 2 MiB attachment changed and written out, against mail-parser's parse and mail-builder's rebuild | 74 µs | 404 µs | 5.43x | 5x |
| `edit` at structure depth 0 | 41 µs | 404 µs | 9.86x | |

Every gate is met, and mime-rs is ahead on every row. Against the-steps-inlined, the second pass of 2026-11-02: `structure` 1.37x to 1.56x, `headers` 1.20x to 1.36x, `headers` at depth 0 1.06x to 1.18x, `full` 1.05x to 1.20x. `build` (1.37x to 1.31x) and `edit` (5.24x to 5.43x) moved with the machine, not their code, which did not change: this run had a load near 3.5 from other sessions, and the two versions' benches run interleaved put this pass's `build` 5% ahead of the previous one and `edit` level (`edit` swings 5% between runs, as its 2 MiB copies are memory-bound).


## What moved the numbers

- The Content-Type of an entity is read as a borrowed `MimeMediaType`, type and subtype in place, one parameter read on its own when asked.
- The parse reads a boundary with a single-parameter scan, borrowed from the input, and the media type without allocating.
- An ordinary field is split with one `memchr2`; an answer that passes its check is used as it came.
- The tree is compact: a header field is one leaf holding its whole line, cut by three offsets (40 bytes), anomalies are a bitset, and a multipart body is boxed, so an entity is 80 bytes and a part 104.
- The tree is built in place: each entity is written into its slot rather than collected flat and folded at the end (2026-11-02, 5% to 11% fewer cycles); since the-state-carries-no-data a part joins its multipart when it starts, so the entity being parsed is reached through the last part of each multipart on the way, with no path to keep.
- Allocations per corpus pass went from 2,841 plus 519 reallocations to 2,155 plus 119 (2026-11-02, the-tree-built-in-place): no copy of every parameter value while looking for one, one delimiter buffer per parse, starting capacities for the parse's own vectors. Then from 1,825 plus 41 to 1,543 plus 29 (the-state-carries-no-data, parse and `entities()`): no path vector, no copy of the dash lines, no stack in `entities()`.
- A text leaf is transcoded after core's UTF-8 validation, whose ASCII fast path the lossy decoder lacks, and reads its charset, `format` and `delsp` in one parameter pass and its two content fields in one header walk. With the allocation cuts above, the first step of 2026-11-02, `full` went 29% faster.
- Lazy structure (D13): `MimeParseOptions::structure_depth` stops the parse at the header block, which no full parse can match on `headers`.
- The writer emits base64 straight into the output, writes a multipart's parts straight into the output whenever a generated boundary cannot collide, checks text for `7bit` with core's word-at-a-time scans, and reserves the output once; base64 and quoted-printable run on the SIMD codecs.
- The parse steps are inlined into the coroutine's `resume` (2026-11-02, the-steps-inlined): each step used to return its request through memory, written by narrow stores and read back whole, and the load waited on the stores (a failed store-to-load forwarding). The same for `Delimiter::at`. 3% to 5% fewer cycles on `structure`, `headers` and `full`.
- The root's task starts in the coroutine state instead of a task vector, which a single-part message no longer allocates (2% to 3% fewer cycles on every parse workload).
- The parse reads a Content-Type value once: the type, subtype and boundary in one lexer pass, and not at all when the type opens with a letter other than `m` (neither a multipart nor a message), the common leaf. The media type lexing is one function, shared by the four readers that each had a copy.
- The coroutine state carries no data (2026-11-02, the-state-carries-no-data): the entity in progress and the multipart being split are fields read one by one. Moved whole through the state, a task was copied with wide loads over the narrow stores that had just written it, its padding included, and each load waited on its stores. Answers are checked as they are used rather than in a pass of their own, and a multipart's first part is entered at once rather than pushed then popped. 6% fewer cycles on `structure`, `headers` and `full`.
- A field, media type or parameter name is compared exactly first, then ASCII case-insensitively, the crate's own lookups using the canonical spellings (`Content-Type`): almost every message writes them so, and the case-insensitive compare cost a fifth of `Kind::of`. 3% to 5% fewer cycles on `structure` and `full`.
- The address reader copies runs of plain bytes at once rather than growing its vectors a byte at a time, and the message identifier reader copies once and filters in place rather than reallocating four times: 7% fewer cycles on `headers`, 13% on `headers` at depth 0. `entities()` walks the tree recursively into one vector (2% on `structure`).

## What the profiles show now

Cycles and instructions per corpus pass (`perf stat`, user space, medians of 7 interleaved runs), mime-rs then mail-parser 1.0 in the same harness: `structure` 370k against 640k, `headers` 454k against 652k, `headers` at depth 0 188k against 249k, `full` 562k against 737k. Instructions: 1,669k, 1,853k, 796k, 2,230k (1,857k, 2,049k, 907k, 2,467k before).

Sampled on `structure`: `resume` is 30% of the cycles, its entry and the multipart built on the stack then copied into its box the hot spots; memmem's searcher construction 7.6%, 60% of it one stall inside memchr (it copies the Rabin-Karp finder it has just built); the allocator about 11%; `push_items` 8%; reading Content-Type (`raw_params`, `Kind::of`, `media`) 7%; the harness's own copies of what `content_type()` returns 7%.

Tried on 2026-11-02 and dropped, no gain beyond the noise: a dash-line search by `memchr` on `-` (4% slower, the corpus has dash-heavy text), a newline walk for bodies under 4 KiB (tried twice, 1% at most for more instructions), the lexer's loops rewritten over `position` or over a local cursor (4% more instructions), a word-at-a-time field name compare (fewer instructions, 3% to 6% more cycles on `headers`), the quoted-string scan over `memchr2`, `#[inline]` hints (ignored), the root entity moved out field by field (4% slower on `lazy`), `resume` inlined into the memchr answerer (fewer instructions, 6% more cycles), the pending multipart split moved out of the state enum, with or without `Task::digest` as a word (fewer instructions, cycles flat), `Task::digest` as a word alone (1%, not worth the type), a byte search in `charset::clean_label` (0.2%).

Tried by the-state-carries-no-data and dropped: the memmem searcher searched in place instead of moved into its iterator (fewer copies, cycles flat), the multipart written in its box with its parts filled in place (2% slower), `Box::write` over `Box::new_uninit` (flat), a field's colon found by a byte loop instead of `memchr2` (6% slower: the vector search finds a short name's colon in one step), the last Content-Type looked up from the end of the block (fewer instructions, cycles flat), the date's numbers read by a digit fold instead of `str::parse` (flat), address words borrowed from the field (94 fewer allocations per pass, cycles flat), `#[inline]` on `MimeEntity::content_type` (3.6% slower), `decoded_as` inlined into `text_checked` so its problem flags stay in registers (2% slower: the wait on the decoding's stores moved to the next load).

## Measuring

Criterion's wall-clock medians move by 2% to 3% between builds of the same code (function layout) and with the load of the machine. To compare two versions of the code, count cycles instead: a harness running one workload a fixed number of times, built with `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-loops=64`, run twice under `perf stat -e cycles:u,instructions:u` at two iteration counts (200 and 1200 passes), the difference divided by the passes. One binary repeats within 1%, but two builds of nearly the same code still differ by up to 1.5% from layout alone, so keep both binaries and compare the medians of 5 to 7 interleaved runs, and read the instruction count beside the cycles. Check a parser change against the previous parser with a differential run (both trees compared after random edits of the corpus), not the tests alone.

The processor counts its failed store forwards (`ls_bad_status2.stli_other` on Zen 5): sampling on that event names each stalling copy, which cycle samples only blur. The count is no cost, though: it barely moved while the-state-carries-no-data cut 15% of the cycles. Criterion compared against its own saved numbers misleads once the machine's load changed; run the previous tree's bench beside the new one, interleaved.

## Levers left, in order

The handoff for the next pass, with the measuring harnesses, is [cairn/changes/the-scratch-area](../cairn/changes/the-scratch-area/proposal.md).

1. The dash-line searcher built once per caller rather than once per parse: it is 7.6% of `structure`, but the crate forbids `unsafe` and core has no lazy static, so only an answerer outliving one parse (an API addition) or a per-arch packed-pair finder (target-gated code) can amortize it.
2. Fewer allocations per parse: the answer buffer, the dash lines, the task and delimiter vectors (four per multipart message) could share one scratch area.
3. The copies still stalling in `resume`: the multipart built on the stack then copied into its box, and the root moved out at the end.
4. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.


## Reading the numbers

The parse corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back; mail-builder writes from its own builder, mime-rs from a decoded model any reader can produce.
