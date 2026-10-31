---
cairn: change
id: a-mime-library
status: active
created: 2026-10-31
---

# A MIME message library

The roadmap for a Pimalaya library that parses, validates, edits and builds Internet messages (RFC 5322 and MIME), on the model of ical-rs and vcard-rs, with Stalwart's mail-parser and mail-builder as the knowledge source, the test oracle and the benchmark to beat. Each phase below is its own change folder in this cairn; this one holds the architecture, the decisions to settle, and the order.

## Why

Seven Pimalaya consumers read messages through mail-parser 0.11 (himalaya, himalaya-tui, linux, android, unfog, neverest, io-pimdir) and two write them through mail-builder (himalaya on 1.0, linux on 0.4). That leaves four gaps.

Nothing edits a message. mail-parser is read-only and mail-builder builds from scratch, so changing a draft's Subject, removing Bcc before submission, adding an attachment, or re-encoding an 8bit part for a server without 8BITMIME means parse, rebuild, and lose every byte the rebuild normalises. Only a byte-faithful tree edits one field and leaves the rest alone, and the same exact bytes are what a signed part (RFC 1847) and a DKIM verifier (RFC 6376) need.

Nothing validates. Neither crate says whether a message conforms: the RFC 5322 3.6 field counts, the RFC 2045 6.4 encodings a composite type may take, the RFC 2046 5.1.1 boundary rules, or which transport extensions (8BITMIME, BINARYMIME, SMTPUTF8) a message needs.

The no_std core cannot use them. io-pimdir derives its mail summaries in its bare core, so src/summary/mail.rs hand-rolls a second, partial MIME reader (RFC 2047, quoted-printable, base64, addresses, message ids, dates, boundaries, attachment detection). mail-parser 1.0 needs std for its CPU feature detection and Rust 1.98.

The shape is already proven twice. ical-rs and vcard-rs each ship a dependency-free decoded model, a byte-faithful syntax tree behind `parser`, per-field lenses that edit one line and keep every other byte, a spec layer read by the decoder, the validator and the builder, and a swept real-world corpus. What took those two crates longest, collecting real-world malformations and their right readings, Stalwart has already done for mail: 201 test messages, 183 of them with frozen expected answers in both line endings, field-level known-answer vectors, a regression catalogue from an adversarial review, and the fastest Rust parser to measure against.

## What

A new repository, pimalaya/mime, holding the crate mime-rs (lib `mime`, item prefix `Mime`, see D1): "MIME message parser, validator, editor and builder library". The local checkout is ~/code/pimalaya/mime, where this cairn already lives.

### Architecture

The layering is ical's and vcard's, one-to-one.

The decoded model is always available and pulls no crate. A message is a `Mime`: an ordered list of `MimeHeader` and a body tree, `MimeBody` being a leaf (decoded content), a multipart (its parts) or a nested message. A header is a `MimeHeaderName` (a closed `MimeHeaderKind` drawn from the IANA permanent message header field registry, or a verbatim unknown name) and a `MimeHeaderValue`, an open payload enum with one arm per RFC 8621 4.1.2 parsed form (raw, text, addresses, grouped addresses, message ids, date, URLs) plus the MIME forms (content type, disposition, transfer encoding, comma list) and an `Unknown` arm keeping the raw value. The value types are bespoke: `MimeMailbox`, `MimeGroup`, `MimeAddressList`, `MimeDateTime`, `MimeMessageId`, `MimeMediaType`, `MimeContentType`, `MimeParam`, `MimeDisposition`, `MimeTransferEncoding`, `MimeText`.

The syntax tree (`tree`, behind the default `parser` feature, via memchr) is everything byte-faithful. Its hub is `MimeCst<'a>`, a tree of plain structs with public fields whose every leaf is a `Cow` borrowed from the input, as `VcardLeaf` and `IcalCst` are: a parsed leaf points into the input, and only a leaf an edit or a build replaces becomes owned. An entity (`MimeEntity`) is a header block and a body. A header field is a name leaf, the blanks and the colon, a raw value leaf that still holds its folds, and its line ending. A multipart body is a preamble, its parts (each its delimiter line as written, transport padding and line ending included, then its entity), a close delimiter and an epilogue. A nested message is a boxed entity. A leaf body is its raw, still-encoded bytes. A line in a header block that is neither a field nor a continuation is an opaque item. Every input byte belongs to exactly one leaf, so serialization writes the leaves in order.

Edits are local by construction. Setting a field replaces that field's value leaf; replacing a part's content replaces that part's body leaf and its Content-Transfer-Encoding value; inserting or removing a part changes one multipart's part list. Nothing else is copied, re-parsed or re-encoded, and every untouched leaf stays borrowed, so editing one field of a 20 MB message costs the size of that field. `to_bytes` writes every leaf once, and `chunks` hands them out in order as borrowed slices, for a caller writing straight to a socket or a file.

The parser does not own its byte searches. Parsing is an I/O-free coroutine, `MimeParse`, shaped as io-imap's: `resume` yields what it needs found, and the caller answers with any code it likes before resuming, whether memchr, the SIMD crate, offsets a store cached at import, a thread pool for a large mbox, or a counter while profiling. A request is a bulk job, never one search:

- `WantsFieldEnds` asks for every field end of one header block (each `\n` whose next line starts with neither SP nor HTAB), stopping at the empty line that ends the block or at a bound;
- `WantsDashLines` asks for every line starting with `--` across a whole body, and the coroutine resolves every delimiter of every nested multipart from that one answer;
- `WantsFind` asks for one needle in a range, for the rare mid-line fallback.

A message costs one round trip per header block plus one per body, so the hand-off is paid per entity rather than per field, and every job is long enough for SIMD to pay off. A request carries offsets into the input the caller already holds, and the caller resumes with the positions in a buffer of its own, reused from one request to the next. `tree::scan` ships the reference answerers as plain functions, one scalar with no dependency and one over memchr (which the `parser` feature already pulls), and `MimeCst::parse(input)` runs the coroutine with the memchr answers, as io-imap's client layer runs its coroutines over std sockets.

Lenses read and edit one field. A marker spelled as its wire token (`SUBJECT`, `FROM`, `MESSAGE_ID`, `CONTENT_TYPE`) keys `entity.header::<SUBJECT>()`, ties the field to its value type, its edit cursor and its decode projection, and carries its `MimeHeaderSpec`. Hyphenated names become underscores, the deviation vcard documents for `SORT_AS`.

The spec layer states what the RFCs allow. `MimeHeaderSpec` gives a field its default form, its cardinality at the top of a message and inside a MIME part, whether it is required, and its RFC section. `MimeMediaSpec` gives a media type the transfer encodings and the parameters it requires. One dispatch bridges the open kinds back to the static specs, and the decoder, the validator and the builder all read it.

Content decoding runs on demand only: transfer decoding, character sets, RFC 2047 encoded words, RFC 2231 parameters, RFC 3676 flowed text, and the RFC 8621 4.1.4 body lists (text body, HTML body, attachments).

The builder encodes the model into a canonical tree, choosing transfer encodings, encoding headers and folding lines. The validator checks a decoded message against its RFCs and mints a `MimeValid<T>` proof.

### Performance shape

The tree does more bookkeeping than mail-parser's store, so the design spends nothing the parse does not need:

- Only structure is read at parse time: the header block is split from one `WantsFieldEnds` answer, and only Content-Type and Content-Transfer-Encoding are parsed per entity, to find boundaries and nested messages. Every other value stays an unparsed leaf and is parsed when read. mail-parser parses its configured header forms eagerly, so the structure workload is where the lazy tree should win.
- Folds are not recorded apart: the raw value leaf holds them and unfolding happens on read, so an untouched field costs one borrowed leaf and an edit simply re-folds.
- Every delimiter of a message comes from one `WantsDashLines` answer over its body, matched by the coroutine against the boundaries still open, so a nested multipart never rescans a byte.
- A parse allocates one item vector per header block and one part vector per multipart, never one per line or per byte, since the leaves borrow. mail-parser's flat arena of input offsets is cheaper and reusable across messages (`MessageBuffers`), and cannot hold an edited leaf: the trade is deliberate. If the structure gate misses, a compact leaf (an input span or an owned buffer behind the same API) is the first lever.
- Nothing is transfer-decoded or transcoded until read, and decoded text is a `Cow`, borrowed when the bytes are already UTF-8 under an identity encoding.
- SIMD comes from whatever answers the search requests (memchr by default, the SIMD crate per D4) and through the codec features from whichever crate backs them. The library itself stays no_std and free of `unsafe`.

## Decisions to settle

Settled on 2026-10-31: the user asked for the whole plan to go ahead autonomously, so every recommendation below is taken as written.

- **D1, the name.** Recommended: crate mime-rs, lib `mime`, prefix `Mime`, repo pimalaya/mime, following ical-rs and vcard-rs, which both reuse a lib name an older crate holds. On 2026-10-31 crates.io had mime-rs, mail-rs and eml-rs all free. The cost: lib `mime` shadows hyperium's `mime` (716 million downloads, a transitive dependency of most HTTP stacks), so a crate depending on both directly must rename one with `package =`. No Pimalaya crate depends on hyperium's `mime` directly (it is only transitive in himalaya's lockfile). The alternative mail-rs (`mail`, prefix `Mail`) avoids that and reads badly (`MailMailbox`); eml-rs (`Eml`) names a file extension rather than the format.
- **D2, transfer codecs behind features.** Recommended: as in vcard and ical, a `base64` feature (base64 crate) and a `quoted-printable` feature (quoted_printable crate), both default, the core keeping encoded content raw without them: an encoded word stays literal in the text form, and the body decoders are not built. Each costs a thin wrapper, because the crates are stricter than mail. base64's decoder rejects line breaks and every byte outside the alphabet (`DecodeError::InvalidByte`), so a line-wrapped MIME body decodes line by line through `decode_slice` (a well-formed 76-character line is a whole number of quanta) and falls back to a filtering pass on a damaged line, which is also where skipped bytes are reported. quoted_printable's `ParseMode::Robust` tolerates without saying what it tolerated and has no RFC 2047 Q mode, so a header word turns `_` into a space first and the wrapper scans for the problems it reports. Encoding needs only base64 chunked at 57 input bytes per line and quoted_printable's `encode_with_options`. Writing both codecs in-house would buy nothing: a scalar decoder of ours is no faster than the crate's.
- **D3, character sets as in vcard and ical.** Recommended: the core transcodes nothing. The default `encoding` feature transcodes through `encoding_rs::Encoding::for_label`, falling back to UTF-8 when the label is absent or unknown, and runs after whichever transfer decoding features are on, exactly as `VcardValueCursor::charset` does. encoding_rs already reads ISO-8859-1 and US-ASCII as windows-1252 and decodes UTF-16LE and UTF-16BE, so the one charset lost against mail-parser is UTF-7 (RFC 2152), which the WHATWG standard leaves out: it reads as UTF-8, and mail-parser's utf7-terminators fixture gets a divergence row. Without `encoding`, an encoded word or a text part in any charset but UTF-8 and US-ASCII stays as written rather than being guessed.
- **D4, the search is a coroutine.** Recommended: `MimeParse` as the architecture describes it, with mail-parser's SIMD kernels in a second crate of this repository (working name mime-simd) answering its requests. mime-rs then stays no_std and free of `unsafe` (it can `forbid(unsafe_code)`), and the SIMD crate is free to use std's runtime CPU detection, its no_std build falling back to the SSE2 and NEON baselines, or to AVX2 when built for it. Consumers on std (himalaya, neverest, linux) answer with it; io-pimdir's core keeps memchr. The alternative, a `simd` feature inside mime-rs, puts the `unsafe` in the main crate.
  - The requests come from what mail-parser's parser searches for: its `Kernel` methods `field_end` (a `\n` not followed by SP or HTAB, found with a second load shifted by one byte so a fold costs no call) and `dash_line` (a `\n` followed by `--`), plus memmem for the mid-line fallback. mail-parser calls them once per field and once per delimiter; the coroutine asks for each as one job over a whole header block or body, which is the shape SIMD runs best, with no setup per call. mail-parser's `Kernel` already switches backends, but over a closed enum no caller can extend; the coroutine opens that seam to any code.
  - The kernels to answer with are mail-parser's src/scan, 1,848 lines (324 of them tests, about 110 `unsafe` sites, each with a SAFETY comment), in NEON, SSE2, SSSE3 and AVX2. Its Apache-2.0 OR MIT licence allows the copy with the copyright notice kept, and its benches/kernels.rs times them against memchr on its own corpora, so they are taken only if phase 0 shows them ahead. In the SIMD crate they keep mail-parser's guards: `unsafe_op_in_unsafe_fn`, `clippy::undocumented_unsafe_blocks`, a fuzz target comparing every answer with the scalar one, and Miri in CI.
  - An answer is checked, never trusted: every position must lie in the request's range, increase, and sit on the bytes its request names, and one that does not is dropped and reported. A wrong answerer can then misplace structure but never break round-trip fidelity, since the leaves still partition the input.
  - Transfer decoding stays on the vcard-style features (D2), with encodify as their backend once it builds without std (today it uses std for CPU detection and `io::Write` streaming), since its base64 and quoted-printable kernels (about 5,000 lines) are too large to copy. Should the `full` gate still miss, the same coroutine shape could carry base64 decoding; that would part from vcard's layering and is not proposed now.
- **D5, parsing never fails.** Recommended: `MimeCst::parse` accepts any bytes and returns a tree. mail-parser returns `None` for input with no header field, but a store must keep and round-trip every message it was handed, so such input is an entity with no header block. Every recovery is recorded as an anomaly on its entity (mail-parser's part flags: missing close delimiter, fallback delimiter, no blank line, unterminated, nesting limit, part limit) and listed for the caller. ical's strict and recovering pair is not needed: mail has no input a reader should refuse.
- **D6, lazy fields.** Recommended: the tree parses only the structural fields eagerly and every other value on access; `decode` builds the model, which is the cache. Reading the same field twice from the tree parses it twice.
- **D7, signed content is sealed.** Recommended: an edit that would change a byte inside the signed first part of a `multipart/signed`, or inside a `multipart/encrypted`, fails with an error naming the protecting entity, unless the caller first removes that layer explicitly. RFC 1847 2.1 and RFC 3156 5 verify the signed bytes as transmitted.
- **D8, HTML.** Recommended: no HTML in the first release. io-pimdir's search extraction and every preview need HTML reduced to text (today `mail_parser::decoders::html::html_to_text`), so an `html` feature follows as its own change, reusing mail-parser's html_to_text, text_to_html and entity vectors.
- **D9, JMAP.** Recommended: decide after phase 3. A `jmap` feature converting the model to and from the RFC 8621 Email object (bodyStructure, bodyValues, the header forms), over `serde_json::Value` as jcard and jscalendar do, would serve io-jmap and himalaya's JMAP backend, and would duplicate io-jmap's own Email types.
- **D10, deterministic output.** Recommended: the no_std builder reads no clock, hostname or randomness. The caller supplies Date and Message-ID (with a `MimeMessageId` helper joining a unique part and a domain), and boundaries derive from a counter checked against the content, so the same model always builds the same bytes. mail-builder generates all three from std.
- **D11, toolchain.** Recommended: `rust-version = "1.88"` for the library, as its siblings. The mail-parser 1.0 dev-dependency needs rustc 1.98 (due 2026-08-20 on the six-week train), while vcard's flake pins fenix monthly at 2026-07-01 (rustc 1.96), so this repository's flake locks a later fenix, and the MSRV job builds the library alone.
- **D12, the Hunny samples.** Recommended: fetch, do not vendor. mail-parser's legacy fixtures carry "Copyright 2010 Hunny Software" and no licence, unlike every other source here, so they come down from a pinned mail-parser tarball at test time, behind an ignored test.
- **D13, lazy structure.** Recommended: let a parse stop at the header block. `MimeParseOptions` gains a structure depth; at 0 an entity's body stays one unparsed leaf, and expanding an entity runs the same coroutine from that entity's body, through `&mut`, so the tree stays `Send` and `Sync` with no cell. Reading a message's headers, or editing its Subject, then costs the size of its header block whatever its attachments weigh, while mail-parser walks every body for boundaries on every parse unless the caller gives up the structure entirely (`parse_headers`). It is the largest gain on offer for `headers` and `edit`, and needs no `unsafe`.

## Phases

| Phase | Change | Delivers | Exit gate |
| --- | --- | --- | --- |
| 0 | [a-repo-for-mime-messages](../archive/a-repo-for-mime-messages/proposal.md) | Repository scaffold, vendored corpus with attribution, fetch script, competitor baseline numbers | Default, bare and all-features builds pass; cargo deny passes; baseline tables recorded |
| 1 | [a-byte-faithful-message-tree](../archive/a-byte-faithful-message-tree/proposal.md) | `MimeCst`: header blocks, multiparts, nested messages, anomalies, bounds, the `parse` fuzz target | Every fixture round-trips byte for byte in LF and CRLF; structure agrees with the frozen answers or a recorded divergence; structure gate met |
| 2 | [header-fields-are-read-by-form](../header-fields-are-read-by-form/proposal.md) | Field grammars, the decoding side of the transfer codecs and charsets, RFC 2047 and RFC 2231, the decoded model, header kinds, markers, the spec table, `decode` and `encode` | Field vectors pass; decoded headers agree with the frozen answers; headers gate met |
| 3 | [content-is-decoded-on-demand](../content-is-decoded-on-demand/proposal.md) | Body decoding, RFC 3676 flowed text, RFC 8621 body lists, text accessors, encoded nested messages | Decoded bodies and body lists agree with the frozen answers; full gate met |
| 4 | [a-built-message-is-canonical](../a-built-message-is-canonical/proposal.md) | Builder and encoder: header encoding, folding, transfer encoding choice, boundaries | Built bytes reparse to their model and read the same through mail-parser; build gate met |
| 5 | [an-edit-touches-only-its-bytes](../an-edit-touches-only-its-bytes/proposal.md) | Lenses and cursors, structural edits, re-encoding, downgrade, sealing | Edit property tests hold; edit gate met |
| 6 | [validation-states-what-a-message-breaks](../validation-states-what-a-message-breaks/proposal.md) | Validator, `MimeValid`, transport requirements | Every rule has a failing and a passing case; corpus classified with asserted counts |
| 7 | [ready-for-a-first-release](../ready-for-a-first-release/proposal.md) | Fuzz campaign, coverage, docs, examples, README, CHANGELOG | Guidelines conformance table all PASS or N/A |

[benchmarked-against-stalwart](../benchmarked-against-stalwart/proposal.md) runs alongside every phase: it builds the harness in phase 0 and owns the gates each phase must meet.

Builder comes before editing because an edit re-encodes what it writes with the builder's encoders. Validation comes last because it reads the spec layer phases 2 to 4 fill.

## Later, each its own change once asked for

- `html`: HTML reduced to text and text to HTML (D8).
- `jmap`: the RFC 8621 Email representation (D9).
- `Received` read as structured trace fields (RFC 5321 4.4); kept raw until a consumer needs it, mail-parser's received.json vectors waiting.
- A `cli` feature with `parse`, `validate`, `build` and `edit` verbs, mirroring vcard's a-cli-behind-a-feature once that one settles its open questions.

## Consumers

Each migration is a change in the consumer's own cairn, opened after the first release, mail-parser and mail-builder leaving each tree once its last use is gone.

1. io-pimdir first, being the one consumer forced into a hand-rolled reader: src/summary/mail.rs (encoded words, addresses, message ids, dates, boundaries, attachment and invitation detection) and src/search/extract.rs, in the bare core.
2. himalaya: reading, header display, attachment list and download, reply and forward building (mail-builder today), submission (removing Bcc becomes an edit), and the IMAP, Maildir, mbox and m2dir backends.
3. himalaya-tui, android/rust (read, compose), linux (compose, quote, view, SMTP, sync), unfog (capture) and neverest.

io-smtp may take the transport requirements from the validator to decide between sending as is, downgrading, and refusing.

## Out of scope

- Mailbox formats: io-mbox, io-maildir and io-m2dir own mbox, Maildir and m2dir.
- Three-way merge: a sent message is immutable (neither IMAP nor JMAP edits one in place), and a draft edited on two devices is replaced, not merged.
- Threading: io-pimdir owns threads; this crate gives the base subject (RFC 5256 2.1) and the reference chain.
- Cryptography: no S/MIME, PGP/MIME, DKIM or ARC. The crate exposes the exact bytes a verifier needs and never verifies.
- Transport: dot-stuffing and the envelope belong to io-smtp.
- HTML rendering and sanitising.

## Risks

- Speed against bookkeeping. A byte-faithful tree is more work than a read-only index, and mail-parser 1.0 is SIMD-tuned throughout. Mitigation: the borrowed-leaf, lazy-field design above, the baseline measured before any code, and a gate per phase, so a miss is profiled before the next feature lands.
- Disagreement with mail-parser on malformed input. Some of its readings are choices, not RFC answers. Every disagreement is a row in a divergence file with its reason, reviewed by a human, never an edited expected answer.
- Scope. HTML, JMAP, Received and the CLI are listed above so they do not leak into phases 1 to 7.
- Toolchain. The dev-dependency's MSRV and the fenix hash trap on Rust releases (see D11).
