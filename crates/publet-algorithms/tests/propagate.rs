//! Personalized `PageRank` behaviour, adapted from `publet-eval`'s original
//! suite (adversarial sybil resistance, order- and run-invariance, decay,
//! per-node normalization) with plain string node labels in place of a
//! caller's content identifiers -- the properties this crate is
//! responsible for do not depend on what a label means.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use publet_algorithms::Fixed6;
use publet_algorithms::propagate::{
    Params, Seed, TrustEdge, propagate, propagate_within, reachable_within,
};

fn params() -> Params {
    Params {
        damping: Fixed6::from_scaled(850_000),
        iterations: 20,
        decay_half_life_days: None,
    }
}

fn roots(names: &[&str]) -> Vec<Seed> {
    names
        .iter()
        .map(|n| Seed {
            key: (*n).to_owned(),
            weight: Fixed6::from_integer(1000),
        })
        .collect()
}

fn edge(from: &str, to: &str, weight: i64) -> TrustEdge {
    TrustEdge {
        subjects: Vec::new(),
        from: from.to_owned(),
        to: to.to_owned(),
        weight: Fixed6::from_integer(weight),
        age_days: 0,
    }
}

#[test]
fn an_unvouched_subgraph_receives_no_weight_at_any_size() {
    // The structural defence: an adversary creating nodes that point to
    // each other builds a component with no inbound edge from the seed set.
    let p = params();
    let r = roots(&["root"]);
    let mut edges = vec![edge("root", "honest", 100)];
    for i in 0..500 {
        let a = format!("sybil{i}");
        let b = format!("sybil{}", (i + 1) % 500);
        edges.push(edge(&a, &b, 1000));
    }
    let weights = propagate(&p, &r, &edges);
    assert!(weights.contains_key("honest"));
    for i in 0..500 {
        assert!(
            !weights.contains_key(&format!("sybil{i}")),
            "a disconnected subgraph must receive zero at every size"
        );
    }
}

#[test]
fn vouching_for_more_nodes_does_not_confer_more_weight() {
    // Per-node normalization: a node's total conferred weight is fixed.
    let p = params();
    let r = roots(&["root"]);
    let narrow = propagate(&p, &r, &[edge("root", "a", 100)]);
    let wide = propagate(
        &p,
        &r,
        &[
            edge("root", "a", 100),
            edge("root", "b", 100),
            edge("root", "c", 100),
        ],
    );
    let a_narrow = narrow.get("a").copied().unwrap();
    let a_wide = wide.get("a").copied().unwrap();
    assert!(
        a_wide < a_narrow,
        "splitting a node's weight must dilute each share, not duplicate it"
    );
}

#[test]
fn propagation_is_invariant_under_input_order() {
    let p = params();
    let r = roots(&["root"]);
    let forward = vec![
        edge("root", "a", 100),
        edge("a", "b", 50),
        edge("b", "c", 25),
        edge("root", "c", 10),
    ];
    let mut reversed = forward.clone();
    reversed.reverse();
    assert_eq!(
        propagate(&p, &r, &forward),
        propagate(&p, &r, &reversed),
        "edge order must not change any weight"
    );
}

#[test]
fn propagation_is_reproducible_across_runs() {
    let p = params();
    let r = roots(&["root"]);
    let edges = vec![edge("root", "a", 100), edge("a", "b", 70)];
    let first = propagate(&p, &r, &edges);
    for _ in 0..16 {
        assert_eq!(propagate(&p, &r, &edges), first);
    }
}

#[test]
fn decay_reduces_an_older_edge() {
    let mut p = params();
    p.decay_half_life_days = Some(30);
    let r = roots(&["root"]);
    let fresh = propagate(&p, &r, &[edge("root", "a", 100), edge("root", "b", 100)]);
    let mut aged = vec![edge("root", "a", 100), edge("root", "b", 100)];
    aged[0].age_days = 120;
    let decayed = propagate(&p, &r, &aged);
    assert!(
        decayed.get("a") < fresh.get("a"),
        "an edge four half-lives old must confer less"
    );
}

#[test]
fn a_subject_confined_edge_applies_only_within_that_subject() {
    let p = params();
    let r = roots(&["root"]);
    let mut confined = edge("root", "a", 100);
    confined.subjects = vec!["rust".to_owned()];
    let unconfined = propagate(&p, &r, &[confined.clone()]);
    let within = propagate_within(&p, &r, &[confined], &["rust".to_owned()]);
    assert!(
        !unconfined.contains_key("a"),
        "a subject-confined edge must not apply with no subject context"
    );
    assert!(
        within.contains_key("a"),
        "a subject-confined edge must apply when the query names that subject"
    );
}

#[test]
fn reachability_is_undirected_and_bounded_by_distance() {
    let edges = vec![edge("a", "b", 1), edge("b", "c", 1), edge("c", "d", 1)];
    assert_eq!(
        reachable_within(&edges, "a", 0),
        BTreeSet::from(["a".to_owned()]),
    );
    let two_hops = reachable_within(&edges, "a", 2);
    assert!(two_hops.contains("c"));
    assert!(!two_hops.contains("d"));
    // Undirected: reachable from either end.
    assert!(reachable_within(&edges, "d", 3).contains("a"));
}
