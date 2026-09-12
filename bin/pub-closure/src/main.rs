//! Emit a publet's transitive `depends` closure (Section 5.4).
//!
//! Exits 1 if the closure contains a cycle, which the specification
//! forbids: an assertion whose terms presuppose each other is not
//! interpretable.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::load;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_FOUND: u8 = 3;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut target: Option<String> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-closure [--dir=DIR] CID");
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

    match graph.depends_closure(&cid) {
        Ok(members) => {
            for member in members {
                println!("{member}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}
