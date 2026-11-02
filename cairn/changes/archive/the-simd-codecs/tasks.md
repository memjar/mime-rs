---
cairn: tasks
change: the-simd-codecs
---

- [x] `cpu: SimdCpu` on `MimeParseOptions`, `MimeBuildOptions` and `MimeEntity`, copied by the parser, edits, expansion and `nested`
- [x] src/codec/base64.rs over base64-codec, the base64 crate dropped
- [x] Tests (token propagation checked against a mutation), examples, benches and fuzz targets build and pass, with and without default features; clippy clean
- [x] `build` and `full` gates measured against the base64 crate (2026-11-02, Zen 5, loaded machine, tightest rows): `build` 40.3 to 18.4 µs (2.2x), 1.3x faster than mail-builder 1.0 at its best 24.2 µs (was 0.61x); `full` 209.8 to 204.8 µs, within noise; to re-measure on an idle machine
- [x] AGENTS.md and CONTRIBUTING.md dependency rule, CHANGELOG.md
- [x] base64-codec public, pinned by git rev d2aed5a
- [x] Spec folded (content-encodings, building)
- [x] Log entry, change archived
