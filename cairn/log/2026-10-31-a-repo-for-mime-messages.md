---
cairn: log
change: a-repo-for-mime-messages
landed: 2026-10-31
---

# A repository for MIME messages

Scaffolded the crate from vcard: Cargo.toml (mime-rs, lib `mime`, edition 2024, rust-version 1.88), the nix files with himalaya's lock (rustc 1.99 in the devshell, enough for the mail-parser 1.0 dev-dependency), deny.toml, the licences with the holder string, .gitignore and the audit workflow. Vendored the Stalwart and Dovecot corpus directories with their frozen answers and attribution, and the Stalwart field vectors.

One decision moved from the plan: quoted-printable is decoded in-house and always on, not through the quoted_printable crate behind a feature. That crate drops every byte outside printable ASCII and rewrites every line ending to CRLF, which loses data in mail, where quoted-printable bodies carry raw 8-bit bytes and LF endings. base64 stays on the base64 crate behind the `base64` feature, and charsets on encoding_rs behind `encoding`, as in vcard and ical.

The README, CONTRIBUTING, AGENTS.md and tests workflow, the fetch script and the baseline benchmarks were left for later changes.

Capabilities moved: decoded-model (ADDED: no dependency on the syntax side), folded once the decoded model exists.
