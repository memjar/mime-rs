---
cairn: change
id: the-simd-codecs
status: landed
created: 2026-11-02
---

# The SIMD codecs

## Why

mime-rs transfer-encodes through the base64 crate and an in-house quoted-printable codec, both scalar. The `build` gate of [benchmarked-against-stalwart](../../benchmarked-against-stalwart/proposal.md) stands at 0.61x mail-builder, the base64 crate's encoder taking about 45% of build time. Pimalaya now has two no_std, I/O-free SIMD codecs written for mail, base64-codec and quoted-printable-codec, which replace the encodify backend D4 of [a-mime-library](../../a-mime-library/proposal.md) planned and which never lost its std dependency. base64-codec's decoding spec pins the exact output and clean flag of src/codec/base64.rs as of 2026-10-31, so the switch changes no byte.

## What

Decided with the user on 2026-11-02:

- **CPU token.** Both codecs take a `simd_cpu::SimdCpu` the application detects once; a library never detects. `MimeParseOptions` and `MimeBuildOptions` gain a `cpu: SimdCpu` field, `SimdCpu::BASELINE` by default. The parser copies it into a new `cpu` field of every `MimeEntity`, so the leaf accessors stay argument-free; edits copy their entity's token, and a body expanded later, or a nested message parsed by `nested`, is parsed under its entity's token. Header field readers cannot reach an entity: RFC 2047 B words decode under `SimdCpu::BASELINE`, which costs nothing at their length.
- **Base64.** The `base64` feature switches from the base64 crate to base64-codec (with `alloc`), body and B word alike, decoding and encoding. `codec::base64::decode` and `codec::base64::encode` take the token first.
- **Dependencies.** The core depends on simd-cpu, no_std and dependency-free; the AGENTS.md rule "`--no-default-features` must stay dependency-free" becomes "depends on nothing but the Pimalaya codec crates". base64-codec is pinned by git rev (d2aed5a), and simd-cpu by the rev base64-codec itself pins (4cde2e9), so one `SimdCpu` type reaches every codec. `rust-version` rises to 1.89, the codecs' own.
- **Benches, examples, fuzz targets.** The `full` and `build` rows of benches/compare.rs run under `SimdCpu::detect()`, and so do the example and the fuzz targets that call the codecs.

Quoted-printable moves in [quoted-printable-over-the-codec](../../quoted-printable-over-the-codec/proposal.md), blocked on two changes to the codec itself.

## Risks

- crates.io refuses git and path dependencies: [ready-for-a-first-release](../../ready-for-a-first-release/proposal.md) cannot publish until simd-cpu and base64-codec are on crates.io.
- `MimeEntity` equality now compares the token: two parses of the same bytes under different tokens are unequal.
