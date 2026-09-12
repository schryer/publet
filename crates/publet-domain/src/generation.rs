//! Generation records and their validity rules (Section 14.1.1).
//!
//! A generation states its membership change explicitly. The alternative --
//! leaving a removal to be discovered by comparing an inclusion proof in one
//! generation against an absence proof in the next -- reveals a removal only
//! to a reader who thinks to look. Requiring the publisher to declare it,
//! with a justification, as a condition of the generation parsing at all is
//! the difference between auditable and audited.

use std::collections::BTreeSet;

use publet_core::{Cid, Object, cbor::Value};
use publet_merkle::membership::Membership;
use thiserror::Error;

/// Why an object accounts for a member leaving a generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RemovalCause {
    /// The member moved to a domain produced by splitting this one.
    Split,
    /// A node published a tombstone for it (Section 13.2).
    Tombstone,
    /// The domain itself was superseded.
    SupersededDomain,
}

impl RemovalCause {
    /// Resolve the identifier used in a generation record.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "split" => Self::Split,
            "tombstone" => Self::Tombstone,
            "superseded-domain" => Self::SupersededDomain,
            _ => return None,
        })
    }

    /// The identifier used in a generation record.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::Tombstone => "tombstone",
            Self::SupersededDomain => "superseded-domain",
        }
    }
}

/// One declared removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    /// The member that left.
    pub cid: Cid,
    /// Why it left.
    pub cause: RemovalCause,
    /// The object accounting for it.
    pub reference: Cid,
}

/// A generation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    /// The domain this generation belongs to.
    pub domain: Cid,
    /// Monotonic index, with no gaps.
    pub index: u64,
    /// The previous generation record, absent only for index 0.
    pub parent: Option<Cid>,
    /// Membership root after this change.
    pub snapshot: Cid,
    /// Members added.
    pub added: Vec<Cid>,
    /// Members removed, each with its justification.
    pub removed: Vec<Removal>,
}

/// Why a generation record is malformed.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum GenerationError {
    /// A required field was absent.
    #[error("generation is missing `{field}`")]
    MissingField {
        /// The absent field.
        field: &'static str,
    },
    /// A field had the wrong type or shape.
    #[error("generation field `{field}` must be {expected}")]
    BadField {
        /// The offending field.
        field: &'static str,
        /// What is required.
        expected: &'static str,
    },
    /// A removal named no cause the specification defines.
    #[error("removal of {cid} names an unknown cause `{cause}`")]
    UnknownCause {
        /// The member removed.
        cid: String,
        /// The unrecognized cause.
        cause: String,
    },
    /// A removal carried no accounting reference.
    #[error(
        "removal of {cid} has no `ref` accounting for it; \
         a generation may not drop a member without saying why"
    )]
    UnaccountedRemoval {
        /// The member removed.
        cid: String,
    },
    /// The record's membership change does not match the roots.
    #[error(
        "generation {index} declares {declared} removal(s) but its membership \
         implies {implied}; an undeclared removal makes the generation malformed"
    )]
    UndeclaredRemoval {
        /// The generation index.
        index: u64,
        /// How many removals were declared.
        declared: usize,
        /// How many the membership change implies.
        implied: usize,
    },
    /// The record declares an addition its membership does not contain.
    #[error("generation {index} declares an addition its membership lacks: {cid}")]
    PhantomAddition {
        /// The generation index.
        index: u64,
        /// The member declared but absent.
        cid: String,
    },
    /// Indices did not advance by exactly one.
    #[error(
        "generation index jumped from {previous} to {found}; indices are monotonic with no gaps"
    )]
    IndexGap {
        /// The previous index.
        previous: u64,
        /// The index found.
        found: u64,
    },
    /// The record does not name the generation it follows.
    #[error("generation {index} does not name its parent")]
    MissingParent {
        /// The generation index.
        index: u64,
    },
}

impl Generation {
    /// Read a generation record from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationError`] if a field is absent or ill-typed, if a
    /// removal names an unknown cause, or if a removal carries no `ref`.
    pub fn from_object(object: &Object) -> Result<Self, GenerationError> {
        let body = object.body();
        let cid = |field: &'static str| -> Result<Cid, GenerationError> {
            body.get(field)
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok())
                .ok_or(GenerationError::BadField {
                    field,
                    expected: "a CID string",
                })
        };

        let added = match body.get("added") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_text()
                        .and_then(|t| t.parse().ok())
                        .ok_or(GenerationError::BadField {
                            field: "added",
                            expected: "an array of CID strings",
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(GenerationError::BadField {
                    field: "added",
                    expected: "an array",
                });
            }
            None => Vec::new(),
        };

        let removed = match body.get("removed") {
            Some(Value::Array(items)) => items
                .iter()
                .map(parse_removal)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(GenerationError::BadField {
                    field: "removed",
                    expected: "an array",
                });
            }
            None => Vec::new(),
        };

        Ok(Self {
            domain: cid("domain")?,
            index: body
                .get("index")
                .and_then(Value::as_uint)
                .ok_or(GenerationError::MissingField { field: "index" })?,
            parent: body
                .get("parent")
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok()),
            snapshot: cid("snapshot")?,
            added,
            removed,
        })
    }

    /// Check the record against the membership it claims to produce.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationError`] if the record declares an addition the
    /// membership lacks, or if the membership implies a removal the record
    /// does not declare.
    pub fn check_against(
        &self,
        previous: &Membership,
        next: &Membership,
    ) -> Result<(), GenerationError> {
        let (implied_added, implied_removed) = publet_merkle::membership::diff(previous, next);

        let declared_added: BTreeSet<String> = self.added.iter().map(ToString::to_string).collect();
        let declared_removed: BTreeSet<String> =
            self.removed.iter().map(|r| r.cid.to_string()).collect();

        for cid in &declared_added {
            if !implied_added.contains(cid) {
                return Err(GenerationError::PhantomAddition {
                    index: self.index,
                    cid: cid.clone(),
                });
            }
        }
        // An undeclared removal is the failure this rule exists to catch.
        if implied_removed
            .iter()
            .any(|c| !declared_removed.contains(c))
        {
            return Err(GenerationError::UndeclaredRemoval {
                index: self.index,
                declared: declared_removed.len(),
                implied: implied_removed.len(),
            });
        }
        Ok(())
    }

    /// Check that this record follows `previous`.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationError`] if the index does not advance by one or
    /// if the parent is not named.
    pub fn follows(&self, previous: &Self) -> Result<(), GenerationError> {
        if self.index != previous.index + 1 {
            return Err(GenerationError::IndexGap {
                previous: previous.index,
                found: self.index,
            });
        }
        if self.parent.is_none() {
            return Err(GenerationError::MissingParent { index: self.index });
        }
        Ok(())
    }
}

fn parse_removal(value: &Value) -> Result<Removal, GenerationError> {
    let cid: Cid = value
        .get("cid")
        .and_then(Value::as_text)
        .and_then(|t| t.parse().ok())
        .ok_or(GenerationError::BadField {
            field: "removed.cid",
            expected: "a CID string",
        })?;
    let cause_id =
        value
            .get("cause")
            .and_then(Value::as_text)
            .ok_or(GenerationError::BadField {
                field: "removed.cause",
                expected: "a text string",
            })?;
    let cause = RemovalCause::from_id(cause_id).ok_or_else(|| GenerationError::UnknownCause {
        cid: cid.to_string(),
        cause: cause_id.to_owned(),
    })?;
    // The rule that makes removals accountable rather than merely visible.
    let reference = value
        .get("ref")
        .and_then(Value::as_text)
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| GenerationError::UnaccountedRemoval {
            cid: cid.to_string(),
        })?;
    Ok(Removal {
        cid,
        cause,
        reference,
    })
}
