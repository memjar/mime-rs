---
cairn: change
id: benchmarked-against-stalwart
status: active
created: 2026-10-31
---

# Benchmarked against Stalwart

Runs alongside every phase of [a-mime-library](../a-mime-library/proposal.md): the harness lands in phase 0, and each phase ends by meeting its gate.

## Why

"Fast" needs a number and a rival. mail-parser 1.0 is the fastest Rust mail parser, published at 1,597 to 17,171 MiB/s on its `full` workload and up to 80,868 MiB/s on `headers` (Apple M4, its own README), three times mailparse and an order of magnitude ahead of GMime, VMime, libEtPan! and Dovecot. Matching it while keeping every byte is the claim worth making; a benchmark that does not do the same work as its rival does not make it.

## What

### Workloads

The three of mail-parser's own comparison, defined exactly as it defines them, so the numbers sit beside its published ones, plus three of ours:

| Workload | Work done by every library |
| --- | --- |
| `structure` | Parse, then read the content type of every part |
| `headers` | Parse, then read the decoded Subject, the first From address, the Date as a Unix timestamp and the Message-ID |
| `full` | Parse, then read every leaf decoded, text parts transcoded to UTF-8 |
| `build` | Build the same message (text, HTML, two attachments, one inline image, non-ASCII names and subject) to bytes |
| `edit` | Change the Subject of a parsed message and write it out; the rival is mail-parser's parse plus mail-builder's rebuild, the only way to do it with them |
| `round-trip` | Parse and write back unchanged; no rival, tracked against ourselves |

Nested messages are parsed in every workload, by the library itself or by the harness when a library hands them back as bytes.

Every group first checks that each library produced the same answer (the same content types, the same four header values, the same decoded bytes, a built message that reads back the same), so a fast wrong answer cannot win, as ical's recurrence benchmarks do.

### Corpora

The same as mail-parser's, through its selection code ported with attribution from its benches/support/corpus.rs, in LF and CRLF forms: the test suite (the rfc, legacy, thirdparty, malformed and sieve messages plus the fetched Stalwart SMTP ones, 123 in its published run), an Enron sample of 3,000 messages taken per user, the seeded synthetic corpora (large attachments, newsletters, long modern header blocks, forwarded messages, dash-heavy bodies, 1 MB of plain text), and its synthetic malformed set.

### Rivals

mail-parser 1.0 (the gate), mail-parser 0.11 (what every consumer runs today, so the migration's gain is visible), mailparse (as a sanity line), mail-builder 1.0 and lettre's message builder for `build`. Micro-benchmarks set the codec wrappers against the bare base64 and quoted_printable crates (the wrapper's own cost), and the base64 crate against base64-simd and encodify (the D4 choice), the charset path against encoding_rs called directly, and the address and date readers against mail-parser's `HeaderForm::parse`.

### Gates

Ratios of throughput to mail-parser 1.0 (or to mail-builder 1.0 for `build`), geometric mean over the corpora, on one machine, set now and revisited once with the phase 1 numbers in hand:

| Workload | Gate | Checked at the end of |
| --- | --- | --- |
| `structure` | at least 1.0x | phase 1 |
| `headers` | at least 0.8x | phase 2 |
| `full` | at least 0.7x, the large-attachment corpus reported apart | phase 3 |
| `build` | at least 1.0x mail-builder 1.0 | phase 4 |
| `edit` | at least 5x the parse-and-rebuild rival on the large-attachment corpus | phase 5 |

A phase missing its gate profiles before it lands (perf through nix), and the profile goes into the log entry. Allocations per message are counted with a counting allocator in a bench-only binary and reported beside throughput.

### Levers beyond mail-parser

Tried in this order, each kept only if its workload moves on the corpora:

1. Lazy structure (D13): a parse that stops at the header block. `headers` and `edit` stop depending on body size at all, which no amount of SIMD gives mail-parser, since it scans every body on every full parse. If D13 lands, the `headers` gate rises to at least 1.0x.
2. The SIMD answerers (D4), mail-parser's kernels answering `WantsFieldEnds` and `WantsDashLines`, if its kernels bench shows them ahead of memchr.
3. A one-pass header-block index as the `WantsFieldEnds` answer: 64 bytes per step classified into `\n` and SP or HTAB bitmasks, every field end of the block derived with bit arithmetic in one sweep, as simdjson's first stage does for JSON, instead of one kernel call per field end. The request is already the whole block, so this changes the answerer alone and nothing in mime-rs.
4. AVX-512 base64: encodify has no AVX-512 kernels, and the VBMI permute decodes 64 characters per step against AVX2's 32. Offered upstream to encodify rather than kept here.

Since the caller answers the searches, every workload is also run once per answerer (scalar, memchr, the SIMD crate), which shows what each is worth on each corpus and keeps one that buys nothing from being kept.

### Reporting

benches/README.md, as ical's and vcard's: dated criterion medians and MiB/s tables per workload and corpus, the machine and rustc, the "read these as a ballpark" paragraph, and a rerun after every dependency bump. A cross-language ballpark against GMime and Dovecot through nix, as ical runs against libical, is welcome and is not a gate.
