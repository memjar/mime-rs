# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0 and mail-builder 1.0. The parse workloads are mail-parser's own three, over the 153 vendored fixtures under tests/corpus, as written (about 108 KiB); `build` writes one message of a text body, an HTML body, two attachments and an inline image (about 110 KiB). Measured on 2026-10-31, rustc 1.99, Linux x86_64. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | rival | ratio | gate |
| --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 817 MiB/s | 839 MiB/s | 0.97x | 1.0x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 743 MiB/s | 822 MiB/s | 0.90x | 0.8x |
| `headers` at structure depth 0, against mail-parser's full parse | 1.79 GiB/s | 822 MiB/s | 2.18x | |
| `headers` at structure depth 0, against mail-parser's `parse_headers` | 1.79 GiB/s | 2.12 GiB/s | 0.85x | |
| `full`: parse, then every leaf decoded, text transcoded | 534 MiB/s | 717 MiB/s | 0.75x | 0.7x |
| `build`: the same message written to bytes, against mail-builder 1.0 | 2.90 GiB/s | 4.76 GiB/s | 0.61x | 1.0x |
| `edit`: the Subject of a message carrying a 2 MiB attachment changed and written out, against mail-parser's parse and mail-builder's rebuild | 77 µs | 390 µs | 5.05x | 5x |
| `edit` at structure depth 0 | 42 µs | 390 µs | 9.2x | |

The `headers`, `full` and `edit` gates are met, `structure` is within a few percent of its own (mail-parser's figures move by about 4% between runs on this machine), and `build` misses its gate.

## What moved the numbers

- The Content-Type of an entity is read as a borrowed `MimeMediaType`, type and subtype in place, one parameter read on its own when asked.
- The parse reads a boundary with a single-parameter scan, and the media type without allocating.
- An ordinary field is split with one `memchr2`; an answer that passes its check is used as it came; the tree is assembled from one stack.
- The tree is compact: a header field is one leaf holding its whole line, cut by three offsets (40 bytes), anomalies are a bitset, and a multipart body is boxed, so an entity is 80 bytes and a part 104.
- Lazy structure (D13): `MimeParseOptions::structure_depth` stops the parse at the header block, which no full parse can match on `headers`.
- The writer emits base64 straight into the output, writes a multipart's parts straight into the output whenever a generated boundary cannot collide (its prefix never occurs in base64 or quoted-printable output), checks text for `7bit` with core's word-at-a-time scans, and reserves the output once: 2.2x on `build` over the first writer.

## What the profiles show now

Callgrind (perf is not usable without changing the kernel's perf_event_paranoid) on `structure`: the parse is most of the time, spread between building header blocks, allocating their item vectors, and the media type and parameter readers.

On `build`: the base64 crate's encoder is 45% of the instructions (about 7 per input byte, against mail-builder's table encoder translating 12 bits per lookup), copies 12%, and the writer's own work most of the rest.

## Levers left, in order

1. The base64-codec crate as the base64 backend, encoding and decoding: the `build` gate and much of `full` hang on it.
2. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.

## Reading the numbers

The parse corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back; mail-builder writes from its own builder, mime-rs from a decoded model any reader can produce.
