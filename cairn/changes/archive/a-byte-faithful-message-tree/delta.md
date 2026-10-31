---
cairn: delta
change: a-byte-faithful-message-tree
---

## ADDED Requirements

### Requirement: Round-trip fidelity

The parser SHALL preserve every byte of a parsed message, so serializing an unedited message reproduces the input exactly: line endings (CRLF, LF or mixed), folds, blanks before a colon, field order and casing, transport padding after a delimiter, the preamble, the epilogue, a missing final line break, and every malformation the input carries.

#### Scenario: A fixture in both line endings
- GIVEN any vendored fixture, and the same fixture with every LF turned into CRLF
- WHEN each is parsed and serialized
- THEN each output is byte-identical to its input

### Requirement: Serialization fixpoint

Serializing a parsed message SHALL produce bytes that reparse to the same bytes, whatever the input.

#### Scenario: Arbitrary input
- GIVEN any byte sequence
- WHEN it is parsed, serialized, and the output parsed and serialized again
- THEN the two outputs are byte-identical

### Requirement: Parsing never fails

`MimeCst::parse` SHALL accept any byte sequence and return a tree, never an error and never a panic. Input whose first line is neither a header field nor a continuation SHALL parse as one entity with no header block, its whole content the body.

#### Scenario: Input with no header
- GIVEN `hello world\nfoo bar\n`
- WHEN it is parsed
- THEN the tree holds one entity with no header block and the input as its body, and it serializes back to the input

#### Scenario: Empty input
- GIVEN zero bytes
- WHEN they are parsed
- THEN the tree holds one empty entity and serializes to zero bytes

### Requirement: Lines end at a line feed

A line SHALL end at LF, a CR immediately before it being part of the ending. A bare CR SHALL be content, as RFC 5322 2.3 gives it no meaning of its own.

### Requirement: The header block

A header block SHALL run from the start of an entity to its first empty line, which belongs to the block as its terminator. A field SHALL be a line holding a colon, together with every following line that starts with SP or HTAB (RFC 5322 2.2.3); a line of blanks alone inside the block SHALL continue the field before it. Blanks between a field name and its colon SHALL stay on the wire and out of the name (the RFC 5322 4.5 obsolete syntax). A line inside the block that is neither a field nor a continuation SHALL be kept as an opaque item that round-trips, SHALL be reported, and SHALL NOT end the block. A block that reaches the end of its entity with no empty line SHALL end there, and the entity SHALL record the anomaly.

#### Scenario: A block with no line break at all
- GIVEN `Subject: last`
- WHEN it is parsed
- THEN the entity has one field, an empty body and the no-blank-line anomaly, and serializes back to the input

#### Scenario: A blank continuation
- GIVEN `Subject: a\r\n   \r\nFrom: x\r\n\r\nbody`
- WHEN it is parsed
- THEN the block holds two fields and the body reads `body`

### Requirement: A field name is never smuggled

A field name SHALL be compared as written: a name holding a space, a control byte or a NUL, or starting with a colon, SHALL NOT match any known field, so `Sub ject` is never read as `Subject`. Names SHALL match known fields ASCII case-insensitively.

#### Scenario: A space inside a name
- GIVEN a block holding `Sub ject: x`
- WHEN the message's Subject is looked up
- THEN none is found, and the field stays in the block as written

### Requirement: Structure comes from the structural fields alone

Parsing SHALL read only Content-Type (its type, subtype and `boundary`) and Content-Transfer-Encoding of each entity, and SHALL keep every other field value as an unparsed leaf. An entity with no Content-Type SHALL be `text/plain`, or `message/rfc822` inside a `multipart/digest` (RFC 2045 5.2, RFC 2046 5.1.5).

### Requirement: The delimiter rule

Inside a multipart with a boundary, a delimiter line SHALL be `--` and the boundary at the start of a line, followed by the end of the line, by transport padding (SP or HTAB) then the end of the line, or by the end of the input; a close delimiter SHALL be followed by `--` (RFC 2046 5.1.1). A line where the boundary is followed by anything else SHALL be content. The line ending before a delimiter SHALL belong to the delimiter, not to the part it ends. What precedes the first delimiter SHALL be the preamble and what follows the close delimiter the epilogue, both kept.

#### Scenario: The delimiter rule
- GIVEN the body `x\n--BND\r\n--BND--\n--BNDX\n--BND \t\n--BND` under boundary `BND`
- WHEN it is split
- THEN `--BND` opens a part, `--BND--` closes, `--BNDX` is content, `--BND` followed by padding opens a part, and `--BND` at the end of input opens one

### Requirement: Damaged multiparts still yield their parts

A multipart SHALL yield its parts however damaged, recording each recovery as an anomaly:

- with no close delimiter, its last part SHALL run to the end of the enclosing body;
- with no delimiter at the start of any line, a `--` and boundary found mid-line SHALL act as the delimiter, flagged as a fallback;
- a boundary made only of dashes SHALL never match;
- a delimiter of an enclosing multipart SHALL close every inner multipart still open, which is flagged unterminated (RFC 2046 5.1.2);
- a delimiter SHALL cut a part's or a nested message's header block that has not reached its empty line;
- a multipart with no `boundary` parameter, or whose boundary never occurs, SHALL be a leaf.

#### Scenario: An inner multipart closed by an outer delimiter
- GIVEN a `multipart/mixed` whose inner `multipart/alternative` never writes its close delimiter before the outer one
- WHEN it is parsed
- THEN the inner multipart is flagged unterminated, the part after the outer delimiter is a child of the outer multipart, and the input round-trips

### Requirement: Nested messages

A `message/rfc822` or `message/global` (RFC 6532 3.7) entity under an identity transfer encoding (none, `7bit`, `8bit`, `binary`) SHALL parse its body as a nested entity in place. Under `base64` or `quoted-printable`, which RFC 2046 5.2.1 forbids and real messages carry, the body SHALL stay a leaf, parsed only when read and at most three encoded levels deep. An empty `message/rfc822`, and an encoded one whose content has no header block, SHALL stay a leaf. `message/partial` and `message/external-body` SHALL be leaves.

### Requirement: Nesting and size are bounded

A parse SHALL nest at most 64 containers, the root included, and hold at most 10,000 entities, each bound a public field of `MimeParseOptions`. Content past a bound SHALL stay raw in the last entity admitted, flagged, and still round-trip. The parse SHALL use an explicit stack rather than recurse per level, and every other walk of the tree SHALL recurse at most the depth bound.

#### Scenario: Ten thousand nested multiparts on a small stack
- GIVEN a message nesting 10,000 multiparts, parsed on a 64 KiB thread
- WHEN it is parsed
- THEN the tree nests 64 levels, the deepest flagged at the nesting limit, and it serializes back to the input

### Requirement: Parsing is linear

No input shape SHALL make parsing superlinear in the input length. The coroutine SHALL ask for a body's dash lines once and match them against every boundary open, never asking again for a nested multipart, and SHALL ask for a fallback search at most once per multipart.

#### Scenario: A body of near-miss delimiters
- GIVEN a multipart body of one million lines each reading `--BNDX`
- WHEN it is parsed, and again at twice the length
- THEN the second parse takes at most about twice the first

### Requirement: Parsing is an I/O-free coroutine

`MimeParse` SHALL perform no byte search of its own. Every search SHALL be a request `resume` yields, carrying offsets into the input, which the caller answers with positions before resuming, using any code it chooses. A request SHALL be a bulk job: every field end of one header block (`WantsFieldEnds`), every line starting with `--` across one body (`WantsDashLines`), or one needle in one range for the mid-line fallback (`WantsFind`). `MimeCst::parse` SHALL be the coroutine answered by `tree::scan::memchr` under the default options.

#### Scenario: The requests of a nested multipart
- GIVEN a `multipart/mixed` message with 30 root fields and three parts, one of them a `multipart/alternative` of two parts
- WHEN it is parsed through the coroutine
- THEN it yields one field-ends request per header block (six) and one dash-lines request for the root body, and none per field or per delimiter

### Requirement: Every answerer answers as the scalar reference

`tree::scan::memchr`, and every answerer meant to replace `tree::scan::scalar`, SHALL return for every request exactly the positions the scalar answerer returns, so the tree never depends on what answered.

#### Scenario: One fixture through two answerers
- GIVEN any fixture, parsed once with the scalar answers and once with the memchr answers
- WHEN the two trees are compared
- THEN they are equal, node for node

### Requirement: An answer is checked, never trusted

The coroutine SHALL check every position of an answer before using it: within the request's range, strictly increasing, and on the bytes its request names. A position failing the check SHALL be dropped and reported as an anomaly. No answer SHALL make the coroutine panic, read out of bounds or loop, and whatever the answers, the tree SHALL round-trip the input byte for byte, its leaves still partitioning the input.

#### Scenario: An answerer returning noise
- GIVEN a fixture, and an answerer that returns arbitrary positions
- WHEN the fixture is parsed through it
- THEN the parse completes, the dropped positions are reported, and the tree serializes back to the input

### Requirement: Anomalies are reported, never thrown

Every recovery SHALL be recorded on the entity it happened in, from a closed set: missing close delimiter, fallback delimiter, no blank line, unterminated multipart, opaque header line, nesting limit, part limit, dropped answer position. `MimeCst::anomalies` SHALL list them with their entity.

### Requirement: A parsed leaf borrows the input

Every leaf of a parsed tree SHALL borrow the input, and parsing SHALL copy no input byte: it allocates only the vectors holding a header block's items and a multipart's parts. `MimeCst::chunks` SHALL yield every leaf in order as a borrowed slice, their concatenation being `to_bytes`.

#### Scenario: A parsed message with a large attachment
- GIVEN a message carrying a 20 MB base64 attachment
- WHEN it is parsed
- THEN every leaf of the tree, the attachment's body included, borrows the input

### Requirement: The corpus is swept

Every vendored fixture SHALL parse, round-trip byte for byte in its LF and CRLF forms, serialize to a fixpoint, and, where it has a frozen answer, carry the structure that answer states (part tree, kinds, header and body offsets), unless tests/corpus/divergence.tsv holds a row for that fixture and path with its reason. Each source directory SHALL assert its fixture count.

#### Scenario: A new disagreement
- GIVEN a fixture whose structure no longer matches its frozen answer, with no divergence row
- WHEN the sweep runs
- THEN it fails, naming the fixture and the first differing path

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
