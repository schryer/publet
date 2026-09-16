//! `pub policy`: author a policy object and set it as the workspace's.
//!
//! `pub init` writes a first policy trusting only the workspace's own
//! (placeholder) key, and says outright that it is a placeholder to
//! replace. Nothing before this command could actually replace it --
//! editing trust roots meant hand-building a `policy` object out of band.
//! This is that missing, ordinary administrative action: not a one-off
//! script, because a workspace's trust roots are not a one-time decision --
//! a new key, a revoked key, a widened or narrowed trust set are all
//! things a workspace legitimately needs to do again.

use std::collections::BTreeMap;

use publet_core::{Cid, HashAlg, Object, cbor::Value};

use crate::workspace::Workspace;

const DEFAULT_WEIGHT: u64 = 1000;

/// Author a policy object trusting the given roots, and adopt it.
///
/// # Errors
///
/// Returns a message if no `--root` is given, a root is not a CID, a
/// numeric override does not parse, or the object cannot be built.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    build_and_adopt(&ws, args)
}

/// The part of [`run`] that does not depend on the process's current
/// directory, so tests can drive it against an explicit workspace without
/// racing each other over a shared, process-global `chdir`.
fn build_and_adopt(ws: &Workspace, args: &[String]) -> Result<(), String> {
    let store = ws.store()?;

    let mut roots: Vec<(String, u64)> = Vec::new();
    let mut damping = 850_000u64;
    let mut iterations = 20u64;
    let mut tau = 660_000u64;
    let mut delta_max = 290_000u64;
    let mut replication_floor = 2u64;
    let mut independence_distance = 2u64;
    let mut created = "2026-09-16T00:00:00Z".to_owned();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--root=") {
            // A CID is itself `pub:<alg>:<digest>`, so an optional
            // `:WEIGHT` suffix cannot be split off from the left -- only a
            // right split whose left half still parses as a CID is one.
            let (key, weight) = match v.rsplit_once(':') {
                Some((k, w)) if k.parse::<Cid>().is_ok() => {
                    let weight = w.parse().map_err(|_| format!("bad weight in --root={v}"))?;
                    (k.to_owned(), weight)
                }
                _ => (v.to_owned(), DEFAULT_WEIGHT),
            };
            key.parse::<Cid>()
                .map_err(|_| format!("--root is not a CID: {key}"))?;
            roots.push((key, weight));
        } else if let Some(v) = arg.strip_prefix("--damping=") {
            damping = v
                .parse()
                .map_err(|_| format!("--damping is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--iterations=") {
            iterations = v
                .parse()
                .map_err(|_| format!("--iterations is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--tau=") {
            tau = v
                .parse()
                .map_err(|_| format!("--tau is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--delta-max=") {
            delta_max = v
                .parse()
                .map_err(|_| format!("--delta-max is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--replication-floor=") {
            replication_floor = v
                .parse()
                .map_err(|_| format!("--replication-floor is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--independence-distance=") {
            independence_distance = v
                .parse()
                .map_err(|_| format!("--independence-distance is not a number: {v}"))?;
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    if roots.is_empty() {
        return Err(
            "at least one --root=CID[:WEIGHT] is required; a policy with no roots \
             assigns zero weight to everything"
                .to_owned(),
        );
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub sign --generate-key` first")?;

    let root_values: Vec<Value> = roots
        .iter()
        .map(|(key, weight)| {
            let mut entry = BTreeMap::new();
            entry.insert("key".to_owned(), Value::Text(key.clone()));
            entry.insert("weight".to_owned(), Value::Uint(*weight));
            Value::Map(entry)
        })
        .collect();

    let bytes = Object::builder("policy", &author)
        .created(&created)
        .field("roots", Value::Array(root_values))
        .field("damping", Value::Uint(damping))
        .field("iterations", Value::Uint(iterations))
        .field("tau", Value::Uint(tau))
        .field("delta_max", Value::Uint(delta_max))
        .field("replication_floor", Value::Uint(replication_floor))
        .field("independence_distance", Value::Uint(independence_distance))
        .build()
        .map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    store.put(&cid, &bytes).map_err(|e| e.to_string())?;
    ws.set("policy", &cid.to_string())?;

    println!("{cid}");
    println!();
    println!("This workspace's policy config now points here. Every command");
    println!("that evaluates relative to a policy (Section 11.9) uses this one");
    println!("from now on; the object it replaces still exists, unchanged, at");
    println!("its own CID.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::create(dir.path()).unwrap();
        ws.set(
            "author",
            "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra",
        )
        .unwrap();
        (dir, ws)
    }

    #[test]
    fn a_root_with_no_weight_suffix_is_not_mistaken_for_one() {
        // A CID is itself `pub:<alg>:<digest>`: naively splitting `--root`
        // on the first or last `:` would slice into the CID's own colons
        // rather than an optional `:WEIGHT` suffix.
        let (_dir, ws) = workspace();
        let root = "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra";
        build_and_adopt(&ws, &[format!("--root={root}")]).unwrap();
    }

    #[test]
    fn a_root_with_an_explicit_weight_parses_both_halves() {
        let (_dir, ws) = workspace();
        let root = "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra";
        build_and_adopt(&ws, &[format!("--root={root}:500")]).unwrap();

        let store = ws.store().unwrap();
        let policy_cid: Cid = ws.get("policy").unwrap().parse().unwrap();
        let bytes = store.get(&policy_cid).unwrap().unwrap();
        let object = Object::parse(&bytes).unwrap().verify(&policy_cid).unwrap();
        let Some(Value::Array(roots)) = object.object().body().get("roots").cloned() else {
            panic!("roots is not an array");
        };
        let Some(Value::Map(entry)) = roots.first().cloned() else {
            panic!("root entry is not a map");
        };
        assert_eq!(entry.get("weight"), Some(&Value::Uint(500)));
    }
}
