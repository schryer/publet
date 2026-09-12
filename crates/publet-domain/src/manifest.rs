//! Domain manifests and their constraints (Section 14.1).
//!
//! Three constraints make local-first operation achievable, and they are
//! normative because without them the property fails silently rather than
//! loudly: a domain is closed under `depends`, contains no blobs, and
//! respects a declared size bound.

use publet_core::{Cid, Object, cbor::Value};
use thiserror::Error;

/// A domain manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Human-readable label. Advisory only.
    pub label: String,
    /// The membership snapshot this manifest names.
    pub snapshot: Cid,
    /// Declared byte ceiling.
    pub bound: u64,
    /// Actual size in bytes.
    pub size: u64,
    /// Relations the domain is closed under.
    pub closed_under: Vec<String>,
    /// The predecessor, when this manifest was produced by a split.
    pub split_of: Option<Cid>,
}

/// Why a manifest is invalid.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum ManifestError {
    /// A required field was absent.
    #[error("manifest is missing `{field}`")]
    MissingField {
        /// The absent field.
        field: &'static str,
    },
    /// A field had the wrong type.
    #[error("manifest field `{field}` must be {expected}")]
    BadField {
        /// The offending field.
        field: &'static str,
        /// What is required.
        expected: &'static str,
    },
    /// The domain exceeds the bound it declared.
    #[error(
        "domain is {size} bytes against a declared bound of {bound}; \
         it must be split, or local-first retrieval stops being affordable"
    )]
    OverBound {
        /// Actual size.
        size: u64,
        /// Declared ceiling.
        bound: u64,
    },
    /// The domain is not closed under `depends`.
    #[error(
        "domain is not closed under `depends`: {cid} is required by a member \
         and absent; evaluating a replica would need a fetch, and the privacy \
         property is lost on the first unresolved term"
    )]
    NotClosed {
        /// The missing dependency.
        cid: String,
    },
}

impl Manifest {
    /// Read a manifest from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] if a field is absent or ill-typed, or if
    /// the declared size exceeds the declared bound.
    pub fn from_object(object: &Object) -> Result<Self, ManifestError> {
        let body = object.body();
        let uint = |field: &'static str| -> Result<u64, ManifestError> {
            body.get(field)
                .and_then(Value::as_uint)
                .ok_or(ManifestError::MissingField { field })
        };
        let manifest = Self {
            label: body
                .get("label")
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_owned(),
            snapshot: body
                .get("snapshot")
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok())
                .ok_or(ManifestError::BadField {
                    field: "snapshot",
                    expected: "a CID string",
                })?,
            bound: uint("bound")?,
            size: uint("size")?,
            closed_under: match body.get("closed_under") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| v.as_text().map(ToOwned::to_owned))
                    .collect(),
                _ => Vec::new(),
            },
            split_of: body
                .get("split_of")
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok()),
        };
        if manifest.size > manifest.bound {
            return Err(ManifestError::OverBound {
                size: manifest.size,
                bound: manifest.bound,
            });
        }
        Ok(manifest)
    }

    /// Whether the manifest claims closure under `depends`.
    #[must_use]
    pub fn claims_depends_closure(&self) -> bool {
        self.closed_under.iter().any(|r| r == "depends")
    }
}

/// Check that every member's `depends` closure is inside the domain.
///
/// # Errors
///
/// Returns [`ManifestError::NotClosed`] naming the first absent dependency.
pub fn check_depends_closure(
    graph: &publet_graph::Graph,
    members: &[String],
) -> Result<(), ManifestError> {
    let present: std::collections::BTreeSet<&str> = members.iter().map(String::as_str).collect();
    for member in members {
        let Ok(cid) = member.parse::<Cid>() else {
            continue;
        };
        let Ok(closure) = graph.depends_closure(&cid) else {
            continue;
        };
        for dependency in closure {
            let text = dependency.to_string();
            if !present.contains(text.as_str()) {
                return Err(ManifestError::NotClosed { cid: text });
            }
        }
    }
    Ok(())
}
