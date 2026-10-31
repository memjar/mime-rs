#![no_main]

//! Coverage-guided fuzz target for the field readers and writers. Two
//! oracles: reading arbitrary bytes in any form never panics, and a value
//! read, written under a field and read again is stable from then on.

use libfuzzer_sys::fuzz_target;
use mime::{
    build::MimeBuildOptions,
    header::{MimeHeader, MimeHeaderName, spec::MimeHeaderForm},
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::MimeHeaderValue,
};

fn written(value: MimeHeaderValue) -> Option<Vec<u8>> {
    let mime = Mime {
        headers: vec![MimeHeader {
            name: MimeHeaderName::new("X-Fuzz"),
            value,
        }],
        body: MimeBody::Text(String::new()),
    };

    mime.to_bytes(&MimeBuildOptions::default()).ok()
}

fn read(bytes: &[u8], form: MimeHeaderForm) -> MimeHeaderValue {
    let cst = MimeCst::parse(bytes);
    let field = cst.root.header.get("X-Fuzz").expect("written field");
    field.read_as(form)
}

fuzz_target!(|data: &[u8]| {
    for form in MimeHeaderForm::ALL {
        let value = form.parse(data);

        let Some(first) = written(value) else {
            continue;
        };

        let once = read(&first, *form);
        let second = written(once.clone()).expect("a read value writes");
        let twice = read(&second, *form);

        assert_eq!(once, twice, "{form} not stable");
        assert_eq!(first, second, "{form} not a fixpoint");
    }
});
