---
cairn: delta
change: quoted-printable-over-the-codec
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: Features as in vcard and ical

Quoted-printable SHALL go through the quoted-printable-codec crate, in the core and always available; the quoted_printable crate is not used, as it drops bytes outside printable ASCII and rewrites every line ending to CRLF, which loses data in mail.

### Requirement: Quoted-printable loses nothing on reading

Quoted-printable decoding SHALL read `=XX` in either case, reporting a lowercase digit, join soft line breaks including one followed by trailing blanks or ending the input, drop the blanks a transport added at the end of a line (RFC 2045 6.7 rule 3), up to 76 of them, the older ones kept as data, keep an `=` that starts no valid sequence literally and report it, pass raw 8-bit bytes and a bare CR through and report them, and keep every line ending as written. In header mode (the RFC 2047 Q encoding) it SHALL read `_` as a space and report only an `=` starting no escape.

### Requirement: The encoders are exact

Quoted-printable SHALL be encoded with the codec's guard on, so a leading `.`, `F` or `-` is escaped as before.

## REMOVED Requirements
