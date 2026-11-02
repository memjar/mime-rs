---
cairn: delta
change: the-differential-target
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: The header block (parsing)

A header block SHALL run from the start of an entity to its first empty line, which belongs to the block as its terminator: a line continuing no field and holding nothing but blanks, carriage returns and form feeds before its line feed, so `\n`, `\r\n`, and the `\r\r\n` of a message whose line endings were converted twice. A first line holding only blanks SHALL also be taken as the terminator of an empty block. The rest of the requirement is unchanged.

#### Scenario: Line endings converted twice
- GIVEN a multipart message whose every line ends in `\r\r\n`
- WHEN it is parsed
- THEN its header block ends on the line `\r\r\n`, its parts are found, and it serializes back to the input

### Requirement: The corpus is swept (parsing)

Every vendored fixture SHALL also read, as written and in both forms, as mail-parser 1.0 reads it live, but where a rule of the differential oracle (tests/common/differential.rs) records a deliberate difference and its reason. The fetched corpora (scripts/fetch-corpora.sh) SHALL be swept the same way by an ignored test, which also checks the round trip and the answerers, and the fuzz crate's `differential` target SHALL run the oracle over arbitrary bytes. The parsing preamble names the oracle beside tests/corpus/divergence.tsv.

### Requirement: Unstructured text (header-fields)

The ends of the value SHALL be trimmed of their blanks, and the blanks an encoded word decodes to SHALL stay, being text. An encoded word's charset SHALL be an ASCII token (RFC 2047 2), dots allowed as `ANSI_X3.4-1968` is written; a word whose charset holds anything else is no word and stays literal.

### Requirement: Dates (header-fields)

A year of four digits or more below 100 SHALL read as a two-digit year, a zero-padded `0001` reading 2001.

### Requirement: Leaf content is re-encoded on replacement (editing)

A leaf in a multipart read through fallback delimiters, or holding none, SHALL be refused as damaged, as a structural edit of that multipart is, since giving the delimiter around the leaf a line of its own would change how the multipart splits.

### Requirement: Field edits (editing)

`remove_all` SHALL take, with a field folded after a line holding no field, the blanks before its name, which would otherwise fold the line after it into the block.

### Requirement: Structural edits rewrite their multipart alone (editing)

An inserted part's delimiter line SHALL end a line of its own even where the sibling delimiter it follows the style of is cut at the end of its multipart.

### Requirement: The field values (conformance)

The validator SHALL report as a violation an address that is not a local part, an `@` and an unquoted domain, and, as unreadable, a date outside the RFC 5322 3.3 grammar and its 4.3 obsolete forms: time fields of two digits, a zone of `+` or `-` and four digits whose minutes are below 60 or a named or military one, a four-digit year from 1900, the day name followed by a comma, nothing after the zone but comments. A comment inside a date, or a missing blank between its parts, SHALL be an advisory, as the other obsolete forms are.

## REMOVED Requirements
