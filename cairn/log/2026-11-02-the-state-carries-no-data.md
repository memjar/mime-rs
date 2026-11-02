---
cairn: log
change: the-state-carries-no-data
landed: 2026-11-02
---

# The state carries no data

Landed a third speed pass with no behaviour change. The coroutine state is a fieldless enum, the entity in progress and the multipart being split are fields read one by one, and answers are checked as they are used; a part joins its multipart when it starts and the first child is entered at once, so the path vector and the part positions are gone; names are compared exactly first, the crate's own lookups using the canonical spellings; `MimeCst::entities` allocates one vector; the address and message identifier readers no longer grow vectors a byte at a time or copy their results twice. The source lost 29 lines of code (it gained 5 once the new comments and blank lines count).

Cycles per corpus pass against the-steps-inlined, medians of 7 interleaved runs: `structure` 428k to 370k (13.6% fewer), `headers` 524k to 454k (13.3%), depth 0 221k to 188k (14.7%), `full` 641k to 562k (12.3%), `edit` level; instructions 1,857k to 1,669k, 2,049k to 1,853k, 907k to 796k, 2,467k to 2,230k. Criterion against mail-parser 1.0, at a machine load near 3.5: `structure` 1.37x to 1.56x, `headers` 1.20x to 1.36x, depth 0 against `parse_headers` 1.06x to 1.18x, `full` 1.05x to 1.20x; `build` 1.31x and `edit` 5.43x, their code unchanged and their move the machine's. Allocations per pass: the parse 1,519 plus 32 reallocations to 1,390 plus 21, `entities()` 306 to 153, the bench's From read 429 plus 97 to 353 plus 26, its Message-ID read 51 plus 41 to 34.

The failed store forwards were found by counting and sampling the processor's own event for them (`ls_bad_status2.stli_other`) rather than read off cycle samples. A differential run against the previous parser (300 rounds of random edits of the corpus, 400,594 checks, a noisy answerer fed alike to both coroutines with their requests compared, and 400,000 random inputs to every field reader) found no difference; breaks planted in the old copy's answer check and address reader were caught.

README.md and benches/README.md carry the new numbers, what was tried and dropped, and the measuring notes. The handoff for the next pass is [the-scratch-area](../changes/the-scratch-area/proposal.md), rewritten.

Capabilities moved: none.
