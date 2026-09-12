//! Produce and verify membership proofs.
//!
//! Three kinds, because one tree cannot do both jobs: inclusion and absence
//! over the sorted membership tree, consistency over the append-only log.

use std::io::Read as _;
use std::process::ExitCode;

use publet_merkle::membership::{Membership, Proof, verify};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut member: Option<String> = None;
    let mut check = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--member=") {
            member = Some(v.to_owned());
        } else if arg == "--verify" {
            check = true;
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-proof --member=CID [--verify] < cids");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let Some(member) = member else {
        eprintln!("--member is required");
        return ExitCode::from(EXIT_USAGE);
    };

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }
    let members = Membership::new(
        input
            .lines()
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty()),
    );

    let proof = members.prove(&member);
    // `Proof` is non-exhaustive, so a future variant must be handled here
    // rather than silently mislabelled.
    let kind = match &proof {
        Proof::Present { .. } => "present",
        Proof::Absent { .. } => "absent",
        _ => "unknown",
    };

    if check && !verify(&member, &proof, &members.root()) {
        eprintln!("proof does not verify against the membership root");
        return ExitCode::from(EXIT_VIOLATION);
    }

    let hex = publet_merkle::log::to_hex(&members.root());
    println!(
        r#"{{"member":"{member}","finding":"{kind}","root":"{hex}","size":{}}}"#,
        members.len()
    );
    ExitCode::SUCCESS
}
