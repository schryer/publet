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

## Versions

`publet-core` is released on a version track of its own, separate from the
`pub` command line tool, and changes slowly. Its releases are recorded as
versions of the package publet `pkg.publet-core` in the
[publet repository](https://github.com/schryer/publet).

Licensed under Apache-2.0.
