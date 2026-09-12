#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Objects, canonical serialization, content identifiers, and signatures
//! for the Publet Protocol.
//!
//! This crate implements Section 4 of the specification. It is the
//! foundation every other crate builds on, and it holds the two invariants
//! that everything downstream assumes: that an object has exactly one
//! byte representation, and that an object is not usable until its
//! identifier and signatures have been checked.

pub mod cbor;
mod cid;
mod error;
mod object;
mod sig;

pub use cid::{Cid, CidError, HashAlg};
pub use error::CanonError;
pub use object::{Builder, Object, ObjectError, Unverified, Verified};
pub use sig::{SigAlg, SigError, signing_message, verify};
