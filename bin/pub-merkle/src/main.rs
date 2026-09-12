//! Compute a membership root from CID lines on stdin.
//!
//! Reads the default stream format, so it composes directly with `pub-ls`.

use std::io::{BufRead as _, Read as _};
use std::process::ExitCode;

use publet_core::{Cid, HashAlg};
use publet_merkle::membership::Membership;

const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut as_cid = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--hex" => as_cid = false,
            "--cid" => as_cid = true,
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("usage: pub-merkle [--cid|--hex] < cids");
                return ExitCode::from(EXIT_USAGE);
            }
        }
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }

    let members: Vec<String> = input
        .as_bytes()
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect();

    let root = Membership::new(members).root();
    if as_cid {
        if let Some(cid) = Cid::from_digest(HashAlg::Sha2_256, &root) {
            println!("{cid}");
        } else {
            eprintln!("digest length does not match the algorithm");
            return ExitCode::from(EXIT_IO);
        }
    } else {
        let hex = publet_merkle::log::to_hex(&root);
        println!("{hex}");
    }
    ExitCode::SUCCESS
}
