# Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` (source in [compare.rs](./compare.rs)) against mail-parser 1.0, on mail-parser's own three workloads over the 183 vendored fixtures under tests/corpus, as written (about 108 KiB). Measured on 2026-10-31, rustc 1.99, Linux x86_64. A dependency bump moves them, so re-run the bench after one.

| workload | mime-rs | mail-parser 1.0 | ratio | gate |
| --- | --- | --- | --- | --- |
| `structure`: parse, then the content type of every part | 433 MiB/s | 845 MiB/s | 0.51x | 1.0x |
| `headers`: parse, then Subject, first From, Date timestamp, Message-ID | 534 MiB/s | 838 MiB/s | 0.64x | 0.8x |
| `full`: parse, then every leaf decoded, text transcoded | 357 MiB/s | 747 MiB/s | 0.48x | 0.7x |

No gate is met yet.

## What the profile showed

Callgrind (perf is not usable without changing the kernel's perf_event_paranoid) on the `structure` workload, after a first round of fixes that brought each workload up by 23 to 29 percent:

- Reading the Content-Type of a part into the owned `MimeContentType` is about a quarter of the time, its `String`s allocated and dropped once per read. mail-parser parses it once during the parse and hands out a borrowed view.
- Allocation and drop of the tree's vectors and `Cow` leaves are about a sixth.
- Building the header block items is about a sixth.

## Levers, in order

1. A borrowed or cached content type, parsed once per entity during the parse.
2. Lazy structure (D13): a parse that stops at the header block, which no full parse can match on `headers`.
3. A compact leaf (an input span or an owned buffer behind the same API), and fewer vectors per entity.
4. The SIMD answerers (D4), once mail-parser's kernel bench says they beat memchr.

## Reading the numbers

The corpus is small malformed and RFC messages, where per-entity overhead dominates; the Enron and synthetic corpora of benchmarked-against-stalwart, fetched rather than vendored, are not wired yet. The two libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back.
