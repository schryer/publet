#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![doc = include_str!("../README.md")]

pub mod cbor;
mod cid;
mod error;
mod object;
mod sig;

pub use cid::{Cid, CidError, HashAlg};
pub use error::CanonError;
pub use object::{Builder, Object, ObjectError, PROTOCOL_VERSION, Unverified, Verified};
pub use sig::{SigAlg, SigError, sign, signing_message, verify, verifying_key};
