//! Structural behaviour of the parse coroutine: its requests, its checks of
//! the answers it gets, its bounds and its linearity.

#![cfg(feature = "parser")]

mod common;

use std::{thread, time::Instant};

use mime::tree::{
    anomaly::MimeAnomaly,
    body::MimeBodyNode,
    coroutine::MimeCoroutineState,
    cst::MimeCst,
    parse::{MimeParse, MimeParseOptions, MimeParseYield},
    scan::memchr,
};

use crate::common::{fixtures, line_endings, shapes};

#[test]
fn a_nested_multipart_costs_one_request_per_block_and_one_for_the_body() {
    let mut input = String::new();

    for i in 0..30 {
        input.push_str(&format!("X-Field-{i}: value\r\n"));
    }

    input.push_str(concat!(
        "Content-Type: multipart/mixed; boundary=outer\r\n",
        "\r\n",
        "--outer\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "one\r\n",
        "--outer\r\n",
        "Content-Type: multipart/alternative; boundary=inner\r\n",
        "\r\n",
        "--inner\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "two\r\n",
        "--inner\r\n",
        "Content-Type: text/html\r\n",
        "\r\n",
        "<p>two</p>\r\n",
        "--inner--\r\n",
        "--outer\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "three\r\n",
        "--outer--\r\n",
    ));

    let input = input.as_bytes();
    let mut parse = MimeParse::new(input, MimeParseOptions::default());
    let mut found = Vec::new();
    let mut arg = None;
    let (mut field_ends, mut dash_lines, mut finds) = (0, 0, 0);

    let cst = loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { range }) => {
                field_ends += 1;
                found.clear();
                memchr::field_ends(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsDashLines { range }) => {
                dash_lines += 1;
                found.clear();
                memchr::dash_lines(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsFind { .. }) => {
                finds += 1;
                arg = Some(&[]);
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    };

    assert_eq!(cst.entities().len(), 6);
    assert_eq!((field_ends, dash_lines, finds), (6, 1, 0));
    assert_eq!(cst.to_bytes(), input);
}

/// Answers every request with positions from a small generator, so most of
/// them fail their check.
fn parse_with_noise(input: &[u8], seed: u64) -> MimeCst<'_> {
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let mut parse = MimeParse::new(input, MimeParseOptions::default());
    let mut found = Vec::new();
    let mut arg = None;

    loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(
                MimeParseYield::WantsFieldEnds { range }
                | MimeParseYield::WantsDashLines { range }
                | MimeParseYield::WantsFind { range, .. },
            ) => {
                found.clear();
                let span = (range.end - range.start + 8) as u64;

                for _ in 0..(next() % 12) {
                    found.push(range.start.saturating_sub(4) + (next() % span) as usize);
                }

                if next() % 2 == 0 {
                    found.sort_unstable();
                }

                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    }
}

#[test]
fn a_noisy_answerer_never_breaks_the_round_trip() {
    for fixture in fixtures("stalwart-regressions", 75) {
        let (_, crlf) = line_endings(&fixture.bytes);

        for seed in 0..8 {
            for input in [&fixture.bytes, &crlf] {
                let cst = parse_with_noise(input, seed);
                assert_eq!(&cst.to_bytes(), input, "{} seed {seed}", fixture.name);
            }
        }
    }
}

#[test]
fn a_wrong_position_is_dropped_and_reported() {
    let input = b"Subject: x\r\n\r\nbody";
    let mut parse = MimeParse::new(input, MimeParseOptions::default());

    let MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { .. }) = parse.resume(None)
    else {
        panic!("a field-ends request");
    };

    let MimeCoroutineState::Complete(cst) = parse.resume(Some(&[3, 11, 2])) else {
        panic!("complete");
    };

    assert_eq!(cst.to_bytes(), input);
    assert!(cst.root.anomalies.contains(MimeAnomaly::DroppedAnswer));
    assert!(cst.root.header.get("subject").is_some());
}

fn nested(depth: usize) -> Vec<u8> {
    let mut out = Vec::new();

    for i in 0..depth {
        out.extend_from_slice(
            format!("Content-Type: multipart/mixed; boundary=b{i}\r\n\r\n--b{i}\r\n").as_bytes(),
        );
    }

    out.extend_from_slice(b"Content-Type: text/plain\r\n\r\nleaf\r\n");

    for i in (0..depth).rev() {
        out.extend_from_slice(format!("--b{i}--\r\n").as_bytes());
    }

    out
}

#[test]
fn ten_thousand_nested_multiparts_parse_on_a_small_stack() {
    let input = nested(10_000);

    thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let cst = MimeCst::parse(&input);
            let entities = cst.entities();

            assert_eq!(cst.to_bytes(), input);
            // NOTE: 64 containers nest; the 65th entity is kept as a leaf.
            assert_eq!(entities.len(), 65);
            assert!(entities[64].anomalies.contains(MimeAnomaly::NestingLimit));
            assert!(matches!(entities[64].body, MimeBodyNode::Leaf(_)));
            assert!(matches!(entities[63].body, MimeBodyNode::Multipart(_)));

            drop(cst);
        })
        .expect("thread")
        .join()
        .expect("no stack overflow");
}

#[test]
fn the_entity_bound_keeps_a_multipart_whole() {
    let mut input = b"Content-Type: multipart/mixed; boundary=b\r\n\r\n".to_vec();

    for _ in 0..50 {
        input.extend_from_slice(b"--b\r\n\r\npart\r\n");
    }

    input.extend_from_slice(b"--b--\r\n");

    let opts = MimeParseOptions {
        max_entities: Some(10),
        ..Default::default()
    };
    let cst = MimeCst::parse_with(&input, opts);

    assert_eq!(cst.to_bytes(), input);
    assert!(cst.root.anomalies.contains(MimeAnomaly::PartLimit));
    assert!(matches!(cst.root.body, MimeBodyNode::Leaf(_)));
}

fn timed(input: &[u8]) -> f64 {
    let start = Instant::now();

    for _ in 0..3 {
        std::hint::black_box(MimeCst::parse(input));
    }

    start.elapsed().as_secs_f64()
}

#[test]
fn near_miss_delimiters_parse_in_linear_time() {
    let body = |lines: usize| {
        let mut input = b"Content-Type: multipart/mixed; boundary=BND\r\n\r\n".to_vec();

        for _ in 0..lines {
            input.extend_from_slice(b"--BNDX\r\n");
        }

        input
    };

    let small = timed(&body(100_000));
    let large = timed(&body(200_000));

    assert!(
        large < small * 4.0,
        "doubling took {large}s against {small}s"
    );
}

#[test]
fn colon_less_folded_lines_parse_in_linear_time() {
    let block = |lines: usize| {
        let mut input = b"Subject: x\r\n".to_vec();

        for _ in 0..lines {
            input.extend_from_slice(b"garbage line\r\n folded\r\n");
        }

        input.extend_from_slice(b"\r\nbody");
        input
    };

    let small = timed(&block(50_000));
    let large = timed(&block(100_000));

    assert!(
        large < small * 4.0,
        "doubling took {large}s against {small}s"
    );
}

#[test]
fn issues_and_sieve_fixtures_match_mail_parser_structure() {
    for (corpus, count) in [("stalwart-issues", 18), ("stalwart-sieve", 6)] {
        for fixture in fixtures(corpus, count) {
            let message = mail_parser::MessageParser::new()
                .parse(&fixture.bytes)
                .expect("mail-parser reads it");

            let theirs: Vec<_> = message
                .parts()
                .map(|part| {
                    (
                        part.offset_header() as usize,
                        part.offset_body() as usize,
                        part.offset_end() as usize,
                    )
                })
                .collect();

            let ours: Vec<_> = shapes(&MimeCst::parse(&fixture.bytes))
                .into_iter()
                .map(|shape| (shape.offset_header, shape.offset_body, shape.offset_end))
                .collect();

            assert_eq!(ours, theirs, "{}", fixture.name);
        }
    }
}

#[test]
fn a_parse_at_structure_depth_zero_reads_the_root_header_block_alone() {
    let input = concat!(
        "Subject: hi\r\n",
        "Content-Type: multipart/mixed; boundary=b\r\n",
        "\r\n",
        "--b\r\n",
        "\r\n",
        "one\r\n",
        "--b--\r\n",
    )
    .as_bytes();

    let opts = MimeParseOptions {
        structure_depth: Some(0),
        ..Default::default()
    };
    let mut parse = MimeParse::new(input, opts);
    let mut found = Vec::new();
    let mut arg = None;
    let mut requests = Vec::new();

    let mut cst = loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(request) => {
                found.clear();

                if let MimeParseYield::WantsFieldEnds { range } = &request {
                    memchr::field_ends(input, range.clone(), &mut found);
                }

                requests.push(request);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    };

    assert_eq!(requests.len(), 1);
    assert!(matches!(requests[0], MimeParseYield::WantsFieldEnds { .. }));
    assert!(matches!(cst.root.body, MimeBodyNode::Unparsed(_)));
    assert_eq!(cst.decode(), MimeCst::parse(input).decode());

    cst.expand(MimeParseOptions::default());
    assert_eq!(cst, MimeCst::parse(input));
}
