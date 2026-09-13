//! Graph behaviour over a fixture reproducing Appendix A of the
//! specification, plus the rules Section 5.2 and Section 6 require.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Class, Graph, GraphError, Lineage, RelationKind, compare};

/// Build an object and return it with its identifier.
fn build(kind: &str, author: &str, created: &str, fields: &[(&str, Value)]) -> (Cid, Vec<u8>) {
    let mut builder = Object::builder(kind, author).created(created);
    for (k, v) in fields {
        builder = builder.field(k, v.clone());
    }
    let bytes = builder.build().expect("object builds");
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

fn author(seed: &str) -> String {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256).to_string()
}

fn add(graph: &mut Graph, cid: &Cid, bytes: &[u8]) -> Result<(), GraphError> {
    let verified = Object::parse(bytes).unwrap().verify(cid).unwrap();
    graph.insert(cid, verified)
}

fn publet(
    author_key: &str,
    created: &str,
    class: &str,
    content: &str,
    depends: &[&Cid],
) -> (Cid, Vec<u8>) {
    let deps = Value::Array(depends.iter().map(|c| Value::Text(c.to_string())).collect());
    let mut fields = vec![
        ("class", Value::Text(class.into())),
        ("lang", Value::Text("en".into())),
        ("content", Value::Text(content.into())),
        ("depends", deps),
    ];
    // Section 5.5: an empirical claim must name a method others can execute.
    if class == "empirical" {
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text("publet".into()));
        entry.insert("role".to_owned(), Value::Text("method".into()));
        entry.insert(
            "ref".to_owned(),
            Value::Text(Cid::of(b"a method publet", HashAlg::Sha2_256).to_string()),
        );
        fields.push(("evidence", Value::Array(vec![Value::Map(entry)])));
    }
    build("publet", author_key, created, &fields)
}

fn relation(author_key: &str, created: &str, kind: &str, from: &Cid, to: &Cid) -> (Cid, Vec<u8>) {
    build(
        "rel",
        author_key,
        created,
        &[
            ("kind", Value::Text(kind.into())),
            ("from", Value::Text(from.to_string())),
            ("to", Value::Text(to.to_string())),
        ],
    )
}

fn verdict(author_key: &str, target: &Cid, aspect: Option<&str>) -> (Cid, Vec<u8>) {
    let mut fields = vec![
        ("kind", Value::Text("verdict".into())),
        ("target", Value::Text(target.to_string())),
    ];
    if let Some(a) = aspect {
        fields.push(("aspect", Value::Text(a.into())));
    }
    build("ann", author_key, "2026-09-12T12:00:00Z", &fields)
}

#[test]
fn appendix_a_lineage_and_translation() {
    let ka = author("K_a");
    let kc = author("K_c");
    let mut g = Graph::new();

    // A.2: K_a publishes P1.
    let (p1, p1b) = publet(
        &ka,
        "2026-09-12T10:00:00Z",
        "empirical",
        "relapse fell",
        &[],
    );
    add(&mut g, &p1, &p1b).unwrap();

    // A.4: K_c translates it.
    let (p3, p3b) = publet(&kc, "2026-09-12T10:05:00Z", "empirical", "rechute", &[]);
    add(&mut g, &p3, &p3b).unwrap();
    let (tr, trb) = relation(&kc, "2026-09-12T10:05:01Z", "translates", &p3, &p1);
    add(&mut g, &tr, &trb).unwrap();

    // A.5: K_a supersedes P1 with P4, authoritative because K_a signed both.
    let (p4, p4b) = publet(
        &ka,
        "2026-09-12T11:00:00Z",
        "empirical",
        "subgroup only",
        &[],
    );
    add(&mut g, &p4, &p4b).unwrap();
    let (sup, supb) = relation(&ka, "2026-09-12T11:00:01Z", "supersedes", &p4, &p1);
    add(&mut g, &sup, &supb).unwrap();

    let lineage = g.lineage(&p1, Lineage::Authoritative);
    assert_eq!(lineage.genesis, p1, "P1 is the genesis");
    assert_eq!(lineage.members, vec![p1.clone(), p4.clone()]);
    assert_eq!(lineage.heads, vec![p4.clone()]);
    assert!(!lineage.is_branched());

    // The translation still points at the exact state it was authored
    // against; staleness is computed, not stored.
    assert_eq!(g.out(RelationKind::Translates, &p3), vec![p1.to_string()]);
    assert_eq!(
        g.incoming(RelationKind::Translates, &p1),
        vec![p3.to_string()]
    );
}

#[test]
fn third_party_supersession_is_not_authoritative() {
    let ka = author("K_a");
    let kb = author("K_b");
    let mut g = Graph::new();

    let (p1, p1b) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "original", &[]);
    add(&mut g, &p1, &p1b).unwrap();
    let (p2, p2b) = publet(&kb, "2026-09-12T10:30:00Z", "empirical", "proposal", &[]);
    add(&mut g, &p2, &p2b).unwrap();
    // K_b proposes to replace K_a's publet: a proposal, not a revision.
    let (r, rb) = relation(&kb, "2026-09-12T10:30:01Z", "supersedes", &p2, &p1);
    add(&mut g, &r, &rb).unwrap();

    let authoritative = g.lineage(&p1, Lineage::Authoritative);
    assert_eq!(
        authoritative.members,
        vec![p1.clone()],
        "a third party's proposal is not the author's revision history"
    );

    let full = g.lineage(&p1, Lineage::Full);
    assert_eq!(full.members, vec![p1.clone(), p2.clone()]);
}

#[test]
fn branching_lineage_reports_several_heads() {
    let ka = author("K_a");
    let mut g = Graph::new();
    let (p1, p1b) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "base", &[]);
    add(&mut g, &p1, &p1b).unwrap();
    for (n, when) in [("a", "2026-09-12T11:00:00Z"), ("b", "2026-09-12T11:30:00Z")] {
        let (p, pb) = publet(&ka, when, "empirical", n, &[]);
        add(&mut g, &p, &pb).unwrap();
        let (r, rb) = relation(&ka, when, "supersedes", &p, &p1);
        add(&mut g, &r, &rb).unwrap();
    }
    let lineage = g.lineage(&p1, Lineage::Authoritative);
    assert_eq!(lineage.heads.len(), 2, "both successors are heads");
    assert!(lineage.is_branched());
}

#[test]
fn acyclic_kinds_reject_cycle_closing_edges() {
    for kind in ["supersedes", "depends", "derived-from"] {
        let ka = author("K_a");
        let mut g = Graph::new();
        let (a, ab) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "a", &[]);
        let (b, bb) = publet(&ka, "2026-09-12T10:00:01Z", "empirical", "b", &[]);
        add(&mut g, &a, &ab).unwrap();
        add(&mut g, &b, &bb).unwrap();

        let (r1, r1b) = relation(&ka, "2026-09-12T10:01:00Z", kind, &a, &b);
        add(&mut g, &r1, &r1b).unwrap();

        let (r2, r2b) = relation(&ka, "2026-09-12T10:02:00Z", kind, &b, &a);
        let err = add(&mut g, &r2, &r2b).unwrap_err();
        assert!(
            matches!(err, GraphError::CycleClosing { .. }),
            "{kind} must reject a cycle, got {err:?}"
        );
    }
}

#[test]
fn cyclic_kinds_permit_cycles() {
    // Section 6: mutual dispute is ordinary disagreement, not an error.
    let ka = author("K_a");
    let mut g = Graph::new();
    let (a, ab) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "a", &[]);
    let (b, bb) = publet(&ka, "2026-09-12T10:00:01Z", "empirical", "b", &[]);
    add(&mut g, &a, &ab).unwrap();
    add(&mut g, &b, &bb).unwrap();

    for (from, to, when) in [(&a, &b, "10:01:00"), (&b, &a, "10:02:00")] {
        let (r, rb) = relation(&ka, &format!("2026-09-12T{when}Z"), "disputes", from, to);
        add(&mut g, &r, &rb).expect("mutual dispute is permitted");
    }
    assert_eq!(g.incoming(RelationKind::Disputes, &a), vec![b.to_string()]);
}

#[test]
fn self_edges_of_acyclic_kinds_are_rejected() {
    let ka = author("K_a");
    let mut g = Graph::new();
    let (a, ab) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "a", &[]);
    add(&mut g, &a, &ab).unwrap();
    let (r, rb) = relation(&ka, "2026-09-12T10:01:00Z", "supersedes", &a, &a);
    assert!(matches!(
        add(&mut g, &r, &rb).unwrap_err(),
        GraphError::CycleClosing { .. }
    ));
}

#[test]
fn verdicts_are_refused_on_classes_that_are_not_truth_apt() {
    for class in ["definitional", "normative", "expressive"] {
        let ka = author("K_a");
        let mut g = Graph::new();
        let (p, pb) = publet(&ka, "2026-09-12T10:00:00Z", class, "term: meaning", &[]);
        add(&mut g, &p, &pb).unwrap();
        let (v, vb) = verdict(&ka, &p, None);
        let err = add(&mut g, &v, &vb).unwrap_err();
        assert!(
            matches!(err, GraphError::VerdictOnNonTruthApt { .. }),
            "{class} must refuse a verdict, got {err:?}"
        );
    }
}

#[test]
fn verdicts_on_provenance_classes_must_name_provenance() {
    for class in ["attributive", "archival"] {
        let ka = author("K_a");
        let mut g = Graph::new();
        let (p, pb) = publet(&ka, "2026-09-12T10:00:00Z", class, "X said Y", &[]);
        add(&mut g, &p, &pb).unwrap();

        let (bad, badb) = verdict(&ka, &p, Some("effect-size"));
        assert!(matches!(
            add(&mut g, &bad, &badb).unwrap_err(),
            GraphError::VerdictAspectNotProvenance { .. }
        ));

        let (good, goodb) = verdict(&ka, &p, Some("provenance"));
        add(&mut g, &good, &goodb).expect("provenance verdicts are permitted");
    }
}

#[test]
fn verdicts_are_permitted_on_truth_apt_classes() {
    for class in ["formal", "empirical", "procedural"] {
        let ka = author("K_a");
        let mut g = Graph::new();
        let (p, pb) = publet(&ka, "2026-09-12T10:00:00Z", class, "a claim", &[]);
        add(&mut g, &p, &pb).unwrap();
        let (v, vb) = verdict(&ka, &p, None);
        add(&mut g, &v, &vb).expect("truth-apt classes accept verdicts");
    }
}

#[test]
fn dependency_closure_is_transitive() {
    let ka = author("K_a");
    let mut g = Graph::new();
    let (d1, d1b) = publet(
        &ka,
        "2026-09-12T09:00:00Z",
        "definitional",
        "relapse: a return",
        &[],
    );
    add(&mut g, &d1, &d1b).unwrap();
    let (d2, d2b) = publet(
        &ka,
        "2026-09-12T09:01:00Z",
        "definitional",
        "return: coming back",
        &[&d1],
    );
    add(&mut g, &d2, &d2b).unwrap();
    let (p, pb) = publet(
        &ka,
        "2026-09-12T10:00:00Z",
        "empirical",
        "rates fell",
        &[&d2],
    );
    add(&mut g, &p, &pb).unwrap();

    let mut closure = g.depends_closure(&p).unwrap();
    closure.sort_by_key(ToString::to_string);
    let mut expected = vec![d1, d2];
    expected.sort_by_key(ToString::to_string);
    assert_eq!(closure, expected);
}

#[test]
fn divergence_and_staleness_are_reported_separately() {
    let ka = author("K_a");
    let kb = author("K_b");
    let mut g = Graph::new();
    let always = |_: &Cid| true;

    // One lineage: "heritability" defined, then superseded.
    let (h1, h1b) = publet(
        &ka,
        "2026-09-12T08:00:00Z",
        "definitional",
        "heritability: proportion of variance attributable to genotype",
        &[],
    );
    add(&mut g, &h1, &h1b).unwrap();
    let (h2, h2b) = publet(
        &ka,
        "2026-09-12T08:30:00Z",
        "definitional",
        "heritability: narrow-sense proportion of additive genetic variance",
        &[],
    );
    add(&mut g, &h2, &h2b).unwrap();
    let (hs, hsb) = relation(&ka, "2026-09-12T08:30:01Z", "supersedes", &h2, &h1);
    add(&mut g, &hs, &hsb).unwrap();

    // Two unrelated lineages for "theory".
    let (t1, t1b) = publet(
        &ka,
        "2026-09-12T08:00:00Z",
        "definitional",
        "theory (mathematics): a set of sentences closed under entailment",
        &[],
    );
    add(&mut g, &t1, &t1b).unwrap();
    let (t2, t2b) = publet(
        &kb,
        "2026-09-12T08:00:00Z",
        "definitional",
        "theory (ordinary speech): a conjecture",
        &[],
    );
    add(&mut g, &t2, &t2b).unwrap();

    // Two claims: one on the old heritability generation and mathematics'
    // theory, the other on the new generation and ordinary speech's theory.
    let (left, leftb) = publet(
        &ka,
        "2026-09-12T10:00:00Z",
        "empirical",
        "claim L",
        &[&h1, &t1],
    );
    add(&mut g, &left, &leftb).unwrap();
    let (right, rightb) = publet(
        &kb,
        "2026-09-12T10:00:00Z",
        "empirical",
        "claim R",
        &[&h2, &t2],
    );
    add(&mut g, &right, &rightb).unwrap();

    let result = compare(&g, &left, &right, &always).unwrap();

    let stale: Vec<&str> = result.stale.iter().map(|c| c.term.as_str()).collect();
    let divergent: Vec<&str> = result.divergent.iter().map(|c| c.term.as_str()).collect();

    assert_eq!(
        stale,
        vec!["heritability"],
        "same genesis, different generation: staleness, not divergence"
    );
    assert_eq!(
        divergent,
        vec!["theory"],
        "no common genesis: two concepts sharing a word"
    );
}

#[test]
fn a_trusted_equivalence_removes_a_conflict() {
    let ka = author("K_a");
    let mut g = Graph::new();
    let (d1, d1b) = publet(
        &ka,
        "2026-09-12T08:00:00Z",
        "definitional",
        "x: one meaning",
        &[],
    );
    let (d2, d2b) = publet(
        &ka,
        "2026-09-12T08:00:01Z",
        "definitional",
        "x: the same thing",
        &[],
    );
    add(&mut g, &d1, &d1b).unwrap();
    add(&mut g, &d2, &d2b).unwrap();
    let (l, lb) = publet(&ka, "2026-09-12T10:00:00Z", "empirical", "L", &[&d1]);
    let (r, rb) = publet(&ka, "2026-09-12T10:00:01Z", "empirical", "R", &[&d2]);
    add(&mut g, &l, &lb).unwrap();
    add(&mut g, &r, &rb).unwrap();

    let none = |_: &Cid| false;
    assert_eq!(compare(&g, &l, &r, &none).unwrap().divergent.len(), 1);

    let (eq, eqb) = relation(&ka, "2026-09-12T10:05:00Z", "equivalent", &d1, &d2);
    add(&mut g, &eq, &eqb).unwrap();

    let all = |_: &Cid| true;
    assert!(
        compare(&g, &l, &r, &all).unwrap().is_empty(),
        "a trusted equivalence means the parties are not disagreeing"
    );
    assert_eq!(
        compare(&g, &l, &r, &none).unwrap().divergent.len(),
        1,
        "and an untrusted one does not"
    );
}

#[test]
fn class_rules_survive_out_of_order_insertion() {
    // The annotation arrives before its target, so the rule cannot be
    // checked on insert; `validate` catches it once the graph is loaded.
    let ka = author("K_a");
    let mut g = Graph::new();
    let (p, pb) = publet(&ka, "2026-09-12T10:00:00Z", "normative", "should fund", &[]);
    let (v, vb) = verdict(&ka, &p, None);
    add(&mut g, &v, &vb).expect("target not yet present");
    add(&mut g, &p, &pb).unwrap();
    assert!(matches!(
        g.validate().unwrap_err(),
        GraphError::VerdictOnNonTruthApt { .. }
    ));
}

#[test]
fn class_identifiers_round_trip() {
    for id in [
        "formal",
        "empirical",
        "attributive",
        "definitional",
        "normative",
        "expressive",
        "archival",
        "procedural",
    ] {
        assert_eq!(Class::from_id(id).unwrap().id(), id);
    }
    assert!(Class::from_id("invented").is_none());
}
