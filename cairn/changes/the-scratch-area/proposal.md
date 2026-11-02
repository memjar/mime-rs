---
cairn: change
id: the-scratch-area
status: active
created: 2026-11-02
---

# The scratch area

The handoff for the next speed pass over the parse and the leaf readers, written at the end of [the-tree-built-in-place](../archive/the-tree-built-in-place/proposal.md) so a fresh session can take over. Read that change, its log entry and [benches/README.md](../../../benches/README.md) first. Part of [benchmarked-against-stalwart](../benchmarked-against-stalwart/proposal.md).

## Where things stand

Criterion on 2026-11-02 (AMD Ryzen AI Max+ 395, Zen 5, rustc 1.99, bench pinned to core 4), mime-rs against mail-parser 1.0 and mail-builder 1.0:

| workload | mime-rs | rival | ratio |
| --- | ---: | ---: | ---: |
| `structure` | 105.8 µs | 127.8 µs | 1.21x |
| `headers` | 122.1 µs | 130.9 µs | 1.07x |
| `headers`, structure depth 0, against `parse_headers` | 50.0 µs | 48.5 µs | 0.97x |
| `full` | 145.4 µs | 142.8 µs | 0.98x |
| `build` | 16.4 µs | 22.1 µs | 1.35x |
| `edit` | 74.2 µs | 391.1 µs | 5.27x |
| `edit`, structure depth 0 | 41.8 µs | 391.1 µs | 9.36x |

Cycles per corpus pass from the harness below (user space, aligned build, codegen-units 1), mime-rs then mail-parser: `structure` 479k against 622k, `headers` 568k against 640k to 669k, `lazy` (depth 0) 226k against 250k, `full` 680k against 705k. Instructions per pass: 2,023k, 2,221k, 956k, 2,618k. The parse retires about 4.2 instructions a cycle, so the instruction count is what to cut; stalls are not the limit.

The two rows still behind are `full` (0.98x criterion, 1.04x in cycles) and the depth 0 row against `parse_headers` (0.97x criterion, 1.10x in cycles). The criterion and cycle ratios differ because the bench links memchr with its `std` feature (criterion turns it on: AVX2 kernels, runtime detection) while a harness depending on mime alone gets SSE2, and because of codegen units; trust each method against itself.

## Where the time goes

Callgrind on `structure` (exact instruction counts, 200 passes):

- The allocator, about 18%: glibc `_int_free` 7%, `malloc` 5%, `free` 3.6%, the rest in `_int_malloc` and the Rust shims. A pass allocates 2,155 blocks for 153 messages and 529 entities. About 1,000 of them are scratch, not tree: per parse the task vector, the path vector, the answer buffer of `cst::answer` (64 slots), the copy of the dash lines into `MimeParse::dashes`, the delimiter buffer; per call of `MimeCst::entities` its result and its stack (306 a pass, the bench calls it once per message). The rest is the tree itself: one item vector per header block (529), a boxed `MimeMultipart` and a parts vector per multipart, a box per nested message.
- Reading Content-Type, about 12%: `MimeMediaType::new` (about 135 instructions a call, called twice per entity, once by `Kind::of` in the parse and once by the bench's `content_type()`), the field name compares in `get_all("content-type")` (`[u8]::eq_ignore_ascii_case`, about 5% with the other name lookups), `raw_params` for the boundary (about 700 instructions a call on a root Content-Type).
- `memmem::Searcher::new` for the dash-line search, about 840 instructions per multipart message, once per parse.
- Dropping the tree: `drop_glue` of `MimeEntity` and `MimeMultipart`, about 9% of `full`, mostly the frees above.
- `full` alone: `text_checked` and `charset::transcode` (UTF-8 validation, the label compares of `is_utf8_label`), about 6%.

## Levers, in order

1. One scratch area per parse. The task vector, the path, the delimiter buffer and the dash lines could live in one allocation, or the answer buffer could become the dash-line store: `cst::answer` is crate-private, so it may hand its `found` vector to the coroutine through a crate-private method instead of the public `resume(Some(&[usize]))`, which forces the copy. Keep the public coroutine API as it is. Worth about 2% per 250 allocations removed (each alloc and free pair costs about 40 cycles here).
2. `MimeCst::entities` with one allocation: its stack could be avoided for shallow trees (a small fixed array spilling into a vector), keeping the pre-order the anomaly indices rely on and no recursion (the spec forbids recursing per level).
3. The dash-line searcher built once per caller rather than once per parse. `memmem::Finder::new` is not const and core has no `OnceLock`, and the crate forbids `unsafe`, so a static is out; the realistic shape is a crate-private answerer holding a `Finder<'static>` that `MimeCst::parse` builds once per call chain, or a per-arch packed-pair finder (`memchr::arch::x86_64::avx2::packedpair::Finder`), which is arch-specific and needs detection. Measure against the edit workload (2 MiB body), where memmem's SIMD scan matters.
4. `MimeMediaType::new` and `raw_params` with fewer instructions per byte. A rewrite of the lexer's loops over `position` was tried and lost (below); a byte table or a fast path for the common `type/subtype;` shape, checked against the lexer on every value of the corpus and the fuzz targets, has not been tried.
5. `is_utf8_label` and `clean_label` on every text leaf: compare the common labels without trimming first.
6. The SIMD answerers (D4) of benchmarked-against-stalwart, once mail-parser's kernel bench says they beat memchr.

## Tried and dropped

All on 2026-11-02, measured in cycles unless said:

- Dash lines by `memchr` on `-` (criterion: `structure` 4% slower): the corpus has dash-heavy text and HTML.
- A newline walk for dash lines under 4 KiB, memmem above: no gain beyond noise.
- `Lexer::until` and `skip_fws` over `slice::position`: 2% more instructions on `structure`, 4% more cycles on `full`.
- A word-at-a-time ASCII case-insensitive name compare (SWAR, exhaustively tested against core): 2% fewer instructions, 3% to 6% more cycles on `headers` and `lazy`.
- `#[inline]` on `Delimiter::at`, and filling `self.dashes` in place instead of moving the copied vector: nothing.
- `push_field` with `reserve(1)` first, or with a placeholder pushed then overwritten: nothing. What worked is `Vec::resize_with`, which computes the item after the capacity check and writes it in place (2.5%).

## Traps

- Criterion medians move by 2% to 3% between builds of the same code (function layout), and more when other sessions build or test on the machine (load around 4 doubled a row's noise). One rebuilt-unchanged run showed `headers` 121.5 µs then 135.7 µs. Decide on cycles, report with criterion on a quiet machine, and check `uptime` before a criterion run.
- perf works for user space only here (`perf_event_paranoid` is 2): `-e cycles:u` and `instructions:u` record and annotate; any call graph (`-g`, `--call-graph dwarf` or `lbr`) fails to open its events. Use callgrind (`nix-shell -p valgrind`) for inclusive costs and exact counts; it runs the SSE2 paths, so it has no AVX-512 trouble on the parse workloads.
- Sampled hot spots skid: a store showing 90% of a function's samples was partly the cost of what precedes it. On this code a 16-byte load right after 8-byte and 4-byte stores of the same struct (a `Vec::push` of a multi-word value staged on the stack) is a real store-forwarding stall, but removing one moved the samples elsewhere for 2.5%, not the 20% the annotation suggested. Confirm every reading with a cycle count.
- A parser rewrite needs the differential run below, not the tests alone: the tests passed on intermediate states the differential would also have passed, but they cover few fallback paths.
- No git command may recover a file; edit code with the Edit tool rather than `perl -0pi` with braces in the pattern (a failed substitution once dropped an `impl` line).

## How to measure

Keep these outside the repo (a scratchpad), and point the path dependency at the working tree.

The cycle harness, Cargo.toml:

```toml
[package]
name = "cyc"
version = "0.0.0"
edition = "2024"

[dependencies]
mime = { package = "mime-rs", path = "/home/soywod/code/pimalaya/mime" }
simd-cpu = { version = "0.0.1", git = "https://github.com/pimalaya/simd-cpu", rev = "f2eb72cff2fafad8ae135e00685bc19f9e80e5cc", features = ["std"] }
mail-parser = { version = "1", default-features = false, features = ["full_encoding"] }

[profile.release]
codegen-units = 1
```

src/main.rs, run as `cyc <workload> <passes>`:

```rust
use std::{fs, hint::black_box, path::PathBuf};
use mime::{header::marker::{DATE, FROM, MESSAGE_ID, SUBJECT}, tree::{body::MimeBodyNode, cst::MimeCst, parse::MimeParseOptions}};

fn main() {
    let _ = simd_cpu::SimdCpu::install_detected();
    let mut args = std::env::args().skip(1);
    let work = args.next().unwrap();
    let n: usize = args.next().unwrap().parse().unwrap();
    let root = PathBuf::from("/home/soywod/code/pimalaya/mime/tests/corpus");
    let mut corpus = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(root).unwrap().flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for dir in dirs {
        let mut files: Vec<_> = fs::read_dir(dir).unwrap().flatten().map(|f| f.path()).filter(|p| p.extension().is_some_and(|e| e == "eml")).collect();
        files.sort();
        for f in files { corpus.push(fs::read(f).unwrap()); }
    }
    let parser = mail_parser::MessageParser::new();
    let lazy = MimeParseOptions { structure_depth: Some(0), ..Default::default() };
    for _ in 0..n {
        for m in &corpus {
            match work.as_str() {
                "structure" => { let cst = MimeCst::parse(black_box(m)); for e in cst.entities() { black_box(e.content_type()); } }
                "headers" | "lazy" => {
                    let cst = if work == "lazy" { MimeCst::parse_with(black_box(m), lazy.clone()) } else { MimeCst::parse(black_box(m)) };
                    black_box(cst.root.header::<SUBJECT>());
                    black_box(cst.root.header::<FROM>().and_then(|l| l.first().cloned()));
                    black_box(cst.root.header::<DATE>().map(|d| d.to_unix_seconds()));
                    black_box(cst.root.header::<MESSAGE_ID>());
                }
                "full" => { let cst = MimeCst::parse(black_box(m)); for e in cst.entities() { if matches!(e.body, MimeBodyNode::Leaf(_)) { if e.content_type().is_top("text") { black_box(e.text()); } else { black_box(e.decoded()); } } } }
                "mp-structure" => { if let Some(m) = parser.parse(black_box(m)) { for p in m.parts() { black_box(p.content_type()); } } }
                "mp-headers" => { if let Some(m) = parser.parse(black_box(m)) { black_box(m.subject()); black_box(m.from().and_then(|f| f.first()).map(|a| a.address())); black_box(m.date().map(|d| d.to_timestamp())); black_box(m.message_id()); } }
                "mp-lazy" => { if let Some(m) = parser.parse_headers(black_box(m)) { black_box(m.subject()); black_box(m.from().and_then(|f| f.first()).map(|a| a.address())); black_box(m.date().map(|d| d.to_timestamp())); black_box(m.message_id()); } }
                "mp-full" => { if let Some(m) = parser.parse(black_box(m)) { for p in m.parts() { if p.is_text() { black_box(p.text()); } else if !p.is_multipart() { black_box(p.decoded()); } } } }
                _ => panic!(),
            }
        }
    }
}
```

The driver: build aligned, then count cycles at two pass counts and divide the difference, which removes the corpus loading:

```sh
cd cyc
RUSTFLAGS='-C llvm-args=-align-all-functions=6 -C llvm-args=-align-loops=64' \
  nix develop /home/soywod/code/pimalaya/mime --command cargo build --release -q
for w in structure headers lazy full; do
  for n in 200 1200; do
    nix-shell -p perf --run "taskset -c 4 perf stat -x, -e cycles:u,instructions:u target/release/cyc $w $n" 2>&1 >/dev/null | cut -d, -f1 | tr '\n' ' '
    echo
  done | awk -v w=$w 'NR==1{c=$1;i=$2} NR==2{printf "%-10s %8.1f kcycles %8.1f kinstr\n", w, ($1-c)/1e6, ($2-i)/1e6}'
done
```

It repeats within 1% (3% on `lazy`). For a baseline of an older commit, `git archive <rev> | tar -x` into a scratch folder and point a second copy of the harness at it.

The differential run, for any parser change: copy src/ and Cargo.toml of the previous parser into a scratch crate renamed `mime-old` (lib name `mime_old`, the `[features]` and `[dependencies]` sections kept, the examples and benches dropped), then for the corpus and 300 rounds of random edits of it (byte replaced, removed or inserted from an alphabet of `-`, CR, LF, `:;="`, `boundary`, `multipart/mixed`, `message/rfc822`, chunks copied elsewhere), at `structure_depth` `None`, `Some(0)` and `Some(1)` with `max_entities: Some(40)` and `max_depth: Some(4)`, assert that `format!("{:?}")` of both trees match, that `to_bytes()` gives the input back, and that the expanded roots match. Break the old copy once on purpose to see it fail.

Allocation counts: a scratch binary with a counting `#[global_allocator]` around `System`, counting allocations, reallocations and a size histogram over one corpus pass of `MimeCst::parse` plus `entities()`.

## Before landing

The suite (`cargo test --all-features` and `--no-default-features`), clippy with `--all-targets`, `cargo build --no-default-features` pulling only quoted-printable-codec and simd-cpu, docs with `-Dwarnings`, `cargo fmt`; the differential run for any parser change; a quiet-machine criterion run of the whole bench, its numbers in README.md (Benchmarks) and benches/README.md; a log entry, this change archived, and the handoff for the pass after it.
