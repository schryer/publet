//! `pub get PATH`: the current value of a fact, found by its path key.
//!
//! A path key such as `place.site.tallinn.basement.floor_area` names a
//! singular by its class and the chain of things it is part of, then a
//! quantity concept (Section 5.9.6). The path is computed from the claims
//! held, never registered, so `pub get` reads every singular here, finds the
//! one whose path it is, and shows the head of each lineage of claims stating
//! a value about it: the value, its unit and measure, the roles of its
//! evidence, and the status those imply by convention (Section 5.9.7).
//!
//! A segment that is an H3 cell asks spatially: `place.<cell>` is every fact
//! about a singular of that class located in the cell, by H3's identifier
//! hierarchy rather than by geometry.

use publet_algorithms::h3::Cell as H3Cell;
use publet_core::Cid;
use publet_core::cbor::Value;
use publet_graph::{Definition, Describes, Fact, Graph, RelationKind};

use crate::workspace::Workspace;

/// Look a path up and print what is found.
///
/// # Errors
///
/// Returns a message if the workspace or directory cannot be read, the path
/// is empty, or nothing held answers it.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let mut path = None;
    let mut dir = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = Some(std::path::PathBuf::from(v));
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else if path.replace(arg.clone()).is_some() {
            return Err("one path at a time".to_owned());
        }
    }
    let path = path.ok_or("usage: pub get PATH [--dir=DIR]")?;
    let segments: Vec<&str> = path.split('.').collect();
    if segments.iter().any(|s| s.is_empty()) {
        return Err(format!("{path}: a path is segments joined by dots"));
    }

    let (graph, local) = if let Some(d) = &dir {
        (
            publet_graph::load::from_dir(d).map_err(|e| e.to_string())?,
            false,
        )
    } else {
        let here = std::env::current_dir().map_err(|e| e.to_string())?;
        let store = Workspace::open(&here)?.store()?;
        (crate::build::graph_of(&store)?, true)
    };

    let found = match segments.iter().position(|s| s.parse::<H3Cell>().is_ok()) {
        Some(at) => within(&graph, &segments, at)?,
        None => by_path(&graph, &segments),
    };
    if found.is_empty() {
        return Err(format!(
            "nothing held here answers {path}: no singular has that path, or no claim states a value about it"
        ));
    }
    let mut first = true;
    for (thing, facts) in found {
        for fact in facts {
            if !first {
                println!();
            }
            first = false;
            show(&graph, &thing, &fact);
        }
    }
    if local {
        println!();
        println!(
            "[local] read from your replica; nothing was disclosed. Paths are computed from what is held here."
        );
    }
    Ok(())
}

/// The singulars whose path is `segments`, or whose path is all but the
/// last segment, which then names the concept; with the facts about each.
fn by_path<'g>(graph: &'g Graph, segments: &[&str]) -> Vec<(String, Vec<Fact<'g>>)> {
    let mut out = Vec::new();
    for claim in graph.singulars() {
        let Ok(own) = graph.singular_path(claim.cid()) else {
            continue;
        };
        let concept = if own == segments {
            None
        } else if own.len() + 1 == segments.len() && segments.starts_with(&own_refs(&own)) {
            segments.last().copied()
        } else {
            continue;
        };
        let facts = current(graph, claim.cid(), concept);
        if !facts.is_empty() {
            out.push((own.join("."), facts));
        }
    }
    out
}

/// The singulars of the class `segments[..at]` located in the H3 cell at
/// `at`, with the facts about each, of the concept after it if one is named.
fn within<'g>(
    graph: &'g Graph,
    segments: &[&str],
    at: usize,
) -> Result<Vec<(String, Vec<Fact<'g>>)>, String> {
    let (class, rest) = segments.split_at(at);
    let Some((cell, concept)) = rest.split_first() else {
        return Ok(Vec::new());
    };
    let cell: H3Cell = cell.parse().map_err(|e| format!("{cell}: {e}"))?;
    let concept = match concept {
        [] => None,
        [one] => Some(*one),
        _ => {
            return Err(
                "after an H3 cell, a path names at most a concept: place.<cell>.floor_area"
                    .to_owned(),
            );
        }
    };
    let mut out = Vec::new();
    for claim in graph.singulars() {
        let Ok(own) = graph.class_path(claim.cid()) else {
            continue;
        };
        if !own_refs(&own).starts_with(class) {
            continue;
        }
        let Some(location) = graph.singular_location(claim.cid()) else {
            continue;
        };
        if !cell.contains(&location) {
            continue;
        }
        let facts = current(graph, claim.cid(), concept);
        if !facts.is_empty() {
            let name = graph
                .singular_path(claim.cid())
                .map_or_else(|_| claim.cid().to_string(), |p| p.join("."));
            out.push((name, facts));
        }
    }
    Ok(out)
}

fn own_refs(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

/// The facts about a singular, or any version of it, of `concept` if one is
/// named, from the claims that head their lineages: a superseded statement
/// of a value is history, not the value.
fn current<'g>(graph: &'g Graph, singular: &Cid, concept: Option<&str>) -> Vec<Fact<'g>> {
    let versions = graph.singular_versions(std::slice::from_ref(singular));
    graph
        .facts_about(&versions)
        .into_iter()
        .filter(|f| concept.is_none_or(|c| f.concept == c))
        .filter(|f| graph.lineage_heads(f.claim.cid()).contains(f.claim.cid()))
        .collect()
}

fn show(graph: &Graph, thing: &str, fact: &Fact<'_>) {
    let cid = fact.claim.cid();
    println!("{thing}.{}", fact.concept);
    println!("claim     {cid}");
    let heads = graph.lineage_heads(cid);
    if heads.len() > 1 {
        println!(
            "lineage   branched: {} heads, each shown; choosing among them is a viewpoint's (Section 6.1)",
            heads.len()
        );
    }
    println!("about     {}", fact.about);
    println!("value     {}", value_text(fact.value));
    if let Some(unit) = &fact.column.unit {
        println!("unit      {unit}");
    }
    let measure =
        fact.column
            .measure
            .as_ref()
            .and_then(|m| match graph.prose_claim(m)?.defines()? {
                Definition::Measure {
                    name, describes, ..
                } => Some((m, name.clone(), *describes)),
                _ => None,
            });
    match &measure {
        Some((m, name, _)) => println!("measure   {name} ({m})"),
        None => println!("measure   none stated: one value, with no uncertainty stated"),
    }
    let roles = roles(graph, cid);
    println!(
        "evidence  {}",
        if roles.is_empty() {
            "none".to_owned()
        } else {
            roles.join(", ")
        }
    );
    let open = open_annotations(graph, cid);
    let (status, why) = status(fact.value, measure.map(|(_, _, d)| d), &roles, open);
    println!("status    {status} ({why}; a convention, Section 5.9.7)");
}

fn value_text(cell: &publet_graph::Cell) -> String {
    match cell {
        publet_graph::Cell::Null => "null: not yet known".to_owned(),
        other => crate::render::cell_text(other),
    }
}

/// The roles of a claim's evidence, in order, each once.
fn roles(graph: &Graph, cid: &Cid) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(Value::Array(entries)) = graph.object(cid).and_then(|o| o.body().get("evidence")) {
        for role in entries
            .iter()
            .filter_map(|e| e.get("role").and_then(Value::as_text))
        {
            if !out.iter().any(|r| r == role) {
                out.push(role.to_owned());
            }
        }
    }
    out
}

/// How many annotations leave the claim open (Section 5.9.7): assessments
/// finding it unsupported, refuted or undetermined, verdicts denying it,
/// and disputes of it that no resolution settles.
fn open_annotations(graph: &Graph, cid: &Cid) -> usize {
    let field = |a: &publet_graph::Annotation, key: &str| -> Option<String> {
        graph
            .object(a.cid())?
            .body()
            .get("value")?
            .get(key)?
            .as_text()
            .map(str::to_owned)
    };
    let judged = graph
        .annotations_of(cid)
        .into_iter()
        .filter(|a| match a.kind() {
            "assessment" => matches!(
                field(a, "verdict").as_deref(),
                Some("unsupported" | "refuted" | "undetermined")
            ),
            "verdict" => field(a, "finding").as_deref() == Some("deny"),
            _ => false,
        })
        .count();
    let disputes = graph
        .incoming(RelationKind::Disputes, cid)
        .iter()
        .filter_map(|d| d.parse::<Cid>().ok())
        .filter(|d| {
            graph
                .annotations_of(d)
                .iter()
                .all(|a| a.kind() != "resolution")
        })
        .count();
    judged + disputes
}

/// The status a fact's data implies, by the convention of Section 5.9.7,
/// with the reason in words.
fn status(
    value: &publet_graph::Cell,
    describes: Option<Describes>,
    roles: &[String],
    open: usize,
) -> (&'static str, String) {
    if matches!(value, publet_graph::Cell::Null) {
        return ("to-verify", "no value is stated yet".to_owned());
    }
    if open > 0 {
        return ("to-verify", format!("{open} annotation(s) leave it open"));
    }
    let grounded = roles.iter().any(|r| {
        matches!(
            r.as_str(),
            "measurement" | "statement" | "quote" | "derivation"
        )
    });
    match describes {
        Some(Describes::Exact) if grounded => (
            "confirmed",
            "an exact value, measured, stated, quoted or derived, and nothing open".to_owned(),
        ),
        Some(Describes::Exact) => (
            "to-verify",
            "an exact value, but no evidence measured, stated, quoted or derived it".to_owned(),
        ),
        Some(_) => (
            "estimate",
            "its measure states uncertainty or variation".to_owned(),
        ),
        None => (
            "estimate",
            "it states no measure, so no certainty".to_owned(),
        ),
    }
}
