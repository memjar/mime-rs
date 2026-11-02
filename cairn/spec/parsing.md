---
cairn: spec
capability: parsing
status: current
---

# Parsing

The byte-faithful side of the crate, gated behind the `parser` feature. `MimeCst` models a message as its root `MimeEntity`, a header block and a body, every leaf a `Cow` borrowed from the input until an edit replaces it. It knows nothing of what a field means beyond what structure needs: it reproduces the wire.

Parsing is maximally liberal, per Postel's law: every input is a message, and every recovery is recorded rather than refused. The behaviour on malformed input follows mail-parser 1.0's frozen answers in tests/corpus, except where tests/corpus/divergence.tsv records a deliberate difference and its reason. On any input, the differential oracle (tests/common/differential.rs) holds this crate to mail-parser 1.0's live reading, each deliberate difference a named rule with its reason.

### Requirement: Round-trip fidelity

The parser SHALL preserve every byte of a parsed message, so serializing an unedited message reproduces the input exactly: line endings (CRLF, LF or mixed), folds, blanks before a colon, field order and casing, transport padding after a delimiter, the preamble, the epilogue, a missing final line break, and every malformation the input carries.

#### Scenario: A fixture in both line endings
- GIVEN any vendored fixture, and the same fixture with every CR removed, and with every LF preceded by a CR
- WHEN each is parsed and serialized
- THEN each output is byte-identical to its input

### Requirement: Serialization fixpoint

Serializing a parsed message SHALL produce bytes that reparse to the same bytes, whatever the input.

### Requirement: Parsing never fails

`MimeCst::parse` SHALL accept any byte sequence and return a tree, never an error and never a panic.

#### Scenario: Input with no header field
- GIVEN `hello world\nfoo bar\n`
- WHEN it is parsed
- THEN the tree holds one entity with no header block and the input as its body, and it serializes back to the input

### Requirement: Lines end at a line feed

A line SHALL end at LF, a CR immediately before it being part of the ending. A bare CR SHALL be content.

### Requirement: The header block

A header block SHALL run from the start of an entity to its first empty line, which belongs to the block as its terminator: a line continuing no field and holding nothing but blanks, carriage returns and form feeds before its line feed, so `\n`, `\r\n`, and the `\r\r\n` of a message whose line endings were converted twice. A first line holding only blanks SHALL also be taken as the terminator of an empty block. A field SHALL be a logical line, a physical line together with every following line starting with SP or HTAB (RFC 5322 2.2.3), a line of blanks continuing the field before it. Blanks between a field name and its colon SHALL stay on the wire and out of the name (the RFC 5322 4.5 obsolete syntax), and leading colons SHALL belong to the name.

A logical line whose first physical line holds no colon SHALL be kept as an opaque item, unless a later physical line of it does, in which case the bytes before that line's content SHALL be the opaque item and the rest the field, so `garbage\n Subject: x` holds a field named `Subject`. An opaque item SHALL round-trip, be reported, and not end the block. A block holding no field at all SHALL be no header block: the entity is all body. A block that reaches the end of its entity with no empty line SHALL end there, flagged.

#### Scenario: A block with no line break at all
- GIVEN `Subject: last`
- WHEN it is parsed
- THEN the entity has one field, an empty body and the no-blank-line anomaly, and serializes back to the input

#### Scenario: A blank continuation
- GIVEN `Subject: a\r\n   \r\nFrom: x\r\n\r\nbody`
- WHEN it is parsed
- THEN the block holds two fields and the body reads `body`

#### Scenario: Line endings converted twice
- GIVEN a multipart message whose every line ends in `\r\r\n`
- WHEN it is parsed
- THEN its header block ends on the line `\r\r\n`, its parts are found, and it serializes back to the input

### Requirement: A field name is never smuggled

A field name SHALL be compared as written, ASCII case-insensitively: a name holding a space or a control byte SHALL NOT match any known field, so `Sub ject` is never read as `Subject`.

### Requirement: Structure comes from the structural fields alone

Parsing SHALL read only the Content-Type (type, subtype and `boundary`) and the Content-Transfer-Encoding of each entity, the last of each winning when a field repeats, and SHALL keep every other field value as an unparsed leaf. An entity with no Content-Type SHALL be a leaf, or a `message/rfc822` inside a `multipart/digest` (RFC 2046 5.1.5).

### Requirement: The delimiter rule

Inside a multipart with a boundary, a delimiter line SHALL be `--` and the boundary at the start of a line, followed by the end of the line, a blank, or the end of the input; a close delimiter SHALL be followed by `--` (RFC 2046 5.1.1). The line ending before a delimiter SHALL belong to the delimiter, unless the previous delimiter line already ended on it. What precedes the first delimiter SHALL be the preamble and what follows the close delimiter the epilogue, both kept.

#### Scenario: The delimiter rule
- GIVEN the body `x\n--BND\r\n--BND--\n--BNDX\n--BND \t\n--BND` under boundary `BND`
- WHEN it is split
- THEN `--BND` opens a part, `--BND--` closes, `--BNDX` is content

### Requirement: Damaged multiparts still yield their parts

A multipart SHALL yield its parts however damaged, recording each recovery as an anomaly:

- with no close delimiter, its last part SHALL run to the end of the enclosing body, flagged unterminated;
- with no opening delimiter at the start of a line before the first close one, a `--` and boundary found mid-line or followed by other text SHALL open it, and with no close delimiter at the start of a line, one found mid-line SHALL close it, flagged as a fallback;
- a delimiter of an enclosing multipart SHALL close every inner multipart still open, which is flagged unterminated (RFC 2046 5.1.2), the shared delimiter lines going to the outer one when a nested multipart reuses its boundary;
- a delimiter SHALL cut a part's header block that never reached its empty line;
- a multipart with no `boundary` parameter, or whose boundary never occurs, SHALL be a leaf, flagged missing its delimiter;
- a boundary made only of dashes SHALL never match, its multipart holding no part.

### Requirement: Nested messages

A `message/rfc822` or `message/global` (RFC 6532 3.7) entity under an identity transfer encoding (none, `7bit`, `8bit`, `binary`) whose content opens with a field SHALL parse its body as a nested entity in place. Under `base64` or `quoted-printable`, which RFC 2046 5.2.1 forbids and real messages carry, the body SHALL stay a leaf, parsed only when read. An empty one, or one whose content opens with no field, SHALL stay a leaf.

### Requirement: Nesting and size are bounded

A parse SHALL nest at most 64 containers on one path, the root included, and hold at most 10,000 entities, each bound a public field of `MimeParseOptions`. A container past the nesting bound, and a multipart past the entity bound, SHALL stay a leaf, flagged, and still round-trip. The parse SHALL use an explicit stack rather than recurse per level, and every other walk of the tree SHALL recurse at most the depth bound.

#### Scenario: Ten thousand nested multiparts on a small stack
- GIVEN a message nesting 10,000 multiparts, parsed on a 64 KiB thread
- WHEN it is parsed
- THEN 64 multiparts nest, the 65th entity is a leaf flagged at the nesting limit, and the tree serializes back to the input

### Requirement: Parsing is linear

No input shape SHALL make parsing superlinear in the input length. The coroutine SHALL ask for a body's dash lines once and match them against every boundary open, and SHALL ask for a fallback search at most once per multipart.

#### Scenario: A body of near-miss delimiters
- GIVEN a multipart body of 100,000 lines each reading `--BNDX`, and one of 200,000
- WHEN each is parsed
- THEN the second takes at most a small multiple of the first

### Requirement: Parsing is an I/O-free coroutine

`MimeParse` SHALL perform no structural search across a header block or a body of its own. Every such search SHALL be a request `resume` yields, carrying offsets into the input, which the caller answers with positions before resuming, with any code it chooses. A request SHALL be a bulk job: every field end of one header block (`WantsFieldEnds`), every line starting with `--` across one body (`WantsDashLines`), or every occurrence of a boundary in one body for the fallback (`WantsFind`). `MimeCst::parse` SHALL be the coroutine answered by `tree::scan::memchr` under the default options.

#### Scenario: The requests of a nested multipart
- GIVEN a `multipart/mixed` message with 30 root fields and three parts, one of them a `multipart/alternative` of two parts
- WHEN it is parsed through the coroutine
- THEN it yields one field-ends request per header block (six) and one dash-lines request, and none per field or per delimiter

### Requirement: Every answerer answers as the scalar reference

`tree::scan::memchr`, and every answerer meant to replace `tree::scan::scalar`, SHALL return for every request exactly the positions the scalar answerer returns, so the tree never depends on what answered.

### Requirement: An answer is checked, never trusted

The coroutine SHALL check every position of an answer before using it: within the request's range, strictly increasing, and on the bytes its request names. A position failing the check SHALL be dropped and reported. No answer SHALL make the coroutine panic, read out of bounds or loop, and whatever the answers, the tree SHALL round-trip the input byte for byte.

#### Scenario: An answerer returning noise
- GIVEN any fixture, and an answerer returning arbitrary positions
- WHEN the fixture is parsed through it
- THEN the parse completes and the tree serializes back to the input

### Requirement: Anomalies are reported, never thrown

Every recovery SHALL be recorded on the entity it happened in, from a closed set (`MimeAnomaly`): no blank line, opaque line, missing delimiter, fallback delimiter, unterminated, nesting limit, part limit, dropped answer. `MimeCst::anomalies` SHALL list them with their entity's index in `MimeCst::entities`.

### Requirement: A parsed leaf borrows the input

Every leaf of a parsed tree SHALL borrow the input, and parsing SHALL copy no input byte. `MimeCst::chunks` SHALL yield every leaf in order as a borrowed slice, their concatenation being `to_bytes`.

### Requirement: The corpus is swept

Every vendored fixture SHALL parse, round-trip byte for byte as written and in its LF and CRLF forms, parse alike through both answerers, and, where it has a frozen answer, carry the structure that answer states (part tree, kinds, header, body and end offsets), unless tests/corpus/divergence.tsv holds a row for that fixture and path with its reason. The fixtures without frozen answers SHALL match mail-parser 1.0's offsets live. Each source directory SHALL assert its fixture count. Every fixture SHALL also read, as written and in both forms, as mail-parser 1.0 reads it live, but where a rule of the differential oracle (tests/common/differential.rs) records a deliberate difference and its reason. The fetched corpora (scripts/fetch-corpora.sh) SHALL be swept the same way by an ignored test, which also checks the round trip and the answerers, and the fuzz crate's `differential` target SHALL run the oracle over arbitrary bytes.

### Requirement: Structure is parsed as deep as asked

`MimeParseOptions::structure_depth` SHALL bound how many levels of containers the parse splits, `None` for all. A container past it SHALL keep its body whole as `MimeBodyNode::Unparsed`, which serializes as written, and `MimeEntity::expand` (or `MimeCst::expand` for every such body) SHALL split it later, the depth counting from that entity, its anomalies joining the entity's. A tree expanded in full SHALL equal the tree a full parse gives. `MimeParse::unparsed` SHALL be the coroutine of one such split, for callers answering the searches themselves. `decode` SHALL split what it needs.

#### Scenario: Reading the headers of a large message
- GIVEN a multipart message parsed at `structure_depth` `Some(0)`
- WHEN the parse runs
- THEN it yields one field-ends request and no other, the root body stays unparsed, and expanding it gives the tree a full parse gives
