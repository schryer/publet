/*!
Objects, canonical serialization, content identifiers and signatures for the
[Publet Protocol]: Section 4 of the specification, and the floor every other
publet crate and tool builds on.

Here is an object built, named by its identifier, and checked against it:

```
use publet_core::{Cid, HashAlg, Object, cbor::Value};

// An author is named by the identifier of their key object.
let author = Cid::of(b"the author's key object", HashAlg::Sha2_256).to_string();

let bytes = Object::builder("claim.prose", &author)
    .created("2026-10-05T00:00:00Z")
    .field("content", Value::Text("Water boils at 100 °C at sea level.".into()))
    .build()?;
let cid = Cid::of(&bytes, HashAlg::Sha2_256);

// Bytes fetched from anywhere are checked against the identifier asked for.
let object = Object::parse(&bytes)?.verify(&cid)?;
assert_eq!(object.object().kind(), "claim.prose");
# Ok::<(), Box<dyn std::error::Error>>(())
```

[Publet Protocol]: https://github.com/schryer/publet/blob/main/spec/index.md

# Overview

The crate holds two invariants that everything built on it assumes:

* **An object has exactly one byte representation.** Objects are CBOR under
  a strict profile (Section 4.1): text map keys sorted by their UTF-8 bytes,
  integers in shortest form, NFC text, and no floats, tags or
  indefinite lengths. [`cbor::decode`] rejects anything else, naming the rule
  broken, so equal content always has the same identifier.
* **An object is not usable until it has been checked.** [`Object::parse`]
  yields an [`Unverified`] object. Only [`Unverified::verify`], against the
  identifier you asked for, gives a [`Verified`] one, and only that is worth
  acting on.

The types:

* [`Object`] is a parsed object: its header, its body, and its bytes.
  [`Object::builder`] makes one, as canonical bytes.
* [`Cid`] is a content identifier: `pub:<algorithm>:<base32 digest>`,
  computed by [`Cid::of`], parsed from text, printed by `Display`.
* [`cbor::Value`] is a value in the profile, with [`cbor::encode`] and
  [`cbor::decode`].
* [`sign`], [`verify`] and [`verifying_key`] make and check detached Ed25519
  signatures, each bound to a *purpose*.

The rest of this page is:

* [Examples](#examples): a short cookbook.
* [Why this crate?](#why-this-crate): what a general CBOR library or IPFS
  identifiers would get wrong here.
* [Related crates](#related-crates): what was considered instead, and why not.
* [Crate features](#crate-features) and [Minimum Rust version](#minimum-rust-version).

Longer documents live in [`_documentation`]: how every item is
[tested](crate::_documentation::testing), how each dependency is
[vouched for](crate::_documentation::supply_chain), the project's
[security](crate::_documentation::security) policy, and the
[changelog](crate::_documentation::changelog).

# Examples

## Example: sign an object for a purpose

A signature covers an object's bytes *and* a purpose. One made for
`authored` says nothing about `retracted`, so an endorsement cannot be
replayed as something else.

```
use publet_core::{SigAlg, sign, verify, verifying_key};

let object = b"an object's canonical bytes";

// A real seed comes from a random source; this crate needs no RNG.
let seed = [7u8; 32];
let public = verifying_key(SigAlg::Ed25519, &seed)?;

let signature = sign(SigAlg::Ed25519, &seed, "authored", object)?;
verify(SigAlg::Ed25519, &public, &signature, "authored", "authored", object)?;
assert!(verify(SigAlg::Ed25519, &public, &signature, "authored", "retracted", object).is_err());
# Ok::<(), publet_core::SigError>(())
```

## Example: non-canonical bytes are refused, by rule

```
use publet_core::cbor::decode;

// 5 written in two bytes; its canonical form is the single byte 0x05.
let error = decode(&[0x18, 0x05]).unwrap_err();
assert_eq!(error.rule(), "shortest-form integers");

// {"b": 1, "a": 2}: keys out of order.
let error = decode(&[0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02]).unwrap_err();
assert_eq!(error.rule(), "sorted map keys");
```

## Example: identifiers are text

```
use publet_core::{Cid, CidError, HashAlg};

let cid = Cid::of(b"", HashAlg::Sha2_256);
let text = cid.to_string();
assert_eq!(text, "pub:sha2-256:4oymiquy7qobjgx36tejs35zeqt24qpemsnzgtfeswmrw6csxbkq");
assert_eq!(text.parse::<Cid>(), Ok(cid));

assert_eq!("ipfs:bafy".parse::<Cid>(), Err(CidError::MissingScheme));
```

# Why this crate?

Content addressing works only if equal content has equal bytes. CBOR allows
many encodings of one value: integers padded to longer forms, keys in any
order, lengths left open. General-purpose decoders accept them all; `ciborium`
and `minicbor` both decode a padded integer to the same value as the
canonical one (see [Related crates](#related-crates)), so a program that
checked the decoded value would accept bytes with a different identifier. Section 4.1 requires rejecting non-canonical bytes outright, never
repairing them. The decoder here checks every rule while parsing, so the
first violation is reported with its offset, and no value is built from bad
input.

The identifiers are the protocol's own (Section 4.2): `pub:` and an algorithm
name, then the digest in lowercase base32. The algorithm travels in the
identifier, so a future migration is a new name rather than a flag day.

And the types make the checks hard to skip. Nothing returns an object you can
act on without verifying it against an identifier first, and [`verify`]
takes the purpose you require as an argument, so a signature cannot be
checked without one.

# Related crates

Crates that do similar jobs, and why this one takes another approach. Each
was checked at the version shown. The CBOR libraries were given encodings
Section 4.1 forbids: integers not in shortest form, indefinite lengths,
unsorted and duplicate keys, non-text keys, floats, tags, non-NFC text, and
trailing bytes.

| Crate | What it is | Why not here |
|---|---|---|
| [`ciborium`](https://crates.io/crates/ciborium) 0.2.2 | General CBOR with serde | Accepted every forbidden encoding, decoding several to the same values as the canonical bytes, so a later check cannot tell them apart. Its canonical key order differs from Section 4.1's. |
| [`minicbor`](https://crates.io/crates/minicbor) 2.3.0 | Small `no_std` CBOR codec | Lenient in the same way: it accepted the six forbidden encodings tried, reading a non-shortest integer as the plain value. |
| [`dcbor`](https://crates.io/crates/dcbor) 0.25.2 | Deterministic CBOR | Strict, and rejects most forbidden encodings. But it orders map keys by encoded length (RFC 8949 §4.2.1), while Section 4.1 orders them by UTF-8 bytes, so it rejects this crate's canonical objects. It also admits floats, tags and non-text keys. |
| [`serde_cbor`](https://crates.io/crates/serde_cbor) 0.11.2 | CBOR with serde | Unmaintained since 2021. |
| [`cid`](https://crates.io/crates/cid) | IPFS content identifiers | A multiformats CID carries a version, a codec and a multibase prefix (`bafy…`). Section 4.2 defines a different text form, `pub:<algorithm>:<base32>`, so its parser and printer don't apply. |

What this crate builds on rather than reimplements: [`sha2`](https://crates.io/crates/sha2)
for SHA-256, [`ed25519-dalek`](https://crates.io/crates/ed25519-dalek) for
signatures, and [`unicode-normalization`](https://crates.io/crates/unicode-normalization)
for the NFC check.

# Crate features

None. Everything in the crate is always available.

# Minimum Rust version

This crate requires Rust 1.98 (its `rust-version`). The minimum may rise in
a release that bumps the minor version, and never in a patch release.
*/

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
#![warn(missing_debug_implementations)]

pub mod cbor;
mod cid;
mod error;
mod object;
mod sig;

pub use cid::{Cid, CidError, HashAlg};
pub use error::CanonError;
pub use object::{Builder, Object, ObjectError, PROTOCOL_VERSION, Unverified, Verified};
pub use sig::{SigAlg, SigError, sign, signing_message, verify, verifying_key};

/// Longer documents: testing, the supply chain, security, and the changelog.
///
/// These modules hold no code. Each is a document, kept in the repository as
/// Markdown and rendered here so it is versioned with the crate.
// Prose documents, one of them generated from release records: clippy's
// backtick rule for API docs does not fit them.
#[allow(clippy::doc_markdown)]
pub mod _documentation {
    /// How every public item is tested: the examples in its documentation, and
    /// the tests that say they cover it.
    #[doc = include_str!("../TESTING.md")]
    pub mod testing {}

    /// Every crate this one depends on, and how its code is vouched for.
    #[doc = include_str!("../SUPPLY-CHAIN.md")]
    pub mod supply_chain {}

    /// How to report a vulnerability, and what is checked before each release.
    #[doc = include_str!("../SECURITY.md")]
    pub mod security {}

    /// Every release, newest first, generated from the package publet.
    #[doc = include_str!("../CHANGELOG.md")]
    pub mod changelog {}
}

/// The README's examples, run as tests without being rendered twice.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
