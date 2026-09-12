//! Serve a store over the retrieval interface (Section 14.4).
//!
//! Within a declared set, service is unconditional and content-blind: there
//! is no flag that filters by subject, author, or jurisdiction, because
//! Section 13.2 does not permit one.

use std::process::ExitCode;
use std::sync::Arc;

use publet_net::{Node, router};
use publet_store::Store;

const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

#[tokio::main]
async fn main() -> ExitCode {
    let mut store_path = std::path::PathBuf::from("objects.redb");
    let mut bind = "127.0.0.1:8787".to_owned();

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--store=") {
            store_path = std::path::PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--bind=") {
            v.clone_into(&mut bind);
        } else {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-serve [--store=FILE] [--bind=ADDR]");
            return ExitCode::from(EXIT_USAGE);
        }
    }

    let store = match Store::open(&store_path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot bind {bind}: {e}");
            return ExitCode::from(EXIT_IO);
        }
    };
    match listener.local_addr() {
        Ok(addr) => eprintln!("serving {} on http://{addr}", store_path.display()),
        Err(e) => eprintln!("bound, but the address is unknown: {e}"),
    }

    if let Err(e) = axum::serve(listener, router(Node::new(store))).await {
        eprintln!("server stopped: {e}");
        return ExitCode::from(EXIT_IO);
    }
    ExitCode::SUCCESS
}
