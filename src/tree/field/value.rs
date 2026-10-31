//! # Reading a value in a form
//!
//! The dispatch from a [`MimeHeaderForm`] to its reader, for a field in a
//! tree ([`read`]) or a value standing alone ([`parse`]).

use alloc::string::String;

use crate::{
    codec::rfc2047,
    header::spec::MimeHeaderForm,
    tree::field::{address, content_type, date, lex::Lexer, list},
    value::{MimeHeaderValue, transfer_encoding::MimeTransferEncoding},
};

/// Reads a raw field value (the bytes after the colon, folds included) in
/// `form`. A value that does not fit its form reads as
/// [`MimeHeaderValue::Raw`].
pub fn read(form: MimeHeaderForm, raw: &[u8]) -> MimeHeaderValue {
    match form {
        MimeHeaderForm::Raw => MimeHeaderValue::Raw(raw.to_vec()),
        MimeHeaderForm::Text => MimeHeaderValue::Text(rfc2047::decode_text(raw).into_owned()),
        MimeHeaderForm::Addresses => MimeHeaderValue::Addresses(address::parse_mailboxes(raw)),
        MimeHeaderForm::GroupedAddresses => {
            MimeHeaderValue::Addresses(address::parse_addresses(raw))
        }
        MimeHeaderForm::MessageIds => MimeHeaderValue::MessageIds(list::parse_message_ids(raw)),
        MimeHeaderForm::Date => match date::parse_date(raw) {
            Some(date) => MimeHeaderValue::Date(date),
            None => MimeHeaderValue::Raw(raw.to_vec()),
        },
        MimeHeaderForm::Urls => MimeHeaderValue::Urls(list::parse_urls(raw)),
        MimeHeaderForm::CommaList => MimeHeaderValue::CommaList(list::parse_comma_list(raw)),
        MimeHeaderForm::ContentType => {
            MimeHeaderValue::ContentType(content_type::parse_content_type(raw))
        }
        MimeHeaderForm::Disposition => {
            MimeHeaderValue::Disposition(content_type::parse_disposition(raw))
        }
        MimeHeaderForm::TransferEncoding => {
            MimeHeaderValue::TransferEncoding(transfer_encoding(raw))
        }
    }
}

/// Reads a value standing alone, outside any message: the bytes after a
/// colon, up to the end of the field (a line break not followed by a
/// blank), anything after it ignored.
pub fn parse(form: MimeHeaderForm, bytes: &[u8]) -> MimeHeaderValue {
    read(form, &bytes[..field_end(bytes)])
}

/// The length of the field value at the start of `bytes`: up to its first
/// line break not followed by a blank, the line break excluded.
pub fn field_end(bytes: &[u8]) -> usize {
    let mut i = 0;

    while let Some(at) = bytes[i..].iter().position(|byte| *byte == b'\n') {
        let lf = i + at;

        if !matches!(bytes.get(lf + 1), Some(b' ' | b'\t')) {
            return if lf > 0 && bytes[lf - 1] == b'\r' {
                lf - 1
            } else {
                lf
            };
        }

        i = lf + 1;
    }

    bytes.len()
}

/// Reads a transfer encoding mechanism, comments around it skipped, as in
/// `8BIT (raw)`.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.1>
pub fn transfer_encoding(raw: &[u8]) -> MimeTransferEncoding {
    let mut lex = Lexer::new(raw);
    lex.skip_cfws();

    let token = lex.until(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'('));
    MimeTransferEncoding::parse(&String::from_utf8_lossy(token))
}
