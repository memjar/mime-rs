---
cairn: delta
change: the-simd-codecs
---

## ADDED Requirements

### Requirement: The CPU token is the application's

No mime-rs function SHALL detect CPU features. `MimeParseOptions` and `MimeBuildOptions` SHALL carry a `simd_cpu::SimdCpu`, `SimdCpu::BASELINE` by default; the parser SHALL copy the parse's token into every `MimeEntity` it builds, an edit SHALL give a new entity its parent's token, and a body expanded later or a nested message parsed by `nested` SHALL be parsed under its entity's token, whatever the options passed say. A leaf accessor SHALL decode under its entity's token, the builder encode under its options' token, and an RFC 2047 encoded word decode and encode under `SimdCpu::BASELINE`.

#### Scenario: A detected token reaches every entity
- GIVEN a multipart holding a message/rfc822 part, parsed with `MimeParseOptions { cpu: SimdCpu::detect(), .. }`, at full depth and at structure depth 0 then expanded with the default options
- WHEN its entities are listed, and when a part is attached to a single-part message parsed the same way
- THEN every entity holds the detected token

## MODIFIED Requirements

### Requirement: Features as in vcard and ical

Base64 SHALL sit behind the `base64` feature (the base64-codec crate) and charset transcoding behind `encoding` (encoding_rs), both default. Quoted-printable SHALL be decoded and encoded in-house and always be available, since the quoted_printable crate drops bytes outside printable ASCII and rewrites every line ending to CRLF, which loses data in mail. Without the feature an encoding needs, an encoded word of that encoding SHALL stay literal in the text form, a body of that encoding SHALL come out as written and be reported, and nothing SHALL be decoded silently or lossily in its place.

#### Scenario: A B-encoded subject without the base64 feature
- GIVEN the crate built without `base64`, and `Subject: =?utf-8?B?w6k=?=`
- WHEN the subject is read as text
- THEN it reads `=?utf-8?B?w6k=?=` as written

### Requirement: Base64 is liberal on reading

Base64 decoding SHALL skip line breaks and blanks, skip and report every other byte outside the alphabet, accept missing padding, read data after padding as a second run and report it, and drop and report a lone trailing symbol.

### Requirement: The encoders are exact

Base64 SHALL be written in lines of 76 characters, each ended by the given line ending, the last one included. Quoted-printable SHALL keep every line within 76 characters, soft break included, encode `=`, every byte outside printable ASCII, a blank ending a line, a leading `.` or `F` an SMTP or mbox reader could take for a marker, and a leading `-`, so that no line of the output can read as a MIME delimiter; in text mode an LF (with the CR before it when the line ending is CRLF) SHALL be a hard break written as the line ending and any other CR data. The Q encoding SHALL keep only letters, digits and `!*+-/` literal (RFC 2047 5(3)). Each SHALL decode, through this crate's decoders, to exactly its input, at every length around the line limit.

### Requirement: The options

`MimeBuildOptions` SHALL state the line ending written (CRLF by default, LF for a local store), whether text may be `8bit` (8BITMIME), whether a leaf may be `binary` (BINARYMIME), whether fields may carry UTF-8 instead of encoded words (RFC 6532), whether `text/plain` with no `format` is written flowed (RFC 3676), whether a part with a non-ASCII `filename` also gets an encoded `name` on its Content-Type for old readers, encoded words in a parameter that RFC 2047 5 forbids and the validator reports, and the CPU token bodies are encoded under.

## REMOVED Requirements
