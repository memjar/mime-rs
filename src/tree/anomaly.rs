//! # Anomalies
//!
//! What the parser worked around. Parsing never fails: every recovery is
//! recorded on the entity it happened in, and the tree still round-trips.

use core::fmt;

/// One recovery the parser made on an entity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MimeAnomaly {
    /// The header block ran into the end of its entity with no empty line.
    NoBlankLine,
    /// A header block line was neither a field nor a continuation.
    OpaqueLine,
    /// A multipart whose boundary never occurs, kept as a leaf.
    MissingDelimiter,
    /// A multipart opened or closed by a delimiter found mid-line.
    FallbackDelimiter,
    /// A multipart with no close delimiter, its last part ending with the body.
    Unterminated,
    /// A container past the nesting bound, kept as a leaf.
    NestingLimit,
    /// A multipart past the entity bound, kept as a leaf.
    PartLimit,
    /// A search answer position that failed its check and was dropped.
    DroppedAnswer,
}

impl fmt::Display for MimeAnomaly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoBlankLine => "header block without blank line",
            Self::OpaqueLine => "opaque header line",
            Self::MissingDelimiter => "missing delimiter",
            Self::FallbackDelimiter => "fallback delimiter",
            Self::Unterminated => "unterminated multipart",
            Self::NestingLimit => "nesting limit reached",
            Self::PartLimit => "part limit reached",
            Self::DroppedAnswer => "dropped search answer",
        })
    }
}

impl MimeAnomaly {
    /// Every anomaly, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::NoBlankLine,
        Self::OpaqueLine,
        Self::MissingDelimiter,
        Self::FallbackDelimiter,
        Self::Unterminated,
        Self::NestingLimit,
        Self::PartLimit,
        Self::DroppedAnswer,
    ];

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// The set of anomalies one entity carries, one bit each.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeAnomalies(u8);

impl MimeAnomalies {
    /// Whether the set holds `anomaly`.
    pub fn contains(&self, anomaly: MimeAnomaly) -> bool {
        self.0 & anomaly.bit() != 0
    }

    /// Adds `anomaly` to the set.
    pub fn insert(&mut self, anomaly: MimeAnomaly) {
        self.0 |= anomaly.bit();
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// The anomalies of the set, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = MimeAnomaly> + '_ {
        MimeAnomaly::ALL
            .into_iter()
            .filter(|anomaly| self.contains(*anomaly))
    }
}
