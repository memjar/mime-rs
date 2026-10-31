//! Shared corpus harness.
//!
//! Every corpus is a directory of `.eml` fixtures under tests/corpus/, with
//! provenance in its ATTRIBUTION.md. Fixtures with frozen answers carry
//! `NAME.json` (mail-parser 1.0's reading of the LF form) and
//! `NAME.crlf.json` (of the CRLF form). tests/corpus/divergence.tsv lists the
//! readings this crate deliberately does not share, one row per fixture and
//! path.

// NOTE: each integration test compiles this module and uses part of it.
#![allow(dead_code)]

use std::{fs, path::PathBuf};

use mime::tree::{
    body::MimeBodyNode,
    coroutine::MimeCoroutineState,
    cst::MimeCst,
    entity::MimeEntity,
    header::MimeHeaderItem,
    parse::{MimeParse, MimeParseOptions, MimeParseYield},
    scan::scalar,
};
use serde_json::Value;

/// One fixture: its name, raw bytes and frozen answers, if any.
pub struct Fixture {
    pub name: String,
    pub bytes: Vec<u8>,
    pub lf: Option<Value>,
    pub crlf: Option<Value>,
}

/// Every `.eml` fixture of `corpus`, sorted by name, asserting exactly
/// `expected` are present so a misfiled or newly added fixture is caught.
pub fn fixtures(corpus: &str, expected: usize) -> Vec<Fixture> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(corpus);

    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("corpus directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("name")
        })
        .filter(|name| name.ends_with(".eml"))
        .collect();

    names.sort();
    assert_eq!(names.len(), expected, "fixture count of {corpus}");

    names
        .into_iter()
        .map(|file| {
            let stem = file.trim_end_matches(".eml");
            let json = |suffix: &str| {
                fs::read(dir.join(format!("{stem}{suffix}")))
                    .ok()
                    .map(|bytes| serde_json::from_slice(&bytes).expect("frozen answer"))
            };

            Fixture {
                name: format!("{corpus}/{stem}"),
                bytes: fs::read(dir.join(&file)).expect("fixture"),
                lf: json(".json"),
                crlf: json(".crlf.json"),
            }
        })
        .collect()
}

/// The LF form (every CR removed) and the CRLF form (every LF preceded by a
/// CR) of a fixture, as mail-parser's own harness derives them.
pub fn line_endings(raw: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let lf: Vec<u8> = raw.iter().copied().filter(|&byte| byte != b'\r').collect();
    let mut crlf = Vec::with_capacity(lf.len() * 2);

    for &byte in &lf {
        if byte == b'\n' {
            crlf.push(b'\r');
        }

        crlf.push(byte);
    }

    (lf, crlf)
}

/// Parses through the scalar answerer instead of memchr.
pub fn parse_scalar(input: &[u8]) -> MimeCst<'_> {
    let mut parse = MimeParse::new(input, MimeParseOptions::default());
    let mut found = Vec::new();
    let mut arg = None;

    loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { range }) => {
                found.clear();
                scalar::field_ends(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsDashLines { range }) => {
                found.clear();
                scalar::dash_lines(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsFind { range, needle }) => {
                found.clear();
                scalar::find(input, range, &needle, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    }
}

/// The structure of one entity as mail-parser numbers parts: its parent,
/// whether it is a container, and its header, body and end offsets.
#[derive(Debug, PartialEq, Eq)]
pub struct Shape {
    pub parent: Option<usize>,
    pub kind: &'static str,
    pub offset_header: usize,
    pub offset_body: usize,
    pub offset_end: usize,
}

/// The shapes of every entity, in document order.
pub fn shapes(cst: &MimeCst<'_>) -> Vec<Shape> {
    let mut out = Vec::new();
    let mut offset = 0;
    walk(&cst.root, None, &mut offset, &mut out);
    out
}

fn walk(entity: &MimeEntity<'_>, parent: Option<usize>, offset: &mut usize, out: &mut Vec<Shape>) {
    let index = out.len();
    let offset_header = *offset;
    let header: usize = entity
        .header
        .items
        .iter()
        .map(|item| match item {
            MimeHeaderItem::Field(field) => {
                field.name.len() + field.colon.len() + field.value.len() + field.eol.len()
            }
            MimeHeaderItem::Opaque(line) => line.len(),
        })
        .sum::<usize>()
        + entity.header.end.len();

    let kind = match &entity.body {
        MimeBodyNode::Leaf(_) => "leaf",
        MimeBodyNode::Multipart(_) => "multipart",
        MimeBodyNode::Message(_) => "message",
    };

    out.push(Shape {
        parent,
        kind,
        offset_header,
        offset_body: offset_header + header,
        offset_end: 0,
    });

    *offset += header;

    match &entity.body {
        MimeBodyNode::Leaf(bytes) => *offset += bytes.len(),
        MimeBodyNode::Multipart(multipart) => {
            *offset += multipart.preamble.len();

            for part in &multipart.parts {
                *offset += part.delimiter.len();
                walk(&part.entity, Some(index), offset, out);
            }

            *offset += multipart.close.len() + multipart.epilogue.len();
        }
        // NOTE: mail-parser gives a nested message's root no parent part.
        MimeBodyNode::Message(nested) => walk(nested, None, offset, out),
    }

    out[index].offset_end = *offset;
}

/// The shapes a frozen answer states.
pub fn frozen_shapes(answer: &Value) -> Vec<Shape> {
    answer["parts"]
        .as_array()
        .expect("parts")
        .iter()
        .map(|part| Shape {
            parent: part["parent"].as_u64().map(|parent| parent as usize),
            kind: match part["kind"].as_str() {
                Some("multipart") => "multipart",
                Some("message") => "message",
                _ => "leaf",
            },
            offset_header: part["offset_header"].as_u64().expect("offset") as usize,
            offset_body: part["offset_body"].as_u64().expect("offset") as usize,
            offset_end: part["offset_end"].as_u64().expect("offset") as usize,
        })
        .collect()
}

/// The divergence rows of tests/corpus/divergence.tsv: fixture, path.
pub fn divergences() -> Vec<(String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/divergence.tsv");

    fs::read_to_string(path)
        .expect("divergence file")
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut columns = line.split('\t');
            let fixture = columns.next().unwrap_or_default().to_string();
            let path = columns.next().unwrap_or_default().to_string();
            (fixture, path)
        })
        .collect()
}
