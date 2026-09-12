//! Sorted membership tree with inclusion and absence proofs.
//!
//! A generation's members are hashed in sorted order, which is what makes
//! absence provable: if a claimed member is missing, the two adjacent
//! members that would bracket it are shown instead, and their adjacency in
//! a sorted tree is itself proof that nothing lies between them.
//!
//! This is why two structures are needed rather than one. Sorting is what
//! yields absence proofs, and it is also what makes consistency proofs
//! unavailable, since inserting a member reorders interior nodes. The
//! append-only [`crate::log`] covers the other half.

use crate::log::{Hash, leaf_hash, root as tree_root};

/// A generation's membership, held in sorted order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Membership {
    members: Vec<String>,
}

/// Evidence that a member is present or absent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Proof {
    /// The member is present at `index`, with this audit path.
    Present {
        /// Position in sorted order.
        index: usize,
        /// Total members.
        size: usize,
        /// Audit path from the leaf upward.
        path: Vec<Hash>,
    },
    /// The member is absent, bracketed by these neighbours.
    ///
    /// `before` and `after` are the adjacent present members. Either is
    /// `None` when the queried value sorts outside the range entirely.
    Absent {
        /// The greatest present member less than the queried one.
        before: Option<(usize, String, Vec<Hash>)>,
        /// The least present member greater than the queried one.
        after: Option<(usize, String, Vec<Hash>)>,
        /// Total members.
        size: usize,
    },
}

impl Membership {
    /// Build from an unsorted collection, sorting and deduplicating.
    #[must_use]
    pub fn new(members: impl IntoIterator<Item = String>) -> Self {
        let mut members: Vec<String> = members.into_iter().collect();
        members.sort();
        members.dedup();
        Self { members }
    }

    /// The members, in sorted order.
    #[must_use]
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// How many members there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether there are no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    fn leaves(&self) -> Vec<Hash> {
        self.members
            .iter()
            .map(|m| leaf_hash(m.as_bytes()))
            .collect()
    }

    /// The membership root for this generation.
    #[must_use]
    pub fn root(&self) -> Hash {
        tree_root(&self.leaves())
    }

    /// Prove that `member` is present or absent.
    #[must_use]
    pub fn prove(&self, member: &str) -> Proof {
        let leaves = self.leaves();
        match self.members.binary_search(&member.to_owned()) {
            Ok(index) => Proof::Present {
                index,
                size: self.members.len(),
                path: crate::log::inclusion_proof(&leaves, index),
            },
            Err(insertion) => {
                let before = insertion.checked_sub(1).and_then(|i| {
                    self.members
                        .get(i)
                        .map(|m| (i, m.clone(), crate::log::inclusion_proof(&leaves, i)))
                });
                let after = self.members.get(insertion).map(|m| {
                    (
                        insertion,
                        m.clone(),
                        crate::log::inclusion_proof(&leaves, insertion),
                    )
                });
                Proof::Absent {
                    before,
                    after,
                    size: self.members.len(),
                }
            }
        }
    }
}

/// Check a proof against a root.
///
/// An absence proof holds when both bracketing members verify, they are
/// adjacent by index, and the queried value sorts strictly between them.
/// Omitting the adjacency check would let a prover skip over the very
/// member whose absence is being claimed.
#[must_use]
pub fn verify(member: &str, proof: &Proof, expected_root: &Hash) -> bool {
    match proof {
        Proof::Present { index, size, path } => crate::log::verify_inclusion(
            &leaf_hash(member.as_bytes()),
            *index,
            *size,
            path,
            expected_root,
        ),
        Proof::Absent {
            before,
            after,
            size,
        } => {
            if *size == 0 {
                return before.is_none()
                    && after.is_none()
                    && *expected_root == crate::log::empty_root();
            }
            let check = |entry: &Option<(usize, String, Vec<Hash>)>| -> bool {
                entry.as_ref().is_none_or(|(index, value, path)| {
                    crate::log::verify_inclusion(
                        &leaf_hash(value.as_bytes()),
                        *index,
                        *size,
                        path,
                        expected_root,
                    )
                })
            };
            if !check(before) || !check(after) {
                return false;
            }
            match (before, after) {
                (Some((i, lower, _)), Some((j, upper, _))) => {
                    *j == i + 1 && lower.as_str() < member && member < upper.as_str()
                }
                // The queried value sorts before every member.
                (None, Some((j, upper, _))) => *j == 0 && member < upper.as_str(),
                // It sorts after every member.
                (Some((i, lower, _)), None) => *i + 1 == *size && lower.as_str() < member,
                (None, None) => false,
            }
        }
    }
}

/// Members present in `next` but not `previous`, and the reverse.
///
/// Returned in sorted order so a generation record built from this is
/// deterministic.
#[must_use]
pub fn diff(previous: &Membership, next: &Membership) -> (Vec<String>, Vec<String>) {
    let added = next
        .members
        .iter()
        .filter(|m| previous.members.binary_search(m).is_err())
        .cloned()
        .collect();
    let removed = previous
        .members
        .iter()
        .filter(|m| next.members.binary_search(m).is_err())
        .cloned()
        .collect();
    (added, removed)
}
