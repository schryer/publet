//! `pub compose`: build a publet and add it to the workspace.
//!
//! Scope is required, not optional. An assertion that states its own
//! validity conditions does not drift, because nothing was left implicit to
//! drift (R4); one that does not is a different assertion every time it is
//! read. The command therefore refuses to build without one rather than
//! supplying a default that would be a claim the author never made.

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Class, check};

use crate::workspace::Workspace;

/// Compose a publet.
///
/// # Errors
///
/// Returns a message if a required field is missing or the class is unknown.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut class = None;
    let mut content = None;
    let mut scope = None;
    let mut lang = "en".to_owned();
    let mut depends: Vec<String> = Vec::new();
    let mut method: Option<String> = None;
    let mut created = "2026-09-12T00:00:00Z".to_owned();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--class=") {
            class = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--content=") {
            content = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--scope=") {
            scope = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--lang=") {
            v.clone_into(&mut lang);
        } else if let Some(v) = arg.strip_prefix("--method=") {
            method = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--depends=") {
            depends.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    let class_id = class.ok_or(
        "--class is required: formal, empirical, attributive, definitional, \
         normative, expressive, archival, or procedural",
    )?;
    let parsed_class =
        Class::from_id(&class_id).ok_or(format!("unknown claim class: {class_id}"))?;
    let content = content.ok_or("--content is required")?;
    let scope = scope.ok_or(
        "--scope is required. State the conditions you assert this under, or \
         \"unconditional\" if you really mean that (Section 5.3)",
    )?;

    // Section 5.5: an empirical claim must name a method others can
    // execute. Supplying a placeholder would assert a reproducibility the
    // author never offered, so the command refuses instead.
    if parsed_class == Class::Empirical && method.is_none() {
        return Err("--method is required for an empirical claim: the CID of a \
             `procedural` publet describing how the observation may be \
             repeated. A measurement without one is a report of an \
             experience (Section 5.5)"
            .to_owned());
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let mut scope_map = std::collections::BTreeMap::new();
    scope_map.insert("domain".to_owned(), Value::Text(scope.clone()));
    scope_map.insert("conditions".to_owned(), Value::Array(Vec::new()));

    let bytes = Object::builder("publet", &author)
        .created(&created)
        .field("class", Value::Text(class_id.clone()))
        .field("lang", Value::Text(lang))
        .field("content", Value::Text(content))
        .field("scope", Value::Map(scope_map))
        .field(
            "depends",
            Value::Array(depends.iter().map(|d| Value::Text(d.clone())).collect()),
        )
        .field("evidence", evidence_for(method.as_deref()))
        .build()
        .map_err(|e| e.to_string())?;

    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    store.put(&cid, &bytes).map_err(|e| e.to_string())?;

    println!("{cid}");

    // Structural findings are warnings, not validity rules: the pressure
    // belongs on the author now, when the fix is cheap.
    let verified = Object::parse(&bytes)
        .map_err(|e| e.to_string())?
        .verify(&cid)
        .map_err(|e| e.to_string())?;
    let publet = publet_graph::Publet::from_object(cid.clone(), verified.object())
        .map_err(|e| e.to_string())?;
    let findings = check(&publet);
    if !findings.is_empty() {
        eprintln!();
        eprintln!("structural findings (warnings, not errors):");
        for finding in findings {
            eprintln!("  {}: {}", finding.test, finding.detail);
        }
    }

    if parsed_class == Class::Empirical {
        eprintln!();
        eprintln!("This is an empirical claim. It cannot reach `accepted` on");
        eprintln!("endorsement alone: it needs a method others can execute and");
        eprintln!("independent reproductions of it (Section 11.4.1).");
    }
    Ok(())
}

/// The evidence list, carrying the method entry when one was named.
fn evidence_for(method: Option<&str>) -> Value {
    let Some(method) = method else {
        return Value::Array(Vec::new());
    };
    let mut entry = std::collections::BTreeMap::new();
    entry.insert("kind".to_owned(), Value::Text("publet".into()));
    entry.insert("role".to_owned(), Value::Text("method".into()));
    entry.insert("ref".to_owned(), Value::Text(method.to_owned()));
    Value::Array(vec![Value::Map(entry)])
}

/// A starting policy trusting only the workspace's own key.
///
/// Deliberately minimal and deliberately announced. A policy with no roots
/// assigns zero weight to everything, so one is needed to evaluate at all;
/// one that trusts only yourself is honest about being a placeholder.
///
/// # Errors
///
/// Returns a message if the object cannot be built.
pub(crate) fn default_policy(author: &Cid) -> Result<Vec<u8>, String> {
    let mut root = std::collections::BTreeMap::new();
    root.insert("key".to_owned(), Value::Text(author.to_string()));
    root.insert("weight".to_owned(), Value::Uint(1000));

    Object::builder("policy", &author.to_string())
        .created("2026-09-12T00:00:00Z")
        .field("roots", Value::Array(vec![Value::Map(root)]))
        .field("damping", Value::Uint(850_000))
        .field("iterations", Value::Uint(20))
        .field("tau", Value::Uint(660_000))
        .field("delta_max", Value::Uint(290_000))
        .field("replication_floor", Value::Uint(2))
        .field("independence_distance", Value::Uint(2))
        .build()
        .map_err(|e| e.to_string())
}
