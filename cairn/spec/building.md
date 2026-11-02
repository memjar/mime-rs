---
cairn: spec
capability: building
status: current
---

# Building

Writing messages: `MimeBuilder` composes a model strictly, and `Mime::to_bytes` writes any model as canonical bytes under `MimeBuildOptions`. Neither needs the parser. Whatever a reader tolerates, the writer relies on none of it: every message it writes reads back, through this crate and through mail-parser 1.0, as what it states.

### Requirement: The builder refuses illegal construction

`MimeBuilder` SHALL refuse, by returning a `MimeBuildError`, a known field given a value of a form other than its own (addresses taking either address form), a second instance of a field RFC 5322 3.6 allows once, a Content-Type, Content-Transfer-Encoding or MIME-Version field (which the writer states itself), a message identifier that is not RFC 5322 3.6.4 `id-left "@" id-right`, an inline resource with no Content-ID, a raw field name holding a byte RFC 5322 2.2 does not allow or starting with `--`, a raw Content-Type, Content-Transfer-Encoding or MIME-Version, and a raw value holding a line break no blank follows. A raw field or part SHALL otherwise be accepted as written, as the escape hatch. Typed fields SHALL be set through the field markers (`builder.header::<SUBJECT>(...)`).

#### Scenario: A date given as text
- GIVEN a builder
- WHEN the Date field is set from free text
- THEN the builder returns an error rather than the message

### Requirement: The builder's layout

The builder SHALL lay a message out as mail-builder does: one part alone; text and HTML in `multipart/alternative`; inline resources after the HTML in a `multipart/related` with `type="text/html"` (RFC 2387); attachments, and inline resources when there is no HTML to reference them, after everything in `multipart/mixed`; an empty `text/plain` when nothing was given. Text parts SHALL be `charset=utf-8`; attachments and inline resources SHALL keep their content unchanged, with a disposition, a `filename` when given and a Content-ID when given.

### Requirement: The options

`MimeBuildOptions` SHALL state the line ending written (CRLF by default, LF for a local store), whether text may be `8bit` (8BITMIME), whether a leaf may be `binary` (BINARYMIME), whether fields may carry UTF-8 instead of encoded words (RFC 6532), whether `text/plain` with no `format` is written flowed (RFC 3676), whether a part with a non-ASCII `filename` also gets an encoded `name` on its Content-Type for old readers, encoded words in a parameter that RFC 2047 5 forbids and the validator reports, and the CPU token bodies are encoded under, the installed one when none is given.

### Requirement: Headers are encoded where RFC 2047 allows

Text that is non-ASCII, holds a control character or holds something a reader would take for an encoded word SHALL be written as RFC 2047 encoded words, only in unstructured text, comments and phrases (RFC 2047 5), in UTF-8, B or Q whichever is shorter, each word at most 75 characters and holding whole characters, the first sized to what is left of its line, so every word decodes on its own. A run of such words SHALL be encoded together with the spaces between them. A phrase SHALL be written as atoms when every word is one, else as a quoted string escaping `"` and `\` when it is printable ASCII, else as encoded words; a name with no address SHALL never be bare atoms. A parameter value SHALL be a token when it is one, a quoted string when it is printable ASCII of at most 60 characters, and otherwise go by RFC 2231 (`name*=utf-8''...`, continuations of at most 60 characters past that). With UTF-8 headers allowed, non-ASCII text SHALL be written as UTF-8 instead of encoded words.

#### Scenario: A display name with a comma and an accent
- GIVEN a mailbox named `Doe, Jöhn` at `john@example.com`
- WHEN it is written into a Cc field
- THEN the field reads back, through this crate and through mail-parser, as one mailbox with that name and address

### Requirement: Lines are folded

A written field SHALL keep every line within 76 characters where a fold point exists, so a line holding an encoded word keeps RFC 2047 2's limit too, and within 998 octets always (RFC 5322 2.1.1), folding only before a space that separates two words. A continuation line SHALL start with that space and SHALL NOT be blank, and no line SHALL end in a blank before its fold. A token longer than 998 octets SHALL fail with `LineTooLong`. A raw value SHALL be written as given, every line break written as the build's line ending and given a space when no blank follows it. A token holding a CR or an LF (a URL, a media type, a disposition, a transfer encoding of a hand-built model) SHALL fail with `LineBreak` rather than end its field, and a raw value line over 998 octets SHALL fail with `LineTooLong`.

#### Scenario: A long subject from any column
- GIVEN a 300-character subject mixing ASCII and non-ASCII words, under a short and a long field name
- WHEN it is written
- THEN every rule above holds, and it reads back as the subject written

### Requirement: What the writer normalises

The writer SHALL write what a reader can read back unchanged, and SHALL normalise what it could not: the ends of text, names and list items trimmed of blanks; empty list items, blank names, parameters with an empty name or value dropped; a leading U+FEFF of text dropped; an address reduced to what an angle address reads back (brackets, parentheses, backslashes and blanks outside closed quoted strings dropped, an obsolete route dropped), and written bare only as a plain `dot-atom@dot-atom`; message identifiers without brackets or blanks; a message with no field given its default Content-Type, since a nested message must open with a field.

### Requirement: Text line breaks and charsets

A text leaf's line breaks (CRLF, or LF alone) SHALL be written as the build's line ending; a CR elsewhere is content, except before a line break under LF, where it could not be told from the break and is dropped. Text SHALL be written in its declared charset when the build can encode every character there, else in UTF-8 with `charset=utf-8`, which non-ASCII text with no charset also gets.

### Requirement: The transfer encoding is chosen per part

A leaf SHALL keep the transfer encoding its model declares when that encoding is valid for its content and the options. Otherwise a text leaf SHALL be written `7bit` when every byte is ASCII, no NUL or stray CR or LF occurs and no line exceeds 998 octets; else `8bit` when allowed and the content fits; else quoted-printable or base64, whichever is shorter. Any other leaf SHALL be `binary` when allowed, else base64 (quoted-printable without the `base64` feature). Content written as is and ending in a CR SHALL not be under LF, where a delimiter after it would take the CR. A leaf, text or binary, under a `multipart` type, as a damaged multipart decodes, SHALL be base64, which never holds the `--` a reader would split at. A `message/rfc822` or `message/global` leaf SHALL be written under the first identity encoding that can carry it. Content written as is SHALL never hold a line reading as a delimiter of an enclosing multipart's boundary (`--` and the boundary at the start of a line); such content is re-encoded, and quoted-printable never starts a line with `-`. A multipart or a nested message SHALL take no encoding but an identity one (RFC 2045 6.4), a nested message whose fields carry UTF-8 SHALL be `message/global` (RFC 6532 3.7), and binary content SHALL never be altered.

### Requirement: Boundaries never collide

A multipart SHALL keep its declared boundary when it is 1 to 70 RFC 2046 5.1.1 `bchars`, does not end in a space, and does not occur in its written parts; otherwise it SHALL get `=_mime-rs_` and a 16-digit hexadecimal counter, the next value tried until one does not occur. Since a generated boundary cannot occur in base64 or quoted-printable output nor start another generated one, the parts SHALL be written straight into the output, unchecked, when no field name starts with `--`, no declared boundary has the generated shape and no content written as is holds the prefix. Generation SHALL use no clock and no randomness. The counters already present in the content are gathered in one pass, so content listing many generated boundaries costs one scan. A boundary SHALL also avoid the boundaries a caller says enclose what is written (an edit's enclosing multiparts): never equal to one, starting one or started by one, the prefix changing its first character when a taken boundary starts it.

#### Scenario: A forwarded message built by the same builder
- GIVEN a multipart message attaching, as `message/rfc822`, another multipart message this builder produced
- WHEN the outer message is built
- THEN no outer delimiter line appears inside the attached message, and both parse back with their own parts

### Requirement: Building is deterministic

The same model and options SHALL build the same bytes, on every run and every platform. The writer SHALL add `MIME-Version: 1.0` to a message before its first Content- field when it has one and no MIME-Version, and SHALL NOT invent a Date or a Message-ID; `MimeMessageId::new(unique, domain)` joins and checks the two halves a caller supplies.

### Requirement: A written message reads back as written

A model written to bytes SHALL parse with no anomaly into the model it states, under the normalisations above, and mail-parser 1.0 SHALL read the same structure and the same subject. Writing the model read back SHALL give the same bytes again, for any model, and for the model of any bytes.
