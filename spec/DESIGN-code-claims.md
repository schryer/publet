# Code as a signed, evidenced claim — a design sketch

Not spec text. A sketch of what the relationships and workflow would need
to look like before any of this becomes a real §-numbered section. Grounds
every abstraction in the same running example this session has used
throughout: `accepts_verdict` (`pub:sha2-256:4bugaz4...`), called by
`check_verdict` (`pub:sha2-256:snohvbz...`), both in
`crates/publet-graph/src/view.rs`.

## Where this starts

Two things converged in conversation that don't sit right stacked the way
they currently are:

1. A code CID's identity needs no signature — anyone can recompute
   SHA-256 of the exact bytes and get the same answer, no trust required.
   That part doesn't change below.
2. But "no signature needed for identity" got read as "no author, no
   assertion, nothing to evidence" — which conflates *identity* with
   *correctness*. Git already has an author for every line. A test suite
   already asserts something about behavior. Neither of those facts is
   currently connected to the code's own CID anywhere in this corpus.

The prose "procedural" publets that exist today (`k4jvlucb...`,
*"to check whether a class accepts a verdict: match it against the three
non-truth-apt variants..."*) were standing in for both jobs at once:
*defining* what the procedure does (in lossy prose, written by whoever
curated the corpus, not by whoever wrote the code) and *being the thing
citations point at*. Splitting those jobs apart is what this sketch does.

## The core move: one carve-out, generalized

§6.2 already established that a relation's endpoint need not be a signed
publet object — it can be a raw content identifier over indexed source.
`implements` is the only kind that currently uses this. Nothing below
invents a new formalism; it proposes widening the *same* carve-out to a
few more places it already fits:

| Where | Today | Proposed |
|---|---|---|
| `implements` (`rel`) | endpoints may be code CIDs | unchanged — already correct |
| `translates` (`rel`) | both sides must be signed publets | `to` may be a code CID |
| `reproduction` (`ann`) | `target` must be a signed publet | `target` may be a code CID |
| `assessment` / `verdict` (`ann`) | `target` must be a signed publet | `target` may be a code CID |
| `proof-checked` (`ann`) | `target` must be a signed publet | `target` may be a code CID |
| authorship | doesn't exist | a detached signature (§4.4), `purpose="authored"`, over a code CID |

Nothing here proposes a new object *kind*. Every row reuses machinery
already built and already specified; only the allowed shape of one field
per kind changes.

## The relationships, concretely

```
                              ┌─────────────────────────────┐
                              │  accepts_verdict             │
                              │  pub:sha2-256:4bugaz4...     │
                              │  (Section 6.2 content id --   │
                              │   unsigned, self-verifying)   │
                              └───────────────┬───────────────┘
                                               │
                ┌──────────────────┬───────────┼───────────────┬──────────────────┐
                │                  │           │               │                  │
       sig, purpose=       ann, kind=    ann, kind=      rel, kind=        (existing, unchanged)
        "authored"         reproduction   assessment      translates       usage citations from
       author = git         target =      target =        to = this CID    OTHER claims that cite
       committer's key      this CID      this CID         from = the        this code as evidence
                            outcome=       verdict=         prose publet     (Section 7.5) --
                            consistent     sound            method=human     e.g. a `depends`-class
                            method=        basis="read      fidelity=        definitional publet
                            <test CID>     and reasoned      idiomatic       grounding itself here
                                           about it"
                                               │
                                               ▼
                                    ┌─────────────────────────┐
                                    │  k4jvlucb... (existing)   │
                                    │  "to check whether a      │
                                    │   class accepts a         │
                                    │   verdict: ..."            │
                                    │  class: procedural         │
                                    │  -- now explicitly a       │
                                    │  RENDERING of the code,    │
                                    │  not an independent claim  │
                                    │  about the same territory  │
                                    └─────────────────────────┘
```

Four new or widened edges land on the code CID; the existing prose publet
moves from "the claim" to "a graded rendering of the claim," via
`translates` — which already has exactly the right shape (`method`,
`fidelity`) for "this prose renders that code."

## The workflow, end to end

1. **Write.** Author writes/changes `accepts_verdict`, with a doc comment
   stating intent — already required practice here, and already part of
   the hashed bytes `pub-code-tree` spans (Section 6.2), so it travels
   with the CID for free.
2. **Test.** Author writes or updates a test exercising it.
3. **Commit.** A git hook runs `pub-code-tree` over the touched files,
   diffs the new CIDs against the previous commit's, and for each
   genuinely new-or-changed leaf, signs a detached `sig` (§4.4,
   `purpose="authored"`) over that CID with the committer's own local
   key — the git-signing-hooks piece already deferred earlier in this
   session, now with a concrete first use.
4. **CI runs the suite.** For each test judged to exercise a given code
   CID (see *Open question 1*, below), CI composes a `reproduction`
   annotation: `target=<code CID>`, `outcome=consistent|inconsistent`,
   `method=<CID of the test procedure>`, `result=<what was observed>`.
   Signed either by a key the author provisioned for their own use in
   CI, or staged unsigned (the artifact's existing compose/stage pattern)
   for the author to sign locally after looking at the run.
5. **Corpus build picks both up.** `queue.py` already resolves every
   code card's identity, links, calls, and callers; it now also surfaces
   who authored a given definition and what its reproduction history
   looks like — pass/fail over time, evidenced by real test runs, not a
   human's read-and-judge.
6. **Prose publets relate, not just cite.** Going forward, a publet like
   `k4jvlucb...` gets a `translates` edge to the code CID
   (`method=human`, `fidelity=idiomatic`, say) instead of (or alongside)
   a bare §7.5 citation. A verdict on the *prose* publet now unambiguously
   means "is this rendering faithful" — the question `translates`
   already exists to carry — while a verdict or reproduction on the
   *code CID* means "does the code do what it claims to."
7. **A human reviewer can sign directly against the code**, no
   intermediate publet required: `assessment`, `target=<code CID>`,
   `verdict=sound`, `basis="read it, reasoned about the match arms"`.
   This is the actual code-review step becoming a real, attributed,
   protocol-native claim — closing the loop this sketch started from.

## What stays exactly as it is

- Code identity itself: unsigned, self-verifying, computed the same way,
  by the same tool, from the same bytes. Nothing above signs a CID's
  *existence* — only claims *about* what it does.
- R7 viewpoint-relative evaluation and R9 (reproduction is decisive):
  apply unchanged. A code CID accruing `reproduction`/`assessment`
  annotations is evaluated the same way any publet's evidence already is
  — no new evaluation logic, only a new *shape* of target for the
  existing one to read.
- The 918 code cards that already exist: nothing here retroactively
  requires signing all of them. This is a going-forward workflow. Whether
  to backfill authorship/reproduction for existing code is a separate,
  much larger decision (Open question 4).

## Formal claims about code: Hoare triples and `proof-checked`

Everything above is `empirical`: a claim settled by reproduction, inherently
incomplete (finite tests sample a finite slice of the input space). Some
code claims are stronger than that, and the spec already has a place for
them that nothing has ever pointed at code.

§5.2 lists `formal` as a sibling of `empirical`: "a mathematical or logical
statement," settled by "proof checking," and — the load-bearing detail —
§7.1's `proof-checked` annotation (`system`, `version`, `artifact`,
`result`) is **decisive** under R9/§11.4 exactly the way `reproduction` is:
*"A `formal` publet with an accepted `proof-checked` annotation from a
checker the policy recognizes is `accepted` regardless of endorsement
weight."* A Hoare triple `{P} C {Q}` — a claim that holds for *every* input
satisfying `P`, not just the ones a test happened to try — is a `formal`
claim, and `proof-checked` is already built to carry it. The only change
needed is the same one-line widening as the row above: `target` may be a
code CID.

### Where P, C, and Q actually live

`C` is the code CID — settled already. `P` and `Q` belong in the source
itself, as machine-checkable contract attributes: Prusti's
`#[requires(...)]`/`#[ensures(...)]`, Creusot's equivalent, or a Kani proof
harness. Because these are ordinary Rust, `pub-code-tree` already hashes
them into the function's span for free, the same way it already hashes doc
comments — no new field, no new object. A maintainer who writes the
contract gets it captured by the mechanism that exists today.

This splits `artifact` into two real shapes, depending on how the checker
attaches to the code:

- **Self-contained contract** (Prusti, Creusot): `P`/`Q` are attributes on
  `C` itself. `artifact` is the *same* CID as `target` — the checker reads
  the function and its own attached contract in one pass.
- **External harness** (Kani): a separate function sets up symbolic inputs
  (encoding `P`) and asserts a property (encoding `Q`) by calling `C`.
  `artifact` is the harness's *own* code CID, distinct from `target`.

The external-harness case has a structural check available for free: a
harness is just a Rust function, so `calls_resolve.py`'s existing
call-graph extraction already records what it calls. Before trusting a
`proof-checked` annotation whose `artifact` differs from its `target`,
tooling can confirm `target` is actually in `artifact`'s call closure —
catching a harness that doesn't even reference the function it claims to
prove something about. This is a structural finding (`compose.rs`'s
existing "warnings, not errors" pattern), not a new validity rule.

One known gap, not solved here: a single harness establishing a joint
property over two functions has no clean way to name both as `target` —
`ann`'s `target` is singular. Composing one `proof-checked` annotation per
covered function, sharing one `artifact`, is workable and matches how every
other annotation kind already has exactly one target; a real multi-target
grouping mechanism would be new machinery this doesn't propose.

### What `proof-checked.value` needs, beyond the one-line table entry

Unlike `reproduction`, `proof-checked` has no dedicated subsection in the
spec today — the §7.1 table row (`system, version, artifact, result`) is
the whole current definition. Making it operationally re-runnable for code
needs two things the table row doesn't carry:

- **`config`**: a bounded model checker's proof is only as strong as its
  bounds. A Kani harness checked with `--unwind=5` does not establish the
  same thing as one checked with `--unwind=50` if there is an unbounded
  loop in play — the bound is part of what was verified, not incidental
  configuration. Without recording it, a `proof-checked` annotation
  overclaims.
- **A `result` vocabulary that admits "the checker didn't finish"**:
  `reproduction` already separates `inconsistent` (a real counterexample)
  from `inconclusive` (the attempt didn't settle anything). A proof check
  needs the same split — `refuted` (a real counterexample) is a materially
  different finding than `timeout` (the solver ran out of budget), and
  collapsing them would make a resource limit look like a correctness
  finding.

Proposed shape (extending, not replacing, the existing table row):

```
value: { system:   <string>,   // "kani" | "verus" | "prusti" | ...
         version:  <string>,   // checker version, pinned
         artifact: <CID>,      // the code CID actually checked (contract
                                // -bearing C, or an external harness)
         reference: <CID>,     // translation validation only: the already
                                // -proven code this was checked against
         config:   <CID of blob>,  // flags, bounds, solver, timeout
         result:   "verified" | "refuted" | "timeout" | "inconclusive",
         data:     <CID of blob>,  // raw checker output
         independence: { ... } }   // reuses reproduction's shape (§7.2)
```

### Erasure is what makes this free, and it comes in three strengths

The common objection to provable code — that it produces slower binaries —
conflates two unrelated things. Verifying *hand-written source* costs
nothing at runtime: Verus classifies code as `spec`, `proof`, or `exec`,
and ghost code is erased after checking, yielding "a normal Rust
executable with all verification overhead removed." Kani reaches the same
place by convention, with harnesses behind `#[cfg(kani)]` so a normal
build is untouched. Verified crypto in production bears this out in the
other direction: Firefox's HACL*-derived Curve25519 was *faster* than the
hand-written code it replaced.

What genuinely does cost performance is verifying a *transformation* —
CompCert runs 7–12% behind `gcc -O1`/`-O2` because proving an aggressive
optimizer correct is far harder than proving a conservative one, so it
stayed conservative. That tradeoff applies to verified compilers, not to
verified application code, and the distinction is worth keeping sharp.

The erasure guarantee itself comes in three strengths, and a
`proof-checked` annotation is only as good as the one behind it:

| Strength | Mechanism | Example |
|---|---|---|
| Language-enforced | ghost/exec mode separation is type-checked | Verus |
| Convention-enforced | ordinary conditional compilation | Kani's `#[cfg(kani)]` |
| None — checks ship and run | runtime contract decorators left enabled | `deal`'s `@deal.pre` in its runtime mode |

The third row should not be eligible for `proof-checked` at all: nothing
was proven, only checked on the paths execution happened to take. That is
`reproduction` evidence at best.

### Translation validation: proving the fast one matches the proven one

Fiat-Crypto's pipeline is the closest existing analogue to the
implementation-variant problem. A reference is proven once in Coq;
optimized derivatives are synthesized (CryptOpt generates x86-64
assembly reported to beat GCC and Clang, and sometimes hand-written
assembly); and every derivative is checked against the reference by a
**translation validator** rather than re-proved from scratch. BoringSSL's
own notes record that even hand-edited generated assembly gets re-checked
this way.

This is a third evidentiary strength, between the other two: not sampled
like tests, not a from-scratch proof either, but a full proof of
*equivalence to something already proven*. It fits `proof-checked` with
`system` naming the validator and the proposed `reference` field naming
the proven code CID the artifact was validated against — and it is
exactly what an efficiency-variant track wants, since re-proving every
vectorized rewrite from first principles is the cost that makes people
give up on verification.

### Not every checker deserves the same weight

R9/§11.4's exact wording is *"a checker **the policy recognizes**."* That
hedge is load-bearing. `deal`'s verification mode is backed by CrossHair,
which its own documentation describes as "something in between deal tests
and formal verification" — a symbolic tester that can sometimes exhaust
all paths but offers no general guarantee. A policy can reasonably treat
Kani or Verus as decisive and CrossHair as strong-but-not-decisive, and
`system`/`version`/`config` are what give it something concrete to
discriminate on. No change needed here; the spec already anticipated it.

### What "independently re-runnable" means operationally

1. Fetch `artifact` — unambiguous, because it's content-addressed; there is
   no "which revision" question.
2. Reproduce the environment `config` names: same checker `system`@
   `version`, same bounds/flags/solver.
3. Run it. If `artifact ≠ target`, confirm via the call graph that
   `artifact` still exercises `target` (source may have moved on).
4. Compare the verdict to `value.result`.

A mismatch here is a sharper signal than an inconsistent empirical
reproduction: a deterministic checker given the exact same input and
configuration should return the exact same answer. A mismatch means either
`config` was incomplete (didn't actually pin everything that mattered),
the checker itself is non-deterministic (real, for SMT-backed tools —
solvers like Z3 are not always bit-for-bit deterministic under parallel or
randomized search even with a fixed seed), or the original `result` was
wrong. `timeout` as its own outcome value exists precisely so a resource
limit on one machine doesn't get read as evidence of either.

### Scope: this is optional, not a replacement

`formal` and `empirical` are siblings in §5.2, not a hierarchy. Most
functions in this codebase — string handling, CLI parsing, graph
traversal — will never get a Kani harness, and nothing here expects them
to. This is a stronger, opt-in track for the functions where the cost pays
for itself: this repo's own canonical CBOR codec is a real candidate —
`rejects_non_shortest_integer`, `rejects_unsorted_map_keys`,
`rejects_indefinite_length` in
[canonical.rs](crates/publet-core/tests/canonical.rs) are exactly the
invariants a bounded model checker earns its keep on, currently only ever
tested empirically. A property-based test (`proptest`) sits between the
two, worth knowing about but worth keeping distinct: it makes `P` explicit
as a generator and `Q` explicit as a predicate, both still ordinary,
freely-hashed Rust source — but it is still sampling, still `empirical`,
not `proof-checked`.

## Declaring the contract test set

Q1 (below) leans coverage-based for the broad evidentiary layer: automatic,
honest, and already-planned. But coverage is a noisy signal for a
different, narrower question this design also needs answered: *are two
differently-implemented code CIDs alternative renderings of the same
claim?* Two behaviorally-equivalent implementations rarely have
byte-identical coverage footprints — an optimized version may skip a
helper the naive one calls — so raw coverage overlap would make renderings
look like different claims more often than not.

The resolution: a function's *contract* — the specific tests that define
its claim for equivalence and ownership-propagation purposes — is a
**declared, curated** set, distinct from whatever coverage happens to
touch it. Concretely, no new object or relation kind: a lightweight,
in-source convention marks which tests are the contract (a `#[cfg(test)]`
module or function tagged by name or doc-comment convention that a small
extension to `pub-code-tree`/`queue.py` recognizes, the same way it already
recognizes `syn::Item::Fn`), and the *provenance* of a `reproduction`
annotation's `method` is what distinguishes the two evidentiary layers:

- **Coverage-based** (CI-composed, automatic): `method` names whatever test
  the coverage tool found touching this code. Broad, cheap, evidentiary —
  unchanged from Q1's answer.
- **Declared** (owner-composed, deliberate): `method` names a test the
  owner explicitly marked as the contract. This is the set an
  implementation must reproduce, with `outcome: consistent`, to be
  claimed as satisfying the contract at all — and it is what an owner's
  `assessment`/authored `sig` is understood to cover: verification
  attaches to *(declared contract, outcome)*, not to one implementation's
  bytes.

A correction to an earlier draft of this sketch, which described two
differently-implemented code CIDs passing the same tests as "alternative
renderings of one claim." §6.2 rejects that framing outright, and is
right to: *"`equivalent` is the wrong relation for this: it asserts
`from` and `to` express the same claim, which two independently written
implementations of one interface do not — they satisfy a shared contract
while remaining, in general, different claims about how."* A vectorized
rewrite **is** a different claim about how — the *how* is exactly what
changed. The correct structure is the next section's: one contract,
several `implements` edges, each independently evidenced. What variants
share is the contract, never claim identity.

This is real, new curation work — a maintainer marks their contract tests
once, deliberately — but it is small, matches how this corpus has already
chosen declared-over-inferred elsewhere when the two diverge, and composes
cleanly with everything above: a code CID can carry a coverage-evidenced
`reproduction`, a declared-contract `reproduction`, *and* a `proof-checked`
annotation simultaneously, each answering a different question at a
different strength.

## One algorithm, many implementations

The question this section answers: can the core knowledge live once, in a
language-neutral form, with every implementation across languages,
architectures, and environments derived from and linked to it?

§6.2 already says yes, in language that reads as though it were written
for exactly this and then never used:

> An `implements` edge states that one thing realizes a contract another
> thing declares — a concrete algorithm satisfying an abstract interface,
> where several implementations MAY exist side by side and a consumer of
> the interface is agnostic to which one it holds.

`implements` is also the *one* relation already permitted to have a raw
code CID as an endpoint. Nothing needs widening. The structure falls out:

```
        ┌──────────────────────────────────────────────┐
        │  ABSTRACT ALGORITHM  (publet, class: formal)  │
        │  content: the algorithm + its laws            │
        │  scope:   the precondition domain             │
        │  states:  invariants, complexity bound        │
        └───────▲──────────────▲──────────────▲─────────┘
                │              │              │
          implements     implements     implements
                │              │              │
        ┌───────┴──────┐ ┌─────┴──────┐ ┌────┴─────────┐
        │ Rust code CID│ │ Python CID │ │ C code CID    │
        │ Verus-proved │ │ CrossHair  │ │ extracted from│
        │              │ │ + contract │ │ F*, refinement│
        │              │ │   tests    │ │ proof         │
        └───────┬──────┘ └─────┬──────┘ └────┬──────────┘
                │              │              │
         reproductions, one per (target triple, toolchain)
         -- the environment axis, per the earlier section
```

Three axes, three mechanisms, none of them new:

| Axis | Varies | Mechanism |
|---|---|---|
| Language | Rust, Python, C | one `implements` edge each |
| Implementation strategy | scalar vs vectorized, within one language | one `implements` edge each — *not* `equivalent` |
| Environment | target triple, toolchain | several `reproduction` annotations on one code CID |

### What evidences the refinement

§6.2 defines the edge but says nothing about what makes it *credible*.
An `implements` edge is an assertion by whoever authored it (R6: relations
are authored by anyone, not only the endpoints' authors) — on its own it
is a claim, not a fact.

The proposal: a `rel` is an object with its own CID, and annotations
target any CID, so **evidence accrues to the edge**. A `proof-checked`
annotation targeting the `implements` edge says a checker confirmed this
implementation satisfies that contract; a `reproduction` targeting the
edge says the implementation passed the contract's declared tests. Either
way the claim being evidenced is the *conformance*, not a property of
either endpoint alone.

This matches how refinement has always been treated — in seL4, the
B-Method, and Event-B, refinement is a proof obligation *between* levels,
never a property of one level. A graph whose relations are first-class
objects can carry that natively. Nothing in §7 forbids annotating a
relation; nothing explicitly blesses it either, so this needs confirming
before it is built on.

### Stating the contract without picking a language

Three approaches exist in the prior art:

- **Mathematical prose** (Z, B, VDM): neutral, but every implementation
  link then needs a human-mediated refinement proof in whatever logic
  that ecosystem's verifier speaks.
- **An executable reference in a pure subset** (HACL*'s F* spec,
  Fiat-Crypto's Coq model): can be *run* as an oracle, which is why it
  dominates in practice — differential testing against a proven reference
  is cheap and strong.
- **Algebraic laws**: round-trip, ordering, idempotence, totality —
  stated once, translatable into any ecosystem's property-testing or
  verification framework.

For this corpus: the abstract publet carries the algorithm and its laws;
each ecosystem carries a *rendering* of those laws as executable contract
tests; and the edge from those tests back to the abstract laws is
`translates`, with its existing `method` and `fidelity` — a lossy
rendering of a single claim, which §6.2 says is precisely what
`translates` is for and what `implements` is not. Both relations end up
exactly where the spec already put them.

### Complexity bounds decide what counts as the same contract

If the abstract publet states a bound, conformance to it is checkable
rather than a judgement call: a constant-factor optimization satisfies
`O(n log n)` and is another `implements` edge; something that changes
complexity class fails the contract and needs its own. This is a firmer
line than "logical change versus efficiency change," which relies on a
human deciding which one they just made.

Worth knowing: essentially no mainstream verifier checks complexity by
default. Verus checks termination, not cost. So a stated bound is, for
now, an ordinary claim evidenced by measurement (`empirical`) rather than
by proof — which is honest, and which the existing machinery already
handles.

### Verification does not flow down the edges

The failure mode this architecture invites: a Verus proof about the Rust
implementation establishing nothing whatsoever about the Python port, but
*appearing* to, because both hang off one contract. Proof is about a
specific artifact in a specific semantics; it does not travel.

So: an abstract contract MUST NOT inherit the standing of its
best-evidenced implementation. Under R7 each implementation's standing
computes separately, and the honest rendering of one contract shows a
machine-checked Rust implementation, a symbolically-tested Python one,
and a C port with only tests — visibly different strengths under one
interface. Without this rule, one proven implementation launders trust
onto every untested sibling, which is the exact opposite of what the
architecture is for.

## Open questions — need a decision before any of this becomes spec text

1. **How does a test get matched to the code CID(s) it reproduces?**
   Three real options, different cost and different honesty:
   - **Curated**: a test declares, by name or attribute, which CID(s) it
     covers. Cheapest to build, relies on ongoing author discipline, and
     goes stale exactly the way the old locator-string citations did if
     nothing enforces it.
   - **Coverage-based**: instrument test runs (`cargo llvm-cov` or
     similar), map covered byte ranges back to code CIDs whose span
     overlaps. Automatic and honest, but a real new build dependency and
     a slower CI step.
   - **Call-graph heuristic**: reuse `calls_resolve.py`'s existing
     `calls`/`called_by` — if a tested function calls `accepts_verdict`,
     treat the run as partial evidence for it too. Free (already built),
     but coarser: calling something doesn't guarantee a given run
     actually exercised the path that matters.
2. **Authorship as a bare `sig`, or a new lightweight annotation kind?**
   A detached signature alone proves "this key signed this CID for this
   purpose" but carries no structured metadata (commit hash, timestamp
   beyond `created`). A new annotation kind could carry more, at the cost
   of being new surface. Leaning toward the bare `sig` first, since it's
   already fully general and adding fields later doesn't break it.
3. **Which annotation kinds actually get the widened target?** The table
   above proposes `reproduction`, `assessment`, and `verdict`. Does
   `critique` belong here too (a reviewer flagging a specific defect in
   the code itself, not in a document)? Does `triage` (Section 7.4)?
4. **Backfill scope.** Once this is real, do any of the 918 existing code
   cards get retroactive authorship/reproduction, or does this apply only
   to code touched from here forward? If backfill is wanted, at what
   pace — mirrors the citation migration's "blind bulk migration... in
   this development phase" precedent, or something more selective?
5. **Does a `translates` edge from an existing prose publet to its code
   CID replace the §7.5 citation, or sit alongside it?** They're not
   redundant — the citation says "here is the exact evidence"; the
   relation says "this claim is a rendering of that evidence, at this
   fidelity" — but authoring both for all existing procedural publets is
   real, additional work worth scoping deliberately.
6. **What marks a test as "the declared contract" rather than incidental
   coverage?** A naming convention, a doc-comment tag, an attribute —
   `pub-code-tree`/`queue.py` need one concrete, parseable answer, not
   three competing ones.
7. **Does a mismatched independent re-run of a `proof-checked` annotation
   retract it outright, or demote it to `undetermined` pending
   investigation?** An empirical `inconsistent` reproduction has R9's
   existing weighing rules to fall back on; a formal claim's decisiveness
   under §11.4 has no stated behavior for what happens when a supposedly
   deterministic re-run disagrees.
8. **Does `config` (proposed above) need its own canonical, comparable
   encoding**, or is a free-form blob CID good enough for a human to read
   but too opaque for tooling to ever detect "these two proof-checked
   annotations used incompatible bounds" automatically?
9. **May an annotation target a `rel` object?** The whole
   evidence-accrues-to-the-edge proposal rests on it. §7 says
   `target: <CID>` with no stated restriction and R6 makes annotation the
   general mechanism for saying anything about anything, but no existing
   kind does this, and the loader has never been asked to.
10. **Does it matter who authored an `implements` edge?** R6 lets anyone
    assert one. An edge authored by the implementation's own author is a
    self-claim; one authored by the contract's owner is closer to
    acceptance; one authored by a third party is a proposal. `supersedes`
    already distinguishes authoritative from proposed by checking whether
    the signer also signed the target — the same test is available here
    and currently unused.
11. **What form does the abstract contract actually take?** Prose plus
    algebraic laws is language-neutral but needs a human-mediated
    refinement proof per ecosystem. An executable reference in a pure
    subset can be run as an oracle — far more useful — but picks a
    language for the reference, which is the thing this structure was
    trying to avoid. HACL* and Fiat-Crypto both chose the oracle.
12. **What happens to `implements` edges when the abstract contract is
    superseded?** A revised algorithm is a new publet in the contract's
    lineage (§6.1), and every existing edge still points at the old head.
    Silently re-pointing them would be a claim nobody made; leaving them
    is honest but means implementations quietly drift to implementing a
    superseded contract with nothing surfacing it.

## Sequencing, if this goes forward

1. Settle the open questions above.
2. Spec: widen §6.2's carve-out language to name `translates`,
   `reproduction`, `assessment`, `verdict`, `proof-checked` explicitly (or
   whichever subset question 3 settles on) — same shape of change as §7.5
   was for `usage`.
3. Build the git-commit-signing hook for authorship (question 2's
   answer), against a small number of real commits before trusting it.
4. Build whichever test-to-code mapping question 1 settles on, starting
   with one real crate rather than the whole corpus, and the declared-
   contract marker (question 6) alongside it.
5. Wire CI to compose (and, per the earlier signing discussion, either
   sign directly with an author-provisioned key or stage for local
   signing) `reproduction` annotations from real test runs.
6. Only then: decide on question 4's backfill, informed by what steps
   2–5 actually cost to run once, for real, on this codebase.
7. `proof-checked` is a separate, later track: pick one real candidate
   (the CBOR canonical codec is the obvious one), write one Kani harness
   or Prusti contract for it, and compose one real `proof-checked`
   annotation before generalizing anything about how it should work.
8. The abstract-contract structure is a third track, and the cheapest
   place to test it is somewhere this corpus already has two
   implementations of one thing. Author one `formal` publet stating the
   contract, one `implements` edge from existing code, and one annotation
   on that edge — then look at whether the edge-as-evidence-target idea
   (question 9) survives contact with the loader before building on it.
