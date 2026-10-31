---
cairn: change
id: validation-states-what-a-message-breaks
status: landed
created: 2026-10-31
---

# Validation states what a message breaks

Phase 6 of [a-mime-library](../../a-mime-library/proposal.md). It folds into a new cairn/spec/conformance.md.

## Why

Postel's law is only half done until the conservative half exists. The parser forgives everything, the builder emits only what conforms, and the validator is what lets a caller ask, of any message, which rules it breaks: before submission, in a test suite, or when a consumer wonders why another client chokes on a message.

Mail's RFCs speak in MUST and in SHOULD far more than iCalendar's or vCard's do, and many real messages break a SHOULD harmlessly (a missing Message-ID, a 120-character line). A validator that blocks on those is ignored, so the two are told apart.

The transport extensions a message needs (8BITMIME, BINARYMIME, SMTPUTF8) are a question every submitter asks and none of the consumers answers today.

## What

- `Mime::validate` and `MimeCst::validate` walk the whole tree and return every problem at once, each a `MimeValidateError` naming the entity, the field where there is one, the rule and its RFC section, with a severity: a violation (MUST, MUST NOT) blocks the proof, an advisory (SHOULD, SHOULD NOT, and the obsolete syntax RFC 5322 4 says not to generate) does not.
- A message with no violation earns `MimeValid<T>`, which only the validator mints, and which converts back into a tree.
- `MimeTransport`: what the message needs from a transport (8-bit bodies, binary bodies, UTF-8 fields, lines over 998 octets), computed on the same walk, not a problem in itself.
- The rule table lives in `MimeHeaderSpec` and `MimeMediaSpec`, the source the builder already reads, and needs no parser.
- The corpus sweep classifies every fixture by outcome (valid, advisories only, violations) with asserted counts, as ical's corpora are classified.
