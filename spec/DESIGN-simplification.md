# Generalization and simplification — a scan of the spec

Not spec text. A read of all 2125 lines against the criterion the document
sets for itself in Section 2:

> The protocol is these sixteen rules applied uniformly to every object
> type. Later sections specify formats and algorithms; **they introduce no
> new principles.** Where a section appears to conflict with a rule, the
> rule governs.

That is a strong, falsifiable test, and it is the one used throughout
below. Anything carrying a principle not derivable from the sixteen rules
is either a rule in disguise or a redundancy. Findings are ordered by what
they cost to fix against what they return, with the two that bear directly
on building publets first.

---

## 1. `doc` is already the thing `publet` is reserved for

**This is the finding that matters most for the stated goal.**

Section 1.4 now reserves *publet* for "a published rendering of a claim:
the layout and presentation that helps others check and verify it."
Section 8 already defines an object that does exactly that:

> A document contains no assertions of its own. `gloss` is presentational
> connective tissue and MUST NOT carry claims […] `role` makes a
> document's *use* of a claim explicit.

A `doc` is a rendering of claims, carrying presentation (`title`,
`sections`, `heading`, `gloss`) and stance (`role`) but no assertions of
its own. That is the reserved definition of a publet, in the spec, under
another name, since before the rename.

Three ways to resolve it, and the choice shapes everything built next:

- **`publet` names `doc`.** One object type, renamed to the word the
  protocol is named after. Cheapest, and it makes the protocol's own noun
  refer to something real again.
- **A publet is the single-claim case of a `doc`.** Nothing new is needed
  — a document with one section holding one item already is one. "Publet"
  becomes a usage convention rather than a type.
- **A publet is a *view specification* over a claim, distinct from a
  document.** A `doc` says *what to include and in what order*; a publet
  would say *how to render one claim so it can be checked* — which
  evidence to surface, which annotations to show, what to put beside it.
  This is the only option that adds a type, and the only one that
  captures "for the purpose of helping it be checked."

The third is the most interesting and the most work. The second is free.
Either way, **nothing should be built called a publet until this is
settled**, because option three is a different object from option two and
they are not mutually convertible after the fact.

---

## 2. R4 and R5 are not applied uniformly, and fixing that subsumes
   several hand-written special cases

The strongest structural finding, and the rename is what exposed it.

> **R4** Declared scope. **Every assertion** states the conditions under
> which its author asserts it.
>
> **R5** Declared class. **Every assertion** states its claim class. The
> class determines what evidence is required and **which judgements are
> admissible**.

All three grammars are assertions. Only one of them obeys:

| | `class` | `scope` |
|---|---|---|
| `claim.prose` | REQUIRED | REQUIRED |
| `claim.relation` | absent | absent |
| `claim.annotation` | absent | absent |

Section 2 says the rules are "applied uniformly to every object type."
Two-thirds of the claim types do not apply R4 or R5 at all. Under the
document's own governing clause — "where a section appears to conflict
with a rule, the rule governs" — this is a defect in Sections 6 and 7, not
a deliberate exemption.

What closing it buys, beyond consistency:

- **The performative/constative table in Section 6 becomes unnecessary.**
  That table exists to say which relations admit a judgement — which is
  *precisely* what R5 says a class already determines. Give relations a
  class and Section 5.2's existing rule ("MUST reject a verdict
  annotation targeting a `definitional`, `normative`, or `expressive`
  claim") covers performatives with no new rule, no new table, and no new
  enforcement path. The table was a special case of a general rule the
  document already had. *(Written into Section 6 recently, by me — it
  should be the first thing this generalization deletes.)*
- **`implements` gets a scope.** "This implementation satisfies that
  contract" is asserted flatly today, with nowhere to say *on aarch64,
  with this toolchain*. Conformance claims are exactly the kind that hold
  under conditions. This is the open gap in `DESIGN-code-claims.md`, and
  it closes for free.
- **Policy `class_rules` start applying to relations.** Section 11.1
  already carries `class_rules: { <claim-class>: <rule> }`. The machinery
  to evaluate relations by class is built and simply has nothing to read.

**The cost is real and should not be minimised.** Every relation and
annotation gains two required fields, so every object's bytes change —
another full CID migration of the kind just completed. And some kinds sit
awkwardly: `depends` is a structural presupposition rather than a claim
about the world, and forcing a class on it may be dishonest. A narrower
version worth considering: require `scope` everywhere (R4 is
unambiguous), and give relations a truth-aptness marker rather than a
full class, since admissible judgement is the only thing Section 6
actually needs from R5.

---

## 3. "State what you were looking at" is one concept implemented four
   times

The document notices this itself, at the end of Section 14.3.3:

> This is the same pattern as a document item's `at` field (Section 8) and
> an evaluation's `snapshot` (Section 11.7) — state what you were looking
> at, so that what you concluded can be checked against it.

Having named the pattern, it implements it four separate ways:

| Where | Field | Names |
|---|---|---|
| `doc` item (§8) | `at` | the lineage head the author read |
| `proposal` (§14.3.3) | `basis` | the generation record read |
| `eval` (§11.7) | `snapshot` | the snapshot evaluated against |
| `snapshot` (§11.7) | `as_of` | the instant the set was taken |

All four are one CID naming the state the author was looking at. A single
optional header field — alongside `prev`, which is already a header-level
"what came before" — would subsume all four and apply automatically to
anything added later. A reproduction run against a replica, an assessment
of a branching lineage, and a translation of a claim that has since been
superseded all want this field and none of them has it.

This is the cleanest win in the document: four names to one, no semantics
lost, no migration required for objects that do not use it.

---

## 4. `proof-checked` and `reproduction` are one mechanism under R9

> **R9** Evidence dominance. Proof settles formal claims; independent
> reproduction settles empirical claims.

One rule, two annotation kinds, structurally parallel and identical in
role — an independent party executed the procedure that settles this
claim's class, and reports what happened:

```
  proof-checked  { system, version, artifact, result }
  reproduction   { method, outcome, result, data, independence }
```

Both are decisive under Section 11.4. Both are re-runnable by a third
party. They differ only in payload, because the classes they serve differ.

A single `evidence` annotation whose required payload is determined by the
target's class would make R9 one mechanism instead of two, and would
extend automatically to any class that later acquires a settling
procedure. It also dissolves a question raised in
`DESIGN-code-claims.md` — whether `proof-checked` needs `config` and
`reference` fields — by making the payload class-parameterized rather than
fixed.

Against it: the payloads genuinely differ, and one kind with a
class-dependent shape is harder to validate than two kinds with fixed
shapes. This is a real trade and not obviously worth making. It is listed
because the *role* is identical and R9 already treats them as one rule;
whether the encoding should follow is a judgement call.

---

## 5. Three layers the document already marks as skippable

The spec says of its own sections:

- **§9.3 Anchors** — "Anchors are not load-bearing. Nothing in Sections 4
  through 8 consults one, and an implementation ignoring this section
  entirely is conformant."
- **§12 Settlement** — "an implementation MAY omit this section entirely
  and remain conformant for all other purposes." Also: "Participation is
  optional."
- **§14.7 Implementation Transparency** — "This is a norm and is not
  verifiable."

Together roughly 150 lines of a 2125-line document, self-declared as
optional. Moving them to a clearly separated non-normative part (or
companion documents) would sharpen what conformance actually requires
without removing anything. For someone implementing enough of the protocol
to render and check a claim, all three are noise on the path.

This is editorial rather than semantic, and correspondingly cheap.

---

## 6. Section 7.1's seventeen annotation kinds have an undeclared
   structure

`assessment`, `verdict`, `proof-checked`, `reproduction`, `classifies`,
`critique`, `usage`, `resolution`, `trusts`, `attests`, `affiliated`,
`personhood`, `assumes-accountability`, `timestamped`, `triage`,
`well-formed`, `witnessed` — presented as one flat table, though they fall
into clear groups:

| Group | Kinds | About |
|---|---|---|
| Judgement | `assessment`, `verdict` | whether a claim holds |
| Evidence | `proof-checked`, `reproduction` | what executing the settling procedure showed |
| Classification | `classifies`, `usage` | what a claim is about, how a term is used |
| Composition | `critique` | how a document selects |
| Process | `resolution`, `triage`, `well-formed` | advisory or thread-settling |
| Identity | `trusts`, `attests`, `affiliated`, `personhood`, `assumes-accountability` | about **keys**, not claims |
| Infrastructure | `timestamped`, `witnessed` | about objects and domains as artifacts |

Stating the grouping costs nothing and changes no semantics. It also
surfaces two things worth a second look: the identity group targets keys
rather than claims, which is a different relation to its target than every
other group has; and `assessment` versus `verdict` is the one pair whose
distinction is not obvious from the table (one is "a signer's judgement"
with a `basis`, the other is "input to evaluation policies" with a coarser
vocabulary — plausibly one kind at two granularities).

---

## 7. Examined and found justified

Reporting these because "I checked and it is not redundant" is useful
signal, and because each looks like duplication until the reasoning is
read:

- **`implements` vs `equivalent` vs `translates` vs `derived-from`.**
  Section 6.2 already distinguishes all four explicitly and correctly:
  conformance to a contract, sameness of claim, graded rendering of one
  claim, and provenance of a copy. Four distinct relations, not one
  over-split.
- **Two Merkle structures (§14.1.1).** "A sorted tree alone cannot yield
  efficient consistency proofs […] an append-only log alone cannot yield
  absence proofs." Minimal for the properties required, not redundant.
- **`bind: "object" | "lineage"` (§8).** Reduces to neither. "A textbook
  citing a physical constant wants a lineage; a paper citing a specific
  cohort result wants an object."

---

## 8. Scope note for the stated goal

Nothing in Section 14 — 347 lines, 16% of the document, and the subject of
four of the sixteen open problems (B.5, B.14, B.15, B.16) — is required to
build a publet or to check a claim. Domains, generations, consistency
proofs, delta synchronization, and the retrieval interface are about
*distributing* a corpus privately at scale. A reader holding a claim's
bytes can verify its CID, read its class and scope, follow its `depends`
closure, and evaluate its annotations under a policy with none of it.

The shortest path from here to "actual publets based on truthful
verifiable claims" runs through Sections 4, 5, 6, 7, and 11, and stops.
That is worth knowing before the largest and least-settled section of the
document is treated as a prerequisite.

---

## Suggested order

1. **Settle finding 1** (what a publet *is*, relative to `doc`). Blocks
   building anything; costs a decision, not code.
2. **Take finding 3** (one "observed state" field). Cleanest win, no
   migration, no semantics lost.
3. **Take finding 5 and 6** (separate the optional layers, group the
   annotation kinds). Editorial, cheap, makes the rest legible.
4. **Decide finding 2** (R4/R5 uniformity). The big one. Deletes the
   Section 6 group table, gives `implements` a scope, activates
   `class_rules` for relations — at the cost of a full CID migration and
   some genuinely awkward cases. Worth doing, worth doing deliberately.
5. **Leave finding 4** (unifying the evidence annotations) until finding 2
   settles, since a class-parameterized payload depends on relations
   having a class.
