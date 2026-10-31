//! No field value makes its reader or its writer superlinear: doubling a
//! hostile value at most about doubles the time it takes.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

use std::time::Instant;

use mime::{
    build::MimeBuildOptions,
    header::spec::MimeHeaderForm,
    mime::{Mime, MimeBody},
};

fn timed(work: impl Fn()) -> f64 {
    let start = Instant::now();

    for _ in 0..3 {
        work();
    }

    start.elapsed().as_secs_f64()
}

fn assert_linear(name: &str, value: impl Fn(usize) -> Vec<u8>, form: MimeHeaderForm) {
    let (small, large) = (value(20_000), value(40_000));
    let small = timed(|| {
        std::hint::black_box(form.parse(&small));
    });
    let large = timed(|| {
        std::hint::black_box(form.parse(&large));
    });

    assert!(
        large < small * 4.0 + 0.01,
        "{name}: doubling took {large}s against {small}s"
    );
}

fn repeat(piece: &str, count: usize) -> Vec<u8> {
    piece.repeat(count).into_bytes()
}

#[test]
fn hostile_values_read_in_linear_time() {
    assert_linear("word starts", |n| repeat("=?", n), MimeHeaderForm::Text);
    assert_linear(
        "unterminated words",
        |n| repeat("=?utf-8?q?a ", n),
        MimeHeaderForm::Text,
    );
    assert_linear(
        "encoded word runs",
        |n| repeat("=?utf-8?q?a?= ", n),
        MimeHeaderForm::Text,
    );
    assert_linear(
        "RFC 2231 sections",
        |n| {
            let mut value = b"text/plain".to_vec();

            for i in 0..n {
                value.extend_from_slice(format!("; name*{i}*=utf-8''%41").as_bytes());
            }

            value
        },
        MimeHeaderForm::ContentType,
    );
    assert_linear(
        "parameters",
        |n| repeat("; a=b", n),
        MimeHeaderForm::ContentType,
    );
    assert_linear(
        "addresses",
        |n| repeat("a@b.c, ", n),
        MimeHeaderForm::GroupedAddresses,
    );
    assert_linear(
        "bare words",
        |n| repeat("a ", n),
        MimeHeaderForm::GroupedAddresses,
    );
    assert_linear(
        "nested comments",
        |n| repeat("(", n),
        MimeHeaderForm::Addresses,
    );
    assert_linear(
        "groups",
        |n| repeat("g: a@b.c; ", n),
        MimeHeaderForm::GroupedAddresses,
    );
    assert_linear(
        "identifiers",
        |n| repeat("<a@b> ", n),
        MimeHeaderForm::MessageIds,
    );
    assert_linear(
        "comma list",
        |n| repeat("a, ", n),
        MimeHeaderForm::CommaList,
    );
    assert_linear("urls", |n| repeat("<a>, ", n), MimeHeaderForm::Urls);
    assert_linear("folded date", |n| repeat("\r\n ", n), MimeHeaderForm::Date);
}

#[test]
fn hostile_text_writes_in_linear_time() {
    let write = |text: String| {
        let mime = Mime {
            headers: vec![mime::header::MimeHeader {
                name: mime::header::MimeHeaderName::new("Subject"),
                value: mime::value::MimeHeaderValue::Text(text.clone()),
            }],
            body: MimeBody::Text(text),
        };

        move || {
            std::hint::black_box(mime.to_bytes(&MimeBuildOptions::default()).unwrap());
        }
    };

    for piece in ["é", "a ", "=?", "\r", "\r\n", "--=_mime-rs_ "] {
        let small = timed(write(piece.repeat(20_000)));
        let large = timed(write(piece.repeat(40_000)));

        assert!(
            large < small * 4.0 + 0.01,
            "{piece:?}: doubling took {large}s against {small}s"
        );
    }
}
