---
cairn: log
change: ready-for-a-first-release
landed: 2026-11-04
---

# Ready for a first release

Landed phase 7 of [a-mime-library](../changes/a-mime-library/proposal.md): the pass over the whole crate before 0.1.0, handed to the user for tag and publish.

Hardening: the differential target against mail-parser 1.0 (39 rules, 9 fixes, see [the-differential-target](2026-11-02-the-differential-target.md)); an adversarial review by a fresh reader, its 11 confirmed findings fixed with tests in tests/edit.rs, tests/build.rs and tests/validate.rs; a tarpaulin pass at 88% of lines (ptrace engine, as the llvm engine cannot read rustc 1.99 profiles); the ignored sweeps over every fetched corpus, every message round-tripping (Enron 517,401, Apache lists 35,760, Stalwart main 242 and v1 242, Stalwart SMTP 9, mail-auth 81, Hunny 54).

Documentation: the src/lib.rs architecture header and full rustdoc under `-D missing_docs`, the eleven examples, README.md, benches/README.md, CONTRIBUTING.md, fuzz/README.md, AGENTS.md, SECURITY.md, CHANGELOG.md 0.1.0, and the guidelines conformance table with every row PASS or N/A.

Release: base64-codec, quoted-printable-codec and simd-cpu moved from git revs to their crates.io 0.1 releases, in the fuzz crate too, whose decode target had stopped building against two simd-cpu tokens. mail-parser is pinned to `=1.0.0` in the dev-dependencies and the fuzz crate, so the live oracle reads as the frozen fixtures do. cairn/ is excluded from the package. `cargo publish --dry-run` passes.

Not done: the 24-hour fuzz runs. Each target has had about an hour; the user chose to release without the long runs, which moved to [a-long-fuzz-campaign](../changes/a-long-fuzz-campaign/proposal.md).

No capability moved: this pass changed no requirement in cairn/spec/.
