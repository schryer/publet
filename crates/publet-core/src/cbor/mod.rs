//! Deterministic CBOR under the profile in Section 4.1 of the specification.
//!
//! The profile is narrower than [RFC 8949] canonical encoding: floating
//! point is excluded entirely, map keys must be text, and text must be in
//! Unicode Normalization Form C. Off-the-shelf decoders are permissive by
//! design, which is the wrong disposition here — Section 4.1 requires that
//! non-canonical bytes be *rejected* rather than repaired, because an
//! object's identity is the hash of its bytes and a decoder that silently
//! normalizes would let two encodings claim one identity.
//!
//! [RFC 8949]: https://www.rfc-editor.org/rfc/rfc8949
//!
//! # Examples
//!
//! ```
//! use publet_core::cbor::{decode, encode, Value};
//!
//! let bytes = encode(&Value::Text("hello".into()));
//! assert_eq!(decode(&bytes)?, Value::Text("hello".into()));
//! # Ok::<(), publet_core::CanonError>(())
//! ```
//!
//! Non-canonical input is refused, and the error names the rule:
//!
//! ```
//! use publet_core::cbor::decode;
//!
//! // 0x18 0x05 encodes 5 in two bytes; canonical form is the single byte 0x05.
//! let err = decode(&[0x18, 0x05]).unwrap_err();
//! assert_eq!(err.rule(), "shortest-form integers");
//! ```

mod decode;
mod encode;

pub use decode::{DEFAULT_MAX_DEPTH, MAX_OBJECT_BYTES, decode, decode_with_limit};
pub use encode::encode;

use std::collections::BTreeMap;

/// A value in the deterministic CBOR profile.
///
/// There is deliberately no float variant and no tag variant: the profile
/// admits neither, so they are unrepresentable rather than rejected later.
/// Maps are [`BTreeMap`] so that encoding is sorted by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Value {
    /// A non-negative integer (CBOR major type 0).
    Uint(u64),
    /// A negative integer, stored as `-1 - n` (CBOR major type 1).
    Nint(u64),
    /// A byte string (CBOR major type 2).
    Bytes(Vec<u8>),
    /// A text string, valid UTF-8 in NFC (CBOR major type 3).
    Text(String),
    /// An array (CBOR major type 4).
    Array(Vec<Value>),
    /// A map with text keys, ordered by UTF-8 bytes (CBOR major type 5).
    Map(BTreeMap<String, Value>),
    /// `false`, `true`, or `null` (CBOR major type 7, simple values).
    Bool(bool),
    /// The null simple value.
    Null,
}

impl Value {
    /// Borrow a map entry, if this is a map containing `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Map(m) => m.get(key),
            _ => None,
        }
    }

    /// Borrow the text, if this is a text string.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Borrow the bytes, if this is a byte string.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// The value, if this is a non-negative integer.
    #[must_use]
    pub fn as_uint(&self) -> Option<u64> {
        match self {
            Self::Uint(n) => Some(*n),
            _ => None,
        }
    }
}
