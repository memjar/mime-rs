---
cairn: change
id: quoted-printable-over-the-codec
status: landed
created: 2026-11-02
---

# Quoted-printable over the codec

Follows [the-simd-codecs](../archive/the-simd-codecs/proposal.md), which moved base64 to base64-codec and brought the `SimdCpu` token in.

## Why

mime-rs encodes and decodes quoted-printable bodies with a scalar in-house codec (src/codec/quoted_printable.rs). quoted-printable-codec, Pimalaya's no_std, I/O-free SIMD codec, beats encodify 1.0.2 on every row it measures, and mime already carries the token it takes.

## What

Decided with the user on 2026-11-02:

- quoted-printable-codec joins the core, not behind a feature: QP stays always available, as today. Bodies decode under the installed token and encode under `MimeBuildOptions::cpu` or the installed one, the validator decoding under the installed one.
- The RFC 2047 Q encoding moves too: the user chose one codec to maintain on 2026-11-02, and quoted-printable-codec's change the-q-encoding (in flight) adds `QuotedPrintableMode::Q`, copying mime's rules and clean flag exactly, with `QuotedPrintableCodec::encoded_len` for `encoded_q_len`. src/codec/quoted_printable.rs then holds no codec of its own.
- The token: the codecs' defaults read `SimdCpu::current()`, and mime already reads the installed token since [the-installed-token](../archive/the-installed-token/proposal.md): bodies decode under a default codec, the builder under `MimeBuildOptions::cpu` or the default.

It waited on two changes to quoted-printable-codec, landed there as the-mail-safe-lines on 2026-11-02:

- An option escaping a `.`, `F` or `-` that starts an encoded line (RFC 2049 3(8)). mime's spec requires it, and its builder relies on it: QP is always `valid` for a leaf, because no line of QP output can read as a delimiter of a declared or enclosing boundary.
- A bare CR kept and reported on decoding, instead of dropped. mime's spec says QP decoding loses nothing; a CR-only QP body would otherwise lose every line break.

The differences left between the two decoders were settled for the codec's rules on 2026-11-02, all within mime's "liberal on reading" requirement: the blanks past 76 before a line break stay as data (mime dropped them all); a `=`, with or without blanks, ending the input is a clean soft break (mime reported it); a lowercase hex digit, a raw 8-bit byte and a bare CR, all kept as before, are reported (mime read them clean). The frozen corpus answers, decoded content and decode problems of 51 quoted-printable fixtures included, did not move.

## Risks

- crates.io refuses git dependencies: mime cannot publish until quoted-printable-codec is on crates.io too.
