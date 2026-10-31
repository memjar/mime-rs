---
cairn: spec
capability: conformance
status: current
---

# Conformance

The validator: `Mime::check` and `MimeCst::check` list every problem a message has against its RFCs, and `validate` turns a message with no violation into a `MimeValid<T>`, the only way to obtain one. The parser forgives everything and the writer emits only what conforms; the validator lets a caller ask, of any message, which rules it breaks.

### Requirement: Validation is a runtime predicate with two severities

Conformance SHALL be checked at runtime, never encoded as a second type. Every problem SHALL be a `MimeValidateError` naming the entity (in document order), the field where there is one, the rule (`MimeValidateRule`, with its RFC section) and a `MimeSeverity`: a violation for a MUST or MUST NOT, which withholds the proof, or an advisory for a SHOULD, a SHOULD NOT or the obsolete syntax RFC 5322 4 forbids generating, which does not. A conformant message MAY carry extension fields and media types.

#### Scenario: A message with no Message-ID
- GIVEN a message conformant in every way but carrying no Message-ID
- WHEN it is validated
- THEN an advisory names RFC 5322 3.6.4, and the message still earns the proof

### Requirement: The message fields

The validator SHALL report, at the top of a message (RFC 5322 3.6): a missing Date or From, and a missing Message-ID as an advisory; a field appearing more often than its spec allows; a From of more than one mailbox with no Sender (RFC 5322 3.6.2); Resent- fields without a Resent-Date or Resent-From (RFC 5322 3.6.6). A trace field below a field that is neither trace nor Resent- SHALL be an advisory (RFC 5322 3.6). A nested message SHALL be held to the same rules as advisories, since a forwarded message is shown, not submitted.

#### Scenario: Two senders and no Sender
- GIVEN a message whose From holds two mailboxes and which has no Sender
- WHEN it is validated
- THEN a violation names RFC 5322 3.6.2

### Requirement: The field values

The validator SHALL report as violations a known structured field whose value does not read in its form, a date naming no real day, a mailbox with no address or an address with no `@` (Return-Path's null path `<>` alone excepted), a From or Sender naming no mailbox (RFC 5322 3.6.2), and a message identifier that is not `id-left "@" id-right`; a date in the obsolete syntax (a named or military zone, a year of fewer than four digits) SHALL be an advisory.

### Requirement: The wire

On the syntax tree the validator SHALL report: a line over 998 octets as a violation and a field line over 78 characters as an advisory (RFC 5322 2.1.1); a stray CR or LF or a NUL in a field as a violation (RFC 5322 2.3); a message whose lines end in LF alone as one advisory (RFC 5322 2.1), its line ending then taken as LF for every other rule; 8-bit bytes in a field that are not UTF-8 as a violation (RFC 6532 3); and an encoded word over 75 characters, inside a quoted string or an angle address of an address field, or in a parameter value, as a violation (RFC 2047 5).

### Requirement: The MIME structure

The validator SHALL report as violations: a Content- field at the top of a message with no MIME-Version (RFC 2045 4); a multipart with no boundary, or one outside 1 to 70 `bchars` or ending in a space (RFC 2046 5.1.1); a multipart or a `message/rfc822` or `message/global` under an encoding other than `7bit`, `8bit` or `binary` (RFC 2045 6.4); a `message/partial` or `message/external-body` not under `7bit` (RFC 2046 5.2.2); content declared `7bit` (or nothing) holding 8-bit bytes, a NUL, a stray CR or LF or lines over 998 octets (RFC 2045 2.7), lines over 998 octets in any other content but under `binary`, and content declared `8bit` holding a NUL, a stray CR or LF or long lines (RFC 2045 2.8); quoted-printable with an invalid sequence or a line over 76 characters (RFC 2045 6.7); base64 holding bytes outside its alphabet (RFC 2045 6.8); a `multipart/related` with no `type` (RFC 2387 3.1); a `multipart/signed` without `protocol` and `micalg` or without exactly two parts (RFC 1847 2.1); a `multipart/encrypted` without `protocol` or without exactly two parts (RFC 1847 2.2); a `multipart/report` with no `report-type` (RFC 6522 3); a Content-ID repeated within one message (RFC 2045 7); and every anomaly the parser recorded.

#### Scenario: A base64 message/rfc822
- GIVEN a forwarded message attached as `message/rfc822` under base64
- WHEN the outer message is validated
- THEN a violation names RFC 2045 6.4, and the nested message still parses and reads

### Requirement: Transport requirements are computed, not judged

The tree validation SHALL also compute `MimeTransport`: whether the message needs 8-bit bodies (8BITMIME), binary bodies (BINARYMIME), UTF-8 fields (SMTPUTF8) or lines over 998 octets. These SHALL NOT be problems: whether a transport offers them is the caller's to know.

### Requirement: The Valid proof

A message with no violation SHALL earn a `MimeValid<T>` that only `validate` mints, carrying the advisories and transport needs found, dereferencing to the message and yielding it with `into_inner`; a `MimeValid<Mime>` writes as any `Mime` does, and a `MimeValid<MimeCst>` yields its tree.

### Requirement: The contract is reachable without a parser

The markers, their specs, the dispatch, `Mime::check`, `Mime::validate`, `MimeBuilder` and `Mime::to_bytes` SHALL all be available with default features off; the model check reads the model's fields and structures, the wire rules being the tree's.

#### Scenario: A build with no parser
- GIVEN default features off
- WHEN a message built by hand is validated
- THEN it validates, and the crate pulls in no dependency

### Requirement: The writer adds no violation

A message written from any model SHALL carry no violation the model's own check does not state, and one built from complete fields SHALL carry no problem at all.

### Requirement: The corpus is classified

Every vendored fixture SHALL be classified as valid, advisories only, or with violations, with the counts per directory asserted.
