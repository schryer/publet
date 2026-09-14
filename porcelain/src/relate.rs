//! `pub relate`: author a relation between two objects.
//!
//! Relations are the edges, and until now nothing in the toolchain could
//! write one: the graph read nine object kinds and the porcelain composed
//! two. A store of publets with no relations is a list, not a graph.
//!
//! Anyone may relate any two objects (R6), including objects they did not
//! author. What the command will not do is write an edge that makes the
//! graph invalid -- and rather than reimplement the acyclicity rule here,
//! where it could drift from the loader's, it builds the object, offers it
//! to the loader alongside everything already held, and stores it only if
//! the loader accepts. There is one implementation of the rule and this is
//! a caller of it.

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{RelationKind, load};

use crate::workspace::Workspace;

/// Author a relation.
///
/// # Errors
///
/// Returns a message if a required field is missing, an identifier does not
/// parse, the kind is unknown, or the edge would make the graph invalid.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut kind = None;
    let mut from = None;
    let mut to = None;
    let mut aspect: Option<String> = None;
    let mut note: Option<String> = None;
    let mut method: Option<String> = None;
    let mut fidelity: Option<String> = None;
    let mut created = "2026-09-12T00:00:00Z".to_owned();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--kind=") {
            kind = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--from=") {
            from = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--to=") {
            to = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--aspect=") {
            aspect = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--note=") {
            note = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--method=") {
            method = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--fidelity=") {
            fidelity = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    let kind_id = kind.ok_or(
        "--kind is required: supersedes, translates, depends, disputes, \
         supports, equivalent, retracts, derived-from, or delegates\n\
         `translates` also requires --method and --fidelity",
    )?;
    let parsed_kind =
        RelationKind::from_id(&kind_id).ok_or(format!("unknown relation kind: {kind_id}"))?;

    let from = from.ok_or("--from is required: the CID the edge starts at")?;
    let to = to.ok_or("--to is required: the CID the edge points to")?;
    let from_cid: Cid = from
        .parse()
        .map_err(|_| format!("--from is not a CID: {from}"))?;
    let to_cid: Cid = to.parse().map_err(|_| format!("--to is not a CID: {to}"))?;

    // Section 18.4: a `translates` edge carries how the rendering was made
    // and how faithful it is. Without them the edge says two things express
    // one claim while hiding that one of them is lossy, which is the fact a
    // reader most needs.
    if parsed_kind == RelationKind::Translates {
        check_translation_fields(method.as_deref(), fidelity.as_deref())?;
    }

    if from_cid == to_cid {
        return Err(format!(
            "--from and --to are the same object. A self-edge asserts \
             nothing under any relation kind, and under `{kind_id}` it is a \
             cycle of length one"
        ));
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let mut builder = Object::builder("rel", &author)
        .created(&created)
        .field("kind", Value::Text(kind_id.clone()))
        .field("from", Value::Text(from.clone()))
        .field("to", Value::Text(to.clone()));
    if let Some(aspect) = &aspect {
        builder = builder.field("aspect", Value::Text(aspect.clone()));
    }
    if let Some(note) = &note {
        builder = builder.field("note", Value::Text(note.clone()));
    }
    if let Some(method) = &method {
        builder = builder.field("method", Value::Text(method.clone()));
    }
    if let Some(fidelity) = &fidelity {
        builder = builder.field("fidelity", Value::Text(fidelity.clone()));
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);

    // Offer the edge to the loader before storing it. An acyclic kind that
    // closes a cycle is refused here for the same reason, by the same code,
    // that would refuse it at load time (Section 6).
    let mut objects = vec![(cid.clone(), bytes.clone())];
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(held) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(held_bytes)) = store.get(&held) {
            objects.push((held, held_bytes));
        }
    }
    load::from_objects(objects).map_err(|e| {
        format!("this edge would make the graph invalid, so it was not stored: {e}")
    })?;

    store.put(&cid, &bytes).map_err(|e| e.to_string())?;
    println!("{cid}");

    // An acyclic kind carries a consequence the author should see at the
    // moment they create it, not discover later from an evaluation.
    if parsed_kind.is_acyclic() {
        eprintln!();
        eprintln!(
            "`{kind_id}` is acyclic and constitutes lineage: nothing is \
             overwritten, but readers following {to} will now find {from} \
             ahead of it."
        );
    }
    Ok(())
}

/// Validate the `method`/`fidelity` pair a `translates` edge requires.
fn check_translation_fields(method: Option<&str>, fidelity: Option<&str>) -> Result<(), String> {
    const METHODS: [&str; 3] = ["human", "machine", "machine-post-edited"];
    const FIDELITIES: [&str; 3] = ["literal", "idiomatic", "adapted"];
    let m = method.ok_or_else(|| {
        format!(
            "--method is required for `translates`: one of {}",
            METHODS.join(", ")
        )
    })?;
    if !METHODS.contains(&m) {
        return Err(format!("unknown translation method: {m}"));
    }
    let f = fidelity.ok_or_else(|| {
        format!(
            "--fidelity is required for `translates`: one of {}",
            FIDELITIES.join(", ")
        )
    })?;
    if !FIDELITIES.contains(&f) {
        return Err(format!("unknown fidelity: {f}"));
    }
    Ok(())
}
