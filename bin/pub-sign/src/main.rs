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
//!
//! `--generate-key` and `--sign` are what this tool was missing: it could
//! compute the bytes a signature covers and check one handed to it, but
//! nothing here ever produced a real one from a private key, because
//! nothing here could generate a private key either. Deliberately the
//! lowest layer of that: raw hex in, raw hex out, no workspace, no object
//! store, no opinion about where a caller keeps the result. `porcelain`'s
//! `pub sign` is the layer that knows about `.publet/signing.key` and
//! writes a `sig` object; this one only knows Ed25519.

use std::io::Read as _;
use std::process::ExitCode;

use publet_core::{SigAlg, sign, signing_message, verify, verifying_key};

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn usage() -> ExitCode {
    eprintln!("usage: pub-sign --purpose=P --message < object.cbor");
    eprintln!("       pub-sign --purpose=P --sign --key=HEX < object.cbor");
    eprintln!("       pub-sign --purpose=P --verify --key=HEX --sig=HEX < object.cbor");
    eprintln!("       pub-sign --generate-key");
    eprintln!();
    eprintln!("  --message        emit the domain-separated bytes a signature covers");
    eprintln!("  --sign           produce a signature over those bytes with a secret key");
    eprintln!("  --verify         check a signature against those bytes");
    eprintln!("  --generate-key   print a new Ed25519 secret and public key, hex-encoded");
    ExitCode::from(EXIT_USAGE)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    })
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

/// A fresh Ed25519 secret key, from the OS's own CSPRNG -- never a
/// pseudo-random source this binary seeds itself, which is exactly the
/// mistake that has broken Ed25519 keys in the wild before.
///
/// # Errors
///
/// Returns a message if the OS cannot supply randomness.
fn generate_key() -> Result<[u8; 32], String> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed)
        .map_err(|e| format!("could not read system randomness: {e}"))?;
    Ok(seed)
}

fn run_generate() -> ExitCode {
    let seed = match generate_key() {
        Ok(seed) => seed,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };
    match verifying_key(SigAlg::Ed25519, &seed) {
        Ok(public) => {
            println!("secret {}", to_hex(&seed));
            println!("public {}", to_hex(&public));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

fn run_sign(purpose: &str, key: Option<String>, target: &[u8]) -> ExitCode {
    let Some(key) = key else {
        eprintln!("--key is required with --sign");
        return usage();
    };
    let Some(key) = from_hex(&key) else {
        eprintln!("--key must be hexadecimal");
        return ExitCode::from(EXIT_USAGE);
    };
    match sign(SigAlg::Ed25519, &key, purpose, target) {
        Ok(signature) => {
            println!("{}", to_hex(&signature));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

fn run_verify(
    purpose: &str,
    key: Option<String>,
    signature: Option<String>,
    target: &[u8],
) -> ExitCode {
    let (Some(key), Some(signature)) = (key, signature) else {
        eprintln!("--key and --sig are required with --verify");
        return usage();
    };
    let (Some(key), Some(signature)) = (from_hex(&key), from_hex(&signature)) else {
        eprintln!("--key and --sig must be hexadecimal");
        return ExitCode::from(EXIT_USAGE);
    };
    match verify(SigAlg::Ed25519, &key, &signature, purpose, purpose, target) {
        Ok(()) => {
            println!("ok {purpose}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

fn main() -> ExitCode {
    let mut purpose: Option<String> = None;
    let mut key: Option<String> = None;
    let mut signature: Option<String> = None;
    let mut show_message = false;
    let mut do_sign = false;
    let mut check = false;
    let mut do_generate = false;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--purpose=") {
            purpose = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--key=") {
            key = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--sig=") {
            signature = Some(v.to_owned());
        } else if arg == "--message" {
            show_message = true;
        } else if arg == "--sign" {
            do_sign = true;
        } else if arg == "--verify" {
            check = true;
        } else if arg == "--generate-key" {
            do_generate = true;
        } else {
            eprintln!("unknown argument: {arg}");
            return usage();
        }
    }

    if do_generate {
        return run_generate();
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
        println!("{}", to_hex(&message));
        return ExitCode::SUCCESS;
    }

    if do_sign {
        return run_sign(&purpose, key, &target);
    }

    if check {
        return run_verify(&purpose, key, signature, &target);
    }

    usage()
}
