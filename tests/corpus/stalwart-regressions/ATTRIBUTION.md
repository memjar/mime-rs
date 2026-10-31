# Stalwart regressions

Small messages pinning one decided case each, from [mail-parser](https://github.com/stalwartlabs/mail-parser)'s adversarial review and GitHub issues. CATALOGUE.md is mail-parser's own README for the directory, naming the case behind each file.

- Source: `resources/eml/regressions/` of the mail-parser 1.0.0 crate, with its expected answers in LF and CRLF form.
- Licence: Apache-2.0 OR MIT, Copyright (C) 2020 Stalwart Labs LLC. These files keep that licence.
- Why: every malformation here was argued once already; the frozen answers say how mail-parser settled it.

Nothing here is edited. A fixture that fails is a bug in mime-rs, or a deliberate divergence recorded in tests/corpus/divergence.tsv.
