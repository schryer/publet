# publet-core

Objects, canonical serialization, content identifiers and signatures for the
[Publet Protocol](https://github.com/schryer/publet/blob/main/spec/index.md):
Section 4 of the specification, and the floor every other publet crate and
tool builds on.

It holds the two invariants everything downstream assumes:

- **An object has exactly one byte representation.** Objects are canonical
  CBOR under a strict profile: sorted text keys, shortest integers, NFC text,
  and no floats, tags or indefinite lengths. The decoder rejects anything
  else, so equal content always hashes to the same identifier.
- **An object is not usable until it has been checked.** Parsing yields an
  `Unverified` object; only `verify`, against the identifier you asked for,
  turns it into a `Verified` one.

## Make, identify and check an object

An object's identifier is `pub:sha2-256:` and the lowercase, unpadded base32
of the SHA-256 of its bytes.

```rust
use publet_core::{Cid, HashAlg, Object, cbor::Value};

// An author is named by the identifier of their key object.
let author = Cid::of(b"the author's key object", HashAlg::Sha2_256).to_string();

let bytes = Object::builder("claim.prose", &author)
    .created("2026-10-05T00:00:00Z")
    .field("content", Value::Text("Water boils at 100 °C at sea level.".into()))
    .build()?;

let cid = Cid::of(&bytes, HashAlg::Sha2_256);
assert!(cid.to_string().starts_with("pub:sha2-256:"));

// Bytes fetched from anywhere are checked against the identifier asked for.
let object = Object::parse(&bytes)?.verify(&cid)?;
assert_eq!(object.object().kind(), "claim.prose");

// The same content, built again, is the same object.
let again = Object::builder("claim.prose", &author)
    .created("2026-10-05T00:00:00Z")
    .field("content", Value::Text("Water boils at 100 °C at sea level.".into()))
    .build()?;
assert_eq!(Cid::of(&again, HashAlg::Sha2_256), cid);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Sign and verify

Signatures are detached Ed25519 over the object's bytes and a *purpose*. A
signature made for one purpose never verifies for another.

```rust
use publet_core::{SigAlg, sign, verify, verifying_key};

let object = b"an object's canonical bytes";

// A real seed comes from a random source; this crate needs no RNG.
let seed = [7u8; 32];
let public = verifying_key(SigAlg::Ed25519, &seed)?;

let signature = sign(SigAlg::Ed25519, &seed, "authored", object)?;
verify(SigAlg::Ed25519, &public, &signature, "authored", "authored", object)?;

// Made for "authored", it says nothing about "endorsed".
assert!(verify(SigAlg::Ed25519, &public, &signature, "authored", "endorsed", object).is_err());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Related crates

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
| [`cid`](https://crates.io/crates/cid) | IPFS content identifiers | A multiformats CID carries a version, a codec and a multibase prefix (`bafy…`). Section 4.2 defines a different text form, `pub:<algorithm>:<base32>`, so the parser and printer don't apply. |

What this crate builds on rather than reimplements: [`sha2`](https://crates.io/crates/sha2)
for SHA-256, [`ed25519-dalek`](https://crates.io/crates/ed25519-dalek) for
signatures, and [`unicode-normalization`](https://crates.io/crates/unicode-normalization)
for the NFC check.

## Versions

`publet-core` is released on a version track of its own, separate from the
`pub` command line tool, and changes slowly. Its releases are recorded as
versions of the package publet `pkg.publet-core` in the
[publet repository](https://github.com/schryer/publet).

Licensed under Apache-2.0.
