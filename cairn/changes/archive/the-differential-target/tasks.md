---
cairn: tasks
change: the-differential-target
---

- [x] scripts/fetch-corpora.sh and scripts/corpora.sha256 ported from mail-parser 1.0.0, with the Hunny samples from the pinned crate; every corpus fetched and verified
- [x] The oracle, tests/common/differential.rs: the two readings as part trees, then content, problems, header values and body lists, each deliberate difference a named rule with its reason
- [x] tests/differential.rs over the vendored corpus in every line-ending form, with a check that the oracle sees a moved part, a changed text and a changed list
- [x] tests/fetched.rs, ignored: round trip, both answerers and the oracle over every fetched corpus
- [x] The fuzz crate's `differential` target, seeded from the fixtures and the `parse` corpus
- [x] Every disagreement found on the fixtures, the fuzz corpora, the fetched corpora and the campaign fixed, with a test failing on the old code, or recorded as a rule
- [x] A one-hour campaign of every target; the edit target's finding fixed with a test failing on the old code
- [x] Spec folded (parsing, header-fields, conformance), divergence row corrected, CONTRIBUTING.md and fuzz/README.md
- [x] Suite, clippy, bare core build, docs, fmt
- [x] Log entry, change archived, ready-for-a-first-release tasks updated
