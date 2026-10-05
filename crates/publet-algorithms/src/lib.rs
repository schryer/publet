#![doc = include_str!("../README.md")]
//!
//! ## What belongs here
//!
//! An algorithm belongs here when it needs nothing of publet's: this crate
//! depends on no other crate in the workspace (`tools/check-deps.py`
//! enforces it), so no content identifier, signed object or spec type
//! crosses into it, and `publet-core` never depends on it. Its only
//! third-party dependency is `sha2`.
//!
//! The code began inside publet (`log` and `membership` in a Merkle crate,
//! `fixed` and the core of `propagate` in `publet-eval`), was audited for
//! coupling to publet's types and extracted, and came back here as one
//! crate with that audit's rule made mechanical.

pub mod fixed;
pub mod graph;
pub mod log;
pub mod membership;
pub mod propagate;
pub mod tree;
pub mod version;

pub use fixed::{Fixed6, SCALE};
