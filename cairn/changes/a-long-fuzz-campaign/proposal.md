---
cairn: change
id: a-long-fuzz-campaign
status: active
created: 2026-11-04
---

# A long fuzz campaign

The 24-hour fuzz runs of [ready-for-a-first-release](../archive/ready-for-a-first-release/proposal.md), moved out of it on 2026-11-04 by the user's call so 0.1.0 could ship without them.

## Why

Every target has had about an hour (3 to 7 minutes each on 2026-10-31, then an hour each on 2026-11-02 in [the-differential-target](../archive/the-differential-target/proposal.md)). An hour finds the shallow crashes; the deep ones in the edit layer and the differential oracle need the long runs mail-parser 1.0 had before its release.

## What

- The parse, scan, answer, fields, decode, write, edit and differential targets each run for at least 24 hours, seeded from the vendored corpus by copy (never pointed at tests/corpus, which libFuzzer would write into), scan over every answerer including the SIMD crate.
- Each crash minimised into a regression fixture with its test, fixed in a patch release.
- Run on an idle core, one target per core, so the campaign does not slow other sessions' measurements.
