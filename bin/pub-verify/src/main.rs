//! Verify an object: canonical form, well-formed header, and identifier.
//!
//! With `--cid=<CID>` the bytes are checked against that identifier, which
//! is the check Section 4.2 requires of anyone who fetched them. Without
//! it, the identifier is reported rather than confirmed, since there is
//! nothing to confirm it against.

use std::io::Read as _;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg, Object};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut expected: Option<String> = None;
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--cid=") {
            expected = Some(v.to_owned());
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-verify [--cid=<CID>] < object.cbor");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }

    let parsed = match Object::parse(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    let cid = match expected {
        Some(text) => match text.parse::<Cid>() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_USAGE);
            }
        },
        None => Cid::of(&input, HashAlg::Sha2_256),
    };

    match parsed.verify(&cid) {
        Ok(v) => {
            println!("{} {} {}", cid, v.object().kind(), v.object().created());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}
