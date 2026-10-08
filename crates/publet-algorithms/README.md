# publet-algorithms

The generic algorithms the [publet](https://github.com/schryer/publet) tools
are built on: an append-only Merkle log (RFC 6962), sorted-set membership with
absence proofs, recursive content trees, exact fixed-point arithmetic,
deterministic trust propagation, graph traversals, and version requirements.
None of them depends on anything publet-specific.

Licensed under Apache-2.0.

### Documentation

[docs.rs/publet-algorithms](https://docs.rs/publet-algorithms): the overview,
a cookbook of examples, why exactness, the crates considered instead, how
every item is tested, and how each dependency is vouched for.

### Usage

```toml
[dependencies]
publet-algorithms = "0.2"
```

### Example

An entry proved to be in a log, by someone holding only the log's root:

```rust
use publet_algorithms::log::{inclusion_proof, leaf_hash, root, verify_inclusion};

let leaves: Vec<_> = ["first", "second", "third"]
    .iter()
    .map(|entry| leaf_hash(entry.as_bytes()))
    .collect();
let head = root(&leaves);

let path = inclusion_proof(&leaves, 1);
assert!(verify_inclusion(&leaves[1], 1, leaves.len() as u64, &path, &head));
```

### Minimum Rust version

Rust 1.85, tested in CI. The minimum may rise in a release that bumps the
minor version, and never in a patch release.

### Security

Report a vulnerability privately through the repository's Security tab.
[`SECURITY.md`](SECURITY.md) describes what is checked, and
[`SUPPLY-CHAIN.md`](SUPPLY-CHAIN.md) how each dependency is vouched for.
