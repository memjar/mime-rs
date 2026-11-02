# Contributing guide

Thank you for investing your time in contributing to mime-rs.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/lib.rs: it is the architecture document of this crate;
4. the [cairn/](./cairn) folder for the development history and living plans (the Cairn convention: spec/, changes/, log/).

Everything below documents only what differs from the Pimalaya standards.

## Deviation: wire-spelled markers

The field markers are spelled exactly as their wire name, hyphens as underscores (`SUBJECT`, `MESSAGE_ID`, `CONTENT_TYPE`), against naming-007, which asks every public item to carry the `Mime` domain prefix. Every other public item does carry it.

They are type-level keys naming a field, written only inside a turbofish (`entity.header::<SUBJECT>()`), never constructed and never handled as values, as vcard-rs's property markers are. Prefixing them would push the field name, the one thing they encode, to the end of a longer identifier.

## Deviation: quoted-printable without a feature

Quoted-printable is decoded and encoded in-house and always available, where vcard-rs and ical-rs put it behind a `quoted-printable` feature over the quoted_printable crate. That crate drops every byte outside printable ASCII and rewrites every line ending to CRLF, which loses data in mail, where quoted-printable bodies carry raw 8-bit bytes and LF line endings. Base64 and charsets stay behind their features.

## Cairn

This repository follows [Cairn](https://github.com/pimalaya/cairn): a living spec, reviewable change proposals and a dated log, kept next to the code.

Non-trivial work starts with a change folder under cairn/changes, and nothing behavioural is done until its delta is folded into cairn/spec and an entry is appended to cairn/log. The activation stanza is [AGENTS.md](./AGENTS.md).

## Build

mime-rs is not an I/O library: its one coroutine, the parse, yields byte searches rather than I/O, and it has no client or TLS layer. It is a no_std library (with alloc) whose core depends on simd-cpu, for the CPU token the application hands to the SIMD codecs, and quoted-printable-codec, every other dependency sitting behind a feature: `parser` takes memchr, `base64` base64-codec, `encoding` encoding_rs.

Everything under the tree module is gated on `parser`, while the decoded model, the writer, the builder and the model validator are always available.

Check every combination that matters, so gated code never leaks into the always-on core:

```sh
cargo build                                          # default features
cargo build --no-default-features                    # simd-cpu alone, no std leak
cargo test --no-default-features --features parser   # the tree without codecs
cargo test --all-features
```

## The fixture corpus

Integration tests sweep a corpus of real and adversarial messages under tests/corpus, one directory per source project, each with its own ATTRIBUTION.md so provenance and licensing stay per project. Most fixtures carry mail-parser 1.0's frozen readings (`NAME.json` for the LF form, `NAME.crlf.json` for the CRLF form), which tests/structure.rs, tests/decoded.rs and tests/headers.rs compare against.

1. drop the message into the matching tests/corpus/<project>/ directory (create a new one, with an ATTRIBUTION.md, for a new source);
2. bump the fixture count for that project in tests/corpus.rs and wherever else the directory is listed, and the classification counts in tests/validate.rs;
3. run cargo test. Every fixture must round-trip byte for byte in both line endings; if it does not, you have found a bug, so fix the code rather than the fixture.

When this crate reads a fixture differently from mail-parser on purpose, add a row to tests/corpus/divergence.tsv (fixture, path, our reading, reason, the reason citing the RFC); a field vector read differently goes to tests/vectors/divergence.tsv. Never edit a frozen answer.

## The differential oracle

tests/common/differential.rs reads one input through this crate and through mail-parser 1.0 and compares the part trees, the decoded content and decode problems, the decoded header values and the body lists. A difference made on purpose is let through by a named rule (`Rule`), whose doc comment states the reason; header values are compared only where the validator finds their syntax sound and their RFC grammar holds, since on malformed values both parsers recover by policy. The oracle runs over the vendored corpus (tests/differential.rs), over arbitrary bytes (the fuzz crate's `differential` target), and over the fetched corpora:

```sh
scripts/fetch-corpora.sh   # Enron, Apache lists, Stalwart, mail-auth, Hunny; about 3 GB
cargo test --release --all-features --test fetched -- --ignored
```

A difference the oracle reports is a bug to fix, with a test failing on the old code, or a deliberate reading to record as a rule with its reason, and as a divergence row when a fixture shows it.

New inputs are also worth handing to the fuzzers, described in [fuzz/README.md](./fuzz/README.md); a finding becomes a case in the test file of its target before it is fixed.

## Examples and benchmarks

The runnable programs live in [./examples](./examples); run one with cargo run --example followed by its name. The comparative benchmark runs with cargo bench --bench compare, and its methodology and current numbers are recorded in [benches/README.md](./benches/README.md).
