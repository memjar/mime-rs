---
cairn: change
id: the-differential-target
status: landed
created: 2026-11-02
---

# The differential target

Two tasks of [ready-for-a-first-release](../ready-for-a-first-release/proposal.md), asked by the user on 2026-11-02 after asking whether mime-rs is as solid as mail-parser: the answer was no, as nothing compared the two beyond the vendored corpus. This change builds the differential target and the fetched-corpus sweeps, and fixes or records every disagreement they find.

## Why

The vendored corpus compares this crate with mail-parser 1.0 on 153 messages and 862 field vectors. Nothing compared them on arbitrary input or on real mail at volume, so the readings mail-parser gets right and this crate does not could only be found by a user.

## What

- One oracle, tests/common/differential.rs: an input read by this crate and by mail-parser 1.0 (its serde form, the shape of the frozen answers), compared as two part trees, then on the decoded content and decode problems of each leaf, the decoded header values and the body lists. Every deliberate difference is a named rule citing its reason, a part a rule explains being left out of the comparisons below it. A header value is compared only when this crate's validator finds its syntax sound and, for addresses, identifiers and content types, when it follows its RFC grammar: on malformed values both parsers recover by policy, and tests/vectors pins this crate's.
- The oracle runs three ways: tests/differential.rs over the vendored corpus in every line-ending form, the fuzz crate's `differential` target over arbitrary bytes, and the ignored tests/fetched.rs over the fetched corpora.
- scripts/fetch-corpora.sh and its sha256 manifest, ported from mail-parser 1.0.0 with attribution, plus the Hunny samples from the pinned mail-parser 1.0.0 crate (D12), into `$MIME_CORPORA` or target/corpora.
- Every disagreement fixed or recorded. Fixed: a line of carriage returns or form feeds before its line feed ends a header block (a message whose line endings were converted twice kept its body and parts in its header block); a four-digit year below 100 reads as a two-digit one (271 Enron messages dated `0001`); the blanks an encoded word decodes to stay in unstructured text; an encoded word's charset is an ASCII token; the validator reports an address with an empty local part or domain, and a date outside the RFC 5322 3.3 and 4.3 grammar, and no longer overflows on a long year. The campaign's edit target found two older bugs, fixed too: a part inserted beside a delimiter cut at the end of its multipart wrote its delimiter with no line ending, and a content edit in a multipart read through fallback delimiters moved its delimiter, which an enclosing multipart reusing the boundary then claimed; such an edit is now refused as damaged. The edit property test then found a third: a field removed after a line holding no field left that line its folding blank, which folded the empty line after it into the block. Recorded: the rules of the oracle, each with its reason.
