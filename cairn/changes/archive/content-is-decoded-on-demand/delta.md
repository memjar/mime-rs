---
cairn: delta
change: content-is-decoded-on-demand
---

## ADDED Requirements

### Requirement: Nothing is decoded until it is read

Parsing and decoding the header block SHALL neither transfer-decode nor transcode any body. A leaf SHALL decode when one of its accessors is called, and its result SHALL borrow the input when the leaf is under an identity transfer encoding and already valid UTF-8 (or binary, for `bytes`).

#### Scenario: A message with a large attachment
- GIVEN a message with a text part and a 10 MB base64 attachment
- WHEN its Subject and its text part are read
- THEN the attachment's bytes are never visited by a decoder

### Requirement: Decoding never fails, it reports

Every leaf accessor SHALL return content for any input. Its checked variant SHALL also return the problems met, from a closed set: malformed transfer encoding, unknown transfer encoding (decoded as identity), unknown charset (decoded as lossy UTF-8), malformed data for the declared charset.

#### Scenario: A stray byte in base64
- GIVEN an `application/octet-stream` leaf under base64 holding `AAEC*AwQF`
- WHEN it is read through the checked accessor
- THEN it yields the bytes 00 01 02 03 04 05 and reports a malformed transfer encoding

### Requirement: The charset of a text part

With the `encoding` feature, a text leaf SHALL be transcoded from its `charset` parameter, a byte order mark it starts with taking precedence, and as UTF-8 when the parameter is absent or unknown, as vcard's `charset` helper reads a value. RFC 2045 5.2's US-ASCII default reads the same for ASCII content, and an unlabelled 8-bit part is far likelier UTF-8 than anything else.

### Requirement: A text prefix stops early

`text_prefix(n)` SHALL return at most `n` characters of the decoded text and SHALL stop transfer-decoding and transcoding once it has them, so its cost follows `n` rather than the part's size.

### Requirement: Flowed text

A `text/plain` leaf with `format=flowed` SHALL read through RFC 3676: a line ending in a space continues into the next line of the same quote depth, one leading space of a space-stuffed line is removed, the trailing space of a flowed line is removed too when `DelSp=yes`, the signature separator `-- ` is never flowed, and quote depth is kept per paragraph. Without `format=flowed` the text SHALL be returned as decoded.

#### Scenario: A flowed paragraph with DelSp
- GIVEN a flowed leaf with `DelSp=yes` holding `Hello wor \r\nld\r\n`
- WHEN its text is read
- THEN it reads `Hello world\r\n`

### Requirement: The body lists

`MimeBodyLists` SHALL compute the text body, HTML body and attachment lists of a message by the RFC 8621 4.1.4 parseStructure algorithm, over the tree and without decoding any leaf, and `has_attachment` SHALL follow RFC 8621 4.1.4's definition. Every leaf SHALL land in at least one list or be counted among the parts in none, and a nested message's lists SHALL be its own.

#### Scenario: An alternative child marked as an attachment
- GIVEN a `multipart/alternative` whose HTML child carries `Content-Disposition: attachment`
- WHEN the body lists are computed
- THEN the HTML child is an attachment and is in neither body list

#### Scenario: An inline image inside a mixed multipart
- GIVEN a `multipart/mixed` holding a text part and an `image/png` part with no disposition
- WHEN the body lists are computed
- THEN the image is in both the text and the HTML body lists and is not an attachment

### Requirement: Encoded nested messages are parsed when read

`nested` on a `message/rfc822` or `message/global` leaf under base64 or quoted-printable SHALL decode it and parse the result as an owned tree, counting toward the encoded nesting bound; past the bound, or when the decoded content has no header block, it SHALL return none and the leaf stays binary.

### Requirement: The corpus decodes as its frozen answers

Every vendored fixture's decoded leaf content, decoding problems and body lists SHALL match its frozen answer, unless the divergence file holds a row for that fixture and path with its reason.

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
