//! Create and verify detached signatures (Section 4.4).
//!
//! The signed message is domain-separated:
//!
//! ```text
//! "pub/v1/sig" || 0x00 || purpose || 0x00 || canonical-bytes-of-target
//! ```
//!
//! `--purpose` is required on both sides and is compared before any
//! cryptography, so a signature made for one purpose cannot be presented as
//! a signature for another. There is no default, because a default would be
//! a claim about intent the signer never made.

use std::io::Read as _;
use std::process::ExitCode;

use publet_core::{SigAlg, signing_message, verify};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn usage() -> ExitCode {
    eprintln!("usage: pub-sign --purpose=P --message < object.cbor");
    eprintln!("       pub-sign --purpose=P --verify --key=HEX --sig=HEX < object.cbor");
    eprintln!();
    eprintln!("  --message  emit the domain-separated bytes a signature covers");
    eprintln!("  --verify   check a signature against those bytes");
    ExitCode::from(EXIT_USAGE)
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Vec<&str> = text
        .as_bytes()
        .chunks(2)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();
    bytes
        .iter()
        .map(|pair| u8::from_str_radix(pair, 16).ok())
        .collect()
}

fn main() -> ExitCode {
    let mut purpose: Option<String> = None;
    let mut key: Option<String> = None;
    let mut signature: Option<String> = None;
    let mut show_message = false;
    let mut check = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--purpose=") {
            purpose = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--key=") {
            key = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--sig=") {
            signature = Some(v.to_owned());
        } else if arg == "--message" {
            show_message = true;
        } else if arg == "--verify" {
            check = true;
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    let Some(purpose) = purpose else {
        eprintln!("--purpose is required; a signature covers a stated purpose");
        return usage();
    };

    let mut target = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut target) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }

    let message = match signing_message(&purpose, &target) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_VIOLATION);
        }
    };

    if show_message {
        let hex: String = message.iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{b:02x}");
            acc
        });
        println!("{hex}");
        return ExitCode::SUCCESS;
    }

    if check {
        let (Some(key), Some(signature)) = (key, signature) else {
            eprintln!("--key and --sig are required with --verify");
            return usage();
        };
        let (Some(key), Some(signature)) = (from_hex(&key), from_hex(&signature)) else {
            eprintln!("--key and --sig must be hexadecimal");
            return ExitCode::from(EXIT_USAGE);
        };
        return match verify(
            SigAlg::Ed25519,
            &key,
            &signature,
            &purpose,
            &purpose,
            &target,
        ) {
            Ok(()) => {
                println!("ok {purpose}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(EXIT_VIOLATION)
            }
        };
    }

    usage()
}
