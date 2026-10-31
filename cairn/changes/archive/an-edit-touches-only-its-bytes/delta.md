---
cairn: delta
change: an-edit-touches-only-its-bytes
---

## ADDED Requirements

### Requirement: Edits are byte-preserving

An edit SHALL rewrite only what it names: every other field, every other entity, every delimiter line, the preamble and the epilogue SHALL keep their bytes. An edited field SHALL be encoded and folded by the building rules, with the line ending its block uses.

#### Scenario: A subject changed on a folded message
- GIVEN a parsed message with folded, oddly spaced fields and a base64 attachment
- WHEN its Subject is set
- THEN the output differs from the input in the Subject field alone

### Requirement: An edit is local

An edit SHALL replace only the leaves it writes, owning their new bytes, and SHALL leave every other leaf borrowed from the input: nothing it does not name is copied, re-parsed or re-encoded, so its cost follows the size of what it writes, not the size of the message. A field edit SHALL touch one field of one header block, a content edit one entity's body leaf and its Content-Transfer-Encoding value, and a structural edit the part list of the one multipart it names (plus that multipart's boundary leaves when the boundary is regenerated).

#### Scenario: A subject changed beside a large attachment
- GIVEN a parsed message carrying a 20 MB base64 attachment
- WHEN its Subject is set
- THEN the Subject field's value leaf is owned, and every other leaf, the attachment's body included, still borrows the input

#### Scenario: One part's content replaced
- GIVEN a parsed `multipart/mixed` with three parts
- WHEN the second part's content is replaced
- THEN only that part's body leaf and its Content-Transfer-Encoding value are owned, and the root's fields, the delimiter lines and the other two parts still borrow the input

### Requirement: Bcc is removed for submission

Removing every field of one name SHALL leave the block otherwise intact, so a message can be submitted without its Bcc (RFC 5322 3.6.3).

#### Scenario: Two Bcc fields
- GIVEN a draft carrying two Bcc fields, one folded
- WHEN every Bcc field is removed
- THEN both are gone, with their continuation lines, and every other byte is unchanged

### Requirement: A new field is placed where it belongs

A field added to a block SHALL go after the last field of the same name, or else at the end of the block before its empty line, and a trace field (Received, Return-Path) SHALL go at the top (RFC 5321 4.4). Nothing already in the block SHALL move.

### Requirement: An item edit keeps the other items

Adding, changing or removing one mailbox or group of an address field SHALL keep the bytes of every other item, the separators between them included where they survive.

#### Scenario: A recipient appended
- GIVEN `To: "Doe, J." <j@x.test>,\r\n  (team) t@x.test\r\n`
- WHEN `k@x.test` is appended
- THEN both existing items keep their bytes, the comment included, and the field reads three mailboxes

### Requirement: Leaf content is re-encoded on replacement

Replacing a leaf's content SHALL encode it by the building rules, or under a transfer encoding the caller names, and SHALL rewrite or add that entity's Content-Transfer-Encoding field and no other.

### Requirement: Structural edits rewrite their multipart alone

Inserting, removing or moving a part SHALL write a delimiter line for an inserted part in the style of its siblings, remove a removed part with its delimiter line, and change nothing outside that multipart. When inserted content holds the multipart's boundary at the start of a line, the boundary SHALL be regenerated, rewriting that multipart's boundary parameter and delimiter lines only.

### Requirement: Wrapping a single part

Wrapping an entity into a new multipart SHALL keep the RFC 5322 message fields on the outer entity and move the Content- fields to the new first child, so adding an attachment to a single-part message yields a `multipart/mixed` whose first part is the original content, byte for byte.

#### Scenario: An attachment added to a plain draft
- GIVEN a single-part `text/plain` draft
- WHEN an attachment is added
- THEN the root is `multipart/mixed`, its first part's content is the original body unchanged, and From, To, Subject and Date are still on the root

### Requirement: A downgrade reports what it cannot fix

`MimeCst::downgrade_7bit` SHALL re-encode every 8bit or binary leaf as quoted-printable or base64, recurse into nested messages rather than encode them (RFC 2046 5.2.1), leave 7-bit leaves untouched, and report every field carrying UTF-8, which it does not rewrite.

### Requirement: Filling the required fields

`MimeCst::fill_required` SHALL add the Date and Message-ID it is given when the message lacks them, and MIME-Version when a Content- field is present without it, leaving every existing byte in place, and SHALL be idempotent.

### Requirement: Protected content is sealed

An edit that would change any byte inside the first part of a `multipart/signed`, or inside a `multipart/encrypted`, SHALL fail with an error naming the protecting entity (RFC 1847 2.1, RFC 3156 5). Edits elsewhere in the message, the root's fields included, SHALL proceed. `unwrap_protected` SHALL replace the protecting multipart by its content entity, after which edits proceed.

#### Scenario: A signed body
- GIVEN a `multipart/signed` message
- WHEN its signed text part's content is replaced
- THEN the edit fails naming the `multipart/signed` entity, and the message is unchanged

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
