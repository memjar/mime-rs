---
cairn: change
id: the-steps-inlined
status: landed
created: 2026-11-02
---

# The steps inlined

The second speed pass of 2026-11-02, asked by the user after [the-tree-built-in-place](../the-tree-built-in-place/proposal.md): read the code and the handoff of [the-scratch-area](../../the-scratch-area/proposal.md), beat the previous pass, and reduce the amount of code while at it. Part of [benchmarked-against-stalwart](../../benchmarked-against-stalwart/proposal.md).

## Why

Two rows were still behind mail-parser 1.0 (`full`, and the depth 0 `headers` against `parse_headers`), and `perf annotate` showed where the cycles went that the instruction counts hid: the coroutine's steps returned their requests through memory, written by narrow stores and read back by a wide copy, and every such load waited on its stores (store-to-load forwarding failing). On `full`, the request copies in `resume` and the `Option<Delimiter>` copy in `split` were each about 4% of the cycles. Reading Content-Type was about a fifth of the `structure` instructions, the media type lexed twice for every multipart, and four readers each carried a copy of the media type lexing.

## What

No behaviour change.

- The parse steps (`start`, `entity`, `body`, `split`, `build`) and `Delimiter::at` are `#[inline(always)]`, so their requests and delimiters stay in registers.
- The root's task starts in the coroutine state (`State::Start`) instead of the task vector.
- `Kind::of` reads the type, subtype and boundary of a Content-Type value in one lexer pass, and returns a leaf at once when the type opens with a letter other than `m`.
- One crate-private `media` lexes the media type for `MimeMediaType::new`, `parse_content_type`, `raw_params` and `param_span`; `raw_param` is gone, `raw_params` starts from a lexer already past the media type.
- The fallback search no longer copies the boundary to check its answer.

The lever list of the-scratch-area was measured first (scratch allocations, the dash-line searcher, the lexer, the charset labels); what did not pay is in [benches/README.md](../../../../benches/README.md), and the-scratch-area stays the handoff, updated for the next pass.
