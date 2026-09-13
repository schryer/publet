//! Settlement: bounties, ledgers, and the things they may never do
//! (Section 12).
//!
//! This crate is optional. Nothing in the object model, the graph, the
//! evaluation layer, the store, the transport, or the command line depends
//! on it, and the conformance suite passes with it absent -- which is how
//! Section 12.1's claim that settlement is optional is demonstrated rather
//! than asserted.
//!
//! It is also the only part of the protocol requiring global consensus and
//! the only part with a regulatory perimeter, so a deployment wanting
//! knowledge infrastructure and not a financial system can stop before it.
//!
//! # What settlement is not
//!
//! The ledger holds and releases funds. It has no epistemic authority: it
//! executes predicates over evaluations anyone can recompute, and every
//! predicate is chosen in advance by the party putting up the money. The
//! [`Prohibition`] type enumerates what no conformant implementation may
//! do, and exists so that those rules are reviewable in one place rather
//! than distributed through prose.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod bounty;
mod ledger;
mod null;

pub use bounty::{Bounty, BountyError, BountyKind, MINIMUM_REVIEW_SHARE};
pub use ledger::{Ledger, LedgerError, LedgerProperties, LedgerProperty, Settlement};
pub use null::NullLedger;

/// Something a conformant implementation may never do (Section 12.5).
///
/// Enumerated as a type so the prohibitions can be reviewed together. Each
/// is a rule about what settlement must not reach, and every one of them
/// is a way the layer could quietly become a market in credibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Prohibition {
    /// Conditioning retrieval, resolution, or indexing on payment.
    ///
    /// What review funding buys is replication priority, never access.
    GateAccess,
    /// Assigning standing on the basis of amounts staked, spent, or earned.
    StandingFromMoney,
    /// Treating bounty value as an input to any evaluation function.
    BountyValueInEvaluation,
    /// Representing a settled bounty to users as a determination of truth.
    SettlementAsTruth,
}

impl Prohibition {
    /// Every prohibition, for review and for tests that assert over them.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::GateAccess,
            Self::StandingFromMoney,
            Self::BountyValueInEvaluation,
            Self::SettlementAsTruth,
        ]
    }

    /// A stable identifier for the rule.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::GateAccess => "gate-access",
            Self::StandingFromMoney => "standing-from-money",
            Self::BountyValueInEvaluation => "bounty-value-in-evaluation",
            Self::SettlementAsTruth => "settlement-as-truth",
        }
    }

    /// Why the rule exists, in one line.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::GateAccess => {
                "review funding buys replication priority, never whether \
                 anyone may read"
            }
            Self::StandingFromMoney => {
                "capital deciding what is believed is the failure mode of the \
                 existing system in a less honest form"
            }
            Self::BountyValueInEvaluation => {
                "an evaluation that can see the money is no longer evaluating \
                 the claim"
            }
            Self::SettlementAsTruth => {
                "a settlement says something about one funder's chosen \
                 viewpoint at one instant, and nothing about the world"
            }
        }
    }
}
