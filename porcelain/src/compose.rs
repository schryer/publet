//! `pub compose`: build a claim and add it to the workspace.
//!
//! Scope is required, not optional. An assertion that states its own
//! validity conditions does not drift, because nothing was left implicit to
//! drift (R4); one that does not is a different assertion every time it is
//! read. The command therefore refuses to build without one rather than
//! supplying a default that would be a claim the author never made.

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Class, check};

use crate::workspace::Workspace;

/// Compose a claim.
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
    let mut cites: Vec<String> = Vec::new();
    let mut cite_notes: Vec<String> = Vec::new();

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
        } else if let Some(v) = arg.strip_prefix("--cite=") {
            cites.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--cite-note=") {
            cite_notes.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    check_cite_notes(&cites, &cite_notes)?;

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
             `procedural` claim describing how the observation may be \
             repeated. A measurement without one is a report of an \
             experience (Section 5.5)"
            .to_owned());
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let bytes = Object::builder("claim.prose", &author)
        .created(&created)
        .field("class", Value::Text(class_id.clone()))
        .field("lang", Value::Text(lang))
        .field("content", Value::Text(content))
        .field("scope", scope_value(&scope))
        .field(
            "depends",
            Value::Array(depends.iter().map(|d| Value::Text(d.clone())).collect()),
        )
        .field(
            "evidence",
            evidence_for(method.as_deref(), &cites, &cite_notes),
        )
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
    let claim = publet_graph::ProseClaim::from_object(cid.clone(), verified.object())
        .map_err(|e| e.to_string())?;
    let findings = check(&claim);
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

/// A note is paired with the citation at the same position; a citation may
/// be given without one, but a note cannot outnumber the citations it
/// annotates -- there would be nothing left to say it about.
fn check_cite_notes(cites: &[String], cite_notes: &[String]) -> Result<(), String> {
    if cite_notes.len() > cites.len() {
        return Err(format!(
            "{} --cite-note value(s) given but only {} --cite value(s); \
             each --cite-note pairs with the --cite given at the same position",
            cite_notes.len(),
            cites.len()
        ));
    }
    Ok(())
}

/// The evidence list (Section 5.5): a `method` entry when one was named,
/// followed by a `citation` entry for each `--cite`.
///
/// A citation names a source lying outside this corpus's integrity
/// guarantees, so `kind` is `"external"` and `ref` is whatever the author
/// gave -- a DOI, a URL, a stable identifier -- displayed as unverified,
/// per Section 5.5. This is deliberately the only evidence role `pub
/// compose` writes besides `method`: `measurement`, `derivation`, and
/// `replication` name evidence a claim's own author produced, which is a
/// different act from citing someone else's, and conflating them here
/// would let a citation masquerade as one of the roles Section 11.4
/// weighs more heavily.
fn evidence_for(method: Option<&str>, cites: &[String], cite_notes: &[String]) -> Value {
    let mut entries = Vec::new();

    if let Some(method) = method {
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text("claim".into()));
        entry.insert("role".to_owned(), Value::Text("method".into()));
        entry.insert("ref".to_owned(), Value::Text(method.to_owned()));
        entries.push(Value::Map(entry));
    }

    for (i, cite) in cites.iter().enumerate() {
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text("external".into()));
        entry.insert("role".to_owned(), Value::Text("citation".into()));
        entry.insert("ref".to_owned(), Value::Text(cite.clone()));
        if let Some(note) = cite_notes.get(i) {
            entry.insert("note".to_owned(), Value::Text(note.clone()));
        }
        entries.push(Value::Map(entry));
    }

    Value::Array(entries)
}

/// Offer a new object to the loader against everything already held, and
/// store it only if the loader accepts.
///
/// The rules that decide whether an object may enter a graph live in
/// `publet_graph` (Section 5.2's class rules, Section 6's acyclicity), and
/// checking them a second time here is how the two implementations would
/// come to disagree. `refused` names what the caller was trying to do, so
/// the message reads in terms of the command the author ran.
///
/// # Errors
///
/// Returns a message if the loader refuses the object or the store fails.
pub(crate) fn offer_then_store(
    store: &publet_store::Store,
    cid: &Cid,
    bytes: &[u8],
    refused: &str,
) -> Result<(), String> {
    let mut objects = vec![(cid.clone(), bytes.to_vec())];
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(held) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(held_bytes)) = store.get(&held) {
            objects.push((held, held_bytes));
        }
    }
    publet_graph::load::from_objects(objects)
        .map_err(|e| format!("{refused}, so it was not stored: {e}"))?;
    store.put(cid, bytes).map_err(|e| e.to_string())
}

/// The scope every claim states (R4, Section 5.3).
///
/// One shape for all three grammars: `pub compose`, `pub relate`, and
/// `pub annotate` each state the conditions their author asserts under,
/// and `"unconditional"` is the explicit way of saying there are none.
pub(crate) fn scope_value(domain: &str) -> Value {
    let mut map = std::collections::BTreeMap::new();
    map.insert("domain".to_owned(), Value::Text(domain.to_owned()));
    map.insert("conditions".to_owned(), Value::Array(Vec::new()));
    Value::Map(map)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(value: &Value) -> &[Value] {
        let Value::Array(entries) = value else {
            panic!("evidence is not an array");
        };
        entries
    }

    fn field<'a>(entry: &'a Value, name: &str) -> Option<&'a str> {
        let Value::Map(map) = entry else {
            panic!("evidence entry is not a map");
        };
        map.get(name).and_then(Value::as_text)
    }

    #[test]
    fn no_method_and_no_citations_is_an_empty_list() {
        assert_eq!(evidence_for(None, &[], &[]), Value::Array(Vec::new()));
    }

    #[test]
    fn a_citation_is_external_and_carries_the_citation_role() {
        let cites = vec!["https://doi.org/10.1136/bmj.b2680".to_owned()];
        let value = evidence_for(None, &cites, &[]);
        let entry = &entries(&value)[0];
        assert_eq!(field(entry, "kind"), Some("external"));
        assert_eq!(field(entry, "role"), Some("citation"));
        assert_eq!(field(entry, "ref"), Some(cites[0].as_str()));
        assert_eq!(field(entry, "note"), None);
    }

    #[test]
    fn a_note_pairs_with_the_citation_at_its_position() {
        let cites = vec!["A".to_owned(), "B".to_owned()];
        let notes = vec!["note for A".to_owned()];
        let value = evidence_for(None, &cites, &notes);
        let list = entries(&value);
        assert_eq!(field(&list[0], "ref"), Some("A"));
        assert_eq!(field(&list[0], "note"), Some("note for A"));
        assert_eq!(field(&list[1], "ref"), Some("B"));
        assert_eq!(field(&list[1], "note"), None);
    }

    #[test]
    fn method_precedes_citations_and_keeps_its_own_kind_and_role() {
        let cites = vec!["https://example.org/case".to_owned()];
        let value = evidence_for(Some("pub:sha2-256:abc"), &cites, &[]);
        let list = entries(&value);
        assert_eq!(list.len(), 2);
        assert_eq!(field(&list[0], "kind"), Some("claim"));
        assert_eq!(field(&list[0], "role"), Some("method"));
        assert_eq!(field(&list[0], "ref"), Some("pub:sha2-256:abc"));
        assert_eq!(field(&list[1], "role"), Some("citation"));
    }
}
