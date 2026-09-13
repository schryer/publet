//! What a ledger must provide, and what it may never decide (Section 12.2).
//!
//! This document specifies no consensus mechanism. It specifies the
//! properties a ledger must have to be usable, and any ledger with them may
//! be used. The trait is therefore about *settlement*, not about ordering:
//! whichever chain, notary, or escrow agent provides the properties below
//! plugs in here, and none of them acquires a say in what is true.

use std::collections::BTreeSet;

use publet_core::Cid;
use thiserror::Error;

/// One property the specification requires of a ledger (Section 12.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum LedgerProperty {
    /// L-1: a deterministic order over accepted transactions.
    TotalOrder,
    /// L-3: funds can be locked and released by predicate.
    ConditionalRelease,
    /// L-4: transactions can carry protocol identifiers durably.
    CarriesIdentifiers,
    /// L-5: no permission is required to post, claim, or verify.
    OpenParticipation,
}

impl LedgerProperty {
    /// Every property a conformant ledger must claim.
    #[must_use]
    pub fn required() -> &'static [Self] {
        &[
            Self::TotalOrder,
            Self::ConditionalRelease,
            Self::CarriesIdentifiers,
            Self::OpenParticipation,
        ]
    }

    /// The identifier used in diagnostics.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::TotalOrder => "a total order over transactions",
            Self::ConditionalRelease => "conditional release of locked funds",
            Self::CarriesIdentifiers => "durable carriage of protocol identifiers",
            Self::OpenParticipation => "open participation",
        }
    }
}

/// What a ledger claims to provide.
///
/// Declared rather than measured. Whether a given ledger actually has these
/// is a question about that ledger, answered by reading its specification
/// and its operating record -- not by this code, which can only record what
/// was claimed and make the claim reviewable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerProperties {
    /// The properties claimed.
    pub claimed: BTreeSet<LedgerProperty>,
    /// L-2: the stated condition after which a transaction will not reverse.
    pub finality_condition: String,
    /// L-6: the stated bound on how long a valid transaction can be excluded.
    pub censorship_bound: String,
}

impl LedgerProperties {
    /// Properties the specification requires and this ledger does not claim.
    #[must_use]
    pub fn missing(&self) -> Vec<LedgerProperty> {
        LedgerProperty::required()
            .iter()
            .copied()
            .filter(|p| !self.claimed.contains(p))
            .collect()
    }

    /// Whether every required property is claimed and both bounds stated.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.missing().is_empty()
            && !self.finality_condition.is_empty()
            && !self.censorship_bound.is_empty()
    }
}

/// The outcome of a bounty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// The bounty settled.
    pub bounty: Cid,
    /// Keys paid the review share, irrespective of what they found.
    pub paid: Vec<Cid>,
    /// Whether the system share returned to the poster.
    ///
    /// It returns on *resolution* in either direction, never on acceptance.
    /// A poster refunded for being accepted would have reason to select a
    /// lenient settlement policy, and under viewpoint-relative evaluation a
    /// lenient policy can always be found.
    pub system_share_returned: bool,
}

/// Why a settlement could not be performed.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum LedgerError {
    /// No ledger is configured.
    #[error(
        "no ledger is configured; settlement is optional and every other \
         layer functions without one (Section 12.1)"
    )]
    NotConfigured,

    /// The ledger does not provide a property the specification requires.
    #[error("the configured ledger does not claim {missing}")]
    MissingProperty {
        /// Which property is absent.
        missing: &'static str,
    },
}

/// A settlement backend.
///
/// Deliberately narrow. There is no method by which a ledger could report a
/// standing, influence one, or gate a read: the type cannot express those
/// operations, so an implementation cannot perform them by accident.
pub trait Ledger {
    /// What this ledger claims to provide.
    fn properties(&self) -> LedgerProperties;

    /// Settle a bounty whose predicate has resolved.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the ledger is unconfigured or incomplete.
    fn settle(&self, bounty: &Cid, qualifying: &[Cid]) -> Result<Settlement, LedgerError>;
}
