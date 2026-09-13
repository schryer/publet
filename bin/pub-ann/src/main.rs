//! Assumed accountability and triage (Sections 10.7 and 7.4).
//!
//! Both annotations bear on the graph without holding authority in it. An
//! assumption stakes the assumer's own standing on another key's output
//! while recording nothing about who holds that key. A triage annotation
//! orders attention and decides nothing: no evaluation reads one, which is
//! why this binary is the only place they surface at all.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_graph::load;

const EXIT_USAGE: u8 = 2;
const EXIT_LOAD: u8 = 4;

fn usage() -> ExitCode {
    eprintln!(
        "usage: pub-ann [--dir=DIR] --assumptions-by=CID\n\
         \x20      pub-ann [--dir=DIR] --triage-of=CID\n\
         \x20      pub-ann [--dir=DIR] --usage-of=CID"
    );
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let mut dir = PathBuf::from(".");
    let mut assumptions_by: Option<Cid> = None;
    let mut triage_of: Option<Cid> = None;
    let mut usage_of: Option<Cid> = None;

    for arg in std::env::args().skip(1) {
        let parse = |v: &str| -> Option<Cid> { v.parse().ok() };
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--assumptions-by=") {
            let Some(cid) = parse(v) else {
                eprintln!("not a CID: {v}");
                return ExitCode::from(EXIT_USAGE);
            };
            assumptions_by = Some(cid);
        } else if let Some(v) = arg.strip_prefix("--usage-of=") {
            let Some(cid) = parse(v) else {
                eprintln!("not a CID: {v}");
                return ExitCode::from(EXIT_USAGE);
            };
            usage_of = Some(cid);
        } else if let Some(v) = arg.strip_prefix("--triage-of=") {
            let Some(cid) = parse(v) else {
                eprintln!("not a CID: {v}");
                return ExitCode::from(EXIT_USAGE);
            };
            triage_of = Some(cid);
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    if assumptions_by.is_none() && triage_of.is_none() && usage_of.is_none() {
        return usage();
    }

    let graph = match load::from_dir(&dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_LOAD);
        }
    };

    if let Some(assumer) = assumptions_by {
        for assumption in graph.assumptions_by(&assumer) {
            // Two keys and a basis. There is no field for who holds the
            // assumed key, because writing that down would create the
            // compellable artifact the design exists to avoid.
            println!(
                r#"{{"assumer":"{}","assumed":"{}","basis":"{}"}}"#,
                assumption.assumer,
                assumption.assumed,
                assumption.basis.id()
            );
        }
    }

    if let Some(target) = usage_of {
        for (source, locator) in graph.usage_of(&target) {
            println!(
                r#"{{"source":"{}","locator":"{}"}}"#,
                source.replace('"', "\\\""),
                locator.replace('"', "\\\"")
            );
        }
    }

    if let Some(target) = triage_of {
        for triage in graph.triage_of(&target) {
            println!(
                r#"{{"target":"{}","finding":"{}","engine":"{}"}}"#,
                triage.target,
                triage.finding.replace('"', "\\\""),
                triage.engine.replace('"', "\\\"")
            );
        }
    }

    ExitCode::SUCCESS
}
