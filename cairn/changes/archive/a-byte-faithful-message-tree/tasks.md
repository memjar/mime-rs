---
cairn: tasks
change: a-byte-faithful-message-tree
---

- [ ] Node types with `Cow` leaves and public fields, after `VcardCst` and `IcalCst`
- [ ] `MimeParse` coroutine and its yields, with io-imap's reference coroutines open; `MimeCst::parse` over the memchr answers
- [ ] `tree::scan::scalar` and `tree::scan::memchr` answerers for `WantsFieldEnds`, `WantsDashLines` and `WantsFind`
- [ ] Answer checks and the dropped-position anomaly
- [ ] Answerer equivalence: every fixture parsed through both answerers gives equal trees; fuzz/fuzz_targets/scan.rs and answer.rs
- [ ] Request count test: one field-ends request per header block, one dash-lines request per body
- [ ] Header block splitter: fields, continuations, opaque lines, the terminating empty line, line endings as written
- [ ] Minimal Content-Type reader (type, subtype, `boundary`) and Content-Transfer-Encoding mechanism reader
- [ ] Multipart splitter over `memmem::Finder`: delimiters, padding, close delimiter, preamble, epilogue
- [ ] Fallback delimiters, outer delimiters closing inner multiparts, cut header blocks
- [ ] Nested `message/rfc822` and `message/global` entities in place; encoded ones left as leaves
- [ ] Iterative walk with an explicit frame stack; `MimeParseOptions` bounds
- [ ] Anomaly set on each entity and `MimeCst::anomalies`
- [ ] `to_bytes`, `chunks` and `Display`
- [ ] Ownership test: every leaf of a parsed fixture borrows the input
- [ ] tests/corpus.rs: byte identity, fixpoint and anomaly counts per vendored directory, in LF and CRLF, fixture counts asserted
- [ ] tests/structure.rs: part tree, kinds and offsets against the frozen answers, divergences read from divergence.tsv
- [ ] Ignored sweep over the fetched corpora, including a live structural comparison with mail-parser 1.0
- [ ] Stack test: 10,000 nested multiparts on a 64 KiB thread
- [ ] Linearity tests: doubling a pathological input at most doubles the work
- [ ] fuzz/ with the `parse` target, its dictionary and README
- [ ] Structure gate of benchmarked-against-stalwart met
- [ ] Fold the delta into cairn/spec/parsing.md, write the log entry, archive this change
