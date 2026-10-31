# Fuzzing

Coverage-guided fuzzing of mime-rs with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer). cargo-fuzz needs a nightly toolchain; on NixOS the dedicated fuzz/shell.nix provides both (nightly via fenix plus cargo-fuzz):

```sh
nix-shell fuzz/shell.nix --run "cargo fuzz run parse"
```

## Targets

- `parse`: arbitrary bytes never panic the parser, the tree serializes back to the input byte for byte, and its output reparses to the same bytes.
- `scan`: the memchr search answerer agrees with the scalar reference for every request, range and needle.
- `answer`: the parse coroutine resumed with arbitrary positions never panics and still round-trips its input.

Do not pass tests/corpus as the corpus directory: libFuzzer writes into the first one and would dump generated inputs into the curated fixtures. Seed a copy instead:

```sh
mkdir -p fuzz/corpus/parse
for f in tests/corpus/*/*.eml; do cp "$f" "fuzz/corpus/parse/$(basename "$(dirname "$f")")_$(basename "$f")"; done
```
