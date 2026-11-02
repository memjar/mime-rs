---
cairn: tasks
change: the-installed-token
---

- [x] Pins: base64-codec a478087, simd-cpu f77308a, in Cargo.toml and fuzz/Cargo.toml
- [x] `MimeEntity::cpu` and `MimeParseOptions::cpu` removed, with their copies in the parser, edits, expansion and `nested`; readers on `SimdCpu::current()`
- [x] `MimeBuildOptions::cpu: Option<SimdCpu>`, resolved when a body is encoded; B words on the default codec
- [x] Tests (the token propagation tests go; the backend a leaf runs is not observable through mime, so the codecs' own install tests cover it), bench, example, fuzz targets; builds with and without default features; clippy, docs
- [x] CHANGELOG.md, spec folded, log entry, change archived
