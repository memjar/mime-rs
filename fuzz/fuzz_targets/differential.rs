#![no_main]

//! Coverage-guided differential target: arbitrary bytes read by this crate
//! and by mail-parser 1.0 agree on the part tree, the decoded content and
//! decode problems, the decoded header values and the body lists, but where
//! a named rule of tests/common/differential.rs lets a deliberate
//! difference through.

#[path = "../../tests/common/differential.rs"]
mod differential;

use libfuzzer_sys::fuzz_target;
use mail_parser::MessageParser;

fuzz_target!(|data: &[u8]| {
    let answer = MessageParser::new()
        .parse(data)
        .map(|message| serde_json::to_value(&message).expect("serializes"));
    let outcome = differential::compare(data, answer.as_ref(), false);

    assert!(
        outcome.differences.is_empty(),
        "{:#?}",
        outcome.differences
    );
});
