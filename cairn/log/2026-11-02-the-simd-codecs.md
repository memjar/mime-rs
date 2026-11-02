---
cairn: log
change: the-simd-codecs
landed: 2026-11-02
---

# The SIMD codecs

Landed base64 over base64-codec (pinned at d2aed5a) in place of the base64 crate, and the CPU token it takes: `cpu: SimdCpu` on `MimeParseOptions`, `MimeBuildOptions` and `MimeEntity`, `SimdCpu::BASELINE` by default. The parser copies the parse's token into every entity, edits give new entities their parent's, and `expand` and `nested` parse under the entity's own. `codec::base64::decode` and `codec::base64::encode` take the token first; RFC 2047 B words run under `BASELINE`. The core now depends on simd-cpu (pinned at 4cde2e9, the rev base64-codec pins), and `rust-version` is 1.89. src/codec/base64.rs shrank from 230 to 136 lines, its line-by-line fast path and filtering pass now inside the codec, whose decoding spec pins this module's former output.

tests/structure.rs and tests/edit.rs check that a detected token reaches every parsed, expanded and edit-made entity; both fail with the propagation removed. On x86_64 `SimdCpu::NONE` equals the default `BASELINE`, so they use `SimdCpu::detect()` and skip on a CPU with no SIMD feature.

The `build` gate, 0.61x mail-builder 1.0 before, now runs 40.3 to 18.4 µs (2.2x), ahead of mail-builder's best of 24.2 µs on the same run; `full` moved within noise (209.8 to 204.8 µs). The machine was loaded by other sessions, so these are the tightest of several runs, to re-measure on an idle machine.

Quoted-printable stays in-house: its move is the change quoted-printable-over-the-codec, waiting on quoted-printable-codec's in-flight refactor and on its two changes (a line-start guard, bare CR kept).

Capabilities moved: content-encodings (ADDED: the CPU token is the application's; MODIFIED: features, base64 reading, the encoders), building (MODIFIED: the options).
