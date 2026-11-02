---
cairn: change
id: the-scratch-area
status: active
created: 2026-11-02
---

# The scratch area

The handoff for the next speed pass over the parse and the leaf readers, written at the end of [the-tree-built-in-place](../archive/the-tree-built-in-place/proposal.md) and rewritten at the end of each pass that took it up, [the-steps-inlined](../archive/the-steps-inlined/proposal.md) then [the-state-carries-no-data](../archive/the-state-carries-no-data/proposal.md), so a fresh session can take over. Read those changes, their log entries and [benches/README.md](../../../benches/README.md) first. Part of [benchmarked-against-stalwart](../benchmarked-against-stalwart/proposal.md).

## Where things stand

Criterion on 2026-11-02 after the-state-carries-no-data (AMD Ryzen AI Max+ 395, Zen 5, rustc 1.99, bench pinned to core 4, machine load near 3.5), mime-rs against mail-parser 1.0 and mail-builder 1.0:

| workload | mime-rs | rival | ratio |
| --- | ---: | ---: | ---: |
| `structure` | 83.5 µs | 130.7 µs | 1.56x |
| `headers` | 98.0 µs | 133.7 µs | 1.36x |
| `headers`, structure depth 0, against `parse_headers` | 42.4 µs | 50.0 µs | 1.18x |
| `full` | 121.6 µs | 146.4 µs | 1.20x |
| `build` | 17.3 µs | 22.6 µs | 1.31x |
| `edit` | 74.3 µs | 403.6 µs | 5.43x |
| `edit`, structure depth 0 | 40.9 µs | 403.6 µs | 9.86x |

Cycles per corpus pass from the harness below (user space, aligned build, codegen-units 1, medians of 7 interleaved runs), mime-rs then mail-parser in the same harness: `structure` 370k against 640k, `headers` 454k against 652k, `lazy` (depth 0) 188k against 249k, `full` 562k against 737k, `edit` (parse and `to_bytes` of the 2 MiB message) about 360k. Instructions per pass: 1,669k, 1,853k, 796k, 2,230k, 1,043k. Allocations per pass: the parse 1,390 plus 21 reallocations, `entities()` 153; the bench's From read 353 plus 26, its Message-ID read 34.

Every row is ahead, the thinnest `headers` at depth 0 against `parse_headers` (1.18x) and `full` (1.20x).

## Where the time goes

Sampled cycles on `structure`: `resume` 30%, its entry and the multipart built on the stack then copied into its box (`Box::new`) the hot spots; memmem's searcher construction 7.6% (60% of it one stall inside memchr, copying the Rabin-Karp finder it has just built); `push_items` 8%; the allocator about 11%; Content-Type reading (`raw_params`, `Kind::of`, `media`) 7%; `entities()` 5%; the harness's own copies of what `content_type()` returns 7% (an artifact of `black_box`, the same in the criterion bench).

On `lazy`, half the cycles are the four header reads: Subject's `decode_text` (three scans, then a copy into a `String`), the date reader (`str::parse` per number), From (seven allocations a read: a vector per word, the phrase buffer then its decoded copy, the mailbox list, and the bench's clone of the first mailbox), and `raw_params` reading the root's boundary only to decide whether its body is a container left unparsed or a leaf.

On `full`, `text_checked` is 9%, 78% of it one load: the problem flags `decoded_as` returns through memory, waited on until every store of the decoding has drained. Inlining `decoded_as` moved the wait to the next load (2% slower).

The failed store forwards per pass (`ls_bad_status2.stli_other`): `structure` 7.4k, `headers` 5.1k, `lazy` 1.7k, `full` 7.1k. They name the sites; their count is no cost (it barely moved while this pass cut 15% of the cycles).

## Levers, in order

1. The dash-line searcher built once per caller: 7.6% of `structure`, 6% of `full`. `memmem::Finder::new` is not const, core has no lazy static and the crate forbids `unsafe`, so the shapes left are an answerer that outlives one parse (an API addition, say a reusable parser holding the finder, as mail-parser's `MessageParser`; the user's call) or a per-arch packed-pair finder (`memchr::arch::x86_64::sse2::packedpair::Finder` and its NEON twin, with a scalar tail under `min_haystack_len`; target-gated code against the goal of less code). Measure on `edit` too, where memmem's vector scan matters.
2. One scratch area per parse: the answer buffer of `cst::answer` (one per parse), the dash lines, the task and delimiter vectors (four allocations per multipart message, one per single-part message), the public `resume(Some(&[usize]))` kept. The spec forbids asking for dash lines per nested multipart (superlinear), so the store stays. Worth about 30 cycles per allocation removed.
3. The copies still stalling in `resume`: the multipart staged then copied by `Box::new` (writing it in its box with `Box::<MimeMultipart>::default()` was 2% slower, `Box::write` flat), the root moved out at completion. Confirm each with an A/B: a stall tends to move to the next copy.
4. The From reader's allocations, then the Subject's: borrowing words from the field cut 94 allocations a pass for no cycles, so measure before taking any.
5. The SIMD answerers (D4) of benchmarked-against-stalwart, once mail-parser's kernel bench says they beat memchr.

## Tried and dropped

All on 2026-11-02, measured in cycles unless said.

By the-tree-built-in-place: dash lines by `memchr` on `-` (criterion: `structure` 4% slower, dash-heavy text in the corpus); a newline walk for dash lines under 4 KiB, memmem above (noise); `Lexer::until` and `skip_fws` over `slice::position` (2% more instructions, 4% more cycles on `full`); a word-at-a-time ASCII case-insensitive name compare (2% fewer instructions, 3% to 6% more cycles on `headers` and `lazy`); `#[inline]` on `Delimiter::at`; filling `self.dashes` in place instead of moving the copied vector; `push_field` with `reserve(1)` first or a placeholder overwritten (what worked is `Vec::resize_with`, which writes the item in place, 2.5%).

By the-steps-inlined: the newline walk again (1% for 1% more instructions); `Lexer::until` over a local cursor (4% more instructions); the quoted-string scan of `quoted_cow` over `memchr2` (no change); `#[inline]` hints on `media` (ignored; only `#[inline(always)]` moves anything); `resume` `#[inline(always)]` into `cst::answer` (1% fewer instructions, 6% more cycles); the root entity moved out field by field (4% slower on `lazy`); the pending `Split` out of the state enum, alone and with `Task::digest` as a word (cycles flat); `Task::digest` as a word alone (1%); a byte search for `*` in `charset::clean_label` (0.2%).

By the-state-carries-no-data:

- The memmem searcher searched in place by `Finder::find` instead of moved into `find_iter`'s iterator: fewer copies, cycles flat.
- The multipart written in its box, its parts reserved then extended in place: 2% slower. `Box::write(Box::new_uninit(), ..)` for the multipart and the unparsed body: flat.
- A field's colon found by a byte loop instead of `memchr2`: 6% slower everywhere, 100k more instructions; the vector search finds a short name's colon in one step.
- `fields` and `get_all` double-ended, the last Content-Type looked up from the end: fewer instructions, cycles flat.
- The date's numbers read by a digit fold instead of `str::parse` (checked against `parse` on two million inputs): flat.
- Address words borrowed from the field (`Cow` atoms and quoted strings) and the phrase buffer kept as its string: 94 fewer allocations per pass, cycles flat.
- `#[inline]` on `MimeEntity::content_type`: 3.6% slower on `structure`.
- `decoded_as` `#[inline(always)]` into `text_checked`, so its problem flags stay in registers: 2% slower on `full`.

## Traps

- One binary measured twice in single runs gave 438k then 414k cycles on `structure`, and mispredicts 1,181 then 785: only medians of interleaved runs decide. Two builds of nearly the same code still differ by up to 1.5% from layout.
- Criterion compared against its saved numbers misleads once the machine's load changed (this pass's `build` read 6% slower against the morning's run while the two versions' benches, run interleaved, put it 5% faster). Build the previous tree's bench in its own target directory and interleave them.
- The failed-forward event names stalls, but removing one often moves the wait to the next load: the decoded output's stores, a `Box::new` copy, the root move. Keep a change only for its cycles.
- perf works for user space only here (`perf_event_paranoid` is 2): `-e cycles:u`, `instructions:u` and `ls_bad_status2.stli_other:u` record and annotate; call graphs fail. callgrind (`nix-shell -p valgrind`) gives exact counts and, with `--tree=caller`, who calls what (memcpy's callers, for one).
- A build script that copies the binary after `cargo build` must stop on a failed build: one copied a stale binary and measured it.
- No git command may recover a file; edit code with the Edit tool. A `perl` substitution using `|` as its delimiter turns every `\|` into an alternation: one rewrote the wrong line of src/tree/cst.rs this pass (restored from the scratch copy of the baseline).
- The differential run never reached the address reader's comma-less split (the corpus holds none): it compares every field reader on random inputs now, and a planted break there was caught only then.

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

The `edit` workload reads one message written by the bench's builder code (a text body and a 2 MiB `application/pdf` attachment) to a file once, then runs `MimeCst::parse` and `to_bytes` on it: when `work == "edit"`, replace the corpus with that file and add the arm `"edit" => { let cst = MimeCst::parse(black_box(m)); black_box(cst.to_bytes()); }`.

Build it aligned with `RUSTFLAGS='-C llvm-args=-align-all-functions=6 -C llvm-args=-align-loops=64'`, stop on a failed build, and copy each version's binary aside under its own name. Count cycles at two pass counts and divide the difference, which removes the corpus loading. To compare two versions, interleave their binaries, 5 to 7 runs each, and compare medians, the instruction count beside the cycles (`ab.sh old-binary new-binary 7 structure headers lazy full`):

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

To find stalling copies, add `ls_bad_status2.stli_other:u` and `ex_ret_brn_misp:u` to the `perf stat` events, and sample on the first (`perf record -c 2000 -e ls_bad_status2.stli_other:u`) with a build carrying debug info (`CARGO_PROFILE_RELEASE_DEBUG=1`, its own target directory), then `perf annotate -s '<full symbol name>'`; the symbol must be written out as `perf report` prints it.

For a baseline, copy the tree before the pass (src, tests, benches, Cargo.toml and Cargo.lock) into a scratch folder and point a second copy of the harness at it.

The differential run, for any parser or reader change: copy src/ and Cargo.toml of the previous code into a scratch crate renamed `mime-old` (lib name `mime_old`, the `[features]` and `[dependencies]` sections kept, the examples and benches dropped), then:

- for the corpus and 300 rounds of random edits of it (byte replaced, removed or inserted from an alphabet of `-`, CR, LF, `:;="`, `boundary`, `multipart/mixed`, `message/rfc822`, `Multipart/Alternative`, `MESSAGE/RFC822`, `(c) `, HTAB, `M`, `m`, `*`, `text/plain`, chunks copied elsewhere), at `structure_depth` `None`, `Some(0)` and `Some(1)` with and without `max_entities: Some(40)` and `max_depth: Some(4)`, assert that `format!("{:?}")` of both trees match, that `to_bytes()` gives the input back, and that the expanded roots match; compare, entity by entity of a full parse, the media type, `param` of `boundary`, `charset`, `name`, `format` and `delsp`, `to_content_type`, `text_checked`, `decoded_checked`, `attachment_name`, and `decode()` of the whole tree;
- drive both coroutines with the same noisy answerer (the memchr answer with positions inserted, removed, duplicated, swapped, or near `usize::MAX`, from one seed), `max_entities: Some(30)`, and assert both the request sequences and the trees match;
- feed every field reader (`parse_addresses`, `parse_mailboxes`, `parse_message_ids`, `parse_urls`, `parse_comma_list`, `parse_date`, `parse_content_type`, `MimeMediaType` and its `param`, `is`, `is_top`) every field value of each parsed message and 400,000 random strings over an address alphabet (`@ < > , ; : " \ ( )`, blanks, folds, encoded words, non-ASCII bytes, digits, month names).

Break the old copy once on purpose, in the very code the change touches, to see it fail.

Allocation counts: a scratch binary with a counting `#[global_allocator]` around `System`, counting allocations, reallocations and a size histogram over one corpus pass of `MimeCst::parse` plus `entities()`, and of each of the four header reads.

## Before landing

The suite (`cargo test --all-features` and `--no-default-features`), clippy with `--all-targets` under both, `cargo build --no-default-features` pulling only quoted-printable-codec and simd-cpu, docs with `-Dwarnings`, `cargo fmt`; the differential run; a criterion run of the whole bench, the previous tree's bench interleaved when the load is not quiet, its numbers in README.md (Benchmarks) and benches/README.md; a log entry, the pass's change archived, and this handoff rewritten.
