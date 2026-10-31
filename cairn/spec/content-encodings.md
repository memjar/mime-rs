---
cairn: spec
capability: content-encodings
status: current
---

# Content encodings

The transfer encodings, character sets and text formats content arrives in, under `codec`, usable without the syntax tree. Decoding is liberal and never fails: it always returns content and says how far it can be trusted. Encoding is exact: what an encoder writes decodes back to its input.

### Requirement: Features as in vcard and ical

Base64 SHALL sit behind the `base64` feature (the base64 crate) and charset transcoding behind `encoding` (encoding_rs), both default. Quoted-printable SHALL be decoded and encoded in-house and always be available, since the quoted_printable crate drops bytes outside printable ASCII and rewrites every line ending to CRLF, which loses data in mail. Without the feature an encoding needs, an encoded word of that encoding SHALL stay literal in the text form, a body of that encoding SHALL come out as written and be reported, and nothing SHALL be decoded silently or lossily in its place.

#### Scenario: A B-encoded subject without the base64 feature
- GIVEN the crate built without `base64`, and `Subject: =?utf-8?B?w6k=?=`
- WHEN the subject is read as text
- THEN it reads `=?utf-8?B?w6k=?=` as written

### Requirement: Base64 is liberal on reading

Base64 decoding SHALL skip line breaks and blanks, skip and report every other byte outside the alphabet, accept missing padding, read data after padding as a second run and report it, and drop and report a lone trailing symbol. A well-formed body SHALL decode line by line through the crate, and only a damaged one through a filtering pass.

### Requirement: Quoted-printable loses nothing on reading

Quoted-printable decoding SHALL read `=XX` in either case, join soft line breaks including one followed by trailing blanks, drop the blanks a transport added at the end of a line (RFC 2045 6.7 rule 3), keep an `=` that starts no valid sequence literally and report it, pass raw 8-bit bytes through, and keep every line ending as written. In header mode (the RFC 2047 Q encoding) it SHALL read `_` as a space.

### Requirement: The encoders are exact

Base64 SHALL be written in lines of 76 characters, each ended by the given line ending, encoded a block of lines per call. Quoted-printable SHALL keep every line within 76 characters, soft break included, encode `=`, every byte outside printable ASCII, a blank ending a line, a leading `.` or `F` an SMTP or mbox reader could take for a marker, and a leading `-`, so that no line of the output can read as a MIME delimiter; in text mode an LF (with the CR before it when the line ending is CRLF) SHALL be a hard break written as the line ending and any other CR data. The Q encoding SHALL keep only letters, digits and `!*+-/` literal (RFC 2047 5(3)). Each SHALL decode, through this crate's decoders, to exactly its input, at every length around the line limit.

### Requirement: Character sets

The core SHALL transcode nothing but UTF-8 and US-ASCII. The `encoding` feature SHALL transcode through `encoding_rs::Encoding::for_label`, quotes and an RFC 2231 language suffix stripped from the label first, after whichever transfer decoding features are on, and SHALL fall back to UTF-8 when the label is unknown, reporting it. A body SHALL take a byte order mark it starts with over its label, and read as UTF-8 with no label. Encoding text into a charset SHALL succeed only when the charset can hold every character of it.

#### Scenario: A Latin-1 encoded word
- GIVEN `Subject: =?iso-8859-1?q?caf=E9?=`
- WHEN it is read as text with `encoding` on, then with it off
- THEN it reads `café`, then `=?iso-8859-1?q?caf=E9?=` as written

### Requirement: Flowed text

RFC 3676 decoding SHALL join a line ending in a space with the next line of the same quote depth, remove that space too with `DelSp=yes`, remove one leading space of a space-stuffed line, never flow the signature separator `-- `, and keep quote depth per paragraph. Encoding SHALL wrap lines longer than the width after a space, whole characters only, remove the trailing spaces of a hard line (and its trailing CRs under an LF line ending), and space-stuff a line starting with a space, `>` or `From `. A CR not before an LF SHALL be content in both directions.

#### Scenario: A flowed paragraph with DelSp
- GIVEN a flowed leaf with `DelSp=yes` holding `Hello wor \r\nld\r\n`
- WHEN its text is read
- THEN it reads `Hello world\r\n`
