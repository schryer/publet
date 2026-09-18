//! Assumed accountability and triage (Sections 10.7 and 7.4), plus a few
//! other annotation-kind lookups (usage, subject membership, tags) that
//! share the same shape: read here because the annotation is what carries
//! the claim, never the object it targets.

use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_graph::Graph;
use publet_graph::load;

const EXIT_USAGE: u8 = 2;
const EXIT_LOAD: u8 = 4;

/// Which lookup was requested and the CID it targets.
#[derive(Default)]
struct Targets {
    dir: PathBuf,
    assumptions_by: Option<Cid>,
    triage_of: Option<Cid>,
    usage_of: Option<Cid>,
    subjects_of: Option<Cid>,
    classified_under: Option<Cid>,
    tagged_of: Option<Cid>,
}

impl Targets {
    fn any_set(&self) -> bool {
        self.assumptions_by.is_some()
            || self.triage_of.is_some()
            || self.usage_of.is_some()
            || self.subjects_of.is_some()
            || self.classified_under.is_some()
            || self.tagged_of.is_some()
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: pub-ann [--dir=DIR] --assumptions-by=CID\n\
         \x20      pub-ann [--dir=DIR] --triage-of=CID\n\
         \x20      pub-ann [--dir=DIR] --usage-of=CID\n\
         \x20      pub-ann [--dir=DIR] --subjects-of=CID\n\
         \x20      pub-ann [--dir=DIR] --classified-under=CID\n\
         \x20      pub-ann [--dir=DIR] --tagged-of=CID"
    );
    ExitCode::from(EXIT_USAGE)
}

/// A flag's prefix and how to reach the `Targets` slot it fills.
type FlagSlot = (&'static str, fn(&mut Targets) -> &mut Option<Cid>);

/// Parse argv into `Targets`, or `Err` with the exit code to return.
fn parse_args(args: impl Iterator<Item = String>) -> Result<Targets, ExitCode> {
    let mut targets = Targets {
        dir: PathBuf::from("."),
        ..Targets::default()
    };

    let flags: [FlagSlot; 6] = [
        ("--assumptions-by=", |t| &mut t.assumptions_by),
        ("--subjects-of=", |t| &mut t.subjects_of),
        ("--classified-under=", |t| &mut t.classified_under),
        ("--usage-of=", |t| &mut t.usage_of),
        ("--triage-of=", |t| &mut t.triage_of),
        ("--tagged-of=", |t| &mut t.tagged_of),
    ];

    for arg in args {
        if let Some(v) = arg.strip_prefix("--dir=") {
            targets.dir = PathBuf::from(v);
            continue;
        }
        let Some((_, slot)) = flags.iter().find(|(prefix, _)| arg.starts_with(prefix)) else {
            eprintln!("unknown argument: {arg}");
            return Err(usage());
        };
        let v = arg.split_once('=').map_or("", |(_, v)| v);
        let Ok(cid) = v.parse::<Cid>() else {
            eprintln!("not a CID: {v}");
            return Err(ExitCode::from(EXIT_USAGE));
        };
        *slot(&mut targets) = Some(cid);
    }

    if !targets.any_set() {
        return Err(usage());
    }
    Ok(targets)
}

/// Print every lookup the caller asked for.
fn print_results(graph: &Graph, targets: &Targets) {
    if let Some(assumer) = &targets.assumptions_by {
        for assumption in graph.assumptions_by(assumer) {
            // Two keys and a basis. There is no field for who holds the
            // assumed key, because writing that down would create the
            // compellable artifact the design exists to avoid.
            println!(
                r#"{{"assumer":"{}","assumed":"{}","grounds":"{}"}}"#,
                assumption.assumer,
                assumption.assumed,
                assumption.basis.id()
            );
        }
    }

    // Subject membership is an annotation rather than a property of the
    // object (R6), so it is read here rather than from the object itself,
    // and competing taxonomies coexist as sets of these by different keys.
    if let Some(target) = &targets.subjects_of {
        for subject in graph.subjects_of(target) {
            println!("{subject}");
        }
    }

    if let Some(subject) = &targets.classified_under {
        for member in graph.classified_under(subject) {
            println!("{member}");
        }
    }

    if let Some(target) = &targets.usage_of {
        for (source, locator) in graph.usage_of(target) {
            println!(
                r#"{{"source":"{}","locator":"{}"}}"#,
                source.replace('"', "\\\""),
                locator.replace('"', "\\\"")
            );
        }
    }

    if let Some(target) = &targets.triage_of {
        for triage in graph.triage_of(target) {
            println!(
                r#"{{"target":"{}","finding":"{}","engine":"{}"}}"#,
                triage.target,
                triage.finding.replace('"', "\\\""),
                triage.engine.replace('"', "\\\"")
            );
        }
    }

    // Section 9.1: the tag is an annotation, never a field on the tagged
    // object itself, so it is read here the same way subject membership is.
    if let Some(target) = &targets.tagged_of {
        for tag in graph.tags_of(target) {
            println!("{tag}");
        }
    }
}

fn main() -> ExitCode {
    let targets = match parse_args(std::env::args().skip(1)) {
        Ok(t) => t,
        Err(code) => return code,
    };

    let graph = match load::from_dir(&targets.dir) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_LOAD);
        }
    };

    print_results(&graph, &targets);
    ExitCode::SUCCESS
}
