//! Emit the lineage containing an object (Section 6.1).
//!
//! `--authoritative` follows only edges signed by a key that also signed
//! the target -- the author's own revision history. `--full` additionally
//! includes third-party proposals to replace. The two are distinguished
//! because the specification requires it: a proposal is a much weaker
//! thing than a revision.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::{Lineage, load};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_FOUND: u8 = 3;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut mode = Lineage::Authoritative;
    let mut heads_only = false;
    let mut target: Option<String> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg == "--full" {
            mode = Lineage::Full;
        } else if arg == "--authoritative" {
            mode = Lineage::Authoritative;
        } else if arg == "--heads" {
            heads_only = true;
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-lineage [--full|--authoritative] [--heads] [--dir=DIR] CID");
            return ExitCode::from(EXIT_USAGE);
        } else {
            target = Some(arg);
        }
    }

    let Some(target) = target else {
        eprintln!("a CID is required");
        return ExitCode::from(EXIT_USAGE);
    };
    let Ok(cid) = target.parse() else {
        eprintln!("not a CID: {target}");
        return ExitCode::from(EXIT_USAGE);
    };

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    if graph.object(&cid).is_none() {
        eprintln!("not in the graph: {cid}");
        return ExitCode::from(EXIT_NOT_FOUND);
    }

    let view = graph.lineage(&cid, mode);
    let list = if heads_only {
        &view.heads
    } else {
        &view.members
    };
    for member in list {
        println!("{member}");
    }
    ExitCode::SUCCESS
}
