---
cairn: change
id: header-fields-are-read-by-form
status: landed
created: 2026-10-31
---

# Header fields are read by form

Phase 2 of [a-mime-library](../../a-mime-library/proposal.md). It folds into new cairn/spec/header-fields.md, cairn/spec/decoded-model.md and cairn/spec/content-encodings.md.

## Why

Most of what a consumer asks of a message is in its header block: who sent it, to whom, when, about what, in reply to what. Every structured field has its own grammar, an obsolete grammar on top (RFC 5322 4), and a long tail of malformations, and the text inside phrases and unstructured fields arrives as RFC 2047 encoded words in some charset. mail-parser's field vectors (address.json, date.json, content_type.json, id.json, list.json, unstructured.json, thread_name.json) are the known answers for that tail.

RFC 8621 4.1.2 already names the forms a field can be read in (raw, text, addresses, grouped addresses, message ids, date, URLs), and lets a client ask for any field in any form. Taking those forms as the model's value arms keeps the crate aligned with the one standard that defines parsed header access, and with io-jmap.

## What

### Forms

`MimeHeaderForm` names the forms: the RFC 8621 seven plus the MIME ones (`ContentType`, `Disposition`, `TransferEncoding`) and `CommaList` (Keywords, Content-Language). Every field reads in its default form through its lens, and in any form through `MimeHeaderField::read_as`; `MimeHeaderForm::parse` reads a value standing alone, outside any message, for callers holding header bytes only (io-pimdir's `derive_meta`).

The default forms follow mail-parser's table with four changes, each a divergence row: the RFC 2369 List-* fields read as URLs (mail-parser reads them as addresses), List-Id as text (RFC 2919 phrase and id), Return-Path as an address (RFC 5322 3.6.7 `angle-addr`, possibly the empty `<>`), and Disposition-Notification-To as addresses (RFC 8098 2.1).

### The decoding half of the codecs

Encoded words need base64, the Q encoding and charset conversion, so the decode side of D2 and D3 lands here: `MimeBase64::decode` and `MimeQuotedPrintable::decode` (with a header mode for Q), the thin wrappers of D2 over the base64 and quoted_printable crates behind the `base64` and `quoted-printable` features, and `MimeCharset`, which looks a label up and transcodes through encoding_rs behind the `encoding` feature, as vcard's `charset` helper does (D3). Phase 3 reuses them on bodies; phase 4 adds the encoders.

### The decoded model

`Mime`, `MimeHeader`, `MimeHeaderName`, `MimeHeaderKind`, `MimeHeaderValue` and the value types of the umbrella proposal, pure data with no dependency on `tree`. `MimeCst::decode` projects a tree onto the model, `encode` projects it back to a canonical tree; the body side of both stays structural here (leaf bytes as written) until phase 3 decodes content.

`MimeHeaderKind` is closed over the fields RFC 5322, RFC 2045, RFC 2183, RFC 2369, RFC 2919, RFC 3282, RFC 2557, RFC 3834, RFC 8058 and RFC 8098 define, registered in the IANA permanent message header field registry, spelled canonically through `Deref` and parsed case-insensitively through `FromStr`, with `MimeHeaderKind::ALL` for the sweep.

### Markers and the spec table

One marker per kind (`DATE`, `FROM`, `SENDER`, `REPLY_TO`, `TO`, `CC`, `BCC`, `MESSAGE_ID`, `IN_REPLY_TO`, `REFERENCES`, `SUBJECT`, `COMMENTS`, `KEYWORDS`, the `RESENT_*` family, `RETURN_PATH`, `RECEIVED`, `MIME_VERSION`, `CONTENT_*`, `LIST_*`, `AUTO_SUBMITTED`, `DISPOSITION_NOTIFICATION_TO`), each carrying its `MimeHeaderSpec`: default form, minimum and maximum count at the top of a message (RFC 5322 3.6 table) and inside a MIME part, and its RFC section. The validator and builder read the same table in phases 4 and 6.

### The base subject

`MimeText::base_subject` strips reply and forward prefixes, list tags and trailing `(fwd)` markers (RFC 5256 2.1), with the localized prefixes thread_name.json and trailing_fwd.json carry.
