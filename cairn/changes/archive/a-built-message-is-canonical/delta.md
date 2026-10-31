---
cairn: delta
change: a-built-message-is-canonical
---

## ADDED Requirements

### Requirement: The builder refuses illegal construction

`MimeBuilder` SHALL refuse, by returning an error, to construct what the spec forbids: a known field given a value of a form other than its own, a second instance of a field RFC 5322 3.6 allows once, a Content-Type, Content-Transfer-Encoding or MIME-Version field the builder writes itself, a Message-ID whose halves are not RFC 5322 3.6.4 `id-left` and `id-right`, and a token longer than 998 octets that no fold can split. A raw field or part SHALL be accepted as written, as the escape hatch.

#### Scenario: A date given as text
- GIVEN a builder
- WHEN the Date field is set from free text
- THEN the builder returns an error rather than the message

### Requirement: Headers are encoded where RFC 2047 allows

Non-ASCII text SHALL be written as RFC 2047 encoded words only in unstructured text, comments and phrases (RFC 2047 5), choosing B or Q by length, each word at most 75 characters and holding whole characters only, so every word decodes on its own. A phrase holding specials SHALL be written as a quoted string, escaping `"` and `\`. A parameter value that is non-ASCII or too long for one line SHALL be written by RFC 2231 (`name*0*=utf-8''...`). With UTF-8 headers allowed (RFC 6532), non-ASCII text SHALL be written as UTF-8 instead of encoded words.

#### Scenario: A display name with a comma and an accent
- GIVEN a mailbox named `Doe, Jöhn` at `john@example.com`
- WHEN it is written into a To field
- THEN the field reads back, through this crate and through mail-parser, as one mailbox with that name and address

### Requirement: Lines are folded

A written field SHALL keep every line within 78 characters where a fold point exists and within 998 octets always (RFC 5322 2.1.1), folding only before existing whitespace (RFC 5322 2.2.3). A continuation line SHALL start with whitespace and SHALL NOT be empty or whitespace-only, no line SHALL end in whitespace before a fold, and a line holding an encoded word SHALL be at most 76 characters (RFC 2047 2).

#### Scenario: A long subject from any column
- GIVEN a 300-character subject mixing ASCII and non-ASCII words
- WHEN it is written
- THEN every rule above holds, and it reads back as the subject written

### Requirement: The transfer encoding is chosen per part

A text part SHALL be written `7bit` when every byte is ASCII, no NUL or bare CR occurs and no line exceeds 998 octets; otherwise `8bit` when 8-bit bodies are allowed and the lines and bytes fit; otherwise quoted-printable or base64, whichever is shorter. A non-text leaf SHALL be base64, or `binary` when binary is allowed. A multipart or a `message/rfc822` SHALL take no encoding but `7bit` or `8bit` (RFC 2045 6.4, RFC 2046 5.2.1), and a nested message whose fields need UTF-8 SHALL be `message/global` (RFC 6532 3.7). Text line breaks SHALL be written as the chosen line ending; binary content SHALL never be altered.

### Requirement: The encoders are exact

Base64 SHALL be written in lines of at most 76 characters (RFC 2045 6.8). Quoted-printable SHALL keep every line within 76 characters, soft break included, encode `=`, every byte outside printable ASCII and whitespace that ends a line, and keep a text part's line breaks as hard breaks (RFC 2045 6.7). Both SHALL decode, through this crate's decoders, to exactly the input, at every length around the line limit.

### Requirement: Boundaries never collide

A generated boundary SHALL be 1 to 70 RFC 2046 5.1.1 `bchars`, quoted in its parameter, and SHALL NOT occur at the start of any line of the content it encloses, nested messages included; a candidate that occurs SHALL be replaced by the next. Generation SHALL use no clock and no randomness.

#### Scenario: A forwarded message built by the same builder
- GIVEN a multipart message attaching, as `message/rfc822`, another multipart message this builder produced
- WHEN the outer message is built
- THEN no outer delimiter line appears inside the attached message, and both parse back with their own parts

### Requirement: Building is deterministic

The same model and options SHALL build the same bytes, on every run and every platform. The builder SHALL add `MIME-Version: 1.0` and SHALL NOT invent a Date or a Message-ID.

### Requirement: A built message reads back as built

A message built from a model SHALL parse, and decode, to that model, with the canonical normalisations this spec states (transfer encodings chosen, line breaks written, defaults made explicit), and mail-parser 1.0 SHALL read it with the same structure and decoded values.

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
