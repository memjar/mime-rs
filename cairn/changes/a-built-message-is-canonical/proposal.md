---
cairn: change
id: a-built-message-is-canonical
status: active
created: 2026-10-31
---

# A built message is canonical

Phase 4 of [a-mime-library](../a-mime-library/proposal.md). It folds into a new cairn/spec/building.md and into conformance.md.

## Why

Composing is the other half of a mail client, and the half where conformance is entirely ours: whatever a reader tolerates, a writer must not rely on. mail-builder 1.0 encodes most of the rules a writer needs, and its tests state them: folding (CRLF ends, no bare CR or LF, continuation lines starting with whitespace, no empty continuation, no trailing whitespace before a fold, the 78-character target, encoded words of at most 75 characters that each decode on their own), RFC 2231 parameters, display-name quoting, the cheaper of quoted-printable and base64, and boundaries that never collide.

Its generation of Date, Message-ID and boundaries reads the clock, the hostname and process entropy, which a no_std core cannot, and which makes output differ run to run (D10).

## What

- `MimeBuilder`, strict, over the decoded model: the message fields, a text body, an HTML body, attachments, inline resources keyed by Content-ID, and a raw escape hatch for any part or field. It lays them out as mail-builder does: one part alone; text and HTML in `multipart/alternative`; inline resources with the HTML in `multipart/related` (RFC 2387); attachments around everything in `multipart/mixed`.
- `MimeBuildOptions` public fields: line ending (CRLF by default, LF for a local store that wants it), 8-bit bodies allowed (8BITMIME transports), binary allowed (BINARYMIME), UTF-8 headers allowed (RFC 6532, SMTPUTF8 transports), flowed text for `text/plain` (RFC 3676), the legacy `name` beside `filename*` for old readers.
- Header encoders: encoded words (B or Q, whichever is shorter) in phrases, comments and unstructured text only; quoted strings for phrases holding specials; RFC 2231 for non-ASCII or long parameters; the folder.
- Body encoders: `MimeBase64::encode` and `MimeQuotedPrintable::encode` (RFC 2045 6.7 and 6.8, 76-character lines) over the same two crates and features as the decoders, the transfer encoding choice, `MimeFlowed::encode`. Built without `base64` or `quoted-printable`, the builder writes only what needs no such encoding and returns an error naming the missing feature for the rest.
- Boundaries derived from a counter and checked against the content they enclose; no randomness.
- `encode` (phase 2) completes: a built `Mime` encodes into a `MimeCst`, so the same lenses edit it and `to_bytes` writes it. Building is available without `parser`; writing bytes needs it.

The caller supplies Date and Message-ID; `MimeMessageId::new(unique, domain)` joins the two halves of RFC 5322 3.6.4 `id-left "@" id-right` and checks them.
