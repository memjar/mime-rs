---
cairn: spec
capability: editing
status: current
---

# Editing

Edits on the syntax tree, under `tree::edit`, that rewrite only what they name. Field edits work on a `MimeHeaderBlock` reached through `MimeCst::header_mut`; content and structural edits work on `MimeCst` by entity index (as `MimeCst::entities` numbers them), since they must see the multipart around the entity they change. Every byte an edit writes follows the building rules (building.md), with the line ending of the block or body it lands in.

### Requirement: Edits are byte-preserving

An edit SHALL rewrite only what it names: every other field, every other entity, every delimiter line, the preamble and the epilogue SHALL keep their bytes, except the line endings an edit must add so that what it writes starts a line of its own (stated below). An edited field SHALL be encoded and folded by the building rules, with the line ending its block uses.

#### Scenario: A subject changed on a folded message
- GIVEN a parsed message with folded, oddly spaced fields and a base64 attachment
- WHEN its Subject is set
- THEN the output differs from the input in the Subject field alone

### Requirement: An edit is local

An edit SHALL replace only the leaves it writes, owning their new bytes, and SHALL leave every other leaf borrowed from the input: nothing it does not name is copied, re-parsed or re-encoded. A field edit SHALL touch one field of one header block, a content edit one entity's body leaf and its Content-Transfer-Encoding field, and a structural edit the part list of the one multipart it names (plus that multipart's boundary leaves when the boundary is regenerated). `MimeHeaderField::is_owned` SHALL tell a written field from a borrowed one.

#### Scenario: A subject changed beside a large attachment
- GIVEN a parsed message carrying a large base64 attachment
- WHEN its Subject is set
- THEN the Subject field is owned, and every other leaf, the attachment's body included, still borrows the input

#### Scenario: One part's content replaced
- GIVEN a parsed `multipart/mixed` with three parts
- WHEN the second part's content is replaced
- THEN only that part's body leaf and its Content-Transfer-Encoding field are owned

### Requirement: Field edits

`MimeHeaderBlock::set` (typed by marker) and `set_value` SHALL rewrite the first field of the name, or insert one when the block has none; `insert` and `insert_value` SHALL add a field; `remove_all` SHALL remove every field of a name with its continuation lines and say how many, and when no field is left the block's opaque lines SHALL go too, since a block of them alone would read as body. Content-Type and Content-Transfer-Encoding SHALL be refused by every field edit (`Reserved`): they state how the body reads, a protecting multipart's included, and change through the content and structural edits alone.

#### Scenario: Two Bcc fields
- GIVEN a draft carrying two Bcc fields, one folded
- WHEN every Bcc field is removed
- THEN both are gone, with their continuation lines, and every other byte is unchanged

### Requirement: A new field is placed where it belongs

A field added to a block SHALL go after the last field of the same name, else after the last field of the block (before any opaque line trailing it), and a trace field (Received, Return-Path) SHALL go at the top (RFC 5321 4.4). Nothing already in the block SHALL move. The line before a new field SHALL get a line ending when it has none, and a block that had no field SHALL get a bare empty line when its own was missing or held blanks.

### Requirement: An item edit keeps the other items

`push_address`, `set_address` and `remove_address` SHALL add, change or remove one mailbox or group of an address field, the items being what commas outside quoted strings, comments, angle addresses and groups separate, and SHALL keep the bytes of every other item, the separators between them included where they survive; an appended item SHALL fold onto a new line when it would pass 76 columns, and removing the last item SHALL remove the field.

#### Scenario: A recipient appended
- GIVEN `To:   "Doe, J." <j@x.test>,\r\n  (team) t@x.test\r\n`
- WHEN `k@x.test` is appended
- THEN both existing items keep their bytes, the comment included, and the field reads three mailboxes

### Requirement: Leaf content is re-encoded on replacement

`MimeCst::replace_content` SHALL encode the content of a leaf by the building rules, or under a transfer encoding the caller names when it can carry it, and SHALL rewrite or add that entity's Content-Transfer-Encoding field and no other; `replace_text` SHALL write text in the leaf's declared charset when it can hold it, and otherwise in UTF-8, rewriting the Content-Type to say `charset=utf-8`. Content written as is SHALL never hold a line reading as a delimiter of an enclosing multipart, being re-encoded otherwise. A header block left without its empty line SHALL get one, the delimiter line after the leaf SHALL get the line ending before it an empty part did not need, and a leaf under a `multipart` type SHALL be refused as damaged.

### Requirement: Structural edits rewrite their multipart alone

`MimeCst::insert_part`, `remove_part` and `move_part` SHALL change the part list of the multipart they name: an inserted part, written by the building rules, SHALL get a delimiter line in the style of its siblings; a removed part SHALL leave with its delimiter line; a moved one SHALL keep its own; the line ending before a delimiter SHALL be added where a part now precedes it, and written CRLF under LF after a part ending in a CR. Removing a multipart's last part SHALL be refused (RFC 2046 5.1.1). When inserted content holds the multipart's boundary, the boundary SHALL be regenerated, rewriting that multipart's boundary parameter and delimiter lines only. A multipart read through fallback delimiters, or holding none, SHALL be refused as damaged, since a delimiter line an edit writes would change how it splits.

### Requirement: Boundaries avoid the enclosing ones

Every boundary an edit writes, the regenerated one, the one of a wrapping multipart and those inside an inserted part, SHALL avoid the boundaries of the multiparts enclosing it: never equal to one, never starting or started by one. A multipart whose boundary conflicts with an enclosing one SHALL be given a fresh boundary before a structural edit on it.

### Requirement: Wrapping and unwrapping

`MimeCst::wrap` SHALL wrap an entity into a new `multipart/<sub>`, keeping its non-Content fields on the entity and moving its Content- fields, in order, to the new first part, whose body is the entity's body byte for byte; a digest part with no Content-Type (a message by default) SHALL have it stated. `MimeCst::attach` SHALL add a part at the end of a `multipart/mixed`, wrapping the entity into one first. `MimeCst::unwrap` SHALL replace a multipart of one part by that part, its Content- fields giving way to the part's, the part's default type stated when it had none.

#### Scenario: An attachment added to a plain draft
- GIVEN a single-part `text/plain` draft
- WHEN an attachment is added
- THEN the root is `multipart/mixed`, its first part's content is the original body unchanged and still borrowed, and From, To, Subject and Date are still on the root

### Requirement: A downgrade reports what it cannot fix

`MimeCst::downgrade_7bit` SHALL re-encode as quoted-printable or base64 every leaf written as is that a 7-bit transport cannot carry, in one walk over the tree, (`8bit`, `binary`, or 8-bit bytes, NUL, stray CR or LF, or long lines under no encoding), recurse into nested messages rather than encode them (RFC 2046 5.2.1), relabel `7bit` the containers whose bodies are now 7-bit, leave 7-bit leaves untouched, and report in `MimeDowngrade` every field carrying UTF-8, which it does not rewrite, and every entity it left because protected content holds it.

### Requirement: Filling the required fields

`MimeCst::fill_required` SHALL add the Date and Message-ID it is given when the message lacks them, and MIME-Version when a Content- field is present without it, leaving every existing byte in place, and SHALL be idempotent.

### Requirement: Protected content is sealed

An edit reached through `MimeCst` that would change any byte inside the first part of a `multipart/signed`, or inside a `multipart/encrypted`, SHALL fail with `MimeEditError::Sealed` naming the protecting entity (RFC 1847 2.1, RFC 3156 5), and so SHALL a content or structural edit on the protecting multipart itself. Edits elsewhere in the message, the protecting multipart's own fields included, SHALL proceed. `MimeCst::protector` SHALL name the protecting entity of any index, and `unwrap_protected` SHALL replace the protecting multipart by its content entity, after which edits proceed.

#### Scenario: A signed body
- GIVEN a `multipart/signed` message
- WHEN its signed text part's content is replaced
- THEN the edit fails naming the `multipart/signed` entity, and the message is unchanged

### Requirement: An edited tree reads back as edited

After any sequence of edits, the tree's bytes SHALL parse to a tree that decodes as the edited tree decodes, and SHALL be a fixpoint; a field edit on the root SHALL leave every entity under it byte for byte.
