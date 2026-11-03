//! # Field markers
//!
//! One zero-sized marker per known field, spelled as its wire name with
//! hyphens as underscores (`SUBJECT`, `MESSAGE_ID`, `CONTENT_TYPE`), keying
//! typed access: `entity.header::<SUBJECT>()` reads the field as the type
//! its form decodes to.
//!
//! Like vcard's property markers, they are type-level keys naming a wire
//! token, never constructed and never handled as values, which is why they
//! carry no domain prefix.

#![allow(non_camel_case_types)]

use alloc::{string::String, vec::Vec};

use crate::{
    header::MimeHeaderKind,
    value::{
        MimeHeaderValue,
        address::MimeAddressList,
        content_type::{MimeContentType, MimeDisposition},
        datetime::MimeDateTime,
        transfer_encoding::MimeTransferEncoding,
    },
};

/// A known field as a type-level key.
pub trait MimeHeaderMarker {
    /// The field the marker names.
    const KIND: MimeHeaderKind;

    /// What the field's default form decodes to.
    type Value;

    /// Takes the typed value out of a decoded one, `None` if not in its form.
    fn from_value(value: MimeHeaderValue) -> Option<Self::Value>;

    /// Wraps a typed value into a decoded one of the field's form.
    fn into_value(value: Self::Value) -> MimeHeaderValue;
}

macro_rules! markers {
    ($($marker:ident => $kind:ident, $arm:ident, $value:ty;)*) => {
        $(
            #[doc = concat!("The `", stringify!($kind), "` field marker.")]
            #[derive(Clone, Copy, Debug)]
            pub struct $marker;

            impl MimeHeaderMarker for $marker {
                const KIND: MimeHeaderKind = MimeHeaderKind::$kind;
                type Value = $value;

                fn from_value(value: MimeHeaderValue) -> Option<$value> {
                    match value {
                        MimeHeaderValue::$arm(value) => Some(value),
                        _ => None,
                    }
                }

                fn into_value(value: $value) -> MimeHeaderValue {
                    MimeHeaderValue::$arm(value)
                }
            }
        )*
    };
}

markers! {
    DATE => Date, Date, MimeDateTime;
    FROM => From, Addresses, MimeAddressList;
    SENDER => Sender, Addresses, MimeAddressList;
    REPLY_TO => ReplyTo, Addresses, MimeAddressList;
    TO => To, Addresses, MimeAddressList;
    CC => Cc, Addresses, MimeAddressList;
    BCC => Bcc, Addresses, MimeAddressList;
    MESSAGE_ID => MessageId, MessageIds, Vec<String>;
    IN_REPLY_TO => InReplyTo, MessageIds, Vec<String>;
    REFERENCES => References, MessageIds, Vec<String>;
    SUBJECT => Subject, Text, String;
    COMMENTS => Comments, Text, String;
    KEYWORDS => Keywords, CommaList, Vec<String>;
    RESENT_DATE => ResentDate, Date, MimeDateTime;
    RESENT_FROM => ResentFrom, Addresses, MimeAddressList;
    RESENT_SENDER => ResentSender, Addresses, MimeAddressList;
    RESENT_TO => ResentTo, Addresses, MimeAddressList;
    RESENT_CC => ResentCc, Addresses, MimeAddressList;
    RESENT_BCC => ResentBcc, Addresses, MimeAddressList;
    RESENT_MESSAGE_ID => ResentMessageId, MessageIds, Vec<String>;
    RETURN_PATH => ReturnPath, Addresses, MimeAddressList;
    RECEIVED => Received, Raw, Vec<u8>;
    MIME_VERSION => MimeVersion, Text, String;
    CONTENT_TYPE => ContentType, ContentType, MimeContentType;
    CONTENT_TRANSFER_ENCODING => ContentTransferEncoding, TransferEncoding, MimeTransferEncoding;
    CONTENT_ID => ContentId, MessageIds, Vec<String>;
    CONTENT_DESCRIPTION => ContentDescription, Text, String;
    CONTENT_DISPOSITION => ContentDisposition, Disposition, MimeDisposition;
    CONTENT_LANGUAGE => ContentLanguage, CommaList, Vec<String>;
    CONTENT_LOCATION => ContentLocation, Text, String;
    LIST_ID => ListId, Text, String;
    LIST_HELP => ListHelp, Urls, Vec<String>;
    LIST_SUBSCRIBE => ListSubscribe, Urls, Vec<String>;
    LIST_UNSUBSCRIBE => ListUnsubscribe, Urls, Vec<String>;
    LIST_POST => ListPost, Urls, Vec<String>;
    LIST_OWNER => ListOwner, Urls, Vec<String>;
    LIST_ARCHIVE => ListArchive, Urls, Vec<String>;
    LIST_UNSUBSCRIBE_POST => ListUnsubscribePost, Text, String;
    AUTO_SUBMITTED => AutoSubmitted, Text, String;
    DISPOSITION_NOTIFICATION_TO => DispositionNotificationTo, Addresses, MimeAddressList;
}
