#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]
//! `pub`: the human-facing command.
//!
//! Local-first by default (R16). Every command says which mode it used,
//! because the privacy properties of reading a replica are real and a
//! reader who does not know which mode they are in cannot know whether they
//! have them.

mod annotate;
mod build;
mod compose;
mod corpus;
mod document;
mod domain;
mod env;
mod payload;
mod policy;
mod propose;
mod read;
mod relate;
mod render;
mod revise;
mod sign;
mod why;
mod workspace;

use std::process::ExitCode;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn usage() -> ExitCode {
    eprintln!("usage: pub <command> [options]");
    eprintln!();
    eprintln!("  init                 create a workspace here");
    eprintln!("  sync                 fetch a domain from a peer by delta");
    eprintln!("  read CID             show an assertion and the scope it was made under");
    eprintln!("  why CID              show its standing and every component of it");
    eprintln!("  compose              build a claim and add it to the workspace");
    eprintln!("  document             build a document ordering references to claims");
    eprintln!("  doc MANIFEST         build a document from a JSON manifest");
    eprintln!("  revise CID           publish the next version and its supersedes edge");
    eprintln!("  build [DIR]          compile named publet sources (in a corpus: drafts only)");
    eprintln!("  publish SLUG...|--all  sign, export, and record drafts; pin their renderings");
    eprintln!("  render SLUG|CID      check, then run, the render pipeline a publet names");
    eprintln!("  corpus init|status|upgrade|map   a corpus, its parents, and its network");
    eprintln!("  delegate --to=KEY    authorize another key to continue this key's lineages");
    eprintln!("  env build|status|path SLUG   provision the pinned toolchain a pipeline runs in");
    eprintln!("  myst SLUG|CID        write a MyST project for a document");
    eprintln!("  relate               author a relation between two objects");
    eprintln!("  annotate             say something about an object without touching it");
    eprintln!("  sign                 generate a signing key, or sign a stored object");
    eprintln!("  policy               author a policy object and set it as the workspace's");
    eprintln!("  propose CID...       submit objects, recording what they were composed against");
    eprintln!("  witness DOMAIN       record the log root you observed");
    eprintln!(
        "  domain               author a domain's manifest, snapshot, and genesis generation"
    );
    ExitCode::from(EXIT_USAGE)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        return usage();
    };
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();

    let result = match command {
        "init" => init(&rest),
        "sync" => sync(&rest).await,
        "read" => read::run(&rest),
        "why" => why::run(&rest),
        "compose" => compose::run(&rest),
        "document" => document::run(&rest),
        "doc" => document::run_manifest(&rest),
        "revise" => revise::run(&rest),
        "build" => build::run(&rest),
        "publish" => corpus::publish(&rest),
        "render" => render::run(&rest),
        "corpus" => corpus::run(&rest),
        "env" => env::run(&rest),
        "delegate" => corpus::delegate(&rest),
        "myst" => render::run_myst(&rest),
        "--version" | "version" => {
            print_version();
            return ExitCode::SUCCESS;
        }
        "relate" => relate::run(&rest),
        "annotate" => annotate::run(&rest),
        "sign" => sign::run(&rest),
        "policy" => policy::run(&rest),
        "propose" => propose::run(&rest),
        "witness" => witness(&rest),
        "domain" => domain::run(&rest),
        "-h" | "--help" | "help" => return usage(),
        other => Err(format!("unknown command: {other}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

/// `pub X.Y.Z` only for a clean build at that version's release tag (or
/// one with no git to ask); any other build says it is unreleased, so an
/// identity claim stating a release can never be satisfied by it.
fn print_version() {
    let version = env!("CARGO_PKG_VERSION");
    let describe = env!("PUBLET_DESCRIBE");
    if describe.is_empty() || describe == format!("v{version}") {
        println!("pub {version}");
        let package = env!("PUBLET_PACKAGE");
        if !package.is_empty() {
            println!("package  pkg.publet-cli#identity — {package}");
        }
    } else if describe.starts_with('v') {
        let after = describe.split('-').next().unwrap_or(describe);
        println!("pub unreleased build after {after} ({describe})");
    } else {
        println!("pub unreleased build ({describe}; no release precedes it)");
    }
}

fn init(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = workspace::Workspace::create(&here)?;
    let store = ws.store()?;

    // A workspace needs a viewpoint before it can evaluate anything, and a
    // policy with no roots assigns zero weight to everything. So one is
    // written now, seeded with the identity this workspace will author as,
    // and the user is told it is theirs to replace.
    let author = publet_core::Cid::of(
        format!("{}", std::process::id()).as_bytes(),
        publet_core::HashAlg::Sha2_256,
    );
    let policy = compose::default_policy(&author)?;
    let cid = publet_core::Cid::of(&policy, publet_core::HashAlg::Sha2_256);
    store.put(&cid, &policy).map_err(|e| e.to_string())?;
    ws.set("policy", &cid.to_string())?;
    ws.set("author", &author.to_string())?;

    for arg in args {
        if let Some(v) = arg.strip_prefix("--peer=") {
            ws.set("peer", v)?;
        }
    }

    println!(
        "workspace created in {}",
        here.join(workspace::DIR).display()
    );
    println!("policy  {cid}");
    println!("author  {author}");
    println!();
    println!("The policy trusts only this workspace's own key. Everything you");
    println!("evaluate is relative to that, so replace it with roots you have");
    println!("actually chosen (Section 11.9).");
    println!();
    // Section 17 requires these stated in plain language before a first
    // publication, not buried in documentation someone may never read.
    println!("Before you publish anything, three things about this system:");
    println!();
    println!("  Publication is permanent and attributed. Every object you");
    println!("  sign is replicated and designed not to be removable, so your");
    println!("  key accumulates a permanent, machine-readable record of every");
    println!("  claim you make and every judgement you render.");
    println!();
    println!("  There is no erasure. `retracts` withdraws a claim without");
    println!("  deleting it, and nodes are required to keep serving what they");
    println!("  hold. Personal data published here cannot be reliably");
    println!("  recalled -- by you or by anyone.");
    println!();
    println!("  Use separate keys for separate contexts. This workspace holds");
    println!("  one; `pub init` elsewhere makes another. Reusing a single key");
    println!("  across unrelated subjects links them permanently, and no");
    println!("  later decision can unlink them.");
    Ok(())
}

async fn sync(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = workspace::Workspace::open(&here)?;

    let mut peer = ws.get("peer");
    let mut domain = ws.get("domain");
    let mut from = 0u64;
    let mut to: Option<u64> = None;
    let mut proxy = None;

    for arg in args {
        if let Some(v) = arg.strip_prefix("--peer=") {
            peer = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--domain=") {
            domain = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--from=") {
            from = v.parse().unwrap_or(0);
        } else if let Some(v) = arg.strip_prefix("--to=") {
            to = v.parse().ok();
        } else if let Some(v) = arg.strip_prefix("--proxy=") {
            proxy = Some(v.to_owned());
        }
    }

    let peer = peer.ok_or("no peer configured; pass --peer=URL")?;
    let domain: publet_core::Cid = domain
        .ok_or("no domain configured; pass --domain=CID")?
        .parse()
        .map_err(|_| "--domain must be a CID".to_owned())?;

    let client = match &proxy {
        Some(p) => publet_net::Client::with_proxy(&peer, p),
        None => publet_net::Client::new(&peer),
    }
    .map_err(|e| e.to_string())?;

    let to = match to {
        Some(n) => n,
        None => client
            .checkpoints(&domain)
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .copied()
            .max()
            .map_or(from, |m| m + 1),
    };

    let objects = client
        .delta(&domain, from, to)
        .await
        .map_err(|e| e.to_string())?;
    let store = ws.store()?;
    let mut stored = 0usize;
    for bytes in objects {
        let cid = publet_core::Cid::of(&bytes, publet_core::HashAlg::Sha2_256);
        store.put(&cid, &bytes).map_err(|e| e.to_string())?;
        stored += 1;
    }
    ws.set("domain", &domain.to_string())?;

    println!("synced {stored} object(s) from generation {from} to {to}");
    println!();
    println!("The request carried this domain and two integers. It disclosed");
    println!("nothing about which objects you already hold (Section 14.3.2),");
    println!("though the peer now knows you follow this domain.");
    Ok(())
}

fn witness(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = workspace::Workspace::open(&here)?;
    let store = ws.store()?;

    let domain: publet_core::Cid = args
        .first()
        .ok_or("a domain CID is required")?
        .parse()
        .map_err(|_| "the argument must be a CID".to_owned())?;

    let members = store
        .members_of(&domain)
        .map_err(|e| e.to_string())?
        .ok_or("that domain is not declared in this workspace")?;
    let root = publet_merkle::membership::Membership::new(members).root();
    let head = store
        .head_generation(&domain)
        .map_err(|e| e.to_string())?
        .unwrap_or(0);

    println!("domain      {domain}");
    println!("generation  {head}");
    println!("log root    {}", publet_merkle::log::to_hex(&root));
    println!();
    println!("Publish this as a `witnessed` annotation and compare it with");
    println!("others: a publisher showing different histories to different");
    println!("readers appears as two roots at one generation (Section 14.1.2).");
    Ok(())
}
