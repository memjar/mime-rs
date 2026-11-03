//! # Transport edits
//!
//! Getting a message ready for a transport: [`MimeCst::fill_required`]
//! adds the fields a submitted message needs, and
//! [`MimeCst::downgrade_7bit`] re-encodes it for a transport without
//! 8BITMIME or BINARYMIME (RFC 6152), reporting what no re-encoding fixes.

use alloc::{string::String, vec, vec::Vec};

use crate::{
    build::{MimeBuildOptions, entity::fits},
    header::{MimeHeaderKind, MimeHeaderName},
    tree::{
        body::MimeBodyNode,
        cst::MimeCst,
        edit::{MimeEditError, Protection, options, structure::separate},
        entity::MimeEntity,
        parse::MimeParseOptions,
    },
    value::{
        MimeHeaderValue, datetime::MimeDateTime, message_id::MimeMessageId,
        transfer_encoding::MimeTransferEncoding,
    },
};

/// What a downgrade to 7bit could not do.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeDowngrade {
    /// The fields carrying UTF-8 (RFC 6532), by entity index and name.
    ///
    /// No re-encoding fixes them losslessly.
    pub utf8_fields: Vec<(usize, String)>,
    /// The protected entities that needed re-encoding, left as they are.
    pub sealed: Vec<usize>,
}

impl MimeDowngrade {
    /// Whether the message is now 7-bit throughout.
    pub fn is_complete(&self) -> bool {
        self.utf8_fields.is_empty() && self.sealed.is_empty()
    }
}

impl MimeCst<'_> {
    /// Adds the Date, Message-ID and MIME-Version fields the message lacks.
    ///
    /// MIME-Version only when it has a Content- field. Every existing byte
    /// stays in place, and a second call changes nothing.
    pub fn fill_required(
        &mut self,
        date: &MimeDateTime,
        message_id: &MimeMessageId,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let header = self.header_mut(0)?;

        if header.get("date").is_none() {
            let name = MimeHeaderName::Known(MimeHeaderKind::Date);
            header.insert_value(&name, &MimeHeaderValue::Date(*date), opts)?;
        }

        if header.get("message-id").is_none() {
            let name = MimeHeaderName::Known(MimeHeaderKind::MessageId);
            let value = MimeHeaderValue::MessageIds(vec![String::from(message_id.clone())]);
            header.insert_value(&name, &value, opts)?;
        }

        let content = header.fields().any(|field| {
            field
                .name()
                .get(..8)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"content-"))
        });

        if content && header.get("mime-version").is_none() {
            let name = MimeHeaderName::Known(MimeHeaderKind::MimeVersion);
            let value = MimeHeaderValue::Text(String::from("1.0"));
            header.insert_value(&name, &value, opts)?;
        }

        Ok(())
    }

    /// Re-encodes the leaves a 7-bit transport cannot carry, reports the rest.
    ///
    /// Leaves in `8bit`, `binary`, or unencoded with 8-bit bytes, NUL, stray CR
    /// or LF or long lines become quoted-printable or base64, their containers
    /// `7bit`; nested messages are recursed into, not encoded (RFC 2046 5.2.1).
    pub fn downgrade_7bit(
        &mut self,
        opts: &MimeBuildOptions,
    ) -> Result<MimeDowngrade, MimeEditError> {
        let opts = MimeBuildOptions {
            allow_8bit: false,
            allow_binary: false,
            ..opts.clone()
        };

        // NOTE: a body the parse left whole is split first, which changes no
        // byte.
        self.expand(MimeParseOptions::default());

        let mut report = MimeDowngrade::default();
        let mut index = 0;
        downgrade(
            &mut self.root,
            false,
            &mut Vec::new(),
            &mut index,
            &opts,
            &mut report,
        )?;

        report.sealed.sort_unstable();
        Ok(report)
    }
}

/// Downgrades one entity and what it holds, numbered as `MimeCst::entities`.
///
/// `protected` says whether a signature or an encryption covers it, `taken`
/// holds the enclosing boundaries.
fn downgrade(
    entity: &mut MimeEntity<'_>,
    protected: bool,
    taken: &mut Vec<String>,
    index: &mut usize,
    opts: &MimeBuildOptions,
    report: &mut MimeDowngrade,
) -> Result<(), MimeEditError> {
    let here = *index;
    *index += 1;

    for field in entity.header.fields() {
        if !field.line().is_ascii() {
            report
                .utf8_fields
                .push((here, String::from(field.name_str())));
        }
    }

    let eol = entity.header.line_ending();
    let encoding = entity.transfer_encoding();
    let identity = matches!(
        encoding,
        MimeTransferEncoding::SevenBit
            | MimeTransferEncoding::EightBit
            | MimeTransferEncoding::Binary
    );
    let protection = Protection::of(entity);
    let boundary = entity.content_type().param("boundary");

    if let MimeBodyNode::Leaf(bytes) = &entity.body {
        let needs =
            identity && (encoding != MimeTransferEncoding::SevenBit || !fits(bytes, false, eol));

        if needs && protected {
            report.sealed.push(here);
        } else if needs {
            let bytes = bytes.to_vec();
            entity.replace_content(&bytes, None, taken, opts)?;
        }

        return Ok(());
    }

    match &mut entity.body {
        MimeBodyNode::Multipart(multipart) => {
            taken.extend(boundary);

            for (position, part) in multipart.parts.iter_mut().enumerate() {
                let covered = match protection {
                    Some(Protection::Signed) => position == 0,
                    Some(Protection::Encrypted) => true,
                    None => false,
                };

                downgrade(
                    &mut part.entity,
                    protected || covered,
                    taken,
                    index,
                    opts,
                    report,
                )?;
            }

            taken.pop();
            separate(multipart, eol);
        }
        MimeBodyNode::Message(nested) => downgrade(nested, protected, taken, index, opts, report)?,
        MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => return Ok(()),
    }

    let relabel = identity
        && encoding != MimeTransferEncoding::SevenBit
        && fits(&body_bytes(&entity.body), false, eol);

    if relabel && (protected || protection.is_some()) {
        report.sealed.push(here);
    } else if relabel {
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentTransferEncoding);
        let value = MimeHeaderValue::TransferEncoding(MimeTransferEncoding::SevenBit);
        entity
            .header
            .set_field(&name, &value, &options(opts, eol))?;
    }

    Ok(())
}

/// The bytes of a body as written.
fn body_bytes(body: &MimeBodyNode<'_>) -> Vec<u8> {
    let mut chunks = Vec::new();
    body.push_chunks(&mut chunks);
    chunks.concat()
}
