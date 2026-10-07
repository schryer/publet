//! Append-only Merkle log with inclusion and consistency proofs.
//!
//! Follows the algorithms of [RFC 6962] so that the constructions are
//! specified elsewhere and independently testable, rather than being an
//! invention of this project. Domain-separating leaf hashes (`0x00`) from
//! interior hashes (`0x01`) is what prevents a leaf from being presented as
//! an interior node, which would otherwise let one tree claim two shapes.
//!
//! [RFC 6962]: https://www.rfc-editor.org/rfc/rfc6962#section-2
//!
//! The log's leaves are successive membership roots, so a consistency proof
//! establishes that generation *m* is a prefix of generation *n*: that
//! nothing recorded earlier was rewritten.

use sha2::{Digest as _, Sha256};

/// A 32-byte digest.
pub type Hash = [u8; 32];

/// Hash of an empty tree, per RFC 6962: `HASH()`.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{empty_root, root, to_hex};
///
/// assert_eq!(root(&[]), empty_root());
/// // SHA-256 of nothing, as RFC 6962 defines it.
/// assert_eq!(
///     to_hex(&empty_root()),
///     "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
/// );
/// ```
#[must_use]
pub fn empty_root() -> Hash {
    Sha256::digest([]).into()
}

/// Leaf hash: `HASH(0x00 || leaf)`.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{leaf_hash, to_hex};
///
/// assert_eq!(
///     to_hex(&leaf_hash(b"a")),
///     "022a6979e6dab7aa5ae4c3e5e45f7e977112a7e63593820dbec1ec738a24f93c",
/// );
/// ```
#[must_use]
pub fn leaf_hash(leaf: &[u8]) -> Hash {
    let mut h = Sha256::new();
    h.update([0x00]);
    h.update(leaf);
    h.finalize().into()
}

/// Interior hash: `HASH(0x01 || left || right)`.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{leaf_hash, node_hash, root};
///
/// let (a, b) = (leaf_hash(b"a"), leaf_hash(b"b"));
/// assert_eq!(node_hash(&a, &b), root(&[a, b]));
/// // Order matters: a log is a sequence.
/// assert_ne!(node_hash(&a, &b), node_hash(&b, &a));
/// ```
#[must_use]
pub fn node_hash(left: &Hash, right: &Hash) -> Hash {
    let mut h = Sha256::new();
    h.update([0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// Largest power of two strictly less than `n`.
///
/// RFC 6962's split point. Defined for `n > 1`.
fn split(n: usize) -> usize {
    debug_assert!(n > 1);
    // The largest power of two strictly below n is the top bit of n - 1.
    // Doubling up to it instead never ends for n above 2^63: the shift
    // wraps to zero, and a peer's proof claiming such a size would hang
    // the verifier.
    1 << (usize::BITS - 1 - (n - 1).leading_zeros())
}

/// Merkle Tree Hash over `leaves`, already leaf-hashed.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{leaf_hash, root, to_hex};
///
/// let leaves = [leaf_hash(b"a"), leaf_hash(b"b")];
/// assert_eq!(
///     to_hex(&root(&leaves)),
///     "b137985ff484fb600db93107c77b0365c80d78f5b429ded0fd97361d077999eb",
/// );
/// ```
#[must_use]
pub fn root(leaves: &[Hash]) -> Hash {
    match leaves.len() {
        0 => empty_root(),
        1 => leaves.first().copied().unwrap_or_else(empty_root),
        n => {
            let k = split(n);
            let (left, right) = leaves.split_at(k);
            node_hash(&root(left), &root(right))
        }
    }
}

/// Audit path proving `index` is in a tree of `leaves.len()` leaves.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{Hash, inclusion_proof, leaf_hash, root, verify_inclusion};
///
/// # let leaves: Vec<Hash> = ["a", "b", "c"].iter().map(|e| leaf_hash(e.as_bytes())).collect();
/// let path = inclusion_proof(&leaves, 2);
/// assert!(verify_inclusion(&leaves[2], 2, leaves.len(), &path, &root(&leaves)));
/// ```
#[must_use]
pub fn inclusion_proof(leaves: &[Hash], index: usize) -> Vec<Hash> {
    let n = leaves.len();
    if index >= n {
        return Vec::new();
    }
    if n == 1 {
        return Vec::new();
    }
    let k = split(n);
    let (left, right) = leaves.split_at(k);
    if index < k {
        let mut path = inclusion_proof(left, index);
        path.push(root(right));
        path
    } else {
        let mut path = inclusion_proof(right, index - k);
        path.push(root(left));
        path
    }
}

/// Render a hash as lower-case hexadecimal.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::to_hex;
///
/// assert_eq!(to_hex(&[0xab; 32]), "ab".repeat(32));
/// ```
#[must_use]
pub fn to_hex(hash: &Hash) -> String {
    use std::fmt::Write as _;
    hash.iter()
        .fold(String::with_capacity(64), |mut acc, byte| {
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

/// Whether the node carrying the leaf is the left or right child.
enum Side {
    Left,
    Right,
}

/// Recompute a root from a leaf and its audit path.
///
/// [`inclusion_proof`] emits the path from the leaf upward, as RFC 6962
/// specifies. Which side each sibling belongs on is only known by
/// descending from the root, so the descent is done first and the sides are
/// then applied in reverse against the path.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{Hash, inclusion_proof, leaf_hash, root, verify_inclusion};
///
/// # let leaves: Vec<Hash> = ["a", "b", "c"].iter().map(|e| leaf_hash(e.as_bytes())).collect();
/// let head = root(&leaves);
/// let path = inclusion_proof(&leaves, 1);
/// assert!(verify_inclusion(&leaves[1], 1, 3, &path, &head));
/// // The same path does not prove a different entry, or a different position.
/// assert!(!verify_inclusion(&leaf_hash(b"forged"), 1, 3, &path, &head));
/// assert!(!verify_inclusion(&leaves[1], 0, 3, &path, &head));
/// ```
#[must_use]
pub fn verify_inclusion(
    leaf: &Hash,
    index: usize,
    size: usize,
    path: &[Hash],
    expected_root: &Hash,
) -> bool {
    if index >= size || size == 0 {
        return false;
    }

    let mut sides = Vec::new();
    let (mut idx, mut sz) = (index, size);
    while sz > 1 {
        let k = split(sz);
        if idx < k {
            sides.push(Side::Left);
            sz = k;
        } else {
            sides.push(Side::Right);
            idx -= k;
            sz -= k;
        }
    }

    if sides.len() != path.len() {
        return false;
    }

    let mut computed = *leaf;
    for (sibling, side) in path.iter().zip(sides.iter().rev()) {
        computed = match side {
            Side::Left => node_hash(&computed, sibling),
            Side::Right => node_hash(sibling, &computed),
        };
    }
    computed == *expected_root
}

/// Proof that a tree of `m` leaves is a prefix of one of `leaves.len()`.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{Hash, consistency_proof, leaf_hash, root, verify_consistency};
///
/// # let leaves: Vec<Hash> = ["a", "b", "c"].iter().map(|e| leaf_hash(e.as_bytes())).collect();
/// // The first two entries form a log that the three-entry log extends.
/// let proof = consistency_proof(&leaves, 2);
/// assert!(verify_consistency(2, 3, &root(&leaves[..2]), &root(&leaves), &proof));
/// ```
#[must_use]
pub fn consistency_proof(leaves: &[Hash], m: usize) -> Vec<Hash> {
    let n = leaves.len();
    if m == 0 || m > n {
        return Vec::new();
    }
    if m == n {
        return Vec::new();
    }
    subproof(leaves, m, true)
}

fn subproof(leaves: &[Hash], m: usize, start: bool) -> Vec<Hash> {
    let n = leaves.len();
    if m == n {
        if start {
            return Vec::new();
        }
        return vec![root(leaves)];
    }
    let k = split(n);
    let (left, right) = leaves.split_at(k);
    if m <= k {
        let mut path = subproof(left, m, start);
        path.push(root(right));
        path
    } else {
        let mut path = subproof(right, m - k, false);
        path.push(root(left));
        path
    }
}

/// Decompose an inclusion path into its inner and border parts.
///
/// `inner` counts the levels where the path turns, `border` the levels
/// above where it only ever folds right. Naming them is what lets a
/// consistency proof be checked without rebuilding either tree.
fn decompose(index: u64, size: u64) -> (u32, u32) {
    let inner = u64::BITS - (index ^ (size - 1)).leading_zeros();
    // `inner` reaches 64 when the size is above 2^63, and shifting a u64 by
    // 64 overflows: a panic where overflow is checked, a wrong answer where
    // it is not. Every bit is then inside, so none is on the border.
    let border = index.checked_shr(inner).unwrap_or(0).count_ones();
    (inner, border)
}

fn chain_inner(seed: Hash, path: &[Hash], index: u64) -> Hash {
    let mut acc = seed;
    for (i, sibling) in path.iter().enumerate() {
        acc = if (index >> i) & 1 == 0 {
            node_hash(&acc, sibling)
        } else {
            node_hash(sibling, &acc)
        };
    }
    acc
}

/// As [`chain_inner`], but folding only where the path turns right.
fn chain_inner_right(seed: Hash, path: &[Hash], index: u64) -> Hash {
    let mut acc = seed;
    for (i, sibling) in path.iter().enumerate() {
        if (index >> i) & 1 == 1 {
            acc = node_hash(sibling, &acc);
        }
    }
    acc
}

fn chain_border_right(seed: Hash, path: &[Hash]) -> Hash {
    path.iter()
        .fold(seed, |acc, sibling| node_hash(sibling, &acc))
}

/// Verify that `old_root` at size `m` is a prefix of `new_root` at size `n`.
///
/// This is the reconstruction of RFC 6962 section 2.1.2: the proof is split
/// into an inner and a border part, each folded in turn, yielding both roots
/// from one path. Getting it wrong is not visible in ordinary use -- proofs
/// would simply always fail, or worse, always pass -- so the round-trip
/// tests exercise every `(m, n)` pair rather than a sample.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::{Hash, consistency_proof, leaf_hash, root, verify_consistency};
///
/// # let leaves: Vec<Hash> = ["a", "b", "c"].iter().map(|e| leaf_hash(e.as_bytes())).collect();
/// let old = root(&leaves[..2]);
/// let proof = consistency_proof(&leaves, 2);
/// assert!(verify_consistency(2, 3, &old, &root(&leaves), &proof));
///
/// // A log that rewrote its history is not consistent with what was seen before.
/// let rewritten: Vec<Hash> = ["a", "B", "c"].iter().map(|e| leaf_hash(e.as_bytes())).collect();
/// assert!(!verify_consistency(2, 3, &old, &root(&rewritten), &consistency_proof(&rewritten, 2)));
/// ```
#[must_use]
pub fn verify_consistency(
    m: usize,
    n: usize,
    old_root: &Hash,
    new_root: &Hash,
    proof: &[Hash],
) -> bool {
    if m > n {
        return false;
    }
    if m == n {
        return proof.is_empty() && old_root == new_root;
    }
    if m == 0 {
        // Every tree contains the empty prefix.
        return proof.is_empty();
    }
    if proof.is_empty() {
        return false;
    }

    let (m64, n64) = (m as u64, n as u64);
    let (mut inner, border) = decompose(m64 - 1, n64);
    let shift = m64.trailing_zeros();
    inner -= shift;

    // When `m` is an exact power of two the old root is the seed and is not
    // carried in the proof; otherwise the proof supplies it.
    let (seed, start) = if m64 == 1u64 << shift {
        (*old_root, 0usize)
    } else {
        match proof.first() {
            Some(h) => (*h, 1usize),
            None => return false,
        }
    };

    let expected = start + inner as usize + border as usize;
    if proof.len() != expected {
        return false;
    }
    let Some(rest) = proof.get(start..) else {
        return false;
    };
    let Some(inner_part) = rest.get(..inner as usize) else {
        return false;
    };
    let Some(border_part) = rest.get(inner as usize..) else {
        return false;
    };

    let mask = (m64 - 1) >> shift;
    let recomputed_old = chain_border_right(chain_inner_right(seed, inner_part, mask), border_part);
    let recomputed_new = chain_border_right(chain_inner(seed, inner_part, mask), border_part);

    recomputed_old == *old_root && recomputed_new == *new_root
}

/// Checkpoint origins for a client catching up to `head`.
///
/// Exponentially spaced, so a client far behind is covered in O(log n)
/// fetches rather than one per entry.
///
/// # Example
///
/// ```
/// use publet_algorithms::log::checkpoints;
///
/// // Behind a head at 100: entries 1, 2, 4, 8, ... back from it.
/// assert_eq!(checkpoints(100), [36, 68, 84, 92, 96, 98, 99]);
/// ```
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
///
/// # Example
///
/// ```
/// use publet_algorithms::log::fetches_required;
///
/// // 0 to 300 is a distance with four set bits, so four fetches.
/// assert_eq!(fetches_required(0, 300), 4);
/// assert_eq!(fetches_required(300, 300), 0);
/// ```
#[must_use]
pub fn fetches_required(current: u64, head: u64) -> u32 {
    if current >= head {
        return 0;
    }
    (head - current).count_ones()
}
