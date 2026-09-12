//! Detached signatures (Section 4.4).
//!
//! A signature is its own object referencing a target by identifier, so
//! signatures accrue to an object without changing its identifier (R3).
//! The signed message is domain-separated:
//!
//! ```text
//! "pub/v1/sig" || 0x00 || purpose || 0x00 || canonical-bytes-of-target
//! ```
//!
//! The separator matters more than it looks. Without it, a signature made
//! for one purpose could be presented as a signature for another, and an
//! endorsement could be replayed as a retraction. Section 4.4 therefore
//! requires rejecting a signature whose purpose does not match the context
//! it is being evaluated in, and [`verify`] takes the expected purpose as
//! an argument so that omitting the check is not possible.

use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use thiserror::Error;

/// The domain separation prefix.
const DOMAIN: &[u8] = b"pub/v1/sig";

/// A signature algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SigAlg {
    /// Ed25519, required by Section 4.4.
    Ed25519,
}

impl SigAlg {
    /// The in-band identifier.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }

    /// Resolve an in-band identifier.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "ed25519" => Some(Self::Ed25519),
            _ => None,
        }
    }
}

/// Reasons a signature does not verify.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum SigError {
    /// The algorithm identifier is not implemented.
    #[error("unknown signature algorithm: {0}")]
    UnknownAlgorithm(String),

    /// The public key was not the right length or shape.
    #[error("malformed public key")]
    MalformedKey,

    /// The signature was not the right length or shape.
    #[error("malformed signature")]
    MalformedSignature,

    /// The signature did not verify over the constructed message.
    #[error("signature does not verify")]
    BadSignature,

    /// The signature was made for a different purpose.
    #[error("purpose mismatch: signature is for {found:?}, context requires {expected:?}")]
    PurposeMismatch {
        /// The purpose recorded in the signature object.
        found: String,
        /// The purpose the evaluating context requires.
        expected: String,
    },

    /// A purpose contained the separator byte.
    #[error("purpose must not contain a NUL byte")]
    PurposeContainsNul,
}

/// Construct the message a signature covers.
///
/// # Errors
///
/// Returns [`SigError::PurposeContainsNul`] if `purpose` contains `0x00`,
/// which would make the separated encoding ambiguous.
pub fn signing_message(purpose: &str, target_bytes: &[u8]) -> Result<Vec<u8>, SigError> {
    if purpose.as_bytes().contains(&0) {
        return Err(SigError::PurposeContainsNul);
    }
    let mut message = Vec::with_capacity(DOMAIN.len() + purpose.len() + target_bytes.len() + 2);
    message.extend_from_slice(DOMAIN);
    message.push(0);
    message.extend_from_slice(purpose.as_bytes());
    message.push(0);
    message.extend_from_slice(target_bytes);
    Ok(message)
}

/// Verify a detached signature over `target_bytes` for `expected_purpose`.
///
/// `found_purpose` is the purpose recorded in the signature object;
/// `expected_purpose` is what the calling context requires. They must
/// match, and the mismatch is reported before any cryptography is done.
///
/// # Errors
///
/// Returns [`SigError`] describing which check failed.
pub fn verify(
    alg: SigAlg,
    public_key: &[u8],
    signature: &[u8],
    found_purpose: &str,
    expected_purpose: &str,
    target_bytes: &[u8],
) -> Result<(), SigError> {
    if found_purpose != expected_purpose {
        return Err(SigError::PurposeMismatch {
            found: found_purpose.to_owned(),
            expected: expected_purpose.to_owned(),
        });
    }
    let message = signing_message(found_purpose, target_bytes)?;
    match alg {
        SigAlg::Ed25519 => {
            let key_bytes: [u8; 32] = public_key.try_into().map_err(|_| SigError::MalformedKey)?;
            let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| SigError::MalformedKey)?;
            let sig_bytes: [u8; 64] = signature
                .try_into()
                .map_err(|_| SigError::MalformedSignature)?;
            let sig = Signature::from_bytes(&sig_bytes);
            key.verify(&message, &sig)
                .map_err(|_| SigError::BadSignature)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    fn key() -> SigningKey {
        // A fixed seed: these tests assert on behaviour, not on secrecy.
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn verifies_a_correct_signature() {
        let sk = key();
        let target = b"canonical bytes";
        let msg = signing_message("endorse", target).unwrap();
        let sig = sk.sign(&msg);
        assert!(
            verify(
                SigAlg::Ed25519,
                sk.verifying_key().as_bytes(),
                &sig.to_bytes(),
                "endorse",
                "endorse",
                target,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_a_signature_replayed_under_another_purpose() {
        let sk = key();
        let target = b"canonical bytes";
        let msg = signing_message("endorse", target).unwrap();
        let sig = sk.sign(&msg);

        // Presented honestly but in the wrong context.
        assert!(matches!(
            verify(
                SigAlg::Ed25519,
                sk.verifying_key().as_bytes(),
                &sig.to_bytes(),
                "endorse",
                "retract",
                target,
            ),
            Err(SigError::PurposeMismatch { .. })
        ));

        // Relabelled to match the context: the domain separation defeats it.
        assert!(matches!(
            verify(
                SigAlg::Ed25519,
                sk.verifying_key().as_bytes(),
                &sig.to_bytes(),
                "retract",
                "retract",
                target,
            ),
            Err(SigError::BadSignature)
        ));
    }

    #[test]
    fn rejects_a_signature_over_different_bytes() {
        let sk = key();
        let msg = signing_message("endorse", b"original").unwrap();
        let sig = sk.sign(&msg);
        assert_eq!(
            verify(
                SigAlg::Ed25519,
                sk.verifying_key().as_bytes(),
                &sig.to_bytes(),
                "endorse",
                "endorse",
                b"tampered",
            ),
            Err(SigError::BadSignature)
        );
    }

    #[test]
    fn purposes_cannot_be_made_ambiguous() {
        assert_eq!(
            signing_message("a\0b", b""),
            Err(SigError::PurposeContainsNul)
        );
        // Without separation, ("ab", "c") and ("a", "bc") would collide.
        let x = signing_message("ab", b"c").unwrap();
        let y = signing_message("a", b"bc").unwrap();
        assert_ne!(x, y);
    }

    #[test]
    fn rejects_malformed_inputs() {
        let sk = key();
        assert_eq!(
            verify(SigAlg::Ed25519, b"short", &[0u8; 64], "p", "p", b""),
            Err(SigError::MalformedKey)
        );
        assert_eq!(
            verify(
                SigAlg::Ed25519,
                sk.verifying_key().as_bytes(),
                b"short",
                "p",
                "p",
                b""
            ),
            Err(SigError::MalformedSignature)
        );
    }
}
