//! One-shot migration of an object directory to the `claim.*` type names.
//!
//! Renaming `type` changes an object's canonical bytes, and therefore its
//! CID, and therefore every reference to it. Nothing in the corpus is
//! signed, so no signature is invalidated and the rewrite is purely
//! mechanical -- but it has to happen in dependency order, because an
//! object's new CID cannot be computed until every CID it names has one.
//!
//! Content addressing guarantees that order exists: an object can only
//! ever have referenced CIDs that already existed when it was composed, so
//! the corpus is necessarily acyclic. The loop below finds that order by
//! repeatedly migrating whatever is fully resolved, and reports a cycle
//! rather than looping forever if that assumption is ever wrong.
//!
//! References are found by walking the whole decoded tree for text that
//! parses as a CID the corpus holds, rather than by enumerating the fields
//! of each type. Field enumeration would silently miss a reference the day
//! a new annotation kind carries one.
//!
//! Prose is deliberately left alone. Migrating bytes is not the same as
//! revising assertions, and a claim whose text defines a term this rename
//! retired needs a human to re-author it, not a search and replace.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use publet_core::cbor::{self, Value};
use publet_core::{Cid, HashAlg};

const EXIT_USAGE: u8 = 2;
const EXIT_FAILED: u8 = 1;

/// Old type name to new.
fn migrated_type(old: &str) -> Option<&'static str> {
    match old {
        "publet" => Some("claim.prose"),
        "rel" => Some("claim.relation"),
        "ann" => Some("claim.annotation"),
        _ => None,
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: pub-migrate --dir=DIR [--out=DIR] [--dry-run]");
    eprintln!();
    eprintln!("  --dir      the object directory to read");
    eprintln!("  --out      where to write migrated objects (default: in place)");
    eprintln!("  --dry-run  report the mapping without writing anything");
    ExitCode::from(EXIT_USAGE)
}

/// The filename an object is stored under: its CID with `:` replaced.
fn file_name(cid: &Cid) -> String {
    format!("{}.cbor", cid.to_string().replace(':', "_"))
}

/// Every CID-shaped text in the tree that the corpus actually holds.
fn referenced(value: &Value, known: &BTreeMap<String, Vec<u8>>, out: &mut Vec<String>) {
    match value {
        Value::Text(text) => {
            if known.contains_key(text) {
                out.push(text.clone());
            }
        }
        Value::Array(items) => {
            for item in items {
                referenced(item, known, out);
            }
        }
        Value::Map(entries) => {
            for item in entries.values() {
                referenced(item, known, out);
            }
        }
        _ => {}
    }
}

/// Rewrite every CID in the tree that has already been migrated.
fn rewrite_cids(value: &Value, mapping: &BTreeMap<String, String>) -> Value {
    match value {
        Value::Text(text) => match mapping.get(text) {
            Some(new) => Value::Text(new.clone()),
            None => Value::Text(text.clone()),
        },
        Value::Array(items) => {
            Value::Array(items.iter().map(|i| rewrite_cids(i, mapping)).collect())
        }
        Value::Map(entries) => Value::Map(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), rewrite_cids(v, mapping)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Section 5.5's evidence vocabulary renamed `publet` to `claim`.
///
/// Scoped deliberately to `evidence[].kind` rather than applied to the
/// tree: the same string appears in claim prose, where rewriting it would
/// be editing an assertion rather than migrating a format.
fn migrate_evidence_kinds(body: &mut BTreeMap<String, Value>) {
    let Some(Value::Array(items)) = body.get("evidence") else {
        return;
    };
    let migrated: Vec<Value> = items
        .iter()
        .map(|item| {
            let Value::Map(entry) = item else {
                return item.clone();
            };
            let mut entry = entry.clone();
            if entry.get("kind") == Some(&Value::Text("publet".to_owned())) {
                entry.insert("kind".to_owned(), Value::Text("claim".to_owned()));
            }
            Value::Map(entry)
        })
        .collect();
    body.insert("evidence".to_owned(), Value::Array(migrated));
}

/// Apply every rename to one decoded object, returning its new bytes.
fn migrate_one(value: &Value, mapping: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let rewritten = rewrite_cids(value, mapping);
    let Value::Map(mut map) = rewritten else {
        return Err("object is not a map".to_owned());
    };

    let Some(Value::Text(kind)) = map.get("type") else {
        return Err("object has no `type`".to_owned());
    };
    if let Some(new_kind) = migrated_type(kind) {
        map.insert("type".to_owned(), Value::Text(new_kind.to_owned()));
    }

    if let Some(Value::Map(body)) = map.get("body") {
        let mut body = body.clone();
        migrate_evidence_kinds(&mut body);
        map.insert("body".to_owned(), Value::Map(body));
    }

    Ok(cbor::encode(&Value::Map(map)))
}

fn load(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut objects = BTreeMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|e| e != "cbor") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        objects.insert(cid.to_string(), bytes);
    }
    Ok(objects)
}

/// Migrate every object, in an order where each one's references already
/// have new identifiers.
///
/// # Errors
///
/// Returns a message if the corpus contains a reference cycle, which
/// content addressing should make impossible, or if an object is malformed.
fn migrate_all(
    decoded: &BTreeMap<String, Value>,
    objects: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<(String, String, Vec<u8>)>, String> {
    let mut mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut pending: Vec<String> = decoded.keys().cloned().collect();
    let mut migrated: Vec<(String, String, Vec<u8>)> = Vec::new();

    while !pending.is_empty() {
        let mut progressed = false;
        let mut still_pending = Vec::new();

        for cid in pending {
            let value = &decoded[&cid];
            let mut refs = Vec::new();
            referenced(value, objects, &mut refs);
            // Self-reference is impossible without a hash preimage, but an
            // object naming itself would deadlock the loop, so skip it.
            let ready = refs
                .iter()
                .all(|r| *r == cid || mapping.contains_key(r.as_str()));
            if !ready {
                still_pending.push(cid);
                continue;
            }
            let bytes = migrate_one(value, &mapping).map_err(|e| format!("{cid}: {e}"))?;
            let new_cid = Cid::of(&bytes, HashAlg::Sha2_256);
            mapping.insert(cid.clone(), new_cid.to_string());
            migrated.push((cid, new_cid.to_string(), bytes));
            progressed = true;
        }

        if !progressed {
            let sample: Vec<&str> = still_pending.iter().take(5).map(String::as_str).collect();
            return Err(format!(
                "cannot order {} object(s): each names another that is not yet \
                 migrated, which means the corpus contains a reference cycle\n  {}",
                still_pending.len(),
                sample.join("\n  ")
            ));
        }
        pending = still_pending;
    }
    Ok(migrated)
}

fn write_all(out: &Path, migrated: &[(String, String, Vec<u8>)]) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for (old, new, bytes) in migrated {
        let new_cid = new
            .parse::<Cid>()
            .map_err(|_| format!("{new}: not a CID"))?;
        std::fs::write(out.join(file_name(&new_cid)), bytes)
            .map_err(|e| format!("{}: {e}", out.display()))?;
        // In-place migration leaves the old file behind under its own name;
        // removing it is the caller's decision, not this tool's.
        if old != new {
            println!("{old} {new}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut dry_run = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(v));
        } else if arg == "--dry-run" {
            dry_run = true;
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    let Some(dir) = dir else {
        return usage();
    };
    let out = out.unwrap_or_else(|| dir.clone());

    let objects = match load(&dir) {
        Ok(objects) => objects,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_FAILED);
        }
    };

    // Decode once; the loop below inspects each object repeatedly.
    let mut decoded: BTreeMap<String, Value> = BTreeMap::new();
    for (cid, bytes) in &objects {
        match cbor::decode(bytes) {
            Ok(value) => {
                decoded.insert(cid.clone(), value);
            }
            Err(e) => {
                eprintln!("{cid}: {e}");
                return ExitCode::from(EXIT_FAILED);
            }
        }
    }

    let migrated = match migrate_all(&decoded, &objects) {
        Ok(migrated) => migrated,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_FAILED);
        }
    };

    let unchanged = migrated.iter().filter(|(old, new, _)| old == new).count();
    eprintln!(
        "migrated {} object(s); {unchanged} unchanged",
        migrated.len()
    );

    if dry_run {
        for (old, new, _) in &migrated {
            println!("{old} {new}");
        }
        return ExitCode::SUCCESS;
    }

    if let Err(e) = write_all(&out, &migrated) {
        eprintln!("{e}");
        return ExitCode::from(EXIT_FAILED);
    }
    ExitCode::SUCCESS
}
