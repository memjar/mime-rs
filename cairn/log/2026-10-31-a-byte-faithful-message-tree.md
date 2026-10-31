---
cairn: log
change: a-byte-faithful-message-tree
landed: 2026-10-31
---

# A byte-faithful message tree

Landed `MimeCst` with `Cow` leaves, the `MimeParse` coroutine yielding bulk search requests, the scalar and memchr answerers, and the answer checks. Every vendored fixture round-trips byte for byte as written and in LF and CRLF forms, both answerers build equal trees, and the parse, scan and answer fuzz targets ran clean (about 234,000, 3.6 million and 400,000 runs).

Matching mail-parser's frozen answers settled several readings: the last Content-Type and Content-Transfer-Encoding win, a block holding no field is no header block, a garbage line before a folded field is split off as an opaque item, a nested message must open with a field, and a delimiter followed by other text or found mid-line opens or closes a multipart that has no strict opener or closer. Four classes are recorded in tests/corpus/divergence.tsv: a header block cut by a delimiter keeps the delimiter's line ending off it (RFC 2046 5.1.1), a reused nested boundary goes to the outer multipart, an open delimiter at the end of input opens an empty part, and an encoded message/rfc822 is parsed only when read.

Capabilities moved: parsing (ADDED).
