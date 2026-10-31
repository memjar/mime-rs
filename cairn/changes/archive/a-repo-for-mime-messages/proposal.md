---
cairn: change
id: a-repo-for-mime-messages
status: landed
created: 2026-10-31
---

# A repository for MIME messages

Phase 0 of [a-mime-library](../../a-mime-library/proposal.md).

## Why

Every later phase needs three things to exist first: a crate that builds in the Pimalaya way, a corpus to sweep from the first parser commit, and the competitors' numbers measured on this machine before a line of ours exists, so the gates of [benchmarked-against-stalwart](../../benchmarked-against-stalwart/proposal.md) are set against something real.

## What

### The scaffold

Copied from vcard and renamed, then checked against pimalaya/.github/GUIDELINES.md with the table protocol (one row per rule, PASS, FAIL or N/A with evidence):

- Cargo.toml: package mime-rs, lib `mime`, edition 2024, `rust-version = "1.88"`, `license = "MIT OR Apache-2.0"`, author `Clément DOUIN (soywod) <pimalaya.org@posteo.net>`, categories `email`, `parser-implementations`, `encoding`, `no-std`, docs.rs `all-features` with `--cfg docsrs`.
- A Cargo workspace if D4 keeps the SIMD answerers as a second crate: mime-rs at the root and the answerers as a member under simd/, empty until phase 0's measurement says whether they get mail-parser's kernels.
- Features, as vcard's: `default = ["parser", "base64", "quoted-printable", "encoding"]`, `parser = ["dep:memchr"]`, `base64 = ["dep:base64"]`, `quoted-printable = ["dep:quoted_printable"]`, `encoding = ["dep:encoding_rs"]`, every dependency `default-features = false` with `alloc`. No other feature until a phase needs one.
- src/lib.rs: unconditional `#![no_std]`, `#![cfg_attr(docsrs, feature(doc_cfg))]`, `extern crate alloc;`, the `//!` architecture header (the umbrella proposal's architecture section, in rustdoc), and the module declarations, `tree` gated on `parser`.
- flake.nix, shell.nix, default.nix and package.nix (`[ mit asl20 ]`) from vcard, with fenix locked late enough for rustc 1.98 (D11) and the fuzz/shell.nix nightly recipe.
- deny.toml from vcard; LICENSE-MIT and LICENSE-APACHE with the holder string and 2026 as the year.
- README.md in the badge-header flavour (no logo.svg): features, an RFC coverage table, usage pointing at docs.rs, examples, the AI policy link before License, Contributing between Social and Sponsoring, the sponsoring block spliced byte-exact from vcard's README rather than retyped.
- CONTRIBUTING.md with its one deviation (wire-spelled markers) and the corpus workflow; AGENTS.md (vcard's, with `Mime` as the prefix); CLAUDE.md as `@AGENTS.md`; SECURITY.md; CHANGELOG.md with an empty `[Unreleased]`.
- .github/workflows/tests.yml and audit.yml from vcard: default, `--no-default-features` and `--all-features` builds, tests, clippy, rustdoc with broken intra-doc links denied, the MSRV build of the library alone.

### The corpus

Vendored under tests/corpus/<source>/, each directory with an ATTRIBUTION.md (source URL, commit, licence, why it is here, "nothing here is edited"), and mail-parser's expected answers beside every fixture that has them (all but stalwart-issues and stalwart-sieve):

| Directory | Messages | Origin | Licence |
| --- | --- | --- | --- |
| stalwart-rfc | 10 | mail-parser resources/eml/rfc, the RFC examples | Apache-2.0 OR MIT |
| stalwart-regressions | 75 | mail-parser resources/eml/regressions, with its README catalogue | Apache-2.0 OR MIT |
| stalwart-issues | 18 | mail-parser resources/eml/issues, GitHub issue reproductions | Apache-2.0 OR MIT |
| stalwart-sieve | 6 | mail-parser resources/sieve, small layouts (plain, alternative, HTML, nested, mixed with an attachment) | Apache-2.0 OR MIT |
| dovecot-thirdparty | 20 | mail-parser resources/eml/thirdparty, from Dovecot's suite | MIT |
| dovecot-malformed | 24 | mail-parser resources/eml/malformed, 023 being a mail-parser fuzz input | MIT |
| rfc | hand-written | RFC 5322 appendix A, RFC 2046 5.1.1, RFC 2047 8, RFC 2231, RFC 3676, RFC 6532, RFC 1847 and RFC 8621 4.1.4 examples | ours |

The expected `.json` and `.crlf.json` answers are mail-parser 1.0's reading of each fixture, frozen as the differential oracle: never edited, superseded only by a row in tests/corpus/divergence.tsv (fixture, path into the answer, our reading, reason).

Field-level vectors go to tests/vectors/stalwart/ with one ATTRIBUTION.md: address.json, content_type.json, date.json, id.json, list.json, unstructured.json, thread_name.json, trailing_fwd.json, received.json (kept for later), and decoders/charset_meta.json; the HTML vectors wait for the html change.

The fetched corpora are never committed. scripts/fetch-corpora.sh, ported from mail-parser's with its sha256 manifest and a `MIME_CORPORA` variable (default target/corpora), downloads the Enron maildir (CMU 2015-05-07 release), the Stalwart main and v1 test resources, the mail-auth messages, the Apache mailing list mboxes, and the Hunny Software samples from a pinned mail-parser tarball (D12). Tests over them are `#[ignore]` and run with `cargo test -- --ignored corpus`.

### The baseline

The benchmark harness lands here with competitors only: mail-parser 1.0, mail-parser 0.11 (what every consumer runs today, renamed through `package =`), mailparse, mail-builder 1.0 and lettre's message builder, over the corpora and workloads benchmarked-against-stalwart defines, plus the two measurements D4 waits on: mail-parser's own benches/kernels.rs (its SIMD kernels against memchr) and the base64 crate against base64-simd and encodify. The numbers go into benches/README.md, dated, with the machine and rustc.

## What this is not

No parser yet. The library compiles empty, its modules declared and documented.
