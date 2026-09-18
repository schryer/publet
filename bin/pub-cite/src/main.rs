//! Check that every citation in a document resolves (Section 8).
//!
//! Section 8: "A client rendering a lineage-bound item MUST display the
//! resolved object, MUST make `at` reachable, and MUST indicate when the
//! two differ." This walks every document's items and reports what a
//! renderer would need to know to meet that: whether `ref` exists and
//! names a claim, and -- for a lineage-bound item -- whether `at` is
//! reachable in that lineage and whether it still names the current head.
//!
//! Divergence (an `at` that is no longer the head) is reported, not
//! failed: `bind: lineage` exists precisely because the current head can
//! move past what the author read, and disclosing that is the point, not
//! a problem with the citation. `bind: object`'s citation is permanently
//! fixed (the default), so nothing about currency applies to it; only
//! whether the reference resolves to a claim is checked.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_graph::{Bind, Document, Graph, Item, Lineage, load};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_LOAD: u8 = 4;

fn usage() -> ExitCode {
    eprintln!("usage: pub-cite [--dir=DIR] [CID...]");
    eprintln!("  with no CIDs, checks every document in DIR");
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut wanted: Vec<Cid> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            return usage();
        } else {
            let Ok(cid) = arg.parse::<Cid>() else {
                eprintln!("not a CID: {arg}");
                return ExitCode::from(EXIT_USAGE);
            };
            wanted.push(cid);
        }
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_LOAD);
        }
    };

    let docs: Vec<&Document> = if wanted.is_empty() {
        graph.documents().collect()
    } else {
        let mut found = Vec::new();
        for cid in &wanted {
            let Some(doc) = graph.document(cid) else {
                eprintln!("not a document in {}: {cid}", dir.display());
                return ExitCode::from(EXIT_LOAD);
            };
            found.push(doc);
        }
        found
    };

    let mut violations = 0u32;
    for doc in docs {
        for item in doc.items() {
            violations += u32::from(!check_item(&graph, doc, item));
        }
    }

    if violations > 0 {
        eprintln!("{violations} citation(s) failed");
        return ExitCode::from(EXIT_VIOLATION);
    }
    ExitCode::SUCCESS
}

/// Check one item, printing a JSON line describing the result.
///
/// Returns whether it passed -- `false` covers everything a renderer
/// cannot proceed on (missing reference, wrong kind, unreachable `at`);
/// divergence is reported but returns `true`, per Section 8's own
/// disclose-don't-forbid treatment of it.
fn check_item(graph: &Graph, doc: &Document, item: &Item) -> bool {
    let doc_cid = doc.cid();
    let reference = &item.reference;

    let Some(object) = graph.object(reference) else {
        println!(r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"missing"}}"#);
        return false;
    };

    if !object.kind().starts_with("claim.") {
        let kind = object.kind();
        println!(
            r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"not-a-claim","kind":"{kind}"}}"#
        );
        return false;
    }

    let Bind::Lineage = item.bind else {
        println!(r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"ok"}}"#);
        return true;
    };

    // `Bind::Lineage` guarantees `at` is present -- `Document::from_object`
    // refuses to parse an item that lacks it -- but this reads the stored
    // value rather than assuming the invariant, so it stays correct if
    // that ever changes.
    let Some(at) = &item.at else {
        println!(r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"no-at"}}"#);
        return false;
    };

    let view = graph.lineage(reference, Lineage::Full);
    if !view.members.contains(at) {
        println!(
            r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"unreachable-at","at":"{at}"}}"#
        );
        return false;
    }

    if view.heads.contains(at) {
        println!(r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"ok"}}"#);
        return true;
    }

    // Not a failure: Section 8 requires a renderer to disclose this, not
    // prevent it. Disclosing it is what this line does.
    let heads: Vec<String> = view.heads.iter().map(|h| format!("\"{h}\"")).collect();
    println!(
        r#"{{"doc":"{doc_cid}","ref":"{reference}","status":"diverged","at":"{at}","heads":[{}]}}"#,
        heads.join(",")
    );
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::BTreeMap;

    use publet_core::{Cid, HashAlg, Object, cbor::Value};
    use publet_graph::Graph;

    fn author() -> String {
        Cid::of(b"test author", HashAlg::Sha2_256).to_string()
    }

    fn insert(graph: &mut Graph, kind: &str, fields: &[(&str, Value)]) -> Cid {
        let mut builder = Object::builder(kind, &author()).created("2026-09-12T00:00:00Z");
        for (k, v) in fields {
            builder = builder.field(k, v.clone());
        }
        let bytes = builder.build().expect("object builds");
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        let verified = Object::parse(&bytes)
            .expect("object parses")
            .verify(&cid)
            .expect("object verifies");
        graph.insert(&cid, verified).expect("object indexes");
        cid
    }

    /// A minimal `claim.prose` -- Section 5.5's evidence requirement only
    /// binds `empirical` claims, so `definitional` needs nothing further.
    fn claim(graph: &mut Graph, content: &str) -> Cid {
        insert(
            graph,
            "claim.prose",
            &[
                ("class", Value::Text("definitional".into())),
                ("lang", Value::Text("en".into())),
                ("content", Value::Text(content.into())),
                ("depends", Value::Array(vec![])),
            ],
        )
    }

    fn supersedes(graph: &mut Graph, new: &Cid, old: &Cid) {
        insert(
            graph,
            "claim.relation",
            &[
                ("kind", Value::Text("supersedes".into())),
                ("from", Value::Text(new.to_string())),
                ("to", Value::Text(old.to_string())),
            ],
        );
    }

    fn item(reference: &Cid, bind: &str, at: Option<&Cid>) -> Value {
        let mut m = BTreeMap::new();
        m.insert("ref".to_owned(), Value::Text(reference.to_string()));
        m.insert("bind".to_owned(), Value::Text(bind.to_owned()));
        if let Some(at) = at {
            m.insert("at".to_owned(), Value::Text(at.to_string()));
        }
        m.insert("role".to_owned(), Value::Text("assert".to_owned()));
        Value::Map(m)
    }

    fn doc(graph: &mut Graph, items: Vec<Value>) -> Cid {
        let mut section = BTreeMap::new();
        section.insert("heading".to_owned(), Value::Text("Section".to_owned()));
        section.insert("items".to_owned(), Value::Array(items));
        insert(
            graph,
            "doc",
            &[
                ("title", Value::Text("Test".to_owned())),
                ("sections", Value::Array(vec![Value::Map(section)])),
            ],
        )
    }

    /// Build a one-item document and run `check_item` on that item.
    fn check(graph: &mut Graph, reference: &Cid, bind: &str, at: Option<&Cid>) -> bool {
        let d = doc(graph, vec![item(reference, bind, at)]);
        let document = graph.document(&d).expect("doc indexed");
        let it = document.items().first().expect("one item");
        super::check_item(graph, document, it)
    }

    #[test]
    fn an_object_bound_reference_to_a_real_claim_passes() {
        let mut graph = Graph::new();
        let target = claim(&mut graph, "a real claim");
        assert!(check(&mut graph, &target, "object", None));
    }

    #[test]
    fn a_missing_reference_fails() {
        let mut graph = Graph::new();
        let phantom = Cid::of(b"never inserted", HashAlg::Sha2_256);
        assert!(!check(&mut graph, &phantom, "object", None));
    }

    #[test]
    fn a_reference_to_a_non_claim_object_fails() {
        let mut graph = Graph::new();
        let not_a_claim = insert(&mut graph, "key", &[]);
        assert!(!check(&mut graph, &not_a_claim, "object", None));
    }

    #[test]
    fn a_lineage_bound_reference_at_the_current_head_passes() {
        let mut graph = Graph::new();
        let genesis = claim(&mut graph, "genesis claim");
        assert!(check(&mut graph, &genesis, "lineage", Some(&genesis)));
    }

    #[test]
    fn a_lineage_bound_reference_behind_the_current_head_is_reported_not_failed() {
        let mut graph = Graph::new();
        let genesis = claim(&mut graph, "genesis claim");
        let successor = claim(&mut graph, "successor claim");
        supersedes(&mut graph, &successor, &genesis);
        // The author read `genesis` (`at`), but `successor` is now the
        // head -- Section 8 requires this be disclosed, not refused.
        assert!(check(&mut graph, &genesis, "lineage", Some(&genesis)));
    }

    #[test]
    fn a_lineage_bound_at_outside_the_lineage_fails() {
        let mut graph = Graph::new();
        let genesis = claim(&mut graph, "genesis claim");
        let unrelated = claim(&mut graph, "an unconnected claim");
        assert!(!check(&mut graph, &genesis, "lineage", Some(&unrelated)));
    }
}
