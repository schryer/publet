//! Manage a store: add objects, declare sets, collect garbage, scan.
//!
//! These are node-operator actions rather than pipeline filters, so they
//! sit behind subcommands instead of becoming separate binaries.

use std::io::Read as _;
use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg, Object};
use publet_store::Store;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn usage() -> ExitCode {
    eprintln!("usage: pub-store [--store=FILE] <add|declare|gc|scan|list>");
    eprintln!("  add              read one object on stdin and store it");
    eprintln!(
        "  declare DOMAIN   declare a held domain; members as CID lines on stdin,
                   checked against the manifest's snapshot root"
    );
    eprintln!("  gc               discard objects outside every declared set");
    eprintln!("  scan             re-hash every object and report mismatches");
    eprintln!("  list             emit every stored identifier");
    eprintln!("  generation       read a generation record on stdin and index it");
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let mut path = PathBuf::from("objects.redb");
    let mut rest: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--store=") {
            path = PathBuf::from(v);
        } else if arg.starts_with("--") {
            return usage();
        } else {
            rest.push(arg);
        }
    }

    let Some(command) = rest.first().cloned() else {
        return usage();
    };

    let store = match Store::open(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    match command.as_str() {
        "add" => add(&store),
        "declare" => declare(&store, rest.get(1).map(String::as_str)),
        "gc" => match store.collect_garbage() {
            Ok(removed) => {
                for cid in &removed {
                    println!("{cid}");
                }
                eprintln!("collected {}", removed.len());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(EXIT_IO)
            }
        },
        "scan" => match store.scan() {
            Ok(findings) => {
                for f in &findings {
                    println!(
                        r#"{{"stored_as":"{}","actual":"{}"}}"#,
                        f.stored_as, f.actual
                    );
                }
                if findings.is_empty() {
                    ExitCode::SUCCESS
                } else {
                    eprintln!(
                        "{} object(s) no longer match their identifier",
                        findings.len()
                    );
                    ExitCode::from(EXIT_VIOLATION)
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(EXIT_IO)
            }
        },
        "generation" => generation(&store),
        "list" => match store.cids() {
            Ok(cids) => {
                for cid in cids {
                    println!("{cid}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(EXIT_IO)
            }
        },
        _ => usage(),
    }
}

fn add(store: &Store) -> ExitCode {
    let mut bytes = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    match store.put(&cid, &bytes) {
        Ok(()) => {
            println!("{cid}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

/// Index a generation record, taking its domain and index from the record
/// itself rather than from arguments that could disagree with it.
fn generation(store: &Store) -> ExitCode {
    let mut bytes = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    let parsed = match Object::parse(&bytes).and_then(|o| o.verify(&cid)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    let record = match publet_domain::Generation::from_object(parsed.object()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    if let Err(e) = store.put(&cid, &bytes) {
        eprintln!("{e}");
        return ExitCode::from(EXIT_VIOLATION);
    }
    match store.put_generation(&record.domain, record.index, &bytes) {
        Ok(()) => {
            eprintln!("indexed generation {} of {}", record.index, record.domain);
            println!("{cid}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_IO)
        }
    }
}

fn declare(store: &Store, domain: Option<&str>) -> ExitCode {
    let Some(domain) = domain else {
        return usage();
    };
    let Ok(cid) = domain.parse::<Cid>() else {
        eprintln!("not a CID: {domain}");
        return ExitCode::from(EXIT_USAGE);
    };
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }
    let members: Vec<String> = input
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    match store.declare(&cid, &members) {
        Ok(()) => {
            eprintln!("declared {cid} with {} member(s)", members.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            // A refused declaration is a protocol violation, not an I/O
            // failure: the manifest is absent or the members do not match it.
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}
