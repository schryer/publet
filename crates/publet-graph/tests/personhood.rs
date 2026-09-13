//! Personhood: the rules about how it is treated, which is nearly all of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use publet_core::{Cid, HashAlg};
use publet_graph::personhood::{Anonymity, Attestation, Nullifier, Registry, Scheme, SchemeError};

fn cid(seed: &str) -> Cid {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256)
}

/// A stub standing in for a zero-knowledge credential scheme.
///
/// It accepts any non-empty proof. That is honest about what a stub can
/// establish: the cryptography is one statement of Section 10.3 and the
/// other eight are about how personhood is treated, which is what these
/// tests exercise.
struct Stub {
    id: &'static str,
    anonymity: Anonymity,
    single_jurisdiction: bool,
}

impl Scheme for Stub {
    fn id(&self) -> &str {
        self.id
    }

    fn anonymity(&self) -> Anonymity {
        self.anonymity
    }

    fn verify(
        &self,
        proof: &[u8],
        issuer_set: &Cid,
        _scope: &str,
    ) -> Result<Nullifier, SchemeError> {
        if self.single_jurisdiction {
            return Err(SchemeError::SingleJurisdiction);
        }
        if proof.is_empty() {
            return Err(SchemeError::BadProof {
                issuer_set: issuer_set.to_string(),
            });
        }
        Ok(Nullifier(proof.to_vec()))
    }
}

fn strong() -> Box<dyn Scheme> {
    Box::new(Stub {
        id: "stub-zk",
        anonymity: Anonymity {
            unlinkable_across_scopes: true,
            issuer_can_link: false,
            revocation_leaks_linkage: false,
        },
        single_jurisdiction: false,
    })
}

fn weak() -> Box<dyn Scheme> {
    Box::new(Stub {
        id: "stub-linkable",
        anonymity: Anonymity {
            unlinkable_across_scopes: false,
            issuer_can_link: true,
            revocation_leaks_linkage: true,
        },
        single_jurisdiction: false,
    })
}

fn attestation(subject: &str, scheme: &str, scope: &str, nullifier: &[u8]) -> Attestation {
    Attestation {
        subject: cid(subject),
        scheme: scheme.to_owned(),
        issuer_set: cid("an issuer set spanning several jurisdictions"),
        scope: scope.to_owned(),
        nullifier: Nullifier(nullifier.to_vec()),
        proof: nullifier.to_vec(),
        anonymity: Anonymity {
            unlinkable_across_scopes: true,
            issuer_can_link: false,
            revocation_leaks_linkage: false,
        },
    }
}

#[test]
fn one_scheme_does_not_meet_the_minimum() {
    // Section 10.3 requires at least two and forbids presenting personhood
    // as binary. An implementation carrying one satisfies neither while
    // looking finished.
    let mut registry = Registry::new();
    registry.register(strong());
    assert!(!registry.meets_scheme_minimum());
    registry.register(weak());
    assert!(registry.meets_scheme_minimum());
    assert_eq!(registry.schemes(), vec!["stub-linkable", "stub-zk"]);
}

#[test]
fn schemes_declare_different_anonymity_and_both_are_carried() {
    // The difference is invisible in the proof, which is why the claims are
    // displayed rather than assumed.
    assert!(!strong().anonymity().issuer_can_link);
    assert!(weak().anonymity().issuer_can_link);
}

#[test]
fn a_person_may_hold_one_key_per_scope() {
    let mut registry = Registry::new();
    registry.register(strong());
    registry.register(weak());

    let first = attestation("key A", "stub-zk", "physics", b"the same person");
    assert_eq!(registry.verify(&first).unwrap(), "stub-zk");

    // The same nullifier from a different key in one scope is the same
    // person twice, which is the bound personhood exists to provide.
    let second = attestation("key B", "stub-zk", "physics", b"the same person");
    assert!(matches!(
        registry.verify(&second),
        Err(SchemeError::NullifierReused { .. })
    ));
}

#[test]
fn one_person_is_unlinkable_across_scopes() {
    let mut registry = Registry::new();
    registry.register(strong());
    // A nullifier is per scope, so the same person in another subject is a
    // different pseudonym and no rule connects them.
    assert!(
        registry
            .verify(&attestation("key A", "stub-zk", "physics", b"person"))
            .is_ok()
    );
    assert!(
        registry
            .verify(&attestation("key B", "stub-zk", "history", b"person"))
            .is_ok()
    );
}

#[test]
fn an_unknown_scheme_is_refused_rather_than_assumed_valid() {
    let mut registry = Registry::new();
    registry.register(strong());
    assert!(matches!(
        registry.verify(&attestation("key A", "invented", "physics", b"p")),
        Err(SchemeError::UnknownScheme { .. })
    ));
}

#[test]
fn a_single_jurisdiction_issuer_set_is_refused() {
    // A policy accepting one state's credentials has made that state the
    // registrar of a global knowledge graph.
    let mut registry = Registry::new();
    registry.register(Box::new(Stub {
        id: "stub-one-state",
        anonymity: strong().anonymity(),
        single_jurisdiction: true,
    }));
    assert_eq!(
        registry.verify(&attestation("key A", "stub-one-state", "physics", b"p")),
        Err(SchemeError::SingleJurisdiction)
    );
}

#[test]
fn verification_takes_bytes_and_returns_a_nullifier() {
    // The trait gives verification no way to make a network call. A check
    // that contacted a service would tell it who is verifying what and
    // when, rebuilding inside the anonymity mechanism the record it exists
    // to prevent.
    let scheme = strong();
    let issuer_set = cid("an issuer set");
    assert!(scheme.verify(b"a proof", &issuer_set, "physics").is_ok());
    assert!(matches!(
        scheme.verify(b"", &issuer_set, "physics"),
        Err(SchemeError::BadProof { .. })
    ));
}
