//! Personalized `PageRank` over the trust graph (Section 11.2).
//!
//! The iteration is:
//!
//! ```text
//!   W_0     = R
//!   W_{n+1} = ((10^6 - a) * R  +  a * (W_n . T)) / 10^6
//!   W       = W_K              where K = policy.iterations
//! ```
//!
//! Two properties are load-bearing and both are enforced by construction
//! rather than by care:
//!
//! * **Exactly `K` iterations.** There is no tolerance parameter, so a
//!   convergence criterion cannot be introduced by accident. Two
//!   implementations that stop at different points produce different
//!   numbers, and settlement depends on them producing the same ones.
//! * **Ordered traversal.** Every map here is a [`BTreeMap`]. A `HashMap`
//!   would make the accumulation order depend on a hash seed, and floating
//!   point would make that visible; integers hide it, which is worse,
//!   because the divergence would surface only on a value near a threshold.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Fixed6, Policy, SCALE};

/// One declared trust edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustEdge {
    /// The key conferring trust.
    pub from: String,
    /// The key receiving it.
    pub to: String,
    /// Declared weight, 1..=1000 per Section 11.1.
    pub weight: Fixed6,
    /// Age in days, for decay. Zero when the policy sets no half-life.
    pub age_days: u64,
}

/// Weight per key under a viewpoint.
pub type Weights = BTreeMap<String, Fixed6>;

/// Run the propagation to a fixed iteration count.
///
/// `edges` need not be sorted; they are grouped deterministically here.
#[must_use]
pub fn propagate(policy: &Policy, edges: &[TrustEdge]) -> Weights {
    // Seed vector, normalized to 10^6 in total.
    let seed_total: Fixed6 = policy.roots.iter().map(|r| r.weight).sum();
    let mut seed: Weights = BTreeMap::new();
    for root in &policy.roots {
        let share = root.weight.mul_div(Fixed6::ONE, seed_total);
        *seed.entry(root.key.to_string()).or_insert(Fixed6::ZERO) = seed
            .get(&root.key.to_string())
            .copied()
            .unwrap_or(Fixed6::ZERO)
            + share;
    }

    // Outbound edges per key, with decay applied before normalization so
    // that an old edge confers less rather than merely ranking lower.
    let mut outbound: BTreeMap<String, BTreeMap<String, Fixed6>> = BTreeMap::new();
    for edge in edges {
        let weight = match policy.decay_half_life_days {
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

    // Per-key normalization: a key's total conferred weight is fixed
    // regardless of how many keys it vouches for, which is what makes a
    // disconnected adversary subgraph worth zero at every size (R7).
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

    let damping = policy.damping;
    let complement = Fixed6::from_scaled(SCALE) - damping;
    let mut weights = seed.clone();

    for _ in 0..policy.iterations {
        let mut next: Weights = BTreeMap::new();
        // Seed contribution: (10^6 - a) * R / 10^6.
        for (key, value) in &seed {
            let share = value.times(complement);
            *next.entry(key.clone()).or_insert(Fixed6::ZERO) =
                next.get(key).copied().unwrap_or(Fixed6::ZERO) + share;
        }
        // Propagated contribution: a * (W . T) / 10^6.
        for (from, share) in &weights {
            let Some(targets) = normalized.get(from) else {
                continue;
            };
            for (to, fraction) in targets {
                let contribution = share.times(*fraction).times(damping);
                *next.entry(to.clone()).or_insert(Fixed6::ZERO) =
                    next.get(to).copied().unwrap_or(Fixed6::ZERO) + contribution;
            }
        }
        weights = next;
    }

    // Keys that never received weight are absent rather than zero, so that
    // callers cannot mistake "not reached" for "reached with nothing".
    weights.retain(|_, v| !v.is_zero());
    weights
}

/// Keys reachable from the roots, for independence checks.
#[must_use]
pub fn reachable_within(
    policy: &Policy,
    edges: &[TrustEdge],
    start: &str,
    distance: u32,
) -> BTreeSet<String> {
    let _ = policy;
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
