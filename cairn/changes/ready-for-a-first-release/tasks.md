---
cairn: tasks
change: ready-for-a-first-release
---

- [ ] 24-hour runs of the parse, scan, answer, fields, decode, write, edit and differential fuzz targets, scan over every answerer including the SIMD crate; crashes minimised into fixtures (done so far: 3 to 7 minutes per target on 2026-10-31, then an hour of every target on 2026-11-02 in [the-differential-target](../archive/the-differential-target/proposal.md), whose two edit findings are fixed with tests)
- [x] Differential fuzz target against mail-parser 1.0; disagreements fixed or recorded ([the-differential-target](../archive/the-differential-target/proposal.md): 39 rules, 9 fixes)
- [x] Adversarial review by a fresh reader; findings filed as tests (11 confirmed: delimiters smuggled through content edits, inserted parts and a quoted-printable soft break; CRLF injection through unchecked tokens; sealing bypassed through the structural fields; three writer and validator gaps; two quadratic paths; all fixed, each with a test in tests/edit.rs, tests/build.rs or tests/validate.rs)
- [x] Tarpaulin coverage pass (88% of lines with the ptrace engine; the llvm engine cannot read rustc 1.99 profiles); text_prefix, unfold and replace_text gained tests
- [x] Ignored sweeps over every fetched corpus, counts recorded (tests/fetched.rs on 2026-11-02: Enron 517,401 messages, Apache lists 35,760, Stalwart main 242 and v1 242, Stalwart SMTP 9, mail-auth 81, Hunny 54, every one round-tripping, alike through both answerers and read as mail-parser reads it but by rule)
- [x] lib.rs architecture header and full rustdoc, with the missing-docs and intra-doc checks clean
- [x] Examples listed in the proposal, each in Cargo.toml with its path and required features
- [x] README.md, benches/README.md, CONTRIBUTING.md, fuzz/README.md, AGENTS.md, SECURITY.md, the tests workflow
- [x] CHANGELOG.md `[Unreleased]` written
- [x] Guidelines conformance table, every row PASS or N/A (2026-11-03: every multi-line doc summary cut to one line, the charset gates moved onto items, the parse state named by its actions, the benchmark tables moved to benches/README.md, long markdown paragraphs split, Cargo dependencies ordered, package.nix and cairn/verify.sh added; the version reads 0.1.0 and CHANGELOG.md has its 0.1.0 section, then on 2026-11-04 the codecs and simd-cpu moved from git revs to their crates.io 0.1 releases, in the fuzz crate too, mail-parser pinned to `=1.0.0` in both so the live oracle matches the frozen fixtures, and cairn/ excluded from the package as in the codec crates)
- [ ] Write the log entry, archive this change, hand release to the user
