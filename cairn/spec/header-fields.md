---
cairn: spec
capability: header-fields
status: current
---

# Header fields

How a field value is read: the forms RFC 8621 4.1.2 names plus the MIME ones, the grammar behind each, the default form of every known field, and the spec table the decoder, the builder and the validator share. The readers live in `tree::field` and are liberal: a value that does not fit reads as the closest thing it means, never as an error. The writers that mirror them are in building.md.

### Requirement: Any field reads in any form

Every field SHALL read in its default form through its marker (`entity.header::<SUBJECT>()`), and in any `MimeHeaderForm` through `MimeHeaderField::read_as`, as RFC 8621 4.1.2 lets a client ask. A value standing alone SHALL read through `MimeHeaderForm::parse`, which takes the bytes after a colon up to the end of the field. Reading SHALL never fail: a value that does not fit the form SHALL read as `Raw`, its bytes still at hand.

#### Scenario: A vendor field read as addresses
- GIVEN a block holding `X-Sender: <kel@example.com>`
- WHEN the field is read as addresses
- THEN it yields one mailbox whose address is `kel@example.com`

### Requirement: Default forms

Each known field SHALL default to one form: text for Subject, Comments, Content-Description, Content-Location, MIME-Version, List-Id, List-Unsubscribe-Post and Auto-Submitted; grouped addresses for From, Sender, Reply-To, To, Cc, Bcc, the Resent- address fields, Return-Path and Disposition-Notification-To; message ids for Message-ID, In-Reply-To, References, Resent-Message-ID and Content-ID; date for Date and Resent-Date; URLs for List-Archive, List-Help, List-Owner, List-Post, List-Subscribe and List-Unsubscribe; comma list for Keywords and Content-Language; the MIME forms for Content-Type, Content-Disposition and Content-Transfer-Encoding; raw for Received and every unknown field. Four differ from mail-parser, each a divergence row: the List-* URL fields (mail-parser reads addresses), List-Id (text, RFC 2919 3), Return-Path (an address, RFC 5322 3.6.7) and Disposition-Notification-To (addresses, RFC 8098 2.1).

### Requirement: The spec table

`MimeHeaderKind::spec` SHALL give every known field its `MimeHeaderSpec`: the kind it describes, which SHALL be the kind it was asked of, its default form, how many times it must and may appear at the top of a message (RFC 5322 3.6), whether a MIME part may carry it, and its RFC section. `MimeHeaderKind` SHALL cover the fields of RFC 5322, RFC 2045, RFC 2183, RFC 2369, RFC 2557, RFC 2919, RFC 3282, RFC 3834, RFC 8058 and RFC 8098 registered in the IANA permanent message header field registry. Each kind SHALL have a marker spelled as its wire name with hyphens as underscores (`MESSAGE_ID`), carrying the kind and the typed value of its form.

### Requirement: Unstructured text

The text form SHALL unfold the value (RFC 5322 2.2.3), decode every RFC 2047 encoded word, drop the linear whitespace between two adjacent encoded words (RFC 2047 6.2), collapse a run of blanks holding a fold to one space, and trim the blanks at the ends of the value; the blanks an encoded word decodes to SHALL stay, being text. Adjacent encoded words of one charset SHALL be joined before transcoding, so a character split across two words survives. A malformed encoded word, or one whose encoding or charset the build cannot decode, SHALL stay literal, and so SHALL a word whose charset is no ASCII token (RFC 2047 2), dots allowed as `ANSI_X3.4-1968` is written. An empty or blank value SHALL read as empty text.

#### Scenario: A character split across two words
- GIVEN `Subject: =?utf-8?B?w6k=?= =?utf-8?B?w6k=?=` holding one two-byte character in each word, and the same character split byte by byte across two words
- WHEN each is read as text
- THEN the first reads `éé` and the second `é`

### Requirement: Addresses

The address forms SHALL read an RFC 5322 3.4 address list with its obsolete syntax (empty list elements, routes, which are dropped, CFWS anywhere), groups in any address field and without their closing `;` (RFC 6854), and UTF-8 in local parts, domains and display names (RFC 6532). A display name SHALL be decoded (encoded words, also inside quoted strings, and quoted pairs unescaped), with the comments folded into it in parentheses. Mailboxes and groups SHALL come in document order. The flat form SHALL list every mailbox, group members included; the grouped form SHALL keep the groups. A lone word with no `@` SHALL read as a domain-less address (a divergence row, mail-parser reads a name).

#### Scenario: A group between two mailboxes
- GIVEN `To: a@x.test, Friends: b@x.test, c@x.test;, d@x.test`
- WHEN it is read flat, then grouped
- THEN the flat form lists a, b, c and d in that order, and the grouped form a mailbox, the group `Friends` holding b and c, and a mailbox

### Requirement: Message ids

The message ids form SHALL read every `msg-id` of the value in order, without its angle brackets or blanks, accepting CFWS between them, and SHALL read words written without brackets as identifiers only when no bracketed one exists.

### Requirement: Dates

The date form SHALL read an RFC 5322 3.3 date-time with the RFC 5322 4.3 obsolete syntax: an optional day name, two-digit years (below 50 in the 2000s, otherwise in the 1900s), three-digit years (plus 1900), four-digit years below 100 read as two-digit ones (a zero-padded `0001` reads 2001), optional seconds, and the named and military zones as RFC 5322 4.3 directs. A date naming no real calendar day SHALL read as `Raw`. `MimeDateTime` SHALL convert to and from Unix seconds and to RFC 3339 text, and display as RFC 5322 writes it.

#### Scenario: An obsolete zone at the end of the input
- GIVEN a block whose last line is `Date: Thu, 13 Feb 1969 23:32 EST` with no line break
- WHEN it is read
- THEN it yields 1969-02-13 23:32:00 at -05:00, and the field takes nothing beyond its own line

### Requirement: URLs

The URLs form SHALL read an RFC 2369 list: every URL inside angle brackets, in order, blanks inside dropped, comments and whitespace between them skipped, anything outside brackets ignored.

### Requirement: Content type and disposition

The content type form SHALL read a type, a subtype and parameters, tolerating CFWS around every token (`image/ jpeg`), a missing subtype and a missing `;`, keeping type, subtype and parameter names as written and comparing them ASCII case-insensitively. A parameter value SHALL be unquoted and unescaped; a parameter with an empty name or value SHALL be dropped. RFC 2231 continuations SHALL be joined in section order, each section counted once and at most 1,000 joined, an RFC 2231 charset and language prefix decoded, and an extended value SHALL win over a plain one of the same name. A value written as RFC 2047 encoded words, which RFC 2047 5 forbids and real messages send, SHALL be decoded. The disposition form SHALL read the same grammar with a disposition type. The structural readers SHALL read the media type and one parameter in place, allocating nothing until a parameter is asked for.

#### Scenario: A filename in three continuations
- GIVEN `Content-Type: image/gif; name*1="about "; name*0="Book "; name*2*=utf-8''%e2%98%95 tables.gif`
- WHEN the `name` parameter is read
- THEN it reads `Book about ☕ tables.gif`

### Requirement: Comma lists and transfer encodings

The comma list form SHALL split at commas outside quoted strings and encoded words, and decode and trim each item, dropping empty ones. The transfer encoding form SHALL read the mechanism token case-insensitively, comments around it skipped, keeping an unknown token as written.

### Requirement: The base subject

`value::subject::base_subject` SHALL strip reply and forward prefixes with their counters (`Re:`, `Fwd:`, `Re[2]:`, their localized spellings and any word of four letters or fewer before a colon), bracketed list tags, `[fwd: ...]` wrappers and trailing `(fwd)` markers (RFC 5256 2.1), repeatedly, borrowing from the subject.

#### Scenario: Stacked prefixes
- GIVEN the subject `Re: Fwd: [list] Lunch on Friday`
- WHEN its base subject is read
- THEN it reads `Lunch on Friday`

### Requirement: The known answers pass

Every Stalwart field vector under tests/vectors/stalwart, and every decoded header value of every vendored fixture whose form both libraries share, SHALL read as its expected answer, unless the matching divergence file holds a row for it with its reason.

### Requirement: Field reading is linear

No field value SHALL make its reader superlinear: runs of encoded-word starts with no end, thousands of RFC 2231 sections or parameters, long address lists, nested comments and folded runs SHALL each read in time proportional to their length.
