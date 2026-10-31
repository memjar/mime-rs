---
cairn: delta
change: validation-states-what-a-message-breaks
---

## ADDED Requirements

### Requirement: Validation is a runtime predicate with two severities

Conformance SHALL be checked at runtime by `validate`, never encoded as a second type. Every problem SHALL carry a severity: a violation for a MUST or MUST NOT, which withholds the proof, or an advisory for a SHOULD, a SHOULD NOT or the obsolete syntax RFC 5322 4 forbids generating, which does not. A conformant message MAY carry extension fields and media types.

#### Scenario: A message with no Message-ID
- GIVEN a message conformant in every way but carrying no Message-ID
- WHEN it is validated
- THEN an advisory names RFC 5322 3.6.4, and the message still earns the proof

### Requirement: The message fields

`validate` SHALL report, at the top of a message (RFC 5322 3.6): a missing or repeated Date or From; a repeated Sender, Reply-To, To, Cc, Bcc, Message-ID, In-Reply-To, References or Subject; a From of more than one mailbox with no Sender (RFC 5322 3.6.2); a Resent- block missing its Resent-Date or Resent-From (RFC 5322 3.6.6). A trace or Resent- field below the fields it should be prepended to SHALL be an advisory (RFC 5322 3.6). A nested message SHALL be held to the same rules as advisories, since a forwarded message is shown, not submitted.

#### Scenario: Two senders and no Sender
- GIVEN a message whose From holds two mailboxes and which has no Sender
- WHEN it is validated
- THEN a violation names RFC 5322 3.6.2

### Requirement: The field grammar

`validate` SHALL report a structured field that does not match its strict RFC 5322 grammar as a violation, one matching only the obsolete grammar as an advisory, a field line over 998 octets as a violation and over 78 characters as an advisory (RFC 5322 2.1.1), a bare CR, a bare LF or a NUL as a violation (RFC 5322 2.3), raw 8-bit bytes that are not UTF-8 as a violation and UTF-8 as needing SMTPUTF8 (RFC 6532), and an encoded word over 75 characters, inside a quoted string, an addr-spec or a parameter value as a violation (RFC 2047 5).

### Requirement: The MIME structure

`validate` SHALL report as violations: a Content- field at the top of a message with no MIME-Version (RFC 2045 4); a multipart with no boundary, or a boundary outside 1 to 70 `bchars` or ending in a space (RFC 2046 5.1.1); a multipart or a `message/rfc822` under any encoding but `7bit`, `8bit` or `binary` (RFC 2045 6.4, RFC 2046 5.2.1); a `message/partial` or `message/external-body` under any encoding but `7bit` (RFC 2046 5.2.2, 5.2.3); a part declared `7bit` holding 8-bit bytes or lines over 998 octets (RFC 2045 2.7); quoted-printable with an invalid sequence or a line over 76 characters (RFC 2045 6.7); base64 holding bytes outside its alphabet (RFC 2045 6.8); a `multipart/related` with no `type` (RFC 2387 3.1); a `multipart/signed` without `protocol` and `micalg`, or without exactly two parts (RFC 1847 2.1); a `multipart/encrypted` without `protocol`, or without exactly two parts (RFC 1847 2.2); a `multipart/report` with no `report-type` (RFC 6522 3); a Content-ID repeated within one message (RFC 2045 7); and every anomaly the parser recorded.

#### Scenario: A base64 message/rfc822
- GIVEN a forwarded message attached as `message/rfc822` under base64
- WHEN the outer message is validated
- THEN a violation names RFC 2046 5.2.1, and the nested message still parses and reads

### Requirement: Transport requirements are computed, not judged

The validation walk SHALL also compute `MimeTransport`: whether the message needs 8-bit bodies (8BITMIME), binary bodies (BINARYMIME), UTF-8 fields (SMTPUTF8) or lines over 998 octets. These SHALL NOT be problems: whether a transport offers them is the caller's to know.

### Requirement: The Valid proof

A message with no violation SHALL earn a `MimeValid<T>` that only the validator mints, and both `Mime` and `MimeValid<Mime>` SHALL convert back into a tree.

### Requirement: The contract is reachable without a parser

The markers, their specs, the dispatch, `Mime::validate` and `MimeBuilder` SHALL all be available with default features off.

#### Scenario: A build with no parser
- GIVEN default features off
- WHEN a message built by hand is validated
- THEN it validates, and the crate pulls in no dependency

### Requirement: Every built message validates

A message built by `MimeBuilder` from any model SHALL validate with no violation.

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
