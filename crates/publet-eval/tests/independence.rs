//! Independence between reproductions (Section 11.4).
//!
//! These test `gather`, which the pinned evaluation vectors do not reach:
//! `pub-eval --vector=` builds an `Evidence` directly from the file, so
//! every change to how evidence is *gathered* from a graph is invisible to
//! that guard. This is the coverage for it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_eval::evidence_for;
use publet_graph::{Graph, load};

fn map(pairs: &[(&str, Value)]) -> Value {
    Value::Map(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn object(kind: &str, author: &str, created: &str, body: &[(&str, Value)]) -> (Cid, Vec<u8>) {
    let mut builder = Object::builder(kind, author).created(created);
    for (k, v) in body {
        builder = builder.field(k, v.clone());
    }
    let bytes = builder.build().unwrap();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

/// A key declaring a human principal. Only these count toward a floor (R11).
fn human_key(seed: &str) -> (Cid, Vec<u8>) {
    object(
        "key",
        &Cid::of(seed.as_bytes(), HashAlg::Sha2_256).to_string(),
        "2026-01-01T00:00:00Z",
        &[
            ("alg", Value::Text("ed25519".into())),
            ("pubkey", Value::Bytes(seed.as_bytes().to_vec())),
            ("principal", Value::Text("human".into())),
        ],
    )
}

fn reproduction(author: &Cid, at: &str, target: &Cid) -> (Cid, Vec<u8>) {
    object(
        "ann",
        &author.to_string(),
        at,
        &[
            ("kind", Value::Text("reproduction".into())),
            ("target", Value::Text(target.to_string())),
            (
                "value",
                map(&[("outcome", Value::Text("consistent".into()))]),
            ),
        ],
    )
}

fn affiliation(
    author: &Cid,
    at: &str,
    subject: &Cid,
    org: &str,
    period: Option<(&str, &str)>,
) -> (Cid, Vec<u8>) {
    let mut value = vec![
        ("org", Value::Text(org.to_owned())),
        ("role", Value::Text("employer".into())),
        ("disclosed_by", Value::Text("self".into())),
    ];
    if let Some((from, to)) = period {
        value.push((
            "period",
            map(&[
                ("from", Value::Text(from.to_owned())),
                ("to", Value::Text(to.to_owned())),
            ]),
        ));
    }
    object(
        "ann",
        &author.to_string(),
        at,
        &[
            ("kind", Value::Text("affiliated".into())),
            ("target", Value::Text(subject.to_string())),
            ("value", map(&value)),
        ],
    )
}

/// Build a graph: one empirical claim, two human reproducers, and whatever
/// affiliations the caller wants between them.
fn graph_with(affiliations: Vec<(Cid, Vec<u8>)>) -> (Graph, Cid) {
    let (first, first_key) = human_key("K_a");
    let (second, second_key) = human_key("K_b");

    let (claim, claim_bytes) = object(
        "publet",
        &first.to_string(),
        "2026-02-01T00:00:00Z",
        &[
            ("class", Value::Text("empirical".into())),
            ("lang", Value::Text("en".into())),
            ("content", Value::Text("the rate fell".into())),
            ("depends", Value::Array(Vec::new())),
            (
                "evidence",
                Value::Array(vec![map(&[
                    ("kind", Value::Text("publet".into())),
                    ("role", Value::Text("method".into())),
                    (
                        "ref",
                        Value::Text(Cid::of(b"a method", HashAlg::Sha2_256).to_string()),
                    ),
                ])]),
            ),
        ],
    );

    let mut objects = vec![
        (first.clone(), first_key),
        (second.clone(), second_key),
        (claim.clone(), claim_bytes),
        reproduction(&first, "2026-03-01T00:00:00Z", &claim),
        reproduction(&second, "2026-03-02T00:00:00Z", &claim),
    ];
    objects.extend(affiliations);
    (load::from_objects(objects).unwrap(), claim)
}

fn keys() -> (Cid, Cid) {
    (human_key("K_a").0, human_key("K_b").0)
}

// Bindings below are named `one`/`two` rather than by key seed: `one` and
// `two` differ by a character and the linter is right that it is a hazard.

#[test]
fn two_unaffiliated_reproducers_are_two() {
    let (graph, claim) = graph_with(Vec::new());
    let evidence = evidence_for(&graph, &claim);
    assert_eq!(evidence.reproductions.consistent, 2);
    assert_eq!(evidence.reproductions.independent_consistent, 2);
}

#[test]
fn two_reproducers_at_one_organization_are_one() {
    // The case the rule exists for. Two people sharing an institution, a
    // grant, and a freezer are not a replication, and before this the
    // implementation counted them as two.
    let (one, two) = keys();
    let org = Cid::of(b"one university", HashAlg::Sha2_256).to_string();
    let (graph, claim) = graph_with(vec![
        affiliation(&one, "2026-01-02T00:00:00Z", &one, &org, None),
        affiliation(&two, "2026-01-02T00:00:00Z", &two, &org, None),
    ]);
    let evidence = evidence_for(&graph, &claim);
    assert_eq!(evidence.reproductions.consistent, 2, "both are still filed");
    assert_eq!(
        evidence.reproductions.independent_consistent, 1,
        "one consortium is one party"
    );
}

#[test]
fn different_organizations_stay_independent() {
    let (one, two) = keys();
    let (graph, claim) = graph_with(vec![
        affiliation(
            &one,
            "2026-01-02T00:00:00Z",
            &one,
            &Cid::of(b"university one", HashAlg::Sha2_256).to_string(),
            None,
        ),
        affiliation(
            &two,
            "2026-01-02T00:00:00Z",
            &two,
            &Cid::of(b"university two", HashAlg::Sha2_256).to_string(),
            None,
        ),
    ]);
    assert_eq!(
        evidence_for(&graph, &claim)
            .reproductions
            .independent_consistent,
        2
    );
}

#[test]
fn the_same_organization_at_different_times_is_independent() {
    // Someone who left before the other arrived shares no freezer.
    let (one, two) = keys();
    let org = Cid::of(b"one university", HashAlg::Sha2_256).to_string();
    let (graph, claim) = graph_with(vec![
        affiliation(
            &one,
            "2026-01-02T00:00:00Z",
            &one,
            &org,
            Some(("2015-01-01", "2018-01-01")),
        ),
        affiliation(
            &two,
            "2026-01-02T00:00:00Z",
            &two,
            &org,
            Some(("2020-01-01", "2024-01-01")),
        ),
    ]);
    assert_eq!(
        evidence_for(&graph, &claim)
            .reproductions
            .independent_consistent,
        2
    );
}

#[test]
fn an_undisclosed_end_date_does_not_manufacture_independence() {
    // An open period is still there. Reading a missing date as "no overlap"
    // would let anyone gain independence by declining to say when they left.
    let (one, two) = keys();
    let org = Cid::of(b"one university", HashAlg::Sha2_256).to_string();
    let (graph, claim) = graph_with(vec![
        affiliation(
            &one,
            "2026-01-02T00:00:00Z",
            &one,
            &org,
            Some(("2015-01-01", "2018-01-01")),
        ),
        affiliation(&two, "2026-01-02T00:00:00Z", &two, &org, None),
    ]);
    assert_eq!(
        evidence_for(&graph, &claim)
            .reproductions
            .independent_consistent,
        1
    );
}
