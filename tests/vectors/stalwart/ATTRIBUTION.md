# Stalwart field vectors

Known-answer vectors for single header field values, from [mail-parser](https://github.com/stalwartlabs/mail-parser).

- Source: `resources/*.json` and `resources/decoders/charset_meta.json` of the mail-parser 1.0.0 crate: address.json, content_type.json, date.json, id.json, list.json, unstructured.json, thread_name.json, trailing_fwd.json, received.json (kept for a later structured Received reader), header_names.txt.
- Licence: Apache-2.0 OR MIT, Copyright (C) 2020 Stalwart Labs LLC. These files keep that licence.
- Why: each vector is one real-world spelling with the reading mail-parser settled on.

Nothing here is edited. A vector we read differently is a row in tests/vectors/divergence.tsv with its reason.
