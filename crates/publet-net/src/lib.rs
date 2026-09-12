//! HTTP client and server for the retrieval interface (Section 14.4).
//!
//! The interface is deliberately narrow. Every route reads content-addressed
//! data or offers a single object, and **no route accepts a list of
//! identifiers the client holds**. `have`/`want` negotiation would let a
//! peer compute a smaller transfer at the cost of learning what the reader
//! has been working with, which forfeits most of what local-first operation
//! provides (Section 14.3.2). A synchronizing client discloses one integer.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod client;
mod server;
pub mod wire;

pub use client::{Client, ClientError};
pub use server::{Node, router};
