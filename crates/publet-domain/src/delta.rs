//! Delta synchronization (Section 14.3.1).
//!
//! A reader at generation *m* advancing to *n* fetches the generation
//! records between them and the objects their `added` lists name. Full
//! retrieval is needed only on first acquisition.
//!
//! Deltas are self-verifying: after applying them the client recomputes the
//! membership root and compares it to the target generation's snapshot. A
//! delta that adds, omits, or substitutes anything fails that comparison, so
//! a client need not trust the peer that served it.

use std::collections::BTreeSet;

use publet_merkle::membership::Membership;
use thiserror::Error;

use crate::Generation;

/// Why a delta could not be applied.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum DeltaError {
    /// The sequence of generation records was not contiguous.
    #[error("delta is not contiguous: expected index {expected}, found {found}")]
    NotContiguous {
        /// The index required next.
        expected: u64,
        /// The index supplied.
        found: u64,
    },
    /// The result did not match the target generation's root.
    #[error(
        "applied delta yields membership root {computed}, not the {expected} \
         the generation declares; the delta added, omitted, or substituted \
         something"
    )]
    RootMismatch {
        /// What the client computed.
        computed: String,
        /// What the generation declared.
        expected: String,
    },
    /// The delta was empty when advancement was requested.
    #[error("no generation records supplied")]
    Empty,
}

/// Apply a contiguous run of generation records to a membership.
///
/// # Errors
///
/// Returns [`DeltaError`] if the records are not contiguous or if the
/// resulting root does not match what the final record declares.
pub fn apply(
    start: &Membership,
    start_index: u64,
    records: &[Generation],
) -> Result<Membership, DeltaError> {
    let Some(last) = records.last() else {
        return Err(DeltaError::Empty);
    };

    let mut members: BTreeSet<String> = start.members().iter().cloned().collect();
    for (expected, record) in (start_index + 1..).zip(records.iter()) {
        if record.index != expected {
            return Err(DeltaError::NotContiguous {
                expected,
                found: record.index,
            });
        }
        for cid in &record.added {
            members.insert(cid.to_string());
        }
        for removal in &record.removed {
            members.remove(&removal.cid.to_string());
        }
    }

    let result = Membership::new(members);
    // The self-verification. Cheap, and it makes trusting the peer that
    // served the delta unnecessary rather than merely unwise.
    let computed = publet_core::Cid::from_digest(publet_core::HashAlg::Sha2_256, &result.root());
    if computed.as_ref() != Some(&last.snapshot) {
        return Err(DeltaError::RootMismatch {
            computed: computed.map_or_else(|| "<malformed>".to_owned(), |c| c.to_string()),
            expected: last.snapshot.to_string(),
        });
    }
    Ok(result)
}

/// Render a root for comparison and display.
#[must_use]
pub fn hex(root: &publet_merkle::log::Hash) -> String {
    publet_merkle::log::to_hex(root)
}

/// Checkpoint origins for a client at `current` advancing to `head`.
///
/// Exponentially spaced, so a client far behind is covered in O(log n)
/// fetches rather than one per generation.
#[must_use]
pub fn checkpoints(head: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut step = 1u64;
    while step <= head {
        out.push(head - step);
        step = step.saturating_mul(2);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// How many fetches a client at `current` needs to reach `head`.
///
/// With checkpoints at every power of two behind the head, a client greedily
/// takes the largest jump that does not overshoot. The distance is therefore
/// covered in one fetch per set bit -- `count_ones`, which is bounded by
/// `log2(distance) + 1` and never exceeds 64.
#[must_use]
pub fn fetches_required(current: u64, head: u64) -> u32 {
    if current >= head {
        return 0;
    }
    (head - current).count_ones()
}
