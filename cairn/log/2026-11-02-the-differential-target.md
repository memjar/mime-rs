---
cairn: log
change: the-differential-target
landed: 2026-11-02
---

# The differential target

Landed the differential oracle against mail-parser 1.0 and the fetched-corpus sweeps of [ready-for-a-first-release](../changes/ready-for-a-first-release/proposal.md), asked by the user after the answer to "is mime-rs as solid as mail-parser" was no: the two had only been compared on the vendored corpus.

tests/common/differential.rs reads an input through this crate and through mail-parser 1.0 (its serde form, the shape of the frozen answers) and compares the two part trees, then the decoded content and decode problems of each leaf, the decoded header values and the body lists. Each deliberate difference is one of 39 named rules stating its reason; a part a rule explains is left out of the comparisons below it, and a header value is compared only where the validator finds its syntax sound and, for addresses, identifiers and content types, where its RFC grammar holds, since on malformed values both parsers recover by policy. It runs over the vendored corpus in every line-ending form (tests/differential.rs, which also checks that it sees a moved part, a changed text and a changed list), over arbitrary bytes (the fuzz crate's `differential` target), and over the fetched corpora (the ignored tests/fetched.rs, which also checks the round trip and the answerers). scripts/fetch-corpora.sh and its manifest come from mail-parser 1.0.0 with attribution, plus the Hunny samples from the pinned mail-parser crate.

Every corpus reads alike but by rule: the 459 vendored forms, the fuzz corpora (84,427 inputs, the differential target's 17,900 among them), and the fetched corpora, Enron 517,401 messages, Apache lists 35,760, Stalwart main 242 and v1 242, Stalwart SMTP 9, mail-auth 81, Hunny 54, each also round-tripping and parsing alike through both answerers.

Fixed, each with a test failing on the old code:

- A line holding only carriage returns or form feeds before its line feed ends a header block, as mail-parser reads it: a message whose CRLF line endings were converted again (`\r\r\n`) kept its body, and every part, in its header block.
- A year of four digits or more below 100 reads as a two-digit year: 271 Enron messages are dated `0001`, which read as year 1.
- The blanks an encoded word decodes to stay in unstructured text, the raw ends alone trimmed (four Enron subjects lost an encoded space).
- An encoded word's charset is an ASCII token; a non-ASCII one made a word decoding to nothing.
- The validator reports an address with an empty local part or domain, and a date outside the RFC 5322 3.3 and 4.3 grammar (time fields of two digits, a zone's minutes below 60, a year from 1900); on the fetched corpora it flags only the Enron dates of year `0001` and `0002`. Its year parse no longer overflows on a long digit run, found by the sweep of the fuzz corpora before landing.
- A part inserted beside a delimiter cut at the end of its multipart got a delimiter with no line ending, and a content edit in a multipart read through fallback delimiters gave its delimiter a line, which an enclosing multipart reusing the boundary then claimed; such an edit is now refused as damaged. Both were older, found by the edit target in the campaign.
- A field removed after a line holding no field left that line its folding blank, which folded the empty line after it into the block, losing the body; it now goes with the field. Older too, found by the edit property test on its last run of the day.

The wider empty line first stopped `memchr::field_ends` inlining into the answer loop, 1% more instructions per corpus pass; with the rare scan out of line and an `#[inline]` hint, instructions per pass sit at or below the previous commit's: `structure` 1,669k to 1,666k, `headers` 1,853k to 1,847k, depth 0 796k to 792k, `full` 2,230k to 2,226k. Cycles stayed within the noise of a machine at load 2 to 3, where two copies of one binary measured 1.7% apart and the sign of the difference with the previous commit changed between runs.

An hour of every fuzz target on the final code but the last fixes, the differential one on four workers: parse 2.2 million executions, scan 110 million, answer 3.1 million, fields 453,000, decode 3.0 million, write 530,000 and edit 701,000, none failing; differential 1.18 million, its 29 findings over the day all recorded as rules. A first run of 16 minutes on older binaries had found the two edit bugs above.

The spec gained the empty line, the differential sweeps, the kept blanks and ASCII charsets of encoded words, the zero-padded years, the three edit rules and the validator's date and address syntax; a divergence row whose reason cited a `?` now cites the text after the base64 padding it is about. CONTRIBUTING.md and fuzz/README.md describe the oracle, the fetch script and the campaign.

Capabilities moved: parsing, header-fields, editing, conformance.
