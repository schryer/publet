//! Log proofs, checked against RFC 6962 constants and exhaustively by
//! round-trip over every tree size and index in range.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use publet_algorithms::log::{
    Hash, consistency_proof, empty_root, inclusion_proof, leaf_hash, root, verify_consistency,
    verify_inclusion,
};

fn hex(h: &Hash) -> String {
    publet_algorithms::log::to_hex(h)
}

fn leaves(n: usize) -> Vec<Hash> {
    (0..n)
        .map(|i| leaf_hash(format!("leaf{i}").as_bytes()))
        .collect()
}

// covers: log::empty_root, log::leaf_hash, log::to_hex
#[test]
fn matches_rfc6962_constants() {
    // MTH({}) = HASH(), the SHA-256 of the empty string.
    assert_eq!(
        hex(&empty_root()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    // MTH({d0}) = HASH(0x00 || d0); for an empty leaf this is the value
    // RFC 6962 gives for a one-entry tree over an empty entry.
    assert_eq!(
        hex(&leaf_hash(b"")),
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
    );
}

// covers: log::leaf_hash, log::node_hash
#[test]
fn leaf_and_interior_hashes_are_domain_separated() {
    // Without separation a leaf could be presented as an interior node and
    // one tree could claim two shapes.
    let a = leaf_hash(b"x");
    let b = leaf_hash(b"y");
    let interior = publet_algorithms::log::node_hash(&a, &b);
    let mut concatenated = Vec::new();
    concatenated.extend_from_slice(&a);
    concatenated.extend_from_slice(&b);
    assert_ne!(interior, leaf_hash(&concatenated));
}

// covers: log::inclusion_proof, log::leaf_hash, log::root, log::verify_inclusion
#[test]
fn inclusion_proofs_round_trip_for_every_index_and_size() {
    for n in 1..=64usize {
        let tree = leaves(n);
        let r = root(&tree);
        for i in 0..n {
            let path = inclusion_proof(&tree, i);
            assert!(
                verify_inclusion(&tree[i], i, n, &path, &r),
                "inclusion failed for index {i} of {n}"
            );
        }
    }
}

// covers: log::inclusion_proof, log::leaf_hash, log::root, log::verify_inclusion
#[test]
fn an_inclusion_proof_does_not_verify_a_different_leaf() {
    let tree = leaves(16);
    let r = root(&tree);
    let path = inclusion_proof(&tree, 5);
    assert!(verify_inclusion(&tree[5], 5, 16, &path, &r));
    assert!(!verify_inclusion(&tree[6], 5, 16, &path, &r));
    assert!(!verify_inclusion(&tree[5], 6, 16, &path, &r));
}

// covers: log::inclusion_proof, log::leaf_hash, log::root, log::verify_inclusion
#[test]
fn a_tampered_inclusion_path_is_rejected() {
    let tree = leaves(16);
    let r = root(&tree);
    let mut path = inclusion_proof(&tree, 3);
    path[0][0] ^= 0xff;
    assert!(!verify_inclusion(&tree[3], 3, 16, &path, &r));
}

// covers: log::consistency_proof, log::leaf_hash, log::root, log::verify_consistency
#[test]
fn consistency_proofs_round_trip_for_every_pair() {
    for n in 1..=48usize {
        let new_tree = leaves(n);
        let new_root = root(&new_tree);
        for m in 1..=n {
            let old_root = root(&new_tree[..m]);
            let proof = consistency_proof(&new_tree, m);
            assert!(
                verify_consistency(m, n, &old_root, &new_root, &proof),
                "consistency failed for m={m} n={n}"
            );
        }
    }
}

// covers: log::consistency_proof, log::leaf_hash, log::root, log::verify_consistency
#[test]
fn a_rewritten_history_fails_consistency() {
    // The attack the log exists to detect: a publisher who changes what an
    // earlier generation contained.
    let honest = leaves(8);
    let mut rewritten = honest.clone();
    rewritten[2] = leaf_hash(b"substituted");

    let old_root = root(&honest[..4]);
    let new_root = root(&rewritten);
    let proof = consistency_proof(&rewritten, 4);
    assert!(
        !verify_consistency(4, 8, &old_root, &new_root, &proof),
        "a rewritten prefix must not verify"
    );
}

// covers: log::consistency_proof, log::leaf_hash, log::root, log::verify_consistency
#[test]
fn a_truncated_history_fails_consistency() {
    let tree = leaves(8);
    let old_root = root(&tree[..6]);
    let shorter = &tree[..4];
    let proof = consistency_proof(shorter, 4);
    assert!(!verify_consistency(6, 4, &old_root, &root(shorter), &proof));
}

// covers: log::leaf_hash, log::root, log::verify_consistency
#[test]
fn identical_sizes_require_an_empty_proof_and_equal_roots() {
    let tree = leaves(5);
    let r = root(&tree);
    assert!(verify_consistency(5, 5, &r, &r, &[]));
    assert!(!verify_consistency(5, 5, &r, &r, &[leaf_hash(b"junk")]));
    assert!(!verify_consistency(5, 5, &r, &leaf_hash(b"other"), &[]));
}

// covers: log::consistency_proof, log::leaf_hash, log::root, log::verify_consistency
#[test]
fn a_tampered_consistency_proof_is_rejected() {
    let tree = leaves(16);
    let old_root = root(&tree[..5]);
    let new_root = root(&tree);
    let mut proof = consistency_proof(&tree, 5);
    proof[0][0] ^= 0xff;
    assert!(!verify_consistency(5, 16, &old_root, &new_root, &proof));
}

// covers: log::verify_inclusion, log::verify_consistency
#[test]
fn absurd_sizes_are_refused_without_hanging() {
    // Found by fuzzing: a proof claiming more than 2^63 entries made the
    // verifier loop forever. Proofs come from peers, so that was a way to
    // stop anyone checking them. The top bit is 2^63 on 64-bit targets and
    // 2^31 on 32-bit ones, where the same loop would have hung.
    let leaf = publet_algorithms::log::leaf_hash(b"x");
    let root = publet_algorithms::log::empty_root();
    let top = 1_usize << (usize::BITS - 1);
    for size in [usize::MAX, usize::MAX - 1, top + 1, top] {
        assert!(!verify_inclusion(&leaf, size - 1, size, &[], &root));
        assert!(!verify_consistency(1, size, &root, &root, &[]));
    }
    // Also found by fuzzing: a consistency proof between sizes like these
    // shifted a u64 by 64 bits, which panics where overflow is checked --
    // as it is in pub's release builds.
    let proof = [leaf; 3];
    #[cfg(target_pointer_width = "64")]
    assert!(!verify_consistency(
        3_352_797_463_764_764_551,
        9_765_923_333_140_350_855,
        &root,
        &root,
        &proof
    ));
    assert!(!verify_consistency(3, usize::MAX, &root, &root, &proof));
    assert!(!verify_consistency(top - 3, usize::MAX, &root, &root, &proof));
}
