//! Emit the content identifier of an object read on stdin.
//!
//! The bytes are validated first: a CID computed over non-canonical bytes
//! would name an object that cannot legitimately exist.

use std::io::Read as _;
use std::process::ExitCode;

use publet_core::{Cid, HashAlg};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut alg = HashAlg::Sha2_256;
    for arg in std::env::args().skip(1) {
        if let Some(id) = arg.strip_prefix("--alg=") {
            if let Some(a) = HashAlg::from_id(id) {
                alg = a;
            } else {
                eprintln!("unknown hash algorithm: {id}");
                return ExitCode::from(EXIT_USAGE);
            }
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-cid [--alg=sha2-256] < object.cbor");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }

    if let Err(err) = publet_core::cbor::decode(&input) {
        eprintln!("rule: {}", err.rule());
        eprintln!("{err}");
        return ExitCode::from(EXIT_VIOLATION);
    }

    println!("{}", Cid::of(&input, alg));
    ExitCode::SUCCESS
}
