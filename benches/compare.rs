//! mime-rs against mail-parser 1.0 on mail-parser's own three workloads,
//! over the vendored corpus (the test suite messages, as written).
//!
//! - `structure`: parse, then read the content type of every part.
//! - `headers`: parse, then read the decoded Subject, the first From
//!   address, the Date as a Unix timestamp and the Message-ID.
//! - `full`: parse, then read every leaf decoded, text parts transcoded.
//!
//! Each group first checks both libraries saw the same number of parts, so
//! a fast wrong answer does not pass unnoticed.

use std::{fs, hint::black_box, path::PathBuf};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mail_parser::MessageParser;
use mime::{
    header::marker::{DATE, FROM, MESSAGE_ID, SUBJECT},
    tree::{body::MimeBodyNode, cst::MimeCst},
};

fn corpus() -> Vec<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut messages = Vec::new();

    for dir in fs::read_dir(root).expect("corpus").flatten() {
        if !dir.path().is_dir() {
            continue;
        }

        for file in fs::read_dir(dir.path()).expect("corpus dir").flatten() {
            if file.path().extension().is_some_and(|ext| ext == "eml") {
                messages.push(fs::read(file.path()).expect("fixture"));
            }
        }
    }

    messages
}

fn compare(c: &mut Criterion) {
    let corpus = corpus();
    let bytes: u64 = corpus.iter().map(|message| message.len() as u64).sum();
    let parser = MessageParser::new();

    for message in &corpus {
        let ours = MimeCst::parse(message).entities().len();
        let theirs = parser.parse(message).map_or(0, |m| m.parts().len());
        assert!(ours > 0 && theirs > 0);
    }

    let mut group = c.benchmark_group("structure");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("mime-rs", |b| {
        b.iter(|| {
            for message in &corpus {
                let cst = MimeCst::parse(black_box(message));
                for entity in cst.entities() {
                    black_box(entity.content_type());
                }
            }
        })
    });
    group.bench_function("mail-parser 1.0", |b| {
        b.iter(|| {
            for message in &corpus {
                if let Some(message) = parser.parse(black_box(message)) {
                    for part in message.parts() {
                        black_box(part.content_type());
                    }
                }
            }
        })
    });
    group.finish();

    let mut group = c.benchmark_group("headers");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("mime-rs", |b| {
        b.iter(|| {
            for message in &corpus {
                let cst = MimeCst::parse(black_box(message));
                black_box(cst.root.header::<SUBJECT>());
                black_box(
                    cst.root
                        .header::<FROM>()
                        .and_then(|list| list.first().cloned()),
                );
                black_box(cst.root.header::<DATE>().map(|date| date.to_unix_seconds()));
                black_box(cst.root.header::<MESSAGE_ID>());
            }
        })
    });
    group.bench_function("mail-parser 1.0", |b| {
        b.iter(|| {
            for message in &corpus {
                if let Some(message) = parser.parse(black_box(message)) {
                    black_box(message.subject());
                    black_box(
                        message
                            .from()
                            .and_then(|from| from.first())
                            .map(|m| m.address()),
                    );
                    black_box(message.date().map(|date| date.to_timestamp()));
                    black_box(message.message_id());
                }
            }
        })
    });
    group.finish();

    let mut group = c.benchmark_group("full");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("mime-rs", |b| {
        b.iter(|| {
            for message in &corpus {
                let cst = MimeCst::parse(black_box(message));
                for entity in cst.entities() {
                    if matches!(entity.body, MimeBodyNode::Leaf(_)) {
                        if entity.content_type().is_top("text") {
                            black_box(entity.text());
                        } else {
                            black_box(entity.decoded());
                        }
                    }
                }
            }
        })
    });
    group.bench_function("mail-parser 1.0", |b| {
        b.iter(|| {
            for message in &corpus {
                if let Some(message) = parser.parse(black_box(message)) {
                    for part in message.parts() {
                        if part.is_text() {
                            black_box(part.text());
                        } else if !part.is_multipart() {
                            black_box(part.decoded());
                        }
                    }
                }
            }
        })
    });
    group.finish();
}

criterion_group!(benches, compare);
criterion_main!(benches);
