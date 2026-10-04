---
title: "The Publet Protocol (PUB)"
subtitle: "draft-schryer-publet-protocol-01"
description: A protocol that makes the unit of publication equal to the unit of assertion, so that a single claim can be cited, checked, and contested on its own, with trust computed from roots each reader chooses rather than decreed globally.
keywords:
  - decentralized knowledge graph
  - content addressing
  - trust propagation
  - reproducibility
  - protocol specification
---

**Internet-Draft** — `draft-schryer-publet-protocol-01`

**Intended status:** Experimental

**Expires:** 2027-03-12

+++ { "part": "abstract" }
The structure of human knowledge is ad hoc and loosely defined, which makes
it prohibitively time-consuming for any one person to link testable,
reproducible facts together into a coherent system explaining the
complexities of the modern world. Attaining complete understanding would
take more than a lifetime of dedicated study, and the option of dedicating
oneself to nothing but study is not available to most people. This places
us in a precarious predicament, where each and every one of us is forced to
trust the knowledge, interpretation, and skill of others to establish what
is true.

We are witnessing a process in the modern world where the hijacking of
truth threatens to unravel the hard-earned, rules-based global order that
vast numbers of people willingly fought and died to establish. It is our
duty to them to treat the truth with respect.

What stands in the way is that a statement cannot currently travel with
its conditions attached. Lifted out of the work that published it, a
statement carries neither the kind of claim it is, nor the scope its
author asserted it under, nor the definitions it presupposes, nor the
evidence it rests on. Every reader who wants to check it must reconstruct
all of that, and none of that labour can be handed to the next reader.
Verification does not accumulate.

The Publet Protocol (PUB) attaches those conditions to the assertion itself.
Its primitive is the **claim**: one immutable, cryptographically signed
assertion that declares what kind of claim it is, the conditions under which
its author asserts it, and the definitions it presupposes. Claims are never
edited. Revision, translation, endorsement, dispute, reproduction,
retraction, and classification are each separate signed objects referring to
a claim rather than altering it, so what an author said stands unchanged
while everything said about it accumulates around it.

PUB defines no network-wide notion of truth. It defines objects whose
integrity and authorship any recipient can verify offline, and a
deterministic evaluation function computing the standing of an assertion
relative to trust roots the reader chooses. Formal claims are settled by
proof and empirical claims by independent reproduction; the evaluation
layer governs only what neither has settled. The aims are stated in full in
Section 1, the protocol as sixteen operating rules in Section 2, and every
term of art in the glossary, Appendix C.
+++

## Status of This Memo

This document is an Internet-Draft submitted in the style of, but not
through, the Internet Engineering Task Force. It is offered for discussion
and comment. It is inappropriate to use this document as reference material
or to cite it other than as "work in progress."

This Internet-Draft will expire on 12 March 2027.

## Copyright Notice

Copyright (c) 2026 the persons identified as the document authors. This
document is released under the terms of the Creative Commons Attribution
4.0 International License.

---

## Table of Contents

1. Introduction
2. Operating Rules
3. Applying the Rules
4. Object Format
5. Prose Claims
6. Relational Claims
7. Annotation Claims
8. Documents
9. Naming and Curation
10. Identity
11. Evaluation
12. Settlement
13. Nodes
14. Domains and Transport
15. Versioning and Extensibility
16. Security Considerations
17. Privacy Considerations
18. IANA Considerations
19. References
- Appendix A. Worked Example
- Appendix B. Open Problems
- Appendix C. Glossary
- Authors' Addresses

---

## 1. Introduction

This document lays out a framework with the following aims.

1. **Let verification accumulate.** Checking an assertion should leave
   behind something the next reader can build on rather than repeat.
   Nothing else in this list matters if the labour of checking cannot be
   handed on, because that is what makes a lifetime insufficient.
2. **Make every assertion travel with its conditions attached.** A claim
   states the class of claim it is, the scope under which its author
   asserts it, and the definitions it presupposes, so that it can be
   checked away from the work that published it and cannot quietly become
   a different claim as the context around it changes.
3. **Make the unit of publication the unit of assertion.** An assertion is
   published as its own object, so that agreeing with one part of a work
   and rejecting another requires duplicating neither.
4. **Keep what settles a question apart from what people think of it.**
   Proof settles formal claims and independent reproduction settles
   empirical ones. No quantity of agreement substitutes for either, and
   the opinion layer governs only what neither has settled.
5. **Locate disagreement precisely.** Where two parties argue past each
   other because they presuppose different definitions of a word, that
   divergence is computed and shown before the dispute is presented as a
   dispute about fact.
6. **Leave the choice of whom to trust with the reader.** Standing is
   computed against trust roots a reader declares, never decreed for the
   network. A key no trust path reaches carries no weight anywhere, at any
   scale, which is what makes unlimited identities useless rather than
   merely discouraged.
7. **Let a reader check without being watched.** Reading and evaluation
   run against a local replica, so that examining a claim discloses
   nothing to anyone.
8. **Keep authorship human and bounded by a lifetime.** Institutions may
   fund, host, and operate; they may not be the author of record.

What the framework does not attempt matters as much as what it does. It
does not determine truth, establish who anyone really is, replace peer
review, or guarantee that anything remains retrievable.

### 1.1 Scope

PUB is a format and evaluation model for assertions that can in principle
be settled by observation, measurement, or formal derivation. It addresses
four failures common to systems that publish knowledge at scale — semantic
drift [PUB-DRIFT], namespace capture [PUB-NSCAP], split-brain forking
[PUB-FORK], and translation desynchronization [PUB-TDESYNC] — by making
the unit of publication equal to the unit of assertion and then never
changing it. Each is defined, and evidenced against a documented case, as
a claim in the Publet Protocol's own corpus — the protocol applied to
itself (Section 19.2).

Normative and political claims are handled by decomposition rather than
exclusion: a policy argument is expressed as empirical claims plus
explicitly stated normative premises, each a separate object. The protocol
locates such disagreements precisely. It does not settle them.

PUB does not determine truth, establish real-world identity, replace peer
review, or guarantee that any object remains retrievable.

**What conformance requires.** Sections 4 through 8 and Section 11 are the
protocol: objects, the three claim grammars, documents, and evaluation. An
implementation that reads, verifies, and evaluates claims needs those and
nothing else. Sections 9.3, 12, and 14.7 say so of themselves — anchors
are "not load-bearing", settlement "MAY [be omitted] entirely", and
implementation transparency "is a norm and is not verifiable". Sections 13
and 14 govern serving and distributing a corpus, which a reader holding
one does not need. Stating this here saves working it out from the inside,
and marks the shortest path from a claim to a reader who can check it.

### 1.2 Assumed Environment

This specification assumes a functioning legal regime with the properties
below. Coercion and manipulation are enforcement problems, not protocol
problems, and this document defines no cryptographic countermeasures for
them.

| | Assumed property |
|---|---|
| **L1** | Coercing a key holder to sign, disclose a private key, or surrender an identity secret is prohibited and the prohibition is enforced. |
| **L2** | Compelling disclosure of a source, a delegation, or the holder of a pseudonymous key is prohibited. |
| **L3** | Signing a knowingly false attestation — of identity, affiliation, reproduction, or review — is actionable. |
| **L4** | Transfer, rental, or sale of a personhood credential is void and unenforceable. |
| **L5** | Lawful removal of published material follows a defined, bounded, appealable, and publicly recorded process. |
| **L6** | Publishing research, measurement, or analysis is not in itself unlawful, and no authority may revoke a person's credentials on the basis of what they publish. |

These properties are load-bearing. Where a deployment operates outside a
regime providing them, the guarantees in Sections 10 through 13 degrade to
the strength of the local regime, and operators should assess that before
relying on them. This document does not restate the degradation case by
case.

### 1.3 Requirements Language

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
"SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and
"OPTIONAL" in this document are to be interpreted as described in BCP 14
[RFC2119] [RFC8174] when, and only when, they appear in all capitals.

Statements labelled **NORM** are conventions this protocol encourages but
cannot verify. They are not conformance requirements.

### 1.4 Terminology

The terms needed to read Sections 2 and 3 are below. **Appendix C is the
full glossary**, covering every term of art in this document; where the
two overlap, they are the same definitions, and the glossary is where new
ones are added.

**Claim** — an immutable signed object asserting a proposition. Claims come
in three grammars, differing only in how the proposition is expressed:
`claim.prose` carries it as text, `claim.annotation` as a predicate over
one object, `claim.relation` as a predicate over two. All three are
assertions by an author, not structure; the graph is what an indexer
derives from them, never a primitive.

**Prose claim** — a claim carrying one assertion as text, with its claim
class and validity scope (Section 5).

**Relational claim** — a claim asserting a typed relationship between two
objects (Section 6). Ordinarily shortened to *relation*.

**Annotation claim** — a claim attaching a judgement to a target
(Section 7). Ordinarily shortened to *annotation*.

**Document** — a work citing sources and ordering references to them,
containing no assertions of its own (Section 8). A document is the citing
side of the relationship a publet is the cited side of.

**Object** — any of the above. Every object is content-addressed and
signed.

**Publet** — not an object type. A publet is a **role** an object plays:
a published rendering of one or more claims, small and single-topic, that
carries a short citation tag (Section 9.1) and is meant to be a stable
thing other work cites, the way a cited paper stands to the paper citing
it. A publet is ordinarily a `doc` (Section 8) bundling one topic's claims
under a heading and a gloss — occasionally a single claim, tagged
directly, with no bundling document at all. Either way, nothing about its
own shape marks it a publet; being findable by a short tag does.

The three layers stay conceptually distinct even though two of them share
one object type: a **claim** is what is asserted, a **publet** is a
tagged, citable rendering of one or more claims, and a **document**
(Section 8) more generally is a work citing sources — publets among
them — while asserting nothing of its own. A large `doc` arguing across
many claims and a small `doc` tagged as a publet are the same object type
serving two different roles, exactly as `formal` and `empirical` are two
roles one `claim.prose` shape serves.

**CID** — the multihash of an object's canonical serialization
(Section 4.2).

**Viewpoint** — a trust policy, a set of trust roots, and a snapshot
boundary. Evaluating under a viewpoint yields weights and standings
(Section 11).

**Standing** — the result of evaluating an assertion under a viewpoint. Not
a truth value (Section 11.4).

**Lineage** — the set of objects connected by `supersedes` edges from a
**genesis** object. A lineage is named by its genesis CID, is a directed
acyclic graph that may branch, and is never itself an object (R15).

**Head** — the object a viewpoint selects as a lineage's current best
state. A branching lineage may have several.

---

## 2. Operating Rules

The protocol is these sixteen rules applied uniformly to every object
type. Later sections specify formats and algorithms; they introduce no new
principles. Where a section appears to conflict with a rule, the rule
governs.

| | Rule |
|---|---|
| **R1** | **Immutability.** An object, once published, is never modified. Revision is a new object plus a `supersedes` relation. |
| **R2** | **Content addressing.** Every object is named by the multihash of its canonical serialization. A reference is a CID; retrieved bytes are verified against it before use. |
| **R3** | **Detached attestation.** Anything a party asserts *about* an object is a separate object signed by that party. No object contains another party's signature or judgement. |
| **R4** | **Declared scope.** Every assertion states the conditions under which its author asserts it. |
| **R5** | **Declared class.** Every assertion states its claim class. The class determines what evidence is required and which judgements are admissible. |
| **R6** | **Open annotation.** Anyone may publish a relation or annotation targeting any object. No permission is required and none can be withheld. |
| **R7** | **Viewpoint relativity.** Standing is computed relative to a declared trust policy and snapshot. There is no network-wide standing, and no key carries weight that no trust path reaches. |
| **R8** | **Reproducible evaluation.** Evaluation is deterministic: the same policy over the same snapshot yields bit-identical results in every conformant implementation. |
| **R9** | **Evidence dominance.** Proof settles formal claims; independent reproduction settles empirical claims. No quantity of endorsement substitutes for either. |
| **R10** | **Novelty requirement.** A dispute contributing no grounds beyond those already covered by a resolution carries no weight. |
| **R11** | **Human signatory.** Authorship, counted reproduction, and stewardship require keys declaring a human principal. Institutions participate through affiliation. |
| **R12** | **Unconditional service.** Within a node's declared set, service is content-blind and unconditional. Ceasing to serve requires a published tombstone. |
| **R13** | **No settlement authority.** Funds settle against predicates declared in advance over reproducible evaluations. Payment never gates access and never affects standing. |
| **R14** | **Machine proposes, key signs.** An automated system may generate any object as a candidate. It enters the graph only when a key signs it, and that key bears the consequences. |
| **R15** | **Object and lineage.** Assertion, evidence, and signature operate on objects. Curation, stewardship, tracking citation, and definitional drift operate on lineages. A lineage is computed from `supersedes` edges and named by its genesis CID; it is never a registered object. |
| **R16** | **Local-first retrieval.** Reviewed knowledge and each proposal domain are bounded in size, dependency-closed, and retrievable in full. Reading and evaluation run against a local replica; another party's node is queried only to synchronize and to propose. |


Three pairings are worth reading together. R3 and R1 jointly mean that an
authored object is finished at publication and everything else accretes
around it. R7 and R9 jointly mean that judgement is subjective but evidence
is not, and that the subjective layer is subordinate to the objective one.
R1 and R15 jointly mean that nothing is ever edited and nothing is ever
stranded: what an author said stands unaltered, while the line of work it
belongs to continues without them. R16 and R7 jointly mean that the
subjectivity of evaluation costs nothing in privacy, because the viewpoint
is applied where the reader is rather than where the data is.

---

## 3. Applying the Rules

Each failure below is resolved by a composition of rules, not by a
mechanism specific to it. This table is the design; the rest of the
document is its encoding.

| Failure | Rules | Resolution |
|---|---|---|
| A cited source changes meaning after citation | R1, R2 | Citations name a CID. The cited bytes cannot change. |
| Pinned citations become an archive of stale claims | R1, R4 | Scope is inside the assertion, so a claim does not go stale by being old; `supersedes` lineage exposes successors without altering what was cited. |
| Names are squatted, auctioned, or captured | R6, R7 | No registry exists. Names are `definitional` claims and `classifies` annotations; resolution is per viewpoint. |
| Disagreement forces whole-work duplication | R1, R3, R6 | Disagreement is a competing claim plus a `disputes` relation. The work is untouched. |
| Forks diverge silently | R6 | Forking is permitted; `derived-from` is required; composition defects are stated as `critique` annotations. |
| Translations desynchronize from sources | R1, R2 | A translation binds to a CID and cannot desynchronize. Staleness is computed from lineage, requiring no action by the translator. |
| An adversary creates unlimited identities | R7 | An unvouched key carries zero weight under every viewpoint, at every scale. |
| An adversary blocks any claim by spamming disputes | R7, R10 | Untrusted disputes carry zero weight; weighted disputes without new grounds carry zero weight. |
| Popular error outvotes correct measurement | R9 | Endorsement cannot reach `accepted` below the replication floor, and inconsistent reproduction outweighs endorsement of any magnitude. |
| Evidence only its owner may examine is presented as established | R9 | `restricted` reproducibility caps standing at `unreplicated`. |
| The same idea proliferates as near-duplicates | R6 | `equivalent` relations form equivalence classes; support and dispute aggregate across a class, counting each signer once. |
| Parties argue past each other over word meanings | R5 | Mandatory `depends` makes divergent definitional premises computable and surfaced before a dispute is presented as factual. |
| Reviewers are paid to reach a conclusion | R13 | The review share pays for filed work irrespective of finding; the system share returns on resolution in either direction. |
| A serving node learns what a reader is considering | R16 | A reader holding a domain replica issues no read request; nothing exists to log. |
| An evaluator learns a reader's trust configuration | R16 | Evaluation runs locally; the policy never leaves the machine. |
| An index silently omits refutations | R16, R2 | Reverse edges within a replica are computed by the reader and are complete; a divergent snapshot root is detectable by comparison. |
| A node suppresses inconvenient material | R12, R2 | Withholding within a declared set is a published violation; content addressing lets any other mirror satisfy the request. |
| An institution accumulates permanent authority | R11 | Only human principals author and steward, so every reputation is bounded by a lifetime. |
| A citation should track the current best statement rather than a fixed one | R15 | A document item binds to a lineage rather than an object, recording both the genesis and the head the author read. |
| Two parties depend on different generations of one definition | R15 | Same-lineage definitional difference is reported as staleness, not divergence, with a different remedy. |
| A line of work outlives its originator | R15, R11 | Curation, stewardship, and delegation attach to lineages, which have no lifespan, rather than to authors, who do. |
| An author dies or loses their key | R1, R11 | Nothing depends on an author remaining available. Claims stand; others continue the lineage; stewardship is a shortcut, not a dependency. |
| Automated systems flood the graph | R7, R14 | Unsigned output does not exist; signed output carries a key's consequences; zero-weight keys are inert under every viewpoint. |
| A published figure cannot be traced to where it was read | R1, R2, R4 | Values travel in a claim's `data` with `source` evidence naming the file at a commit, the query, the export, or the claim they came from; a source claim is also in `depends`, so the chain back to the inputs is a closure (Sections 5.5, 5.8). |
| The same values must be shown differently in different places | R1, R3 | How to show data is a `view` on the citing document's item, not part of the claim; renderings are rebuilt wherever they are shown and are never objects (Section 8). |

---

## 4. Object Format

### 4.1 Canonical Serialization

Objects are serialized as deterministic CBOR [RFC8949] under this profile:

- integers in shortest form;
- map keys are text strings, unique, sorted by UTF-8 byte sequence;
- no indefinite-length items;
- **no floating point anywhere.** Non-integer quantities are decimal
  strings with an explicit unit, or rational pairs;
- text is UTF-8 in Normalization Form C.

An implementation MUST reject non-canonical bytes. It MUST NOT
re-canonicalize and accept.

### 4.2 Content Identifiers

```
  pub:<hash-algorithm-id>:<base32-lower-no-pad(digest)>
```

`sha2-256` is REQUIRED; `sha3-256` and `blake3` are RECOMMENDED. A
reference MAY carry retrieval hints; an implementation MUST verify
retrieved bytes against the CID and ignore conflicting hints.

### 4.3 Common Header

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `pub` | string | MUST | Protocol version. |
| `type` | string | MUST | Object type (Section 18.2). |
| `created` | string | MUST | RFC 3339 UTC, second precision. |
| `author` | CID | MUST | CID of the signing Key Object. |
| `body` | map | MUST | Type-specific payload. |
| `prev` | CID | MAY | Author's previous object, forming a per-key chain. |
| `basis` | CID | MAY | The state the author was looking at (Section 4.6). |
| `ext` | map | MAY | Extension fields (Section 15). |

`created` is an unverified claim. Where correctness depends on time, use a
timestamp attestation (Section 10.5).

### 4.4 Signatures

Signatures are detached objects (R3):

```
  { pub, type: "sig", created, author,
    body: { target: <CID>, alg: <signature-algorithm-id>,
            value: <bytes>, purpose: <string> } }
```

The signed message is `"pub/v1/sig" || 0x00 || purpose || 0x00 ||
canonical-bytes-of-target`. An implementation MUST reject a signature whose
`purpose` does not match its evaluation context.

Any number of signatures from any number of keys MAY accrue to an object at
any time without changing its CID.

`ed25519` is REQUIRED. `ml-dsa-65` [FIPS204] is RECOMMENDED for new
deployments.

### 4.5 Size

Implementations MUST accept objects up to 64 KiB and MAY reject larger.
Images, datasets, and media are referenced by CID as opaque **blobs** and
are not objects (Section 4.7).

---

### 4.6 Stated Basis

`basis` names whatever the author was looking at when they composed the
object: a lineage head, a generation record, a snapshot, a domain state.
It is one field because it is one idea — **state what you were looking at,
so that what you concluded can be checked against it** — and the same idea
otherwise recurs under a different name in every section that needs it.

`basis` is advisory and never gating. Nothing is overwritten (R1), so a
stale basis cannot clobber anything and is not grounds for rejecting
anything; it exists to disclose. What it buys is that the interval between
what an author read and what was true when their work landed is
permanently recorded and auditable by anyone.

Where a type has a field naming the same thing more specifically, that
field governs and `basis` is redundant: a document item's `at` (Section 8)
names the head of one cited lineage rather than the state of the whole
work, and an `eval` names its `snapshot` because reproducing the
evaluation requires it (R8).

### 4.7 Blobs

A **blob** is a byte string named by the CID of those exact bytes
(Section 4.2), with no header, no canonical form beyond the bytes
themselves, and no signature. It is the one stored thing that is not an
object: it asserts nothing and has no author. Whatever is asserted about
it is asserted by the object that cites it, which is signed, and which
states what the bytes are (`media`, a media type [RFC6838]) and how many
there are (`size`, in bytes).

A blob is verified exactly as an object is (R2): an implementation MUST
verify a blob's bytes against its CID before use, and MUST reject a blob
whose length differs from the `size` stated by the object citing it. A
cited blob that is not held is unavailable, not absent from the claim:
an implementation MUST NOT present a claim as verified while a blob its
`data` cites is unavailable, for the reason Section 5.4 gives for
dependencies.

Blobs carry no size ceiling of their own, are never members of a domain
(Section 14.1), and are fetched separately. Because a blob cannot state its
own meaning, one blob MAY be cited by any number of objects, each stating
something different about the same bytes.

---

## 5. Prose Claims

A prose claim carries its proposition as text: the grammar of Section 6
applies a predicate to two objects, Section 7 applies one to a single
object, and this one leaves the proposition unanalyzed for a reader to
interpret. All three are claims (Section 1.4), and an author asserting one
is making the same kind of speech act in each case.

### 5.1 Body

```
  { pub: "1", type: "claim.prose", created, author,
    body: {
      class:           <claim-class>,       // R5, REQUIRED
      lang:            <BCP47 tag>,         // REQUIRED
      content:         <string>,            // REQUIRED, <= 4096 bytes
      scope:           <scope>,             // R4, REQUIRED
      depends:         [ <CID>, ... ],
      evidence:        [ <evidence>, ... ], // per class
      reproducibility: <repro>,             // per class
      medium:          <medium>,            // for non-text content
      data:            <data>               // OPTIONAL, Section 5.8
    } }
```

`content` is one assertion, in one language. The size ceiling is structural
pressure toward atomicity. Where a claim is about a set of values, the
values are carried in `data` and `content` remains the one sentence stating
what they are (Section 5.8).

### 5.2 Claim Classes

Classes apply to **every** claim, whatever its grammar (R5). A prose claim
declares its class in `class`, because the same sentence could be asserted
as any of several. A relation or an annotation takes the class its `kind`
carries, because the kind already fixes what is being asserted: the tables
in Sections 6 and 7 give the mapping, and declaring it again per object
would only create something to disagree with.

| Class | Content is | Settled by | Verdicts |
|---|---|---|---|
| `formal` | a mathematical or logical statement | proof checking | permitted; `settled` is decisive |
| `empirical` | a claim about the observable world | independent reproduction | permitted |
| `attributive` | "*X* said *Y*" | provenance of the attribution | on provenance only |
| `definitional` | a stipulated meaning | usage evidence | not permitted; use `usage` |
| `normative` | an ought-claim | not truth-apt | not permitted |
| `expressive` | poetry, fiction, testimony | not truth-apt | not permitted |
| `archival` | a primary-source record | provenance and custody | on provenance only |
| `procedural` | an instruction or method | reproduction reports | permitted |
| `performative` | an act done by asserting it | felicity conditions, not evidence | not permitted |

Implementations MUST reject a verdict annotation targeting a
`definitional`, `normative`, `expressive`, or `performative` claim. For
`attributive` and `archival`, a verdict's `aspect` MUST be `provenance`. A
claim about the *content* of a quoted or archived statement is made by
publishing an `empirical` claim asserting it, and contesting that.

A **performative** claim is not true or false, because asserting it is
what does the thing: a retraction does not describe a withdrawal, it
withdraws. What such a claim has instead of evidence is **felicity
conditions** — who is entitled to perform the act, and what must hold for
it to take effect. Those conditions are stated per kind rather than per
class, because they differ: only the author of an assertion may withdraw
it, while anyone at all may dispute it. This class is why Sections 6 and 7
need no separate rule about which of their kinds may be judged.

`formal` and `empirical` claims made in prose carry their own evidence
in-body (Section 5.5). The same claim made as a relation or an annotation
cannot — those grammars have no `evidence` field — so its evidence accrues
as annotations against its CID instead, which R3 and R6 already permit and
R9 already weighs. The requirement in Section 5.5 is a requirement on
prose claims specifically, not a general one that relations fail.

### 5.3 Scope

```
  scope: { conditions: [ <CID>, ... ],
           domain:     <string>,
           temporal:   { from, to },
           precision:  <string> }
```

Where an author asserts something unconditionally, `domain` is
`"unconditional"` and `conditions` is empty. Authors SHOULD use it rarely.

### 5.4 Dependencies

`depends` lists claims whose meaning is presupposed, typically
`definitional` claims fixing terms. Dependency edges MUST be acyclic. An
implementation MUST NOT present a claim as verified while any member of
its dependency closure is unavailable.

### 5.5 Evidence

```
  { kind: "blob" | "claim" | "external",
    ref:  <CID or URI>,
    role: "measurement" | "derivation" | "citation"
        | "replication" | "method" | "source",
    note:     <string>,
    revision: <string>,   // role "source": the version read
    locator:  <string>,   // role "source": where within `ref`
    query:    <string> }  // role "source": the query that selected it
```

For `empirical` claims, `evidence` MUST be non-empty and MUST include
exactly one entry with `role: "method"` referencing a `procedural` claim
describing how the observation may be repeated.

`external` references lie outside the protocol's integrity guarantees and
MUST be displayed as unverified.

**Source evidence.** An entry with `role: "source"` states where values
carried in `data` (Section 5.8) were read from. It records provenance, not
support: a source is not a measurement, does not count toward any
replication floor, and no quantity of sources substitutes for the evidence
R9 requires. The shape follows what was read:

- a file in a version-controlled tree: `kind: "external"`, `ref` the
  repository, `revision` the commit read, `locator` the path within it;
- a table in a database or warehouse: `kind: "external"`, `ref` the table,
  `query` the query that selected the rows, and `revision` the snapshot or
  time read where the store offers one;
- an exported file: `kind: "blob"`, `ref` the CID of the export as read,
  which makes the source itself verifiable (Section 4.7);
- another claim: `kind: "claim"`, `ref` its CID.

A `source` entry of kind `claim` MUST also appear in `depends`, and an
implementation MUST reject a claim in which it does not. A value derived
from another claim's values presupposes that claim in exactly the sense
Section 5.4 describes, and listing it there is what makes the chain from a
figure back to its inputs walkable by the same closure that walks
definitions. A claim carrying `data` SHOULD carry at least one `source`
entry, and implementations SHOULD warn at authoring time where it does not.
Source entries are ordinarily written by the program that gathered the
values rather than typed by an author.

Evidence and `depends` entries naming a claim are covered by the citing
claim's CID (R1, R2), so they necessarily reference objects that already
existed. Circular support — a set of claims whose evidence paths lead only
back to each other — therefore cannot be constructed in-body without a hash
preimage. Third-party relations (Section 6) carry no such guarantee.

### 5.6 Reproducibility

```
  reproducibility: { class: "open" | "restricted"
                          | "unique-event" | "destructive",
                     controller:   <CID or string>,  // if restricted
                     requirements: <string> }
```

- **`open`** — any party with the stated resources can attempt it.
- **`restricted`** — attempting it requires access a named party controls.
- **`unique-event`** — unrepeatable in principle.
- **`destructive`** — repeatable only on a different sample.

`restricted` claims cannot reach `accepted` (Section 11.4). For
`unique-event` claims, what is verifiable is the record rather than the
observation: such claims SHOULD reference an `archival` claim and are
evaluated on provenance, with uncoordinated contemporaneous observation
treated as the nearest analogue of replication.

### 5.7 Atomicity

A claim whose content is an assertion SHOULD express one assertion, such
that a reader could agree with it and disagree with the next sentence of
the work it came from, and such that negating it yields exactly one
coherent counter-claim.

Implementations SHOULD warn at authoring time where: the content holds more
than one independent clause; a coordinating conjunction joins two
stand-alone assertions; more than one quantitative claim appears;
unresolved anaphora point outside the claim; or a contested term is absent
from `depends`. A failing claim is flagged, not invalid.

These tests divide into two, and the division is not stylistic.

**Atomicity** — independent clauses, coordinating conjunctions, and
quantitative claims — applies only to the classes whose content *is* the
assertion: `formal`, `empirical`, `definitional`, and `normative`. It MUST
NOT be applied to `procedural`, `attributive`, `archival`, or `expressive`.
Their content instructs, quotes, records, or expresses, and the test above
has no reading for any of them: a procedure does not negate, and negating
an attribution denies that *X* said it, which says nothing about how many
sentences *Y* contains.

For `procedural` claims the exemption holds at every level of
decomposition. A method may be a single claim, or it may name
sub-procedures in `depends`, each of which may name further ones until base
steps are reached; `depends` is acyclic, so the decomposition terminates.
The dependency closure is the whole method, and Section 5.5's requirement
that an `empirical` claim name **exactly one** `procedural` claim is a
requirement on the *citation* — one identifier names the method
unambiguously — not a requirement that the method be a leaf.

Atomicity does not apply at any level of that tree, because a step is not
an assertion however small it is: negating "warm the sample" yields no
counter-claim. Where a method is too long to express as claims at all, it
references a `blob`.

**Self-containment** — unresolved anaphora and undeclared contested terms —
applies to every class without exception. A method directing the reader to
"then add it" with no antecedent is a defective method, and a quotation cut
so that its pronouns dangle is a badly cut quotation.

A **decomposition profile** is a document stating a field's conventions for
where to cut. Profiles carry no authority; a claim MAY name the profile it
was authored under in `ext`.

Implementations MUST NOT deduplicate by content similarity. Whether two
phrasings assert the same thing is a claim, and claims are made by signing
them (R6, R14).

### 5.8 Data

A claim about a set of values -- a budget by line item, a month of
transactions, a calibration series -- states what the values are in
`content` and carries the values themselves in `data`, in one of two
shapes:

```
  data: { columns: [ { name: <string>, unit: <string> }, ... ],
          rows:    [ [ <cell>, ... ], ... ] }          // a table
      | { media: <media type>, ref: <CID of blob>,
          size:  <unsigned integer> }                  // a file
```

A table's cells are text, integers, or null. Non-integer quantities are
decimal strings (Section 4.1), and the unit belongs to the column, so that
a value is never separated from what it counts. `unit` MAY be omitted for a
column whose values have none, such as names. Column names MUST be
non-empty and unique within a table, every row MUST carry exactly as many
cells as there are columns, and an implementation MUST reject a claim
whose `data` violates either. A file is a blob (Section 4.7), and `media`
and `size` are the citing claim's statement of what it is.

`data` carries values and nothing about how to show them: no ordering
beyond the rows' own, no number formatting, no grouping, no totals row.
Those are presentation, they belong to the document that shows the data
(Section 8), and two documents MAY show one claim's data differently. A
total displayed beside the rows is computed by whatever renders them, and
is not part of what was asserted.

The claim asserts its `data` as a whole, under its own `class` and
`scope`: a budget is `normative`, because planned figures are decisions; a
month of bank transactions is `archival`; a set of measurements is
`empirical` and needs its method as any other empirical claim does. The
atomicity tests of Section 5.7 apply to `content`, not to the rows -- a
table of forty figures is one assertion about what the forty figures are,
and negating it yields one counter-claim: that they are not. Contesting a
single value is contesting the claim, with the dispute's `aspect` naming
the row and column.

**NORM.** Why a set of values was assembled as it was -- which items were
included, why they are grouped by phase, which exchange rate was applied --
is itself a claim, and is published as a `normative` or `procedural` claim
that the data claim names in `depends`, not written into `content` or
`scope`. The reasoning can then be disputed or revised on its own lineage
without touching the values, and the values can be revised without
reopening the reasoning.

A new version of a data claim is a new object plus `supersedes`, as with
any claim (R1). An author SHOULD publish one only when the values or their
sources change: re-rendering the same values is not a revision, and a
lineage that grows on every build obscures the revisions that matter.

---

## 6. Relational Claims

```
  { pub: "1", type: "claim.relation", created, author,
    body: { kind: <relation-kind>, from: <CID>, to: <CID>,
            scope: <scope>,               // R4, REQUIRED
            aspect: <string>, note: <string> } }
```

`scope` is required here for the reason it is required on a prose claim
(R4): a relation is an assertion, and an assertion that does not state its
conditions is a different assertion every time it is read. It is
load-bearing rather than ceremonial — an `implements` edge that holds on
one target architecture and not another is the ordinary case, not an edge
case, and without `scope` there is nowhere to say so. Where an author does
mean it unconditionally, `domain` is `"unconditional"` and `conditions` is
empty, exactly as in Section 5.3.

A relation is a claim whose proposition is a predicate drawn from a closed
vocabulary and applied to two objects: `implements(A, B)`,
`supersedes(A, B)`. It is an assertion by its author, not an edge in a
structure — edges are what an indexer derives by filing relations under
their arguments, the same way lineages are computed and never declared
(Section 6.1). Two keys MAY assert contradicting relations over one pair;
both objects are valid, and which a reader sees is viewpoint-relative (R7).

Relations are authored by anyone (R6), not only by the endpoints' authors.

Relations MAY form cycles, and implementations MUST NOT assume otherwise:
`A disputes B` and `B disputes A` is ordinary mutual disagreement,
`equivalent` cycles are the mechanism by which equivalence classes form
(Section 11.6), and `trusts` cycles are handled by damping (Section 11.2).
Kinds marked **acyclic** below are the exception; for these an
implementation MUST reject an edge closing a cycle, since each expresses a
lineage or a presupposition that a cycle would make incoherent.

| Kind | Asserts | Class | Notes |
|---|---|---|---|
| `supersedes` | `from` replaces `to` | `performative` | Felicitous only when signed by a key that signed `to`, or by a key holding an immediate `lineage` delegation from it (Section 10.6); otherwise a proposal, which clients MUST distinguish. Successors MAY branch. Acyclic. Constitutes lineage (Section 6.1). |
| `translates` | `from` translates `to` | `empirical` | Carries `method` (`human`/`machine`/`machine-post-edited`) and `fidelity` (`literal`/`idiomatic`/`adapted`). |
| `implements` | `from` implements the interface `to` | `empirical` | Acyclic. `from` and `to` need not be signed claim objects (Section 6.2). |
| `depends` | `from` presupposes `to` | `definitional` | Acyclic; implementations MUST reject cycle-closing edges. |
| `disputes` | `from` argues against `to` | `performative` | MUST name a claim stating grounds. |
| `supports` | `from` argues for `to` | `performative` | |
| `equivalent` | `from` and `to` express the same claim | `definitional` | Not transitive; classes are computed per viewpoint. |
| `retracts` | the signer withdraws a prior assertion | `performative` | Felicitous only from a key that signed the target. Does not delete; nodes MUST continue serving (R12). Clients MUST display it prominently. |
| `derived-from` | `from` copies or adapts `to` | `attributive` | Required for forks. Acyclic. |
| `delegates` | `from` authorizes `to` to continue | `performative` | See Section 10.6. |

The `Class` column is not a new axis. It is Section 5.2 applied to this
grammar (R5), and it is what determines which judgements are admissible
here, exactly as it does for a prose claim. No rule specific to relations
is needed: a verdict on `retracts` is refused because `performative`
claims are not truth-apt, the same refusal Section 5.2 already makes for
`normative` ones.

Reading the column tells you what kind of thing each relation is. The
`empirical` ones describe how things stand and can be wrong: whether an
implementation satisfies a contract, or a translation renders its source
faithfully, is settled by checking, and the evidence accrues as
annotations against the relation's own CID — because what is evidenced is
the relationship, not either endpoint. The `definitional` ones are settled
by usage rather than by verdict, which is why `equivalent` classes are
computed per viewpoint rather than voted on. `derived-from` is a
provenance claim, so a verdict on one must name the `provenance` aspect.

The `performative` ones do something by being asserted. Their felicity
conditions differ and are stated per kind above: only the key that signed
an object may felicitously retract or supersede it, while anyone at all
may dispute one, provided they name grounds. That difference is why
felicity is stated per kind and not per class.

### 6.1 Lineage (R15)

A lineage is the set of objects reachable by `supersedes` edges from a
genesis object — one with no outbound authoritative `supersedes` — and is
named by that genesis CID. Lineages are computed, never declared, and no
object registers one.

Two lineages are computable over the same edges:

- The **authoritative lineage** uses only edges signed by a key that also
  signed the target: an author's own revision history.
- The **full lineage** additionally includes third-party proposals to
  replace.

Implementations MUST be able to render both and MUST distinguish them.
Where a lineage branches, selection of a head is viewpoint-relative (R7):
the evaluator ranks candidate heads by standing under the reader's policy,
and several heads MAY be returned.

Claims, documents, and definitional claims all form lineages by the same
rule. A definitional lineage is the recorded evolution of a term's meaning,
and Section 9.2 depends on it.

Retraction (`retracts`) applies to an object, never to a lineage. Clients
SHOULD indicate when an ancestor of a displayed object is retracted.

### 6.2 Implementation

An `implements` edge states that one thing realizes a contract another
thing declares — a concrete algorithm satisfying an abstract interface,
where several implementations MAY exist side by side and a consumer of
the interface is agnostic to which one it holds.

Unlike every other relation, `from` and `to` need not each be a signed
claim object. A concrete implementation is commonly a piece of source
code, and the interface it implements a type signature or trait
declaration; both are ordinarily addressed by a content identifier
computed directly over that code (Section 4.2), not by the CID of a
signed envelope wrapping it. An implementation MAY additionally exist as
a full claim object — carrying its own citations, verdicts, and
lineage — but is not required to, and the `implements` edge is equally
valid either way. A CID that names code rather than a signed object MUST
NOT be treated as though a `pub read` of it will succeed; readers
resolve it against the source it names, not against the object store.

`equivalent` is the wrong relation for this: it asserts `from` and `to`
express the same claim, which two independently written implementations
of one interface do not — they satisfy a shared contract while
remaining, in general, different claims about how. `translates` is also
the wrong shape: it grades the fidelity of one rendering of a single
claim, not the conformance of one artifact to another's contract.

Acyclic for the same reason `depends` is: an interface an implementation
implements is presupposed by it, and a cycle would make one thing both
the contract and a satisfier of its own contract, which is incoherent
rather than merely unusual.

---

## 7. Annotation Claims

```
  { pub: "1", type: "claim.annotation", created, author,
    body: { kind: <annotation-kind>, target: <CID>,
            scope: <scope>,               // R4, REQUIRED
            aspect: <string>, value: <kind-specific>, note: <string> } }
```

`scope` is required here for the same reason (R4). A reproduction that was
consistent under stated conditions, an assessment sound only within a
named domain, and a trust edge meant for one subject are all ordinary, and
each needs somewhere to say so. Where the annotation is meant
unconditionally, `domain` is `"unconditional"` and `conditions` is empty.

An annotation is a claim whose proposition is a predicate from a closed
vocabulary applied to one object, carrying a kind-specific payload. It is
the same grammar as Section 6 at arity one, which is why the two share
`kind`, `aspect`, and `note`.

`target` MAY name any object, including another claim of any grammar.
Annotating a relation is the ordinary way to evidence one of the
`empirical` relation kinds (Section 6): what is evidenced is the
relationship the relation asserts, which is a claim in its own right and
not a property of either endpoint. Implementations MUST NOT require that
`target` resolve to a prose claim.

### 7.1 Kinds

The kinds group by what they are *about*, and each takes a class from
Section 5.2 the same way a relation kind does (R5). The class is what
determines whether the annotation may itself be judged: a reproduction
reports something observable and so can be contradicted, while a verdict
performs a judgement and so cannot be affirmed or denied in turn.

**Evidence** — an independent party executed what settles a claim of this
class, and reports what happened. Decisive under R9.

| Kind | Value | Class | Purpose |
|---|---|---|---|
| `settled` | See Section 7.2 | `empirical` | For `formal`, `empirical`, and `procedural` claims: an independent party executed what settles this class, and reports what happened. |
| `well-formed` | structural test results | `empirical` | Section 5.7. Re-runnable over the claim alone. |

**Judgement** — a signer states how they find a claim. Performative: the
saying is the judging, so these carry no truth to affirm or deny.

| Kind | Value | Class | Purpose |
|---|---|---|---|
| `assessment` | `verdict` ∈ {`sound`, `sound-in-scope`, `superseded`, `unsupported`, `refuted`, `undetermined`}, `grounds` | `performative` | A signer's judgement. An author's self-assessment is one annotation among many, not a privileged field (R3). |
| `verdict` | `finding` ∈ {`affirm`, `deny`, `abstain`}, `aspect`, `method`, `effort` | `performative` | Input to evaluation policies. Class-restricted per Section 5.2. |
| `critique` | `defect` ∈ {`omission`, `ordering`, `false-balance`, `decontextualized`, `selection-bias`, `other`}, `omitted` | `performative` | Targets a **document**. Makes composition contestable, naming omitted claims. |
| `triage` | See Section 7.4 | `performative` | Machine-assisted, advisory only. Re-runnable, not reproducible. |
| `resolution` | See Section 7.3 | `performative` | Settles a dispute thread under a named policy. |

**Naming** — what a claim is about, and how a term is actually used.

| Kind | Value | Class | Purpose |
|---|---|---|---|
| `classifies` | subject CID | `definitional` | Subject membership is annotation, never a property of the object (R6). |
| `usage` | See Section 7.5 | `attributive` | For `definitional` claims. Cites where a sense was found. |
| `tagged` | `tag` matching `<domain>-<name>-MM-YYYY` | `definitional` | Section 9.1. A short citation tag for a publet. |

**Identity** — these target a **key** rather than a claim, which no other
group does (Section 10).

| Kind | Value | Class | Purpose |
|---|---|---|---|
| `trusts` | `weight` 1..1000, `subjects` | `performative` | Trust graph edges (Section 11.3). |
| `attests` | identity claims | `empirical` | Section 10.2. Carries evidence and method. |
| `affiliated` | organization, role, period | `attributive` | Section 10.4. |
| `personhood` | See Section 10.3 | `formal` | Optional. A proof, checked rather than believed. |
| `assumes-accountability` | See Section 10.7 | `performative` | Declaring it is what assumes it. |

**Artifact** — properties of objects and domains as stored things rather
than as assertions.

| Kind | Value | Class | Purpose |
|---|---|---|---|
| `timestamped` | `at`, `service`, `proof` | `attributive` | Section 10.5. A service asserts the object existed. |
| `witnessed` | `generation`, `log_root`, `observed` | `empirical` | Section 14.1.2. Observable, and compared across witnesses. |

Implementations MUST NOT present raw annotation counts as a quality signal;
annotations MUST be evaluated through a viewpoint before display (R7).

### 7.2 Settlement

R9 names two acts that settle a claim -- proof checking a `formal` one,
reproducing an `empirical` or `procedural` one -- and until now each had
its own annotation kind with its own shape, though both report the same
thing: an independent party executed what settles a claim of this class,
and what happened when they did. One kind carries both:

```
  value: { method:   <CID of a procedural claim executed>
                    | { system: <string>, version: <string>,
                        artifact: <CID> },
           outcome:  <per-class outcome, below>,
           deviations: <string>,
           result:   <CID of a claim stating what was observed>,
           data:     <CID of blob>,
           independence: { funding: <string>,
                           shared_materials: [ <CID>, ... ] } }
```

Rendering a document by a render pipeline (Section 8) is a settlement of
the pipeline's procedural claim in exactly this sense: the outputs, by
identifier, are what was observed.

`method` names what was executed. For an `empirical` or `procedural`
claim it is the CID of the procedural claim followed. For a `formal`
claim it is a checker descriptor -- the tool, its version, and the CID it
checked -- independently re-runnable the same way: fetch `artifact`, run
`system`@`version` against it, and compare.

`outcome` is drawn from the vocabulary its class settles by. For
`empirical` and `procedural` claims: `consistent` | `inconsistent` |
`inconclusive` | `method-underspecified`. `method-underspecified` reports
a defect in the documentation, not in the claim, and is expected to be
cheap and blameless to file. For `formal` claims: `verified` | `refuted`
| `inconclusive` -- a checker that exhausts its resources before deciding
is inconclusive, not a counterexample, and MUST NOT be treated as one.

Two settlements are **independent** under a viewpoint when no trust path
shorter than the policy's `independence_distance` connects their signers,
they share no `affiliated` annotation naming the same organization over an
overlapping period, they declare no shared funding source, and they share
no listed materials -- a different checker version or a different
toolchain counts for a `formal` claim the way a different lab does for an
`empirical` one. Settlements counted toward a replication floor MUST be
signed by human principals (R11).

### 7.3 Dispute Threads and Resolution

A **dispute thread** is the transitive closure of `disputes` and `supports`
relations from a target. Threads are computed; nobody owns one.

```
  value: { thread_root: <CID>,
           outcome: "sustained" | "overturned"
                  | "scope-narrowed" | "no-consensus",
           policy: <CID>, snapshot: <CID>,
           grounds: [ <CID>, ... ] }
```

A resolution carries no authority of its own; its force is the weight of
its signer under the reader's viewpoint (R7). Conflicting resolutions may
coexist. `no-consensus` is a first-class outcome and MUST NOT be treated as
failure.

**Novelty (R10).** A `disputes` relation whose grounds are contained in the
grounds of a resolution covering the same target is **redundant**.
Implementations MUST compute redundancy as set containment over grounds
CIDs. Viewpoints SHOULD assign redundant disputes zero weight in the
divergence factor. A disputant producing new grounds is never redundant,
however unpopular their position.

**Escalation.** A thread exceeding a policy-specified age with weighted
participation below a policy-specified floor is **stalled**. Index nodes
SHOULD surface stalled threads to weighted non-participants; pools and
bounty posters SHOULD prioritize them.

### 7.4 Machine-Proposed Objects (R14)

A model may generate any object as a candidate. The candidate becomes an
object when a key signs it, and that key gains or loses weight accordingly.
Keys operating principally on machine output SHOULD carry the
`is-automated` attestation.

Machine-assisted judgements are published as `triage` annotations with a
REQUIRED disclosure block:

```
  value: { finding: "redundant-with" | "novel" | "duplicate-of"
                  | "divergent-terms" | "unsupported-citation",
           refs:   [ <CID>, ... ],
           engine: { model, version, prompt: <CID of blob>,
                     inputs: <CID of snapshot>, sampling } }
```

Triage is **re-runnable, not reproducible**, and implementations MUST NOT
present it as equivalent to `settled`.

Implementations MUST NOT suppress, hide, collapse by default, or zero-weight
any object solely on the basis of a triage annotation. Redundancy under R10
is determined by set containment, never by a model's opinion. Triage MAY
order attention; flagged objects remain present, readable, and reachable. A
human override is a `disputes` relation against the triage annotation and
MUST cost no more than any other dispute.

A model judging novelty by similarity to an existing corpus is disposed to
classify heterodox arguments as restatements of refuted ones, and will do so
while reporting high aggregate accuracy. Deployments SHOULD publish the rate
at which triage findings are overturned on appeal, and SHOULD treat a low
rate as evidence that appeals are too expensive rather than that triage is
accurate.

### 7.5 Usage Citations

```
  value: { code: [ <content identifier, Section 6.2>, ... ] }
       |  { source: <path or URL>, locator: <string> }
```

A `usage` annotation is evidence for a `definitional` claim -- Section 5.2
permits no verdict on one, and usage stands in its place. Which shape
applies is decided by what the evidence is, not by preference.

**When the evidence is source code this corpus can hash**, the annotation
MUST name it by content identifier (`code`), computed the same way an
`implements` relation's raw-code endpoint is (Section 6.2) -- never by a
file path plus a description of where in it to look. A reader resolves
`code` by exact lookup against the current source tree: there is nothing to
match, so there is nothing to match incorrectly. `code` MAY name more than
one identifier when a single citation genuinely rests on several
definitions read together (two functions that only make sense in tandem).
Implementations MUST refuse to author a `source`-plus-`locator` citation
against a file they can already compute content identifiers for -- the
fragile form is not merely discouraged where the precise one is available,
it is not on offer.

**When no such identifier exists to compute** -- the evidence is a document,
an RFC section, prose, or code outside what this corpus indexes -- the
annotation falls back to `source` (a path or URL) and an optional `locator`
(free text). This form does not carry the cited text: recording a location
rather than a restatement is what keeps a corpus of definitions from
becoming a corpus of quotations, and is why citing a source needs no
licence from it.

**Staleness.** A `code` identifier absent from the current source tree
means the code has moved, been renamed, or been removed since the citation
was authored -- not a location to re-derive by guessing at what the author
might have meant instead. Implementations MUST report this as a disclosed
staleness, in the spirit of Section 14.3.3 ("the basis exists to disclose,
not to gate"), and MUST NOT silently substitute a different target.

---

## 8. Documents

```
  { pub: "1", type: "doc", created, author,
    body: { title, abstract: <CID>, lang, license: <SPDX>,
            sections: [ { heading,
                          items: [ { ref:  <CID>,
                                     bind: "object" | "lineage",
                                     at:   <CID>,   // if bind = lineage
                                     role: "assert" | "quote" | "contrast"
                                         | "background" | "counterpoint",
                                     view: { renderer: <string>,
                                             options:  <map> },
                                     gloss: <string> } ] } ] } }
```

A document contains no assertions of its own. `gloss` is presentational
connective tissue and MUST NOT carry claims; clients SHOULD render glosses
distinctly. `role` makes a document's *use* of a claim explicit —
including a claim as `counterpoint` is not endorsing it.

`view` says how this document shows the cited object's `data`
(Section 5.8) at this point: `renderer` names a presentation, such as a
table or a chart, and `options` are that renderer's settings -- which
columns, in what order, grouped how, with what totals and number format.
Renderer names and their options are not registered; they are agreements
between a document's author and the clients that render it. Like `gloss`,
a view is presentational and MUST NOT carry claims: a renderer MUST NOT
display a value that is neither in the cited object nor computed from it,
and SHOULD mark computed values, such as totals, as computed. A client
that does not recognize a renderer, or cannot apply its options, MUST
still display the cited object, and MUST NOT omit the item. Because the
view lives in the citing document and not in the cited claim, the same
data MAY be a full table in one document and a two-column summary or a
chart in another, and none of those renderings is itself an object.

**Non-normative.** A document MAY say how it is itself rendered by citing,
as `background`, a **render pipeline**: a publet bundling a `procedural`
claim whose `depends` lists its steps in order (Section 5.7), each step
naming the identity claims of the programs it runs. Citing a procedure as
background asserts nothing, so the document stays free of assertions; the
pipeline never cites the documents it renders, so no cycle arises; and
because the citation is a CID, the rendering a reader is shown can be
checked against the exact method, tools, and versions its author named.
The rendering itself remains no object, as above.

Such a document SHOULD cite the pipeline by lineage (`bind: "lineage"`),
so that revising the pipeline -- a new tool version, a reworded step --
is not a reason to revise every document it renders: the document records
the version its author read in `at`, and a client renders with the
lineage's current head. When a rendering is accepted, its lineage is pinned
as a `settled` annotation on the pipeline's procedural claim (Section
7.2): `result` names a claim stating which document was rendered and which
outputs, by identifier, each step produced with which tool versions, and
`data` names the accepted output kept as a blob. Two settlements whose
outputs carry identical identifiers are an equivalence anyone can check by
rendering again; no one need assert it.

A pipeline MAY in turn cite, as `background`, an **environment**: a publet
stating how to provision the pipeline's toolchain into a local directory --
procedural steps whose fetched artifacts are pinned by identifier -- and
the **host requirements** it cannot provision itself, each marked as needed
to provision or at run time. An environment is keyed by its own identifier,
not by any document, so few are needed and every pipeline citing one
shares it; running only what it provisioned is what makes a rendering
depend on recorded inputs rather than on the machine. *Future work:* an
environment could equally be provided as a container image pinned by
digest, which suits a deployed rendering service; the publet that states
the toolchain would not change.

`bind` declares what the author meant to cite (R15). With `bind: "object"`
— the default — `ref` is a claim CID and the citation is permanently
fixed. With `bind: "lineage"`, `ref` is a genesis CID, `at` records the head
the author actually read, and a client resolves the current head under the
reader's viewpoint.

A client rendering a lineage-bound item MUST display the resolved object,
MUST make `at` reachable, and MUST indicate when the two differ. This gives
a reader both what the author read and what has since been said, which
neither binding alone provides. A textbook citing a physical constant wants
a lineage; a paper citing a specific cohort result wants an object.

Documents are immutable (R1) and form lineages by the same rule as claims
(Section 6.1); revision is a new document plus `supersedes`. Errata,
editions, and corrections are lineage generations, and a document lineage
MAY be anchored (Section 9.3).

Forking is permitted. A fork reusing another document's structure MUST
carry `derived-from`, and viewpoints SHOULD heavily discount an undeclared
fork detected by structural overlap. Composition defects are stated as
`critique` annotations (Section 7.1); clients MUST surface critiques when
rendering a document and SHOULD offer a view with named omissions
interleaved.

Documents MAY be authored by keys carrying no identity attestation.

---

## 9. Naming and Curation

### 9.1 No Namespace

PUB defines no mapping from strings to objects. Names enter as
`definitional` claims describing a term's meaning and usage, and as
`classifies` annotations attaching objects to them. Competing definitions
for the same string are the normal case, not a conflict to resolve.

A subject is a keyword claim that other keyword claims are classified
under. There is no distinguished taxonomy; multiple taxonomies coexist as
sets of `classifies` annotations by different keys, selected among by
viewpoint (R7).

**Short citation tags.** A publet (Section 1.4) is found by a short,
human-typable tag rather than by its CID, for the same reason a paper is
cited as `[Smith2024]` rather than by its DOI in running prose. The
temptation this invites is precisely what this document warns of
elsewhere: a namespace a reader can be steered by is a namespace someone
can capture (Section 1.1, `namespace capture`). The tag is therefore
built the same way every other name in this section is -- an annotation,
not a field on the tagged object itself:

```
  { kind: "tagged", target: <CID>, value: { tag: <string> } }
```

`tag` MUST match `<domain>-<name>-MM-YYYY`: `domain` is 3-6 letters
naming a domain or subject area, `name` is exactly six letters mnemonic
for the publet, followed by the two-digit month and four-digit year the
tag was assigned. The domain segment is a range rather than a fixed
width because a domain's own natural abbreviation is not always six
letters -- forcing one would trade recognizability for a rule that
serves nothing. The rest stays fixed so a tag is still recognizable as
one at a glance; nothing about the format is authoritative. Anyone
MAY tag any object (R6); two keys tagging different objects with the same
string is a namespace collision handled exactly like two `classifies`
annotations attaching different objects to one keyword -- both stand,
and which a reader sees is resolved per viewpoint, never by precedence or
by who filed first.

A tag is advisory in the same sense `basis` is (Section 4.6): withdrawing
one is a `retracts` relation against the `tagged` annotation, never an
edit, and the tagged object's own CID -- its actual identity -- never
depends on whether a tag naming it exists at all.

### 9.2 Definitional Divergence

Given a `disputes` relation between `P` and `Q`, an implementation MUST
compute:

```
  pairs(P, Q) =
    { (D_p, D_q) : D_p in closure(P.depends), D_q in closure(Q.depends),
                   D_p != D_q,
                   D_p.class = D_q.class = "definitional",
                   term(D_p) = term(D_q),
                   no trusted `equivalent` relation joins D_p and D_q }

  divergent_terms(P, Q) = { t : (D_p, D_q) in pairs,
                                lineage(D_p) != lineage(D_q) }

  stale_terms(P, Q)     = { t : (D_p, D_q) in pairs,
                                lineage(D_p) == lineage(D_q) }
```

The distinction is R15 and the remedies differ.

**Divergence** — the parties depend on definitions with no common genesis.
They mean different things by one word. The constructive response is two
scoped claims, one asserting the claim under each definition, not a ruling
on which definition is correct.

**Staleness** — the parties depend on different generations of one
definitional lineage. They mean the same thing, one of them as it was
understood earlier. The constructive response is for the party on the older
generation to supersede their claim against the current head, or to state
in `scope` why the earlier generation is the one they intend.

Where either set is non-empty, clients MUST surface it, naming the term and
both definitions, before presenting the dispute as factual, and MUST label
which of the two it is. Index nodes SHOULD emit both as structured fields on
any dispute returned.

### 9.3 Lineage Anchors

**Non-normative** (Section 1.1). An anchor is a maintained shortcut to
the head of a lineage, for readers who want a curated answer rather than
a computed one. Anchors are where
curation attaches (R15): stewards do not own assertions, which are
immutable and authored by whoever signed them, but they do maintain a
recommended path through a line of work.

```
  { pub: "1", type: "anchor", created, author,
    body: { lineage: <genesis CID>, current: <CID>, rationale: <CID>,
            stewards: [ <CID>, ... ],      // 1..9, human principals (R11)
            threshold: <M>,
            qualified: { policy: <CID>, floor: <integer>,
                         nominated: [ <CID>, ... ] },
            liveness: { renew_within_days: <integer> },
            prev: <CID> } }
```

`current` MUST be a member of the named lineage. An anchor over a
branching lineage is a steward circle's declaration of which branch they
recommend, which is the ordinary case rather than an exception. A claim,
document, or definitional lineage MAY each be anchored, and a concept whose
definition has its own lineage is anchored through that lineage rather than
through a fixed definitional CID.

Anchor states are immutable (R1); updating means publishing a new state
with `prev` set, carrying `threshold` steward signatures.

**Anchors are not load-bearing.** Nothing in Sections 4 through 8 consults
one, and an implementation ignoring this section entirely is conformant. A
reader who trusts no anchors loses convenience only: lineages remain
computable and every claim remains retrievable and evaluable.

Adding a steward requires `threshold` circle signatures plus the nominee's
acceptance. A steward MAY resign unilaterally. If the circle falls below
`threshold` live signatories, or liveness lapses, the anchor becomes
**dormant** and any qualified key meeting the declared floor MAY publish a
successor. Competing anchors coexist and resolve per viewpoint; there is no
election, because an electorate requires a global identity registry.

No cap on anchors per key is specified. Anchor counts and acquisition rates
are computable; viewpoints SHOULD discount implausible holdings and clients
SHOULD display counts wherever an anchor is shown.

---

## 10. Identity

### 10.1 Keys

```
  { pub: "1", type: "key", created, author: <self>,
    body: { alg: <signature-algorithm-id>, pubkey: <bytes>,
            principal: "human" | "organization" | "automated", // REQUIRED
            label: <string>, rotates: <CID>,
            policy: { valid_from, valid_until } } }
```

The CID of the key object is the identity. `label` is advisory, non-unique,
and MUST NOT be used for resolution or display without a fingerprint.

There is no identity registry, no proof of personhood requirement, and no
one-human-one-key assumption. Every mechanism here is designed to remain
correct while an adversary holds arbitrarily many keys.

### 10.2 Assurance

A key MAY accumulate `attests` annotations from other keys claiming
`same-person-as`, `affiliated-with`, `credentialed-in`, `distinct-from`,
`is-organization`, or `is-automated`, each with evidence and method.

A key with no attestations is fully anonymous and fully able to publish. It
carries weight only where a trust path reaches it (R7). A pseudonymous key
can accumulate standing over years of corroborated work without its holder
ever being identifiable.

### 10.3 Human Signatories (R11)

Authorship of claims, counted reproductions, and anchor stewardship are
reserved to keys declaring `principal: "human"`. Organizations and
automated systems may hold keys, sign, fund, and operate infrastructure;
they may not be the author of record.

A key declaring `organization` or `automated` MUST NOT count toward a
replication floor and MUST NOT hold stewardship. Viewpoints SHOULD discount
claims authored by such keys. Clients MUST display principal type wherever
authorship is shown.

Institutions are immortal and humans are not. An organizational key accrues
merit indefinitely and compounds authority across generations of staff who
bear none of the consequences. Human signatories bound every reputation in
the system by a lifespan.

**Personhood attestation** is OPTIONAL. A zero-knowledge presentation over
an issuer-signed credential, carrying a nullifier `PRF(holder_secret,
scope)`, proves a human-issued credential underlies a key without revealing
whom, and bounds keys per person within a scope.

```
  { kind: "personhood", target: <CID of key>,
    value: { scheme, scheme_version, issuer_set: <CID of document>,
             scope, nullifier: <bytes>, proof: <bytes>,
             anonymity: { unlinkable_across_scopes: bool,
                          issuer_can_link: bool,
                          revocation_leaks_linkage: bool } } }
```

- Personhood MUST NOT be a precondition of publishing anything.
- A policy requiring it MUST accept issuers from multiple independent
  jurisdictions.
- Issuer sets MUST be published as content-addressed documents, so that
  changes are visible, dated, and disputable.
- Implementations MUST support at least two schemes, MUST display the
  `anonymity` block, and MUST NOT present personhood as binary.
- Viewpoints MUST NOT let personhood override Section 11.4. Uniqueness is
  not honesty: the documented failures of modern science were committed by
  identified, credentialed, unique people, and were detected by replication.
- Where personhood is used, it SHOULD be proven by zero-knowledge
  presentation rather than by signing objects with a credential key, which
  would import non-repudiation into a record that depends on authors being
  free to revise and retract.

**NORM:** Deployments should resist requiring personhood. It is the most
requestable mechanism here, and the cost of conceding it falls on the
participants with the least protection.

### 10.4 Affiliation

```
  { kind: "affiliated", target: <CID of object or key>,
    value: { org: <CID of organization key>,
             role: "employer" | "funder" | "host-facility"
                 | "data-provider" | "sponsor",
             period: { from, to }, disclosed_by: "self" | "org" } }
```

Affiliation is metadata for resource tracking, funding disclosure, and
conflict analysis — not authorship. It is load-bearing for independence
(Section 7.2): without it, four replications by one consortium cannot be
distinguished from four independent ones.

### 10.5 Rotation, Revocation, Timestamping

A new key object with `rotates` set, signed by **both** keys, transfers
continuity. A `revoke` object signed by the key, or by a threshold it
designated in advance, declares it invalid from a stated instant with scope
`all` or `after-effective`.

A `timestamped` annotation from an independent service asserts that a CID
existed at a time. Authors SHOULD timestamp; archives MUST. Timestamping is
what preserves the value of signatures whose authors are no longer
available to re-sign, which over archival timescales is all of them.

A lost key is a key that publishes nothing further. Under R1 nothing breaks
and no recovery mechanism is required.

### 10.6 Delegation

```
  { type: "claim.relation", body: { kind: "delegates", from: <CID of key>,
      to: <CID of key>,
      aspect: "lineage" | "stewardship" | "authorship",
      effective: "immediate" | "on-dormancy",
      dormancy_days: <integer> } }
```

Delegation transfers the capacity to *continue* — to supersede, to steward,
to maintain a line of work. It does not transfer authorship of existing
claims, which R1 forbids. Merit already earned stays with the key that
earned it. With `effective: "on-dormancy"` it is a pre-signed instrument
taking effect after a stated silence.

Both endpoints are keys and carry no identity, so a delegation discloses
nothing about either holder.

### 10.7 Assumed Accountability

```
  { kind: "assumes-accountability", target: <CID of key>,
    value: { scope: "all" | [ <CID of subject>, ... ],
             grounds: "identity-known-to-me" | "work-reviewed-by-me"
                    | "institutional",
             from: <RFC3339>, until: <RFC3339> | null } }
```

A key publicly declares that it stands behind another key's output while
disclosing nothing about who holds it. A viewpoint requiring personhood MAY
accept a pseudonymous key on the strength of an assumption from a key that
has one. The assumer stakes real standing: the assumed key's failures cost
the assumer weight in every subject where they hold any.

- An assumption MUST NOT be treated as authorship. The assumed key remains
  the author and accrues the merit.
- Implementations MUST NOT infer, display, or record linkage between an
  assumer and the holder of an assumed key.
- An assumption MAY be withdrawn prospectively, never retroactively.
- A viewpoint MUST NOT let an assumption substitute for Section 11.4.

Assumptions are published rather than hidden so that fronting is detectable
in aggregate: the count of assumptions a key holds, their rate, subject
spread, and correlation with disclosed affiliation are all computable.
Viewpoints SHOULD discount implausible holdings; clients MUST display the
count wherever an assumption is relied upon.

---

## 11. Evaluation

### 11.1 Policy Object

```
  { pub: "1", type: "policy", created, author,
    body: { roots: [ { key: <CID>, weight: <integer>,
                       subjects: [ <CID>, ... ] | "all" } ],
            damping: <integer>,          // scaled by 10^6
            iterations: <integer>,
            edge_sources: [ "explicit" | "endorsement" | "coauthorship" ],
            decay: { half_life_days } | null,
            tau: <integer>, delta_max: <integer>,
            class_rules: { <claim-class>: <rule> },
            replication_floor: <integer>,
            independence_distance: <integer>,
            equivalence: "trusted-only" | "none",
            snapshot: <CID> } }
```

A policy MUST declare at least one root.

### 11.2 Propagation

Weight is a personalized PageRank over the scoped trust graph. Let `R` be
the seed vector over roots normalized to 10^6, `T` the trust matrix with
each key's outbound declared weights normalized, and `a` the damping:

```
  W_0     = R
  W_{n+1} = ((10^6 - a) * R  +  a * (W_n · T)) / 10^6
  W       = W_K              where K = policy.iterations
```

**All arithmetic MUST be integer, scaled by 10^6, division truncating
toward zero. Evaluation MUST terminate after exactly `K` iterations, never
on a convergence criterion.** This is R8: settlement (Section 12) depends
on bit-identical results across implementations, which floating point and
convergence thresholds cannot provide.

`K` SHOULD be at least 20; implementations MUST reject `K` above 200.

An edge scoped to subject *S* is present only when evaluating a target
classified under *S* or a descendant, descent itself computed under the
same policy. If `decay` is set, an edge's contribution is multiplied by
`2^(-age_days / half_life_days)` in scaled integer arithmetic.

Per-key normalization of outbound trust fixes a key's total conferred
weight regardless of how many keys it vouches for. A disconnected adversary
subgraph receives weight `0` for every size (R7). The residual attack is
compromise of a key the reader already trusts, mitigated by small root sets
and decay.

### 11.3 Standing

```
  standing := { affirm_weight, deny_weight, abstain_weight, active_weight,
                reproductions: { consistent, inconsistent, inconclusive,
                                 independent_consistent },
                reproducibility_class,
                retracted: bool,
                lineage: <genesis CID>,
                head: [ <CID>, ... ],          // under this viewpoint
                superseded_by: [ <CID>, ... ],
                class,
                result: "accepted" | "contested" | "rejected"
                      | "undetermined" | "unreplicated" | "not-truth-apt" }
```

Clients MUST NOT reduce standing to a badge without exposing its
components, and MUST render `scope` wherever they render a standing.

For `definitional`, `normative`, and `expressive` classes, `result` is
always `not-truth-apt`.

### 11.4 Evidence Dominance (R9)

**Formal.** A `formal` claim with an accepted `settled` annotation
carrying a `verified` outcome from a checker the policy recognizes is
`accepted` regardless of endorsement weight, and no quantity of denial
changes it.

**Empirical.** A policy MUST define a `replication_floor`: a minimum count
of independent `consistent` reproductions signed by human principals below
which `accepted` is unreachable whatever the endorsement weight.
Independent `inconsistent` reproductions MUST outweigh endorsement of any
magnitude.

A viewpoint MUST NOT return `accepted` for an `empirical` claim whose
reproducibility class is `restricted`; the ceiling is `unreplicated`. For
`unique-event` claims the floor is satisfied instead by independent
contemporaneous observation or verified provenance.

`unreplicated` is not a criticism. It is a visible, permanent,
machine-readable demand for work, and the set of well-endorsed,
widely-cited, never-reproduced claims is a query.

### 11.5 Acceptance Predicate

With `A` = affirm weight, `D` = deny weight, `V = A + D`:

```
  accepted     iff  V > 0 and A / V >= tau + delta and REPLICATION_OK
  rejected     iff  V > 0 and D / V >= tau + delta
  contested    iff  V > 0 and neither of the above
  unreplicated      if the weight condition for accepted holds
                    but REPLICATION_OK does not
  undetermined      otherwise
```

`REPLICATION_OK` is vacuously true for non-`empirical` classes. The
denominator is weight that evaluated this claim, not the network's total
active weight.

Replication gates `accepted` but not `rejected`: one independent
inconsistent reproduction is strong evidence against a claim, while any
number of endorsements is weak evidence for one.

```
  delta = min( delta_max,  delta_max * D_arg / (A + D_arg) )
```

`D_arg` is the weight of disputes that are argued, non-redundant under R10,
and not covered by a weighted `sustained` resolution. `no-consensus`
resolutions contribute in full.

### 11.6 Equivalence

Where `equivalence` is `trusted-only`, the evaluator computes connected
components over `equivalent` relations whose signers carry non-zero weight
and aggregates affirm and deny weight across each component before applying
Section 11.5. A signer's weight MUST be counted at most once per class.

### 11.7 Snapshots and Results

```
  { type: "snapshot", body: { root: <merkle root over sorted CIDs>,
                              count, as_of, source } }

  { type: "eval", body: { policy: <CID>, snapshot: <CID>, target: <CID>,
                          standing, engine, engine_version } }
```

Given a policy, a snapshot, and this document, any implementation MUST
produce a bit-identical standing (R8). A published `eval` that does not
reproduce is checkable evidence of a faulty or dishonest evaluator.

### 11.8 Defaults

Clients MUST ship a default policy, MUST make its roots inspectable in one
interaction from any displayed standing, and MUST allow full replacement.

Evaluation SHOULD be performed locally against a domain replica (R16), in
which case a policy is never transmitted. Where evaluation is delegated,
the policy CID discloses the reader's trust configuration (Section 17), and
implementations SHOULD offer evaluation under a generic published policy as
an alternative.

**NORM:** Default policies should be published by plural, independent,
mutually critical bodies, and vendors should ship several. Concentration of
readers on one default is the likeliest path by which a deployment
recreates the authority this design avoids.

---

## 12. Settlement

**Non-normative** (Section 1.1): §12.1 states this outright ("an
implementation MAY omit this section entirely and remain conformant for
all other purposes"). Reading, checking, and evaluating a claim needs
none of what follows.

### 12.1 Boundary (R13)

The settlement layer funds review, replication, and mirroring. It is bound
by three rules:

1. The ledger has no epistemic authority. It executes predicates over
   `eval` objects that anyone can recompute (R8), each chosen in advance by
   the party putting up the money.
2. The ledger MUST NOT gate read access to any object. There is no paid
   tier, no rent-to-retain, and no access fee. What funding buys is
   **replication priority** — a claim on the mirroring subsidy — never
   whether anyone may read.
3. Participation is optional. Every other layer functions with no ledger
   present, and an implementation MAY omit this section entirely and remain
   conformant for all other purposes.

### 12.2 Ledger Requirements

This document specifies no consensus mechanism. A conformant ledger MUST
provide: a deterministic total order over accepted transactions; a stated
finality condition; conditional release of locked funds by predicate;
transactions carrying PUB CIDs durably; open participation; and a stated
bound on censorship of valid transactions.

PUB is ledger-agnostic and multi-ledger. This is the only part of the
protocol requiring global consensus and the only part with a regulatory
perimeter.

### 12.3 Bounties

```
  { pub: "1", type: "bounty", created, author,
    body: { target: <CID>, ledger, amount,
            policy: <CID>,                  // the settlement viewpoint
            snapshot_rule: { as_of },
            requires: { min_reviews, min_effort, independence },
            base_share: <basis points>,
            window: { opens, closes }, refund_to } }
```

The poster names the viewpoint their money settles against. The ledger
enforces a choice published and content-addressed before any work began; it
does not decide whose judgement counts. The same claim may settle as
accepted under one bounty and rejected under another, and both settlements
are correct.

An escrow has two parts:

- The **review share** (`base_share`, minimum 6000 basis points) pays for
  labour and is divided among all reviewers satisfying `requires`,
  **irrespective of finding**. Implementations MUST reject a bounty below
  this floor.
- The **system share** returns to `refund_to` if the target **resolved** in
  either direction, and is divided among qualifying reviewers if the window
  closed `undetermined`.

The system share returns on resolution, not acceptance: an author paid for
being accepted would have an incentive to select a lenient settlement
policy. Being clearly wrong costs an author nothing.

### 12.4 Bounty Kinds

| Kind | Funds | Paid for |
|---|---|---|
| Review | reviewer labour | filed review objects meeting `requires` |
| Replication | executing a declared method | a filed `settled`, any outcome, including `method-underspecified` |
| Access | opening restricted evidence | a controller granting independently verifiable access, converting `restricted` to `open` |
| Equivalence | deduplication labour | a signed `equivalent` relation with an argued claim |
| Mirror | storage and bandwidth | random-challenge retrieval proofs against a committed set |

A bounty whose payment depends on the result is not buying evidence; it is
buying a conclusion.

Reviewer qualification is viewpoint-gated: a reviewer qualifies only where
their weight under the bounty's policy exceeds a stated floor, so fresh
keys and Sybil farms qualify for nothing. `independence` rules MAY exclude
reviewers sharing a short trust path with the author, recent co-authors, or
keys named in a declared conflict.

Machine-assisted review is governed by R14. A reviewer who signs unread is
defrauding the bounty, is detectable from signing rates the graph records,
and loses the only asset that qualifies them.

**NORM:** Pools should prioritize objects that are highly cited, poorly
replicated, or at elevated risk of suppression over objects that are merely
popular.

### 12.5 Prohibitions

A conformant implementation MUST NOT condition retrieval, resolution, or
indexing on payment; assign standing on the basis of amounts staked, spent,
or earned; treat bounty value as an input to any function in Section 11; or
represent a settled bounty to users as a determination of truth.

---

## 13. Nodes

### 13.1 Roles

**Mirror** — stores and serves objects by CID. **Index** — computes and
serves resolutions under declared policies. **Evaluator** — computes and
publishes `eval` objects. **Archive** — a mirror committing to indefinite
retention and timestamping. **Settlement participant** — interacts with a
ledger.

### 13.2 Serving Obligation (R12)

A mirror MUST serve any structurally valid object in its declared set to
any requester, without discrimination on the object's content, claim class,
subject, language, author identity or jurisdiction, or the requester's
identity or jurisdiction.

A mirror MUST NOT: return not-found for an object it holds; apply selective
latency, rate limits, or degradation; require payment, registration,
authentication, or assent to terms; or withhold objects that are retracted,
disputed, superseded, rejected under any policy, or offensive to the
operator.

A mirror's declared set is its own choice, expressed as a set of domain
CIDs (Section 14.1); a topically specialized mirror is fully conformant.
Nodes MUST publish their declared set, so that selective withholding is
detectable by anyone who reads the declaration. Where a requester holds the
domain, withholding is detectable by snapshot-root comparison without any
request at all.

Ceasing to serve an object in the declared set requires a published
**tombstone**:

```
  { pub: "1", type: "tombstone", created, author,
    body: { target: <CID>, set: <CID>, effective: <RFC3339>,
            cause: "legal-order" | "operator-decision" | "loss"
                 | "capacity",
            authority: <string>, note: <string> } }
```

Under L5 a lawful removal order is defined, bounded, appealable, and
publicly recorded; the tombstone is its protocol-visible counterpart.
Undisclosed removal remains a violation. Peers MUST treat a tombstone as a
replication trigger: a conformant mirror receiving one for an object it
does not hold SHOULD acquire and serve it.

### 13.3 Index Nodes

An index node MUST publish the CID of every policy it evaluates under,
accept a caller-supplied policy CID, return with every result the policy
CID and snapshot CID and enough detail to recompute the ranking, and never
insert items the declared policy's evaluation does not yield.

Manipulation is therefore detectable by recomputation, which is the only
form of prohibition a remote party can check.

### 13.4 Evaluator Nodes and Archives

An evaluator MUST make its engine and version identifiable and MUST produce
results that reproduce under R8. An archive MUST timestamp every object it
accepts and MUST retain objects regardless of subsequent retraction,
dispute, or standing.

### 13.5 Enforcement

There is no protocol-level enforcement. A peer observes a violation and
publishes signed evidence; other peers verify it independently; peers stop
routing to the violator and revoke `trusts` edges; and because objects are
content-addressed and replicated, requests are satisfied elsewhere. This is
eviction by irrelevance, and it works to the degree that replication is
real.

---

## 14. Domains and Transport

### 14.1 Domains (R16)

A **domain** is a bounded, content-addressed collection of objects
retrievable in full. Reading and evaluation are performed against a local
replica of a domain, not by querying another party's node.

```
  { pub: "1", type: "domain", created, author,
    body: { label:    <string>,
            snapshot: <CID>,              // of a snapshot object (11.7)
            bound:    <integer>,          // declared byte ceiling
            size:     <integer>,
            closed_under: [ "depends" ],
            split_of: <CID> } }           // if produced by a split
```

A domain manifest is an ordinary object. Anyone may publish one, competing
definitions coexist, and selection among them is viewpoint-relative (R7).
There is no registrar and no authoritative partition of knowledge.
Successive generations of a domain form a lineage by `supersedes` like any
other object (R15); `split_of` records partition rather than revision.

Three constraints make local-first operation actually achievable. They are
normative because without them the property silently fails.

**Domains MUST be closed under `depends`.** A domain contains the full
dependency closure of every claim it holds. Without this, evaluating a
local replica requires fetching definitional claims from elsewhere, and
the privacy property is lost on the first unresolved term. Closure is
required over `depends` only — not over `evidence`, `disputes`, or
`supports` — which keeps it tractable, since dependency closures consist of
`definitional` claims and those are small. Shared definitions are
duplicated across domains at identical CIDs, which costs storage and
nothing else.

**Domains MUST NOT contain blobs.** Datasets, images, audio, and video are
referenced by CID and fetched separately. A domain is objects only.
Without this the size bound is unachievable. Fetching a blob is a
disclosure, but a coarse one: it reveals interest in a dataset, not the
proposition a reader was evaluating.

**Domains MUST declare and respect a size bound.** When a domain exceeds
its declared `bound`, it is split: two or more new domain manifests are
published, each within the bound and each `depends`-closed, with
`split_of` naming the predecessor. The bound is what makes full retrieval
and local evaluation affordable, and therefore what makes every property
in Section 14.2 hold.

Reviewed knowledge and proposal domains are bounded identically. There is
no architectural distinction between reading settled work and reading work
under consideration.

#### 14.1.1 Generations and Consistency

A domain evolves. Successive states are **generations**, and a reader must
be able to verify that a generation contains everything its predecessor
contained. A timestamp cannot establish this: `created` is an unverified
claim (Section 4.3), two generations bearing the same instant are
indistinguishable, and an ordering says nothing about containment.

This matters more under R16 than it would otherwise. A reader querying a
node can compare answers across nodes; a reader holding a replica has no
second opinion. Local-first operation therefore converts the omission
attack from "omit from a query response" into "omit from a distribution" —
persistent, inherited by every downstream copy, and invisible. Generation
consistency is what makes a local replica trustworthy rather than merely
private.

Two Merkle structures are REQUIRED, because one cannot do both jobs:

- A **generation log**: an append-only Merkle tree whose leaves are
  successive snapshot roots, in order. Supports **consistency proofs** —
  that log state *m* is a prefix of log state *n*.
- A **membership tree** per generation: a Merkle tree over that
  generation's member CIDs in sorted order. Supports **inclusion** and
  **absence** proofs for a given CID.

A sorted tree alone cannot yield efficient consistency proofs, since
insertion reorders internal nodes; an append-only log alone cannot yield
absence proofs. Implementations MUST publish both and MUST serve proofs of
each kind on request.

A publisher advancing a domain MUST issue a consistency proof from the
previous generation and MUST publish a **generation record** stating the
membership change explicitly:

```
  { pub: "1", type: "generation", created, author,
    body: { domain:   <CID of domain manifest>,
            index:    <integer>,          // monotonic, no gaps
            parent:   <CID of previous generation record>,
            snapshot: <CID>,              // membership root after this change
            added:    [ <CID>, ... ],
            removed:  [ { cid: <CID>,
                          cause: "split" | "tombstone" | "superseded-domain",
                          ref:   <CID> } ] } }
```

Every entry in `removed` MUST carry a `ref` accounting for it — a
`split_of` manifest, a tombstone (Section 13.2), or a successor domain.
A generation whose membership root implies a removal not listed in
`removed`, or a listed removal with no `ref`, is **malformed** and
implementations MUST reject it.

Declaring removals rather than leaving them to be discovered is the point.
An inclusion proof in one generation and an absence proof in the next will
reveal a removal to anyone who thinks to look; a generation record makes
the publisher state it, with a justification, as a condition of the
generation being well formed at all.

#### 14.1.2 Witnessing

Consistency proofs bind a publisher to one history. They do not prevent a
publisher showing *different* histories to different readers.

A **witness attestation** is an ordinary annotation:

```
  { type: "claim.annotation", body: { kind: "witnessed", target: <CID of domain>,
      value: { generation: <integer>, log_root: <bytes>,
               observed: <RFC3339> } } }
```

Any holder may publish one. Because a log root is a single small value,
witnessing is cheap to gossip and cheap to compare, and a split view is
detectable as two witnessed roots at the same generation that no
consistency proof reconciles. Readers SHOULD witness the domains they hold
and SHOULD check witnesses from keys with no trust path to the publisher.

Timestamps remain necessary and are not displaced: the generation log
establishes integrity and order, while `observed` and Section 10.5
attestations establish *freshness* — whether a replica is current, and how
stale it has become.

### 14.2 Local-First Operation

Where a reader holds a domain replica:

- **No party observes what is read.** No request is issued, so nothing
  exists to log, subpoena, or correlate. This is elimination, not
  mitigation.
- **No party learns the reader's policy.** Evaluation is local, so trust
  roots, thresholds, and replication floors never leave the machine.
- **Reverse edges are complete and verifiable within the domain.** The
  reader computes them from the replica. No node can omit a refutation,
  because no node is answering. Suppression by omission does not operate
  against a local replica, provided the replica's generation history is
  verified (Section 14.1.1); without that check the attack moves upstream
  into distribution rather than disappearing.
- **No evaluator need be trusted.** The reader is the evaluator. R8's
  reproducibility requirement remains necessary for settlement
  (Section 12), where a third party computes, and is otherwise moot.
- **Evaluation cost is bounded by the domain bound**, not by the size of
  the graph.

These follow from the size bound rather than from any cryptographic
mechanism, which is why the bound is normative.

### 14.3 Synchronization and Query Mode

#### 14.3.1 Delta Synchronization

A reader at generation *m* advancing to generation *n* fetches the
generation records *m+1..n* and the objects their `added` lists name. Full
retrieval is needed only on first acquisition.

Deltas are **self-verifying**. After applying them a client MUST recompute
the membership root and compare it to generation *n*'s `snapshot`, and MUST
verify the consistency proof from *m* to *n* (Section 14.1.1). A delta that
adds, omits, or substitutes anything fails the root comparison, so a client
need not trust the peer that served it.

To bound the cost for a client far behind, publishers SHOULD offer
**checkpoint deltas** at exponentially spaced intervals — cumulative
changes from generation *n*−1, *n*−2, *n*−4, *n*−8, and so on — so that any
gap is covered in O(log n) fetches. A publisher unwilling to retain history
MAY serve full packs only; a client with no common ancestor falls back to
full retrieval.

Pack and delta streams SHOULD be compressed. Per-object delta encoding is
not specified and is not worth its complexity here: objects are immutable
(R1), so a revision is a new object rather than a modified one, and objects
are capped at 64 KiB with most far smaller.

#### 14.3.2 What Synchronization Discloses

Delta synchronization discloses one integer: the generation the reader
holds. It does not disclose which objects the reader has, has read, or
lacks.

Implementations MUST NOT use `have`/`want` style set negotiation, in which
a client enumerates its holdings so a peer can compute a minimal transfer.
Partial holdings are a fingerprint of what a reader has been working with,
and disclosing them forfeits much of what R16 provides. Merkle-subtree
reconciliation carries the same disclosure at finer granularity; it MAY be
offered for peers with no common generation ancestor or holding partial
replicas, and where offered a client MUST be warned before use.

Synchronizing a domain discloses that the reader follows that domain, and
nothing about which objects within it were read. This is a coarse signal
and a large reduction, not zero. Readers with a threat model SHOULD
synchronize over an anonymizing transport, SHOULD synchronize domains
beyond those they need, and SHOULD prefer mirrors carrying many domains.

**Query mode** — retrieving individual objects from another party's node —
remains available and is the degraded path. It exists for thin clients,
casual lookups, and cross-domain references. It re-exposes everything
Section 14.2 eliminates: the serving node learns exactly which proposition
was requested, and a supplied policy CID discloses the reader's whole trust
configuration.

Implementations MUST default to local-first operation where the reader has
the storage to hold the relevant domain, MUST make the mode in use visible,
and MUST warn before a first query that discloses a personal policy.

**NORM:** The great majority of readers will use thin clients on devices
that cannot hold a domain, and will therefore use the degraded path. The
privacy properties of Section 14.2 are real and are unavailable to most
people most of the time. Deployments should size domains with this in mind
rather than treating query mode as an edge case.

#### 14.3.3 Proposing Against a Basis

Under R16 a reader composes new objects against a local replica at a known
generation. The domain may advance between the moment they read and the
moment their work lands. A submission therefore declares what it was
composed against:

```
  { pub: "1", type: "proposal", created, author, basis,
    body: { domain:  <CID of domain manifest>,
            objects: [ <CID>, ... ],   // the objects being submitted
            touches: [ <CID>, ... ] }} // lineages and threads addressed
```

**A stale `basis` is never grounds for rejection** (Section 4.6).
Nothing in this protocol is overwritten (R1), so a proposal cannot
clobber a concurrent one and there is no lost update to prevent.
Divergent successors are a branching lineage, which is permitted and
resolved per viewpoint (Section 6.1). The basis exists to disclose,
not to gate.

What implementations MUST do is compute and display the **relevant delta**:
the membership changes between `basis` and the current generation,
intersected with the closure of `touches`. This is cheap — generation
records carry explicit `added` and `removed` lists (Section 14.1.1) — and
it isolates the three ways a proposal can be stale:

- the object being superseded has itself been superseded since `basis`,
  so the lineage will branch;
- a claim in the `depends` closure has been superseded, making the
  proposal's definitional dependency a stale generation (Section 9.2);
- a dispute the proposal answers has been resolved, which may make it
  redundant under R10.

Each is reported to the author before submission and recorded for
reviewers afterward. None blocks the write.

Both ends of the race are permanently recorded and need no extra
machinery: `basis` is the generation the author read, and the generation
record that first contains the proposal's objects is the generation it
landed in. The interval between them is the exposure, and it is auditable
by anyone.

This is the same pattern as a document item's `at` field (Section 8) and an
evaluation's `snapshot` (Section 11.7) — state what you were looking at, so
that what you concluded can be checked against it.

### 14.4 Retrieval Interface

```
  GET  /pub/v1/domain/{cid}    -> domain manifest
  GET  /pub/v1/domain/{cid}/log
                               -> generation log head and root
  GET  /pub/v1/domain/{cid}/consistency/{m}/{n}
                               -> consistency proof, generation m to n
  GET  /pub/v1/domain/{cid}/proof/{objcid}
                               -> inclusion or absence proof
  GET  /pub/v1/domain/{cid}/pack
                               -> the domain's objects as one stream
  GET  /pub/v1/domain/{cid}/generation/{n}
                               -> generation record n
  GET  /pub/v1/domain/{cid}/delta/{m}/{n}
                               -> generation records and objects, m -> n
  GET  /pub/v1/domain/{cid}/checkpoints
                               -> available checkpoint delta origins
  GET  /pub/v1/object/{cid}    -> canonical bytes, or 404   (query mode)
  GET  /pub/v1/related/{cid}   -> referencing CIDs          (query mode)
  POST /pub/v1/object          -> offer an object; 202 or 409
  POST /pub/v1/proposal        -> submit a proposal; 202
  GET  /pub/v1/domain/{cid}/relevant/{basis}
                               -> delta restricted to named lineages
```

A client MUST verify retrieved bytes against the requested CID before
parsing, and MUST verify a received pack against the domain manifest's
snapshot root before use.

### 14.5 Reverse Edges Across Domains

Within a replica, reverse edges are complete. Across domains they are not:
a dispute against a claim in one domain may be published into another.

Two mitigations, and the first is strong. A generation's membership tree
commits to exactly what the domain contains and the generation log commits
to its whole history, so both a divergent replica and a retroactive removal
are detectable — by root comparison and by consistency proof respectively
(Sections 14.1.1, 14.1.2). This is a large improvement over query mode,
where omission is undetectable by construction. Second, disputes SHOULD be
published into the domain of their target, and index nodes SHOULD publish
coverage commitments for cross-domain edges.

Cross-domain omission remains possible (Appendix B.1).

### 14.6 Discovery and Replication

Nodes discover each other through a DHT keyed by CID, static peer lists, or
out-of-band means. Implementations using a Kademlia-style DHT SHOULD apply
S/Kademlia-style hardening; eclipse is the practical route to suppressing
an object without any node refusing to serve it.

A mirror's declared set (Section 13.2) is expressed as a set of domain
CIDs. A node MAY garbage-collect objects outside it and MUST publish a
reduced set before ceasing service.

Nodes MUST NOT log retrieval requests against a requester identity beyond
operational necessity. This requirement is unverifiable by a requester, and
readers with a real threat model SHOULD rely on local replication rather
than on a node's declared behaviour.

### 14.7 Implementation Transparency

**Non-normative** (Section 1.1): a norm, not verifiable by any observer,
as stated below.

The software that reads, evaluates, and synchronizes domains SHOULD be
published as objects within the graph — source under an OSI-approved
licence, referenced by a `procedural` claim describing how to build and
run it, subject to the same review and lineage as any other content.

This is a norm and is not verifiable: no observer can confirm that a
participant ran the published code. It does not need to be verifiable.
Under R8, any result a reader doubts can be recomputed, and under R16 the
reader holds the inputs to recompute it with. Transparency of
implementation is useful here because it makes the computation
*understandable*, not because it makes it *trustworthy*; trustworthiness
comes from determinism and from holding the data.

## 15. Versioning and Extensibility

`pub` carries a major version. An implementation MUST reject an
unrecognized major version rather than partially interpret it.

Every hash and signature carries an in-band algorithm identifier. No
algorithm is hardcoded except as a mandatory-to-implement baseline. A
knowledge archive outlasts its primitives; the defences are **CID
migration** (a signed rehash attestation binding an object's identifier
under two hash algorithms), **signature re-attestation** by holders of
still-valid keys, and **timestamping** (Section 10.5), which is the only
mechanism that preserves authorship evidence for authors no longer
available to re-sign.

Implementations SHOULD sign with both a classical and a post-quantum
algorithm during transitions, and MUST treat a signature from a deprecated
algorithm as authentic only alongside a timestamp predating that
algorithm's deprecation.

`ext` carries deployment-specific fields keyed by reverse-DNS name. `ext`
contents are included in the CID and in signatures. An implementation
encountering an unrecognized `ext` key MUST ignore its contents and MUST
NOT treat the object as malformed. An extension MUST NOT change the meaning
of any field defined here.

---

## 16. Security Considerations

**Threat model.** Adversaries control unlimited keys, operate nodes, and
observe traffic; some nodes are hostile and some merely faulty; partitions
are common and indefinite. No honest majority is assumed anywhere. Sections
4 through 11 require none. Coercion of key holders is out of scope by L1.

**Sybil attacks.** Addressed structurally by R7: an unvouched key carries
zero weight under every viewpoint at every scale. Layers below evaluation
are deliberately Sybil-permissive — anyone may publish — because publishing
and being believed are separate concerns.

**Vouching capture.** Compromising or corrupting a key readers already
trust is not solvable in protocol. Mitigated by small deliberate root sets,
decay, revocation, and plural defaults. Clients SHOULD prompt periodic root
review.

**Suppression by omission.** Eliminated within a domain replica, where the
reader computes reverse edges completely (R16, Section 14.2). It persists
in two places: in query mode, where a node cannot prove it returned every
referencing object, and across domains, where a dispute may be published
where the target's holders will not look. Snapshot-root comparison makes
divergence between replicas detectable, so the attack is no longer
undetectable by construction. It remains the most effective attack
available against readers who do not hold the relevant domain, which is
most readers (Appendix B.1, B.15).

**Decontextualization.** A well-supported claim quoted without its scope
and counterpoints is a laundering vector. Mitigated by mandatory scope
(R4), mandatory dependency-closure availability, and `critique`. Not
eliminated.

**Attrition.** R10 bounds the weight an attritional disputant exerts but
not the attention they consume, since determining that grounds are not new
is itself work. Triage (Section 7.4) makes verifying a proposed finding
cheaper than determining one afresh, which lowers volume without changing
the ratio. Appendix B.2.

**Evaluation cost.** Annotation floods inflate evaluation cost for
everyone. Implementations SHOULD prune zero-weight keys before propagation,
SHOULD bound per-key annotation intake, and MUST bound `iterations`.

**Long-term cryptographic failure.** Every baseline algorithm will be
broken within an archive's intended lifetime. Timestamping is the
mitigation, must be applied at authoring time, and cannot be applied
retroactively.

**Canonicalization.** Serialization ambiguity yields two valid encodings
for one object, breaking equivalence reasoning and enabling signature
confusion. Hence Section 4.1's strictness, the float prohibition, and the
requirement to reject rather than re-canonicalize.

**Ledger risks.** Settlement inherits reorgs, transaction censorship,
congestion pricing, and seizure. Funded review also concentrates reviewer
attention on funded topics, which correlates with commercial interest
rather than importance. R13 removes the incentive to reject; it cannot
create an incentive to review the unfunded.

**Default policy capture.** If one default policy achieves overwhelming
client share, its root set becomes the de facto authority over what most
readers see, with no rule violated. This is the most likely way a
deployment fails and is a governance problem, not a protocol problem.

**Triage bias.** Section 7.4. A similarity-based novelty judge is disposed
against heterodoxy and reports high aggregate accuracy while being wrong on
the cases that matter.

---

## 17. Privacy Considerations

**Publication is permanent and attributed.** A key accumulates a
machine-readable record of every claim, judgement, and vouching it has
made — a richer profile of an intellectual life than most people create
deliberately. Implementations MUST state this in plain language before a
user's first publication and MUST NOT default to reusing one key across
unrelated subjects.

**Pseudonymity is the default and is real.** No attestation is required to
publish. Users SHOULD maintain separate keys for separate contexts; clients
SHOULD make multi-key operation ordinary rather than advanced, and SHOULD
warn on detectable cross-key correlation such as two keys publishing into
one narrow subject within short windows. Pseudonymity is defeated by
writing style, citation pattern, and timing long before it is defeated by
cryptography.

**Nullifier determinism.** Where personhood attestation is used, disclosure
of a `holder_secret` retroactively deanonymizes every pseudonym derived
from it, in every scope, for all time. L1 and L2 place compelled disclosure
out of scope; clients MUST nonetheless warn before a holder's first
presentation.

**Reading is private under R16 and exposed without it.** A reader holding
a domain replica issues no read request and transmits no policy: there is
nothing to log, correlate, or compel. This is the intended path and it
eliminates rather than mitigates the problem.

In query mode (Section 14.3) both exposures return in full. The serving
node learns exactly which proposition was requested, and content addressing
makes that unusually precise: a CID is globally canonical, the
CID-to-content mapping is public by construction (R12), and a claim is a
single proposition with no aggregation noise. A supplied policy CID
additionally discloses the reader's trust roots, thresholds, and
replication floor — a richer intellectual and political profile than any
individual read.

**Synchronization discloses domain-level interest.** Which domains a reader
follows is visible to whoever serves them, though not which objects within
a domain were read. Mitigations in Section 14.3.

**Most readers will be in query mode.** Thin clients on constrained devices
cannot hold a domain. The privacy properties above are real and are
unavailable to the majority of readers most of the time; implementations
MUST make the mode in use visible rather than allowing it to be assumed.

**Erasure.** Objects are designed not to be deletable and mirrors are
required to serve them (R12). Personal data published into the graph, by
its subject or by anyone else, cannot be reliably retracted; `retracts`
deliberately does not delete. Removal under L5 is recorded by tombstone
rather than concealed. Implementations MUST state this before first
publication.

---

## 18. IANA Considerations

Registries below operate under Specification Required.

| | Registry | Initial entries |
|---|---|---|
| 18.1 | Hash algorithms | `sha2-256` (required), `sha3-256`, `blake3` |
| 18.2 | Object types | `claim.prose`, `claim.relation`, `claim.annotation`, `doc`, `key`, `sig`, `revoke`, `policy`, `snapshot`, `eval`, `bounty`, `tombstone`, `anchor`, `domain`, `generation`, `proposal` |
| 18.3 | Claim classes | `formal`, `empirical`, `attributive`, `definitional`, `normative`, `expressive`, `archival`, `procedural`, `performative` |
| 18.4 | Relation kinds | `supersedes`, `translates`, `implements`, `depends`, `disputes`, `supports`, `equivalent`, `retracts`, `derived-from`, `delegates` |
| 18.5 | Annotation kinds | `assessment`, `verdict`, `settled`, `classifies`, `tagged`, `critique`, `usage`, `resolution`, `trusts`, `attests`, `affiliated`, `personhood`, `assumes-accountability`, `timestamped`, `triage`, `well-formed`, `witnessed` |
| 18.6 | Principal types | `human`, `organization`, `automated` |
| 18.7 | Reproducibility classes | `open`, `restricted`, `unique-event`, `destructive` |
| 18.8 | Signature algorithms | `ed25519` (required), `ml-dsa-65` (recommended) |
| 18.9 | URI scheme | `pub:` |

Additions to 18.3 MUST specify required evidence fields and permitted
annotation kinds.

---

## 19. References

### 19.1 Normative

- [RFC2119] Bradner, S., BCP 14, RFC 2119, March 1997.
- [RFC8174] Leiba, B., BCP 14, RFC 8174, May 2017.
- [RFC8949] Bormann, C. and P. Hoffman, "CBOR", STD 94, RFC 8949, 2020.
- [RFC6838] Freed, N., Klensin, J., and T. Hansen, "Media Type
  Specifications and Registration Procedures", BCP 13, RFC 6838, 2013.
- [RFC3339] Klyne, G. and C. Newman, RFC 3339, July 2002.
- [RFC5646] Phillips, A. and M. Davis, BCP 47, RFC 5646, September 2009.
- [UAX15] Unicode Consortium, "Unicode Normalization Forms", UAX #15.
- [FIPS204] NIST, "Module-Lattice-Based Digital Signature Standard", 2024.

### 19.2 Informative

- [RFC8785] Rundgren, A. et al., "JSON Canonicalization Scheme", RFC 8785.
- [RFC3161] Adams, C. et al., "Time-Stamp Protocol", RFC 3161, 2001.
- [EIGENTRUST] Kamvar, S. et al., WWW 2003.
- [PPR] Haveliwala, T., "Topic-Sensitive PageRank", WWW 2002.
- [SKAD] Baumgart, I. and S. Mies, "S/Kademlia", ICPADS 2007.
- [IPFS] Benet, J., "IPFS - Content Addressed, Versioned, P2P File System".
- [VC] W3C, "Verifiable Credentials Data Model".
- [PUB-DRIFT] "semantic drift", `pub:sha2-256:fbclzjiumgxa3tcg6wkosstckf3zntem5nlj43metjeuamflbbmq`,
  evidenced at `pub:sha2-256:ctewxqxz3r7wev6q45dk35tuiveu7im4w4ucwgogh2mzgugybonq`
  (the scientific/colloquial sense of "theory" in public debate over
  evolution), Publet Protocol corpus.
- [PUB-NSCAP] "namespace capture", `pub:sha2-256:6mkky5apmllactdltc7fwd375gbburafbeu7o4k53bmdfeqrqpja`,
  evidenced at `pub:sha2-256:h5f6gu7d4i7eabwoqzoztdgmf4sutpefrtkynzkvqbmcsrx3wixq`
  (Birsan 2021, dependency confusion), Publet Protocol corpus.
- [PUB-FORK] "split-brain forking", `pub:sha2-256:tqdxdknxbv6xfhco2euugvwiixwo674vyx5ezujbpcm3e2jtqufa`,
  evidenced at `pub:sha2-256:deqlmuqgnld5sm227mw7hjmoukp4zgn446xwai6h6vgvuxc3d42a`
  and `pub:sha2-256:wuydae7jndz5plqcfqtpcrw2cajjksoxbb2fqldoa4mpzdnurrqq`
  (the 2002 Spanish Wikipedia fork), Publet Protocol corpus.
- [PUB-TDESYNC] "translation desynchronization", `pub:sha2-256:7oxmte5jd6hjkecughukoulo5op7vd4rh2gmf7usv3n5hl7c35wq`,
  evidenced at `pub:sha2-256:glmmoyxtufg5kfl4npy6b6fyjonjxokaxmkx5btg64yjin62fxwq`
  (cross-lingual factual inconsistency across Wikipedia language editions),
  Publet Protocol corpus.

---

## Appendix A. Worked Example

**A.1** Keys `K_a`, `K_b`, `K_c` are published, all pseudonymous, all
declaring `principal: "human"`.

**A.2** `K_a` publishes `P1`:

```
  class:   "empirical"
  content: "In the 2031 cohort (n=4,182), daily supplementation with
            compound X reduced 12-month relapse incidence from 18.4%
            to 11.9%."
  scope:   { domain: "adults 40-65, single-centre, unblinded",
             temporal: { from: "2031-01", to: "2032-01" },
             precision: "95% CI 4.1-8.9 percentage points" }
  depends: [ P_def_relapse, P_def_compound_X ]
  evidence: [ { kind: "blob", ref: <dataset>, role: "measurement" },
              { kind: "claim", ref: M1, role: "method" } ]
  reproducibility: { class: "open", requirements: "..." }
```

Single-centre, unblinded, one age band, one year are inside the object the
CID addresses. `P1` cannot be cited without them.

**A.3** `K_b` publishes `P2` arguing the unblinded design cannot support the
stated effect size, plus `disputes(P2 → P1, aspect: "method")` and a
`verdict` of `deny`.

**A.4** `K_c` publishes `P3`, a French translation, plus `translates(P3 →
P1)`. `P3` binds to `P1`'s CID and can never desynchronize.

**A.5** `K_a` publishes `P4` restricting the effect to a subgroup, plus
`supersedes(P4 → P1)`, authoritative because `K_a` signed both. `P1` is
unmodified and remains retrievable. `P3` now translates a superseded claim;
clients say so without `K_c` acting.

**A.6** `K_d`, at an institution recorded by an `affiliated` annotation,
executes `M1` and files a `settled` annotation with outcome `inconsistent`.

**A.7** Reader **R1**'s policy roots trust in a methodology group with an
edge to `K_b`. `P1` evaluates to `rejected`, `superseded_by: [P4]`.

**A.8** Reader **R2**'s policy roots in a patient-advocacy organization
with an edge to `K_a` and none to `K_b`, so `K_b`'s verdict carries zero
weight. `P1` nonetheless does **not** evaluate to `accepted`: the
replication floor is unmet and one independent inconsistent reproduction
outweighs endorsement (R9). Result is `unreplicated`.

R2's trust choices moved the endorsement arithmetic and could not move the
evidence. That is the intended relationship between Sections 11.4 and 11.5.

**A.9** A funder posts a replication bounty on `P1`. Every qualifying
reproduction is paid, whatever it finds.

**A.10** An advocate asserts `N1` (`normative`): "the programme should be
funded", with `depends` naming `P4`, cost claim `P5`, counterfactual
claim `P6`, and `N0` (`normative`): "public funds should go to
interventions whose benefit exceeds their cost."

A reader rejecting `N1` while accepting `P4`, `P5`, `P6` has rejected `N0`,
which is now a named object rather than an unstated assumption. `N0` has no
verdict and never will.

---

## Appendix B. Open Problems

**B.1 Cross-domain reverse edges.** Solved within a domain replica (R16),
where the reader computes reverse edges completely. Across domains a
dispute may be published where the target's holders will not see it.
Snapshot-root comparison makes divergence between replicas detectable, so
this is no longer undetectable by construction, but it is not closed.

**B.2 Attrition.** R10 bounds an attritional disputant's weight but not the
attention they consume. A mechanism making redundancy-checking cheaper than
novelty-claiming would close this; none is known.

**B.3 Triage bias.** No measurement is specified that distinguishes "triage
is accurate" from "appeals are too expensive to file."

**B.4 Bootstrapping trust roots.** A new user has no basis for choosing
roots and will take the default. Everything this design achieves rests on a
choice most users will never make.

**B.5 Evaluation cost at scale.** Largely addressed by the domain bound
(R16): evaluation runs over one bounded replica rather than the whole
graph. Cross-domain evaluation — ranking results spanning many domains, or
computing trust paths through keys active in several — is unbounded again,
and the resolution of approximate-for-display against exact-for-settlement
(R8) is unspecified.

**B.6 Atomicity.** Structural tests catch compound assertions but say
nothing about granularity: "the rate was 11.9%" and "the rate fell from
18.4% to 11.9%" both pass and are not the same claim.

**B.7 Composition.** `critique` makes composition contestable but not
evaluable. There is no analogue of standing for "is this document's
selection of evidence fair," and building one may be harder than anything
else here.

**B.8 Replication floors.** R9 requires a floor and offers no basis for
choosing it. The right value differs by field, by method cost, and by how
much one reproduction decorrelates from the original.

**B.9 Scope inflation.** Authors have every incentive to state validity
conditions narrowly enough to be unfalsifiable and broadly enough to be
interesting, and nothing detects the manipulation.

**B.10 Machine-generated volume.** Nothing bounds the rate at which
automated systems produce well-formed, plausible, individually defensible
claims. Zero-weight keys are cheap to ignore at evaluation and not cheap
to store.

**B.11 Onboarding cost.** Decomposing an argument into classified, scoped,
dependency-declared claims is more work than writing prose, and the
benefit accrues to readers. No protocol rule fixes an incentive gradient
pointing the wrong way; tooling must (Section 7.4).

**B.12 Anchor concentration.** Observability is the only pressure against a
determined actor accumulating curatorial control across many keys.

**B.13 The assumed environment.** Sections 10 through 13 are conditional on
L1 through L6. No deployment environment currently provides them in full,
and this document specifies no fallback.

**B.14 Domain boundaries.** Nothing says where one domain ends and the
next begins, and the size bound forces splits without saying where to cut.
Anyone may publish a competing partition. Whether practice converges on
stable, roughly shared domains, or fragments into overlapping partitions
that defeat the completeness properties of Section 14.2, is an empirical
question only deployment answers.

**B.15 The thin-client majority.** R16's guarantees require holding a
domain. Most readers will not, so most reading will occur in the degraded
path (Section 14.3). A design whose central privacy property is available
mainly to well-resourced readers has a distributional problem that no
protocol rule addresses.

**B.16 Choosing the bound.** The size bound determines whether local-first
operation is achievable and how often domains split, and this document
offers no basis for setting it. Too large and the majority cannot
participate locally; too small and splitting fragments fields and inflates
cross-domain references, which are the cases R16 does not cover.

---

## Appendix C. Glossary

Every term of art this document uses, in one place. Where a definition
here and a section disagree, the section governs; this appendix is a
reader's aid, not a normative source.

### The object model

**Claim** — an immutable signed object asserting a proposition. The
protocol's primitive. Claims come in three **grammars** differing only in
how the proposition is expressed, and all three are assertions by an
author rather than structure (Section 1.4).

**Prose claim** (`claim.prose`) — a claim carrying its proposition as
text, with a class and a scope (Section 5).

**Relational claim** (`claim.relation`) — a claim whose proposition is a
predicate from a closed vocabulary applied to **two** objects, such as
`implements(A, B)`. Ordinarily shortened to *relation* (Section 6).

**Annotation claim** (`claim.annotation`) — the same grammar at arity
one: a predicate applied to a single object, carrying a kind-specific
payload. Ordinarily shortened to *annotation* (Section 7).

**Object** — any signed, content-addressed unit the protocol defines.
Every claim is an object; so are keys, policies, documents, domains, and
the rest (Section 18.2).

**Publet** — not an object type; a **role**. A published rendering of one
or more claims, small and single-topic, carrying a short citation tag
(Section 9.1) and meant to be stably cited by other work -- ordinarily a
`doc`, occasionally a single tagged claim. The thing other work cites; a
reference source in the ordinary sense.

**Short citation tag** — `<domain>-<name>-MM-YYYY`, a `tagged` annotation
naming a publet for citation. Self-declared, non-authoritative, and
resolved per viewpoint like any other name (Section 9.1); a colliding tag
is the ordinary case, never an error.

**Document** — a work citing sources and ordering references to them,
asserting nothing of its own. The citing side of the relationship a publet
is the cited side of (Section 8).

**Data** — the optional `data` field of a prose claim: a table of typed
columns and rows, or a file carried as a blob. The claim's `content` says
what the values are; `data` carries them (Section 5.8).

**Blob** — a byte string named by the CID of its bytes. Unsigned and not
an object; what it is, and how large, is stated by the object citing it,
and it is verified against its CID before use like anything else
(Section 4.7).

**Source** — an evidence entry with `role: "source"`, stating where values
in `data` were read from: a file at a commit, a warehouse table and query,
an exported blob, or another claim (Section 5.5). Provenance, not support.

**View** — the optional presentation a document item gives the data it
cites: a renderer name and its options. Presentational like a gloss, and
carrying no claims (Section 8).

**Rendering** — what a view produces when a client applies it: a table on
a page, a chart, a CSV download. Never an object and never a publet; it is
rebuilt from the cited data wherever it is shown (Sections 5.8, 8).

**Render pipeline** — a publet whose `procedural` claims state how a
document is turned into a rendering: the steps, the programs each runs, and
the versions it was stated against. A document names the pipeline it is
rendered by as a `background` item; the pipeline never names the documents
(Section 8).

**Draft** and **publish** — workspace notions, not protocol objects. A
draft is built and rendered locally and is never signed or distributed;
publishing is the deliberate act of signing, distributing, and pinning
what was accepted. The protocol only ever sees what is published, so a
corpus's history records decisions, not experiments.

**CID** — content identifier. The multihash of an object's canonical
serialization. A reference is always a CID, and retrieved bytes are
verified against it before use (Sections 4.2, R2).

**Canonical serialization** — the single byte representation an object is
permitted to have, so that one object has exactly one identifier
(Section 4.1).

**Body** — the type-specific payload of an object. **Header** — the fields
every object carries regardless of type (Section 4.3).

**Scope** — the conditions under which an author asserts a claim. Required
on every claim, whatever its grammar (R4, Section 5.3). `"unconditional"`
is the explicit way of saying there are none, which is distinguishable
from having said nothing.

**Basis** — an optional header field naming the state the author was
looking at when composing: a lineage head, a generation record, a
snapshot. Advisory and never gating (Section 4.6).

**Grounds** — what a judgement rests on. Required on an assessment,
because a judgement without stated grounds is a preference, and a reader
cannot weigh a preference.

### Claim classes

**Claim class** — what kind of claim something is, which determines what
evidence is required and which judgements are admissible (R5,
Section 5.2). A prose claim declares its class; a relation or annotation
takes the class its `kind` carries.

**`formal`** — a mathematical or logical statement, settled by proof
checking. **`empirical`** — a claim about the observable world, settled by
independent reproduction. **`procedural`** — an instruction or method,
settled by reproduction reports. **`attributive`** — "*X* said *Y*",
verified on the provenance of the attribution. **`archival`** — a
primary-source record, verified on provenance and custody.
**`definitional`** — a stipulated meaning, described by usage rather than
voted true. **`normative`** — an ought-claim. **`expressive`** — poetry,
fiction, testimony. **`performative`** — an act done by asserting it.

The last four are not truth-apt and admit no verdict.

**Felicity conditions** — what a performative claim has instead of
evidence: who is entitled to perform the act, and what must hold for it to
take effect. A retraction is not true or false; it either takes effect or
it does not. Stated per kind, because they differ — only an author may
withdraw their own assertion, while anyone at all may dispute it.

### Relations

**`supersedes`** — `from` replaces `to`; constitutes lineage.
**`retracts`** — the signer withdraws a prior assertion, without deleting
it. **`delegates`** — authorizes another key to continue a line of work.
These three are performative and felicitous only from a key that signed
the target.

**`implements`** — `from` realizes the contract `to` declares; several
implementations may exist side by side (Section 6.2). **`translates`** —
`from` renders `to` in another language, at a stated fidelity. Both are
empirical: whether they hold is settled by checking.

**`depends`** — `from` presupposes `to`, typically a definitional claim
fixing a term. **`equivalent`** — `from` and `to` express the same claim;
not transitive, and classes are computed per viewpoint. Both definitional.

**`derived-from`** — `from` copies or adapts `to`. Required for forks, and
a provenance claim. **`disputes`** / **`supports`** — argues against or
for; a dispute must name a claim stating grounds.

### Evaluation

**Viewpoint** — a trust policy, a set of trust roots, and a snapshot
boundary. Evaluating under one yields weights and standings (Section 11).

**Policy** — the object declaring how a viewpoint computes: its roots,
damping, thresholds, replication floor, and class rules (Section 11.1).

**Trust root** — a key a policy trusts directly, and the origin of all
weight under that viewpoint. A key no trust path reaches carries none.

**Standing** — the result of evaluating an assertion under a viewpoint.
**Not a truth value** (Section 11.4).

**Replication floor** — the minimum count of independent consistent
reproductions below which an empirical claim cannot reach `accepted`,
whatever the endorsement weight (R9).

**Independence** — whether two reproductions decorrelate: separate
funding, facilities, instruments, and keys. Two keys held by one person
are not two reproductions (Section 7.2, Section 10.2).

**Snapshot** — the set of objects an evaluation ran against, named by a
Merkle root, so a published result can be recomputed (Section 11.7).
**`eval`** — a published evaluation result, checkable by recomputation
(R8).

**Equivalence class** — the set of claims joined by trusted `equivalent`
relations, across which support and dispute aggregate while counting each
signer once (Section 11.6).

### Lineage and curation

**Lineage** — the set of objects reachable by `supersedes` edges from a
**genesis** object, named by that genesis CID. Computed, never declared,
and never itself an object (R15).

**Head** — the object a viewpoint selects as a lineage's current best
state. A branching lineage may have several, and selecting among them is
viewpoint-relative.

**Authoritative lineage** — the edges signed by a key that also signed the
target: an author's own revision history. The **full lineage** adds
third-party proposals to replace.

**Divergence** and **staleness** — two parties depending on definitions
with no common genesis have diverged and mean different things by a word;
two depending on different generations of one lineage are merely stale.
The remedies differ (Section 9.2).

**Anchor** — a maintained shortcut to the head of a lineage, for readers
wanting a curated answer rather than a computed one. Explicitly not
load-bearing (Section 9.3). **Steward** — a human principal maintaining
one.

### Identity

**Key** — an object declaring a public key and a principal type. Its CID
is the identity; there is no registry and no proof-of-personhood
requirement (Section 10.1).

**Principal** — `human`, `organization`, or `automated`. Authorship,
counted reproduction, and stewardship are reserved to human principals
(R11).

**Attestation** — a claim by one key about another: same person,
affiliated, credentialed, distinct. Carries evidence and method
(Section 10.2).

**Assumed accountability** — a key publicly declaring it stands behind
another key's output, staking its own standing, while disclosing nothing
about who holds the other (Section 10.7).

### Distribution

**Domain** — a bounded, content-addressed collection of objects
retrievable in full, closed under `depends` and within a declared size
bound. Reading and evaluation run against a local replica of one (R16,
Section 14.1).

**Generation** — one state of a domain. A **generation record** states the
membership change explicitly, listing what was added and removed and why
(Section 14.1.1).

**Membership tree** — a Merkle tree over a generation's member CIDs,
supporting inclusion and absence proofs. **Generation log** — an
append-only Merkle tree over successive snapshot roots, supporting
consistency proofs. Both are required, because neither does the other's
job.

**Consistency proof** — evidence that one log state is a prefix of
another, binding a publisher to a single history. **Witness** — an
annotation recording the log root a holder observed, so that a publisher
showing different histories to different readers is detectable.

**Delta synchronization** — advancing a replica by fetching only the
generation records and objects since the generation held. Self-verifying,
and discloses only which generation the reader had.

**Query mode** — retrieving individual objects from another party's node.
The degraded path: the serving node learns exactly which proposition was
requested (Section 14.3.2).

### Nodes

**Mirror** — stores and serves objects by CID within a **declared set**,
unconditionally and content-blind (R12). **Index** — computes and serves
resolutions under declared policies. **Evaluator** — computes and
publishes `eval` objects. **Archive** — a mirror committing to indefinite
retention and timestamping.

**Tombstone** — the published record required before a node ceases to
serve something in its declared set. Undisclosed removal remains a
violation (Section 13.2).

### Settlement

**Bounty** — an escrow funding review, replication, access, deduplication,
or mirroring, settling against a viewpoint named in advance. The **review
share** pays for filed work irrespective of finding; the **system share**
returns on resolution in either direction. Payment never gates access and
never affects standing (R13, Section 12).

### Rule shorthand

**R1**–**R16** — the sixteen operating rules of Section 2, which govern
wherever a later section appears to conflict with one.

**L1**–**L6** — the six assumed properties of the legal environment
(Section 1.2). Load-bearing: where a deployment operates outside a regime
providing them, the guarantees of Sections 10 through 13 degrade
accordingly.

---

## Authors' Addresses

David Schryer
Email: schryer@gmail.com

Claude (Anthropic), drafting collaborator
