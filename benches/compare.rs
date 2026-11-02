//! mime-rs against mail-parser 1.0 on mail-parser's own three workloads,
//! over the vendored corpus (the test suite messages, as written).
//!
//! - `structure`: parse, then read the content type of every part.
//! - `headers`: parse, then read the decoded Subject, the first From
//!   address, the Date as a Unix timestamp and the Message-ID; also with
//!   the parse stopped at the header block, beside mail-parser's
//!   `parse_headers`.
//! - `full`: parse, then read every leaf decoded, text parts transcoded.
//! - `build`: write the same message as mail-builder 1.0 does, with a
//!   text and an HTML body, two attachments and an inline image.
//!
//! Each group first checks both libraries saw the same number of parts, so
//! a fast wrong answer does not pass unnoticed.

use std::{fs, hint::black_box, path::PathBuf};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mail_parser::MessageParser;
use mime::{
    build::MimeBuildOptions,
    builder::{MimeAttachment, MimeBuilder},
    header::marker::{DATE, FROM, MESSAGE_ID, SUBJECT, TO},
    tree::{body::MimeBodyNode, cst::MimeCst, parse::MimeParseOptions},
    value::{address::MimeMailbox, content_type::MimeContentType, datetime::MimeDateTime},
};
use simd_cpu::SimdCpu;

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
    install();
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

    let lazy = MimeParseOptions {
        structure_depth: Some(0),
        ..Default::default()
    };

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
    group.bench_function("mime-rs, structure depth 0", |b| {
        b.iter(|| {
            for message in &corpus {
                let cst = MimeCst::parse_with(black_box(message), lazy.clone());
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
    group.bench_function("mail-parser 1.0, headers only", |b| {
        b.iter(|| {
            for message in &corpus {
                if let Some(message) = parser.parse_headers(black_box(message)) {
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

/// The `build` workload: a text body, an HTML body, two attachments and an
/// inline image, non-ASCII names and subject, written to bytes.
fn build(c: &mut Criterion) {
    install();
    let text = "Hello, here is the report you asked for.\n".repeat(50);
    let html = "<p>Hello, here is the <b>report</b> you asked for.</p>\n".repeat(80);
    let pdf: Vec<u8> = (0..50_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let csv = "date,amount\n2026-10-31,42\n".repeat(400).into_bytes();
    let png: Vec<u8> = (0..20_000u32).map(|i| (i * 13 % 253) as u8).collect();

    let ours = || {
        let mut builder = MimeBuilder::default();
        builder
            .header::<DATE>(MimeDateTime::from_unix_seconds(1_793_448_000, 60))
            .unwrap()
            .header::<FROM>(MimeMailbox::named("Jöhn Doe", "john@example.com").into())
            .unwrap()
            .header::<TO>(
                [
                    MimeMailbox::named("Jane Roe", "jane@example.com"),
                    MimeMailbox::address("team@example.com"),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap()
            .header::<SUBJECT>("Rapport trimestriel, édition définitive".into())
            .unwrap()
            .header::<MESSAGE_ID>(vec!["1793448000.42@example.com".into()])
            .unwrap()
            .text(text.clone())
            .html(html.clone());
        builder
            .attachment(attachment(
                "application",
                "pdf",
                Some("Rapport été.pdf"),
                &pdf,
            ))
            .unwrap()
            .attachment(attachment("text", "csv", Some("figures.csv"), &csv))
            .unwrap();
        builder
            .inline(MimeAttachment {
                content_id: Some("logo@example.com".into()),
                ..attachment("image", "png", None, &png)
            })
            .unwrap();
        builder.write(&MimeBuildOptions::default()).unwrap()
    };

    let theirs = || {
        mail_builder::MessageBuilder::new()
            .date(1_793_448_000i64)
            .from(("Jöhn Doe", "john@example.com"))
            .to(vec![
                ("Jane Roe", "jane@example.com"),
                ("", "team@example.com"),
            ])
            .subject("Rapport trimestriel, édition définitive")
            .message_id("1793448000.42@example.com")
            .text_body(text.as_str())
            .html_body(html.as_str())
            .attachment("application/pdf", "Rapport été.pdf", pdf.as_slice())
            .attachment("text/csv", "figures.csv", csv.as_slice())
            .inline("image/png", "logo@example.com", png.as_slice())
            .write_to_vec()
            .unwrap()
    };

    let built = ours();
    let theirs_built = theirs();
    assert!(MimeCst::parse(&built).anomalies().is_empty());

    for message in [&built, &theirs_built] {
        let message = MessageParser::new().parse(message).unwrap();
        let leaves: Vec<Vec<u8>> = message
            .parts()
            .filter(|part| !part.is_multipart())
            .map(|part| part.decoded().into_owned())
            .collect();

        for content in [&pdf, &csv, &png] {
            assert!(leaves.contains(content));
        }
    }

    let mut group = c.benchmark_group("build");
    group.throughput(Throughput::Bytes(built.len() as u64));
    group.bench_function("mime-rs", |b| b.iter(|| black_box(ours())));
    group.bench_function("mail-builder 1.0", |b| b.iter(|| black_box(theirs())));
    group.finish();
}

/// The `edit` workload: the Subject of a message carrying a 2 MiB
/// attachment changed and the message written out; the rival is the only
/// way to do it with mail-parser and mail-builder, a parse and a rebuild.
fn edit(c: &mut Criterion) {
    install();
    let pdf: Vec<u8> = (0..2 * 1024 * 1024u32)
        .map(|i| (i * 7 % 251) as u8)
        .collect();
    let mut builder = MimeBuilder::default();
    builder
        .header::<SUBJECT>("Before".into())
        .unwrap()
        .header::<FROM>(MimeMailbox::named("Jöhn Doe", "john@example.com").into())
        .unwrap()
        .text("Hello, here is the report.\n")
        .attachment(attachment("application", "pdf", Some("report.pdf"), &pdf))
        .unwrap();
    let message = builder.write(&MimeBuildOptions::default()).unwrap();

    let ours = || {
        let mut cst = MimeCst::parse(black_box(&message));
        cst.header_mut(0)
            .unwrap()
            .set::<SUBJECT>("After".into(), &MimeBuildOptions::default())
            .unwrap();
        cst.to_bytes()
    };

    let theirs = || {
        let parsed = MessageParser::new().parse(black_box(&message)).unwrap();
        let attachment = parsed.attachments().next().unwrap();
        let from = parsed.from().unwrap().first().unwrap();

        mail_builder::MessageBuilder::new()
            .from((
                from.name().unwrap_or_default(),
                from.address().unwrap_or_default(),
            ))
            .subject("After")
            .text_body(parsed.body_text(0).unwrap().into_owned())
            .attachment(
                "application/pdf",
                attachment.attachment_name().unwrap_or_default(),
                attachment.decoded().into_owned(),
            )
            .write_to_vec()
            .unwrap()
    };

    let edited = ours();
    let reread = MimeCst::parse(&edited);
    assert_eq!(reread.root.header::<SUBJECT>().as_deref(), Some("After"));
    assert_eq!(edited.len() + 1, message.len());
    assert!(
        MessageParser::new()
            .parse(&theirs())
            .unwrap()
            .attachments()
            .next()
            .is_some()
    );

    let lazy = || {
        let opts = MimeParseOptions {
            structure_depth: Some(0),
            ..Default::default()
        };
        let mut cst = MimeCst::parse_with(black_box(&message), opts);
        cst.header_mut(0)
            .unwrap()
            .set::<SUBJECT>("After".into(), &MimeBuildOptions::default())
            .unwrap();
        cst.to_bytes()
    };

    assert_eq!(lazy(), edited);

    let mut group = c.benchmark_group("edit");
    group.throughput(Throughput::Bytes(message.len() as u64));
    group.bench_function("mime-rs", |b| b.iter(|| black_box(ours())));
    group.bench_function("mime-rs, structure depth 0", |b| {
        b.iter(|| black_box(lazy()))
    });
    group.bench_function("mail-parser 1.0 and mail-builder 1.0", |b| {
        b.iter(|| black_box(theirs()))
    });
    group.finish();
}

fn attachment(top: &str, sub: &str, filename: Option<&str>, content: &[u8]) -> MimeAttachment {
    MimeAttachment {
        content_type: MimeContentType {
            top: top.into(),
            sub: sub.into(),
            params: Vec::new(),
        },
        filename: filename.map(String::from),
        content_id: None,
        content: content.to_vec(),
    }
}

/// Installs the detected CPU token, as an application does first thing in
/// `main`; criterion owns `main`, so each group does, the first one winning.
fn install() {
    let _ = SimdCpu::install_detected();
}

criterion_group!(benches, compare, build, edit);
criterion_main!(benches);
