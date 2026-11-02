---
cairn: change
id: the-installed-token
status: landed
created: 2026-11-02
---

# The installed token

Follows [the-simd-codecs](../archive/the-simd-codecs/proposal.md), which brought the CPU token in as a field of the options and of every entity.

## Why

simd-cpu now holds a process-wide token (its change an-installed-token, f77308a): the application installs it once, first thing in `main`, and libraries read `SimdCpu::current()`, the installed token or `SimdCpu::BASELINE`. The `cpu` field of every `MimeEntity`, copied by the parser, by edits and by expansion, and taking part in entity equality, existed only to carry the token from the parse to the leaf readers. It can go.

## What

Settled with the user on 2026-11-02: the application installs, libraries read the installed token, and an explicit token, where a caller can give one, wins.

- `MimeEntity` loses its `cpu` field, and `MimeParseOptions` its own: the parser decodes nothing, and the leaf accessors take no options, so they decode under `SimdCpu::current()`, read per leaf. RFC 2047 B words decode and encode under it too, instead of `BASELINE`.
- `MimeBuildOptions::cpu` becomes `Option<SimdCpu>`: `None`, the default, takes `SimdCpu::current()` when a body is encoded; `Some` is the caller's token.
- `codec::base64::decode` and `codec::base64::encode` keep their explicit token, the codec module being usable alone.
- base64-codec is pinned at a478087 and simd-cpu at f77308a, in Cargo.toml and fuzz/Cargo.toml.
- The bench installs the detected token before its rows; the example installs it in `main`; tests and fuzz targets drop the field or pass `None`.

The proposal of an-installed-token in simd-cpu had a `cpu: Option<SimdCpu>` in `MimeParseOptions` too: without the entity field it would reach nothing, so it goes.

## Risks

- A reader built before an install decodes under whatever is installed when it reads, not when it parsed: the token is the process's, so this only matters before `main` installs, where the baseline is safe.
