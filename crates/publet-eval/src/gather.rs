//! Collecting the evidence a claim has attracted (Sections 7 and 11).
//!
//! The graph holds signed objects; evaluation consumes weights and counts.
//! This is the bridge, and it is deliberately thin: it reads what was
//! signed and makes no judgements, because whose signatures count is a
//! viewpoint question that belongs to the evaluation itself.

use std::collections::{BTreeMap, BTreeSet};

use publet_core::{Cid, cbor::Value};
use publet_graph::{Class, Graph, RelationKind};

use crate::{Evidence, Reproducibility, Reproductions};

/// Gather the evidence bearing on `target`.
///
/// Annotations are attributed to their signing key rather than counted, so
/// that the evaluation can weight them and count each signer once.
#[must_use]
pub fn evidence_for(graph: &Graph, target: &Cid) -> Evidence {
    let mut evidence = Evidence {
        reproducibility: reproducibility_of(graph, target),
        ..Evidence::default()
    };

    let mut filed_reproductions: Vec<Filed> = Vec::new();

    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        if object.kind() != "ann" {
            continue;
        }
        let body = object.body();
        let Some(Value::Text(kind)) = body.get("kind") else {
            continue;
        };
        let Some(Value::Text(annotated)) = body.get("target") else {
            continue;
        };
        if annotated != &target.to_string() {
            continue;
        }
        let author = object.author().to_string();

        match kind.as_str() {
            "verdict" => match finding_of(body) {
                Some("affirm") => {
                    evidence.affirm.insert(author);
                }
                Some("deny") => {
                    evidence.deny.insert(author);
                }
                Some("abstain") => {
                    evidence.abstain.insert(author);
                }
                _ => {}
            },
            "proof-checked" => {
                if result_of(body) == Some("accepted") {
                    evidence.proof_checked = true;
                }
            }
            // Section 5.2: a definitional publet is settled by usage
            // evidence and accepts no verdict. A citation names where a
            // term is used, and deliberately need not reproduce the text:
            // recording a location is what keeps a corpus of definitions
            // from becoming a corpus of restatements.
            "usage" => {
                if let Some(source) = body
                    .get("value")
                    .and_then(|v| v.get("source"))
                    .and_then(Value::as_text)
                {
                    evidence.usage.insert(source.to_owned());
                }
            }
            "reproduction" => {
                if let Some(filed) = read_reproduction(
                    body,
                    object.author(),
                    authored_by_human(graph, object.author()),
                ) {
                    filed_reproductions.push(filed);
                }
            }
            _ => {}
        }
    }

    tally(graph, &filed_reproductions, &mut evidence.reproductions);

    // A dispute counts only when it names a publet stating grounds, which
    // separates an argument from an objection (Section 6.4), and only when
    // it is not redundant under R10.
    let settled = settled_grounds(graph, target);
    for from in graph.incoming(RelationKind::Disputes, target) {
        let Ok(cid) = from.parse::<Cid>() else {
            continue;
        };
        if graph.publet(&cid).is_none() {
            continue;
        }
        // R10: a dispute contributing no grounds beyond those already
        // covered by a resolution carries no weight. Computed as set
        // containment over identifiers, so it needs no judgement and no
        // authority -- and a disputant who does produce new grounds is
        // never redundant, however unpopular their position.
        if settled.contains(&cid.to_string()) {
            continue;
        }
        if let Some(object) = graph.object(&cid) {
            evidence.argued_disputes.insert(object.author().to_string());
        }
    }

    // Retraction is authoritative only from a key that signed the target.
    if let Some(object) = graph.object(target) {
        let author = object.author().to_string();
        for from in graph.incoming(RelationKind::Retracts, target) {
            let Ok(cid) = from.parse::<Cid>() else {
                continue;
            };
            if graph
                .object(&cid)
                .is_some_and(|r| r.author().to_string() == author)
            {
                evidence.retracted = true;
            }
        }
    }

    evidence
}

/// Grounds already covered by a resolution of `target`.
///
/// Only resolutions whose outcome is `sustained` settle anything:
/// `no-consensus` records that weighted participants examined the question
/// and did not converge, which is exactly the controversy the divergence
/// factor exists to express, so its grounds remain live.
fn settled_grounds(graph: &Graph, target: &Cid) -> BTreeSet<String> {
    let mut out = BTreeSet::new();

    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        if object.kind() != "ann" {
            continue;
        }
        let body = object.body();
        if body.get("kind").and_then(Value::as_text) != Some("resolution") {
            continue;
        }
        if body.get("target").and_then(Value::as_text) != Some(&target.to_string()) {
            continue;
        }
        let Some(value) = body.get("value") else {
            continue;
        };
        if value.get("outcome").and_then(Value::as_text) != Some("sustained") {
            continue;
        }
        if let Some(Value::Array(grounds)) = value.get("grounds") {
            out.extend(
                grounds
                    .iter()
                    .filter_map(|g| g.as_text().map(ToOwned::to_owned)),
            );
        }
    }
    out
}

/// The claim class of a target, if it is a publet.
#[must_use]
pub fn class_of(graph: &Graph, target: &Cid) -> Option<Class> {
    graph.publet(target).map(publet_graph::Publet::class)
}

fn reproducibility_of(graph: &Graph, target: &Cid) -> Reproducibility {
    graph
        .object(target)
        .and_then(|o| o.body().get("reproducibility"))
        .and_then(|r| r.get("class"))
        .and_then(Value::as_text)
        .and_then(Reproducibility::from_id)
        .unwrap_or_default()
}

fn finding_of(body: &std::collections::BTreeMap<String, Value>) -> Option<&str> {
    body.get("value")?.get("finding")?.as_text()
}

fn result_of(body: &std::collections::BTreeMap<String, Value>) -> Option<&str> {
    body.get("value")?.get("result")?.as_text()
}

/// Whether a key declares a human principal (R11).
///
/// A key whose object is absent is treated as non-human: a reproduction
/// counted toward a floor must be attributable to a person, and an
/// unresolvable key is not.
fn authored_by_human(graph: &Graph, author: &Cid) -> bool {
    graph
        .object(author)
        .and_then(|o| {
            publet_graph::Key::from_object(author.clone(), o)
                .ok()
                .map(|k| k.principal().is_human())
        })
        .unwrap_or(false)
}

/// One filed reproduction, before independence has been decided.
///
/// Independence cannot be decided while reading: Section 11.4 defines it
/// *between* two reproductions, so nothing about a single annotation
/// settles it. Reading and deciding are therefore separate passes.
struct Filed {
    author: Cid,
    outcome: String,
    /// Whether the filer declared materials shared with another attempt.
    shares_materials: bool,
    human: bool,
}

/// Read one reproduction annotation.
fn read_reproduction(
    body: &std::collections::BTreeMap<String, Value>,
    author: &Cid,
    human: bool,
) -> Option<Filed> {
    let outcome = body
        .get("value")
        .and_then(|v| v.get("outcome"))
        .and_then(Value::as_text)?;
    let shares_materials = body
        .get("value")
        .and_then(|v| v.get("independence"))
        .is_some_and(|i| i.get("shared_materials").is_some());
    Some(Filed {
        author: author.clone(),
        outcome: outcome.to_owned(),
        shares_materials,
        human,
    })
}

/// Count reproductions, deciding independence between them (Section 11.4).
///
/// Two reproductions are independent under a viewpoint when they share no
/// `affiliated` annotation naming the same organization over an overlapping
/// period, they declare no shared materials, and -- counted toward a floor
/// -- both are signed by human principals (R11).
///
/// The affiliation rule is what distinguishes four replications by one
/// consortium from four independent ones, and it is a statement about
/// *pairs*: a set of colleagues contributes one, not one each.
fn tally(graph: &Graph, filed: &[Filed], counts: &mut Reproductions) {
    for one in filed {
        match one.outcome.as_str() {
            "consistent" => counts.consistent += 1,
            "inconsistent" => counts.inconsistent += 1,
            "inconclusive" | "method-underspecified" => counts.inconclusive += 1,
            _ => {}
        }
    }

    for outcome in ["consistent", "inconsistent"] {
        let eligible: Vec<&Filed> = filed
            .iter()
            .filter(|f| f.outcome == outcome && f.human && !f.shares_materials)
            .collect();
        let groups = independent_groups(graph, &eligible);
        let n = u32::try_from(groups).unwrap_or(u32::MAX);
        if outcome == "consistent" {
            counts.independent_consistent = n;
        } else {
            counts.independent_inconsistent = n;
        }
    }
}

/// How many mutually independent parties these reproductions represent.
///
/// Colleagues are merged: sharing an organization over an overlapping
/// period makes two filers one party for this purpose. The merge is
/// transitive -- if A and B share a laboratory and B and C share a grant,
/// all three are one party -- so this is connected components over the
/// "not independent of" relation, not a pairwise count.
fn independent_groups(graph: &Graph, filed: &[&Filed]) -> usize {
    let affiliations: Vec<Vec<Affiliation>> = filed
        .iter()
        .map(|f| affiliations_of(graph, &f.author))
        .collect();

    let related = |i: usize, j: usize| -> bool {
        let (Some(left), Some(right)) = (filed.get(i), filed.get(j)) else {
            return false;
        };
        if left.author == right.author {
            return true;
        }
        match (affiliations.get(i), affiliations.get(j)) {
            (Some(a), Some(b)) => shares_affiliation(a, b),
            _ => false,
        }
    };

    let mut group: BTreeMap<usize, usize> = BTreeMap::new();
    let mut parties = 0;
    for start in 0..filed.len() {
        if group.contains_key(&start) {
            continue;
        }
        let mut stack = vec![start];
        while let Some(current) = stack.pop() {
            if group.insert(current, parties).is_some() {
                continue;
            }
            for other in 0..filed.len() {
                if !group.contains_key(&other) && related(current, other) {
                    stack.push(other);
                }
            }
        }
        parties += 1;
    }
    parties
}

/// A declared affiliation (Section 10.4).
struct Affiliation {
    org: String,
    from: Option<String>,
    to: Option<String>,
}

/// Affiliations declared for a key by `affiliated` annotations.
fn affiliations_of(graph: &Graph, key: &Cid) -> Vec<Affiliation> {
    let mut out = Vec::new();
    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        if object.kind() != "ann" {
            continue;
        }
        let body = object.body();
        if body.get("kind").and_then(Value::as_text) != Some("affiliated") {
            continue;
        }
        if body.get("target").and_then(Value::as_text) != Some(&key.to_string()) {
            continue;
        }
        let Some(value) = body.get("value") else {
            continue;
        };
        let Some(org) = value.get("org").and_then(Value::as_text) else {
            continue;
        };
        let bound = |name: &str| {
            value
                .get("period")
                .and_then(|p| p.get(name))
                .and_then(Value::as_text)
                .map(ToOwned::to_owned)
        };
        out.push(Affiliation {
            org: org.to_owned(),
            from: bound("from"),
            to: bound("to"),
        });
    }
    out
}

/// Whether two keys share an organization over an overlapping period.
///
/// An absent bound is open: someone who has not said when they left is
/// still there. Treating a missing date as "no overlap" would let an
/// undisclosed end date manufacture independence.
fn shares_affiliation(left: &[Affiliation], right: &[Affiliation]) -> bool {
    for a in left {
        for b in right {
            if a.org != b.org {
                continue;
            }
            let a_from = a.from.as_deref().unwrap_or("");
            let b_from = b.from.as_deref().unwrap_or("");
            let a_to = a.to.as_deref().unwrap_or("~");
            let b_to = b.to.as_deref().unwrap_or("~");
            if a_from <= b_to && b_from <= a_to {
                return true;
            }
        }
    }
    false
}

/// Trust edges declared by `trusts` annotations in the graph.
#[must_use]
pub fn trust_edges(graph: &Graph) -> Vec<crate::TrustEdge> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        if object.kind() != "ann" {
            continue;
        }
        let body = object.body();
        if body.get("kind").and_then(Value::as_text) != Some("trusts") {
            continue;
        }
        let Some(Value::Text(to)) = body.get("target") else {
            continue;
        };
        let weight = body
            .get("value")
            .and_then(|v| v.get("weight"))
            .and_then(Value::as_uint)
            .unwrap_or(1);
        let from = object.author().to_string();
        if seen.insert((from.clone(), to.clone())) {
            let subjects = body
                .get("value")
                .and_then(|v| v.get("subjects"))
                .and_then(|v| match v {
                    Value::Array(items) => Some(
                        items
                            .iter()
                            .filter_map(|i| i.as_text().map(ToOwned::to_owned))
                            .collect::<Vec<String>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            out.push(crate::TrustEdge {
                subjects,
                from,
                to: to.clone(),
                weight: crate::Fixed6::from_scaled(i128::from(weight) * crate::SCALE),
                age_days: 0,
            });
        }
    }
    out
}
