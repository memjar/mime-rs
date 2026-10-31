#![no_main]

//! Coverage-guided fuzz target for the parser. Three oracles: parsing
//! arbitrary bytes never panics, the tree serializes back to the input byte
//! for byte, and its output reparses to the same bytes.

use libfuzzer_sys::fuzz_target;
use mime::tree::cst::MimeCst;

fuzz_target!(|data: &[u8]| {
    let cst = MimeCst::parse(data);
    let bytes = cst.to_bytes();

    assert_eq!(bytes, data, "not byte-faithful");
    assert_eq!(MimeCst::parse(&bytes).to_bytes(), bytes, "not a fixpoint");

    let _ = cst.anomalies();
});
