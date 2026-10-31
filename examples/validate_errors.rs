//! Validates a message: every problem with its rule, RFC section and
//! severity, what the message needs of a transport, and the proof a
//! message with no violation earns.

use mime::tree::cst::MimeCst;

fn main() {
    let raw = concat!(
        "From: a@example.com, b@example.com\r\n",
        "Subject: no Date, no Sender, no Message-ID\r\n",
        "Content-Type: multipart/mixed\r\n",
        "\r\n",
        "body\r\n",
    );

    let output = MimeCst::parse(raw.as_bytes()).check();

    for problem in &output.problems {
        println!("{:?}: {problem}", problem.severity);
    }

    println!("transport: {:?}", output.transport);
    println!(
        "valid: {}",
        MimeCst::parse(raw.as_bytes()).validate().is_ok()
    );
}
