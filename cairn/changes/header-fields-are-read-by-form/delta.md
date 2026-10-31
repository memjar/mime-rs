---
cairn: delta
change: header-fields-are-read-by-form
---

## ADDED Requirements

### Requirement: Any field reads in any form

Every field SHALL read in its default form through its lens, and in any `MimeHeaderForm` through `MimeHeaderField::read_as`, as RFC 8621 4.1.2 lets a client ask. A value standing alone SHALL read through `MimeHeaderForm::parse`. Reading SHALL never fail: a value that does not fit the form SHALL read as no value, its raw bytes still at hand.

#### Scenario: A vendor field read as addresses
- GIVEN a block holding `X-Sender: <kel@example.com>`
- WHEN the field is read as addresses
- THEN it yields one mailbox whose address is `kel@example.com`

### Requirement: Default forms

Each known field SHALL default to one form: text for Subject, Comments and Content-Description; addresses for From, Sender, Reply-To, To, Cc, Bcc, the Resent- address fields, Return-Path and Disposition-Notification-To; message ids for Message-ID, In-Reply-To, References, Resent-Message-ID and Content-ID; date for Date and Resent-Date; URLs for List-Archive, List-Help, List-Owner, List-Post, List-Subscribe and List-Unsubscribe; comma list for Keywords and Content-Language; the MIME forms for Content-Type, Content-Disposition and Content-Transfer-Encoding; raw for every other field, unknown ones included.

### Requirement: Unstructured text

The text form SHALL unfold the value (RFC 5322 2.2.3), decode every RFC 2047 encoded word standing as a word of its own, drop the linear whitespace between two adjacent encoded words (RFC 2047 6.2), and trim leading and trailing whitespace. Adjacent encoded words of one charset SHALL be joined before transcoding, so a character split across two words survives. A malformed encoded word SHALL stay literal. An empty or blank value SHALL read as empty text, not as no value.

#### Scenario: A character split across two words
- GIVEN `Subject: =?utf-8?B?w6k=?= =?utf-8?B?w6k=?=` holding one two-byte character in each word, and the same character split byte by byte across two words
- WHEN each is read as text
- THEN the first reads `éé` and the second `é`

### Requirement: Addresses

The address forms SHALL read an RFC 5322 3.4 address list with its obsolete syntax (empty list elements, routes, which are dropped, CFWS anywhere), groups in any address field (RFC 6854), and UTF-8 in local parts, domains and display names (RFC 6532). A display name SHALL be decoded (encoded words, quoted strings unescaped). Mailboxes and groups SHALL come in document order. The flat form SHALL list every mailbox, group members included; the grouped form SHALL keep the groups.

#### Scenario: A group between two mailboxes
- GIVEN `To: a@x.test, Friends: b@x.test, c@x.test;, d@x.test`
- WHEN it is read flat, then grouped
- THEN the flat form lists a, b, c and d in that order, and the grouped form a mailbox, the group `Friends` holding b and c, and a mailbox

### Requirement: Message ids

The message ids form SHALL read every `msg-id` of the value in order, without its angle brackets, accepting CFWS between them and an id written without brackets.

### Requirement: Dates

The date form SHALL read an RFC 5322 3.3 date-time with the RFC 5322 4.3 obsolete syntax: an optional day name, two-digit years (below 50 in the 2000s, otherwise in the 1900s), three-digit years (plus 1900), optional seconds, and the named and military zones as RFC 5322 4.3 directs. A date naming no real calendar day SHALL read as no value. `MimeDateTime` SHALL convert to Unix seconds and to RFC 3339 text.

#### Scenario: An obsolete zone at the end of the input
- GIVEN a block whose last line is `Date: Thu, 13 Feb 1969 23:32 EST` with no line break
- WHEN it is read
- THEN it yields 1969-02-13 23:32:00 at -05:00, and the field takes nothing beyond its own line

### Requirement: URLs

The URLs form SHALL read an RFC 2369 list: every URL inside angle brackets, in order, comments and whitespace between them skipped, anything outside brackets ignored.

### Requirement: Content type and disposition

The content type form SHALL read a type, a subtype and parameters, tolerating CFWS around every token (`image/ jpeg`) and a missing subtype, and comparing type, subtype and parameter names ASCII case-insensitively while keeping them as written. A parameter value SHALL be unquoted and unescaped. RFC 2231 continuations SHALL be joined in section order, each section counted once and the number of sections bounded, and an RFC 2231 charset and language prefix SHALL be decoded. A value written as RFC 2047 encoded words, which RFC 2047 5 forbids and real messages send, SHALL be decoded. The disposition form SHALL read the same grammar with a disposition type.

#### Scenario: A filename in three continuations
- GIVEN `Content-Type: image/gif; name*1="about "; name*0="Book "; name*2*=utf-8''%e2%98%95 tables.gif`
- WHEN the `name` parameter is read
- THEN it reads `Book about ☕ tables.gif`

### Requirement: Comma lists and transfer encodings

The comma list form SHALL split at commas outside quoted strings and decode and trim each item. The transfer encoding form SHALL read the mechanism token case-insensitively, keeping an unknown token as written.

### Requirement: The base subject

`MimeText::base_subject` SHALL strip reply and forward prefixes with their counters (`Re:`, `Fwd:`, `Re[2]:` and their localized spellings), bracketed list tags and trailing `(fwd)` markers (RFC 5256 2.1), borrowing from the subject.

#### Scenario: Stacked prefixes
- GIVEN the subject `Re: Fwd: [list] Lunch on Friday`
- WHEN its base subject is read
- THEN it reads `Lunch on Friday`

### Requirement: The field vectors pass

Every Stalwart field vector under tests/vectors/stalwart SHALL read as its expected answer, unless the divergence file holds a row for it with its reason.

### Requirement: Field reading is linear

No field value SHALL make its reader superlinear: folded runs of colon-less lines, thousands of RFC 2231 sections and long runs of encoded words SHALL each read in time proportional to their length.

### Requirement: Closed vocabularies reached through FromStr and Deref

`MimeHeaderKind`, `MimeHeaderForm` and `MimeTransferEncoding` SHALL each be a closed, fieldless enum whose canonical spelling is reached through `Deref<Target = str>` and whose parsing is case-insensitive through `FromStr`. `MimeHeaderKind::ALL` SHALL enumerate every known field.

### Requirement: Open payloads keep unmodelled data

`MimeHeaderName` SHALL hold a known kind or a verbatim unknown name, and `MimeHeaderValue` SHALL carry an `Unknown` arm holding the raw value, so a field outside the model survives a decode and an encode.

#### Scenario: A vendor field
- GIVEN a message carrying `X-Vendor-Thing: a; b`
- WHEN it is decoded and encoded again
- THEN the field comes back with its name and value as written

### Requirement: Decoding projects, encoding projects back

`MimeCst::decode` SHALL project a tree onto the decoded `Mime`, and `encode` SHALL project a `Mime` back onto a canonical tree, so decoding, encoding and decoding again yields the first decoded message.

### Requirement: The spec dispatch answers for the field it is asked about

The bridge from a header kind to its marker's `MimeHeaderSpec` SHALL carry the kind it describes, and that kind SHALL be the one it was dispatched from.

### Requirement: Decoding is feature-gated

Each transfer decoder SHALL sit behind its own feature, `base64` and `quoted-printable`, and charset transcoding behind `encoding`, as in vcard and ical. Without the feature an encoding needs, an encoded word of that encoding SHALL stay literal in the text form, and nothing SHALL be decoded silently or lossily in its place.

#### Scenario: A B-encoded subject without the base64 feature
- GIVEN the crate built without `base64`, and `Subject: =?utf-8?B?w6k=?=`
- WHEN the subject is read as text
- THEN it reads `=?utf-8?B?w6k=?=` as written

### Requirement: Transfer decoding is liberal

Base64 decoding SHALL skip line breaks and whitespace, skip and report every other byte outside the alphabet, and accept missing or misplaced padding. Quoted-printable decoding SHALL read `=XX` in either case, join soft line breaks including one followed by trailing whitespace, keep an `=` that starts no valid sequence literally and report it, and in header mode (the RFC 2047 Q encoding) read `_` as a space.

### Requirement: Character sets

The core SHALL transcode nothing. The `encoding` feature SHALL transcode through `encoding_rs::Encoding::for_label`, quotes and an RFC 2231 language suffix stripped from the label first, after whichever transfer decoding features are on, and SHALL fall back to UTF-8 when the label is absent or unknown, reporting an unknown one. Without `encoding`, an encoded word in a charset other than UTF-8 or US-ASCII SHALL stay literal.

#### Scenario: A Latin-1 encoded word
- GIVEN `Subject: =?iso-8859-1?q?caf=E9?=`
- WHEN it is read as text with `encoding` on, then with it off
- THEN it reads `café`, then `=?iso-8859-1?q?caf=E9?=` as written

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
