//! Emit objects by identifier, reading identifiers from arguments or stdin.
//!
//! Reads CID lines, the default stream format, so it sits at the end of a
//! pipeline that selected what to fetch. Bytes are verified against the
//! identifier before they are emitted: a store that returned the wrong
//! object would otherwise be indistinguishable from one that returned the
//! right one.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::Cid;
use publet_store::Store;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_FOUND: u8 = 3;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut path = PathBuf::from("objects.redb");
    let mut targets: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--store=") {
            path = PathBuf::from(v);
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-cat [--store=FILE] [CID...]");
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

    let store = match Store::open(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let mut out = std::io::stdout().lock();
    for target in targets {
        let Ok(cid) = target.parse::<Cid>() else {
            eprintln!("not a CID: {target}");
            return ExitCode::from(EXIT_USAGE);
        };
        match store.get(&cid) {
            Ok(Some(bytes)) => {
                // Re-verify on the way out. The store checks on write, but
                // a caller holding these bytes has no other assurance.
                if !cid.verifies(&bytes) {
                    eprintln!("{cid}: stored bytes no longer match their identifier");
                    return ExitCode::from(EXIT_VIOLATION);
                }
                if let Err(e) = out.write_all(&bytes) {
                    eprintln!("cannot write stdout: {e}");
                    return ExitCode::from(EXIT_IO);
                }
            }
            Ok(None) => {
                eprintln!("not in the store: {cid}");
                return ExitCode::from(EXIT_NOT_FOUND);
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_IO);
            }
        }
    }
    ExitCode::SUCCESS
}
