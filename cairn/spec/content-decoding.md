---
cairn: spec
capability: content-decoding
status: current
---

# Content decoding

What a reader shows of a message: its leaves decoded on demand, its text body, HTML body and attachments, and nested messages that arrived encoded. Everything here reads the syntax tree and decodes nothing until asked.

### Requirement: Nothing is decoded until it is read

Parsing and decoding the header block SHALL neither transfer-decode nor transcode any body. A leaf SHALL decode when one of its accessors is called, and its result SHALL borrow the input when the leaf is under an identity transfer encoding (and already valid UTF-8, for text).

#### Scenario: A message with a large attachment
- GIVEN a message with a text part and a 10 MB base64 attachment
- WHEN its Subject and its text part are read
- THEN the attachment's bytes are never visited by a decoder

### Requirement: The leaf accessors

A leaf SHALL offer `bytes` (as written), `quoted_printable` and `base64` (one per decoder, the raw bytes when the leaf is not so encoded), `decoded` (by whichever decoders the build carries), `decoded_len`, `text` (decoded, transcoded and unwrapped when flowed) and `text_prefix`, and the checked variants `decoded_checked` and `text_checked`. A multipart, a nested message or an unsplit body SHALL have none.

### Requirement: Decoding never fails, it reports

Every leaf accessor SHALL return content for any input. Its checked variant SHALL also return the problems met, from a closed set (`MimeDecodeProblems`): malformed transfer encoding, unknown transfer encoding (decoded as identity), unknown charset (decoded as lossy UTF-8), malformed data for the declared charset.

#### Scenario: A stray byte in base64
- GIVEN an `application/octet-stream` leaf under base64 holding `AAEC*AwQF`
- WHEN it is read through the checked accessor
- THEN it yields the bytes 00 01 02 03 04 05 and reports a malformed transfer encoding

### Requirement: The charset of a text part

A text leaf SHALL be transcoded from its `charset` parameter, a byte order mark it starts with taking precedence, and as UTF-8 when the parameter is absent or unknown. RFC 2045 5.2's US-ASCII default reads the same for ASCII content, and an unlabelled 8-bit part is far likelier UTF-8 than anything else.

### Requirement: A text prefix stops early

`text_prefix(n)` SHALL return at most `n` characters of the decoded text and SHALL stop transfer-decoding and transcoding once it has them, cutting the leaf after a line break, so its cost follows `n` rather than the part's size.

### Requirement: Flowed text is unwrapped

A `text/plain` leaf with `format=flowed` SHALL read through RFC 3676 (content-encodings.md), honouring `DelSp`. Without `format=flowed` the text SHALL be returned as decoded.

### Requirement: The body lists

`MimeBodyLists` SHALL compute the text body, HTML body and attachment lists of a message by the RFC 8621 4.1.4 parseStructure algorithm, over the tree and without decoding any leaf, and `has_attachment` SHALL follow RFC 8621 4.1.4's definition. A part of a `multipart/digest` with no Content-Type SHALL count as a `message/rfc822`, and a nested message SHALL be one attachment, its own lists its own. HTML is not reduced to text, so an HTML-only message has an empty text body list, as RFC 8621 4.1.4 computes it.

#### Scenario: An alternative child marked as an attachment
- GIVEN a `multipart/alternative` whose HTML child carries `Content-Disposition: attachment`
- WHEN the body lists are computed
- THEN the HTML child is an attachment and is in neither body list

#### Scenario: An inline image inside a mixed multipart
- GIVEN a `multipart/mixed` holding a text part and an `image/png` part with no disposition
- WHEN the body lists are computed
- THEN the image is in both the text and the HTML body lists and is not an attachment

### Requirement: Encoded nested messages are parsed when read

`nested` on a `message/rfc822` or `message/global` leaf under base64 or quoted-printable SHALL decode it and parse the result as an owned tree, one level per call; when the decoded content opens with no header field, it SHALL return none and the leaf stays binary. RFC 2046 5.2.1 forbids encoding a message, so the structural parse keeps such a leaf whole (a divergence row).

### Requirement: The corpus decodes as its frozen answers

Every vendored fixture's decoded leaf content, decoding problems and body lists SHALL match its frozen answer, unless the divergence file holds a row for that fixture and path with its reason.
