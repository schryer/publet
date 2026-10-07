//! This crate's RFC 6962 log against an independent implementation.
//!
//! `ct-merkle` implements the same RFC. Given the same leaves, it must
//! produce the same root, the same audit paths and the same consistency
//! proofs, byte for byte, for every size and position tried. It is a
//! dev-dependency only: nothing here ships.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ct_merkle::mem_backed_tree::MemoryBackedTree;
use proptest::prelude::*;
use publet_algorithms::log::{Hash, consistency_proof, inclusion_proof, leaf_hash, root};
use sha2_011::Sha256;

fn theirs(leaves: &[Vec<u8>]) -> MemoryBackedTree<Sha256, Vec<u8>> {
    let mut tree = MemoryBackedTree::new();
    for leaf in leaves {
        tree.push(leaf.clone());
    }
    tree
}

fn ours(leaves: &[Vec<u8>]) -> Vec<Hash> {
    leaves.iter().map(|l| leaf_hash(l)).collect()
}

fn concat(hashes: &[Hash]) -> Vec<u8> {
    hashes.concat()
}

fn logs() -> impl Strategy<Value = Vec<Vec<u8>>> {
    proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..24), 1..70)
}

proptest! {
    // covers: log::root, log::leaf_hash
    #[test]
    fn roots_agree(leaves in logs()) {
        let expected = theirs(&leaves).root().as_bytes().to_vec();
        prop_assert_eq!(root(&ours(&leaves)).to_vec(), expected);
    }

    // covers: log::inclusion_proof
    #[test]
    fn audit_paths_agree(leaves in logs(), pick in any::<prop::sample::Index>()) {
        let index = pick.index(leaves.len());
        let expected = theirs(&leaves).prove_inclusion(index).as_bytes().to_vec();
        prop_assert_eq!(concat(&inclusion_proof(&ours(&leaves), index)), expected);
    }

    // covers: log::consistency_proof
    #[test]
    fn consistency_proofs_agree(leaves in logs(), pick in any::<prop::sample::Index>()) {
        prop_assume!(leaves.len() > 1);
        // An old size from 1 to n - 1: ct-merkle proves its own prefix.
        let old = 1 + pick.index(leaves.len() - 1);
        let expected = theirs(&leaves).prove_consistency(leaves.len() - old).as_bytes().to_vec();
        prop_assert_eq!(concat(&consistency_proof(&ours(&leaves), old)), expected);
    }
}
