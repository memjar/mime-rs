---
cairn: change
id: a-byte-faithful-message-tree
status: landed
created: 2026-10-31
---

# A byte-faithful message tree

Phase 1 of [a-mime-library](../../a-mime-library/proposal.md). It folds into a new cairn/spec/parsing.md.

## Why

Editing, signed-content handling and a mail store that never loses a byte all stand on one property: parsing then serializing an unedited message gives back the input exactly. mail-parser keeps offsets into the raw input but cannot write anything back, and a parse-then-rebuild loses folds, parameter order, quoting, transport padding, preambles, epilogues and every malformation a real message carries.

## What

### The tree

`MimeCst<'a>` is built the way `VcardCst` and `IcalCst` are: plain structs with public fields, every leaf a `Cow<'a, [u8]>` (or `Cow<'a, str>` for a field name) borrowed from the input, owned only once an edit or a build replaces it.

- An entity (`MimeEntity`) is a header block and a body.
- A header block (`MimeHeaderBlock`) is a `Vec` of items, each a field (`MimeHeaderField`: name leaf, the blanks and colon, the raw value leaf with its folds, the line ending) or an opaque line, then the empty line ending the block, if any.
- A body (`MimeBodyNode`) is a leaf (raw, still-encoded bytes), a multipart (preamble, a `Vec` of parts each holding its delimiter line and its entity, the close delimiter, the epilogue) or a nested message (a boxed entity).
- `MimeCst::to_bytes` writes every leaf in order, `MimeCst::chunks` yields the same leaves as borrowed slices, and `Display` is the lossy UTF-8 convenience over them.

This shape is what makes phase 5's edits local: a field edit swaps one value leaf, a part edit one body leaf, a structural edit one part list, and everything else stays borrowed. mail-parser's flat arena of input offsets would be faster to build and cannot hold an edited byte.

Parsing reads Content-Type and Content-Transfer-Encoding of each entity, the minimum needed to find boundaries and nested messages, and leaves every other value as an unparsed leaf (D6). The Content-Type reader is the phase 2 grammar's first, smallest slice (type, subtype, `boundary`), shared rather than duplicated.

### The reading, field by field of the regression catalogue

mail-parser 1.0's regression catalogue (tests/corpus/stalwart-regressions/README.md once vendored) is a list of decided cases. This change adopts every structural one, each as a requirement in the delta or a row of the divergence file:

- Delimiters: `--` and the boundary at the start of a line, then end of line, transport padding or `--`; a longer token is content; the line ending before a delimiter belongs to it (RFC 2046 5.1.1).
- Fallbacks: a delimiter found only mid-line opens or closes the multipart and is flagged (GitHub issue 156); a boundary made only of dashes never matches; an outer delimiter closes every inner multipart still open, which is flagged unterminated (RFC 2046 5.1.2).
- Cut header blocks: a delimiter cuts a part's or a nested message's header block that never reached its empty line.
- Missing pieces: no close delimiter, a multipart with no boundary parameter, a boundary never found, an empty `message/rfc822`, an encoded `message/rfc822` whose content has no header.
- Header blocks: a field name holding a space, a leading colon, a control byte or a NUL; blanks before the colon (the RFC 5322 4.5 obsolete syntax); a whitespace-only line inside a block; a colon-less line before a fold; a block with no line break at all.

### The parse coroutine

The parse is `tree::parse::MimeParse`, an I/O-free coroutine shaped as io-imap's (D4), the exact types settled with io-imap's reference coroutines open. It performs no byte search of its own: it yields bulk requests carrying offsets into the input, and the caller answers each with positions in a buffer it owns and reuses:

```rust
let mut parse = MimeParse::new(input, MimeParseOptions::default());
let mut found = Vec::new();
let mut arg = None;

let cst = loop {
    match parse.resume(arg.take()) {
        MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { range }) => {
            found.clear();
            mime_simd::field_ends(input, range, &mut found);
            arg = Some(found.as_slice());
        }
        MimeCoroutineState::Yielded(MimeParseYield::WantsDashLines { range }) => {
            found.clear();
            mime_simd::dash_lines(input, range, &mut found);
            arg = Some(found.as_slice());
        }
        MimeCoroutineState::Yielded(MimeParseYield::WantsFind { range, needle }) => {
            found.clear();
            mime::tree::scan::memchr::find(input, range, needle, &mut found);
            arg = Some(found.as_slice());
        }
        MimeCoroutineState::Complete(cst) => break cst,
    }
};
```

The order of the requests follows the message: field ends of the root header block (bounded by the end of input, stopping at its empty line), then, for a multipart or a nested message, the dash lines of the whole body at once, from which the coroutine resolves every delimiter of every nested multipart against the boundaries open at that point, then field ends of each part's header block, bounded by the part's end so a delimiter cuts a block that never reached its empty line. `WantsFind` comes only when a multipart has no delimiter at the start of any line.

`tree::scan::scalar` (plain loops, no dependency, written to be obviously right) and `tree::scan::memchr` answer every request as plain functions, and `MimeCst::parse(input)` runs the coroutine over the memchr answers with the default options. The SIMD answerers are a later crate that only has to agree with the scalar ones.

### Bounds and linearity

Depth, part count and encoded nesting are bounded (64, 10,000 and 3, mail-parser's defaults and io-pimdir's `MIME_DEPTH_MAX`), set through `MimeParseOptions` public fields. The parse is iterative over an explicit stack of frames, as mail-parser's builder is, so no input recurses on the call stack while it is read; every later walk of the boxed tree (serialization, decode, clone, drop) recurses at most the depth bound, as ical's do under its own. A body's dash lines are asked for once and matched against every open boundary, so neither nesting nor a body full of near-miss lines makes the parse rescan a byte.

### The fuzz target

fuzz/fuzz_targets/parse.rs from the first commit, with ical's two oracles (never panics, serializes to a byte-stable fixpoint) plus byte identity with the input, and a MIME dictionary (`--`, `Content-Type:`, `multipart/`, `boundary=`, `message/rfc822`, `Content-Transfer-Encoding:`, `base64`, CR, LF). Beside it, fuzz/fuzz_targets/scan.rs runs every request through every answerer built in, over arbitrary bytes and ranges, against the scalar answers, and fuzz/fuzz_targets/answer.rs resumes the coroutine with arbitrary positions, checking it never panics and the tree still round-trips.
