//! Assumed accountability and triage: bearing on the graph without
//! authority in it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Assumption, Basis, Graph, Triage};

fn cid(seed: &str) -> Cid {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256)
}

fn build(kind: &str, author: &Cid, fields: &[(&str, Value)]) -> (Cid, Vec<u8>) {
    let mut builder = Object::builder(kind, &author.to_string()).created("2026-09-12T10:00:00Z");
    for (k, v) in fields {
        builder = builder.field(k, v.clone());
    }
    let bytes = builder.build().unwrap();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

fn add(graph: &mut Graph, cid: &Cid, bytes: &[u8]) {
    let verified = Object::parse(bytes).unwrap().verify(cid).unwrap();
    graph.insert(cid, verified).unwrap();
}

fn assumption_object(assumer: &Cid, pseudonym: &Cid) -> (Cid, Vec<u8>) {
    let mut value = std::collections::BTreeMap::new();
    value.insert(
        "basis".to_owned(),
        Value::Text("work-reviewed-by-me".into()),
    );
    build(
        "ann",
        assumer,
        &[
            ("kind", Value::Text("assumes-accountability".into())),
            ("target", Value::Text(pseudonym.to_string())),
            ("value", Value::Map(value)),
        ],
    )
}

#[test]
fn an_assumption_is_not_authorship() {
    let assumer = cid("a named researcher");
    let pseudonym = cid("a pseudonymous key");
    let (id, bytes) = assumption_object(&assumer, &pseudonym);
    let verified = Object::parse(&bytes).unwrap().verify(&id).unwrap();

    let assumption = Assumption::from_object(verified.object()).unwrap();
    assert_eq!(assumption.assumer, assumer);
    assert_eq!(assumption.assumed, pseudonym);
    assert_eq!(assumption.basis, Basis::WorkReviewedByMe);

    // The record names two keys and a basis, and nothing else. There is no
    // field for who holds the assumed key, because writing the link down is
    // the thing the design refuses to do: a recorded link is a compellable,
    // permanently replicated artifact where none existed.
    let source = include_str!("../src/assumption.rs");
    let struct_body = source
        .split("pub struct Assumption {")
        .nth(1)
        .expect("the struct must be present")
        .split('}')
        .next()
        .expect("the struct must close");
    let fields: Vec<&str> = struct_body
        .lines()
        .filter_map(|l| l.trim().strip_suffix(','))
        .filter(|l| !l.starts_with("///"))
        .collect();
    assert_eq!(
        fields,
        vec!["pub assumer: Cid", "pub assumed: Cid", "pub basis: Basis"],
        "an assumption records two keys and a basis; anything more would be \
         the link the design refuses to write down"
    );
}

#[test]
fn assumptions_are_counted() {
    // Fronting and protecting a dissident are the same object one at a
    // time. What separates them is visible only in aggregate.
    let assumer = cid("an assumer");
    let mut graph = Graph::new();
    for n in 0..3 {
        let pseudonym = cid(&format!("pseudonym {n}"));
        let (id, bytes) = assumption_object(&assumer, &pseudonym);
        add(&mut graph, &id, &bytes);
    }
    assert_eq!(graph.assumptions_by(&assumer).len(), 3);
    assert!(graph.assumptions_by(&cid("someone else")).is_empty());
}

#[test]
fn triage_is_read_by_nothing_that_computes() {
    // Advisory by construction: the evaluation layer never reads a triage
    // annotation, so one cannot suppress, hide, or zero-weight anything.
    // Redundancy under R10 is set containment over grounds identifiers, a
    // mechanical test, never a model's opinion.
    let eval_source = include_str!("../../publet-eval/src/gather.rs");
    let standing_source = include_str!("../../publet-eval/src/standing.rs");
    for source in [eval_source, standing_source] {
        assert!(
            !source.contains("triage"),
            "the evaluation layer must not read triage annotations"
        );
    }
}

#[test]
fn a_triage_annotation_without_its_engine_is_not_read() {
    // An undisclosed machine judgement cannot be re-run, and one that
    // cannot be re-run is an opinion wearing the shape of evidence.
    let author = cid("an operator");
    let target = cid("a dispute");

    let mut bare = std::collections::BTreeMap::new();
    bare.insert("finding".to_owned(), Value::Text("redundant-with".into()));
    let (id, bytes) = build(
        "ann",
        &author,
        &[
            ("kind", Value::Text("triage".into())),
            ("target", Value::Text(target.to_string())),
            ("value", Value::Map(bare)),
        ],
    );
    let verified = Object::parse(&bytes).unwrap().verify(&id).unwrap();
    assert!(Triage::from_object(verified.object()).is_none());

    // With the disclosure block it reads.
    let mut engine = std::collections::BTreeMap::new();
    engine.insert("model".to_owned(), Value::Text("a-model".into()));
    engine.insert("version".to_owned(), Value::Text("2026-09".into()));
    let mut value = std::collections::BTreeMap::new();
    value.insert("finding".to_owned(), Value::Text("redundant-with".into()));
    value.insert("engine".to_owned(), Value::Map(engine));
    let (id, bytes) = build(
        "ann",
        &author,
        &[
            ("kind", Value::Text("triage".into())),
            ("target", Value::Text(target.to_string())),
            ("value", Value::Map(value)),
        ],
    );
    let verified = Object::parse(&bytes).unwrap().verify(&id).unwrap();
    let triage = Triage::from_object(verified.object()).unwrap();
    assert_eq!(triage.engine, "a-model 2026-09");
}
