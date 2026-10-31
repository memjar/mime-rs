---
cairn: tasks
change: ready-for-a-first-release
---

- [ ] 24-hour runs of the parse, scan, answer, fields, decode, write and edit fuzz targets, scan over every answerer including the SIMD crate; crashes minimised into fixtures (done so far: 3 to 7 minutes per target on 2026-10-31, every finding fixed and kept as a case)
- [ ] Differential fuzz target against mail-parser 1.0; disagreements fixed or recorded
- [x] Adversarial review by a fresh reader; findings filed as tests (11 confirmed: delimiters smuggled through content edits, inserted parts and a quoted-printable soft break; CRLF injection through unchecked tokens; sealing bypassed through the structural fields; three writer and validator gaps; two quadratic paths; all fixed, each with a test in tests/edit.rs, tests/build.rs or tests/validate.rs)
- [x] Tarpaulin coverage pass (88% of lines with the ptrace engine; the llvm engine cannot read rustc 1.99 profiles); text_prefix, unfold and replace_text gained tests
- [ ] Ignored sweeps over every fetched corpus, counts recorded (waits for the fetch script)
- [x] lib.rs architecture header and full rustdoc, with the missing-docs and intra-doc checks clean
- [x] Examples listed in the proposal, each in Cargo.toml with its path and required features
- [x] README.md, benches/README.md, CONTRIBUTING.md, fuzz/README.md, AGENTS.md, SECURITY.md, the tests workflow
- [x] CHANGELOG.md `[Unreleased]` written
- [ ] Guidelines conformance table, every row PASS or N/A (open rows: inline-002 summaries over one line, markdown-006 paragraphs over three lines, crate-006 cfg inside charset function bodies, naming-013 parse state names)
- [ ] Write the log entry, archive this change, hand release to the user
