//! The settlement layer's command-line surface (Section 12).
//!
//! Settlement is optional, so this binary may be absent from a deployment
//! entirely. When it is present and no ledger is configured -- the default,
//! and the only configuration this implementation ships -- it refuses every
//! settlement and says why. That refusal is the observable form of the
//! optionality: everything else in the system keeps working.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg, Object};
use publet_settle::{Bounty, Ledger as _, NullLedger, Prohibition};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn usage() -> ExitCode {
    eprintln!(
        "usage: pub-settle --prohibitions\n\
         \x20      pub-settle --bounty=FILE\n\
         \x20      pub-settle --settle=FILE [--qualifying=CID]..."
    );
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let mut prohibitions = false;
    let mut bounty: Option<PathBuf> = None;
    let mut settle: Option<PathBuf> = None;
    let mut qualifying: Vec<Cid> = Vec::new();

    for arg in std::env::args().skip(1) {
        if arg == "--prohibitions" {
            prohibitions = true;
        } else if let Some(v) = arg.strip_prefix("--bounty=") {
            bounty = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--settle=") {
            settle = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--qualifying=") {
            let Ok(cid) = v.parse() else {
                eprintln!("not a CID: {v}");
                return ExitCode::from(EXIT_USAGE);
            };
            qualifying.push(cid);
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    if prohibitions {
        for prohibition in Prohibition::all() {
            println!(
                r#"{{"forbids":"{}","reason":"{}"}}"#,
                prohibition.id(),
                prohibition.reason().replace('"', "\\\"")
            );
        }
        return ExitCode::SUCCESS;
    }

    let Some(path) = bounty.or(settle.clone()) else {
        return usage();
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            return ExitCode::from(EXIT_IO);
        }
    };
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    let parsed = match Object::parse(&bytes).and_then(|p| p.verify(&cid)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    let read = match Bounty::from_object(parsed.object()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    if settle.is_none() {
        println!(
            r#"{{"kind":"{}","target":"{}","policy":"{}","base_share":{}}}"#,
            read.kind.id(),
            read.target,
            read.policy,
            read.review_share
        );
        return ExitCode::SUCCESS;
    }

    // No ledger is configured, and none can be: this implementation ships
    // the null one only. Choosing a chain is a deployment's decision and
    // the protocol takes no position on it (Section 12.1).
    match NullLedger.settle(&cid, &qualifying) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}
