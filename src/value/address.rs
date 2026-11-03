//! # Addresses
//!
//! The decoded value of an address field: mailboxes and groups in document
//! order (RFC 5322 3.4, groups anywhere as RFC 6854 allows).

use alloc::{string::String, vec::Vec};

/// One mailbox: a display name, an address, or both.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.4>
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeMailbox {
    /// The decoded display name, comments included in parentheses after it.
    pub name: Option<String>,
    /// The address (`local@domain`), without angle brackets or comments.
    pub address: Option<String>,
}

/// A named group of mailboxes.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.4>
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeGroup {
    /// The decoded group name.
    pub name: Option<String>,
    /// The members, in order.
    pub mailboxes: Vec<MimeMailbox>,
}

/// One item of an address list.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MimeAddress {
    /// A mailbox standing alone.
    Mailbox(MimeMailbox),
    /// A group of mailboxes.
    Group(MimeGroup),
}

/// An address list: mailboxes and groups in document order.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeAddressList {
    /// The items, in document order.
    pub items: Vec<MimeAddress>,
}

impl MimeMailbox {
    /// A mailbox with an address and no name.
    pub fn address(address: impl Into<String>) -> Self {
        Self {
            name: None,
            address: Some(address.into()),
        }
    }

    /// A mailbox with a name and an address.
    pub fn named(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            address: Some(address.into()),
        }
    }
}

impl MimeAddressList {
    /// Every mailbox, group members included, in document order.
    ///
    /// The RFC 8621 4.1.2.3 flat form.
    pub fn mailboxes(&self) -> impl Iterator<Item = &MimeMailbox> {
        self.items.iter().flat_map(|item| match item {
            MimeAddress::Mailbox(mailbox) => core::slice::from_ref(mailbox).iter(),
            MimeAddress::Group(group) => group.mailboxes.iter(),
        })
    }

    /// The first mailbox, group members included.
    pub fn first(&self) -> Option<&MimeMailbox> {
        self.mailboxes().next()
    }

    /// Whether the list holds no mailbox and no group.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl From<MimeMailbox> for MimeAddressList {
    fn from(mailbox: MimeMailbox) -> Self {
        Self {
            items: Vec::from([MimeAddress::Mailbox(mailbox)]),
        }
    }
}

impl FromIterator<MimeMailbox> for MimeAddressList {
    fn from_iter<I: IntoIterator<Item = MimeMailbox>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().map(MimeAddress::Mailbox).collect(),
        }
    }
}
