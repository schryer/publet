//! `pub env`: provision the toolchain a render pipeline runs in.
//!
//! An **environment publet** says how to assemble a pinned toolchain in a
//! local directory, and what the host must already provide to do it. It is
//! shaped like a render pipeline: a `procedural` method whose `depends`
//! lists its steps in order, each a table of commands. Beside them, a
//! `host` claim lists the **host requirements** -- the external
//! dependencies the environment cannot provision itself -- each with the
//! version it requires, how to check it, and `when` it is needed:
//! `provision` (only to build the environment) or `runtime` (whenever
//! something runs in it).
//!
//! An environment is keyed by the toolchain, not by any document: its
//! directory is named by the environment publet's identifier, so every
//! pipeline citing it shares one, and a revised environment is simply a new
//! directory. That keeps environments a sparse set.
//!
//! Provisioning runs downloads and installers, so it is held to what
//! rendering is held to, and more:
//!
//! * the environment publet must be signed by a trusted key unless it is
//!   this workspace's own draft;
//! * every host requirement is checked before anything runs;
//! * steps run without a shell, and only declared host programs;
//! * an output written `path@CID` -- a download -- must hash to that
//!   identifier before any later step can use it.
//!
//! `environment.json` is written last. A directory without it is not an
//! environment, whatever else it holds, so a failure part-way through never
//! looks provisioned.
//!
//! A container image pinned by digest would be another way to provide the
//! same toolchain -- for a deployed service, say -- and could sit behind
//! this same publet. It is not built here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use publet_core::Cid;
use publet_graph::{Class, Data};

use crate::render::{Ctx, Names, Pipeline};
use crate::workspace::Workspace;

/// The file that marks a directory as a provisioned environment.
const RECORD: &str = "environment.json";

/// `pub env build|status|path SLUG|CID`.
///
/// # Errors
///
/// As each subcommand.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let usage = "usage: pub env <build|status|path> SLUG|CID";
    let (Some(command), Some(target)) = (args.first(), args.get(1)) else {
        return Err(usage.to_owned());
    };
    if let Some(extra) = args.get(2) {
        return Err(format!("unknown argument: {extra}"));
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let names = Names::read(&crate::render::default_lock())?;
    let store = ws.store()?;
    let graph = crate::corpus::visible_graph(&here, &store)?;
    let mut trusted = crate::render::trusted_keys(&ws, &graph);
    trusted.extend(names.parent_keys());
    let ctx = Ctx {
        graph: &graph,
        names: &names,
    };
    let env = ctx.head(&names.resolve(target)?);
    match command.as_str() {
        "path" => {
            println!("{}", dir_of(&env).display());
            Ok(())
        }
        "status" => {
            status(&ctx, &env);
            Ok(())
        }
        "build" => build(&ctx, &env, &trusted, &store),
        _ => Err(usage.to_owned()),
    }
}

/// The directory an environment is provisioned in:
/// `$XDG_CACHE_HOME/publet/env/<CID>`, or `~/.cache/...`.
pub(crate) fn dir_of(env: &Cid) -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".cache"));
    cache
        .join("publet")
        .join("env")
        .join(env.to_string().replace(':', "_"))
}

/// Whether `dir` holds a completed provisioning of `env`.
fn provisioned(dir: &Path, env: &Cid) -> bool {
    crate::payload::read_json(&dir.join(RECORD))
        .ok()
        .and_then(|r| r.get("env").and_then(|e| e.as_str()).map(str::to_owned))
        .is_some_and(|e| e == env.to_string())
}

/// For `pub render`: the environment's directory, once it is provisioned,
/// signed, and its runtime host requirements still hold. What does not
/// hold is added to `problems`, as the rest of the pre-flight is.
///
/// # Errors
///
/// Returns a message if the environment is not provisioned here at all.
pub(crate) fn ready(
    ctx: &Ctx<'_>,
    env: &Cid,
    trusted: &BTreeSet<String>,
    problems: &mut Vec<String>,
) -> Result<PathBuf, String> {
    let dir = dir_of(env);
    if !provisioned(&dir, env) {
        return Err(format!(
            "the environment {} is not provisioned on this machine; provision it with \
             `pub env build {env}`",
            ctx.label(env)
        ));
    }
    let pipeline = Pipeline::from_publet(ctx, env)?;
    problems.extend(pipeline.unsigned(ctx, trusted));
    problems.extend(check_hosts(ctx, env, &hosts(ctx, env), Some("runtime")).0);
    Ok(dir)
}

/// One host requirement.
struct Host {
    claim: Cid,
    name: String,
    requires: String,
    check: String,
    expect: String,
    when: String,
}

/// The host requirements an environment publet states: rows of any
/// `archival` table it cites with `name`, `requires`, `check`, and `when`
/// columns.
fn hosts(ctx: &Ctx<'_>, env: &Cid) -> Vec<Host> {
    let mut out = Vec::new();
    for cid in cited_claims(ctx, env) {
        let Some(claim) = ctx.claim(&cid) else {
            continue;
        };
        let Some(Data::Table { columns, rows }) = claim.data() else {
            continue;
        };
        if claim.class() != Class::Archival {
            continue;
        }
        let at = |name: &str| columns.iter().position(|c| c.name == name);
        let (Some(n), Some(r), Some(c), Some(w)) =
            (at("name"), at("requires"), at("check"), at("when"))
        else {
            continue;
        };
        for row in rows {
            let cell = |i: Option<usize>| {
                i.and_then(|i| row.get(i))
                    .map(crate::render::cell_text)
                    .unwrap_or_default()
            };
            out.push(Host {
                claim: cid.clone(),
                name: cell(Some(n)),
                requires: cell(Some(r)),
                check: cell(Some(c)),
                expect: cell(at("expect")),
                when: cell(Some(w)),
            });
        }
    }
    out
}

/// Every claim an environment publet cites, directly or through `depends`.
fn cited_claims(ctx: &Ctx<'_>, env: &Cid) -> Vec<Cid> {
    let mut out: Vec<Cid> = Vec::new();
    let items = ctx
        .graph
        .document(env)
        .map(publet_graph::Document::references)
        .unwrap_or_default();
    for item in items {
        let closure = ctx.graph.depends_closure(&item).unwrap_or_default();
        for cid in std::iter::once(item).chain(closure) {
            if !out.contains(&cid) {
                out.push(cid);
            }
        }
    }
    out
}

/// Check host requirements -- all of them, or those needed `when`.
/// Returns the problems found and what each check printed.
fn check_hosts(
    ctx: &Ctx<'_>,
    env: &Cid,
    hosts: &[Host],
    when: Option<&str>,
) -> (Vec<String>, Vec<(String, String)>) {
    let mut problems = Vec::new();
    let mut observed = Vec::new();
    for host in hosts.iter().filter(|h| when.is_none_or(|w| h.when == w)) {
        let who = format!(
            "{} requires `{}` {} ({})",
            ctx.label(env),
            host.name,
            host.requires,
            ctx.label(&host.claim)
        );
        let mut words = host.check.split_whitespace();
        if words.next() != Some(host.name.as_str()) {
            problems.push(format!(
                "{who}: its check must invoke `{}` itself",
                host.name
            ));
            continue;
        }
        let output = match std::process::Command::new(&host.name).args(words).output() {
            Ok(o) => {
                let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&o.stderr));
                text.trim().to_owned()
            }
            Err(e) => {
                problems.push(format!("{who}: it is not available on this host ({e})"));
                continue;
            }
        };
        let version_ok = host.requires.is_empty()
            || output.split_whitespace().any(|word| {
                let version = word
                    .trim_start_matches('v')
                    .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
                crate::render::satisfies(version, &host.requires)
            });
        if !version_ok || (!host.expect.is_empty() && !output.contains(&host.expect)) {
            problems.push(format!("{who}: this host has {output:?}"));
        }
        // The first line is the version; the rest is licence banners.
        let first = output.lines().next().unwrap_or_default().to_owned();
        observed.push((host.name.clone(), first));
    }
    (problems, observed)
}

fn status(ctx: &Ctx<'_>, env: &Cid) {
    let dir = dir_of(env);
    println!("env       {}", ctx.label(env));
    println!("path      {}", dir.display());
    if provisioned(&dir, env) {
        println!("state     provisioned");
    } else {
        println!("state     not provisioned; `pub env build {env}` provisions it");
    }
    let (problems, observed) = check_hosts(ctx, env, &hosts(ctx, env), None);
    for (name, seen) in observed {
        println!("host      {name}: {seen}");
    }
    for p in problems {
        println!("problem   {p}");
    }
}

fn build(
    ctx: &Ctx<'_>,
    env: &Cid,
    trusted: &BTreeSet<String>,
    store: &publet_store::Store,
) -> Result<(), String> {
    let dir = dir_of(env);
    println!("env       {}", ctx.label(env));
    if provisioned(&dir, env) {
        println!("already provisioned at {}", dir.display());
        return Ok(());
    }
    let pipeline = Pipeline::from_publet(ctx, env)?;
    let hosts = hosts(ctx, env);

    let mut problems = pipeline.unsigned(ctx, trusted);
    let (host_problems, observed) = check_hosts(ctx, env, &hosts, None);
    problems.extend(host_problems);
    // Provisioning runs host programs, and only the ones it declares.
    let declared: BTreeSet<&str> = hosts.iter().map(|h| h.name.as_str()).collect();
    for program in pipeline.programs() {
        if program != "pub" && !declared.contains(program.as_str()) {
            problems.push(format!(
                "{} runs `{program}`, which it does not declare as a host requirement",
                ctx.label(env)
            ));
        }
    }
    if !problems.is_empty() {
        let mut message = String::from("pre-flight failed, so nothing was provisioned:\n");
        for p in &problems {
            message.push_str("  - ");
            message.push_str(p);
            message.push('\n');
        }
        return Err(message.trim_end().to_owned());
    }

    // An earlier attempt that never finished is not an environment.
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(dir.join("bin")).map_err(|e| e.to_string())?;
    materialize(ctx, env, store, &dir)?;
    let values = [("{env}", dir.display().to_string())];
    let outputs = crate::render::execute(ctx, &pipeline, &dir, &values, None)?;

    let record = serde_json::json!({
        "env": env.to_string(),
        "host": observed.iter().map(|(n, o)| serde_json::json!({"name": n, "observed": o}))
            .collect::<Vec<_>>(),
        "outputs": outputs.iter().map(|(_, path, cid)| serde_json::json!({
            "path": path.strip_prefix(&dir).unwrap_or(path).display().to_string(),
            "cid": cid.to_string(),
        })).collect::<Vec<_>>(),
        "built": crate::corpus::now(),
    });
    std::fs::write(
        dir.join(RECORD),
        serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    println!("provisioned at {}", dir.display());
    Ok(())
}

/// Write the files an environment publet carries -- claims whose `data` is
/// a file and whose source `locator` is `env:<path>` -- into the
/// environment before any step runs.
fn materialize(
    ctx: &Ctx<'_>,
    env: &Cid,
    store: &publet_store::Store,
    dir: &Path,
) -> Result<(), String> {
    for cid in cited_claims(ctx, env) {
        let Some(claim) = ctx.claim(&cid) else {
            continue;
        };
        let Some((blob, _)) = claim.data().and_then(Data::blob) else {
            continue;
        };
        let Some(target) = claim
            .sources()
            .iter()
            .find_map(|s| s.locator.as_deref().and_then(env_target))
        else {
            continue;
        };
        let bytes = store
            .get_blob(blob)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{} cites {blob}, which is not held here", ctx.label(&cid)))?;
        let path = dir.join(target);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The path inside an environment an `env:` locator names; never one that
/// climbs out of it.
fn env_target(locator: &str) -> Option<&str> {
    let path = locator.strip_prefix("env:")?;
    let safe = !path.is_empty()
        && !path.starts_with('/')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    safe.then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_environment_lives_under_its_identifier() {
        let cid = Cid::of(b"env", publet_core::HashAlg::Sha2_256);
        let dir = dir_of(&cid);
        assert!(dir.ends_with(cid.to_string().replace(':', "_")));
        assert!(dir.parent().is_some_and(|p| p.ends_with("publet/env")));
    }

    #[test]
    fn an_env_locator_stays_inside_the_environment() {
        assert_eq!(
            env_target("env:curvenote/package.json"),
            Some("curvenote/package.json")
        );
        assert_eq!(env_target("env:../escape"), None);
        assert_eq!(env_target("env:/etc/passwd"), None);
        assert_eq!(env_target("templates/x.typ"), None);
    }
}
