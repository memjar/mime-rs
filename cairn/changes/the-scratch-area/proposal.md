---
cairn: change
id: the-scratch-area
status: active
created: 2026-11-02
---

# The scratch area

The handoff for the next speed pass over the parse and the leaf readers, written at the end of [the-tree-built-in-place](../archive/the-tree-built-in-place/proposal.md) and rewritten at the end of [the-steps-inlined](../archive/the-steps-inlined/proposal.md), the pass that took it up, so a fresh session can take over. Read those two changes, their log entries and [benches/README.md](../../../benches/README.md) first. Part of [benchmarked-against-stalwart](../benchmarked-against-stalwart/proposal.md).

## Where things stand

Criterion on 2026-11-02 after the-steps-inlined (AMD Ryzen AI Max+ 395, Zen 5, rustc 1.99, bench pinned to core 4), mime-rs against mail-parser 1.0 and mail-builder 1.0:

| workload | mime-rs | rival | ratio |
| --- | ---: | ---: | ---: |
| `structure` | 92.7 µs | 126.7 µs | 1.37x |
| `headers` | 108.0 µs | 129.2 µs | 1.20x |
| `headers`, structure depth 0, against `parse_headers` | 46.5 µs | 49.5 µs | 1.06x |
| `full` | 134.0 µs | 141.2 µs | 1.05x |
| `build` | 16.3 µs | 22.4 µs | 1.37x |
| `edit` | 73.9 µs | 386.9 µs | 5.24x |
| `edit`, structure depth 0 | 38.7 µs | 386.9 µs | 10.0x |

Cycles per corpus pass from the harness below (user space, aligned build, codegen-units 1, medians of 7 interleaved runs), mime-rs then mail-parser: `structure` 431k against 623k, `headers` 522k against 634k, `lazy` (depth 0) 220k against 248k, `full` 625k against 705k, `edit` (parse and `to_bytes` of the 2 MiB message) 385k. Instructions per pass: 1,857k, 2,049k, 907k, 2,467k, 1,044k.

Every row is ahead. The thinnest are `full` and the depth 0 row against `parse_headers`, 1.05x and 1.06x. The criterion and cycle ratios differ because the bench links memchr with its `std` feature (criterion turns it on: AVX2 kernels, runtime detection) while a harness depending on mime alone gets SSE2, and because of codegen units; trust each method against itself.

## Where the time goes

Cycles first, from `perf annotate` (sampled, user space): the last pass found that instruction counts hid store-forwarding stalls, a wide load waiting on narrower stores of the same bytes. Inlining the steps into `resume` removed the worst; two remain, together about a tenth of the `structure` and `lazy` cycles:

- `mem::replace(&mut self.state, State::Done)` at the top of `resume` copies the whole state enum (Task-sized payload plus the unused tail of the larger variants), and loads bytes last written narrowly: the byte of `Task::digest` and its padding, or stale bytes of the variant's tail. Moving the `Split` out of the enum and making `digest` a word did not pay (instructions down, cycles flat): the stall moved to the next copy.
- `mem::take(&mut self.root)` when the parse completes copies the root entity, whose anomaly byte and body were written just before; on `lazy` the root is the only entity, so the copy follows the writes closely. A field-wise move was 4% slower.

Then instructions, callgrind on `structure` (exact counts, 100 passes):

- The allocator, about 20%. A pass allocates 1,554 blocks (plus 32 reallocations) for 153 messages and 529 entities, and `entities()` 306 more. About 600 are scratch: the answer buffer of `cst::answer` (64 slots, one per parse), and per multipart message the task vector, the path vector, the copy of the dash lines into `MimeParse::dashes` and the delimiter buffer. Dropping `MimeParse` is 3% on its own. The rest is the tree: one item vector per header block, a boxed `MimeMultipart` and a parts vector per multipart, a box per nested message.
- Reading Content-Type, about 20% with the bench's own `content_type()`: the field name compares of `get_all("content-type")` (`[u8]::eq_ignore_ascii_case`, about 3%), `media` (about 160 instructions a call), `raw_params` for the boundary (about 570 instructions a multipart; the quoted value scan and the name compare). `Kind::of` no longer lexes a type opening with a letter other than `m`.
- The dash-line search, 6.5%: building the memmem searcher a third of it (and a stall inside memchr's own constructor, 4% of the cycles), the search the rest; `line_delimiters` 5% (a `memcmp` call per candidate).
- `full` alone: `text_checked` with `charset::transcode` and `is_utf8_label`, which trims and splits every label (`clean_label`) before six compares, about 5%.

## Levers, in order

1. The two stalling copies of `resume`. The state copy wants a state whose resumable data is read field by field rather than copied whole, without a byte field padded inside a copied struct; the root copy wants the root's last writes far from the move, or no move. Confirm each try with `perf annotate` and an interleaved A/B, as a stall tends to move to the next copy.
2. One scratch area per parse, unchanged from the first handoff: the task vector, the path, the delimiter buffer and the dash lines in one allocation, or the answer buffer handed to the coroutine as the dash-line store through a crate-private method, the public `resume(Some(&[usize]))` kept. The spec forbids asking for dash lines per nested multipart (superlinear), so the store stays. Worth about 2% per 250 allocations removed.
3. `MimeCst::entities` with one allocation, pre-order kept, no recursion. Splicing children after their parent in the output is one allocation but quadratic on wide trees; a small fixed stack of part iterators spilling into a vector is linear.
4. The dash-line searcher built once per caller: `memmem::Finder::new` is not const, core has no `OnceLock`, the crate forbids `unsafe`. A per-arch packed-pair finder or an answerer that outlives one parse are the shapes left; measure on `edit` too, where memmem's SIMD scan matters.
5. The field name compare for the crate's own lowercase names (only the input side needs lowering), measured in cycles: the SWAR version lost.
6. The SIMD answerers (D4) of benchmarked-against-stalwart, once mail-parser's kernel bench says they beat memchr.

## Tried and dropped

All on 2026-11-02, measured in cycles unless said. By the-tree-built-in-place:

- Dash lines by `memchr` on `-` (criterion: `structure` 4% slower): the corpus has dash-heavy text and HTML.
- A newline walk for dash lines under 4 KiB, memmem above: no gain beyond noise.
- `Lexer::until` and `skip_fws` over `slice::position`: 2% more instructions on `structure`, 4% more cycles on `full`.
- A word-at-a-time ASCII case-insensitive name compare (SWAR, exhaustively tested against core): 2% fewer instructions, 3% to 6% more cycles on `headers` and `lazy`.
- `#[inline]` on `Delimiter::at`, and filling `self.dashes` in place instead of moving the copied vector: nothing.
- `push_field` with `reserve(1)` first, or with a placeholder pushed then overwritten: nothing. What worked is `Vec::resize_with`, which computes the item after the capacity check and writes it in place (2.5%).

By the-steps-inlined:

- The newline walk again, after the inlining: 1% on `structure` and `full` for 1% more instructions.
- `Lexer::until` over a local cursor with `skip_fws` built on it: 4% more instructions everywhere.
- The quoted-string scan of `quoted_cow` over `memchr2`: no change in instructions or cycles.
- `#[inline]` hints on `media`: ignored by LLVM (identical instruction counts); only `#[inline(always)]` moves anything.
- `resume` itself `#[inline(always)]` into `cst::answer`: 1% fewer instructions, 6% more cycles.
- The root entity moved out field by field: 4% slower on `lazy`.
- The pending `Split` out of the state enum, alone and with `Task::digest` as a word: instructions down 1.3%, cycles flat.
- `Task::digest` as a word alone: 1% on `structure`, `headers` and `full`, not worth the odd type.
- A byte search for `*` in `charset::clean_label`: 0.2% of `full`.


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

It repeats within 1% (3% on `lazy`) for one binary, but two builds of nearly the same code differ by up to 1.5% from layout alone. To compare two versions, keep both binaries and interleave them, 5 to 7 runs each, comparing medians and reading the instruction count beside the cycles (`ab.sh old-binary new-binary 7 structure headers lazy full`):

```sh
A=$1; B=$2; r=$3; shift 3
one() {
  for n in 200 1200; do
    nix-shell -p perf --run "taskset -c 4 perf stat -x, -e cycles:u,instructions:u $1 $2 $n" 2>&1 >/dev/null | cut -d, -f1 | tr '\n' ' '
    echo
  done | awk 'NR==1{c=$1;i=$2} NR==2{printf "%.1f %.1f\n", ($1-c)/1e6, ($2-i)/1e6}'
}
for w in "$@"; do
  : > a.txt; : > b.txt
  for i in $(seq $r); do one $A $w >> a.txt; one $B $w >> b.txt; done
  for f in a b; do sort -n $f.txt | awk -v w=$w -v f=$f '{a[NR]=$1; i=$2} END{printf "%-10s %s med %7.1f kcycles %8.1f kinstr\n", w, f, a[int((NR+1)/2)], i}'; done
done
```

The `edit` workload of the harness reads one message written by the bench's builder code (a text body and a 2 MiB `application/pdf` attachment) to a file once, then runs `MimeCst::parse` and `to_bytes` on it: when `work == "edit"`, replace the corpus with that file and add the arm `"edit" => { let cst = MimeCst::parse(black_box(m)); black_box(cst.to_bytes()); }`.

For a baseline of an older commit, `git archive <rev> | tar -x` into a scratch folder and point a second copy of the harness at it.


The differential run, for any parser change: copy src/ and Cargo.toml of the previous parser into a scratch crate renamed `mime-old` (lib name `mime_old`, the `[features]` and `[dependencies]` sections kept, the examples and benches dropped), then for the corpus and 300 rounds of random edits of it (byte replaced, removed or inserted from an alphabet of `-`, CR, LF, `:;="`, `boundary`, `multipart/mixed`, `message/rfc822`, `Multipart/Alternative`, `MESSAGE/RFC822`, `(c) `, HTAB, `M`, `m`, `*`, `text/plain`, chunks copied elsewhere), at `structure_depth` `None`, `Some(0)` and `Some(1)` with `max_entities: Some(40)` and `max_depth: Some(4)`, assert that `format!("{:?}")` of both trees match, that `to_bytes()` gives the input back, and that the expanded roots match. Also compare, entity by entity of a full parse, the media type, `param` of `boundary`, `charset`, `name`, `format` and `delsp`, `to_content_type`, `text_checked`, `decoded_checked`, `attachment_name`, and `decode()` of the whole tree. Break the old copy once on purpose, in the very code the change touches, to see it fail: with the first alphabet, a case-sensitive `multipart` compare planted in the old parser went unseen, as the corpus writes the type in lowercase only.

Allocation counts: a scratch binary with a counting `#[global_allocator]` around `System`, counting allocations, reallocations and a size histogram over one corpus pass of `MimeCst::parse` plus `entities()`.

## Before landing

The suite (`cargo test --all-features` and `--no-default-features`), clippy with `--all-targets`, `cargo build --no-default-features` pulling only quoted-printable-codec and simd-cpu, docs with `-Dwarnings`, `cargo fmt`; the differential run for any parser change; a quiet-machine criterion run of the whole bench, its numbers in README.md (Benchmarks) and benches/README.md; a log entry, this change archived, and the handoff for the pass after it.
