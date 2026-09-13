//! The object graph: typed views, edge indices, lineage, dependency
//! closures, equivalence classes, and definitional divergence.
//!
//! This crate implements Sections 5 through 9 of the specification. It
//! answers structural questions over a set of verified objects and makes no
//! judgements: whose assertions count is a viewpoint question, which belongs
//! to `publet-eval`. Where a computation needs that input -- equivalence
//! classes, divergence -- the predicate is a parameter rather than an
//! assumption.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod divergence;
mod document;
mod error;
mod graph;
mod lint;
mod view;

pub use divergence::{Divergence, TermConflict, compare};
pub use error::GraphError;
pub use graph::{Graph, Lineage, LineageView};
pub use lint::{Finding, check};
pub use view::{Annotation, Class, Key, Principal, Publet, Relation, RelationKind};

pub mod load;
