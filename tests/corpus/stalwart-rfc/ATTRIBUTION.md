# Stalwart RFC examples

The example messages of the MIME and Internet Message Format RFCs, as collected by [mail-parser](https://github.com/stalwartlabs/mail-parser).

- Source: `resources/eml/rfc/` of the mail-parser 1.0.0 crate, with its expected answers (`NAME.json` for LF input, `NAME.crlf.json` for CRLF input).
- Licence: Apache-2.0 OR MIT, Copyright (C) 2020 Stalwart Labs LLC. These files keep that licence.
- Why: the canonical shapes every reader must get right, with mail-parser's reading of each frozen as the differential oracle.

Nothing here is edited. A fixture that fails is a bug in mime-rs, or a deliberate divergence recorded in tests/corpus/divergence.tsv.
