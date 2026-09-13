//! Structural authoring tests for a publet (Section 5.7).
//!
//! Findings are warnings, not validity rules: a flagged publet is still a
//! valid publet. Exit 0 when clean, 1 when anything fired, so the command
//! composes in a pipeline that wants only well-formed publets.

use std::io::{BufRead as _, IsTerminal as _};
use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::{check_in, load};

const EXIT_FINDINGS: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_LOAD: u8 = 4;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut targets: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-lint [--dir=DIR] [CID...]");
            return ExitCode::from(EXIT_USAGE);
        } else {
            targets.push(arg);
        }
    }

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
            return ExitCode::from(EXIT_LOAD);
        }
    };

    let mut any = false;
    for target in targets {
        let Ok(cid) = target.parse() else {
            eprintln!("not a CID: {target}");
            return ExitCode::from(EXIT_USAGE);
        };
        let Some(publet) = graph.publet(&cid) else {
            continue;
        };
        for finding in check_in(&graph, publet) {
            any = true;
            println!(
                r#"{{"cid":"{cid}","test":"{}","detail":"{}"}}"#,
                finding.test,
                finding.detail.replace('"', "\\\"")
            );
        }
    }

    if any {
        ExitCode::from(EXIT_FINDINGS)
    } else {
        ExitCode::SUCCESS
    }
}
