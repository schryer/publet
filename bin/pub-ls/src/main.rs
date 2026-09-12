//! List identifiers in a store, optionally filtered by type and class.
//!
//! Emits CID lines, the default stream format, so the output pipes into
//! every other plumbing command.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_graph::{Class, load};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut want_type: Option<String> = None;
    let mut want_class: Option<Class> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--type=") {
            want_type = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--class=") {
            if let Some(c) = Class::from_id(v) {
                want_class = Some(c);
            } else {
                eprintln!("unknown claim class: {v}");
                return ExitCode::from(EXIT_USAGE);
            }
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-ls [--dir=DIR] [--type=KIND] [--class=CLASS]");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    for cid_text in graph.cids() {
        let Ok(cid) = cid_text.parse() else { continue };
        if let Some(kind) = &want_type
            && graph.object(&cid).is_none_or(|o| o.kind() != kind)
        {
            continue;
        }
        if let Some(class) = want_class
            && graph.publet(&cid).is_none_or(|p| p.class() != class)
        {
            continue;
        }
        println!("{cid_text}");
    }
    ExitCode::SUCCESS
}
