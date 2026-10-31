---
cairn: change
id: an-edit-touches-only-its-bytes
status: landed
created: 2026-10-31
---

# An edit touches only its bytes

Phase 5 of [a-mime-library](../../a-mime-library/proposal.md). It folds into a new cairn/spec/editing.md.

## Why

This is the capability neither Stalwart crate has, and the one the consumers keep reimplementing with a parse and a rebuild:

- a draft whose Subject or recipients change, saved back to the store;
- a message submitted over SMTP with its Bcc field removed (RFC 5322 3.6.3) and nothing else touched;
- an attachment added to a single-part draft, which turns its root into a `multipart/mixed`;
- an 8bit message sent to a server without 8BITMIME, re-encoded part by part (RFC 6152);
- a missing Date or Message-ID filled before submission.

Each rebuild today loses folds, field order, quoting and every byte the builder normalises, and breaks any signature over the parts it re-encodes. vcard's editing layer settled the contract: a setter that names a component rewrites that component alone. The tree of phase 1 makes it cheap as well as faithful: an edit swaps the `Cow` leaves it writes and leaves every other leaf borrowed, so it never copies the message it edits.

## What

- Field edits through the lenses: `entity.header_mut::<SUBJECT>()` and its cursor set, insert, remove; address-list item edits that keep every other item's bytes; untyped `MimeHeaderField` edits for any name.
- Placement: a new field goes after the last field of its name, or at the end of the block before its empty line, except a trace field (Received, Return-Path), which goes first (RFC 5321 4.4). A new or edited field takes the block's own line ending.
- Leaf edits: replace content, re-encoded by the builder's rules or under a given transfer encoding, the Content-Transfer-Encoding field rewritten in place or added.
- Structural edits: insert, remove and move a part in a multipart; wrap an entity into a new multipart (the message fields stay on the root, the content fields move to the new child); unwrap a multipart of one part; regenerate a boundary only when inserted content holds it, rewriting that multipart's delimiter lines and parameter alone.
- `MimeCst::downgrade_7bit`: every 8bit or binary leaf re-encoded as quoted-printable or base64, recursing into nested messages; a field carrying UTF-8 (RFC 6532), which no re-encoding fixes losslessly, is reported rather than rewritten.
- `MimeCst::fill_required(date, message_id)`: adds the missing ones, idempotent.
- Sealing (D7): edits under a `multipart/signed` or `multipart/encrypted` fail with `MimeEditError::Sealed` naming the protecting entity, unless the caller first removes that layer with `unwrap_protected`.
