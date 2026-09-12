//! Collecting the evidence a claim has attracted (Sections 7 and 11).
//!
//! The graph holds signed objects; evaluation consumes weights and counts.
//! This is the bridge, and it is deliberately thin: it reads what was
//! signed and makes no judgements, because whose signatures count is a
//! viewpoint question that belongs to the evaluation itself.

use std::collections::BTreeSet;

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
            "reproduction" => count_reproduction(body, &mut evidence.reproductions),
            _ => {}
        }
    }

    // A dispute counts only when it names a publet stating grounds, which
    // is what separates an argument from an objection (Section 6.4).
    for from in graph.incoming(RelationKind::Disputes, target) {
        let Ok(cid) = from.parse::<Cid>() else {
            continue;
        };
        if graph.publet(&cid).is_none() {
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

fn count_reproduction(
    body: &std::collections::BTreeMap<String, Value>,
    counts: &mut Reproductions,
) {
    let Some(outcome) = body
        .get("value")
        .and_then(|v| v.get("outcome"))
        .and_then(Value::as_text)
    else {
        return;
    };
    // Independence is a viewpoint computation over trust paths and shared
    // affiliation; a reproduction that declares none is counted as
    // independent, and a viewpoint may reduce that.
    let independent = body
        .get("value")
        .and_then(|v| v.get("independence"))
        .is_none_or(|i| i.get("shared_materials").is_none());

    match outcome {
        "consistent" => {
            counts.consistent += 1;
            if independent {
                counts.independent_consistent += 1;
            }
        }
        "inconsistent" => {
            counts.inconsistent += 1;
            if independent {
                counts.independent_inconsistent += 1;
            }
        }
        "inconclusive" | "method-underspecified" => counts.inconclusive += 1,
        _ => {}
    }
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
            out.push(crate::TrustEdge {
                from,
                to: to.clone(),
                weight: crate::Fixed6::from_scaled(i128::from(weight) * crate::SCALE),
                age_days: 0,
            });
        }
    }
    out
}
