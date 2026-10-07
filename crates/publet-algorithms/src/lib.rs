/*!
The generic algorithms the [publet] tools are built on: an append-only Merkle
log, sorted-set membership with absence proofs, recursive content trees, exact
fixed-point arithmetic, deterministic trust propagation, graph traversals, and
version requirements.

None of them knows anything about publet. Nodes are strings, hashes are
32-byte arrays and weights are [`Fixed6`]; a caller with richer identifiers
converts at its own boundary. Here is an entry proved to be in a log, by
someone holding only the log's root:

```
use publet_algorithms::log::{inclusion_proof, leaf_hash, root, verify_inclusion};

let leaves: Vec<_> = ["first", "second", "third"]
    .iter()
    .map(|entry| leaf_hash(entry.as_bytes()))
    .collect();
let head = root(&leaves);

let path = inclusion_proof(&leaves, 1);
assert!(verify_inclusion(&leaves[1], 1, leaves.len(), &path, &head));
assert!(!verify_inclusion(&leaf_hash(b"forged"), 1, leaves.len(), &path, &head));
```

[publet]: https://github.com/schryer/publet

# Overview

| Module | What it does |
|---|---|
| [`log`] | An append-only Merkle log as in RFC 6962: inclusion proofs, consistency proofs that a later log extends an earlier one, and a power-of-two checkpoint schedule for catching up |
| [`membership`] | A sorted set with a root hash: prove a value is present, or provably absent |
| [`tree`] | Git-style recursive hashing of named, nested content |
| [`fixed`] | [`Fixed6`], base-10⁶ fixed-point arithmetic with no conversion from floats |
| [`propagate`] | Personalized `PageRank` over a weighted graph, run for an exact number of iterations, with edges that can be limited to subjects and lose weight with age |
| [`graph`] | Traversals over a neighbour function: reachability, a closure that refuses a cycle, a chain's root, and a topological order that names the cycle when there is one |
| [`version`] | Whether a tool's version meets a requirement: `^X.Y` as Cargo reads it, `X.Y+`, or a prefix |

Every traversal and accumulation runs in a fixed order, so the same input
always gives the same output, down to the bit. That is the property publet
needs most: two people evaluating the same objects must get the same answer.

The rest of this page is:

* [Examples](#examples): a short cookbook.
* [Why this crate?](#why-this-crate): what belongs here, and why exactness.
* [Related crates](#related-crates): what was considered instead, and why not.
* [Crate features](#crate-features) and [Minimum Rust version](#minimum-rust-version).

Longer documents live in [`_documentation`]: how every item is
[tested](crate::_documentation::testing), how each dependency is
[vouched for](crate::_documentation::supply_chain), the project's
[security](crate::_documentation::security) policy, and the
[changelog](crate::_documentation::changelog).

# Examples

## Example: prove something is absent from a set

```
use publet_algorithms::membership::{Membership, Proof, verify};

let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
let root = members.root();

let proof = members.prove("banana");
assert!(matches!(proof, Proof::Absent { .. }));
// "apple" and "cherry" are adjacent, so nothing lies between them.
assert!(verify("banana", &proof, &root));
```

## Example: order work by its dependencies

```
use std::collections::BTreeMap;
use publet_algorithms::graph::topological_order;

let deps = BTreeMap::from([("app", vec!["lib", "util"]), ("lib", vec!["util"])]);
let needs = |id: &str| -> Vec<String> {
    deps.get(id).into_iter().flatten().map(|d| d.to_string()).collect()
};
assert_eq!(topological_order(["app", "lib", "util"], needs).unwrap(), ["util", "lib", "app"]);

// A cycle is reported as the path that closes it.
let cyclic = |id: &str| vec![if id == "a" { "b" } else { "a" }.to_string()];
assert_eq!(topological_order(["a"], cyclic).unwrap_err(), ["a", "b", "a"]);
```

## Example: exact arithmetic

```
use publet_algorithms::Fixed6;

let third = Fixed6::ONE.ratio(Fixed6::from_integer(3));
// Truncated, never rounded, so every implementation agrees.
assert_eq!(third.to_string(), "0.333333");
```

# Why this crate?

**What belongs here.** An algorithm belongs here when it needs nothing of
publet's. The crate depends on no other publet crate, and the workspace
checks that mechanically (`tools/check-deps.py`), so no content identifier,
signed object or specification type can reach it. Its only dependency is
`sha2`. The code began inside publet and was extracted after an audit for
coupling to publet's types; that audit's rule is now the dependency check.

**Why exactness.** Results that decide whether a claim stands must come out
the same for everyone who computes them. Floating point cannot promise that:
the result can depend on the platform, the compiler and the order of
operations. So weights are [`Fixed6`] (integers scaled by 10⁶, truncating),
propagation runs an exact number of iterations rather than to a tolerance,
and every map is ordered.

# Related crates

Crates that do similar jobs, and why this one takes another approach. Each
was checked at the version shown.

| Crate | What it is | Why not here |
|---|---|---|
| [`ct-merkle`](https://crates.io/crates/ct-merkle) 0.3.0 | The RFC 6962 append-only log | The same construction as [`log`]. This crate exposes it as plain functions over a slice of leaf hashes, which [`membership`]'s sorted tree is built from, rather than a tree type. |
| [`rs_merkle`](https://crates.io/crates/rs_merkle) 1.5.0 | General Merkle trees | Not RFC 6962: interior nodes are hashed without the RFC's domain separation, so its root for the same leaves differs and its proofs cannot be checked against a transparency log's. |
| [`petgraph`](https://crates.io/crates/petgraph) 0.8.3 | Graph types and algorithms | Its `PageRank` uses floating point, which bit-identical results rule out, and its traversals need the graph built as a structure first. |
| [`pathfinding`](https://crates.io/crates/pathfinding) 4.16.0 | Graph algorithms over successor functions | Close to [`graph`]'s design, but its topological sort reports a cycle as a single node rather than the path, and its visit order is not a documented guarantee. Results here depend on that order. |
| [`rust_decimal`](https://crates.io/crates/rust_decimal) 1.43.0 | Decimal arithmetic | Deterministic, but with different precision and rounding from [`Fixed6`]. Switching would change every propagation result. |
| [`semver`](https://crates.io/crates/semver) 1.0.28 | Cargo's flavour of semantic versioning | Use it for release arithmetic on strict `MAJOR.MINOR.PATCH` versions, as pubrel does; this crate keeps no version type of its own. [`version::satisfies`] exists because tool versions such as `0.15` aren't semver. |

# Crate features

None. Everything in the crate is always available.

# Minimum Rust version

This crate requires Rust 1.98 (its `rust-version`). The minimum may rise in
a release that bumps the minor version, and never in a patch release.
*/

#![deny(missing_docs)]
#![warn(missing_debug_implementations)]

pub mod fixed;
pub mod graph;
pub mod log;
pub mod membership;
pub mod propagate;
pub mod tree;
pub mod version;

pub use fixed::{Fixed6, SCALE};

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
