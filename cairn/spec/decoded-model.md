---
cairn: spec
capability: decoded-model
status: current
---

# Decoded model

`Mime` is a message (or a MIME part) as pure data: its decoded header fields in order and its body, with no wire detail (folds, quoting, preambles, boundaries as written). The syntax tree keeps the wire; the model is what a reader means and a writer states. It is always available, whatever the features.

### Requirement: No dependency on the syntax side

The decoded model, the spec layer, the builder, the writer and the validator SHALL compile with `--no-default-features` and pull in no crate beyond `core` and `alloc`. Everything byte-faithful SHALL sit under `tree`, gated on `parser`, and every dependency behind a feature.

#### Scenario: The bare core
- GIVEN the crate built with `--no-default-features`
- WHEN the dependency graph is inspected
- THEN it contains no crate beyond `core` and `alloc`

### Requirement: The shape of a message

A `Mime` SHALL hold an ordered list of `MimeHeader` and a `MimeBody`. A body SHALL be the text of a `text` leaf (`Text`, transfer-decoded, transcoded to UTF-8, flowed text unwrapped), the bytes of any other leaf or of a text leaf the build cannot transcode (`Binary`, transfer-decoded where the build can), the parts of a multipart (`Multipart`), or a nested message (`Message`).

### Requirement: Open payloads keep unmodelled data

`MimeHeaderName` SHALL hold a known `MimeHeaderKind` or a verbatim unknown name, and `MimeHeaderValue` SHALL carry a `Raw` arm holding the value as written, which is also what a value that fits no other form reads as, so a field outside the model survives a decode and a write.

#### Scenario: A vendor field
- GIVEN a message carrying `X-Vendor-Thing: a; b`
- WHEN it is decoded and written again
- THEN the field comes back with its name and value as written

### Requirement: Values are bespoke types

The values SHALL be crate types, one per form: `MimeAddressList` of `MimeAddress` (a `MimeMailbox` or a `MimeGroup`), `MimeDateTime`, message identifiers as strings without angle brackets (`MimeMessageId` checks one), URLs, comma lists, `MimeContentType` and `MimeDisposition` with their `MimeParam`s, and `MimeTransferEncoding`.

### Requirement: Closed vocabularies reached through FromStr and Deref

`MimeHeaderKind` and `MimeHeaderForm` SHALL each be a closed, fieldless enum whose canonical spelling is reached through `Deref<Target = str>` and whose parsing is ASCII case-insensitive through `FromStr`, each with an `ALL` list. `MimeTransferEncoding` SHALL be spelled through `Deref` and read with `MimeTransferEncoding::parse`, an unknown mechanism kept as written in its `Unknown` arm.

### Requirement: Decoding projects

`MimeCst::decode` SHALL project a tree onto the model: every field read in its default form, every `text` leaf as text, every other leaf transfer-decoded where the build can, an encoded `message/rfc822` or `message/global` leaf parsed as a nested message, and a body the parse left unsplit split first.
