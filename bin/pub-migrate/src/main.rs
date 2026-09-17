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
//! Prose is left alone unless `--rewrite-prose` is passed. Migrating bytes
//! is not the same as revising assertions, so retiring a term inside a
//! claim's own text is a decision for whoever owns the claim, not a side
//! effect of a format change. The flag exists because that decision was
//! taken for this corpus; it protects repository paths and the protocol's
//! own name, both of which legitimately still contain the old word.
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
//! only in the main one still resolves; each object is written back to the
//! directory it came from.

use std::collections::BTreeMap;
use std::path::PathBuf;
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

/// Types whose identity depends on a set root rather than on references,
/// and which are therefore re-authored rather than migrated.
const REAUTHORED: [&str; 2] = ["domain", "generation"];

fn usage() -> ExitCode {
    eprintln!("usage: pub-migrate --dir=DIR... [--dry-run] [--rewrite-prose]");
    eprintln!();
    eprintln!("  --dir            an object directory to migrate, repeatable");
    eprintln!("  --dry-run        report the mapping without writing anything");
    eprintln!("  --rewrite-prose  also retire `publet` inside claim text");
    ExitCode::from(EXIT_USAGE)
}

/// Whether the byte at `i` continues a word, for the purposes of deciding
/// that an occurrence of `publet` is a standalone term rather than part of
/// a path or an identifier.
fn joins_word(bytes: &[u8], i: usize) -> bool {
    bytes.get(i).is_some_and(|b| {
        b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-' || *b == b'/' || *b == b'.'
    })
}

/// Retire `publet` as a term inside prose, leaving paths and the
/// protocol's own name intact.
fn retire_term(text: &str) -> String {
    // `the Publet Protocol` is the protocol, not the retired object type,
    // and appears as a scope domain throughout this corpus.
    const KEEP: &str = "Publet Protocol";
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &text[i..];
        if rest.starts_with(KEEP) {
            out.push_str(KEEP);
            i += KEEP.len();
            continue;
        }
        let lower = rest.len() >= 6 && rest[..6].eq_ignore_ascii_case("publet");
        if lower && !joins_word(bytes, i.wrapping_sub(1)) {
            let plural = rest.as_bytes().get(6) == Some(&b's');
            let end = i + if plural { 7 } else { 6 };
            if !joins_word(bytes, end) {
                let upper = rest.as_bytes().first() == Some(&b'P');
                out.push_str(match (upper, plural) {
                    (true, true) => "Claims",
                    (true, false) => "Claim",
                    (false, true) => "claims",
                    (false, false) => "claim",
                });
                i = end;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Apply [`retire_term`] to every text in a subtree.
fn rewrite_prose(value: &Value) -> Value {
    match value {
        Value::Text(text) => Value::Text(retire_term(text)),
        Value::Array(items) => Value::Array(items.iter().map(rewrite_prose).collect()),
        Value::Map(entries) => Value::Map(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), rewrite_prose(v)))
                .collect(),
        ),
        other => other.clone(),
    }
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
fn migrate_one(
    value: &Value,
    mapping: &BTreeMap<String, String>,
    prose: bool,
) -> Result<Vec<u8>, String> {
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
        // Prose rewriting is confined to the body: the header's `type` is a
        // vocabulary term that `migrated_type` owns, and must not be caught
        // by a rule meant for sentences.
        if prose && let Value::Map(rewritten) = rewrite_prose(&Value::Map(body.clone())) {
            body = rewritten;
        }
        map.insert("body".to_owned(), Value::Map(body));
    }

    Ok(cbor::encode(&Value::Map(map)))
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
    prose: bool,
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
            let bytes = migrate_one(value, &mapping, prose).map_err(|e| format!("{cid}: {e}"))?;
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
    let mut prose = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dirs.push(PathBuf::from(v));
        } else if arg == "--dry-run" {
            dry_run = true;
        } else if arg == "--rewrite-prose" {
            prose = true;
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

    let migrated = match migrate_all(&decoded, &objects, prose) {
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

    #[test]
    fn retires_the_term_where_it_is_a_term() {
        assert_eq!(
            retire_term("lineage: the chain of revisions of one publet"),
            "lineage: the chain of revisions of one claim"
        );
        assert_eq!(
            retire_term("term extraction covers definitional publets only"),
            "term extraction covers definitional claims only"
        );
        assert_eq!(retire_term("one publet's content"), "one claim's content");
    }

    #[test]
    fn leaves_paths_and_the_protocol_name_alone() {
        // These are the two places the old word is still correct: the
        // repositories are really named that, and so is the protocol.
        for path in [
            "publet/crates/publet-eval/src/propagate.rs",
            "docs/publet-specification/index.md",
            "publet-corpus/objects",
        ] {
            assert_eq!(retire_term(path), path, "rewrote a path: {path}");
        }
        assert_eq!(retire_term("the Publet Protocol"), "the Publet Protocol");
    }

    #[test]
    fn a_sentence_mixing_both_keeps_only_the_path() {
        assert_eq!(
            retire_term("the publet cited publet/crates/publet-core/src/cid.rs"),
            "the claim cited publet/crates/publet-core/src/cid.rs"
        );
    }

    #[test]
    fn case_and_number_survive() {
        assert_eq!(retire_term("Publets are claims"), "Claims are claims");
        assert_eq!(retire_term("Publet"), "Claim");
    }
}
