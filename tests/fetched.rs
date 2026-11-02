//! Sweeps over the fetched corpora (scripts/fetch-corpora.sh, into
//! `$MIME_CORPORA` or target/corpora): every message parses, serializes
//! back byte for byte, parses alike through both answerers, and reads as
//! mail-parser 1.0 reads it but where a rule of the differential oracle
//! (tests/common/differential.rs) lets a deliberate difference through.
//!
//! Ignored, as the corpora are fetched rather than vendored:
//!
//! ```sh
//! cargo test --release --all-features --test fetched -- --ignored
//! ```

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

mod common;
#[path = "common/differential.rs"]
mod differential;

use std::{env, fs, path::PathBuf};

use mail_parser::MessageParser;
use mime::tree::cst::MimeCst;

use crate::common::parse_scalar;

/// The corpora: a directory under the corpora root, and whether every file
/// is a message (else only `.eml` and `.mbox` files are).
const CORPORA: [(&str, bool); 7] = [
    ("enron/maildir", true),
    ("apache-mbox", false),
    ("stalwart/tests", false),
    ("stalwart-v1/tests", false),
    ("stalwart-smtp", false),
    ("mail-auth/resources", false),
    ("hunny", false),
];

fn files(dir: PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            files(path, out);
        } else {
            out.push(path);
        }
    }
}

/// The messages of an mbox: what follows each `From ` line at a line start.
fn mbox(bytes: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut at = 0;

    while at < bytes.len() {
        if bytes[at..].starts_with(b"From ") && (at == 0 || bytes[at - 1] == b'\n') {
            starts.push(at);
        }

        match bytes[at..].iter().position(|byte| *byte == b'\n') {
            Some(lf) => at += lf + 1,
            None => break,
        }
    }

    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            let end = starts.get(index + 1).copied().unwrap_or(bytes.len());
            let body = bytes[start..end]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(end, |lf| start + lf + 1);
            &bytes[body..end]
        })
        .collect()
}

#[test]
#[ignore = "needs scripts/fetch-corpora.sh"]
fn fetched_corpora_round_trip_and_read_as_mail_parser_reads_them() {
    let root = env::var_os("MIME_CORPORA").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/corpora"),
        PathBuf::from,
    );
    let mut failures = Vec::new();
    let mut swept = 0;

    for (corpus, all) in CORPORA {
        let mut paths = Vec::new();
        files(root.join(corpus), &mut paths);
        paths.sort();

        let mut messages = 0;

        for path in paths {
            let mbox_file = path.extension().is_some_and(|ext| ext == "mbox");
            let eml = path.extension().is_some_and(|ext| ext == "eml");

            if !(all || mbox_file || eml) {
                continue;
            }

            let bytes = fs::read(&path).expect("message");
            let inputs = if mbox_file {
                mbox(&bytes)
            } else {
                vec![bytes.as_slice()]
            };

            for (index, input) in inputs.into_iter().enumerate() {
                let name = format!("{}#{index}", path.display());
                let cst = MimeCst::parse(input);
                messages += 1;

                if cst.to_bytes() != input {
                    failures.push(format!("{name}: not byte-faithful"));
                }

                if parse_scalar(input) != cst {
                    failures.push(format!("{name}: the answerers disagree"));
                }

                let answer = MessageParser::new()
                    .parse(input)
                    .map(|message| serde_json::to_value(&message).expect("serializes"));

                for difference in differential::compare(input, answer.as_ref(), false).differences {
                    failures.push(format!(
                        "{name} {}: ours {}, theirs {}",
                        difference.path, difference.ours, difference.theirs
                    ));
                }
            }
        }

        println!("{corpus}: {messages} messages");
        swept += messages;
    }

    assert!(swept > 0, "no corpus under {}", root.display());
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures
            .iter()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
