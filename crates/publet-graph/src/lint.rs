//! Section 5.7 over a loaded graph.
//!
//! The tests themselves live in `publet-lint`, which sees one publet's text
//! and nothing else. This module answers the one question that needs the
//! graph -- which terms are contested -- and hands the answer down.

use publet_core::Cid;
use publet_lint::Term;

use crate::graph::Graph;
use crate::view::{Class, Publet, RelationKind};

pub use publet_lint::Finding;

/// Run the structural tests over a publet's content alone.
///
/// Use [`check_in`] where a graph is available: it additionally reports
/// contested terms the publet uses without declaring.
#[must_use]
pub fn check(publet: &Publet) -> Vec<Finding> {
    publet_lint::check(publet.content(), &[])
}

/// Run the structural tests with the graph's view of which terms are
/// contested.
#[must_use]
pub fn check_in(graph: &Graph, publet: &Publet) -> Vec<Finding> {
    let contested = contested_terms(graph);
    let declared = publet.depends();
    // A definition states its term; it does not use it. Asking a rival
    // definition to declare a dependency on the definition it disputes
    // would make the two circular, and R5 forbids that for good reason.
    let own = graph
        .object(publet.cid())
        .and_then(|object| publet.term(object));
    let terms: Vec<Term<'_>> = contested
        .iter()
        .filter(|(word, _)| own.as_deref() != Some(word.as_str()))
        .map(|(word, defining)| Term {
            word,
            declared: declared.contains(defining),
        })
        .collect();
    publet_lint::check(publet.content(), &terms)
}

/// Terms whose defining publet something disputes, paired with that publet.
///
/// A term nobody disputes needs no declaration: the reader and the author
/// already agree about it, and requiring `depends` on every word would make
/// the list useless. The list exists to pin down the words that are the
/// disagreement.
fn contested_terms(graph: &Graph) -> Vec<(String, Cid)> {
    let mut out = Vec::new();
    for raw in graph.cids() {
        let Ok(cid) = raw.parse::<Cid>() else {
            continue;
        };
        let Some(publet) = graph.publet(&cid) else {
            continue;
        };
        if publet.class() != Class::Definitional {
            continue;
        }
        if graph.incoming(RelationKind::Disputes, &cid).is_empty() {
            continue;
        }
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        if let Some(term) = publet.term(object) {
            out.push((term, cid));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
