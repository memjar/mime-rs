---
cairn: tasks
change: a-mime-library
---

- [x] Settle D1 to D13 in the proposal, recording each answer there before phase 0 starts (taken as recommended on 2026-10-31; quoted-printable moved in-house, see the phase 0 log)
- [x] Rename this cairn's folder and every mention of mime-rs if D1 picks another name (D1 kept mime-rs)
- [x] Phase 0: land a-repo-for-mime-messages
- [ ] Phase 0 to 7: keep benchmarked-against-stalwart current, its gate checked at the end of each phase (phase 4 missed its gate, see its log)
- [x] Phase 1: land a-byte-faithful-message-tree
- [x] Phase 2: land header-fields-are-read-by-form
- [x] Phase 3: land content-is-decoded-on-demand
- [ ] Settle D9 (JMAP) now that the model exists
- [x] Phase 4: land a-built-message-is-canonical
- [x] Phase 5: land an-edit-touches-only-its-bytes
- [x] Phase 6: land validation-states-what-a-message-breaks
- [x] Phase 7: land ready-for-a-first-release (without the 24-hour fuzz runs, now a-long-fuzz-campaign)
- [ ] Open the io-pimdir migration change in io-pimdir's cairn
- [ ] Open the himalaya migration change in himalaya's cairn
- [ ] Open the remaining migration changes (himalaya-tui, android, linux, unfog, neverest)
- [ ] Open the html change (D8)
