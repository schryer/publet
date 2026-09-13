//! Bounties and the rule that keeps review honest (Section 12.3).

use publet_core::{Cid, Object, cbor::Value};
use thiserror::Error;

/// The floor on the review share, in basis points (Section 12.3).
///
/// The review share pays for labour and is divided among all qualifying
/// reviewers irrespective of their findings, because the labour is
/// identical whether they affirm, deny, or abstain. A bounty that paid only
/// on one outcome would be buying a conclusion rather than evidence.
pub const MINIMUM_REVIEW_SHARE: u64 = 6000;

/// What a bounty funds (Section 12.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BountyKind {
    /// Reviewer labour, paid for filed review objects meeting `requires`.
    Review,
    /// Executing a declared method, paid for any filed reproduction --
    /// consistent, inconsistent, or method-underspecified alike.
    Replication,
    /// Opening restricted evidence, paid to a controller who grants
    /// independently verifiable access.
    Access,
    /// Deduplication labour, paid for an argued `equivalent` relation.
    Equivalence,
    /// Storage and bandwidth, paid against retrieval proofs.
    Mirror,
}

impl BountyKind {
    /// Resolve the identifier used in a bounty body.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "review" => Self::Review,
            "replication" => Self::Replication,
            "access" => Self::Access,
            "equivalence" => Self::Equivalence,
            "mirror" => Self::Mirror,
            _ => return None,
        })
    }

    /// The identifier used in a bounty body.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Replication => "replication",
            Self::Access => "access",
            Self::Equivalence => "equivalence",
            Self::Mirror => "mirror",
        }
    }
}

/// Why a bounty is not well formed.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum BountyError {
    /// A required field was absent or ill-typed.
    #[error("bounty field `{field}` must be {expected}")]
    BadField {
        /// The offending field.
        field: &'static str,
        /// What is required.
        expected: &'static str,
    },

    /// The review share was below the floor.
    #[error(
        "review share of {found} basis points is below the {MINIMUM_REVIEW_SHARE} \
         floor; the share pays for labour and is divided irrespective of \
         finding, and a bounty below the floor is buying a conclusion"
    )]
    ReviewShareTooLow {
        /// The share declared.
        found: u64,
    },

    /// The kind is not one the specification defines.
    #[error("unknown bounty kind `{found}`")]
    UnknownKind {
        /// The unrecognized identifier.
        found: String,
    },

    /// The bounty named no settlement viewpoint.
    #[error(
        "a bounty must name the policy it settles against; the ledger enforces \
         a choice published in advance, it does not decide whose judgement counts"
    )]
    NoPolicy,
}

/// A posted bounty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounty {
    /// What is being funded.
    pub kind: BountyKind,
    /// The object under review.
    pub target: Cid,
    /// The viewpoint this bounty settles against.
    ///
    /// Named by the poster, published and content-addressed before any work
    /// begins. The ledger enforces that choice; it does not make one.
    pub policy: Cid,
    /// The review share, in basis points.
    pub review_share: u64,
}

impl Bounty {
    /// Read a bounty from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`BountyError`] if a field is absent or ill-typed, if the
    /// review share is below the floor, or if no settlement policy is named.
    pub fn from_object(object: &Object) -> Result<Self, BountyError> {
        let body = object.body();
        let cid = |field: &'static str| -> Result<Cid, BountyError> {
            body.get(field)
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok())
                .ok_or(BountyError::BadField {
                    field,
                    expected: "a CID string",
                })
        };

        let kind_id = body
            .get("kind")
            .and_then(Value::as_text)
            .unwrap_or("review")
            .to_owned();
        let kind =
            BountyKind::from_id(&kind_id).ok_or(BountyError::UnknownKind { found: kind_id })?;

        let policy = cid("policy").map_err(|_| BountyError::NoPolicy)?;

        let review_share =
            body.get("base_share")
                .and_then(Value::as_uint)
                .ok_or(BountyError::BadField {
                    field: "base_share",
                    expected: "basis points",
                })?;
        if review_share < MINIMUM_REVIEW_SHARE {
            return Err(BountyError::ReviewShareTooLow {
                found: review_share,
            });
        }

        Ok(Self {
            kind,
            target: cid("target")?,
            policy,
            review_share,
        })
    }
}
