//! Deterministic trust propagation and standing computation.
//!
//! This crate implements Section 11. It is a pure function of its inputs:
//! no clock, no randomness, no I/O, no network. The dependency guard
//! enforces that mechanically, because a single edge to any of those would
//! let two conformant implementations disagree with nothing to catch it.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod gather;

/// Exact base-10^6 fixed-point arithmetic (R8: bit-identical evaluation
/// across implementations, which floating point cannot give). Moved to
/// `graphset` after an audit found it referenced nothing publet-specific;
/// re-exported here unchanged so every existing `publet_eval::Fixed6`
/// caller keeps compiling, and so R8 stays traceable to the code that
/// keeps it from the crate that owes it, not only from the generic crate
/// that happens to implement it.
pub use graphset::{Fixed6, SCALE};

pub use gather::{class_of, evidence_for, trust_edges};
mod policy;
mod propagate;

pub use policy::{MAX_ITERATIONS, Policy, PolicyError, Root};
pub use propagate::{TrustEdge, Weights, propagate, propagate_within, reachable_within};
mod standing;

pub use standing::{
    Assessment, Evidence, Outcome, Reproducibility, Reproductions, Standing, evaluate,
};
