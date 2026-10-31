---
cairn: tasks
change: ready-for-a-first-release
---

- [ ] 24-hour runs of the parse, scan, answer, fields, decode, build and edit fuzz targets, scan over every answerer including the SIMD crate; crashes minimised into fixtures
- [ ] Differential fuzz target against mail-parser 1.0; disagreements fixed or recorded
- [ ] Adversarial review by a fresh reader; findings filed as fixtures
- [ ] Tarpaulin coverage pass
- [ ] Ignored sweeps over every fetched corpus, counts recorded
- [ ] lib.rs architecture header and full rustdoc, with the missing-docs and intra-doc checks clean
- [ ] Examples listed in the proposal, each in Cargo.toml with its required features
- [ ] README.md, benches/README.md, CONTRIBUTING.md, fuzz/README.md final
- [ ] CHANGELOG.md `[Unreleased]` written
- [ ] Guidelines conformance table, every row PASS or N/A
- [ ] Write the log entry, archive this change, hand release to the user
