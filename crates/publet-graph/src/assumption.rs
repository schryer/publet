//! Assumed accountability and machine-proposed objects
//! (Sections 10.7 and 7.4).
//!
//! Two mechanisms that look unrelated and share a discipline: both let
//! something outside the graph bear on it without acquiring authority
//! inside it.

use publet_core::{Cid, Object, cbor::Value};

/// What an assumer knows about the key they stand behind (Section 10.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Basis {
    /// A person holds the link in memory. They can be compelled, and no
    /// protocol rule protects them; the contribution here is limited to
    /// not writing the link down.
    IdentityKnownToMe,
    /// The assumer has reviewed the work rather than knowing the author.
    WorkReviewedByMe,
    /// An institution stands behind it.
    Institutional,
}

impl Basis {
    /// Resolve the identifier used in an annotation.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "identity-known-to-me" => Self::IdentityKnownToMe,
            "work-reviewed-by-me" => Self::WorkReviewedByMe,
            "institutional" => Self::Institutional,
            _ => return None,
        })
    }

    /// The identifier used in an annotation.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::IdentityKnownToMe => "identity-known-to-me",
            Self::WorkReviewedByMe => "work-reviewed-by-me",
            Self::Institutional => "institutional",
        }
    }
}

/// A key publicly standing behind another key's output.
///
/// Published rather than hidden, and that is the whole design. Recording
/// the link and encrypting it would manufacture a compellable, permanently
/// replicated artifact where none existed, while giving the protected
/// person nothing this does not. The secret that cannot be seized is the
/// one that was never written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assumption {
    /// The key doing the vouching.
    pub assumer: Cid,
    /// The key vouched for.
    pub assumed: Cid,
    /// What the assumer is going on.
    pub basis: Basis,
}

impl Assumption {
    /// Read an assumption from a verified annotation object.
    #[must_use]
    pub fn from_object(object: &Object) -> Option<Self> {
        let body = object.body();
        if body.get("kind").and_then(Value::as_text)? != "assumes-accountability" {
            return None;
        }
        Some(Self {
            assumer: object.author().clone(),
            assumed: body.get("target").and_then(Value::as_text)?.parse().ok()?,
            basis: body
                .get("value")
                .and_then(|v| v.get("basis"))
                .and_then(Value::as_text)
                .and_then(Basis::from_id)?,
        })
    }
}

/// What a machine-assisted judgement found (Section 7.4).
///
/// Advisory by construction. Nothing in the evaluation layer reads a triage
/// annotation: redundancy under R10 is set containment over grounds
/// identifiers, a mechanical test, never a model's opinion that two
/// arguments are substantively the same.
///
/// A model judging novelty by similarity to an existing corpus is disposed
/// to classify heterodox arguments as restatements of refuted ones, and
/// will do so while reporting high aggregate accuracy, because accuracy is
/// dominated by the ordinary cases. Keeping it out of the evaluation path
/// bounds that; nothing prevents it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triage {
    /// The object judged.
    pub target: Cid,
    /// What the model reported.
    pub finding: String,
    /// The model, version, prompt and inputs, so the run can be repeated.
    ///
    /// Required. Triage is *re-runnable*, not reproducible: model versions
    /// are withdrawn, sampling is stochastic, hardware differs. The
    /// disclosure makes it auditable, not verifiable, and the gap between
    /// those words is why it may not be presented as equivalent to a
    /// machine-checked proof.
    pub engine: String,
}

impl Triage {
    /// Read a triage annotation, requiring its disclosure block.
    ///
    /// Returns `None` when the engine is undeclared: an undisclosed
    /// machine judgement cannot be re-run, and one that cannot be re-run is
    /// an opinion wearing the shape of evidence.
    #[must_use]
    pub fn from_object(object: &Object) -> Option<Self> {
        let body = object.body();
        if body.get("kind").and_then(Value::as_text)? != "triage" {
            return None;
        }
        let value = body.get("value")?;
        let engine = value.get("engine")?;
        let model = engine.get("model").and_then(Value::as_text)?;
        let version = engine.get("version").and_then(Value::as_text)?;
        Some(Self {
            target: body.get("target").and_then(Value::as_text)?.parse().ok()?,
            finding: value.get("finding").and_then(Value::as_text)?.to_owned(),
            engine: format!("{model} {version}"),
        })
    }
}
