//! The Stalwart field vectors: every header value in tests/vectors/stalwart
//! read through this crate's readers and compared with its expected
//! reading, unless tests/vectors/divergence.tsv holds a row for it.

#![cfg(feature = "parser")]

use std::{fs, path::PathBuf};

use mime::{
    header::spec::MimeHeaderForm,
    tree::field::value::parse,
    value::{MimeHeaderValue, address::MimeAddress, subject::base_subject},
};
use serde_json::{Value, json};

fn vectors(file: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/stalwart")
        .join(file);

    serde_json::from_slice(&fs::read(path).expect("vector file")).expect("vector json")
}

fn divergences() -> Vec<(String, usize)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/divergence.tsv");

    fs::read_to_string(path)
        .expect("divergence file")
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let vector = columns.next()?.to_string();
            let index = columns.next()?.parse().ok()?;
            Some((vector, index))
        })
        .collect()
}

/// Compares every vector of `file`, returning the differences not
/// recorded as divergences.
fn sweep(file: &str, input: &str, ours: impl Fn(&Value) -> Value) -> Vec<String> {
    let divergences = divergences();

    vectors(file)
        .iter()
        .enumerate()
        .filter_map(|(index, vector)| {
            let got = ours(&vector[input]);
            let expected = &vector["expected"];

            (strip(got.clone()) != strip(expected.clone())
                && !divergences.contains(&(file.to_string(), index)))
            .then(|| {
                format!(
                    "{file}[{index}] {}: ours {got}, theirs {expected}",
                    vector[input]
                )
            })
        })
        .collect()
}

fn bytes(value: &Value) -> Vec<u8> {
    value.as_str().expect("a string input").as_bytes().to_vec()
}

fn mailbox(name: &Option<String>, address: &Option<String>) -> Value {
    json!({ "name": name, "address": address })
}

#[test]
fn every_field_vector_reads_as_expected_or_diverges_on_record() {
    let mut failures = Vec::new();

    failures.extend(sweep("unstructured.json", "header", |input| {
        match parse(MimeHeaderForm::Text, &bytes(input)) {
            MimeHeaderValue::Text(text) => json!(text),
            other => json!(format!("{other:?}")),
        }
    }));

    failures.extend(sweep("id.json", "header", |input| {
        let ids = parse(MimeHeaderForm::MessageIds, &bytes(input));
        let ids = ids.as_message_ids().unwrap_or_default();
        if ids.is_empty() {
            Value::Null
        } else {
            json!(ids)
        }
    }));

    failures.extend(sweep("list.json", "header", |input| {
        let items = parse(MimeHeaderForm::CommaList, &bytes(input));
        let items = items.as_comma_list().unwrap_or_default();
        if items.is_empty() {
            Value::Null
        } else {
            json!(items)
        }
    }));

    failures.extend(sweep("date.json", "header", |input| {
        match parse(MimeHeaderForm::Date, &bytes(input)) {
            MimeHeaderValue::Date(date) => json!({
                "year": date.year,
                "month": date.month,
                "day": date.day,
                "hour": date.hour,
                "minute": date.minute,
                "second": date.second,
                "tz_before_gmt": date.offset < 0,
                "tz_hour": date.offset.unsigned_abs() / 60,
                "tz_minute": date.offset.unsigned_abs() % 60,
            }),
            _ => Value::Null,
        }
    }));

    failures.extend(sweep("address.json", "header", |input| {
        let list = parse(MimeHeaderForm::GroupedAddresses, &bytes(input));
        let items: Vec<Value> = list
            .as_addresses()
            .map(|list| list.items.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|item| match item {
                MimeAddress::Mailbox(m) => json!({ "mailbox": mailbox(&m.name, &m.address) }),
                MimeAddress::Group(g) => json!({ "group": {
                    "name": g.name,
                    "mailboxes": g.mailboxes.iter().map(|m| mailbox(&m.name, &m.address)).collect::<Vec<_>>(),
                }}),
            })
            .collect();

        if items.is_empty() { Value::Null } else { json!(items) }
    }));

    failures.extend(sweep("content_type.json", "header", |input| {
        let MimeHeaderValue::ContentType(ct) = parse(MimeHeaderForm::ContentType, &bytes(input))
        else {
            return Value::Null;
        };

        let attributes: Vec<Value> = ct
            .params
            .iter()
            .map(|param| json!([param.name.to_ascii_lowercase(), param.value]))
            .collect();

        json!({
            "c_type": (!ct.top.is_empty()).then(|| ct.top.to_ascii_lowercase()),
            "c_subtype": (!ct.sub.is_empty()).then(|| ct.sub.to_ascii_lowercase()),
            "attributes": (!attributes.is_empty()).then_some(attributes),
        })
    }));

    failures.extend(sweep("thread_name.json", "input", |input| {
        json!(base_subject(input.as_str().expect("a string")))
    }));

    failures.extend(sweep("trailing_fwd.json", "input", |input| {
        json!(base_subject(input.as_str().expect("a string")))
    }));

    assert!(
        failures.is_empty(),
        "{} vector differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Drops null members, so an absent member and a null one compare equal,
/// and reads an object whose members are all null as null.
fn strip(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let map: serde_json::Map<String, Value> = map
                .into_iter()
                .map(|(key, value)| (key, strip(value)))
                .filter(|(_, value)| !value.is_null())
                .collect();

            if map.is_empty() {
                Value::Null
            } else {
                Value::Object(map)
            }
        }
        Value::Array(items) => Value::Array(items.into_iter().map(strip).collect()),
        other => other,
    }
}
