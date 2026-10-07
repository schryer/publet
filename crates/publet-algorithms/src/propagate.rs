//! Personalized `PageRank` over a weighted, labelled graph.
//!
//! The iteration is:
//!
//! ```text
//!   W_0     = R
//!   W_{n+1} = ((10^6 - a) * R  +  a * (W_n . T)) / 10^6
//!   W       = W_K              where K = params.iterations
//! ```
//!
//! Two properties are load-bearing and both are enforced by construction
//! rather than by care:
//!
//! * **Exactly `K` iterations.** There is no tolerance parameter, so a
//!   convergence criterion cannot be introduced by accident. Two
//!   implementations that stop at different points produce different
//!   numbers, and reproducibility depends on them producing the same ones.
//! * **Ordered traversal.** Every map here is a [`BTreeMap`]. A `HashMap`
//!   would make the accumulation order depend on a hash seed, and floating
//!   point would make that visible; integers hide it, which is worse,
//!   because the divergence would surface only on a value near a threshold.
//!
//! Nodes are plain `String` labels. A caller whose graph is keyed by
//! something richer -- a content identifier, a database row, a URL --
//! converts to and from `String` at its own boundary; nothing here needs to
//! know what the label means, only that two equal labels are the same node.

use std::collections::{BTreeMap, BTreeSet};

use crate::Fixed6;

/// One declared, weighted, directed edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustEdge {
    /// The node conferring weight.
    pub from: String,
    /// The node receiving it.
    pub to: String,
    /// Declared weight, before normalization.
    pub weight: Fixed6,
    /// Age in days, for decay. Zero when the caller applies no half-life.
    pub age_days: u64,
    /// Labels this edge is confined to.
    ///
    /// Empty means unconfined: the edge applies regardless of context. A
    /// non-empty list restricts the edge to a query run with a matching
    /// label in `subjects`, which is what makes a per-context seed weight
    /// meaningful rather than one more undifferentiated scalar.
    pub subjects: Vec<String>,
}

/// One seed weight: a node the walk restarts to, and how much.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    /// The seeded node.
    pub key: String,
    /// Relative seed weight, before normalization.
    pub weight: Fixed6,
}

/// The parameters a propagation run needs, independent of any particular
/// caller's seed-selection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// Damping factor, scaled by 10^6 (see [`crate::SCALE`]).
    pub damping: Fixed6,
    /// Exactly how many iterations to run. Never a convergence criterion.
    pub iterations: u32,
    /// Half-life for edge decay, in days. `None` disables decay.
    pub decay_half_life_days: Option<u64>,
}

/// Weight per node.
pub type Weights = BTreeMap<String, Fixed6>;

/// Run the propagation to a fixed iteration count.
///
/// `edges` need not be sorted; they are grouped deterministically here.
///
/// # Example
///
/// ```
/// use publet_algorithms::Fixed6;
/// use publet_algorithms::propagate::{Params, Seed, TrustEdge, propagate};
///
/// let edge = |from: &str, to: &str| TrustEdge {
///     from: from.into(), to: to.into(), weight: Fixed6::ONE, age_days: 0, subjects: vec![],
/// };
/// let params = Params { damping: Fixed6::from_scaled(850_000), iterations: 20, decay_half_life_days: None };
/// let seeds = [Seed { key: "alice".into(), weight: Fixed6::ONE }];
/// let edges = [edge("alice", "bob"), edge("bob", "carol"), edge("dave", "erin")];
///
/// let weights = propagate(&params, &seeds, &edges);
/// // Weight flows from the seed along edges, less at each step...
/// assert!(weights["alice"] > weights["bob"] && weights["bob"] > weights["carol"]);
/// // ...and never to nodes the seed cannot reach.
/// assert!(!weights.contains_key("dave") && !weights.contains_key("erin"));
/// // The same input gives the same output, to the last digit.
/// assert_eq!(propagate(&params, &seeds, &edges), weights);
/// ```
#[must_use]
pub fn propagate(params: &Params, roots: &[Seed], edges: &[TrustEdge]) -> Weights {
    propagate_within(params, roots, edges, &[])
}

/// Propagate for a query confined to the given subjects.
///
/// An edge confined to subjects applies only when the query names one of
/// them. Propagating with no subject context therefore uses the unconfined
/// edges alone: an edge declared "on Rust terminology" has not said
/// anything about a query that is not one.
///
/// # Example
///
/// ```
/// use publet_algorithms::Fixed6;
/// use publet_algorithms::propagate::{Params, Seed, TrustEdge, propagate, propagate_within};
///
/// let edge = |from: &str, to: &str| TrustEdge {
///     from: from.into(), to: to.into(), weight: Fixed6::ONE, age_days: 0, subjects: vec![],
/// };
/// let params = Params { damping: Fixed6::from_scaled(850_000), iterations: 20, decay_half_life_days: None };
/// let seeds = [Seed { key: "alice".into(), weight: Fixed6::ONE }];
/// // alice vouches for bob only on Rust terminology.
/// let edges = [TrustEdge { subjects: vec!["rust".into()], ..edge("alice", "bob") }];
///
/// assert!(!propagate(&params, &seeds, &edges).contains_key("bob"));
/// assert!(propagate_within(&params, &seeds, &edges, &["rust".into()]).contains_key("bob"));
/// ```
#[must_use]
pub fn propagate_within(
    params: &Params,
    roots: &[Seed],
    edges: &[TrustEdge],
    subjects: &[String],
) -> Weights {
    let edges: Vec<TrustEdge> = edges
        .iter()
        .filter(|e| e.subjects.is_empty() || e.subjects.iter().any(|s| subjects.contains(s)))
        .cloned()
        .collect();
    propagate_all(params, roots, &edges)
}

fn propagate_all(params: &Params, roots: &[Seed], edges: &[TrustEdge]) -> Weights {
    // Seed vector, normalized to Fixed6::ONE in total.
    let seed_total: Fixed6 = roots.iter().map(|r| r.weight).sum();
    let mut seed: Weights = BTreeMap::new();
    for root in roots {
        let share = root.weight.mul_div(Fixed6::ONE, seed_total);
        let entry = seed.entry(root.key.clone()).or_insert(Fixed6::ZERO);
        *entry = *entry + share;
    }

    // Outbound edges per node, with decay applied before normalization so
    // that an old edge confers less rather than merely ranking lower.
    let mut outbound: BTreeMap<String, BTreeMap<String, Fixed6>> = BTreeMap::new();
    for edge in edges {
        let weight = match params.decay_half_life_days {
            Some(half_life) => edge.weight.halve_fractional(edge.age_days, half_life),
            None => edge.weight,
        };
        let slot = outbound
            .entry(edge.from.clone())
            .or_default()
            .entry(edge.to.clone())
            .or_insert(Fixed6::ZERO);
        *slot = *slot + weight;
    }

    // Per-node normalization: a node's total conferred weight is fixed
    // regardless of how many nodes it points to, which is what makes a
    // disconnected subgraph worth zero at every size.
    let normalized: BTreeMap<String, BTreeMap<String, Fixed6>> = outbound
        .into_iter()
        .map(|(from, targets)| {
            let total: Fixed6 = targets.values().copied().sum();
            let shares = targets
                .into_iter()
                .map(|(to, w)| (to, w.mul_div(Fixed6::ONE, total)))
                .collect();
            (from, shares)
        })
        .collect();

    let damping = params.damping;
    let complement = Fixed6::from_scaled(crate::SCALE) - damping;
    let mut weights = seed.clone();

    for _ in 0..params.iterations {
        let mut next: Weights = BTreeMap::new();
        // Seed contribution: (10^6 - a) * R / 10^6.
        for (key, value) in &seed {
            let share = value.times(complement);
            let entry = next.entry(key.clone()).or_insert(Fixed6::ZERO);
            *entry = *entry + share;
        }
        // Propagated contribution: a * (W . T) / 10^6.
        for (from, share) in &weights {
            let Some(targets) = normalized.get(from) else {
                continue;
            };
            for (to, fraction) in targets {
                let contribution = share.times(*fraction).times(damping);
                let entry = next.entry(to.clone()).or_insert(Fixed6::ZERO);
                *entry = *entry + contribution;
            }
        }
        weights = next;
    }

    // Nodes that never received weight are absent rather than zero, so
    // that callers cannot mistake "not reached" for "reached with
    // nothing".
    weights.retain(|_, v| !v.is_zero());
    weights
}

/// Nodes reachable from `start` within `distance` hops, over `edges`
/// treated as undirected.
///
/// # Example
///
/// ```
/// use std::collections::BTreeSet;
/// use publet_algorithms::Fixed6;
/// use publet_algorithms::propagate::{TrustEdge, reachable_within};
///
/// let edge = |from: &str, to: &str| TrustEdge {
///     from: from.into(), to: to.into(), weight: Fixed6::ONE, age_days: 0, subjects: vec![],
/// };
/// let edges = [edge("a", "b"), edge("b", "c"), edge("c", "d")];
/// // One hop from b, in either direction.
/// assert_eq!(
///     reachable_within(&edges, "b", 1),
///     BTreeSet::from(["a".to_string(), "b".to_string(), "c".to_string()]),
/// );
/// ```
#[must_use]
pub fn reachable_within(edges: &[TrustEdge], start: &str, distance: u32) -> BTreeSet<String> {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .insert(edge.to.as_str());
        adjacency
            .entry(edge.to.as_str())
            .or_default()
            .insert(edge.from.as_str());
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut frontier: BTreeSet<&str> = BTreeSet::from([start]);
    seen.insert(start.to_owned());
    for _ in 0..distance {
        let mut next: BTreeSet<&str> = BTreeSet::new();
        for node in &frontier {
            if let Some(neighbours) = adjacency.get(node) {
                for n in neighbours {
                    if seen.insert((*n).to_owned()) {
                        next.insert(n);
                    }
                }
            }
        }
        frontier = next;
    }
    seen
}
