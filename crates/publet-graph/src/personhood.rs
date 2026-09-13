//! Personhood attestation (Section 10.3).
//!
//! An anonymous credential proves that a human-issued credential underlies a
//! key **without revealing which human**, and bounds keys per person within
//! a scope by a nullifier. This module is the shape a real scheme drops
//! into; it implements none.
//!
//! # Verification is offline, and must stay so
//!
//! [`Scheme::verify`] takes bytes and returns a nullifier. It makes no
//! network call, and the trait gives it no way to. That is deliberate: a
//! personhood check that contacts a service would tell that service who is
//! verifying what and when -- rebuilding, inside the anonymity mechanism,
//! exactly the correlatable record the mechanism exists to prevent. It
//! would also make every check a liveness dependency on a party who could
//! silence a key by declining to answer, invisibly.
//!
//! What a third party supplies is therefore static, publishable data: an
//! issuer set and a verification key. Not an endpoint.
//!
//! # What this module cannot establish
//!
//! [`Anonymity`] records what a scheme *claims*. Those are properties of
//! the scheme, checked by reading its specification -- running a proof
//! verifies the proof and says nothing about whether the issuer can
//! correlate presentations. A test asserting that anonymity holds would be
//! theatre, so none is written.

use std::collections::BTreeMap;

use publet_core::{Cid, Object, cbor::Value};
use thiserror::Error;

/// A per-person, per-scope pseudonym.
///
/// Deterministic for a holder and scope, revealing nothing about either.
/// Two keys presenting the same nullifier in one scope are held by the same
/// person; keys in different scopes are unlinkable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nullifier(pub Vec<u8>);

/// What a scheme claims about linkability.
///
/// Every field is a claim about the scheme, not a measurement of it.
/// Implementations display these alongside any personhood claim, because a
/// scheme whose issuer can link presentations offers a very different
/// guarantee from one whose issuer cannot, and the difference is invisible
/// in the proof itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anonymity {
    /// Whether presentations in different scopes are unlinkable.
    pub unlinkable_across_scopes: bool,
    /// Whether the issuer, colluding with a verifier, can link presentations.
    pub issuer_can_link: bool,
    /// Whether revocation traffic leaks linkage.
    pub revocation_leaks_linkage: bool,
}

/// Why a presentation did not verify.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum SchemeError {
    /// The proof did not verify against the issuer set.
    #[error("the presentation does not verify against issuer set {issuer_set}")]
    BadProof {
        /// The issuer set it was checked against.
        issuer_set: String,
    },
    /// The scheme is not one this implementation carries.
    #[error("unknown personhood scheme `{found}`")]
    UnknownScheme {
        /// The unrecognized identifier.
        found: String,
    },
    /// This nullifier has already been seen in this scope.
    ///
    /// The uniqueness bound: one person, one key, per scope. It is the only
    /// thing personhood buys that nothing else in the protocol can.
    #[error("this person already holds a key in scope `{scope}`")]
    NullifierReused {
        /// The scope in which uniqueness is claimed.
        scope: String,
    },
    /// The issuer set was drawn from a single jurisdiction.
    #[error(
        "an issuer set must draw on multiple independent jurisdictions; \
         a policy accepting one state's credentials has made that state the \
         registrar of a global knowledge graph"
    )]
    SingleJurisdiction,
}

/// A personhood scheme.
///
/// A real implementation wraps a proof-system verifier and a published
/// issuer set. Nothing about this trait permits a network call, and nothing
/// downstream needs one.
pub trait Scheme {
    /// The identifier carried in a `personhood` annotation.
    fn id(&self) -> &str;

    /// What this scheme claims about linkability.
    fn anonymity(&self) -> Anonymity;

    /// Verify a presentation, yielding the nullifier it carries.
    ///
    /// # Errors
    ///
    /// Returns [`SchemeError`] if the proof does not verify.
    fn verify(&self, proof: &[u8], issuer_set: &Cid, scope: &str)
    -> Result<Nullifier, SchemeError>;
}

/// A personhood attestation as it appears in the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// The key the attestation covers.
    pub subject: Cid,
    /// The scheme used.
    pub scheme: String,
    /// The issuer set, as a content-addressed document.
    ///
    /// A document rather than a constant, so that which issuers are
    /// accepted is visible, dated, and disputable. It is a political
    /// question with no technical answer, and keeping it in a versioned
    /// object at least makes the contest happen in the open.
    pub issuer_set: Cid,
    /// The scope uniqueness is claimed within.
    pub scope: String,
    /// The nullifier.
    pub nullifier: Nullifier,
    /// The proof bytes.
    pub proof: Vec<u8>,
    /// What the scheme claims about linkability.
    pub anonymity: Anonymity,
}

impl Attestation {
    /// Read an attestation from a verified annotation object.
    #[must_use]
    pub fn from_object(object: &Object) -> Option<Self> {
        let body = object.body();
        if body.get("kind").and_then(Value::as_text)? != "personhood" {
            return None;
        }
        let value = body.get("value")?;
        let anonymity = value.get("anonymity")?;
        let flag = |name: &str| matches!(anonymity.get(name), Some(Value::Bool(true)));
        Some(Self {
            subject: body.get("target").and_then(Value::as_text)?.parse().ok()?,
            scheme: value.get("scheme").and_then(Value::as_text)?.to_owned(),
            issuer_set: value
                .get("issuer_set")
                .and_then(Value::as_text)?
                .parse()
                .ok()?,
            scope: value.get("scope").and_then(Value::as_text)?.to_owned(),
            nullifier: Nullifier(value.get("nullifier").and_then(Value::as_bytes)?.to_vec()),
            proof: value
                .get("proof")
                .and_then(Value::as_bytes)
                .unwrap_or(&[])
                .to_vec(),
            anonymity: Anonymity {
                unlinkable_across_scopes: flag("unlinkable_across_scopes"),
                issuer_can_link: flag("issuer_can_link"),
                revocation_leaks_linkage: flag("revocation_leaks_linkage"),
            },
        })
    }
}

/// The schemes an implementation carries, and the nullifiers it has seen.
///
/// Section 10.3 requires at least two schemes and forbids presenting
/// personhood as binary. A registry holding one would satisfy neither, so
/// [`Registry::verify`] reports the scheme that verified rather than a
/// boolean.
#[derive(Default)]
pub struct Registry {
    schemes: BTreeMap<String, Box<dyn Scheme>>,
    seen: BTreeMap<(String, Vec<u8>), Cid>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("schemes", &self.schemes.keys().collect::<Vec<_>>())
            .field("nullifiers_seen", &self.seen.len())
            .finish()
    }
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a scheme.
    pub fn register(&mut self, scheme: Box<dyn Scheme>) {
        self.schemes.insert(scheme.id().to_owned(), scheme);
    }

    /// The schemes carried, in identifier order.
    #[must_use]
    pub fn schemes(&self) -> Vec<&str> {
        self.schemes.keys().map(String::as_str).collect()
    }

    /// Whether this implementation carries enough schemes to be conformant.
    #[must_use]
    pub fn meets_scheme_minimum(&self) -> bool {
        self.schemes.len() >= 2
    }

    /// Verify an attestation and record its nullifier.
    ///
    /// # Errors
    ///
    /// Returns [`SchemeError`] if the scheme is unknown, the proof does not
    /// verify, or the nullifier has already been seen in this scope by a
    /// different key.
    pub fn verify(&mut self, attestation: &Attestation) -> Result<&str, SchemeError> {
        let scheme =
            self.schemes
                .get(&attestation.scheme)
                .ok_or_else(|| SchemeError::UnknownScheme {
                    found: attestation.scheme.clone(),
                })?;
        scheme.verify(
            &attestation.proof,
            &attestation.issuer_set,
            &attestation.scope,
        )?;

        let key = (attestation.scope.clone(), attestation.nullifier.0.clone());
        match self.seen.get(&key) {
            Some(existing) if existing != &attestation.subject => {
                Err(SchemeError::NullifierReused {
                    scope: attestation.scope.clone(),
                })
            }
            _ => {
                self.seen.insert(key, attestation.subject.clone());
                Ok(scheme.id())
            }
        }
    }
}
