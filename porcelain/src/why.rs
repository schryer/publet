//! `pub why`: show a standing and every component of it.
//!
//! Section 11.3 forbids reducing a standing to a badge without exposing its
//! components. A reader told only "accepted" cannot tell whether that rests
//! on replication or on agreement, and those are not the same claim.

use publet_core::Cid;
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

    // Section 5.2: a definitional publet is settled by usage evidence and
    // accepts no verdict, so the weight block above is all zeroes for one
    // by construction. Showing the citations is the only way this reader
    // sees anything at all about why a definition stands.
    if class == Class::Definitional {
        println!();
        println!("usage");
        if evidence.usage.is_empty() {
            println!("  no corpus citations");
            println!();
            println!("  A definition is described by usage, not voted true.");
            println!("  With nothing citing it, nothing supports this one.");
        } else {
            for source in &evidence.usage {
                println!("  {source}");
            }
            println!();
            println!(
                "  {} distinct source(s). Two citations of one work are one",
                evidence.usage.len()
            );
            println!("  work agreeing with itself, so sources are counted, not citations.");
        }
    }

    println!();
    println!("{}", explain(standing.result));
    println!();
    println!("Computed under policy {policy_cid}, locally. Your trust roots");
    println!("decide this; another reader's roots may decide otherwise.");
    Ok(())
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
