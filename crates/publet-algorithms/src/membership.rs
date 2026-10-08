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
    ///
    /// Positions and sizes are `u64` on every target: a proof comes from
    /// someone else, whose set may be larger than this machine's `usize`.
    Present {
        /// Position in sorted order.
        index: u64,
        /// Total members.
        size: u64,
        /// Audit path from the leaf upward.
        path: Vec<Hash>,
    },
    /// The member is absent, bracketed by these neighbours.
    ///
    /// `before` and `after` are the adjacent present members. Either is
    /// `None` when the queried value sorts outside the range entirely.
    Absent {
        /// The greatest present member less than the queried one.
        before: Option<(u64, String, Vec<Hash>)>,
        /// The least present member greater than the queried one.
        after: Option<(u64, String, Vec<Hash>)>,
        /// Total members.
        size: u64,
    },
}

impl Membership {
    /// Build from an unsorted collection, sorting and deduplicating.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::Membership;
    ///
    /// // Order and duplicates in the input do not matter.
    /// let one = Membership::new(["plum", "apple", "apple"].map(String::from));
    /// let other = Membership::new(["apple", "plum"].map(String::from));
    /// assert_eq!(one.root(), other.root());
    /// ```
    #[must_use]
    pub fn new(members: impl IntoIterator<Item = String>) -> Self {
        let mut members: Vec<String> = members.into_iter().collect();
        members.sort();
        members.dedup();
        Self { members }
    }

    /// The members, in sorted order.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::Membership;
    ///
    /// # let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
    /// assert_eq!(members.members(), ["apple", "cherry", "plum"]);
    /// ```
    #[must_use]
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// How many members there are.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::Membership;
    ///
    /// # let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
    /// assert_eq!(members.len(), 3);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether there are no members.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::Membership;
    ///
    /// assert!(Membership::new(Vec::<String>::new()).is_empty());
    /// ```
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
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::Membership;
    ///
    /// # let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
    /// let more = Membership::new(["apple", "cherry", "plum", "quince"].map(String::from));
    /// assert_ne!(members.root(), more.root());
    /// ```
    #[must_use]
    pub fn root(&self) -> Hash {
        tree_root(&self.leaves())
    }

    /// Prove that `member` is present or absent.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::membership::{Membership, Proof, verify};
    ///
    /// # let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
    /// let root = members.root();
    ///
    /// let present = members.prove("cherry");
    /// assert!(matches!(present, Proof::Present { .. }));
    /// assert!(verify("cherry", &present, &root));
    ///
    /// // "banana" is absent: "apple" and "cherry" are adjacent, so nothing lies between.
    /// let absent = members.prove("banana");
    /// assert!(matches!(absent, Proof::Absent { .. }));
    /// assert!(verify("banana", &absent, &root));
    /// ```
    #[must_use]
    pub fn prove(&self, member: &str) -> Proof {
        let leaves = self.leaves();
        match self.members.binary_search(&member.to_owned()) {
            Ok(index) => Proof::Present {
                index: index as u64,
                size: self.members.len() as u64,
                path: crate::log::inclusion_proof(&leaves, index),
            },
            Err(insertion) => {
                let before = insertion.checked_sub(1).and_then(|i| {
                    self.members
                        .get(i)
                        .map(|m| (i as u64, m.clone(), crate::log::inclusion_proof(&leaves, i)))
                });
                let after = self.members.get(insertion).map(|m| {
                    (
                        insertion as u64,
                        m.clone(),
                        crate::log::inclusion_proof(&leaves, insertion),
                    )
                });
                Proof::Absent {
                    before,
                    after,
                    size: self.members.len() as u64,
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
///
/// # Example
///
/// ```
/// use publet_algorithms::membership::{Membership, verify};
///
/// # let members = Membership::new(["apple", "cherry", "plum"].map(String::from));
/// let root = members.root();
/// let proof = members.prove("banana");
/// assert!(verify("banana", &proof, &root));
/// // A proof of absence for "banana" proves nothing about "cherry".
/// assert!(!verify("cherry", &proof, &root));
/// ```
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
            let check = |entry: &Option<(u64, String, Vec<Hash>)>| -> bool {
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
///
/// # Example
///
/// ```
/// use publet_algorithms::membership::{Membership, diff};
///
/// let before = Membership::new(["apple", "cherry"].map(String::from));
/// let after = Membership::new(["cherry", "plum"].map(String::from));
/// let (added, removed) = diff(&before, &after);
/// assert_eq!(added, ["plum"]);
/// assert_eq!(removed, ["apple"]);
/// ```
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
