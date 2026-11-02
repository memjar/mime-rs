---
cairn: log
change: the-steps-inlined
landed: 2026-11-02
---

# The steps inlined

Landed a second speed pass with no behaviour change. The parse steps and `Delimiter::at` are inlined into `resume`, so a request no longer goes through memory (its narrow stores read back whole had stalled the loads); the root's task starts in the coroutine state, so a single-part message allocates no task vector; the parse reads a Content-Type value once, type, subtype and boundary in one lexer pass, and returns a leaf without lexing when the type opens with a letter other than `m`; one `media` function lexes the media type for the four readers that each had a copy. The source lost 11 lines of code (3 once the new comments count).

Cycles per corpus pass against the-tree-built-in-place, medians of 7 interleaved runs: `structure` 485k to 431k (11% fewer), `headers` 571k to 522k (9%), depth 0 226k to 220k (3%), `full` 677k to 625k (8%); instructions 2,023k to 1,857k, 2,221k to 2,049k, 956k to 907k, 2,618k to 2,467k. Criterion against mail-parser 1.0: `structure` 1.21x to 1.37x, `headers` 1.07x to 1.20x, depth 0 against `parse_headers` 0.97x to 1.06x, `full` 0.98x to 1.05x; `build` and `edit` unchanged (1.37x, 5.24x). A differential run against the previous parser (300 rounds of random edits of the corpus, the mutation alphabet widened with case variants, comments and tabs) found no difference.

README.md and benches/README.md carry the new numbers, what was tried and dropped, and the measuring rule that two builds are compared by interleaved medians. The handoff for the next pass is [the-scratch-area](../changes/the-scratch-area/proposal.md), updated.

Capabilities moved: none.
