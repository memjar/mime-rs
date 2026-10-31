#![no_main]

//! The parse coroutine resumed with arbitrary positions: it never panics and
//! the tree still round-trips. The input is a message, a NUL byte, then the
//! answer bytes, read two at a time as positions.

use libfuzzer_sys::fuzz_target;
use mime::tree::{
    coroutine::MimeCoroutineState,
    parse::{MimeParse, MimeParseOptions},
};

fuzz_target!(|data: &[u8]| {
    let split = data.iter().position(|byte| *byte == 0).unwrap_or(data.len());
    let (input, answers) = data.split_at(split);
    let mut positions = answers.chunks(2).map(|pair| pair.iter().fold(0usize, |n, b| n << 8 | *b as usize));

    let mut parse = MimeParse::new(input, MimeParseOptions::default());
    let mut found = Vec::new();
    let mut arg = None;

    let cst = loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(_) => {
                found.clear();
                found.extend(positions.by_ref().take(4));
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    };

    assert_eq!(cst.to_bytes(), input, "not byte-faithful");
});
