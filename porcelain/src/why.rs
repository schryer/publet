//! `pub why`: show a standing and every component of it.
//!
//! Section 11.3 forbids reducing a standing to a badge without exposing its
//! components. A reader told only "accepted" cannot tell whether that rests
//! on replication or on agreement, and those are not the same claim.

use std::collections::BTreeSet;

use publet_core::Cid;
use publet_core::cbor::Value;
use publet_eval::{
    Outcome, Policy, class_of, evaluate, evidence_for, propagate_within, trust_edges,
};
use publet_graph::Class;
use publet_graph::load;

use crate::workspace::Workspace;

/// Explain a target's standing.
///
/// # Errors
///
/// Returns a message if the workspace, policy, or target is unavailable.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let target: Cid = args
        .first()
        .ok_or("a CID is required")?
        .parse()
        .map_err(|_| "the argument must be a CID".to_owned())?;

    let policy_cid = ws
        .policy_cid()
        .ok_or("no policy configured; run `pub init`")?;
    let policy_bytes = store
        .get(&policy_cid)
        .map_err(|e| e.to_string())?
        .ok_or("the configured policy is not in your replica")?;
    let policy_object = publet_core::Object::parse(&policy_bytes)
        .map_err(|e| e.to_string())?
        .verify(&policy_cid)
        .map_err(|e| e.to_string())?;
    let policy = Policy::from_object(policy_object.object()).map_err(|e| e.to_string())?;

    let mut objects = Vec::new();
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(bytes)) = store.get(&cid) {
            objects.push((cid, bytes));
        }
    }
    let graph = load::from_objects(objects).map_err(|e| e.to_string())?;

    let class = class_of(&graph, &target)
        .ok_or_else(|| format!("{target} is not a publet in your replica"))?;
    let evidence = evidence_for(&graph, &target);
    // Trust confined to subjects applies only to something in one of them
    // (Section 18.5), so the target's own memberships are the context the
    // propagation runs in.
    let subjects = graph.subjects_of(&target);
    let weights = propagate_within(&policy, &trust_edges(&graph), &subjects);
    let standing = evaluate(&policy, class, &evidence, &weights);

    println!("{target}");
    println!("class            {}", class.id());
    println!("result           {}", standing.result.id());
    println!();
    println!("weight");
    println!("  affirm         {}", standing.affirm_weight);
    println!("  deny           {}", standing.deny_weight);
    println!("  abstain        {}", standing.abstain_weight);
    println!("  active         {}", standing.active_weight);
    println!("  delta          {}", standing.delta);
    println!("  threshold      {}", policy.tau + standing.delta);
    println!();
    println!("evidence");
    println!(
        "  reproducibility            {}",
        standing.reproducibility_class.id()
    );
    println!(
        "  consistent                 {}",
        standing.reproductions.consistent
    );
    println!(
        "  inconsistent               {}",
        standing.reproductions.inconsistent
    );
    println!(
        "  independent consistent     {} (floor {})",
        standing.reproductions.independent_consistent, policy.replication_floor
    );
    println!(
        "  independent inconsistent   {}",
        standing.reproductions.independent_inconsistent
    );
    if standing.retracted {
        println!("  retracted                  yes");
    }

    if class == Class::Definitional {
        print_usage(&graph, &target, &evidence.usage);
    }
    print_assessments(&evidence.assessments);

    println!();
    println!("{}", explain(standing.result));
    println!();
    println!("Computed under policy {policy_cid}, locally. Your trust roots");
    println!("decide this; another reader's roots may decide otherwise.");
    Ok(())
}

/// Show the judgements filed about a claim.
///
/// Separated from the weight block on purpose. An assessment is not input
/// to the outcome: `sound-in-scope` is a remark about where a claim holds,
/// and presenting it next to a computed weight would invite reading it as
/// one. That is also why it is sayable about a definitional publet, which
/// accepts no verdict at all.
fn print_assessments(assessments: &[publet_eval::Assessment]) {
    if assessments.is_empty() {
        return;
    }
    println!();
    println!("assessments (judgements, not inputs to the outcome)");
    for assessment in assessments {
        println!("  {:<16} {}", assessment.verdict, assessment.basis);
    }
    let in_scope = assessments
        .iter()
        .filter(|a| a.verdict == "sound-in-scope")
        .count();
    if in_scope > 0 {
        println!();
        println!(
            "  {in_scope} judge(s) hold this sound within its scope and not \
             beyond it."
        );
        println!("  Read the scope above before carrying it elsewhere.");
    }
}

/// Show the citations supporting a definition.
///
/// Section 5.2 permits no verdict on a definitional publet and settles it
/// by usage instead, so the weight block above is all zeroes for one by
/// construction. Without this the reader sees nothing at all about why a
/// definition stands.
fn print_usage(graph: &publet_graph::Graph, target: &Cid, sources: &BTreeSet<String>) {
    println!();
    println!("usage");
    if sources.is_empty() {
        println!("  no corpus citations");
        println!();
        println!("  A definition is described by usage, not voted true.");
        println!("  With nothing citing it, nothing supports this one.");
        return;
    }
    for source in sources {
        let where_used = citation_locators(graph, target, source);
        if where_used.is_empty() {
            println!("  {source}");
        } else {
            println!("  {source}  ({})", where_used.join("; "));
        }
    }
    println!();
    println!(
        "  {} distinct source(s). Two citations of one work are one",
        sources.len()
    );
    println!("  work agreeing with itself, so sources are counted, not citations.");
}

/// Where in a source a sense was found, for every citation naming it.
///
/// The count above is of distinct sources, because two citations of one
/// work are one work agreeing with itself. The locators are still worth
/// showing: a citation that cannot be looked up is an assertion, and the
/// whole point of usage evidence is that a reader can go and check.
fn citation_locators(graph: &publet_graph::Graph, target: &Cid, source: &str) -> Vec<String> {
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
        if body.get("kind").and_then(Value::as_text) != Some("usage")
            || body.get("target").and_then(Value::as_text) != Some(&target.to_string())
        {
            continue;
        }
        let Some(value) = body.get("value") else {
            continue;
        };
        if value.get("source").and_then(Value::as_text) != Some(source) {
            continue;
        }
        let locator = value.get("locator").and_then(Value::as_text);
        let sense = value.get("sense").and_then(Value::as_text);
        match (locator, sense) {
            (Some(l), Some(n)) => out.push(format!("{l}, {n}")),
            (Some(l), None) => out.push(l.to_owned()),
            (None, Some(n)) => out.push(n.to_owned()),
            (None, None) => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Say which rule produced the outcome, since the number alone does not.
fn explain(result: Outcome) -> &'static str {
    match result {
        Outcome::Accepted => "Accepted: the weight condition holds and the evidence permits it.",
        Outcome::Unreplicated => {
            "Unreplicated: the weight condition holds and the evidence does not.\n\
             No amount of agreement substitutes for one replication (R9). This is\n\
             not a criticism -- most true claims are unreplicated most of the time."
        }
        Outcome::Rejected => "Rejected: weight or evidence is against it.",
        Outcome::Contested => "Contested: weighted participants examined it and did not converge.",
        Outcome::Undetermined => {
            "Undetermined: no participant carrying weight under your policy has\n\
             evaluated this. That is a fact about your viewpoint, not about the claim."
        }
        Outcome::NotTruthApt => {
            "Not truth-apt: this class has no verdict and never will.\n\
             Look at attribution, usage, and critique instead."
        }
        _ => "Outcome not recognized by this build.",
    }
}
