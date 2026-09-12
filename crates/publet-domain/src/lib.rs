//! Domain manifests, generations, deltas, and witnesses (Section 14).
//!
//! A domain is a bounded, dependency-closed collection retrievable in full.
//! Local-first operation (R16) rests on that bound: it is what makes holding
//! a replica affordable, and holding a replica is what makes reading private
//! and reverse edges complete.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod delta;
mod generation;
mod manifest;
mod witness;

pub use delta::{DeltaError, apply, checkpoints, fetches_required, hex};
pub use generation::{Generation, GenerationError, Removal, RemovalCause};
pub use manifest::{Manifest, ManifestError, check_depends_closure};
pub use witness::{SplitView, Witness, detect_split_views};
