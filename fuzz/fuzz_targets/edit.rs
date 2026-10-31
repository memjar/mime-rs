#![no_main]

//! Coverage-guided fuzz target for the edits. The first bytes pick a few
//! edits, the rest is the message they are applied to. Two oracles: no
//! edit panics, and after each the tree reads back as itself, its bytes a
//! fixpoint.

use libfuzzer_sys::fuzz_target;
use mime::{
    build::MimeBuildOptions,
    header::{MimeHeaderName, marker::SUBJECT},
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeMailbox},
    },
};

fuzz_target!(|data: &[u8]| {
    let Some((&count, data)) = data.split_first() else {
        return;
    };

    let count = usize::from(count % 4) + 1;

    if data.len() < count * 3 {
        return;
    }

    let (ops, input) = data.split_at(count * 3);
    let mut cst = MimeCst::parse(input);
    let opts = MimeBuildOptions::default();

    for op in ops.chunks(3) {
        let entities = cst.entities().len();
        let index = usize::from(op[1]) % entities;
        let arg = usize::from(op[2]);
        let text = String::from_utf8_lossy(&op[1..]).into_owned();

        let _ = match op[0] % 9 {
            0 => cst
                .header_mut(index)
                .and_then(|header| Ok(header.set::<SUBJECT>(text, &opts)?)),
            1 => cst.header_mut(index).and_then(|header| {
                let value = MimeHeaderValue::Raw(format!(" {text}").into_bytes());
                Ok(header.insert_value(&MimeHeaderName::new("X-Fuzz"), &value, &opts)?)
            }),
            2 => cst
                .header_mut(index)
                .and_then(|header| Ok(header.remove_all("to").map(drop)?)),
            3 => cst.header_mut(index).and_then(|header| {
                let item = MimeAddress::Mailbox(MimeMailbox::address("f@x.test"));
                header.push_address("To", &item, &opts)
            }),
            4 => cst.replace_content(index, op, None, &opts),
            5 => {
                let part = Mime {
                    headers: vec![],
                    body: MimeBody::Text(text),
                };
                cst.insert_part(index, arg, &part, &opts)
            }
            6 => cst.remove_part(index, arg % 4, &opts).map(drop),
            7 => cst.move_part(index, arg % 4, arg / 64, &opts),
            _ => cst.wrap(index, "mixed", &opts),
        };

        let bytes = cst.to_bytes();
        let reread = MimeCst::parse(&bytes);

        assert_eq!(reread.to_bytes(), bytes, "not a fixpoint");
        assert_eq!(reread.decode(), cst.decode(), "does not read back as edited");
    }
});
