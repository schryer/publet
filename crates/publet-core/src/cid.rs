//! Content identifiers (Section 4.2).
//!
//! A CID names an object by the hash of its canonical serialization:
//!
//! ```text
//! pub:<hash-algorithm-id>:<base32-lower-no-pad(digest)>
//! ```
//!
//! The algorithm identifier travels in-band so that a future migration is a
//! new identifier rather than a flag day (Section 15).
//!
//! # Examples
//!
//! ```
//! use publet_core::{cbor, Cid, HashAlg};
//!
//! let bytes = cbor::encode(&cbor::Value::Text("hello".into()));
//! let cid = Cid::of(&bytes, HashAlg::Sha2_256);
//! assert!(cid.to_string().starts_with("pub:sha2-256:"));
//! assert!(cid.verifies(&bytes));
//! ```

use std::fmt;
use std::str::FromStr;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// A hash algorithm usable for content identifiers.
///
/// `sha2-256` is required by Section 4.2. Further algorithms are added as
/// variants rather than as a trait object, because the set is small, closed
/// at any given protocol version, and exhaustively matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HashAlg {
    /// SHA-256, required by Section 4.2.
    Sha2_256,
}

impl HashAlg {
    /// The in-band identifier used in a CID.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Sha2_256 => "sha2-256",
        }
    }

    /// Resolve an in-band identifier.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "sha2-256" => Some(Self::Sha2_256),
            _ => None,
        }
    }

    /// Hash `bytes` with this algorithm.
    #[must_use]
    pub fn digest(self, bytes: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha2_256 => Sha256::digest(bytes).to_vec(),
        }
    }
}

/// A content identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cid {
    alg: HashAlg,
    digest: Vec<u8>,
}

impl Cid {
    /// Compute the identifier of `bytes` under `alg`.
    ///
    /// `bytes` must already be canonical; this does not check that, because
    /// callers holding a decoded object have checked it already and callers
    /// holding raw input should use [`crate::cbor::decode`] first.
    #[must_use]
    pub fn of(bytes: &[u8], alg: HashAlg) -> Self {
        Self {
            alg,
            digest: alg.digest(bytes),
        }
    }

    /// The algorithm this identifier uses.
    #[must_use]
    pub fn alg(&self) -> HashAlg {
        self.alg
    }

    /// The raw digest.
    #[must_use]
    pub fn digest(&self) -> &[u8] {
        &self.digest
    }

    /// Whether `bytes` hash to this identifier.
    ///
    /// Section 4.2 requires a recipient to verify retrieved bytes against
    /// the identifier they asked for before parsing them.
    #[must_use]
    pub fn verifies(&self, bytes: &[u8]) -> bool {
        self.alg.digest(bytes) == self.digest
    }
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pub:{}:{}", self.alg.id(), base32_lower(&self.digest))
    }
}

/// Reasons a string is not a well-formed CID.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum CidError {
    /// The `pub:` scheme prefix was absent.
    #[error("not a CID: expected a `pub:` prefix")]
    MissingScheme,
    /// The three-part structure was not present.
    #[error("not a CID: expected pub:<algorithm>:<digest>")]
    Malformed,
    /// The algorithm identifier is not implemented.
    #[error("unknown hash algorithm: {0}")]
    UnknownAlgorithm(String),
    /// The digest was not lower-case unpadded base32.
    #[error("invalid digest encoding: {0}")]
    InvalidDigest(&'static str),
    /// The digest length did not match the algorithm.
    #[error("digest length {got} does not match {alg} (expected {expected})")]
    WrongDigestLength {
        /// The algorithm named in the identifier.
        alg: &'static str,
        /// Expected digest length in bytes.
        expected: usize,
        /// Length actually decoded.
        got: usize,
    },
}

impl FromStr for Cid {
    type Err = CidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s.strip_prefix("pub:").ok_or(CidError::MissingScheme)?;
        let (alg_id, digest_text) = rest.split_once(':').ok_or(CidError::Malformed)?;
        if alg_id.is_empty() || digest_text.is_empty() {
            return Err(CidError::Malformed);
        }
        let alg = HashAlg::from_id(alg_id)
            .ok_or_else(|| CidError::UnknownAlgorithm(alg_id.to_owned()))?;
        let digest = base32_lower_decode(digest_text)?;
        let expected = alg.digest(&[]).len();
        if digest.len() != expected {
            return Err(CidError::WrongDigestLength {
                alg: alg.id(),
                expected,
                got: digest.len(),
            });
        }
        Ok(Self { alg, digest })
    }
}

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// RFC 4648 base32, lower case, no padding.
///
/// Every index below is masked to five bits and the alphabet has exactly 32
/// entries, so the lookups cannot be out of range. A fallback character
/// would silently corrupt a digest, which is worse than an annotated index.
#[allow(clippy::indexing_slicing)]
fn base32_lower(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &byte in data {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((acc >> bits) & 0x1f) as usize;
            out.push(char::from(ALPHABET[index]));
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(char::from(ALPHABET[index]));
    }
    out
}

fn base32_lower_decode(text: &str) -> Result<Vec<u8>, CidError> {
    let mut out = Vec::with_capacity(text.len().div_ceil(8) * 5);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for ch in text.chars() {
        let value = match ch {
            'a'..='z' => u32::from(ch as u8 - b'a'),
            '2'..='7' => u32::from(ch as u8 - b'2') + 26,
            'A'..='Z' => {
                return Err(CidError::InvalidDigest("digests are lower case"));
            }
            '=' => return Err(CidError::InvalidDigest("digests are unpadded")),
            _ => return Err(CidError::InvalidDigest("not a base32 character")),
        };
        acc = (acc << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            // Masking to 8 bits makes the narrowing exact.
            #[allow(clippy::cast_possible_truncation)]
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_text() {
        let cid = Cid::of(b"hello world", HashAlg::Sha2_256);
        let text = cid.to_string();
        assert_eq!(text.parse::<Cid>().unwrap(), cid);
    }

    #[test]
    fn base32_matches_rfc4648_vectors_lowercased_unpadded() {
        // RFC 4648 section 10, with padding removed and lower-cased.
        assert_eq!(base32_lower(b"f"), "my");
        assert_eq!(base32_lower(b"fo"), "mzxq");
        assert_eq!(base32_lower(b"foo"), "mzxw6");
        assert_eq!(base32_lower(b"foob"), "mzxw6yq");
        assert_eq!(base32_lower(b"fooba"), "mzxw6ytb");
        assert_eq!(base32_lower(b"foobar"), "mzxw6ytboi");
    }

    #[test]
    fn rejects_malformed_identifiers() {
        assert_eq!("nope".parse::<Cid>(), Err(CidError::MissingScheme));
        assert_eq!("pub:sha2-256".parse::<Cid>(), Err(CidError::Malformed));
        assert!(matches!(
            "pub:md5:aaaa".parse::<Cid>(),
            Err(CidError::UnknownAlgorithm(_))
        ));
        assert!(matches!(
            "pub:sha2-256:MZXW6".parse::<Cid>(),
            Err(CidError::InvalidDigest(_))
        ));
        assert!(matches!(
            "pub:sha2-256:mzxw6".parse::<Cid>(),
            Err(CidError::WrongDigestLength { .. })
        ));
    }

    #[test]
    fn verifies_only_the_bytes_it_names() {
        let cid = Cid::of(b"a", HashAlg::Sha2_256);
        assert!(cid.verifies(b"a"));
        assert!(!cid.verifies(b"b"));
    }
}
