//! `pub propose`: submit objects, recording what they were composed against.
//!
//! A proposal declares its basis: the generation the author read. The domain
//! may advance between reading and landing, and under R1 nothing is
//! overwritten, so a stale basis cannot cause a lost update. It is
//! information to disclose, never a reason to refuse.
//!
//! What the command does with it is report the **relevant delta**: the three
//! ways a proposal can be stale, each of which has a different remedy.

use publet_core::Cid;
use publet_graph::{Graph, Lineage, RelationKind, load};

use crate::workspace::Workspace;

/// Submit objects with a recorded basis.
///
/// # Errors
///
/// Returns a message if the workspace or an argument is unusable.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut targets: Vec<Cid> = Vec::new();
    let mut basis: Option<u64> = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--basis=") {
            basis = v.parse().ok();
        } else if let Ok(cid) = arg.parse::<Cid>() {
            targets.push(cid);
        } else {
            return Err(format!("not a CID: {arg}"));
        }
    }
    if targets.is_empty() {
        return Err("at least one CID is required".to_owned());
    }

    let domain = ws.get("domain").and_then(|d| d.parse::<Cid>().ok());
    let head = domain
        .as_ref()
        .and_then(|d| store.head_generation(d).ok().flatten())
        .unwrap_or(0);
    let basis = basis.unwrap_or(head);

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

    println!("proposing {} object(s)", targets.len());
    println!("basis     generation {basis}");
    if head > basis {
        println!("head      generation {head}");
    }
    println!();

    let mut any = false;
    for target in &targets {
        any |= report_staleness(&graph, target);
    }

    if any {
        println!();
        println!("A stale basis is never grounds for rejection. Nothing here is");
        println!("overwritten (R1), so a concurrent proposal cannot be clobbered;");
        println!("divergent successors are a branching lineage, resolved per");
        println!("viewpoint. The above is disclosed, not enforced.");
    } else {
        println!("nothing has moved under these objects since the basis");
    }
    Ok(())
}

/// Report the three ways a proposal can be stale (Section 14.3.3).
fn report_staleness(graph: &Graph, target: &Cid) -> bool {
    let mut found = false;

    // 1. The object being superseded has itself been superseded, so the
    //    lineage will branch.
    for to in graph.out(RelationKind::Supersedes, target) {
        let Ok(superseded) = to.parse::<Cid>() else {
            continue;
        };
        let view = graph.lineage(&superseded, Lineage::Full);
        let others: Vec<&Cid> = view.heads.iter().filter(|h| *h != target).collect();
        if !others.is_empty() {
            found = true;
            println!("{target}");
            println!("  supersedes {superseded}, which already has other successors:");
            for other in others {
                println!("    {other}");
            }
            println!("  the lineage will branch; selection is viewpoint-relative");
        }
    }

    // 2. A publet in the dependency closure has been superseded, so the
    //    proposal rests on an earlier generation of a definition.
    if let Ok(closure) = graph.depends_closure(target) {
        for dependency in closure {
            let view = graph.lineage(&dependency, Lineage::Authoritative);
            if view.heads.iter().any(|h| *h != dependency) {
                found = true;
                println!("{target}");
                println!("  depends on {dependency}, which has been superseded by:");
                for head in &view.heads {
                    if *head != dependency {
                        println!("    {head}");
                    }
                }
                println!("  supersede against the head, or say in `scope` why the");
                println!("  earlier generation is the one you intend (Section 9.2)");
            }
        }
    }

    // 3. A dispute this answers has been resolved, which may make it
    //    redundant under R10.
    for disputed in graph.out(RelationKind::Disputes, target) {
        let Ok(cid) = disputed.parse::<Cid>() else {
            continue;
        };
        let resolutions = resolutions_for(graph, &cid);
        if !resolutions.is_empty() {
            found = true;
            println!("{target}");
            println!("  disputes {cid}, for which resolutions already exist:");
            for r in resolutions {
                println!("    {r}");
            }
            println!("  a dispute adding no new grounds carries no weight (R10)");
        }
    }

    found
}

fn resolutions_for(graph: &Graph, target: &Cid) -> Vec<String> {
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
        let is_resolution =
            body.get("kind").and_then(publet_core::cbor::Value::as_text) == Some("resolution");
        let names_target = body
            .get("target")
            .and_then(publet_core::cbor::Value::as_text)
            == Some(&target.to_string());
        if is_resolution && names_target {
            out.push(cid_text.to_owned());
        }
    }
    out
}
