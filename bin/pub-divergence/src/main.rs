//! Report whether two publets presuppose a term differently (Section 9.2).
//!
//! Emits JSON Lines, since the result is structured: a term, two
//! definitions, and which of the two outcomes it is. The distinction
//! matters because the remedies differ -- divergence calls for two scoped
//! publets, staleness for superseding against the head.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_graph::{compare, load};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_FOUND: u8 = 3;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut trust_all = true;
    let mut cids: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg == "--trust-none" {
            trust_all = false;
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-divergence [--dir=DIR] [--trust-none] CID CID");
            return ExitCode::from(EXIT_USAGE);
        } else {
            cids.push(arg);
        }
    }

    let Ok([left_text, right_text]) = <[String; 2]>::try_from(cids) else {
        eprintln!("exactly two CIDs are required");
        return ExitCode::from(EXIT_USAGE);
    };
    let (Ok(left), Ok(right)) = (left_text.parse::<Cid>(), right_text.parse::<Cid>()) else {
        eprintln!("both arguments must be CIDs");
        return ExitCode::from(EXIT_USAGE);
    };

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    if graph.object(&left).is_none() || graph.object(&right).is_none() {
        eprintln!("both publets must be in the graph");
        return ExitCode::from(EXIT_NOT_FOUND);
    }

    let trusted = move |_: &Cid| trust_all;
    let result = match compare(&graph, &left, &right, &trusted) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    for (kind, conflicts) in [("divergent", &result.divergent), ("stale", &result.stale)] {
        for c in conflicts {
            println!(
                r#"{{"finding":"{kind}","term":"{}","left":"{}","right":"{}"}}"#,
                c.term.replace('"', "\\\""),
                c.left,
                c.right
            );
        }
    }
    ExitCode::SUCCESS
}
