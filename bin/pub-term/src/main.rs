//! Resolve a term to the publets defining it (Section 9.1).
//!
//! There is no namespace and no registry. A term is whatever some
//! `definitional` publet says it is, several publets may say different
//! things, and choosing between them belongs to a viewpoint rather than to
//! a lookup. So this reports every definition it finds and never one.
//!
//! It also reports whether a term is contested, because that is the fact
//! that changes what an author should do next: a contested term used
//! without being declared in `depends` draws a lint finding, and knowing
//! which terms are contested is how an author avoids collecting them.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::{RelationKind, load};

const EXIT_NOT_FOUND: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_LOAD: u8 = 4;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut list = false;
    let mut wanted: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if arg == "--list" {
            list = true;
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-term [--dir=DIR] TERM...");
            eprintln!("       pub-term [--dir=DIR] --list");
            return ExitCode::from(EXIT_USAGE);
        } else {
            wanted.push(arg.to_lowercase());
        }
    }

    if !list && wanted.is_empty() {
        eprintln!("usage: pub-term [--dir=DIR] TERM...");
        eprintln!("       pub-term [--dir=DIR] --list");
        return ExitCode::from(EXIT_USAGE);
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_LOAD);
        }
    };

    let mut found = 0usize;
    for (term, cid) in graph.terms() {
        if !list && !wanted.contains(&term) {
            continue;
        }
        found += 1;
        let contested = !graph.incoming(RelationKind::Disputes, &cid).is_empty();
        println!(
            r#"{{"term":"{}","cid":"{cid}","contested":{contested}}}"#,
            escape(&term)
        );
    }

    if found == 0 && !list {
        // Not an error in the sense of something being wrong: nobody has
        // defined the term here yet, which is the ordinary state of a term
        // in a graph that does not contain it.
        eprintln!("no definition here for: {}", wanted.join(", "));
        return ExitCode::from(EXIT_NOT_FOUND);
    }
    ExitCode::SUCCESS
}

/// Terms come from publet content and may carry quotes.
fn escape(term: &str) -> String {
    term.replace('\\', "\\\\").replace('"', "\\\"")
}
