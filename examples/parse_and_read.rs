//! Parses a message and reads its fields through the markers, and its text
//! body decoded.

use mime::{
    header::marker::{DATE, FROM, SUBJECT, TO},
    tree::cst::MimeCst,
};

fn main() {
    let raw = concat!(
        "From: Alice <alice@example.com>\r\n",
        "To: Bob <bob@example.com>, carol@example.com\r\n",
        "Subject: =?utf-8?q?Caf=C3=A9?= tomorrow\r\n",
        "Date: Sat, 31 Oct 2026 12:00:00 +0100\r\n",
        "Content-Type: text/plain; charset=iso-8859-1\r\n",
        "Content-Transfer-Encoding: quoted-printable\r\n",
        "\r\n",
        "Voil=E0 le menu.\r\n",
    );

    let cst = MimeCst::parse(raw.as_bytes());
    let root = &cst.root;

    println!("Subject: {:?}", root.header::<SUBJECT>());
    println!(
        "From: {:?}",
        root.header::<FROM>().and_then(|from| from.first().cloned())
    );
    println!(
        "Date: {:?}",
        root.header::<DATE>().map(|date| date.to_rfc3339())
    );

    for mailbox in root.header::<TO>().unwrap_or_default().mailboxes() {
        println!("To: {:?} <{:?}>", mailbox.name, mailbox.address);
    }

    println!("Text: {:?}", root.text());
}
