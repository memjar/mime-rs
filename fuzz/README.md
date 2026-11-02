# Fuzzing

Coverage-guided fuzzing of mime-rs with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer). cargo-fuzz needs a nightly toolchain; on NixOS the dedicated fuzz/shell.nix provides both (nightly via fenix plus cargo-fuzz):

```sh
nix-shell fuzz/shell.nix --run "cargo fuzz run parse"
```

## Targets

- `parse`: arbitrary bytes never panic the parser, the tree serializes back to the input byte for byte, and its output reparses to the same bytes.
- `scan`: the memchr search answerer agrees with the scalar reference for every request, range and needle.
- `answer`: the parse coroutine resumed with arbitrary positions never panics and still round-trips its input.
- `fields`: every field form read from arbitrary bytes never panics, and a value read, written and read again is stable, its bytes a fixpoint.
- `decode`: the content codecs over arbitrary bytes never panic, and what the base64 and quoted-printable encoders write decodes back exactly.
- `write`: the model of arbitrary bytes, written under options the first byte picks, rebuilds to the same bytes and reads back the same.
- `edit`: a few edits the first bytes pick, applied to the rest, never panic, and after each the tree reads back as edited, its bytes a fixpoint.
- `differential`: arbitrary bytes read by this crate and by mail-parser 1.0 agree on the part tree, the decoded content, the decoded header values and the body lists, but where a rule of tests/common/differential.rs lets a deliberate difference through (see CONTRIBUTING.md). Seed it with fixtures and the `parse` corpus as a second directory.

Do not pass tests/corpus as the corpus directory: libFuzzer writes into the first one and would dump generated inputs into the curated fixtures. Seed a copy instead:

```sh
mkdir -p fuzz/corpus/parse
for f in tests/corpus/*/*.eml; do cp "$f" "fuzz/corpus/parse/$(basename "$(dirname "$f")")_$(basename "$f")"; done
```

The `write` and `edit` targets read their first bytes as options or edits; seed them with fixtures prefixed accordingly (`printf '\x00'`, or `printf '\x03\x00\x00\x00\x05\x01\x00\x09\x02\x01'` for `edit`). A finding becomes a case in the target's test file (tests/build.rs `fuzz_findings_rebuild_stably` for `write`) before it is fixed. A `differential` finding is either a bug, fixed with a test failing on the old code, or a deliberate reading, recorded as a rule with its reason.

In fork mode a finding does not stop the run, so a campaign can last, as here the `differential` target for a day:

```sh
nix-shell fuzz/shell.nix --run "cd fuzz && cargo fuzz run differential corpus/differential corpus/parse -- -fork=4 -ignore_crashes=1 -max_total_time=86400"
```
