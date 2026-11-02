---
cairn: change
id: the-state-carries-no-data
status: landed
created: 2026-11-02
---

# The state carries no data

The third speed pass of 2026-11-02, asked by the user after [the-steps-inlined](../the-steps-inlined/proposal.md): read the code and the handoff of [the-scratch-area](../../the-scratch-area/proposal.md), beat the previous pass, and reduce the amount of code while at it. Part of [benchmarked-against-stalwart](../../benchmarked-against-stalwart/proposal.md).

## Why

The previous pass left two store-forwarding stalls in `resume` that it could not remove: each try moved the stall to the next copy. This pass counted the failed forwards with the processor's own event (`ls_bad_status2.stli_other`, about 7,700 per `structure` pass, 15 per entity) and sampled on it, which named every site instead of guessing from cycle samples: the padding of `Task::digest` copied through the state enum by two overlapping 4-byte loads, the first part's task read back whole right after `build` pushed it, the dash lines copied right after they were checked, the result vector of `entities` copied out, and in the header readers vectors grown one byte at a time, four reallocations per message identifier. Cycle samples named the rest: the ASCII case-insensitive name compares, a fifth of `Kind::of` and as much again in the readers' own `content_type` lookups and parameter scans.

## What

No behaviour change.

- The coroutine state is a fieldless enum: the entity in progress and the multipart being split are fields read one by one, never moved whole.
- An answer is checked as it is used, every position still checked and a failure still reported as before, so `checked` and its copy are gone and the dash lines are kept in place.
- A part joins its multipart when its task starts, the first child entered at once rather than pushed and popped; the entity being parsed is reached through the last part of each multipart on the way, so the path vector and the part positions are gone.
- A field, media type or parameter name is compared exactly first, the crate's own lookups using the canonical spellings (`Content-Type`), then ASCII case-insensitively: one crate-private `eq_ignore_case`.
- `MimeCst::entities` walks the tree recursively into one vector, as the serializer does.
- The address reader copies runs of plain bytes at once, keeps its address buffer as the string, and hands mailboxes to a closure instead of a one-element vector; the message identifier reader copies once and filters in place.

What did not pay is in [benches/README.md](../../../../benches/README.md), and the-scratch-area stays the handoff, rewritten for the next pass.
