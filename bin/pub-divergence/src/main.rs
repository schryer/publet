//! Report how publets presuppose a term differently (Section 9.2).
//!
//! Two identifiers give the pairwise answer: a term, two definitions, and
//! which of the two outcomes it is. The distinction matters because the
//! remedies differ -- divergence calls for two scoped publets, staleness
//! for superseding against the head.
//!
//! Three or more give the partition instead. A corpus drawn from several
//! sources does not ask whether two of them differ; it asks how all of
//! them divide on a term and which one stands alone. Pairwise comparison
//! answers that in six runs and no aggregate, so it is a different
//! question rather than more of the same one.
//!
//! Emits JSON Lines either way.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_graph::{compare, load, partition};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_FOUND: u8 = 3;

/// Terms come from publet content and may carry quotes.
fn escape(term: &str) -> String {
    term.replace('\\', "\\\\").replace('"', "\\\"")
}

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
            eprintln!("       pub-divergence [--dir=DIR] CID CID CID...");
            return ExitCode::from(EXIT_USAGE);
        } else {
            cids.push(arg);
        }
    }

    if cids.len() < 2 {
        eprintln!("at least two CIDs are required");
        return ExitCode::from(EXIT_USAGE);
    }
    let mut parsed: Vec<Cid> = Vec::new();
    for text in &cids {
        let Ok(cid) = text.parse::<Cid>() else {
            eprintln!("not a CID: {text}");
            return ExitCode::from(EXIT_USAGE);
        };
        parsed.push(cid);
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    for cid in &parsed {
        if graph.object(cid).is_none() {
            eprintln!("not in the graph: {cid}");
            return ExitCode::from(EXIT_NOT_FOUND);
        }
    }

    // Two is the pairwise question; more is the partition. The divergent
    // and stale outcomes are a comparison of two lineages and have no
    // N-way reading, so they are not reported for more than two.
    if let [left, right] = parsed.as_slice() {
        let trusted = move |_: &Cid| trust_all;
        let result = match compare(&graph, left, right, &trusted) {
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
                    escape(&c.term),
                    c.left,
                    c.right
                );
            }
        }
        return ExitCode::SUCCESS;
    }

    let partitions = match partition(&graph, &parsed) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };
    for term in partitions {
        let groups: Vec<String> = term
            .groups
            .iter()
            .map(|(definition, members)| {
                let held: Vec<String> = members.iter().map(|m| format!(r#""{m}""#)).collect();
                format!(
                    r#"{{"definition":"{definition}","publets":[{}]}}"#,
                    held.join(",")
                )
            })
            .collect();
        println!(
            r#"{{"finding":"partition","term":"{}","groups":[{}]}}"#,
            escape(&term.term),
            groups.join(",")
        );
    }
    ExitCode::SUCCESS
}
