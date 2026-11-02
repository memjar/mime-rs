---
cairn: tasks
change: quoted-printable-over-the-codec
---

- [x] quoted-printable-codec: line-start guard option and bare CR kept (its change the-mail-safe-lines)
- [x] quoted-printable-codec pinned by git rev beside base64-codec, sharing its simd-cpu rev
- [x] src/codec/quoted_printable.rs bodies and Q words over quoted-printable-codec, nothing in-house left; the entity, builder and validator call sites take the resolved token
- [x] Frozen corpus answers unchanged; tests, fuzz targets, every feature set, clippy, docs
- [x] Spec folded (content-encodings: the codec named, the leniency differences settled), log entry, CHANGELOG.md
