//! Gets a draft ready for submission: Bcc removed, Date and Message-ID
//! filled in, and the message downgraded to 7bit for a transport without
//! 8BITMIME, the fields no downgrade can fix reported.

use mime::{
    build::MimeBuildOptions,
    tree::cst::MimeCst,
    value::{datetime::MimeDateTime, message_id::MimeMessageId},
};

fn main() {
    let draft = concat!(
        "From: Alice <alice@example.com>\r\n",
        "To: bob@example.com\r\n",
        "Bcc: secret@example.com\r\n",
        "Subject: Grüße\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "Content-Transfer-Encoding: 8bit\r\n",
        "\r\n",
        "Schöne Grüße\r\n",
    );

    let opts = MimeBuildOptions::default();
    let mut cst = MimeCst::parse(draft.as_bytes());

    cst.header_mut(0).unwrap().remove_all("bcc").unwrap();

    let date = MimeDateTime::from_unix_seconds(1_793_448_000, 60);
    let id = MimeMessageId::new("1793448000.1", "example.com").unwrap();
    cst.fill_required(&date, &id, &opts).unwrap();

    let report = cst.downgrade_7bit(&opts).unwrap();
    println!("UTF-8 fields left: {:?}", report.utf8_fields);
    println!("{}", String::from_utf8_lossy(&cst.to_bytes()));
}
