//! The trust policy object (Section 11.1).
//!
//! A viewpoint is fully specified by a signed, content-addressed policy, so
//! that any evaluation can be reproduced by anyone holding the policy, the
//! snapshot, and the specification.

use publet_core::{Cid, Object, cbor::Value};
use thiserror::Error;

use crate::Fixed6;

/// Upper bound on iterations, from Section 11.2.
pub const MAX_ITERATIONS: u32 = 200;

/// Why a policy could not be read.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum PolicyError {
    /// A required field was absent.
    #[error("policy is missing `{field}`")]
    MissingField {
        /// The absent field.
        field: &'static str,
    },
    /// A field had the wrong type.
    #[error("policy field `{field}` must be {expected}")]
    BadField {
        /// The offending field.
        field: &'static str,
        /// What is required.
        expected: &'static str,
    },
    /// No trust roots were declared.
    #[error(
        "a policy must declare at least one root; a viewpoint with no anchors assigns zero weight to everything"
    )]
    NoRoots,
    /// Iterations exceeded the protocol ceiling.
    #[error(
        "iterations {found} exceeds the limit of {MAX_ITERATIONS}; unbounded iteration is a denial-of-service vector"
    )]
    TooManyIterations {
        /// The value declared.
        found: u32,
    },
}

/// One trust root and the weight the viewpoint seeds it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The key trusted a priori.
    pub key: Cid,
    /// Relative seed weight.
    pub weight: Fixed6,
}

/// A fully specified viewpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Keys trusted a priori, and their seed weights.
    pub roots: Vec<Root>,
    /// Damping, scaled by 10^6.
    pub damping: Fixed6,
    /// Exactly how many iterations to run. Never a convergence criterion.
    pub iterations: u32,
    /// Base systemic threshold.
    pub tau: Fixed6,
    /// Ceiling on the divergence factor.
    pub delta_max: Fixed6,
    /// Minimum independent consistent reproductions for `accepted`.
    pub replication_floor: u32,
    /// Minimum trust-path distance for two reproductions to be independent.
    pub independence_distance: u32,
    /// Half-life for edge decay, in days. `None` disables decay.
    pub decay_half_life_days: Option<u64>,
    /// Whether equivalence classes are computed from trusted edges.
    pub use_equivalence: bool,
}

impl Policy {
    /// Read a policy from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] if a field is absent, ill-typed, or outside
    /// the bounds the specification sets.
    pub fn from_object(object: &Object) -> Result<Self, PolicyError> {
        let body = object.body();

        let roots = match body.get("roots") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    let key = item
                        .get("key")
                        .and_then(Value::as_text)
                        .and_then(|t| t.parse::<Cid>().ok())
                        .ok_or(PolicyError::BadField {
                            field: "roots.key",
                            expected: "a CID string",
                        })?;
                    let weight = item.get("weight").and_then(Value::as_uint).ok_or(
                        PolicyError::BadField {
                            field: "roots.weight",
                            expected: "a non-negative integer",
                        },
                    )?;
                    Ok(Root {
                        key,
                        weight: Fixed6::from_scaled(i128::from(weight) * crate::SCALE),
                    })
                })
                .collect::<Result<Vec<_>, PolicyError>>()?,
            Some(_) => {
                return Err(PolicyError::BadField {
                    field: "roots",
                    expected: "an array",
                });
            }
            None => return Err(PolicyError::MissingField { field: "roots" }),
        };
        if roots.is_empty() || roots.iter().all(|r| r.weight.is_zero()) {
            return Err(PolicyError::NoRoots);
        }

        let iterations = u32::try_from(uint(body.get("iterations"), "iterations")?)
            .map_err(|_| PolicyError::TooManyIterations { found: u32::MAX })?;
        if iterations > MAX_ITERATIONS {
            return Err(PolicyError::TooManyIterations { found: iterations });
        }

        Ok(Self {
            roots,
            damping: scaled(body.get("damping"), "damping")?,
            iterations,
            tau: scaled(body.get("tau"), "tau")?,
            delta_max: scaled(body.get("delta_max"), "delta_max")?,
            replication_floor: u32::try_from(uint(
                body.get("replication_floor"),
                "replication_floor",
            )?)
            .unwrap_or(u32::MAX),
            independence_distance: u32::try_from(uint(
                body.get("independence_distance"),
                "independence_distance",
            )?)
            .unwrap_or(u32::MAX),
            decay_half_life_days: match body.get("decay_half_life_days") {
                Some(Value::Uint(n)) => Some(*n),
                _ => None,
            },
            use_equivalence: !matches!(
                body.get("equivalence").and_then(Value::as_text),
                Some("none")
            ),
        })
    }
}

fn uint(value: Option<&Value>, field: &'static str) -> Result<u64, PolicyError> {
    match value {
        Some(Value::Uint(n)) => Ok(*n),
        Some(_) => Err(PolicyError::BadField {
            field,
            expected: "a non-negative integer",
        }),
        None => Err(PolicyError::MissingField { field }),
    }
}

/// Read a field already expressed in units of 10^-6.
fn scaled(value: Option<&Value>, field: &'static str) -> Result<Fixed6, PolicyError> {
    Ok(Fixed6::from_scaled(i128::from(uint(value, field)?)))
}
