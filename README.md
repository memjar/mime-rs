# 📨 mime-rs [![Documentation](https://img.shields.io/docsrs/mime-rs?style=flat&logo=docs.rs&logoColor=white)](https://docs.rs/mime-rs/latest/mime) [![Coverage](https://img.shields.io/codecov/c/github/pimalaya/mime/master?style=flat&logo=codecov&logoColor=white)](https://codecov.io/gh/pimalaya/mime) [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya) [![Sponsor](https://img.shields.io/badge/sponsor-pink?style=flat&logo=github-sponsors&logoColor=white)](https://pimalaya.org/sponsor/)

MIME message parser, validator, editor and builder library for Rust

## Table of contents

- [Features](#features)
- [RFC coverage](#rfc-coverage)
- [Usage](#usage)
- [Examples](#examples)
- [Benchmarks](#benchmarks)
- [AI policy](https://github.com/pimalaya/.github/blob/master/AI_POLICY.md)
- [License](#license)
- [Social](#social)
- [Contributing](./CONTRIBUTING.md)
- [Sponsoring](#sponsoring)

## Features

- **Byte-faithful editing**: an edit rewrites what it names, every other byte comes back unchanged.
- **Forgiving parser**: any bytes parse, round-trip unchanged, with every recovery reported.
- **Checked against mail-parser**: over the test corpus, half a million real messages (Enron, Apache lists) and fuzzed bytes, it reads what mail-parser 1.0 reads, but where a documented rule says why not.
- **Lazy**: parse the header block alone, split the body or decode a part only when asked.
- **I/O-free**: parsing is a coroutine handing its byte searches to the caller (memchr by default).
- **Strict building**: messages checked against the standard and written deterministically.
- **Validation**: every rule a message breaks, and a proof type only a valid message earns.
- **Fast**: ahead of mail-parser and mail-builder on every benchmark, base64 and quoted-printable on SIMD codecs.
- **Portable**: no_std, free of `unsafe`.

> [!TIP]
> mime-rs uses [cargo features](https://doc.rust-lang.org/cargo/reference/features.html) to gate optional support. The default feature set is declared in [Cargo.toml](./Cargo.toml) or on [docs.rs](https://docs.rs/crate/mime-rs/latest/features).

## RFC coverage

| Spec   | What is covered                                                                                   |
|--------|---------------------------------------------------------------------------------------------------|
| [5322] | Internet Message Format: fields, addresses, dates, identifiers, folding, with the obsolete syntax |
| [2045] | MIME: header fields, transfer encodings (quoted-printable, base64, identity)                      |
| [2046] | MIME media types: multiparts, delimiters, nested messages, digests                                |
| [2047] | Encoded words in unstructured text, phrases and comments                                          |
| [2183] | Content-Disposition                                                                               |
| [2231] | Parameter value continuations, charsets and languages                                             |
| [2369] | List-* fields as URLs                                                                             |
| [2387] | multipart/related                                                                                 |
| [1847] | multipart/signed and multipart/encrypted, sealed against edits                                    |
| [3676] | format=flowed text, read and written                                                              |
| [5256] | Base subject for threading                                                                        |
| [6532] | UTF-8 in fields, read and written                                                                 |
| [8621] | Header forms and the text body, HTML body and attachment lists of JMAP                            |

[5322]: https://www.rfc-editor.org/rfc/rfc5322
[2045]: https://www.rfc-editor.org/rfc/rfc2045
[2046]: https://www.rfc-editor.org/rfc/rfc2046
[2047]: https://www.rfc-editor.org/rfc/rfc2047
[2183]: https://www.rfc-editor.org/rfc/rfc2183
[2231]: https://www.rfc-editor.org/rfc/rfc2231
[2369]: https://www.rfc-editor.org/rfc/rfc2369
[2387]: https://www.rfc-editor.org/rfc/rfc2387
[1847]: https://www.rfc-editor.org/rfc/rfc1847
[3676]: https://www.rfc-editor.org/rfc/rfc3676
[5256]: https://www.rfc-editor.org/rfc/rfc5256
[6532]: https://www.rfc-editor.org/rfc/rfc6532
[8621]: https://www.rfc-editor.org/rfc/rfc8621

## Usage

See documentation at [docs.rs](https://docs.rs/mime-rs/latest/mime).

## Examples

See complete examples at [./examples](./examples).

## Benchmarks

[criterion](https://crates.io/crates/criterion) medians of `cargo bench --bench compare` on 2026-11-02, on an AMD Ryzen AI Max+ 395 (Zen 5), rustc 1.99. Each italic delta compares its row with mime-rs. The method, the workloads in full and what moved the numbers are in [benches/README.md](./benches/README.md).

### Parsing

The 153 messages of tests/corpus (RFC, legacy, third-party and malformed ones, about 108 KiB), in MiB/s of message bytes:

- `structure`: parse, then the content type of every part.
- `headers`: parse, then the decoded Subject, the first From address, the Date as a timestamp and the Message-ID.
- `header block`: `headers` with the parse stopped at the header block (`structure_depth: Some(0)`), against mail-parser's `parse_headers`.
- `full`: parse, then every leaf decoded, text transcoded to UTF-8.

| MiB/s | structure | headers | header block | full |
| --- | ---: | ---: | ---: | ---: |
| **mime-rs** | **1,257** | **1,071** | **2,478** | **863** |
| [mail-parser](https://crates.io/crates/mail-parser) 1.0 | *(-36%)* **804** | *(-27%)* **786** | *(-15%)* **2,099** | *(-17%)* **717** |

### Building and editing

In GiB/s of message bytes:

- `build`: a message of a text body, an HTML body, two attachments and an inline image (about 110 KiB) written to bytes.
- `edit`: the Subject of a message carrying a 2 MiB attachment changed and the message written out; with mail-parser and mail-builder, the only way is a parse and a rebuild.
- `edit, header block`: `edit` with the parse stopped at the header block.

| GiB/s | build | edit | edit, header block |
| --- | ---: | ---: | ---: |
| **mime-rs** | **6.34** | **35.99** | **65.31** |
| [mail-builder](https://crates.io/crates/mail-builder) 1.0 | *(-23%)* **4.85** | | |
| [mail-parser](https://crates.io/crates/mail-parser) 1.0 and [mail-builder](https://crates.io/crates/mail-builder) 1.0 | | *(-82%)* **6.62** | *(-90%)* **6.62** |

The libraries do different work: mail-parser builds a read-only index, mime-rs a tree every byte of which can be edited and written back.

## License

This project is licensed under either of:

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

## Social

- Chat on [Matrix](https://matrix.to/#/#pimalaya:matrix.org)
- News on [Mastodon](https://fosstodon.org/@pimalaya) or [RSS](https://fosstodon.org/@pimalaya.rss)
- Mail at [pimalaya.org@posteo.net](mailto:pimalaya.org@posteo.net)

## Sponsoring

[![nlnet](https://nlnet.nl/logo/banner-160x60.png)](https://nlnet.nl/)

Special thanks to the [NLnet foundation](https://nlnet.nl/) and the [European Commission](https://www.ngi.eu/) that have been financially supporting the project for years:

- 2022 → 2023: [NGI Assure](https://nlnet.nl/project/Himalaya/)
- 2023 → 2024: [NGI Zero Entrust](https://nlnet.nl/project/Pimalaya/)
- 2024 → 2026: [NGI Zero Core](https://nlnet.nl/project/Pimalaya-PIM/)
- 2026 → 2027: [NGI Zero Commons Fund](https://nlnet.nl/project/Pimalaya-pimdir/)

This program is part of Pimalaya, free software funded entirely by grants and donations. If you find it useful, consider [sponsoring](https://pimalaya.org/sponsor/) its development:

[![GitHub](https://img.shields.io/badge/-GitHub%20Sponsors-fafbfc?logo=GitHub%20Sponsors)](https://github.com/sponsors/soywod)
[![Ko-fi](https://img.shields.io/badge/-Ko--fi-ff5e5a?logo=Ko-fi&logoColor=ffffff)](https://ko-fi.com/pimalaya)
[![Buy Me a Coffee](https://img.shields.io/badge/-Buy%20Me%20a%20Coffee-ffdd00?logo=Buy%20Me%20A%20Coffee&logoColor=000000)](https://www.buymeacoffee.com/pimalaya)
[![Liberapay](https://img.shields.io/badge/-Liberapay-f6c915?logo=Liberapay&logoColor=222222)](https://liberapay.com/pimalaya)
[![thanks.dev](https://img.shields.io/badge/-thanks.dev-000000?logo=data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjQuMDk3IiBoZWlnaHQ9IjE3LjU5NyIgY2xhc3M9InctMzYgbWwtMiBsZzpteC0wIHByaW50Om14LTAgcHJpbnQ6aW52ZXJ0IiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Ik05Ljc4MyAxNy41OTdINy4zOThjLTEuMTY4IDAtMi4wOTItLjI5Ny0yLjc3My0uODktLjY4LS41OTMtMS4wMi0xLjQ2Mi0xLjAyLTIuNjA2di0xLjM0NmMwLTEuMDE4LS4yMjctMS43NS0uNjc4LTIuMTk1LS40NTItLjQ0Ni0xLjIzMi0uNjY5LTIuMzQtLjY2OUgwVjcuNzA1aC41ODdjMS4xMDggMCAxLjg4OC0uMjIyIDIuMzQtLjY2OC40NTEtLjQ0Ni42NzctMS4xNzcuNjc3LTIuMTk1VjMuNDk2YzAtMS4xNDQuMzQtMi4wMTMgMS4wMjEtMi42MDZDNS4zMDUuMjk3IDYuMjMgMCA3LjM5OCAwaDIuMzg1djEuOTg3aC0uOTg1Yy0uMzYxIDAtLjY4OC4wMjctLjk4LjA4MmExLjcxOSAxLjcxOSAwIDAgMC0uNzM2LjMwN2MtLjIwNS4xNTYtLjM1OC4zODQtLjQ2LjY4Mi0uMTAzLjI5OC0uMTU0LjY4Mi0uMTU0IDEuMTUxVjUuMjNjMCAuODY3LS4yNDkgMS41ODYtLjc0NSAyLjE1NS0uNDk3LjU2OS0xLjE1OCAxLjAwNC0xLjk4MyAxLjMwNXYuMjE3Yy44MjUuMyAxLjQ4Ni43MzYgMS45ODMgMS4zMDUuNDk2LjU3Ljc0NSAxLjI4Ny43NDUgMi4xNTR2MS4wMjFjMCAuNDcuMDUxLjg1NC4xNTMgMS4xNTIuMTAzLjI5OC4yNTYuNTI1LjQ2MS42ODIuMTkzLjE1Ny40MzcuMjYuNzMyLjMxMi4yOTUuMDUuNjIzLjA3Ni45ODQuMDc2aC45ODVabTE0LjMxNC03LjcwNmgtLjU4OGMtMS4xMDggMC0xLjg4OC4yMjMtMi4zNC42NjktLjQ1LjQ0NS0uNjc3IDEuMTc3LS42NzcgMi4xOTVWMTQuMWMwIDEuMTQ0LS4zNCAyLjAxMy0xLjAyIDIuNjA2LS42OC41OTMtMS42MDUuODktMi43NzQuODloLTIuMzg0di0xLjk4OGguOTg0Yy4zNjIgMCAuNjg4LS4wMjcuOTgtLjA4LjI5Mi0uMDU1LjUzOC0uMTU3LjczNy0uMzA4LjIwNC0uMTU3LjM1OC0uMzg0LjQ2LS42ODIuMTAzLS4yOTguMTU0LS42ODIuMTU0LTEuMTUydi0xLjAyYzAtLjg2OC4yNDgtMS41ODYuNzQ1LTIuMTU1LjQ5Ny0uNTcgMS4xNTgtMS4wMDQgMS45ODMtMS4zMDV2LS4yMTdjLS44MjUtLjMwMS0xLjQ4Ni0uNzM2LTEuOTgzLTEuMzA1LS40OTctLjU3LS43NDUtMS4yODgtLjc0NS0yLjE1NXYtMS4wMmMwLS40Ny0uMDUxLS44NTQtLjE1NC0xLjE1Mi0uMTAyLS4yOTgtLjI1Ni0uNTI2LS40Ni0uNjgyYTEuNzE5IDEuNzE5IDAgMCAwLS43MzctLjMwNyA1LjM5NSA1LjM5NSAwIDAgMC0uOTgtLjA4MmgtLjk4NFYwaDIuMzg0YzEuMTY5IDAgMi4wOTMuMjk3IDIuNzc0Ljg5LjY4LjU5MyAxLjAyIDEuNDYyIDEuMDIgMi42MDZ2MS4zNDZjMCAxLjAxOC4yMjYgMS43NS42NzggMi4xOTUuNDUxLjQ0NiAxLjIzMS42NjggMi4zNC42NjhoLjU4N3oiIGZpbGw9IiNmZmYiLz48L3N2Zz4=)](https://thanks.dev/u/gh/soywod)
[![PayPal](https://img.shields.io/badge/-PayPal-0079c1?logo=PayPal&logoColor=ffffff)](https://www.paypal.com/paypalme/soywod)
