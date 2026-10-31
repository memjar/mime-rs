# Dovecot malformed

Messages from the [Dovecot](https://github.com/dovecot/core) test suite, as vendored by [mail-parser](https://github.com/stalwartlabs/mail-parser).

- Source: `resources/eml/malformed/` of the mail-parser 1.0.0 crate, with mail-parser's expected answers in LF and CRLF form.
- Licence: MIT, Copyright (c) 2003-2018 Dovecot authors (see https://github.com/dovecot/core/blob/master/COPYING). The expected answers are mail-parser's, Apache-2.0 OR MIT, Copyright (C) 2020 Stalwart Labs LLC.
- 023.eml is not Dovecot's: it is a minimised fuzzer input reported against mail-parser 0.11, kept by mail-parser as a regression test.
- Why: an independent implementation's view of what real and broken messages look like.

Nothing here is edited. A fixture that fails is a bug in mime-rs, or a deliberate divergence recorded in tests/corpus/divergence.tsv.
