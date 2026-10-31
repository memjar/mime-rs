# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0, on mail-parser's own three workloads over the 183 vendored fixtures under tests/corpus, as written (about 108 KiB). Measured on 2026-10-31, rustc 1.99, Linux x86_64. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | mail-parser 1.0 | ratio | gate | first run |
| --- | --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 836 MiB/s | 846 MiB/s | 0.99x | 1.0x | 0.35x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 762 MiB/s | 807 MiB/s | 0.94x | 0.8x | 0.43x |
| `full`: parse, then every leaf decoded, text transcoded | 557 MiB/s | 739 MiB/s | 0.75x | 0.7x | 0.35x |

The `headers` and `full` gates are met, and `structure` is within measurement noise of its own. mail-parser's own figures move by about 4% between runs on this machine.

## What moved the numbers

- The Content-Type of an entity is read as a borrowed `MimeMediaType`, type and subtype in place, one parameter read on its own when asked, instead of an owned `MimeContentType` rebuilt on every read.
- The parse reads a boundary with a single-parameter scan, and the media type without allocating.
- An ordinary field is split with one `memchr2`; an answer that passes its check is used as it came; the tree is assembled from one stack.
- The typed lens reads a field in its marker's form, without deriving the kind from the name.
- The tree is compact: a header field is one leaf holding its whole line, cut by three offsets (40 bytes, down from 96), anomalies are a bitset, and a multipart body is boxed, so an entity is 80 bytes (down from 168) and a part 104 (down from 192).

## What the profile shows now

Callgrind (perf is not usable without changing the kernel's perf_event_paranoid) on `structure`: the parse is most of the time, spread between building header blocks, allocating their item vectors, and the media type and parameter readers.

## Levers left, in order

1. Lazy structure (D13): a parse that stops at the header block, which no full parse can match on `headers`.
2. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.

## Reading the numbers

The corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The two libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back.
