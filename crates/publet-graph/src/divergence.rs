//! Definitional divergence and staleness (Section 9.2).
//!
//! A large share of apparent factual disagreement is definitional
//! disagreement conducted in the vocabulary of fact. Because `depends` is
//! explicit, it is computable rather than a matter of noticing.
//!
//! The two outcomes have different remedies, which is why they are reported
//! separately rather than as one "the parties disagree about a word":
//!
//! - **Divergence** — the definitions have no common genesis. The parties
//!   mean different things. The response is two scoped publets, one under
//!   each definition.
//! - **Staleness** — the definitions are different generations of one
//!   lineage. The parties mean the same thing, one of them as it was
//!   understood earlier. The response is to supersede against the head, or
//!   to say in `scope` why the earlier generation is intended.

use std::collections::BTreeMap;

use publet_core::Cid;

use crate::GraphError;
use crate::graph::{Graph, Lineage};
use crate::view::Class;

/// One term two publets presuppose differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermConflict {
    /// The term both definitions define.
    pub term: String,
    /// The definitional publet the first party depends on.
    pub left: Cid,
    /// The definitional publet the second party depends on.
    pub right: Cid,
}

/// The result of comparing two publets' definitional premises.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Divergence {
    /// Terms whose definitions have no common genesis.
    pub divergent: Vec<TermConflict>,
    /// Terms whose definitions are generations of one lineage.
    pub stale: Vec<TermConflict>,
}

impl Divergence {
    /// Whether anything was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.divergent.is_empty() && self.stale.is_empty()
    }
}

/// Compare the definitional premises of two publets.
///
/// `trusted` decides whose `equivalent` relations count, since a trusted
/// equivalence between two definitions means the parties do not in fact
/// disagree. Whose assertions count is a viewpoint question, so the
/// predicate is supplied by the caller.
///
/// # Errors
///
/// Returns [`GraphError`] if either dependency closure contains a cycle.
pub fn compare(
    graph: &Graph,
    left: &Cid,
    right: &Cid,
    trusted: &dyn Fn(&Cid) -> bool,
) -> Result<Divergence, GraphError> {
    let left_terms = definitional_terms(graph, left)?;
    let right_terms = definitional_terms(graph, right)?;

    let mut out = Divergence::default();
    for (term, left_def) in &left_terms {
        let Some(right_def) = right_terms.get(term) else {
            continue;
        };
        if left_def == right_def {
            continue;
        }
        // A trusted equivalence means the parties are not disagreeing.
        if graph
            .equivalence_class(left_def, trusted)
            .iter()
            .any(|c| c == right_def)
        {
            continue;
        }
        let conflict = TermConflict {
            term: term.clone(),
            left: left_def.clone(),
            right: right_def.clone(),
        };
        // Genesis identity is what separates the two outcomes: one lineage
        // means one concept seen at two moments; two lineages mean two
        // concepts sharing a word.
        let left_genesis = graph.lineage(left_def, Lineage::Authoritative).genesis;
        let right_genesis = graph.lineage(right_def, Lineage::Authoritative).genesis;
        if left_genesis == right_genesis {
            out.stale.push(conflict);
        } else {
            out.divergent.push(conflict);
        }
    }
    Ok(out)
}

/// Terms defined by the definitional publets in a publet's closure.
/// How a set of publets divides on one term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermPartition {
    /// The term they divide on.
    pub term: String,
    /// One entry per definition in play: the definition, and the publets
    /// presupposing it. Ordered largest group first, so the minority
    /// readings -- the ones worth looking at -- sort to the end.
    pub groups: Vec<(Cid, Vec<Cid>)>,
}

/// Partition publets by which definition of each shared term they use.
///
/// `compare` answers whether two publets differ. That is not the question a
/// corpus drawn from several sources asks: with four sources and one term,
/// what is wanted is how all four divide and which one stands alone.
/// Pairwise comparison answers it in six runs and no aggregate.
///
/// Only terms with more than one definition in play are returned. Agreement
/// is the ordinary case and reporting it would bury the disagreements.
///
/// # Errors
///
/// Returns [`GraphError`] if a dependency closure cannot be resolved.
pub fn partition(graph: &Graph, publets: &[Cid]) -> Result<Vec<TermPartition>, GraphError> {
    // term -> definition -> the publets presupposing that definition
    let mut seen: BTreeMap<String, BTreeMap<String, Vec<Cid>>> = BTreeMap::new();
    for cid in publets {
        for (term, definition) in definitional_terms(graph, cid)? {
            seen.entry(term)
                .or_default()
                .entry(definition.to_string())
                .or_default()
                .push(cid.clone());
        }
    }

    let mut out = Vec::new();
    for (term, by_definition) in seen {
        if by_definition.len() < 2 {
            continue;
        }
        let mut groups: Vec<(Cid, Vec<Cid>)> = by_definition
            .into_iter()
            .filter_map(|(definition, mut members)| {
                members.sort_by_key(ToString::to_string);
                definition.parse::<Cid>().ok().map(|d| (d, members))
            })
            .collect();
        // Largest first, then by identifier so the order is stable.
        groups.sort_by(|a, b| {
            b.1.len()
                .cmp(&a.1.len())
                .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
        });
        out.push(TermPartition { term, groups });
    }
    Ok(out)
}

fn definitional_terms(graph: &Graph, cid: &Cid) -> Result<BTreeMap<String, Cid>, GraphError> {
    let mut out = BTreeMap::new();
    for dep in graph.depends_closure(cid)? {
        let Some(publet) = graph.publet(&dep) else {
            continue;
        };
        if publet.class() != Class::Definitional {
            continue;
        }
        let Some(object) = graph.object(&dep) else {
            continue;
        };
        if let Some(term) = publet.term(object) {
            out.insert(term, dep);
        }
    }
    Ok(out)
}
