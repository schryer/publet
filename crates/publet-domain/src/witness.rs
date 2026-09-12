//! Witness attestations and split-view detection (Section 14.1.2).
//!
//! Consistency proofs bind a publisher to one history. They do not stop a
//! publisher showing *different* histories to different readers. Because a
//! log root is a single small value, witnessing is cheap to gossip and
//! cheap to compare, and a split view shows as two witnessed roots at one
//! generation that no consistency proof reconciles.

use std::collections::BTreeMap;

use publet_core::{Cid, Object, cbor::Value};

/// One observer's record of a domain at a generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    /// The observing key.
    pub observer: Cid,
    /// The domain observed.
    pub domain: Cid,
    /// The generation index.
    pub generation: u64,
    /// The log root at that generation.
    pub log_root: Vec<u8>,
    /// When it was observed. An unverified claim, as ever.
    pub observed: String,
}

/// Two witnesses reporting different roots for one generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitView {
    /// The domain in question.
    pub domain: Cid,
    /// The generation at which the histories differ.
    pub generation: u64,
    /// The conflicting witnesses.
    pub conflicting: Vec<Witness>,
}

impl Witness {
    /// Read a witness attestation from a verified annotation object.
    #[must_use]
    pub fn from_object(object: &Object) -> Option<Self> {
        let body = object.body();
        if body.get("kind").and_then(Value::as_text)? != "witnessed" {
            return None;
        }
        let value = body.get("value")?;
        Some(Self {
            observer: object.author().clone(),
            domain: body.get("target").and_then(Value::as_text)?.parse().ok()?,
            generation: value.get("generation").and_then(Value::as_uint)?,
            log_root: value.get("log_root").and_then(Value::as_bytes)?.to_vec(),
            observed: value
                .get("observed")
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_owned(),
        })
    }
}

/// Find generations where witnesses disagree about the log root.
///
/// Disagreement is the signal. A publisher serving one history to everyone
/// produces witnesses that all agree, however dishonest that single history
/// might be -- which is why witnessing complements consistency proofs
/// rather than replacing them.
#[must_use]
pub fn detect_split_views(witnesses: &[Witness]) -> Vec<SplitView> {
    let mut by_generation: BTreeMap<(String, u64), Vec<&Witness>> = BTreeMap::new();
    for w in witnesses {
        by_generation
            .entry((w.domain.to_string(), w.generation))
            .or_default()
            .push(w);
    }

    let mut out = Vec::new();
    for ((_, generation), group) in by_generation {
        let roots: std::collections::BTreeSet<&[u8]> =
            group.iter().map(|w| w.log_root.as_slice()).collect();
        if roots.len() > 1
            && let Some(first) = group.first()
        {
            out.push(SplitView {
                domain: first.domain.clone(),
                generation,
                conflicting: group.iter().map(|w| (*w).clone()).collect(),
            });
        }
    }
    out
}
