//! Loading a graph from a directory of object files.
//!
//! The plumbing commands all need the same thing: read every object under a
//! directory, verify each against the identifier its filename claims, and
//! index it. Putting it here rather than in each binary keeps the binaries
//! thin and makes this testable.

use std::path::Path;

use publet_core::{Cid, HashAlg, Object};

use crate::{Graph, GraphError};

/// Why a directory could not be loaded.
#[derive(Debug)]
#[non_exhaustive]
pub enum LoadError {
    /// The directory could not be read.
    Io(std::io::Error),
    /// An object failed to parse or verify.
    Object(String, String),
    /// An object was rejected by a graph rule.
    Graph(GraphError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Object(name, why) => write!(f, "{name}: {why}"),
            Self::Graph(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Build a graph from objects already in hand.
///
/// Each pair is verified against its identifier before it is indexed, and
/// objects rejected on the first pass are retried once everything else is
/// in, so arrival order does not decide validity.
///
/// # Errors
///
/// Returns [`LoadError`] if an object fails to parse or verify, or if the
/// graph rejects it once the whole set is present.
pub fn from_objects<I>(objects: I) -> Result<Graph, LoadError>
where
    I: IntoIterator<Item = (Cid, Vec<u8>)>,
{
    let mut graph = Graph::new();
    let mut deferred = Vec::new();
    for (cid, bytes) in objects {
        let verified = Object::parse(&bytes)
            .map_err(|e| LoadError::Object(cid.to_string(), e.to_string()))?
            .verify(&cid)
            .map_err(|e| LoadError::Object(cid.to_string(), e.to_string()))?;
        if graph.insert(&cid, verified).is_err() {
            deferred.push((cid, bytes));
        }
    }
    for (cid, bytes) in deferred {
        let verified = Object::parse(&bytes)
            .map_err(|e| LoadError::Object(cid.to_string(), e.to_string()))?
            .verify(&cid)
            .map_err(|e| LoadError::Object(cid.to_string(), e.to_string()))?;
        graph.insert(&cid, verified).map_err(LoadError::Graph)?;
    }
    graph.validate().map_err(LoadError::Graph)?;
    Ok(graph)
}

/// Load every `*.cbor` file under `dir` into a graph.
///
/// Each file's bytes are verified against their own computed identifier, so
/// a corrupted file is refused rather than indexed.
///
/// # Errors
///
/// Returns [`LoadError`] on the first file that cannot be read, parsed,
/// verified, or admitted.
pub fn from_dir(dir: &Path) -> Result<Graph, LoadError> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(LoadError::Io)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cbor"))
        .collect();
    // Deterministic order: a graph built twice from one directory must be
    // the same graph, and directory iteration order is not guaranteed.
    entries.sort();

    let mut graph = Graph::new();
    let mut deferred = Vec::new();
    for path in entries {
        let bytes = std::fs::read(&path).map_err(LoadError::Io)?;
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        let verified = Object::parse(&bytes)
            .map_err(|e| LoadError::Object(name.clone(), e.to_string()))?
            .verify(&cid)
            .map_err(|e| LoadError::Object(name.clone(), e.to_string()))?;
        // Relations referring to objects not yet loaded are retried once
        // everything else is in, so file order does not decide validity.
        if graph.insert(&cid, verified).is_err() {
            deferred.push((cid, bytes, name));
        }
    }
    for (cid, bytes, name) in deferred {
        let verified = Object::parse(&bytes)
            .map_err(|e| LoadError::Object(name.clone(), e.to_string()))?
            .verify(&cid)
            .map_err(|e| LoadError::Object(name.clone(), e.to_string()))?;
        graph.insert(&cid, verified).map_err(LoadError::Graph)?;
    }
    // Rules whose subject is a target cannot be checked on insert, because
    // objects arrive in whatever order the directory yields. Without this
    // call a class violation slips through whenever the annotation happens
    // to sort before its target.
    graph.validate().map_err(LoadError::Graph)?;
    Ok(graph)
}
