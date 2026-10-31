# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0, on mail-parser's own three workloads over the 183 vendored fixtures under tests/corpus, as written (about 108 KiB). Measured on 2026-10-31, rustc 1.99, Linux x86_64. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | mail-parser 1.0 | ratio | gate | first run |
| --- | --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 740 MiB/s | 813 MiB/s | 0.91x | 1.0x | 0.35x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 682 MiB/s | 800 MiB/s | 0.85x | 0.8x | 0.43x |
| `full`: parse, then every leaf decoded, text transcoded | 528 MiB/s | 733 MiB/s | 0.72x | 0.7x | 0.35x |

The `headers` and `full` gates are met; `structure` is not. mail-parser's own figures move by about 4% between runs on this machine.

## What moved the numbers

- The Content-Type of an entity is read as a borrowed `MimeMediaType`, type and subtype in place, one parameter read on its own when asked, instead of an owned `MimeContentType` rebuilt on every read.
- The parse reads a boundary with a single-parameter scan, and the media type without allocating.
- An ordinary field is split with one `memchr2`; an answer that passes its check is used as it came; the tree is assembled from one stack.
- The typed lens reads a field in its marker's form, without deriving the kind from the name.

## What the profile shows now

Callgrind (perf is not usable without changing the kernel's perf_event_paranoid) on `structure`: the parse is most of the time, spread between building header blocks, freeing and moving the tree's structs (a `Cow<[u8]>` leaf is 24 bytes, a field holds four), and the media type and parameter readers.

## Levers left, in order

1. Lazy structure (D13): a parse that stops at the header block, which no full parse can match on `headers`.
2. A compact leaf (an input span or an owned buffer behind the same API), and fewer vectors per entity.
3. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.

## Reading the numbers

The corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The two libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back.
