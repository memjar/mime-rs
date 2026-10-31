#![no_main]

//! Coverage-guided fuzz target for the writer. The model of arbitrary
//! bytes, as parsed and decoded, is written; two oracles: writing never
//! panics, and once written the message rebuilds to the same bytes.

use libfuzzer_sys::fuzz_target;
use mime::{
    build::{MimeBuildOptions, MimeLineEnding},
    tree::cst::MimeCst,
};

fuzz_target!(|data: &[u8]| {
    let Some((&flags, data)) = data.split_first() else {
        return;
    };

    let opts = MimeBuildOptions {
        line_ending: if flags & 1 == 0 {
            MimeLineEnding::Crlf
        } else {
            MimeLineEnding::Lf
        },
        allow_8bit: flags & 2 != 0,
        allow_binary: flags & 4 != 0,
        allow_utf8_headers: flags & 8 != 0,
        flowed: flags & 16 != 0,
        legacy_name: flags & 32 != 0,
    };

    let model = MimeCst::parse(data).decode();

    let Ok(first) = model.to_bytes(&opts) else {
        return;
    };

    let once = MimeCst::parse(&first).decode();
    let second = once.to_bytes(&opts).expect("a written model writes again");
    let twice = MimeCst::parse(&second).decode();

    assert_eq!(once, twice, "not stable");
    assert_eq!(first, second, "not a fixpoint");
});
