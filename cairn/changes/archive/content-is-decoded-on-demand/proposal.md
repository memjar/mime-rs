---
cairn: change
id: content-is-decoded-on-demand
status: landed
created: 2026-10-31
---

# Content is decoded on demand

Phase 3 of [a-mime-library](../../a-mime-library/proposal.md). It folds into a new cairn/spec/content-decoding.md and extends decoded-model.md.

## Why

What a reader shows is a message's text body, its HTML body and its attachments, decoded. Which parts those are is not obvious from the tree: RFC 8621 4.1.4 gives the one specified algorithm (parseStructure), and mail-parser's regression catalogue (GitHub issues 67, 70, 73, 107) records how it lands on real layouts: an alternative child with an attachment disposition, a named part inside an alternative, inline images in a mixed or a related multipart, a text part that lands in no list.

Decoding must stay lazy: a reader listing a mailbox never pays for attachments, and a search indexer reads a text prefix, not a whole body. mail-parser's lazy accessors (`decoded`, `text`, `text_prefix`, `decoded_len`, the checked variants) are the model.

Neither Stalwart crate reads `format=flowed` (RFC 3676), which most text clients still send and which a terminal reader and a reply quoter both need.

## What

- Leaf accessors on the tree and the model, named and layered as vcard's value cursor: `bytes` (as written, still transfer-encoded), `quoted_printable` and `base64` (one per feature, the raw bytes when the leaf is not so encoded), `charset` (transfer-decoded by whichever features are on, then transcoded, behind `encoding`), plus the mail-only `decoded_len` (computed without decoding where the encoding allows), `text_prefix` (stops decoding once it has enough characters), and checked variants returning `MimeDecodeProblems` beside the result.
- `MimeFlowed`: RFC 3676 decoding, applied by `text` when the part says `format=flowed`, with `DelSp` honoured and quote depth kept per paragraph.
- `MimeBodyLists`: the RFC 8621 4.1.4 text body, HTML body and attachments of a message, plus `has_attachment`, in document order, computed on the tree without decoding.
- Encoded nested messages: `nested` on a `message/rfc822` leaf under base64 or quoted-printable decodes it and parses the result as an owned tree, within the encoded nesting bound of phase 1.
- `decode` now fills leaf content in the model: text parts as text, others as bytes, problems attached.

The text and HTML conversions mail-parser performs for a part with no alternative wait for the html change (D8), so here an HTML-only message has an empty text body list, exactly as RFC 8621 4.1.4 computes it.
