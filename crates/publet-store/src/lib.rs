//! Object persistence, declared sets, and garbage collection
//! (Sections 13.1, 13.2, 14.6).
//!
//! A node's declared set is the commitment it has made: within it, service
//! is unconditional and content-blind, and ceasing to serve requires a
//! published tombstone. Outside it, a node owes nothing and may discard at
//! will. The store enforces that boundary rather than documenting it.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;
mod store;

pub use error::StoreError;
pub use store::{Corruption, Store};
