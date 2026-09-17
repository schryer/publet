#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]
//! One-shot migration of an object directory to the current object format.
//!
//! This binary carries whatever migration is currently being run. The
//! machinery is durable -- dependency ordering, CID rewriting across
//! several directories, skipping kinds whose identity rests on a set root
//! -- and the transformation in `migrate_one` is replaced each time. Past
//! migrations live in git history rather than accumulating here as dead
//! branches nothing will take again.
//!
//! **Current migration: R4 and R5 applied to every claim.** Relations and
//! annotations gain the `scope` every assertion states, and the field
//! inside an assessment naming what a judgement rests on becomes
//! `grounds`, so that `basis` has one meaning -- the state an author was
//! looking at (Section 4.6).
//!
//! Renaming or adding a body field changes an object's canonical bytes,
//! and therefore its CID, and therefore every reference to it. Nothing in
//! this corpus is signed, so no signature is invalidated and the rewrite
//! is mechanical -- but it has to happen in dependency order, because an
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
//! `domain` and `generation` objects are skipped. A manifest's `snapshot`
//! is a membership root over member CIDs -- a hash of the set, not a
//! reference to an object -- so rewriting the members would leave the root
//! naming a set that no longer exists. Those are re-authored by
//! `pub domain` against the migrated directory, which recomputes the root
//! by construction.
//!
//! Several directories MAY be given. They are migrated against one shared
//! index, so an object in a domain directory that names an object held
//! only in the main one still resolves; each object is written back to
//! every directory it came from.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::cbor::{self, Value};
use publet_core::{Cid, HashAlg};

const EXIT_USAGE: u8 = 2;
const EXIT_FAILED: u8 = 1;

/// Types whose identity depends on a set root rather than on references,
/// and which are therefore re-authored rather than migrated.
const REAUTHORED: [&str; 2] = ["domain", "generation"];

fn usage() -> ExitCode {
    eprintln!("usage: pub-migrate --dir=DIR... [--dry-run]");
    eprintln!();
    eprintln!("  --dir      an object directory to migrate, repeatable");
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

/// Apply every rename to one decoded object, returning its new bytes.
fn migrate_one(value: &Value, mapping: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let rewritten = rewrite_cids(value, mapping);
    let Value::Map(mut map) = rewritten else {
        return Err("object is not a map".to_owned());
    };

    let Some(Value::Text(kind)) = map.get("type").cloned() else {
        return Err("object has no `type`".to_owned());
    };

    if let Some(Value::Map(body)) = map.get("body") {
        let mut body = body.clone();
        if matches!(kind.as_str(), "claim.relation" | "claim.annotation") {
            add_unconditional_scope(&mut body);
        }
        if kind == "claim.annotation" {
            rename_basis_to_grounds(&mut body);
        }
        map.insert("body".to_owned(), Value::Map(body));
    }

    Ok(cbor::encode(&Value::Map(map)))
}

/// R4: every assertion states the conditions its author asserts under.
///
/// Existing relations and annotations were composed before the rule was
/// applied to their grammars, and every one of them was in fact meant
/// unconditionally -- there was no way to say otherwise. `"unconditional"`
/// is therefore the honest migration, and Section 5.3 provides it as an
/// explicit value rather than as an absence.
fn add_unconditional_scope(body: &mut BTreeMap<String, Value>) {
    if body.contains_key("scope") {
        return;
    }
    let mut scope = BTreeMap::new();
    scope.insert("domain".to_owned(), Value::Text("unconditional".to_owned()));
    scope.insert("conditions".to_owned(), Value::Array(Vec::new()));
    body.insert("scope".to_owned(), Value::Map(scope));
}

/// `basis` inside a judgement becomes `grounds`.
///
/// The word now has one meaning across the format -- the state an author
/// was looking at (Section 4.6) -- and "grounds" is what the document
/// already called what a judgement rests on, in Section 6's requirement
/// that a dispute name them.
fn rename_basis_to_grounds(body: &mut BTreeMap<String, Value>) {
    let Some(Value::Map(value)) = body.get("value") else {
        return;
    };
    let Some(basis) = value.get("basis").cloned() else {
        return;
    };
    let mut value = value.clone();
    value.remove("basis");
    value.insert("grounds".to_owned(), basis);
    body.insert("value".to_owned(), Value::Map(value));
}

/// Every object across every directory, indexed by identifier.
///
/// An object legitimately appears in more than one directory -- a domain
/// publishes its own copy of what it holds -- so each one remembers every
/// directory it was found in and is written back to all of them.
type Loaded = (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<PathBuf>>);

fn load(dirs: &[PathBuf]) -> Result<Loaded, String> {
    let mut objects = BTreeMap::new();
    let mut origins: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for dir in dirs {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "cbor") {
                continue;
            }
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let cid = Cid::of(&bytes, HashAlg::Sha2_256);
            origins
                .entry(cid.to_string())
                .or_default()
                .push(dir.clone());
            objects.insert(cid.to_string(), bytes);
        }
    }
    Ok((objects, origins))
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

/// Write each migrated object back to every directory it came from, and
/// remove the file it replaces.
fn write_all(
    migrated: &[(String, String, Vec<u8>)],
    origins: &BTreeMap<String, Vec<PathBuf>>,
) -> Result<(), String> {
    for (old, new, bytes) in migrated {
        let old_cid = old
            .parse::<Cid>()
            .map_err(|_| format!("{old}: not a CID"))?;
        let new_cid = new
            .parse::<Cid>()
            .map_err(|_| format!("{new}: not a CID"))?;
        for dir in origins.get(old).into_iter().flatten() {
            std::fs::write(dir.join(file_name(&new_cid)), bytes)
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            if old != new {
                std::fs::remove_file(dir.join(file_name(&old_cid)))
                    .map_err(|e| format!("{}: {e}", dir.display()))?;
            }
        }
        if old != new {
            println!("{old} {new}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut dry_run = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dirs.push(PathBuf::from(v));
        } else if arg == "--dry-run" {
            dry_run = true;
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    if dirs.is_empty() {
        return usage();
    }

    let (objects, origins) = match load(&dirs) {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_FAILED);
        }
    };

    // Decode once; the loop below inspects each object repeatedly. Objects
    // whose identity rests on a set root are left out entirely -- see the
    // module comment on `REAUTHORED`.
    let mut decoded: BTreeMap<String, Value> = BTreeMap::new();
    let mut skipped = 0usize;
    for (cid, bytes) in &objects {
        let value = match cbor::decode(bytes) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("{cid}: {e}");
                return ExitCode::from(EXIT_FAILED);
            }
        };
        let kind = match &value {
            Value::Map(map) => match map.get("type") {
                Some(Value::Text(kind)) => kind.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        if REAUTHORED.contains(&kind.as_str()) {
            skipped += 1;
            continue;
        }
        decoded.insert(cid.clone(), value);
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
        "migrated {} object(s) across {} director{}; {unchanged} unchanged, \
         {skipped} left for re-authoring",
        migrated.len(),
        dirs.len(),
        if dirs.len() == 1 { "y" } else { "ies" }
    );

    if dry_run {
        for (old, new, _) in &migrated {
            println!("{old} {new}");
        }
        return ExitCode::SUCCESS;
    }

    if let Err(e) = write_all(&migrated, &origins) {
        eprintln!("{e}");
        return ExitCode::from(EXIT_FAILED);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_of(bytes: &[u8]) -> BTreeMap<String, Value> {
        let Ok(Value::Map(map)) = cbor::decode(bytes) else {
            panic!("not a map");
        };
        let Some(Value::Map(body)) = map.get("body") else {
            panic!("no body");
        };
        body.clone()
    }

    fn object(kind: &str, body: BTreeMap<String, Value>) -> Value {
        let mut map = BTreeMap::new();
        map.insert(
            "author".to_owned(),
            Value::Text(
                "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra".to_owned(),
            ),
        );
        map.insert("body".to_owned(), Value::Map(body));
        map.insert(
            "created".to_owned(),
            Value::Text("2026-09-17T00:00:00Z".to_owned()),
        );
        map.insert("pub".to_owned(), Value::Text("1".to_owned()));
        map.insert("type".to_owned(), Value::Text(kind.to_owned()));
        Value::Map(map)
    }

    #[test]
    fn a_relation_gains_an_unconditional_scope() {
        let mut body = BTreeMap::new();
        body.insert("kind".to_owned(), Value::Text("implements".to_owned()));
        let bytes = migrate_one(&object("claim.relation", body), &BTreeMap::new()).unwrap();

        let Some(Value::Map(scope)) = body_of(&bytes).get("scope").cloned() else {
            panic!("no scope");
        };
        assert_eq!(
            scope.get("domain"),
            Some(&Value::Text("unconditional".to_owned()))
        );
        assert_eq!(scope.get("conditions"), Some(&Value::Array(Vec::new())));
    }

    #[test]
    fn a_scope_already_stated_is_left_alone() {
        // Re-running a migration must not overwrite a real scope with
        // `unconditional`, which would silently widen a claim.
        let mut scope = BTreeMap::new();
        scope.insert("domain".to_owned(), Value::Text("aarch64".to_owned()));
        scope.insert("conditions".to_owned(), Value::Array(Vec::new()));
        let mut body = BTreeMap::new();
        body.insert("kind".to_owned(), Value::Text("implements".to_owned()));
        body.insert("scope".to_owned(), Value::Map(scope));

        let bytes = migrate_one(&object("claim.relation", body), &BTreeMap::new()).unwrap();
        let Some(Value::Map(scope)) = body_of(&bytes).get("scope").cloned() else {
            panic!("no scope");
        };
        assert_eq!(
            scope.get("domain"),
            Some(&Value::Text("aarch64".to_owned()))
        );
    }

    #[test]
    fn an_assessments_basis_becomes_grounds() {
        let mut value = BTreeMap::new();
        value.insert("verdict".to_owned(), Value::Text("sound".to_owned()));
        value.insert("basis".to_owned(), Value::Text("read it".to_owned()));
        let mut body = BTreeMap::new();
        body.insert("kind".to_owned(), Value::Text("assessment".to_owned()));
        body.insert("value".to_owned(), Value::Map(value));

        let bytes = migrate_one(&object("claim.annotation", body), &BTreeMap::new()).unwrap();
        let Some(Value::Map(value)) = body_of(&bytes).get("value").cloned() else {
            panic!("no value");
        };
        assert_eq!(
            value.get("grounds"),
            Some(&Value::Text("read it".to_owned()))
        );
        assert_eq!(value.get("basis"), None);
    }

    #[test]
    fn a_prose_claim_is_untouched_but_for_its_references() {
        // Prose claims already state a scope and have no `basis` to rename,
        // so this migration must leave their bytes alone entirely.
        let mut body = BTreeMap::new();
        body.insert("content".to_owned(), Value::Text("a claim".to_owned()));
        let before = object("claim.prose", body);
        let after = migrate_one(&before, &BTreeMap::new()).unwrap();
        assert_eq!(after, cbor::encode(&before));
    }
}
