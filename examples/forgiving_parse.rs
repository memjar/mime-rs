//! Parses a damaged message: the parse never fails, the tree still
//! round-trips byte for byte, and what the parser worked around is listed.

use mime::tree::cst::MimeCst;

fn main() {
    let raw = concat!(
        "Subject: no blank line before the body\r\n",
        "Content-Type: multipart/mixed; boundary=b\r\n",
        "garbage that is not a field\r\n",
        "\r\n",
        "--b\r\n",
        "\r\n",
        "a part whose multipart never closes\r\n",
    );

    let cst = MimeCst::parse(raw.as_bytes());

    assert_eq!(cst.to_bytes(), raw.as_bytes());

    for (entity, anomaly) in cst.anomalies() {
        println!("entity {entity}: {anomaly}");
    }
}
