//! Synchronize a domain from a peer by delta (Section 14.3.1).
//!
//! The request carries a domain identifier and two generation indices. It
//! discloses nothing about which objects the caller already holds, because
//! Section 14.3.2 forbids that and because partial holdings fingerprint what
//! a reader has been working with.
//!
//! Every object received is verified against its own identifier before it is
//! stored, so a peer serving altered bytes is caught here rather than later.

use std::process::ExitCode;

use publet_core::{Cid, HashAlg};
use publet_net::Client;
use publet_store::Store;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

#[tokio::main]
async fn main() -> ExitCode {
    let mut store_path = std::path::PathBuf::from("objects.redb");
    let mut peer: Option<String> = None;
    let mut proxy: Option<String> = None;
    let mut domain: Option<String> = None;
    let mut from = 0u64;
    let mut to: Option<u64> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--store=") {
            store_path = std::path::PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--peer=") {
            peer = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--proxy=") {
            proxy = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--from=") {
            from = v.parse().unwrap_or(0);
        } else if let Some(v) = arg.strip_prefix("--to=") {
            to = v.parse().ok();
        } else if let Some(v) = arg.strip_prefix("--domain=") {
            domain = Some(v.to_owned());
        } else if arg.starts_with("--") {
            eprintln!("unknown argument: {arg}");
            eprintln!(
                "usage: pub-sync --peer=URL --domain=CID [--from=N] [--to=N] \
                 [--store=FILE] [--proxy=URL]"
            );
            return ExitCode::from(EXIT_USAGE);
        } else {
            domain = Some(arg);
        }
    }

    let (Some(peer), Some(domain)) = (peer, domain) else {
        eprintln!("--peer and --domain are required");
        return ExitCode::from(EXIT_USAGE);
    };
    let Ok(domain) = domain.parse::<Cid>() else {
        eprintln!("--domain must be a CID");
        return ExitCode::from(EXIT_USAGE);
    };

    let client = match proxy {
        Some(p) => Client::with_proxy(&peer, &p),
        None => Client::new(&peer),
    };
    let client = match client {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let to = match to {
        Some(n) => n,
        None => match client.checkpoints(&domain).await {
            Ok(points) => points.iter().copied().max().map_or(from, |m| m + 1),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_IO);
            }
        },
    };

    let objects = match client.delta(&domain, from, to).await {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let store = match Store::open(&store_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let mut stored = 0usize;
    for bytes in objects {
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        match store.put(&cid, &bytes) {
            Ok(()) => stored += 1,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_VIOLATION);
            }
        }
    }

    println!(r#"{{"domain":"{domain}","from":{from},"to":{to},"stored":{stored}}}"#);
    ExitCode::SUCCESS
}
