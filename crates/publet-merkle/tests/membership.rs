//! Inclusion and absence over the sorted membership tree.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use publet_merkle::membership::{Membership, Proof, diff, verify};

fn set(items: &[&str]) -> Membership {
    Membership::new(items.iter().map(|s| (*s).to_owned()))
}

#[test]
fn present_members_prove_inclusion() {
    let m = set(&["b", "d", "f", "h"]);
    let root = m.root();
    for member in ["b", "d", "f", "h"] {
        let proof = m.prove(member);
        assert!(matches!(proof, Proof::Present { .. }), "{member}");
        assert!(verify(member, &proof, &root), "{member}");
    }
}

#[test]
fn absent_members_prove_absence() {
    let m = set(&["b", "d", "f", "h"]);
    let root = m.root();
    // Between existing members, and outside the range at either end.
    for member in ["a", "c", "e", "g", "z"] {
        let proof = m.prove(member);
        assert!(matches!(proof, Proof::Absent { .. }), "{member}");
        assert!(verify(member, &proof, &root), "{member}");
    }
}

#[test]
fn an_absence_proof_does_not_verify_a_present_member() {
    let m = set(&["b", "d", "f"]);
    let root = m.root();
    // The proof that "c" is absent must not be reusable to claim "d" is.
    let proof = m.prove("c");
    assert!(verify("c", &proof, &root));
    assert!(
        !verify("d", &proof, &root),
        "d is present and must not verify"
    );
    assert!(!verify("b", &proof, &root));
}

#[test]
fn non_adjacent_brackets_are_rejected() {
    // The adjacency check is what stops a prover skipping over the member
    // whose absence is claimed.
    let m = set(&["b", "d", "f", "h"]);
    let root = m.root();
    let Proof::Absent { size, .. } = m.prove("c") else {
        panic!("expected absence");
    };
    let leaves: Vec<_> = ["b", "d", "f", "h"]
        .iter()
        .map(|s| publet_merkle::log::leaf_hash(s.as_bytes()))
        .collect();
    // Bracket "d" with "b" and "f", skipping over it.
    let forged = Proof::Absent {
        before: Some((
            0,
            "b".into(),
            publet_merkle::log::inclusion_proof(&leaves, 0),
        )),
        after: Some((
            2,
            "f".into(),
            publet_merkle::log::inclusion_proof(&leaves, 2),
        )),
        size,
    };
    assert!(
        !verify("d", &forged, &root),
        "brackets must be adjacent, or absence can be forged for a present member"
    );
}

#[test]
fn a_tampered_bracket_is_rejected() {
    let m = set(&["b", "d", "f"]);
    let root = m.root();
    let Proof::Absent {
        before,
        after,
        size,
    } = m.prove("c")
    else {
        panic!("expected absence");
    };
    let mut broken = before.clone().unwrap();
    broken.2[0][0] ^= 0xff;
    let forged = Proof::Absent {
        before: Some(broken),
        after,
        size,
    };
    assert!(!verify("c", &forged, &root));
}

#[test]
fn the_root_is_independent_of_insertion_order() {
    let forward = set(&["a", "b", "c", "d"]);
    let backward = Membership::new(["d", "c", "b", "a"].iter().map(|s| (*s).to_owned()));
    assert_eq!(forward.root(), backward.root());
}

#[test]
fn duplicates_do_not_change_the_root() {
    let plain = set(&["a", "b"]);
    let duplicated = Membership::new(["a", "b", "a", "b"].iter().map(|s| (*s).to_owned()));
    assert_eq!(plain.root(), duplicated.root());
    assert_eq!(duplicated.len(), 2);
}

#[test]
fn an_empty_membership_proves_absence_of_everything() {
    let m = Membership::default();
    let root = m.root();
    let proof = m.prove("anything");
    assert!(verify("anything", &proof, &root));
}

#[test]
fn diff_reports_additions_and_removals_in_sorted_order() {
    let before = set(&["a", "b", "c"]);
    let after = set(&["b", "c", "d", "e"]);
    let (added, removed) = diff(&before, &after);
    assert_eq!(added, vec!["d".to_owned(), "e".to_owned()]);
    assert_eq!(removed, vec!["a".to_owned()]);
}

#[test]
fn a_changed_membership_changes_the_root() {
    let before = set(&["a", "b", "c"]);
    let after = set(&["a", "b"]);
    assert_ne!(before.root(), after.root());
}
