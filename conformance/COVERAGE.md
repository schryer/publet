# Conformance coverage

Which normative statements of the specification the functional
suite exercises. Gaps are listed rather than hidden: a report
that showed only what passes would say nothing about what is
untested.

- specification: `/home/david/git/docs/publet-specification/index.md`
- sections with normative language: 44
- sections with at least one scenario: 23 (52%)

## Covered

| Section | Statements | Scenarios |
|---|---|---|
| 11.1 | 1 | conformance/viewpoint.feature |
| 11.2 | 3 | conformance/determinism.feature |
| 11.3 | 2 | porcelain/read.feature, porcelain/why.feature |
| 11.4 | 3 | conformance/evidence_dominance.feature, conformance/restricted_access.feature |
| 11.6 | 1 | conformance/equivalence.feature |
| 11.7 | 1 | conformance/reproducible_evaluation.feature |
| 11.8 | 2 | conformance/viewpoint.feature |
| 13.2 | 4 | plumbing/cat.feature, scenarios/store.feature |
| 14.1 | 3 | plumbing/merkle.feature |
| 14.1.1 | 6 | conformance/consistency.feature, conformance/generations.feature |
| 14.3.1 | 2 | conformance/consistency.feature |
| 14.3.2 | 5 | conformance/no_have_want.feature |
| 14.3.3 | 1 | scenarios/stale_basis.feature |
| 14.4 | 2 | scenarios/two_node_sync.feature |
| 4.1 | 1 | conformance/canonical_form.feature |
| 4.2 | 2 | plumbing/cid.feature, plumbing/verify.feature |
| 4.3 | 5 | plumbing/verify.feature |
| 4.4 | 2 | conformance/signatures.feature |
| 4.5 | 1 | conformance/object_size.feature |
| 5.1 | 4 | conformance/immutability.feature |
| 5.2 | 2 | conformance/class_rules.feature |
| 6.1 | 1 | plumbing/lineage.feature |
| 9.2 | 3 | plumbing/divergence.feature |

## Not yet covered

| Section | Statements |
|---|---|
| 1.3 | 1 |
| 10.1 | 2 |
| 10.3 | 9 |
| 10.5 | 1 |
| 10.7 | 4 |
| 12.1 | 1 |
| 12.2 | 1 |
| 12.3 | 1 |
| 12.5 | 1 |
| 13.3 | 1 |
| 13.4 | 3 |
| 14.6 | 2 |
| 14.7 | 11 |
| 5.4 | 2 |
| 5.5 | 2 |
| 5.7 | 7 |
| 7.1 | 2 |
| 7.2 | 1 |
| 7.3 | 2 |
| 7.4 | 9 |
| 9.3 | 1 |

## Tags naming sections with no normative language

- 11.5
- 14.2
- 14.3
- 5.3
- 6
