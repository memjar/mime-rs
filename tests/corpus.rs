//! Round-trip and structure sweep over the vendored corpus.
//!
//! Every fixture, as written and in its LF and CRLF forms, must parse,
//! serialize back to itself byte for byte, parse the same through the
//! scalar and the memchr answerers, and, where it has a frozen answer,
//! carry the structure that answer states unless a divergence row says
//! otherwise.

#![cfg(feature = "parser")]

mod common;

use mime::tree::{cst::MimeCst, parse::MimeParseOptions};

use crate::common::{
    Fixture, divergences, fixtures, frozen_shapes, line_endings, parse_scalar, shapes,
};

const CORPORA: [(&str, usize); 6] = [
    ("dovecot-malformed", 24),
    ("dovecot-thirdparty", 20),
    ("stalwart-issues", 18),
    ("stalwart-regressions", 75),
    ("stalwart-rfc", 10),
    ("stalwart-sieve", 6),
];

fn round_trips(name: &str, input: &[u8]) {
    let cst = MimeCst::parse(input);
    assert_eq!(cst.to_bytes(), input, "not byte-faithful: {name}");
    assert_eq!(cst, parse_scalar(input), "answerers disagree: {name}");

    let bytes = cst.to_bytes();
    assert_eq!(
        MimeCst::parse(&bytes).to_bytes(),
        bytes,
        "not a fixpoint: {name}"
    );

    for depth in 0..3 {
        let opts = MimeParseOptions {
            structure_depth: Some(depth),
            ..Default::default()
        };
        let mut lazy = MimeCst::parse_with(input, opts);
        assert_eq!(lazy.to_bytes(), input, "lazy not byte-faithful: {name}");

        lazy.expand(MimeParseOptions::default());
        assert_eq!(lazy, cst, "expanded differs at depth {depth}: {name}");
    }
}

#[test]
fn every_fixture_round_trips_in_every_line_ending() {
    for (corpus, count) in CORPORA {
        for fixture in fixtures(corpus, count) {
            let (lf, crlf) = line_endings(&fixture.bytes);

            round_trips(&fixture.name, &fixture.bytes);
            round_trips(&format!("{} (lf)", fixture.name), &lf);
            round_trips(&format!("{} (crlf)", fixture.name), &crlf);
        }
    }
}

#[test]
fn structure_agrees_with_the_frozen_answers() {
    let divergences = divergences();
    let mut failures = Vec::new();

    for (corpus, count) in CORPORA {
        for Fixture {
            name,
            bytes,
            lf,
            crlf,
        } in fixtures(corpus, count)
        {
            let (lf_bytes, crlf_bytes) = line_endings(&bytes);

            for (form, input, answer) in [("lf", &lf_bytes, lf), ("crlf", &crlf_bytes, crlf)] {
                let Some(answer) = answer else {
                    continue;
                };

                let ours = shapes(&MimeCst::parse(input));
                let theirs = frozen_shapes(&answer);
                let path = format!("{form}.structure");

                if ours != theirs && !divergences.contains(&(name.clone(), path.clone())) {
                    let first = ours
                        .iter()
                        .zip(&theirs)
                        .position(|(a, b)| a != b)
                        .unwrap_or(ours.len().min(theirs.len()));

                    failures.push(format!(
                        "{name} {path}: {} parts against {}, first difference at part {first}: ours {:?}, theirs {:?}",
                        ours.len(),
                        theirs.len(),
                        ours.get(first),
                        theirs.get(first),
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} structure differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
