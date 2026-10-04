# Content-addressed code: where this is going

Tier 1 exists: [`tree`](src/tree.rs) gives any nested structure a
recursive Merkle hash, and `pub-code-tree` (in the `publet` workspace)
points it at real Rust files -- every `fn`, `struct`, `enum`, `trait`,
`impl`, `mod` gets a CID computed from its own exact source text, not
guessed at from a hand-written citation. This document is the plan for
the two tiers beyond that, and it leans on machinery `publet` already
specifies rather than inventing new protocol where existing protocol
already fits.

The source file stays the logical source of truth throughout. Git,
diffs, `rustfmt`, an editor's jump-to-definition -- none of it stops
working. What changes is that alongside the file, every definition in
it also has a durable, content-addressed identity a knowledge graph can
point at, cite, and relate to other definitions.

## Tier 2, part A: a stable name over an evolving hash

A content hash is the right *identity* for a piece of code -- unambiguous,
tamper-evident, exactly the thing R1 already insists on for every other
publet object. It is a poor *name*: nobody wants to discuss
`pub:sha2-256:vvlol4i2ut4um7wa7d2qyutcxyiako5lnly5k4f53iqok4gxhwjq` in a
design review, and the moment a function's body changes, its hash does
too, which is correct and also means the hash cannot be the thing a
long-lived reference points at.

Section 9.3 already names this exact problem and solves it for
definitional lineages: an **anchor**,

```
{ type: "anchor", body: { lineage: <genesis CID>, current: <CID>,
    rationale, stewards, threshold, ... } }
```

is "a maintained shortcut to the head of a lineage." Point one at a
definition instead of a definitional publet and the properties transfer
exactly: `lineage` is the genesis CID (the function's first published
version, the identity that survives every edit), `current` is whichever
CID is presently live, and every edit in between is an ordinary
`supersedes` link, so the full history stays walkable and nothing is
silently overwritten. The anchor's own key *is* the canonical Rust path
-- `graphset::propagate::propagate_all` -- so the human-legible name and
the cryptographic identity coexist without either standing in for the
other. Moving what a name points at requires `threshold` steward
signatures, not a bare write, which is what keeps "swappable" from
degrading into "ambiguous": the pointer can move, but only as a signed,
attributed act with a `rationale` CID explaining why.

**What this needs**: a `pub anchor` command to author these (mirroring
`pub domain`), and a convention for what "the canonical name" is for a
Rust item (the module path is the obvious default; `impl` methods use
`Type::method` the way `pub-code-tree`'s own output already does).

## Tier 2, part B: interface and implementation, without a new formalism

Rust already has the isomorphism this needs: a `trait` is an interface,
an `impl Trait for Type` block is one implementation of it, and the
relationship between them is explicit in the syntax, not inferred from
naming or documentation. `pub-code-tree`'s AST walk can already tell
these apart -- hashing a trait's signatures separately from an impl
block's bodies is a small extension of Tier 1, not new ground.

What's missing is a relation kind to say it in the graph. Checked the
existing table (§9, R6) for a fit and there isn't a good one:
`translates` is about renderings of *one* claim with a fidelity level
(literal/idiomatic/adapted) -- the wrong shape, since two implementations
of one interface aren't restatements of each other. `equivalent` asserts
"the same claim" outright, which overclaims exactly the way the ZFC/Rust
discussion two turns ago warned against -- two different algorithms
satisfying one contract are not identical claims even when behaviorally
interchangeable.

**Proposed**: a new relation kind, `implements` (`from` implements the
interface `to`), analogous in shape to `translates` but for behavioral
conformance rather than notation. This is a real spec change -- touches
`RelationKind`, the relation table, acyclicity rules -- and belongs to
you as the spec's author, not something to add unilaterally to the enum.
Free functions that share a signature with no common trait are the
harder case that comes after: "same interface" there means structural
signature matching (generics, lifetimes, where-clauses all complicate
what "same" means), deliberately deferred until the trait-mediated case
is proven.

## Tier 3: context-aware selection

Once several CIDs `implements` one interface, *which one gets compiled
in* is a choice -- and publet already has a mechanism for "which of
several candidates wins, and why" that isn't invented for this purpose:
viewpoint-relative evaluation (R7). A trust policy selects which roots
get weight and, transitively, which claims stand. A **selection policy**
over `implements` candidates is the same shape of question one layer
down: given an interface, a target (architecture, optimize-for-size vs.
speed, whatever the build cares about), and a set of implementing CIDs,
which one wins -- the same way a `Policy` object already picks trust
roots rather than the protocol hard-coding an answer.

This is genuinely speculative and depends on `implements` existing
first. Worth designing deliberately once there, not now: what a
selection policy's fields should be, whether selection happens at build
time or is baked into a generated `#[cfg]`, how a reader audits *why* a
particular implementation was chosen for a particular build the way they
can already audit why a claim reached a particular standing.

## Sequencing

1. Tier 1 (done): per-definition CIDs, computed and verified.
2. Wire `pub-code-tree`'s output into the corpus pipeline as the
   identifier source, replacing the locator-regex heuristic.
3. `pub anchor`, and the naming convention for what an anchor's key is.
4. Extend `pub-code-tree` to separate a trait's interface hash from an
   `impl` block's implementation hash.
5. Propose `implements` as a spec change; once accepted, generate it
   automatically from `impl Trait for Type` syntax.
6. Design the selection-policy shape, with real build scenarios in hand
   rather than in the abstract.
