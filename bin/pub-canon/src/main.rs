//! Validate canonical CBOR.
//!
//! Reads bytes on stdin. On success writes them to stdout unchanged and
//! exits 0. On failure writes nothing to stdout, names the violated rule on
//! stderr, and exits 1 -- Section 4.1 requires rejecting rather than
//! repairing, so there is deliberately no flag that makes this normalize.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

/// Protocol violation.
const EXIT_VIOLATION: u8 = 1;
/// Usage error.
const EXIT_USAGE: u8 = 2;
/// I/O error.
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = match parse_args(&args) {
        Ok(v) => v,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: pub-canon [--format=json] < object.cbor");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("cannot read stdin: {e}");
        return ExitCode::from(EXIT_IO);
    }

    match publet_core::cbor::decode(&input) {
        Ok(_) => {
            if let Err(e) = std::io::stdout().write_all(&input) {
                eprintln!("cannot write stdout: {e}");
                return ExitCode::from(EXIT_IO);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            if json {
                // Hand-built so the tool has no serialization dependency;
                // the fields here are what the conformance scenarios read.
                eprintln!(
                    "{{\"ok\":false,\"rule\":\"{}\",\"detail\":\"{}\"}}",
                    escape(err.rule()),
                    escape(&err.to_string())
                );
            } else {
                eprintln!("rule: {}", err.rule());
                eprintln!("{err}");
            }
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

fn parse_args(args: &[String]) -> Result<bool, String> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--format=json" => json = true,
            "--format=text" => json = false,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(json)
}

fn escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            c => vec![c],
        })
        .collect()
}
