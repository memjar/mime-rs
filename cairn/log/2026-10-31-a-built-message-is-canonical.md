---
cairn: log
change: a-built-message-is-canonical
landed: 2026-10-31
---

# A built message is canonical

Landed the writer (`Mime::to_bytes` under `MimeBuildOptions`: line ending, 8bit, binary, UTF-8 headers, flowed text, legacy `name`), the strict `MimeBuilder` with mail-builder's layout and its refusals, `MimeMessageId`, and the encoders: encoded words (B or Q, whichever is shorter, whole characters, the first word sized to its line), phrases, RFC 2231 parameters, the folder, the transfer encoding choice, flowed text and deterministic boundaries (`=_mime-rs_` and a 16-digit counter). The writer needs no parser.

The property test (tests/build.rs) writes arbitrary models under arbitrary options and checks they parse with no anomaly into what was written, that mail-parser 1.0 sees the same parts and subject, and that writing the model read back gives the same bytes; it ran 30,000 cases clean. The `write` fuzz target writes the model of arbitrary bytes and checks the same fixpoint; it found fourteen classes, each fixed and kept as a case in tests/build.rs: an empty list item written as `""`, a blank name, a header-less nested message read as a leaf, a CR before an LF under LF (which the model cannot tell from a line break, so it is dropped), binary or unknown-encoded content ending in a CR under LF (a delimiter takes the CR), a garbage address written bare or with blanks, quotes, brackets or a route inside angle brackets, a message identifier holding a bracket, a leading U+FEFF read back as a byte order mark, a parameter with an empty name or value, a leaf under a multipart type (a damaged multipart) reparsed as a multipart, now written in base64, and flowed wrapping exposing a soft break once a CR was dropped. The quoted-printable encoder now takes a CR before an LF for part of the break only under CRLF.

The build gate is missed: 0.61x mail-builder 1.0. Callgrind puts 45% of the instructions in the base64 crate's encoder (about 7 per input byte, against mail-builder's table encoder translating 12 bits per lookup), 12% in copies and most of the rest in the writer itself. The writer was cut from 0.28x to 0.61x by emitting base64 straight into the output, writing multiparts straight into it when a generated boundary cannot collide, checking text for `7bit` with core's word-at-a-time scans and reserving the output once; the remaining lever is the base64-codec crate being built in its own repository, to integrate once it lands.

Decisions that moved from the plan: the folder targets 76 columns rather than 78, so lines holding encoded words keep RFC 2047's limit too; a build without `base64` writes quoted-printable where it would write base64, rather than failing, since quoted-printable is always available; and the strict builder takes typed fields through the markers.

Capabilities moved: building (ADDED).
