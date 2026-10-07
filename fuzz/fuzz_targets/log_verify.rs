//! Merkle log proof checking on arbitrary proofs.
//!
//! Proofs arrive from peers. Checking one must never panic, whatever the
//! sizes, the index or the path; a proof is simply true or false.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use publet_algorithms::log::{Hash, verify_consistency, verify_inclusion};

#[derive(Arbitrary, Debug)]
struct Input {
    leaf: Hash,
    index: usize,
    size: usize,
    old_size: usize,
    old_root: Hash,
    new_root: Hash,
    path: Vec<Hash>,
}

fuzz_target!(|input: Input| {
    let _ = verify_inclusion(&input.leaf, input.index, input.size, &input.path, &input.new_root);
    let _ = verify_consistency(input.old_size, input.size, &input.old_root, &input.new_root, &input.path);
});
