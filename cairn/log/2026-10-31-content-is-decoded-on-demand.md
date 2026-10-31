---
cairn: log
change: content-is-decoded-on-demand
landed: 2026-10-31
---

# Content is decoded on demand

Landed the leaf accessors (`bytes`, `quoted_printable`, `base64`, `decoded`, `decoded_len`, `text`, `text_prefix` and the checked variants with `MimeDecodeProblems`), charset selection for text parts (byte order mark, then the parameter, then UTF-8), RFC 3676 flowed decoding, the RFC 8621 4.1.4 body lists with `has_attachment`, and `nested` for encoded nested messages. `decode` now fills leaves: `MimeBody::Leaf` split into `Text` (a text leaf decoded and transcoded) and `Binary`, which the builder needs to choose charsets and transfer encodings, and an encoded `message/rfc822` decodes to `MimeBody::Message`. tests/decoded.rs holds every fixture's decoded content, problems and body lists against the frozen answers.

The decode fuzz target found that `flowed::encode` sliced a line inside a UTF-8 character and that both flowed functions took a final CR with no LF after it for a line ending; both are fixed, with unit tests failing on the old code. The vendor-named accessor `charset` of the plan is `text` here. The ignored sweep over fetched corpora waits for the fetch script of phase 7.

The full gate is met at 0.75x mail-parser 1.0.

Capabilities moved: content-decoding (ADDED), decoded-model (MODIFIED: `Text` and `Binary` leaves).
