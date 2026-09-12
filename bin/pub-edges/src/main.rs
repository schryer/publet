//! Emit identifiers related to a target by a named relation kind.
//!
//! `--in` gives the reverse-edge query, which content addressing alone
//! cannot answer and which is complete here only because the caller holds
//! the whole graph.

use std::io::{BufRead as _, IsTerminal as _};
use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::{RelationKind, load};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut kind: Option<RelationKind> = None;
    let mut incoming = false;
    let mut targets: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--kind=") {
            if let Some(k) = RelationKind::from_id(v) {
                kind = Some(k);
            } else {
                eprintln!("unknown relation kind: {v}");
                return ExitCode::from(EXIT_USAGE);
            }
        } else if arg == "--in" {
            incoming = true;
        } else if arg == "--out" {
            incoming = false;
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-edges --kind=KIND [--in|--out] [--dir=DIR] [CID...]");
            return ExitCode::from(EXIT_USAGE);
        } else {
            targets.push(arg);
        }
    }

    let Some(kind) = kind else {
        eprintln!("--kind is required");
        return ExitCode::from(EXIT_USAGE);
    };

    if targets.is_empty() && !std::io::stdin().is_terminal() {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                targets.push(trimmed.to_owned());
            }
        }
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    for target in targets {
        let Ok(cid) = target.parse() else {
            eprintln!("not a CID: {target}");
            return ExitCode::from(EXIT_USAGE);
        };
        let related = if incoming {
            graph.incoming(kind, &cid)
        } else {
            graph.out(kind, &cid)
        };
        for r in related {
            println!("{r}");
        }
    }
    ExitCode::SUCCESS
}
