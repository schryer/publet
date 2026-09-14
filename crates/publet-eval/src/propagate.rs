//! Adapts this workspace's [`Policy`] (Cid-typed trust roots) to
//! `graphset`'s generic personalized-`PageRank` propagation.
//!
//! The propagation algorithm itself moved to `graphset` after an audit
//! found it never touched `Cid` or any other publet-specific type --
//! `Policy::roots[i].key` was read as a string and never used as anything
//! more. What stays here is the boundary: converting a `Policy` into the
//! plain `Params`/`Seed` pair the generic algorithm actually needs. This
//! is the smallest possible adapter, not a reimplementation, and it is
//! also the only place in this workspace where "trust policy" (a
//! publet-specific concept) touches "weighted graph propagation" (a
//! generic one) -- exactly where the mathematics and this workspace's own
//! definitions part ways.
//!
//! Per-key normalization (every node's total conferred weight is fixed
//! regardless of how many nodes it points to) is what makes a disconnected
//! adversary subgraph worth zero at every size (R7); that property lives
//! in `graphset::propagate` now, and is exercised from here.

use graphset::propagate::{Params, Seed};

pub use graphset::propagate::{TrustEdge, Weights};

use crate::Policy;

fn params(policy: &Policy) -> Params {
    Params {
        damping: policy.damping,
        iterations: policy.iterations,
        decay_half_life_days: policy.decay_half_life_days,
    }
}

fn seeds(policy: &Policy) -> Vec<Seed> {
    policy
        .roots
        .iter()
        .map(|r| Seed {
            key: r.key.to_string(),
            weight: r.weight,
        })
        .collect()
}

/// Run the propagation to a fixed iteration count.
///
/// `edges` need not be sorted; they are grouped deterministically here.
#[must_use]
pub fn propagate(policy: &Policy, edges: &[TrustEdge]) -> Weights {
    graphset::propagate::propagate(&params(policy), &seeds(policy), edges)
}

/// Propagate trust for a target belonging to the given subjects.
///
/// An edge confined to subjects applies only when the thing being evaluated
/// is in one of them. Evaluating with no subject context therefore uses the
/// unconfined edges alone: someone who said "I trust this reference on Rust
/// terminology" has not said anything about a claim that is not one.
#[must_use]
pub fn propagate_within(policy: &Policy, edges: &[TrustEdge], subjects: &[String]) -> Weights {
    graphset::propagate::propagate_within(&params(policy), &seeds(policy), edges, subjects)
}

/// Keys reachable from the roots, for independence checks.
#[must_use]
pub fn reachable_within(
    policy: &Policy,
    edges: &[TrustEdge],
    start: &str,
    distance: u32,
) -> std::collections::BTreeSet<String> {
    let _ = policy;
    graphset::propagate::reachable_within(edges, start, distance)
}
