//! Computes the RFC 8621 text body, HTML body and attachment lists of a
//! message, without decoding any part.

use mime::tree::cst::MimeCst;

fn main() {
    let raw = concat!(
        "Content-Type: multipart/mixed; boundary=outer\r\n",
        "\r\n",
        "--outer\r\n",
        "Content-Type: multipart/alternative; boundary=inner\r\n",
        "\r\n",
        "--inner\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "Hello\r\n",
        "--inner\r\n",
        "Content-Type: text/html\r\n",
        "\r\n",
        "<p>Hello</p>\r\n",
        "--inner--\r\n",
        "--outer\r\n",
        "Content-Type: application/pdf\r\n",
        "Content-Disposition: attachment; filename=report.pdf\r\n",
        "\r\n",
        "%PDF\r\n",
        "--outer--\r\n",
    );

    let cst = MimeCst::parse(raw.as_bytes());
    let lists = cst.root.body_lists();

    println!(
        "text: {:?}",
        lists
            .text
            .iter()
            .map(|part| part.text())
            .collect::<Vec<_>>()
    );
    println!(
        "html: {:?}",
        lists
            .html
            .iter()
            .map(|part| part.text())
            .collect::<Vec<_>>()
    );
    println!(
        "attachments: {:?}",
        lists
            .attachments
            .iter()
            .map(|part| part.attachment_name())
            .collect::<Vec<_>>()
    );
    println!("has attachment: {}", lists.has_attachment());
}
