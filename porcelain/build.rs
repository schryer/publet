//! Record where this `pub` was built from, for `pub --version`.
//!
//! A release is a `vX.Y.Z` tag on `main`, recorded as a version of the
//! package publet `pkg.publet-cli` (see the README's "Versions and
//! releases"). So `pub --version` claims a version only when built exactly
//! at that version's tag from a clean tree -- anything else says it is an
//! unreleased build -- and names the package publet the release recorded.
//! Without git (a source archive), the version stands as written.

use std::path::Path;
use std::process::Command;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = Path::new(&manifest).join("..");

    let git = root.join(".git");
    for watched in ["HEAD", "refs", "packed-refs", "index"] {
        let path = git.join(watched);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let lock = root.join("corpus").join("corpus.lock");
    println!("cargo:rerun-if-changed={}", lock.display());

    let describe = Command::new("git")
        .args(["describe", "--tags", "--dirty", "--always"])
        .current_dir(&root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    println!("cargo:rustc-env=PUBLET_DESCRIBE={describe}");

    // The lock writes one row per line; the package identity's is the one
    // naming its slug.
    let package = std::fs::read_to_string(&lock)
        .unwrap_or_default()
        .lines()
        .find(|l| l.contains("\"slug\": \"pkg.publet-cli#identity\""))
        .and_then(|l| l.split("\"cid\": \"").nth(1))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned();
    println!("cargo:rustc-env=PUBLET_PACKAGE={package}");
}
