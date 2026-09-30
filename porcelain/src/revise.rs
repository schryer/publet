//! `pub revise OLD`: publish the next version of a claim or document and
//! the `supersedes` edge to it, in one step.
//!
//! Revision is a new object plus `supersedes` (R1), and until now that was
//! two commands whose second a program could forget, leaving a new version
//! no lineage leads to. This command does both, and stores neither unless
//! both are admissible.
//!
//! A claim is revised by starting from the old one's body and applying
//! only the flags given -- the same flags `pub compose` takes -- so a
//! gather step that re-reads its inputs passes `--data` and `--source` and
//! nothing else. A document is revised from a manifest, as `pub doc` reads.
//!
//! When the result is the old body exactly, nothing is published and the
//! old identifier is printed: Section 5.8 asks that a new version exist
//! only when the values or their sources change, and a build that re-runs
//! with the same inputs should leave the lineage as it found it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::Class;
use publet_store::Store;

use crate::compose::{self, ClaimFlags};
use crate::workspace::Workspace;

/// Revise an object.
///
/// Prints the new object's identifier and then the `supersedes`
/// relation's, one per line; or, when nothing changed, only the old
/// identifier.
///
/// # Errors
///
/// Returns a message if the old object is not held or cannot be revised
/// this way, a flag is invalid, or the loader refuses either new object.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut old = None;
    let mut manifest: Option<PathBuf> = None;
    let mut flags = ClaimFlags::default();
    for arg in args {
        if let Some(v) = arg.strip_prefix("--manifest=") {
            manifest = Some(PathBuf::from(v));
        } else if !flags.take(arg)? {
            if arg.starts_with("--") {
                return Err(format!("unknown argument: {arg}"));
            }
            old = Some(arg.clone());
        }
    }

    let old_text = old.ok_or("the identifier of the version to revise is required")?;
    let old_cid: Cid = old_text
        .parse()
        .map_err(|_| format!("not a CID: {old_text}"))?;
    let old_bytes = store
        .get(&old_cid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("not in your replica: {old_cid}"))?;
    let old_object = Object::parse(&old_bytes)
        .map_err(|e| e.to_string())?
        .verify(&old_cid)
        .map_err(|e| e.to_string())?
        .into_inner();

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let (body, created) = match old_object.kind() {
        "claim.prose" => {
            if manifest.is_some() {
                return Err("--manifest revises a document; this is a claim, \
                            revised with the flags `pub compose` takes"
                    .to_owned());
            }
            compose::check_cite_notes(&flags.cites, &flags.cite_notes)?;
            (revise_claim(&store, &old_object, &flags)?, flags.created)
        }
        "doc" => {
            let path = manifest.ok_or(
                "--manifest=FILE is required to revise a document: the next \
                 version's structure, in the form `pub doc` reads",
            )?;
            if flags_other_than_created(&flags) {
                return Err("a document is revised from its manifest alone; \
                            claim flags do not apply to it"
                    .to_owned());
            }
            let (body, manifest_created) = crate::document::body_from_manifest(&path)?;
            (body, flags.created.or(manifest_created))
        }
        other => {
            return Err(format!(
                "{old_cid} is a `{other}`; `pub revise` publishes new versions \
                 of prose claims and documents"
            ));
        }
    };

    if &body == old_object.body() {
        println!("{old_cid}");
        eprintln!("unchanged: the new version would be identical to {old_cid},");
        eprintln!("so none was published (Section 5.8).");
        return Ok(());
    }

    let created = created.unwrap_or_else(|| compose::DEFAULT_CREATED.to_owned());
    let (cid, bytes) = if old_object.kind() == "doc" {
        crate::document::build_document(&author, &created, body)?
    } else {
        let (cid, bytes, claim) = compose::build_claim(&author, &created, body)?;
        compose::warn_about(&claim);
        (cid, bytes)
    };

    let relation = supersedes(&author, &created, &cid, &old_cid)?;
    let relation_cid = Cid::of(&relation, HashAlg::Sha2_256);

    // Offer the new version first so the relation's `from` resolves, but
    // offer the relation before telling anyone about either: a version no
    // lineage leads to is the failure this command exists to prevent.
    compose::offer_then_store(&store, &cid, &bytes, "the new version was refused")?;
    if let Err(e) = compose::offer_then_store(
        &store,
        &relation_cid,
        &relation,
        "the supersedes edge was refused",
    ) {
        let _ = store.remove(&cid);
        return Err(e);
    }

    println!("{cid}");
    println!("{relation_cid}");

    // Section 6: `supersedes` is felicitous only from a key that signed
    // the target. From any other key it is a proposal to replace, which is
    // permitted and which a reader must be able to tell apart.
    if old_object.author().to_string() != author {
        eprintln!();
        eprintln!("{old_cid} was authored by another key, so this `supersedes`");
        eprintln!("is a proposal to replace it, not a revision of it (Section 6).");
    }
    Ok(())
}

/// The next version of a claim: the old body with only the given flags
/// applied.
///
/// Evidence is replaced by role: `--method` replaces the method entry,
/// `--cite` the citations, `--source` the sources, and entries of any role
/// not given are kept. `depends` is replaced when `--depends` is given;
/// otherwise the old list is kept, less any claim that was a source and no
/// longer is. Either way every claim now named as a source is added, as
/// Section 5.5 requires.
fn revise_claim(
    store: &Store,
    old: &Object,
    flags: &ClaimFlags,
) -> Result<BTreeMap<String, Value>, String> {
    let mut body = old.body().clone();

    if let Some(class) = &flags.class {
        Class::from_id(class).ok_or(format!("unknown claim class: {class}"))?;
        body.insert("class".to_owned(), Value::Text(class.clone()));
    }
    if let Some(content) = &flags.content {
        body.insert("content".to_owned(), Value::Text(content.clone()));
    }
    if let Some(lang) = &flags.lang {
        body.insert("lang".to_owned(), Value::Text(lang.clone()));
    }
    if let Some(scope) = &flags.scope {
        body.insert("scope".to_owned(), compose::scope_value(scope));
    }

    match (&flags.data, flags.no_data) {
        (Some(_), true) => return Err("--data and --no-data contradict each other".to_owned()),
        (Some(path), false) => {
            body.insert(
                "data".to_owned(),
                crate::payload::data_from_file(store, path)?,
            );
        }
        (None, true) => {
            body.remove("data");
        }
        (None, false) => {}
    }

    let old_entries = match body.get("evidence") {
        Some(Value::Array(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    let role_of = |entry: &Value| {
        entry
            .get("role")
            .and_then(Value::as_text)
            .map(str::to_owned)
    };
    let replaced = |role: Option<&str>| match role {
        Some("method") => flags.method.is_some(),
        Some("citation") => !flags.cites.is_empty(),
        Some("source") => !flags.sources.is_empty(),
        _ => false,
    };
    let old_source_claims: Vec<String> = old_entries
        .iter()
        .filter(|e| role_of(e).as_deref() == Some("source"))
        .filter(|e| e.get("kind").and_then(Value::as_text) == Some("claim"))
        .filter_map(|e| e.get("ref").and_then(Value::as_text).map(str::to_owned))
        .collect();

    let mut evidence: Vec<Value> = old_entries
        .into_iter()
        .filter(|e| !replaced(role_of(e).as_deref()))
        .collect();
    if let Value::Array(added) =
        compose::evidence_for(flags.method.as_deref(), &flags.cites, &flags.cite_notes)
    {
        evidence.extend(added);
    }
    let (source_entries, source_claims) = compose::source_entries(store, &flags.sources)?;
    evidence.extend(source_entries);
    body.insert("evidence".to_owned(), Value::Array(evidence));

    let mut depends: Vec<String> = if flags.depends.is_empty() {
        let old_depends = match body.get("depends") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_text)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        if flags.sources.is_empty() {
            old_depends
        } else {
            old_depends
                .into_iter()
                .filter(|d| !old_source_claims.contains(d))
                .collect()
        }
    } else {
        flags.depends.clone()
    };
    compose::add_missing(&mut depends, &source_claims);
    body.insert("depends".to_owned(), compose::text_array(&depends));

    Ok(body)
}

fn flags_other_than_created(flags: &ClaimFlags) -> bool {
    flags.class.is_some()
        || flags.content.is_some()
        || flags.scope.is_some()
        || flags.lang.is_some()
        || !flags.depends.is_empty()
        || flags.method.is_some()
        || !flags.cites.is_empty()
        || !flags.sources.is_empty()
        || flags.data.is_some()
        || flags.no_data
}

/// The `supersedes` relation from the new version to the old.
///
/// Unconditional in scope: what it asserts is that one object replaces
/// another, and the conditions each was asserted under are in each.
fn supersedes(author: &str, created: &str, new: &Cid, old: &Cid) -> Result<Vec<u8>, String> {
    Object::builder("claim.relation", author)
        .created(created)
        .field("kind", Value::Text("supersedes".to_owned()))
        .field("from", Value::Text(new.to_string()))
        .field("to", Value::Text(old.to_string()))
        .field("scope", compose::scope_value("unconditional"))
        .build()
        .map_err(|e| e.to_string())
}
