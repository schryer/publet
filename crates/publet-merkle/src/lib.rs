//! Merkle structures for domains (Section 14.1.1).
//!
//! Two trees, because neither can do both jobs. The append-only [`log`]
//! yields consistency proofs -- that one generation is a prefix of a later
//! one -- but cannot prove absence. The sorted [`membership`] tree yields
//! inclusion and absence proofs but cannot yield efficient consistency
//! proofs, since inserting a member reorders interior nodes.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod log;
pub mod membership;
