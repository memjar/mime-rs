---
cairn: tasks
change: a-repo-for-mime-messages
---

- [ ] Create pimalaya/mime on GitHub (user)
- [ ] Copy and rename the vcard scaffold: Cargo.toml, nix files, deny.toml, licences, workflows
- [ ] Lock fenix late enough for rustc 1.98 and check the devshell's `rustc --version`
- [ ] Write src/lib.rs: `no_std` skeleton, architecture header, module declarations
- [ ] Write README.md, CONTRIBUTING.md, AGENTS.md, CLAUDE.md, SECURITY.md, CHANGELOG.md
- [ ] Vendor the six Stalwart and Dovecot corpus directories with their expected answers and ATTRIBUTION.md files
- [ ] Write the hand-written rfc corpus from the RFC examples
- [ ] Vendor the field vectors under tests/vectors/stalwart with their ATTRIBUTION.md
- [ ] Create an empty tests/corpus/divergence.tsv with its header row
- [ ] Port scripts/fetch-corpora.sh and its manifest, adding the pinned Hunny tarball
- [ ] Land the competitor-only benchmark harness and record the baseline in benches/README.md
- [ ] Run mail-parser's benches/kernels.rs and the base64 codec comparison on this machine, and record both for D4
- [ ] `cargo build`, `cargo build --no-default-features`, `cargo build --all-features`, `cargo deny check` all pass
- [ ] Run the guidelines conformance table and fix every FAIL
- [ ] Fold the delta into cairn/spec/decoded-model.md, write the log entry, archive this change
