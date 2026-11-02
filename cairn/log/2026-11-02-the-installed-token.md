---
cairn: log
change: the-installed-token
landed: 2026-11-02
---

# The installed token

Landed the process-wide CPU token in place of the plumbing the-simd-codecs added the same day: `MimeEntity` and `MimeParseOptions` lost their `cpu` field, with every copy of it in the parser, edits, expansion and `nested`, which went back to their state before that change. Leaf accessors and RFC 2047 B words decode under `SimdCpu::current()`, the token the application installed or the baseline. `MimeBuildOptions::cpu` is now `Option<SimdCpu>`, `None` taking `SimdCpu::current()` when a body is encoded. The `codec::base64` functions keep their explicit token.

base64-codec is pinned at a478087 and simd-cpu at f77308a. The bench installs the detected token at the start of each group, criterion owning `main`; the example installs it first thing in `main`. The two tests of the-simd-codecs checking the entity field went with it; the backend a leaf runs is not observable through mime, and the codecs' own install tests cover the token.

The proposal of an-installed-token in simd-cpu had a `cpu` field in `MimeParseOptions` too: without the entity field it would reach nothing, so it went.

Capabilities moved: content-encodings (MODIFIED: the CPU token is the application's), building (MODIFIED: the options).
