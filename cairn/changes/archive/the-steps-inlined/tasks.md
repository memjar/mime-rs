---
cairn: tasks
change: the-steps-inlined
---

- [x] Cycle harness, differential run and allocation counter rebuilt from the-scratch-area; baseline of the previous commit and of mail-parser
- [x] The root's task in the coroutine state
- [x] The parse steps and `Delimiter::at` inlined into `resume`
- [x] One media type lexer for the four readers; the parse reads a Content-Type once, and not at all for a type opening with a letter other than `m`
- [x] Tried and dropped, in benches/README.md: newline walk for small bodies, lexer cursor, `memchr2` quoted strings, inlined `resume`, the split out of the state enum, `Task::digest` as a word, field-wise root move, `clean_label` byte search
- [x] Suite, clippy, bare core build, docs, fmt; differential run (300 rounds, 400,594 checks, a planted break caught)
- [x] Criterion run; README.md and benches/README.md updated
- [x] Log entry, this change archived, the-scratch-area updated as the next handoff
