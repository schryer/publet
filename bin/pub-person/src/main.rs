//! Personhood's command-line surface (Section 10.3).
//!
//! Personhood is never required to publish. This binary exists to make two
//! rules observable: that an implementation carrying fewer than two schemes
//! is not conformant, and that within one scope a person holds one key.
//!
//! # The schemes here prove nothing
//!
//! This implementation ships no personhood scheme. A real one wraps a
//! proof system and a published issuer set, and choosing them is a
//! political question the protocol does not answer. What follows are
//! demonstration schemes whose `verify` performs no cryptography at all:
//! they exist so the rules above have something to run against. Their
//! identifiers begin with `demo-` and they refuse to be silent about it,
//! so that a deployment cannot reach for one by accident.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg, Object};
use publet_graph::personhood::{Anonymity, Attestation, Nullifier, Registry, Scheme, SchemeError};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

/// A scheme that performs no cryptography. See the module note.
struct Demo {
    id: &'static str,
    anonymity: Anonymity,
}

impl Scheme for Demo {
    fn id(&self) -> &str {
        self.id
    }

    fn anonymity(&self) -> Anonymity {
        self.anonymity
    }

    fn verify(
        &self,
        proof: &[u8],
        _issuer_set: &Cid,
        _scope: &str,
    ) -> Result<Nullifier, SchemeError> {
        // Accepts any non-empty proof. This is not a verification and is
        // not meant to resemble one.
        if proof.is_empty() {
            return Err(SchemeError::BadProof {
                issuer_set: "a demonstration scheme requires a non-empty proof".to_owned(),
            });
        }
        Ok(Nullifier(proof.to_vec()))
    }
}

fn demo(id: &str) -> Option<Demo> {
    match id {
        // Differing anonymity claims, because Section 10.3 requires them to
        // be displayed rather than collapsed into "verified".
        "demo-unlinkable" => Some(Demo {
            id: "demo-unlinkable",
            anonymity: Anonymity {
                unlinkable_across_scopes: true,
                issuer_can_link: false,
                revocation_leaks_linkage: false,
            },
        }),
        "demo-issuer-linkable" => Some(Demo {
            id: "demo-issuer-linkable",
            anonymity: Anonymity {
                unlinkable_across_scopes: true,
                issuer_can_link: true,
                revocation_leaks_linkage: true,
            },
        }),
        _ => None,
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: pub-person [--scheme=ID]... --schemes\n\
         \x20      pub-person [--scheme=ID]... --attest=FILE...\n\
         carried demonstration schemes: demo-unlinkable, demo-issuer-linkable"
    );
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let mut registry = Registry::new();
    let mut report = false;
    let mut attestations: Vec<PathBuf> = Vec::new();

    for arg in std::env::args().skip(1) {
        if arg == "--schemes" {
            report = true;
        } else if let Some(v) = arg.strip_prefix("--scheme=") {
            let Some(scheme) = demo(v) else {
                eprintln!("unknown personhood scheme `{v}`");
                return usage();
            };
            registry.register(Box::new(scheme));
        } else if let Some(v) = arg.strip_prefix("--attest=") {
            attestations.push(PathBuf::from(v));
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    if report {
        for id in registry.schemes() {
            let Some(scheme) = demo(id) else { continue };
            let a = scheme.anonymity();
            println!(
                r#"{{"scheme":"{id}","unlinkable_across_scopes":{},"issuer_can_link":{},"revocation_leaks_linkage":{}}}"#,
                a.unlinkable_across_scopes, a.issuer_can_link, a.revocation_leaks_linkage
            );
        }
        if registry.meets_scheme_minimum() {
            return ExitCode::SUCCESS;
        }
        eprintln!(
            "carries {} personhood scheme(s); Section 10.3 requires at least \
             two, so that no single issuer set becomes the registrar of a \
             global knowledge graph",
            registry.schemes().len()
        );
        return ExitCode::from(EXIT_VIOLATION);
    }

    if attestations.is_empty() {
        return usage();
    }

    for path in attestations {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("cannot read {}: {e}", path.display());
                return ExitCode::from(EXIT_IO);
            }
        };
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        let parsed = match Object::parse(&bytes).and_then(|p| p.verify(&cid)) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_VIOLATION);
            }
        };
        let Some(attestation) = Attestation::from_object(parsed.object()) else {
            eprintln!("{}: not a personhood annotation", path.display());
            return ExitCode::from(EXIT_VIOLATION);
        };
        match registry.verify(&attestation) {
            Ok(scheme) => println!(
                r#"{{"subject":"{}","scope":"{}","scheme":"{scheme}"}}"#,
                attestation.subject, attestation.scope
            ),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_VIOLATION);
            }
        }
    }

    ExitCode::SUCCESS
}
