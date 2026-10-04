//! `pub domain`: author a domain manifest and its genesis generation
//! record (Section 14.1).
//!
//! Until now nothing in the toolchain could write these object kinds --
//! `publet_domain::Manifest` and `Generation` only ever read them back. A
//! domain kept as a directory of files needs the real Merkle-backed
//! objects Section 14 defines, not a naming convention that merely looks
//! like one: this reads every object actually in the directory, computes
//! the membership root the same way `pub witness` and `Store::declare`
//! do, and checks the `depends` closure it claims rather than assuming a
//! directory that looks complete is one.
//!
//! The manifest's `snapshot` field is the membership root itself, wrapped
//! as a CID via [`Cid::from_digest`] -- not the CID of a separately
//! authored object. That is what `Store::declare` compares against, and
//! matching it is what lets `pub-store declare` verify this domain
//! independently rather than merely accept whatever shape this command
//! happens to produce.
//!
//! `--also-closed-under` extends the closure `depends` alone checks to
//! `translates` and/or `equivalent`. Those relations assert two objects
//! express one claim, so a domain holding one side without the other is
//! silently partial the same way an unmet `depends` edge is -- but
//! unlike `evidence`, `disputes`, and `supports` (deliberately exempt:
//! Section 14.1), a translation or an equivalence is usually authored by
//! the same party as a deliberate alternate rendering, not open-ended
//! commentary, so requiring it stays affordable.

use publet_algorithms::membership::Membership;
use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::RelationKind;

use crate::workspace::Workspace;

struct Args {
    dir: std::path::PathBuf,
    label: String,
    bound: u64,
    created: String,
    also: Vec<RelationKind>,
}

fn parse_also(spec: &str) -> Result<Vec<RelationKind>, String> {
    spec.split(',')
        .map(|part| {
            let part = part.trim();
            match RelationKind::from_id(part) {
                Some(k @ (RelationKind::Translates | RelationKind::Equivalent)) => Ok(k),
                Some(_) => Err(format!(
                    "--also-closed-under does not accept `{part}`: only `translates` and \
                     `equivalent` express \"the same claim, another rendering\" -- closing \
                     over evidence, disputes, or supports would make the bound unaffordable \
                     (Section 14.1)"
                )),
                None => Err(format!("unknown relation kind: {part}")),
            }
        })
        .collect()
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut dir: Option<std::path::PathBuf> = None;
    let mut label: Option<String> = None;
    let mut bound: Option<u64> = None;
    let mut created = "2026-09-14T00:00:00Z".to_owned();
    let mut also = Vec::new();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = Some(std::path::PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--label=") {
            label = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--bound=") {
            bound = Some(
                v.parse()
                    .map_err(|_| "--bound must be a byte count".to_owned())?,
            );
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else if let Some(v) = arg.strip_prefix("--also-closed-under=") {
            also = parse_also(v)?;
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    Ok(Args {
        dir: dir.ok_or("--dir is required: the domain's object directory")?,
        label: label.ok_or("--label is required")?,
        bound: bound.ok_or("--bound is required: the declared byte ceiling, in bytes")?,
        created,
        also,
    })
}

/// The manifest and genesis generation, with their CIDs and the root they
/// both name.
struct Built {
    domain: (Cid, Vec<u8>),
    generation: (Cid, Vec<u8>),
    snapshot: Cid,
    root: publet_algorithms::log::Hash,
}

/// Author the domain manifest and genesis generation as one consistent
/// pair -- both name the same snapshot CID, which is only guaranteed by
/// building them together from one computed root.
fn build_objects(author: &str, a: &Args, members: &[String], size: u64) -> Result<Built, String> {
    let root = Membership::new(members.iter().cloned()).root();
    let snapshot = Cid::from_digest(HashAlg::Sha2_256, &root)
        .ok_or("could not address the membership root as a CID")?;

    let closed_under: Vec<Value> = std::iter::once("depends".to_owned())
        .chain(a.also.iter().map(|k| k.id().to_owned()))
        .map(Value::Text)
        .collect();
    let domain_bytes = Object::builder("domain", author)
        .created(&a.created)
        .field("label", Value::Text(a.label.clone()))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field("bound", Value::Uint(a.bound))
        .field("size", Value::Uint(size))
        .field("closed_under", Value::Array(closed_under))
        .build()
        .map_err(|e| e.to_string())?;
    let domain_cid = Cid::of(&domain_bytes, HashAlg::Sha2_256);

    // A genesis generation: everything currently held counts as "added",
    // nothing as "removed", against an empty predecessor membership. It is
    // declared explicitly rather than left implicit so a second generation
    // later has something real to be checked against (Section 14.1.1).
    let generation_bytes = Object::builder("generation", author)
        .created(&a.created)
        .field("domain", Value::Text(domain_cid.to_string()))
        .field("index", Value::Uint(0))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field(
            "added",
            Value::Array(members.iter().map(|m| Value::Text(m.clone())).collect()),
        )
        .field("removed", Value::Array(Vec::new()))
        .build()
        .map_err(|e| e.to_string())?;
    let generation_cid = Cid::of(&generation_bytes, HashAlg::Sha2_256);

    Ok(Built {
        domain: (domain_cid, domain_bytes),
        generation: (generation_cid, generation_bytes),
        snapshot,
        root,
    })
}

/// An object's identifier and bytes.
pub(crate) type Signed = (Cid, Vec<u8>);

/// A domain manifest and its genesis generation over `members`, for a
/// caller that has its own label, bound, and members rather than flags --
/// `pub build` in a corpus, on the corpus's first generation.
///
/// # Errors
///
/// Returns a message if the objects cannot be built.
pub(crate) fn manifest_and_genesis(
    author: &str,
    label: &str,
    bound: u64,
    created: &str,
    members: &[String],
    size: u64,
) -> Result<(Signed, Signed), String> {
    let args = Args {
        dir: std::path::PathBuf::new(),
        label: label.to_owned(),
        bound,
        created: created.to_owned(),
        also: Vec::new(),
    };
    let built = build_objects(author, &args, members, size)?;
    Ok((built.domain, built.generation))
}

/// The generation record after `parent`: `index`, the membership root of
/// `members`, and the members `added` since. Nothing is ever removed from a
/// corpus -- objects are immutable and a corpus only grows -- so `removed`
/// is empty, and a membership that lost a member is refused by the caller.
///
/// # Errors
///
/// Returns a message if the root cannot be addressed or the object built.
pub(crate) fn next_generation(
    author: &str,
    created: &str,
    domain: &Cid,
    index: u64,
    parent: &Cid,
    members: &[String],
    added: &[String],
) -> Result<(Cid, Vec<u8>), String> {
    let root = Membership::new(members.iter().cloned()).root();
    let snapshot = Cid::from_digest(HashAlg::Sha2_256, &root)
        .ok_or("could not address the membership root as a CID")?;
    let bytes = Object::builder("generation", author)
        .created(created)
        .field("domain", Value::Text(domain.to_string()))
        .field("index", Value::Uint(index))
        .field("parent", Value::Text(parent.to_string()))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field(
            "added",
            Value::Array(added.iter().map(|m| Value::Text(m.clone())).collect()),
        )
        .field("removed", Value::Array(Vec::new()))
        .build()
        .map_err(|e| e.to_string())?;
    Ok((Cid::of(&bytes, HashAlg::Sha2_256), bytes))
}

/// Build and store a domain's manifest and genesis generation.
///
/// # Errors
///
/// Returns a message if a required flag is missing, the directory will
/// not load, the domain is not closed under `depends`, or the declared
/// bound is exceeded.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let a = parse_args(args)?;

    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    // Loading (rather than just listing files) means a corrupt object or a
    // relation pointing outside the directory is caught here, not by
    // whoever clones this domain expecting it to be self-contained.
    let graph = publet_graph::load::from_dir(&a.dir).map_err(|e| e.to_string())?;
    let members: Vec<String> = graph.cids().into_iter().map(ToOwned::to_owned).collect();
    publet_domain::check_depends_closure(&graph, &members).map_err(|e| e.to_string())?;
    for kind in &a.also {
        publet_domain::check_equivalence_closure(&graph, &members, *kind)
            .map_err(|e| e.to_string())?;
    }

    let size: u64 = std::fs::read_dir(&a.dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "cbor"))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    if size > a.bound {
        return Err(format!(
            "domain is {size} bytes against a declared bound of {}; \
             raise --bound or split the domain before snapshotting it (Section 14.1)",
            a.bound
        ));
    }

    let built = build_objects(&author, &a, &members, size)?;
    for (cid, bytes) in [&built.domain, &built.generation] {
        let name = format!("{}.cbor", cid.to_string().replace(':', "_"));
        std::fs::write(a.dir.join(name), bytes).map_err(|e| e.to_string())?;
    }

    println!("wrote 2 objects into {}", a.dir.display());
    println!();
    let closed_under_desc = std::iter::once("depends")
        .chain(a.also.iter().map(|k| k.id()))
        .collect::<Vec<_>>()
        .join(", ");
    println!("domain      {}", built.domain.0);
    println!(
        "  \"{}\", {size} of {} declared bytes, closed under {closed_under_desc}",
        a.label, a.bound
    );
    println!("generation  {}", built.generation.0);
    println!("  index 0 (genesis), {} added, 0 removed", members.len());
    println!();
    println!(
        "snapshot    {}  ({} member(s), root {})",
        built.snapshot,
        members.len(),
        publet_algorithms::log::to_hex(&built.root)
    );
    println!("  not a stored object -- the membership root itself, addressed as");
    println!("  a CID, exactly as `pub-store declare` will recompute it");
    Ok(())
}
