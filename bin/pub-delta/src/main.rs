//! Apply generation records to a membership and verify the result.
//!
//! The verification is the point. After applying a delta the client
//! recomputes the membership root and compares it to what the final
//! generation declares, so a delta that adds, omits, or substitutes
//! anything is refused without the serving peer being trusted.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg, Object};
use publet_domain::Generation;
use publet_merkle::membership::Membership;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut from = 0u64;
    let mut start: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--from=") {
            if let Ok(n) = v.parse() {
                from = n;
            } else {
                eprintln!("--from must be a generation index");
                return ExitCode::from(EXIT_USAGE);
            }
        } else if let Some(v) = arg.strip_prefix("--member=") {
            start.push(v.to_owned());
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-delta [--dir=DIR] [--from=N] [--member=CID]...");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let mut records = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cannot read {}: {e}", dir.display());
            return ExitCode::from(EXIT_IO);
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cbor"))
        .collect();
    paths.sort();

    for path in paths {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        let Ok(parsed) = Object::parse(&bytes) else {
            continue;
        };
        let Ok(verified) = parsed.verify(&cid) else {
            continue;
        };
        if verified.object().kind() != "generation" {
            continue;
        }
        match Generation::from_object(verified.object()) {
            Ok(g) => records.push(g),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_VIOLATION);
            }
        }
    }

    records.sort_by_key(|g| g.index);
    if records.is_empty() {
        eprintln!("no generation records found in {}", dir.display());
        return ExitCode::from(EXIT_VIOLATION);
    }

    let membership = Membership::new(start);
    match publet_domain::apply(&membership, from, &records) {
        Ok(result) => {
            println!(
                r#"{{"generations":{},"members":{},"root":"{}"}}"#,
                records.len(),
                result.len(),
                publet_merkle::log::to_hex(&result.root())
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}
