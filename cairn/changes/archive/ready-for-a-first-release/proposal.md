---
cairn: change
id: ready-for-a-first-release
status: landed
created: 2026-10-31
---

# Ready for a first release

Phase 7 of [a-mime-library](../a-mime-library/proposal.md).

## Why

Phases 1 to 6 each land with their own tests and fuzz target; this phase is the pass over the whole that a first release needs before seven consumers start moving onto it: a long fuzz campaign across every target, the coverage sweep, the documentation, and the guidelines check. mail-parser 1.0 shipped after an adversarial review whose findings became its regression catalogue; the same review here is cheaper, since that catalogue is already in the corpus.

## What

### Hardening

- Every fuzz target (parse, fields, decode, build, edit) run for at least 24 hours each, seeded from the vendored corpus by copy (never pointed at tests/corpus, which libFuzzer would write into), each crash minimised into a regression fixture.
- A differential target: arbitrary bytes through this crate and mail-parser 1.0, comparing the part tree, the body lists and the decoded headers, every disagreement either fixed or added to the divergence file.
- An adversarial review of the tree, the field readers and the edit layer by a reader who did not write them, its findings filed as fixtures the way mail-parser's review findings were.
- `cargo tarpaulin --all-features --engine llvm` towards full coverage, dead code deleted rather than tested, nothing restructured for the number.
- The ignored sweeps run over every fetched corpus (Enron, Apache lists, Stalwart resources, mail-auth, Hunny), counts recorded in the log entry.

### Documentation

- src/lib.rs header as the architecture document, every public item documented, `RUSTFLAGS="-D missing_docs"` and broken intra-doc links denied, `Refs:` lines with checked RFC anchors.
- examples/: parse_and_read, forgiving_parse, body_lists, content_encodings, strict_builder, raw_builder, reply_and_forward, edit_a_draft, submission (Bcc removed, required fields filled, downgrade on demand), validate_errors, headers_only (a header block read alone, as io-pimdir's metadata path does).
- README.md features and RFC coverage table final; benches/README.md tables final; CONTRIBUTING.md corpus workflow; fuzz/README.md.
- CHANGELOG.md `[Unreleased]` listing the first release's items, one per bullet.
- The guidelines conformance table run again, every row PASS or N/A.

Version bump, tag and publish stay the user's.
