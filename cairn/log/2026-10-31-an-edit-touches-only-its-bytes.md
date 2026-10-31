---
cairn: log
change: an-edit-touches-only-its-bytes
landed: 2026-10-31
---

# An edit touches only its bytes

Landed `tree::edit`: field edits on `MimeHeaderBlock` (`set`, `insert`, typed through the markers, `remove_all`, and the address item edits `push_address`, `set_address`, `remove_address`), and on `MimeCst` by entity index the content edits (`replace_content`, `replace_text`), the structural ones (`insert_part`, `remove_part`, `move_part`, `wrap`, `unwrap`, `attach`), `downgrade_7bit` with its `MimeDowngrade` report, `fill_required`, and sealing (`header_mut` and `entity_mut` refuse protected content, `protector`, `unwrap_protected`). Every written byte goes through the phase 4 writer, which gained a field line writer, a leaf encoder and a list of boundaries to avoid.

Decisions that moved from the plan: content and structural edits live on `MimeCst` rather than on the entity, because they must see the multipart around the entity (to give the next delimiter line its line ending, and to avoid the enclosing boundaries a written boundary must not collide with; a wrap that ignored them reused its parent's boundary); the cursor API of the plan became plain methods; removing a multipart's last part is refused (`LastPart`, RFC 2046 5.1.1), and a multipart read through fallback delimiters or holding none, or a leaf under a multipart type, is refused as `Damaged`, since a strict delimiter line an edit writes would change how the parser splits it.

The property test and the edit fuzz target (the edited tree reads back as edited and is a fixpoint) found ten classes, each fixed: a header block with no empty line swallowing new content, a field glued onto a field or delimiter line with no line ending, a trailing opaque line or a blank line holding spaces turned into an early block end or a continuation, garbage lines left alone once the last field goes, an empty part's next delimiter missing its line ending, a part's content ending in a CR losing it to an LF delimiter (now written CRLF there), a digest part's implicit `message/rfc822` lost on wrapping or unwrapping, a boundary of dashes alone that the parser never splits at, and the fallback reading of a damaged multipart changed by a strict delimiter. The property test ran 30,000 cases clean and the fuzz target five minutes clean.

The edit gate is met: changing the Subject of a message carrying a 2 MiB attachment and writing it out takes 77 µs, 5.05x mail-parser's parse and mail-builder's rebuild (390 µs), and 42 µs at structure depth 0 (9.2x), where the cost is the copy of the output.

Capabilities moved: editing (ADDED).
