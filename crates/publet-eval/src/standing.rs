//! Standing: evidence dominance and the acceptance predicate
//! (Sections 11.3 to 11.6).
//!
//! The order of the rules matters. Proof settles a formal claim and
//! independent reproduction settles an empirical one; endorsement weight
//! decides only what neither has settled. Structuring the computation so
//! that weight is consulted *last* is what makes R9 hold by construction
//! rather than by remembering to check.

use std::collections::{BTreeMap, BTreeSet};

use publet_graph::Class;

use crate::{Fixed6, Policy};

/// How a claim's supporting observation can be repeated (Section 5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Reproducibility {
    /// Any party with the stated resources can attempt it.
    #[default]
    Open,
    /// Attempting it requires access a named party controls.
    Restricted,
    /// Unrepeatable in principle.
    UniqueEvent,
    /// Repeatable only on a different sample.
    Destructive,
}

impl Reproducibility {
    /// Resolve the identifier used in a publet body.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "open" => Self::Open,
            "restricted" => Self::Restricted,
            "unique-event" => Self::UniqueEvent,
            "destructive" => Self::Destructive,
            _ => return None,
        })
    }

    /// The identifier used in a publet body.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Restricted => "restricted",
            Self::UniqueEvent => "unique-event",
            Self::Destructive => "destructive",
        }
    }
}

/// Counts of filed reproductions (Section 7.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reproductions {
    /// Reproductions agreeing with the claim.
    pub consistent: u32,
    /// Reproductions disagreeing with it.
    pub inconsistent: u32,
    /// Reproductions that reached no conclusion.
    pub inconclusive: u32,
    /// Consistent reproductions that are independent of one another.
    pub independent_consistent: u32,
    /// Inconsistent reproductions that are independent of one another.
    pub independent_inconsistent: u32,
}

/// The outcome of evaluating a claim under a viewpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// Weight and evidence both support the claim.
    Accepted,
    /// Weighted participants examined it and did not converge.
    Contested,
    /// Weight is against the claim.
    Rejected,
    /// No weighted participant evaluated it.
    Undetermined,
    /// Weight supports it, but the evidence does not yet.
    Unreplicated,
    /// The class is not truth-apt.
    NotTruthApt,
}

impl Outcome {
    /// The identifier used in an `eval` object.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Contested => "contested",
            Self::Rejected => "rejected",
            Self::Undetermined => "undetermined",
            Self::Unreplicated => "unreplicated",
            Self::NotTruthApt => "not-truth-apt",
        }
    }
}

/// Signed judgements and evidence gathered for one claim.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// Keys affirming, by identifier.
    pub affirm: BTreeSet<String>,
    /// Keys denying.
    pub deny: BTreeSet<String>,
    /// Keys abstaining.
    pub abstain: BTreeSet<String>,
    /// Keys whose disputes are argued and non-redundant.
    pub argued_disputes: BTreeSet<String>,
    /// Whether a recognized checker accepted a proof of the claim.
    pub proof_checked: bool,
    /// Filed reproductions.
    pub reproductions: Reproductions,
    /// How the claim's observation can be repeated.
    pub reproducibility: Reproducibility,
    /// Whether the claim has been retracted by its author.
    pub retracted: bool,
    /// Judgements filed about this claim (Section 5.2).
    ///
    /// An assessment is *not* a verdict. A verdict is input to an
    /// evaluation policy; an assessment is a signer saying what they think,
    /// and an author's assessment of their own work is one annotation among
    /// many rather than a privileged field (R3). Nothing here reaches the
    /// outcome -- which is what makes `sound-in-scope` sayable about a
    /// definitional publet, where no verdict may be cast at all.
    pub assessments: Vec<Assessment>,
    /// Distinct corpora cited as evidence of usage (Section 5.2).
    ///
    /// The evidence a `definitional` publet is settled by. Verdicts are not
    /// permitted on that class, so without this a definition has no
    /// evidence channel at all -- which is what it had until this existed.
    /// Held as the set of sources rather than a count, because two
    /// citations of one dictionary are one dictionary agreeing with itself.
    pub usage: BTreeSet<String>,
}

/// A signer's judgement about a claim (Section 5.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Assessment {
    /// The key that filed it.
    pub author: String,
    /// One of `sound`, `sound-in-scope`, `superseded`, `unsupported`,
    /// `refuted`, `undetermined`.
    pub verdict: String,
    /// What the judgement rests on. Required: a judgement without one is a
    /// preference, and the reader cannot weigh a preference.
    pub basis: String,
}

/// The result of evaluating one claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// Weight affirming.
    pub affirm_weight: Fixed6,
    /// Weight denying.
    pub deny_weight: Fixed6,
    /// Weight abstaining.
    pub abstain_weight: Fixed6,
    /// Weight that evaluated this claim at all.
    pub active_weight: Fixed6,
    /// The divergence factor applied.
    pub delta: Fixed6,
    /// Reproduction counts.
    pub reproductions: Reproductions,
    /// The claim's reproducibility class.
    pub reproducibility_class: Reproducibility,
    /// Whether the claim is retracted.
    pub retracted: bool,
    /// The claim class.
    pub class: Class,
    /// The outcome.
    pub result: Outcome,
}

/// Aggregate weight over a set of keys, counting each key once.
///
/// Section 11.6 requires that a signer's weight count at most once per
/// equivalence class, so endorsing five paraphrases of a claim is worth
/// exactly as much as endorsing one.
fn weight_of(keys: &BTreeSet<String>, weights: &BTreeMap<String, Fixed6>) -> Fixed6 {
    keys.iter().filter_map(|k| weights.get(k)).copied().sum()
}

/// Evaluate a claim under a viewpoint.
#[must_use]
pub fn evaluate(
    policy: &Policy,
    class: Class,
    evidence: &Evidence,
    weights: &BTreeMap<String, Fixed6>,
) -> Standing {
    let affirm_weight = weight_of(&evidence.affirm, weights);
    let deny_weight = weight_of(&evidence.deny, weights);
    let abstain_weight = weight_of(&evidence.abstain, weights);
    let active_weight = affirm_weight + deny_weight + abstain_weight;

    // delta = min(delta_max, delta_max * D_arg / (A + D_arg)).
    let argued = weight_of(&evidence.argued_disputes, weights);
    let delta = if (affirm_weight + argued).is_zero() {
        Fixed6::ZERO
    } else {
        policy
            .delta_max
            .min(policy.delta_max.mul_div(argued, affirm_weight + argued))
    };

    let result = outcome(policy, class, evidence, affirm_weight, deny_weight, delta);

    Standing {
        affirm_weight,
        deny_weight,
        abstain_weight,
        active_weight,
        delta,
        reproductions: evidence.reproductions,
        reproducibility_class: evidence.reproducibility,
        retracted: evidence.retracted,
        class,
        result,
    }
}

fn outcome(
    policy: &Policy,
    class: Class,
    evidence: &Evidence,
    affirm: Fixed6,
    deny: Fixed6,
    delta: Fixed6,
) -> Outcome {
    // A class that is not truth-apt has no verdict and never will.
    if !class.accepts_verdict() {
        return Outcome::NotTruthApt;
    }

    // A retracted claim carries no positive standing, whatever its weight.
    if evidence.retracted {
        return Outcome::Rejected;
    }

    // A proof is not a poll. Section 11.4: a recognized checker's
    // acceptance settles a formal claim regardless of endorsement, and no
    // quantity of denial changes it.
    if class == Class::Formal && evidence.proof_checked {
        return Outcome::Accepted;
    }

    // One independent inconsistent reproduction outweighs endorsement of
    // any magnitude. Checked before the weight arithmetic so that no amount
    // of agreement can reach past it.
    if class == Class::Empirical && evidence.reproductions.independent_inconsistent > 0 {
        return Outcome::Rejected;
    }

    let total = affirm + deny;
    if total.is_zero() {
        return Outcome::Undetermined;
    }

    let threshold = policy.tau + delta;
    let affirm_share = affirm.ratio(total);
    let deny_share = deny.ratio(total);

    if deny_share >= threshold {
        return Outcome::Rejected;
    }
    if affirm_share >= threshold {
        return if replication_ok(policy, class, evidence) {
            Outcome::Accepted
        } else {
            // The weight condition holds and the evidence does not. Not a
            // criticism: most true claims are unreplicated most of the
            // time, and saying so is more informative than a verdict that
            // conceals it.
            Outcome::Unreplicated
        };
    }
    Outcome::Contested
}

/// Whether the evidence permits `accepted` (Section 11.4).
fn replication_ok(policy: &Policy, class: Class, evidence: &Evidence) -> bool {
    if class != Class::Empirical {
        // Vacuously true for every other class.
        return true;
    }
    match evidence.reproducibility {
        // If nobody else is permitted to measure, the measurement cannot
        // establish a general claim however plausible it is found.
        Reproducibility::Restricted => false,
        // Replication is unavailable in principle, so the floor is met by
        // independent contemporaneous observation or verified provenance,
        // which arrive as consistent reproductions of the record.
        Reproducibility::UniqueEvent => evidence.reproductions.independent_consistent >= 1,
        Reproducibility::Open | Reproducibility::Destructive => {
            evidence.reproductions.independent_consistent >= policy.replication_floor
        }
    }
}
