#![no_main]

//! Every search answerer agrees with the scalar reference, for every
//! request, over arbitrary bytes and ranges. The first two bytes pick the
//! range, the third the needle length.

use libfuzzer_sys::fuzz_target;
use mime::tree::scan::{memchr, scalar};

fuzz_target!(|data: &[u8]| {
    let [a, b, n, input @ ..] = data else {
        return;
    };

    let len = input.len();
    let start = (*a as usize).min(len);
    let end = (start + *b as usize * 4).min(len);
    let needle = &input[start..(start + *n as usize % 8).min(end)];

    let (mut ours, mut theirs) = (Vec::new(), Vec::new());

    scalar::field_ends(input, start..end, &mut ours);
    memchr::field_ends(input, start..end, &mut theirs);
    assert_eq!(ours, theirs, "field ends");

    ours.clear();
    theirs.clear();
    scalar::dash_lines(input, start..end, &mut ours);
    memchr::dash_lines(input, start..end, &mut theirs);
    assert_eq!(ours, theirs, "dash lines");

    ours.clear();
    theirs.clear();
    scalar::find(input, start..end, needle, &mut ours);
    memchr::find(input, start..end, needle, &mut theirs);
    assert_eq!(ours, theirs, "find");
});
