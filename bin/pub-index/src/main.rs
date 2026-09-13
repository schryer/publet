//! Resolve a subject under a declared policy (Section 13.3).
//!
//! Every result carries the policy identifier, the snapshot identifier, and
//! the standing each ranking rests on, so a caller who suspects manipulation
//! recomputes and compares. That is the only form of prohibition a remote
//! party can check: requiring an operator to run particular source code, or
//! to price at cost, is not verifiable from outside, while recomputation is
//! verifiable by everyone.
//!
//! Nothing is inserted that the declared policy's evaluation does not yield.
//! There is no paid-placement flag to omit, because there is no code path
//! that could add one.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg};
use publet_eval::{Policy, class_of, evaluate, evidence_for, propagate, trust_edges};
use publet_graph::load;
use publet_merkle::membership::Membership;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut policy_cid: Option<String> = None;
    let mut class_filter: Option<String> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--policy=") {
            policy_cid = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--class=") {
            class_filter = Some(v.to_owned());
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-index --policy=CID [--class=CLASS] [--dir=DIR]");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    // The caller supplies the policy. An index node that chose one for you
    // would be answering a different question from the one you asked.
    let Some(policy_cid) = policy_cid else {
        eprintln!("--policy is required: an index resolves under a declared");
        eprintln!("viewpoint, and there is no default viewpoint to fall back on");
        return ExitCode::from(EXIT_USAGE);
    };
    let Ok(policy_cid) = policy_cid.parse::<Cid>() else {
        eprintln!("--policy must be a CID");
        return ExitCode::from(EXIT_USAGE);
    };

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    let Some(policy_object) = graph.object(&policy_cid) else {
        eprintln!("the policy {policy_cid} is not in this store");
        return ExitCode::from(EXIT_VIOLATION);
    };
    let policy = match Policy::from_object(policy_object) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    // The snapshot the ranking was computed over, so it can be recomputed.
    let members: Vec<String> = graph.cids().iter().map(ToString::to_string).collect();
    let snapshot = Membership::new(members).root();
    let snapshot_cid = Cid::from_digest(HashAlg::Sha2_256, &snapshot)
        .map_or_else(|| "<malformed>".to_owned(), |c| c.to_string());

    let weights = propagate(&policy, &trust_edges(&graph));

    let mut rows = Vec::new();
    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse::<Cid>() else {
            continue;
        };
        let Some(class) = class_of(&graph, &cid) else {
            continue;
        };
        if let Some(want) = &class_filter
            && class.id() != want
        {
            continue;
        }
        let standing = evaluate(&policy, class, &evidence_for(&graph, &cid), &weights);
        rows.push((cid_text.to_owned(), class.id(), standing));
    }

    // Ordering is by affirm weight then identifier, so two runs over one
    // snapshot produce the same list in the same order.
    rows.sort_by(|a, b| {
        b.2.affirm_weight
            .cmp(&a.2.affirm_weight)
            .then_with(|| a.0.cmp(&b.0))
    });

    for (cid, class, standing) in rows {
        println!(
            r#"{{"cid":"{cid}","class":"{class}","result":"{}","affirm":"{}","policy":"{policy_cid}","snapshot":"{snapshot_cid}"}}"#,
            standing.result.id(),
            standing.affirm_weight,
        );
    }
    ExitCode::SUCCESS
}
