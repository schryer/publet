# publet-core

Objects, canonical serialization, content identifiers and signatures for the
[Publet Protocol](https://github.com/schryer/publet/blob/main/spec/index.md):
the floor every other publet crate and tool builds on.

Licensed under Apache-2.0.

### Documentation

[docs.rs/publet-core](https://docs.rs/publet-core): the overview, a cookbook
of examples, why this crate exists, the crates considered instead, how every
item is tested, and how each dependency is vouched for.

### Usage

```toml
[dependencies]
publet-core = "0.1"
```

### Example

An object built, named by its identifier, and checked against it:

```rust
use publet_core::{Cid, HashAlg, Object, cbor::Value};

let author = Cid::of(b"the author's key object", HashAlg::Sha2_256).to_string();

let bytes = Object::builder("claim.prose", &author)
    .created("2026-10-05T00:00:00Z")
    .field("content", Value::Text("Water boils at 100 °C at sea level.".into()))
    .build()?;
let cid = Cid::of(&bytes, HashAlg::Sha2_256);

let object = Object::parse(&bytes)?.verify(&cid)?;
assert_eq!(object.object().kind(), "claim.prose");
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Minimum Rust version

Rust 1.85, tested in CI. The minimum may rise in a release that bumps the
minor version, and never in a patch release.

### Security

Report a vulnerability privately through the repository's Security tab.
[`SECURITY.md`](SECURITY.md) describes what is checked, and
[`SUPPLY-CHAIN.md`](SUPPLY-CHAIN.md) how each dependency is vouched for.
