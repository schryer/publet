//! Section 11 behaviour, with the dominance rules tested adversarially.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};

use publet_eval::{
    Evidence, Fixed6, Outcome, Policy, Reproducibility, Reproductions, Root, TrustEdge, evaluate,
    propagate,
};
use publet_graph::Class;

fn key(name: &str) -> String {
    use publet_core::{Cid, HashAlg};
    Cid::of(name.as_bytes(), HashAlg::Sha2_256).to_string()
}

fn policy(roots: &[&str]) -> Policy {
    Policy {
        roots: roots
            .iter()
            .map(|r| Root {
                key: key(r).parse().unwrap(),
                weight: Fixed6::from_integer(1000),
            })
            .collect(),
        damping: Fixed6::from_scaled(850_000),
        iterations: 20,
        tau: Fixed6::from_scaled(660_000),
        delta_max: Fixed6::from_scaled(290_000),
        replication_floor: 2,
        independence_distance: 2,
        decay_half_life_days: None,
        use_equivalence: true,
    }
}

fn edge(from: &str, to: &str, weight: i64) -> TrustEdge {
    TrustEdge {
        from: key(from),
        to: key(to),
        weight: Fixed6::from_integer(weight),
        age_days: 0,
    }
}

fn keys(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| key(n)).collect()
}

/// Weights in which every named key carries the same amount.
fn flat(names: &[&str]) -> BTreeMap<String, Fixed6> {
    names
        .iter()
        .map(|n| (key(n), Fixed6::from_scaled(100_000)))
        .collect()
}

// --- propagation -----------------------------------------------------------

#[test]
fn an_unvouched_subgraph_receives_no_weight_at_any_size() {
    // R7: the structural defence. An adversary creating keys that trust
    // each other builds a component with no inbound edge from the seed set.
    let p = policy(&["root"]);
    let mut edges = vec![edge("root", "honest", 100)];
    for i in 0..500 {
        let a = format!("sybil{i}");
        let b = format!("sybil{}", (i + 1) % 500);
        edges.push(edge(&a, &b, 1000));
    }
    let weights = propagate(&p, &edges);
    assert!(weights.contains_key(&key("honest")));
    for i in 0..500 {
        assert!(
            !weights.contains_key(&key(&format!("sybil{i}"))),
            "a disconnected subgraph must receive zero at every size"
        );
    }
}

#[test]
fn vouching_for_more_keys_does_not_confer_more_weight() {
    // Per-key normalization: a key's total conferred weight is fixed.
    let p = policy(&["root"]);
    let narrow = propagate(&p, &[edge("root", "a", 100)]);
    let wide = propagate(
        &p,
        &[
            edge("root", "a", 100),
            edge("root", "b", 100),
            edge("root", "c", 100),
        ],
    );
    let a_narrow = narrow.get(&key("a")).copied().unwrap();
    let a_wide = wide.get(&key("a")).copied().unwrap();
    assert!(
        a_wide < a_narrow,
        "splitting a key's trust must dilute each share, not duplicate it"
    );
}

#[test]
fn propagation_is_invariant_under_input_order() {
    let p = policy(&["root"]);
    let forward = vec![
        edge("root", "a", 100),
        edge("a", "b", 50),
        edge("b", "c", 25),
        edge("root", "c", 10),
    ];
    let mut reversed = forward.clone();
    reversed.reverse();
    assert_eq!(
        propagate(&p, &forward),
        propagate(&p, &reversed),
        "edge order must not change any weight"
    );
}

#[test]
fn propagation_is_reproducible_across_runs() {
    let p = policy(&["root"]);
    let edges = vec![edge("root", "a", 100), edge("a", "b", 70)];
    let first = propagate(&p, &edges);
    for _ in 0..16 {
        assert_eq!(propagate(&p, &edges), first);
    }
}

#[test]
fn decay_reduces_an_older_edge() {
    let mut p = policy(&["root"]);
    p.decay_half_life_days = Some(30);
    let fresh = propagate(&p, &[edge("root", "a", 100), edge("root", "b", 100)]);
    let mut aged = vec![edge("root", "a", 100), edge("root", "b", 100)];
    aged[0].age_days = 120;
    let decayed = propagate(&p, &aged);
    assert!(
        decayed.get(&key("a")) < fresh.get(&key("a")),
        "an edge four half-lives old must confer less"
    );
}

// --- evidence dominance ----------------------------------------------------

#[test]
fn overwhelming_endorsement_cannot_accept_an_unreplicated_claim() {
    // The adversarial case the plan requires: 50 trusted keys affirming,
    // zero reproductions. R9 says this is `unreplicated`, never `accepted`.
    let p = policy(&["root"]);
    let names: Vec<String> = (0..50).map(|i| format!("expert{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let evidence = Evidence {
        affirm: keys(&refs),
        reproducibility: Reproducibility::Open,
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&refs));
    assert_eq!(standing.result, Outcome::Unreplicated);
    assert_ne!(standing.result, Outcome::Accepted);
}

#[test]
fn one_independent_inconsistent_reproduction_outweighs_any_endorsement() {
    let p = policy(&["root"]);
    let names: Vec<String> = (0..50).map(|i| format!("expert{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let evidence = Evidence {
        affirm: keys(&refs),
        reproductions: Reproductions {
            independent_inconsistent: 1,
            inconsistent: 1,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&refs));
    assert_eq!(standing.result, Outcome::Rejected);
}

#[test]
fn meeting_the_replication_floor_permits_acceptance() {
    let p = policy(&["root"]);
    let evidence = Evidence {
        affirm: keys(&["a", "b", "c"]),
        reproductions: Reproductions {
            consistent: 2,
            independent_consistent: 2,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&["a", "b", "c"]));
    assert_eq!(standing.result, Outcome::Accepted);
}

#[test]
fn restricted_access_caps_standing_below_accepted() {
    // If nobody else is permitted to measure, the measurement cannot
    // establish a general claim however many reproductions its controller
    // reports.
    let p = policy(&["root"]);
    let evidence = Evidence {
        affirm: keys(&["a", "b", "c"]),
        reproducibility: Reproducibility::Restricted,
        reproductions: Reproductions {
            consistent: 99,
            independent_consistent: 99,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&["a", "b", "c"]));
    assert_eq!(standing.result, Outcome::Unreplicated);
}

#[test]
fn a_proof_is_not_a_poll() {
    let p = policy(&["root"]);
    let evidence = Evidence {
        deny: keys(&["a", "b", "c", "d", "e"]),
        proof_checked: true,
        ..Evidence::default()
    };
    let standing = evaluate(
        &p,
        Class::Formal,
        &evidence,
        &flat(&["a", "b", "c", "d", "e"]),
    );
    assert_eq!(
        standing.result,
        Outcome::Accepted,
        "no quantity of denial changes a checked proof"
    );
}

#[test]
fn replication_gates_acceptance_but_not_rejection() {
    // The asymmetry of Section 11.5: refuting is cheaper than establishing.
    let p = policy(&["root"]);
    let evidence = Evidence {
        deny: keys(&["a", "b", "c"]),
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&["a", "b", "c"]));
    assert_eq!(
        standing.result,
        Outcome::Rejected,
        "rejection needs no reproductions"
    );
}

#[test]
fn non_empirical_classes_are_not_gated_by_replication() {
    let p = policy(&["root"]);
    let evidence = Evidence {
        affirm: keys(&["a", "b", "c"]),
        ..Evidence::default()
    };
    for class in [Class::Procedural, Class::Attributive, Class::Archival] {
        let standing = evaluate(&p, class, &evidence, &flat(&["a", "b", "c"]));
        assert_eq!(standing.result, Outcome::Accepted, "{class:?}");
    }
}

// --- the predicate ---------------------------------------------------------

#[test]
fn every_outcome_is_reachable() {
    let p = policy(&["root"]);
    let w = flat(&["a", "b", "c", "d"]);

    let accepted = evaluate(
        &p,
        Class::Empirical,
        &Evidence {
            affirm: keys(&["a", "b", "c"]),
            reproductions: Reproductions {
                independent_consistent: 2,
                ..Reproductions::default()
            },
            ..Evidence::default()
        },
        &w,
    );
    assert_eq!(accepted.result, Outcome::Accepted);

    let unreplicated = evaluate(
        &p,
        Class::Empirical,
        &Evidence {
            affirm: keys(&["a", "b", "c"]),
            ..Evidence::default()
        },
        &w,
    );
    assert_eq!(unreplicated.result, Outcome::Unreplicated);

    let rejected = evaluate(
        &p,
        Class::Empirical,
        &Evidence {
            deny: keys(&["a", "b", "c"]),
            ..Evidence::default()
        },
        &w,
    );
    assert_eq!(rejected.result, Outcome::Rejected);

    let contested = evaluate(
        &p,
        Class::Empirical,
        &Evidence {
            affirm: keys(&["a", "b"]),
            deny: keys(&["c", "d"]),
            ..Evidence::default()
        },
        &w,
    );
    assert_eq!(contested.result, Outcome::Contested);

    let undetermined = evaluate(&p, Class::Empirical, &Evidence::default(), &w);
    assert_eq!(undetermined.result, Outcome::Undetermined);

    for class in [Class::Definitional, Class::Normative, Class::Expressive] {
        let standing = evaluate(
            &p,
            class,
            &Evidence {
                affirm: keys(&["a", "b", "c"]),
                ..Evidence::default()
            },
            &w,
        );
        assert_eq!(standing.result, Outcome::NotTruthApt, "{class:?}");
    }
}

#[test]
fn the_denominator_is_weight_that_evaluated_the_claim() {
    // Section 11.5: not the network's total active weight. Adding an
    // uninvolved key with weight must not change the outcome.
    let p = policy(&["root"]);
    let evidence = Evidence {
        affirm: keys(&["a", "b", "c"]),
        reproductions: Reproductions {
            independent_consistent: 2,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let narrow = evaluate(&p, Class::Empirical, &evidence, &flat(&["a", "b", "c"]));
    let wide = evaluate(
        &p,
        Class::Empirical,
        &evidence,
        &flat(&["a", "b", "c", "bystander", "onlooker"]),
    );
    assert_eq!(narrow.result, wide.result);
    assert_eq!(narrow.affirm_weight, wide.affirm_weight);
}

// --- divergence factor -----------------------------------------------------

#[test]
fn an_untrusted_dispute_contributes_nothing() {
    // R7: spamming disputes from unvouched keys accomplishes nothing at
    // any price, because they carry zero weight.
    let p = policy(&["root"]);
    let w = flat(&["a", "b", "c"]);
    let clean = Evidence {
        affirm: keys(&["a", "b", "c"]),
        reproductions: Reproductions {
            independent_consistent: 2,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let spammed = Evidence {
        argued_disputes: (0..1000).map(|i| key(&format!("spammer{i}"))).collect(),
        ..clean.clone()
    };
    assert_eq!(
        evaluate(&p, Class::Empirical, &clean, &w).delta,
        evaluate(&p, Class::Empirical, &spammed, &w).delta
    );
    assert_eq!(
        evaluate(&p, Class::Empirical, &spammed, &w).result,
        Outcome::Accepted
    );
}

#[test]
fn a_weighted_argued_dispute_raises_the_bar() {
    let p = policy(&["root"]);
    let w = flat(&["a", "b", "c", "critic"]);
    let quiet = Evidence {
        affirm: keys(&["a", "b", "c"]),
        ..Evidence::default()
    };
    let disputed = Evidence {
        argued_disputes: keys(&["critic"]),
        ..quiet.clone()
    };
    let without = evaluate(&p, Class::Empirical, &quiet, &w);
    let with = evaluate(&p, Class::Empirical, &disputed, &w);
    assert!(
        with.delta > without.delta,
        "genuine controversy raises delta"
    );
    assert!(with.delta <= p.delta_max, "delta is capped at delta_max");
}

#[test]
fn a_retracted_claim_carries_no_positive_standing() {
    let p = policy(&["root"]);
    let evidence = Evidence {
        affirm: keys(&["a", "b", "c"]),
        retracted: true,
        reproductions: Reproductions {
            independent_consistent: 9,
            ..Reproductions::default()
        },
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &flat(&["a", "b", "c"]));
    assert_eq!(standing.result, Outcome::Rejected);
}

#[test]
fn a_signer_endorsing_many_paraphrases_counts_once() {
    // Section 11.6: weight is aggregated over a set of keys, so one key
    // appearing repeatedly contributes its weight once.
    let p = policy(&["root"]);
    let w = flat(&["a"]);
    let evidence = Evidence {
        affirm: keys(&["a", "a", "a"]),
        ..Evidence::default()
    };
    let standing = evaluate(&p, Class::Empirical, &evidence, &w);
    assert_eq!(standing.affirm_weight, Fixed6::from_scaled(100_000));
}
