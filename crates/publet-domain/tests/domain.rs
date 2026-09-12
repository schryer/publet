//! Generation validity, delta self-verification, and checkpoint bounds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_domain::{
    DeltaError, Generation, GenerationError, Manifest, ManifestError, RemovalCause, apply,
    checkpoints, fetches_required,
};
use publet_merkle::membership::Membership;

fn cid(seed: &str) -> Cid {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256)
}

fn root_cid(m: &Membership) -> Cid {
    Cid::from_digest(HashAlg::Sha2_256, &m.root()).unwrap()
}

fn arr(items: Vec<Value>) -> Value {
    Value::Array(items)
}

fn removal(member: &Cid, cause: &str, reference: Option<&Cid>) -> Value {
    let mut map = std::collections::BTreeMap::new();
    map.insert("cid".to_owned(), Value::Text(member.to_string()));
    map.insert("cause".to_owned(), Value::Text(cause.to_owned()));
    if let Some(r) = reference {
        map.insert("ref".to_owned(), Value::Text(r.to_string()));
    }
    Value::Map(map)
}

fn generation_object(
    index: u64,
    snapshot: &Cid,
    added: &[&Cid],
    removed: Vec<Value>,
    parent: Option<&Cid>,
) -> Object {
    let mut builder = Object::builder("generation", &cid("author").to_string())
        .created("2026-09-12T10:00:00Z")
        .field("domain", Value::Text(cid("domain").to_string()))
        .field("index", Value::Uint(index))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field(
            "added",
            arr(added.iter().map(|c| Value::Text(c.to_string())).collect()),
        )
        .field("removed", arr(removed));
    if let Some(p) = parent {
        builder = builder.field("parent", Value::Text(p.to_string()));
    }
    let bytes = builder.build().unwrap();
    let id = Cid::of(&bytes, HashAlg::Sha2_256);
    Object::parse(&bytes)
        .unwrap()
        .verify(&id)
        .unwrap()
        .into_inner()
}

#[test]
fn a_removal_without_a_reference_is_rejected() {
    // The rule that makes removals accountable rather than merely visible.
    let member = cid("departing");
    let object = generation_object(
        1,
        &cid("snapshot"),
        &[],
        vec![removal(&member, "tombstone", None)],
        Some(&cid("parent")),
    );
    let err = Generation::from_object(&object).unwrap_err();
    assert!(
        matches!(err, GenerationError::UnaccountedRemoval { .. }),
        "got {err:?}"
    );
}

#[test]
fn a_removal_with_a_reference_is_accepted() {
    let member = cid("departing");
    let object = generation_object(
        1,
        &cid("snapshot"),
        &[],
        vec![removal(&member, "tombstone", Some(&cid("the tombstone")))],
        Some(&cid("parent")),
    );
    let generation = Generation::from_object(&object).unwrap();
    assert_eq!(generation.removed.len(), 1);
    assert_eq!(generation.removed[0].cause, RemovalCause::Tombstone);
}

#[test]
fn an_unknown_removal_cause_is_rejected() {
    let member = cid("departing");
    let object = generation_object(
        1,
        &cid("snapshot"),
        &[],
        vec![removal(&member, "because-i-felt-like-it", Some(&cid("x")))],
        Some(&cid("parent")),
    );
    assert!(matches!(
        Generation::from_object(&object).unwrap_err(),
        GenerationError::UnknownCause { .. }
    ));
}

#[test]
fn an_undeclared_removal_makes_a_generation_malformed() {
    // A publisher drops a member and says nothing. The membership root
    // implies the removal; the record does not declare it.
    let a = cid("a");
    let b = cid("b");
    let previous = Membership::new([a.to_string(), b.to_string()]);
    let next = Membership::new([a.to_string()]);

    let object = generation_object(1, &root_cid(&next), &[], vec![], Some(&cid("parent")));
    let generation = Generation::from_object(&object).unwrap();
    let err = generation.check_against(&previous, &next).unwrap_err();
    assert!(
        matches!(err, GenerationError::UndeclaredRemoval { .. }),
        "got {err:?}"
    );
}

#[test]
fn a_declared_removal_is_well_formed() {
    let a = cid("a");
    let b = cid("b");
    let previous = Membership::new([a.to_string(), b.to_string()]);
    let next = Membership::new([a.to_string()]);
    let object = generation_object(
        1,
        &root_cid(&next),
        &[],
        vec![removal(&b, "tombstone", Some(&cid("tombstone object")))],
        Some(&cid("parent")),
    );
    Generation::from_object(&object)
        .unwrap()
        .check_against(&previous, &next)
        .expect("a declared removal is well formed");
}

#[test]
fn indices_must_advance_by_exactly_one() {
    let first = Generation::from_object(&generation_object(
        1,
        &cid("s1"),
        &[],
        vec![],
        Some(&cid("p")),
    ))
    .unwrap();
    let skipped = Generation::from_object(&generation_object(
        3,
        &cid("s3"),
        &[],
        vec![],
        Some(&cid("p")),
    ))
    .unwrap();
    assert!(matches!(
        skipped.follows(&first).unwrap_err(),
        GenerationError::IndexGap { .. }
    ));
}

#[test]
fn a_delta_verifies_against_the_declared_root() {
    let a = cid("a");
    let b = cid("b");
    let start = Membership::new([a.to_string()]);
    let end = Membership::new([a.to_string(), b.to_string()]);

    let record = Generation::from_object(&generation_object(
        1,
        &root_cid(&end),
        &[&b],
        vec![],
        Some(&cid("parent")),
    ))
    .unwrap();

    let applied = apply(&start, 0, &[record]).expect("delta applies");
    assert_eq!(applied.root(), end.root());
}

#[test]
fn a_tampered_delta_fails_root_comparison() {
    // The peer adds an object the generation never declared. The client
    // recomputes the root and refuses, without needing to trust the peer.
    let a = cid("a");
    let b = cid("b");
    let smuggled = cid("smuggled");
    let start = Membership::new([a.to_string()]);
    let honest_end = Membership::new([a.to_string(), b.to_string()]);

    let record = Generation::from_object(&generation_object(
        1,
        &root_cid(&honest_end),
        &[&b, &smuggled],
        vec![],
        Some(&cid("parent")),
    ))
    .unwrap();

    let err = apply(&start, 0, &[record]).unwrap_err();
    assert!(
        matches!(err, DeltaError::RootMismatch { .. }),
        "got {err:?}"
    );
}

#[test]
fn a_non_contiguous_delta_is_refused() {
    let a = cid("a");
    let start = Membership::new([a.to_string()]);
    let record = Generation::from_object(&generation_object(
        5,
        &cid("s"),
        &[],
        vec![],
        Some(&cid("p")),
    ))
    .unwrap();
    assert!(matches!(
        apply(&start, 0, &[record]).unwrap_err(),
        DeltaError::NotContiguous { .. }
    ));
}

#[test]
fn a_client_far_behind_syncs_in_logarithmic_fetches() {
    // A client 300 generations behind must not need 300 fetches.
    let fetches = fetches_required(0, 300);
    assert!(fetches <= 9, "300 generations took {fetches} fetches");
    for distance in [1u64, 7, 64, 300, 1000, 100_000] {
        let n = fetches_required(0, distance);
        let bound = u64::BITS - distance.leading_zeros();
        assert!(
            u64::from(n) <= u64::from(bound),
            "distance {distance} took {n} fetches, above log2 bound {bound}"
        );
    }
}

#[test]
fn checkpoints_are_exponentially_spaced() {
    let points = checkpoints(100);
    assert!(points.contains(&99));
    assert!(points.contains(&98));
    assert!(points.contains(&96));
    assert!(points.contains(&92));
    assert!(points.contains(&36));
    assert!(points.len() < 10, "checkpoints must be sparse: {points:?}");
}

#[test]
fn a_domain_over_its_bound_is_rejected() {
    let bytes = Object::builder("domain", &cid("author").to_string())
        .created("2026-09-12T10:00:00Z")
        .field("label", Value::Text("physics".into()))
        .field("snapshot", Value::Text(cid("s").to_string()))
        .field("bound", Value::Uint(1000))
        .field("size", Value::Uint(2000))
        .build()
        .unwrap();
    let id = Cid::of(&bytes, HashAlg::Sha2_256);
    let object = Object::parse(&bytes).unwrap().verify(&id).unwrap();
    assert!(matches!(
        Manifest::from_object(object.object()).unwrap_err(),
        ManifestError::OverBound { .. }
    ));
}

#[test]
fn a_domain_within_its_bound_is_accepted() {
    let bytes = Object::builder("domain", &cid("author").to_string())
        .created("2026-09-12T10:00:00Z")
        .field("label", Value::Text("physics".into()))
        .field("snapshot", Value::Text(cid("s").to_string()))
        .field("bound", Value::Uint(10_000))
        .field("size", Value::Uint(2000))
        .field("closed_under", arr(vec![Value::Text("depends".into())]))
        .build()
        .unwrap();
    let id = Cid::of(&bytes, HashAlg::Sha2_256);
    let object = Object::parse(&bytes).unwrap().verify(&id).unwrap();
    let manifest = Manifest::from_object(object.object()).unwrap();
    assert!(manifest.claims_depends_closure());
}
