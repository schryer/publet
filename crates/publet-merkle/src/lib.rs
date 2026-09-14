//! Merkle structures for domains (Section 14.1.1).
//!
//! The append-only log and sorted membership tree that used to live here
//! moved to the standalone `graphset` crate after an audit found neither
//! referenced anything from `publet-core` -- no `Cid`, no `Object`, nothing
//! this workspace specifically defines. Both are re-exported at their
//! original path below, so every existing `publet_merkle::log::...` and
//! `publet_merkle::membership::...` caller in this workspace keeps
//! compiling unchanged; new code should reach for `graphset` directly.
//!
//! Two trees, because neither can do both jobs. The append-only [`log`]
//! yields consistency proofs -- that one generation is a prefix of a later
//! one -- but cannot prove absence. The sorted [`membership`] tree yields
//! inclusion and absence proofs but cannot yield efficient consistency
//! proofs, since inserting a member reorders interior nodes.

pub use graphset::log;
pub use graphset::membership;
