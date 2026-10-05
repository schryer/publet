# publet-algorithms

The generic algorithms the [publet](https://github.com/schryer/publet) tools
are built on. None of them knows anything about publet: this crate depends
on no other publet crate, and its only dependency is `sha2`. Nodes are
strings, hashes are 32-byte arrays and weights are `Fixed6`. A caller with
richer identifiers converts at its own boundary.

| Module | What it does |
|---|---|
| `log` | An append-only Merkle log as in RFC 6962: inclusion proofs, consistency proofs that a later log extends an earlier one, and a power-of-two checkpoint schedule for catching up |
| `membership` | A sorted set with a root hash: prove a value is present, or provably absent |
| `tree` | Git-style recursive hashing of named, nested content |
| `fixed` | `Fixed6`, base-10⁶ fixed-point arithmetic with no conversion from floats, so results are bit-identical everywhere |
| `propagate` | Personalized PageRank over a weighted graph, run for an exact number of iterations, with edges that can be limited to subjects and lose weight with age |
| `graph` | Traversals: reachability, closure that refuses a cycle, a chain's root, and topological order that names the cycle when there is one |
| `version` | `MAJOR.MINOR.PATCH` arithmetic, and requirement matching (`^X.Y`, `X.Y+`, prefixes) |

Every traversal and accumulation runs in a fixed order, so the same input
always gives the same output, down to the bit.

## Prove an entry is in a log

```rust
use publet_algorithms::log::{inclusion_proof, leaf_hash, root, verify_inclusion};

let leaves: Vec<_> = ["first", "second", "third"]
    .iter()
    .map(|entry| leaf_hash(entry.as_bytes()))
    .collect();
let head = root(&leaves);

// Anyone holding only the root can check the proof.
let path = inclusion_proof(&leaves, 1);
assert!(verify_inclusion(&leaves[1], 1, leaves.len(), &path, &head));
assert!(!verify_inclusion(&leaf_hash(b"forged"), 1, leaves.len(), &path, &head));
```

## Prove something is absent from a set

```rust
use publet_algorithms::membership::{Membership, Proof, verify};

let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
let root = members.root();

let proof = members.prove("banana");
assert!(matches!(proof, Proof::Absent { .. }));
// "apple" and "cherry" are adjacent, so nothing lies between them.
assert!(verify("banana", &proof, &root));
```

## Order work by its dependencies

```rust
use std::collections::BTreeMap;
use publet_algorithms::graph::topological_order;

let deps = BTreeMap::from([
    ("app", vec!["lib", "util"]),
    ("lib", vec!["util"]),
]);
let needs = |id: &str| -> Vec<String> {
    deps.get(id).into_iter().flatten().map(|d| d.to_string()).collect()
};

let order = topological_order(["app", "lib", "util"], needs).unwrap();
assert_eq!(order, ["util", "lib", "app"]);

// A cycle is reported as the path that closes it.
let cyclic = |id: &str| vec![if id == "a" { "b" } else { "a" }.to_string()];
assert_eq!(topological_order(["a"], cyclic).unwrap_err(), ["a", "b", "a"]);
```

## Exact arithmetic, and versions

```rust
use publet_algorithms::Fixed6;
use publet_algorithms::version::{Bump, Version, satisfies};

let third = Fixed6::ONE.ratio(Fixed6::from_integer(3));
assert_eq!(third.to_string(), "0.333333");

let release = Version::parse("1.4.2").unwrap().bump(Bump::Minor);
assert_eq!(release.to_string(), "1.5.0");
assert!(satisfies("1.98.1", "1.80+"));
assert!(satisfies("0.3.2", "^0.1"));
assert!(!satisfies("0.150", "0.15"));
```

## Related crates

Crates that do similar jobs, and why this one takes another approach. Each
was checked at the version shown.

| Crate | What it is | Why not here |
|---|---|---|
| [`ct-merkle`](https://crates.io/crates/ct-merkle) 0.3.0 | The RFC 6962 append-only log | The same construction as `log`. This crate exposes it as plain functions over a slice of leaf hashes, which `membership`'s sorted tree is built from, rather than a tree type. |
| [`rs_merkle`](https://crates.io/crates/rs_merkle) 1.5.0 | General Merkle trees | Not RFC 6962: interior nodes are hashed without the RFC's domain separation, so its root for the same leaves differs and its proofs cannot be checked against a transparency log's. |
| [`petgraph`](https://crates.io/crates/petgraph) 0.8.3 | Graph types and algorithms | Its PageRank uses floating point, which bit-identical results rule out, and its traversals need the graph built as a structure first. |
| [`pathfinding`](https://crates.io/crates/pathfinding) 4.16.0 | Graph algorithms over successor functions | Close to `graph`'s design, but its topological sort reports a cycle as a single node rather than the path, and its visit order is not a documented guarantee. Results here depend on that order. |
| [`rust_decimal`](https://crates.io/crates/rust_decimal) 1.43.0 | Decimal arithmetic | Deterministic, but with different precision and rounding from `Fixed6`. Switching would change every propagation result. |
| [`semver`](https://crates.io/crates/semver) 1.0.28 | Cargo's flavour of semantic versioning | `version::satisfies` also matches tool versions that aren't semver, such as `0.15` and `1.80+`. |

## Versions

`publet-algorithms` is released on a version track of its own. Its releases
are recorded as versions of the package publet `pkg.publet-algorithms` in
the [publet repository](https://github.com/schryer/publet).

Licensed under Apache-2.0.
