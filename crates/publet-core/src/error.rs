//! Error types for object parsing and verification.

use thiserror::Error;

/// A way in which bytes failed the canonical serialization profile.
///
/// Section 4.1 requires an implementation to *reject* non-canonical bytes
/// rather than repair them, so every variant here is a hard failure. The
/// variants are distinct because the command-line tools name the violated
/// rule on stderr, and the conformance scenarios assert on that name.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum CanonError {
    /// Input ended in the middle of an item.
    #[error("truncated input: needed {needed} more byte(s) at offset {offset}")]
    Truncated {
        /// Byte offset where the shortfall was detected.
        offset: usize,
        /// How many further bytes the item required.
        needed: usize,
    },

    /// Bytes remained after a complete top-level item.
    #[error("trailing data: {count} byte(s) after the top-level item")]
    TrailingData {
        /// Number of unconsumed bytes.
        count: usize,
    },

    /// An integer used a longer encoding than necessary.
    #[error("shortest-form integers: value {value} encoded in {used} byte(s) at offset {offset}")]
    NonShortestInteger {
        /// Byte offset of the item's head.
        offset: usize,
        /// The decoded argument.
        value: u64,
        /// Additional bytes the encoder used for the argument.
        used: usize,
    },

    /// An indefinite-length string, array, or map was present.
    #[error("no indefinite length: indefinite-length item at offset {offset}")]
    IndefiniteLength {
        /// Byte offset of the item's head.
        offset: usize,
    },

    /// A floating point value was present.
    #[error("no floating point: float at offset {offset}")]
    Float {
        /// Byte offset of the item's head.
        offset: usize,
    },

    /// A map key was not a text string.
    #[error("text map keys: non-text key at offset {offset}")]
    NonTextKey {
        /// Byte offset of the offending key.
        offset: usize,
    },

    /// Two map keys were equal.
    #[error("unique map keys: duplicate key {key:?} at offset {offset}")]
    DuplicateKey {
        /// Byte offset of the repeated key.
        offset: usize,
        /// The repeated key.
        key: String,
    },

    /// Map keys were not in ascending UTF-8 byte order.
    #[error("sorted map keys: {previous:?} precedes {current:?} at offset {offset}")]
    UnsortedKeys {
        /// Byte offset of the out-of-order key.
        offset: usize,
        /// The preceding key.
        previous: String,
        /// The key that should have come first.
        current: String,
    },

    /// A text string was not valid UTF-8.
    #[error("valid UTF-8: invalid sequence at offset {offset}")]
    InvalidUtf8 {
        /// Byte offset of the string's contents.
        offset: usize,
    },

    /// A text string was not in Unicode Normalization Form C.
    #[error("NFC normalization: {text:?} is not in NFC at offset {offset}")]
    NotNfc {
        /// Byte offset of the string's contents.
        offset: usize,
        /// The offending text.
        text: String,
    },

    /// A major type or simple value outside the profile was present.
    #[error("unsupported item: {detail} at offset {offset}")]
    Unsupported {
        /// Byte offset of the item's head.
        offset: usize,
        /// What was encountered.
        detail: &'static str,
    },

    /// Nesting exceeded the implementation limit.
    #[error("nesting too deep: limit {limit} exceeded at offset {offset}")]
    TooDeep {
        /// Byte offset where the limit was reached.
        offset: usize,
        /// The configured limit.
        limit: usize,
    },

    /// The item was larger than Section 4.5 permits.
    #[error("object too large: {size} bytes exceeds the {limit} byte limit")]
    TooLarge {
        /// Size of the input.
        size: usize,
        /// The configured ceiling.
        limit: usize,
    },
}

impl CanonError {
    /// The short rule name this violation maps to.
    ///
    /// Stable across releases: the conformance scenarios match on it.
    #[must_use]
    pub fn rule(&self) -> &'static str {
        match self {
            Self::Truncated { .. } => "complete items",
            Self::TrailingData { .. } => "single top-level item",
            Self::NonShortestInteger { .. } => "shortest-form integers",
            Self::IndefiniteLength { .. } => "no indefinite length",
            Self::Float { .. } => "no floating point",
            Self::NonTextKey { .. } => "text map keys",
            Self::DuplicateKey { .. } => "unique map keys",
            Self::UnsortedKeys { .. } => "sorted map keys",
            Self::InvalidUtf8 { .. } => "valid UTF-8",
            Self::NotNfc { .. } => "NFC normalization",
            Self::Unsupported { .. } => "supported item types",
            Self::TooDeep { .. } => "nesting limit",
            Self::TooLarge { .. } => "object size limit",
        }
    }
}
