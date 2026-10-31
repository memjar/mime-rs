//! # Header fields
//!
//! The decoded header field ([`MimeHeader`]), its name ([`MimeHeaderName`],
//! a closed [`MimeHeaderKind`] or a verbatim unknown name), and the spec
//! layer every reader of a field consults: [`spec`] states what the RFCs
//! allow of each known field, [`marker`] keys typed access by field.
//!
//! The known fields are those of the IANA permanent message header field
//! registry this crate covers: RFC 5322, the MIME RFCs (2045, 2183, 2557,
//! 3282), the list fields (2369, 2919, 8058), RFC 3834 and RFC 8098.

pub mod marker;
pub mod spec;

use alloc::string::String;
use core::{fmt, ops::Deref, str::FromStr};

use crate::{
    header::spec::{MimeHeaderForm, MimeHeaderSpec},
    value::MimeHeaderValue,
};

/// A decoded header field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeHeader {
    /// The field name.
    pub name: MimeHeaderName,
    /// The value, read in the field's default form.
    pub value: MimeHeaderValue,
}

/// A field name: a known kind, or a verbatim unknown name.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MimeHeaderName {
    /// A field this crate knows.
    Known(MimeHeaderKind),
    /// Any other field, spelled as written.
    Unknown(String),
}

impl MimeHeaderName {
    /// Reads a name: a known kind when it spells one, ASCII
    /// case-insensitively, an unknown name otherwise.
    pub fn new(name: &str) -> Self {
        match MimeHeaderKind::from_str(name) {
            Ok(kind) => Self::Known(kind),
            Err(()) => Self::Unknown(String::from(name)),
        }
    }

    /// The kind, for a known name.
    pub fn kind(&self) -> Option<MimeHeaderKind> {
        match self {
            Self::Known(kind) => Some(*kind),
            Self::Unknown(_) => None,
        }
    }

    /// The name's spelling: canonical for a known one, as written for an
    /// unknown one.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(kind) => kind,
            Self::Unknown(name) => name,
        }
    }

    /// Whether the name is `name`, compared ASCII case-insensitively.
    pub fn is(&self, name: &str) -> bool {
        self.as_str().eq_ignore_ascii_case(name)
    }

    /// The form the field is read in by default: its spec's for a known
    /// name, raw for an unknown one.
    pub fn default_form(&self) -> MimeHeaderForm {
        self.kind()
            .map_or(MimeHeaderForm::Raw, |kind| kind.spec().form)
    }
}

impl fmt::Display for MimeHeaderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

macro_rules! kinds {
    ($($variant:ident => $spelling:literal, $form:ident, $min:literal, $max:expr, $part:literal, $rfc:literal;)*) => {
        /// A header field this crate knows.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum MimeHeaderKind {
            $(
                #[doc = concat!("`", $spelling, "`, ", $rfc, ".")]
                $variant,
            )*
        }

        impl MimeHeaderKind {
            /// Every known field.
            pub const ALL: &[Self] = &[$(Self::$variant),*];

            /// What the RFCs allow of the field.
            pub const fn spec(&self) -> MimeHeaderSpec {
                match self {
                    $(Self::$variant => MimeHeaderSpec {
                        kind: Self::$variant,
                        form: MimeHeaderForm::$form,
                        min: $min,
                        max: $max,
                        part: $part,
                        rfc: $rfc,
                    },)*
                }
            }

            fn spelling(&self) -> &'static str {
                match self {
                    $(Self::$variant => $spelling,)*
                }
            }
        }
    };
}

kinds! {
    Date => "Date", Date, 1, Some(1), false, "RFC 5322 3.6.1";
    From => "From", GroupedAddresses, 1, Some(1), false, "RFC 5322 3.6.2";
    Sender => "Sender", GroupedAddresses, 0, Some(1), false, "RFC 5322 3.6.2";
    ReplyTo => "Reply-To", GroupedAddresses, 0, Some(1), false, "RFC 5322 3.6.2";
    To => "To", GroupedAddresses, 0, Some(1), false, "RFC 5322 3.6.3";
    Cc => "Cc", GroupedAddresses, 0, Some(1), false, "RFC 5322 3.6.3";
    Bcc => "Bcc", GroupedAddresses, 0, Some(1), false, "RFC 5322 3.6.3";
    MessageId => "Message-ID", MessageIds, 0, Some(1), false, "RFC 5322 3.6.4";
    InReplyTo => "In-Reply-To", MessageIds, 0, Some(1), false, "RFC 5322 3.6.4";
    References => "References", MessageIds, 0, Some(1), false, "RFC 5322 3.6.4";
    Subject => "Subject", Text, 0, Some(1), false, "RFC 5322 3.6.5";
    Comments => "Comments", Text, 0, None, false, "RFC 5322 3.6.5";
    Keywords => "Keywords", CommaList, 0, None, false, "RFC 5322 3.6.5";
    ResentDate => "Resent-Date", Date, 0, None, false, "RFC 5322 3.6.6";
    ResentFrom => "Resent-From", GroupedAddresses, 0, None, false, "RFC 5322 3.6.6";
    ResentSender => "Resent-Sender", GroupedAddresses, 0, None, false, "RFC 5322 3.6.6";
    ResentTo => "Resent-To", GroupedAddresses, 0, None, false, "RFC 5322 3.6.6";
    ResentCc => "Resent-Cc", GroupedAddresses, 0, None, false, "RFC 5322 3.6.6";
    ResentBcc => "Resent-Bcc", GroupedAddresses, 0, None, false, "RFC 5322 3.6.6";
    ResentMessageId => "Resent-Message-ID", MessageIds, 0, None, false, "RFC 5322 3.6.6";
    ReturnPath => "Return-Path", GroupedAddresses, 0, None, false, "RFC 5322 3.6.7";
    Received => "Received", Raw, 0, None, false, "RFC 5322 3.6.7";
    MimeVersion => "MIME-Version", Text, 0, Some(1), true, "RFC 2045 4";
    ContentType => "Content-Type", ContentType, 0, Some(1), true, "RFC 2045 5";
    ContentTransferEncoding => "Content-Transfer-Encoding", TransferEncoding, 0, Some(1), true, "RFC 2045 6";
    ContentId => "Content-ID", MessageIds, 0, Some(1), true, "RFC 2045 7";
    ContentDescription => "Content-Description", Text, 0, Some(1), true, "RFC 2045 8";
    ContentDisposition => "Content-Disposition", Disposition, 0, Some(1), true, "RFC 2183 2";
    ContentLanguage => "Content-Language", CommaList, 0, Some(1), true, "RFC 3282 2";
    ContentLocation => "Content-Location", Text, 0, Some(1), true, "RFC 2557 4";
    ListId => "List-Id", Text, 0, Some(1), false, "RFC 2919 3";
    ListHelp => "List-Help", Urls, 0, Some(1), false, "RFC 2369 3.1";
    ListSubscribe => "List-Subscribe", Urls, 0, Some(1), false, "RFC 2369 3.2";
    ListUnsubscribe => "List-Unsubscribe", Urls, 0, Some(1), false, "RFC 2369 3.3";
    ListPost => "List-Post", Urls, 0, Some(1), false, "RFC 2369 3.4";
    ListOwner => "List-Owner", Urls, 0, Some(1), false, "RFC 2369 3.5";
    ListArchive => "List-Archive", Urls, 0, Some(1), false, "RFC 2369 3.6";
    ListUnsubscribePost => "List-Unsubscribe-Post", Text, 0, Some(1), false, "RFC 8058 3.1";
    AutoSubmitted => "Auto-Submitted", Text, 0, Some(1), false, "RFC 3834 5";
    DispositionNotificationTo => "Disposition-Notification-To", GroupedAddresses, 0, Some(1), false, "RFC 8098 2.1";
}

impl Deref for MimeHeaderKind {
    type Target = str;

    fn deref(&self) -> &str {
        self.spelling()
    }
}

impl FromStr for MimeHeaderKind {
    type Err = ();

    /// Reads a field name ASCII case-insensitively; a name holding a blank
    /// or a control byte matches nothing.
    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL
            .iter()
            .find(|kind| kind.spelling().eq_ignore_ascii_case(name))
            .copied()
            .ok_or(())
    }
}

impl fmt::Display for MimeHeaderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}
