---
cairn: log
change: header-fields-are-read-by-form
landed: 2026-10-31
---

# Header fields are read by form

Landed the field readers (unstructured text with RFC 2047 encoded words, addresses flat and grouped with the obsolete syntax, message ids, dates with the obsolete zones and years, RFC 2369 URLs, content types and dispositions with RFC 2231 continuations, comma lists, transfer encodings), the decoded model (`Mime`, `MimeHeader`, the closed `MimeHeaderKind` of 40 fields with its spec table, the open `MimeHeaderName` and `MimeHeaderValue`), the field markers, `MimeHeaderForm` with `FromStr`, `Deref` and `parse` for a value standing alone, and the RFC 5256 base subject. Every Stalwart field vector reads as expected or diverges on record (67 rows), and tests/headers.rs compares over 1,000 decoded fields of the vendored fixtures with mail-parser's frozen answers.

Decisions that moved from the plan: `MimeHeaderValue::Raw` is the unknown arm the delta asked for, rather than a separate `Unknown`; `MimeTransferEncoding` keeps an `Unknown` arm, so it is read with `parse` and spelled with `Deref` but has no `FromStr`; the base subject is the free function `value::subject::base_subject`; and `encode`, the model written back as bytes, landed with the builder as `Mime::to_bytes`. Three header divergence classes joined tests/corpus/divergence.tsv: Return-Path read as an address, List-Id read as text, and a lone word with no `@` read as a domain-less address where mail-parser reads a name. The tests that needed every codec but were gated on `parser` alone now gate on all three features, and one address unit test needing `encoding` was split off.

The headers gate is met at 0.90x mail-parser 1.0.

Capabilities moved: header-fields (ADDED), decoded-model (ADDED, with the phase 0 requirement), content-encodings (ADDED).
