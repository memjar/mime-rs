---
cairn: log
change: quoted-printable-over-the-codec
landed: 2026-11-02
---

# Quoted-printable over the codec

Landed quoted-printable through quoted-printable-codec, in the core: src/codec/quoted_printable.rs keeps its five functions and no codec of its own. Bodies decode under the installed token (`decode` now takes the token, as base64's does), the builder encodes under `MimeBuildOptions::cpu` or the installed one, with the codec's line-start guard on so the builder can still treat quoted-printable as always valid inside a multipart. Q words run the codec's Q mode, scalar on every backend.

The codec first gained the two rules mime needed, as its change the-mail-safe-lines: the guard (a `.`, `F` or `-` starting a line escaped, RFC 2049 3(8)) and a bare CR kept and reported on decoding. The remaining differences between the decoders were settled for the codec's rules, with the user on 2026-11-02: blanks past 76 before a line break stay as data; an `=` ending the input, with or without blanks, is a clean soft break; a lowercase hex digit, a raw 8-bit byte and a bare CR, all still kept, are reported. Encoding differs in one place: a line that would reach exactly 76 characters before a hard break or the end of the input now takes a soft break, as the codec keeps every line within 75 characters before its break; both forms are valid.

Pins: base64-codec d39f6b9, quoted-printable-codec d63612b, simd-cpu f2eb72c (dependency, dev-dependency and fuzz), the three codecs sharing one simd-cpu.

Verification: the frozen corpus answers did not move, the decoded content and decode problems of 51 quoted-printable fixtures included; the suite under every feature set, clippy, docs, the fuzz crate (its decode target now also encodes text and checks no line starts with `-`); `--no-default-features` builds for thumbv6m-none-eabi and riscv32imc-unknown-none-elf, depending on simd-cpu and quoted-printable-codec alone. Bench on a lightly loaded Zen 5: `build` 18.1 us against mail-builder's 23.0, `full` 189 us against mail-parser's 146.

Capabilities moved: content-encodings (MODIFIED: features, quoted-printable reading, the encoders).
