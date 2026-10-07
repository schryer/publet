//! Recursive tree hashing: nesting, path lookup, order-independence,
//! and that a change anywhere propagates to every ancestor's hash.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use publet_algorithms::tree::Node;

fn leaf_fn(body: &str) -> Node {
    Node::leaf(body.as_bytes())
}

// covers: Node::hash, Node::leaf
#[test]
fn two_leaves_with_the_same_content_hash_the_same() {
    assert_eq!(leaf_fn("fn f() {}").hash(), leaf_fn("fn f() {}").hash());
}

// covers: Node::hash, Node::leaf
#[test]
fn two_leaves_with_different_content_hash_differently() {
    assert_ne!(leaf_fn("fn f() {}").hash(), leaf_fn("fn g() {}").hash());
}

// covers: Node::branch, Node::hash, Node::leaf, Node::with_child
#[test]
fn a_branch_commits_to_its_children() {
    let a = Node::branch("impl Foo")
        .with_child("bar", leaf_fn("fn bar() {}"))
        .with_child("baz", leaf_fn("fn baz() {}"));
    let b = Node::branch("impl Foo")
        .with_child("bar", leaf_fn("fn bar() {}"))
        .with_child("baz", leaf_fn("fn baz() {}"));
    assert_eq!(a.hash(), b.hash());

    let changed = Node::branch("impl Foo")
        .with_child("bar", leaf_fn("fn bar() { /* different */ }"))
        .with_child("baz", leaf_fn("fn baz() {}"));
    assert_ne!(
        a.hash(),
        changed.hash(),
        "changing one child must change the branch's hash"
    );
}

// covers: Node::branch, Node::hash, Node::leaf, Node::with_child
#[test]
fn child_insertion_order_does_not_affect_the_hash() {
    let forward = Node::branch("mod m")
        .with_child("a", leaf_fn("fn a() {}"))
        .with_child("b", leaf_fn("fn b() {}"))
        .with_child("c", leaf_fn("fn c() {}"));
    let backward = Node::branch("mod m")
        .with_child("c", leaf_fn("fn c() {}"))
        .with_child("b", leaf_fn("fn b() {}"))
        .with_child("a", leaf_fn("fn a() {}"));
    assert_eq!(forward.hash(), backward.hash());
}

// covers: Node::branch, Node::hash, Node::leaf, Node::with_child
#[test]
fn renaming_a_child_changes_the_branch_hash_even_if_content_is_identical() {
    // The name is part of what a branch commits to -- moving `bar`'s
    // content under the name `qux` is a different tree, not the same
    // one relocated, because a reader following the name `bar` must be
    // able to tell it is now absent.
    let original = Node::branch("mod m").with_child("bar", leaf_fn("fn shared() {}"));
    let renamed = Node::branch("mod m").with_child("qux", leaf_fn("fn shared() {}"));
    assert_ne!(original.hash(), renamed.hash());
}

// covers: Node::branch, Node::find, Node::hash, Node::leaf, Node::with_child
#[test]
fn nesting_propagates_a_leaf_change_to_every_ancestor() {
    let build = |body: &str| {
        Node::branch("mod outer").with_child(
            "inner",
            Node::branch("mod inner").with_child("f", leaf_fn(body)),
        )
    };
    let a = build("fn f() { 1 }");
    let b = build("fn f() { 2 }");
    assert_ne!(a.hash(), b.hash(), "root hash");
    assert_ne!(
        a.find("inner").unwrap().hash(),
        b.find("inner").unwrap().hash(),
        "intermediate branch hash"
    );
    assert_ne!(
        a.find("inner/f").unwrap().hash(),
        b.find("inner/f").unwrap().hash(),
        "leaf hash"
    );
}

// covers: Node::branch, Node::find, Node::hash, Node::leaf, Node::with_child
#[test]
fn find_resolves_a_slash_joined_path() {
    let tree = Node::branch("mod outer").with_child(
        "inner",
        Node::branch("mod inner").with_child("f", leaf_fn("fn f() {}")),
    );
    assert!(tree.find("inner/f").is_some());
    assert!(tree.find("inner/missing").is_none());
    assert!(tree.find("missing/f").is_none());
    assert_eq!(tree.find("").unwrap().hash(), tree.hash());
}

// covers: Node::branch, Node::leaf, Node::leaves, Node::with_child
#[test]
fn leaves_lists_every_leaf_with_its_full_path_in_sorted_order() {
    let tree = Node::branch("mod outer")
        .with_child("b", leaf_fn("fn b() {}"))
        .with_child(
            "a",
            Node::branch("mod a").with_child("nested", leaf_fn("fn nested() {}")),
        );
    let paths: Vec<String> = tree.leaves().into_iter().map(|(p, _)| p).collect();
    assert_eq!(paths, vec!["a/nested".to_owned(), "b".to_owned()]);
}

// covers: Node::branch, Node::hash
#[test]
fn a_leaf_and_a_branch_never_collide_even_with_matching_bytes() {
    // The branch tag (0x02) and the leaf tag (0x00, from crate::log)
    // guarantee this structurally, not by chance of the specific bytes
    // chosen in this test.
    let content = b"anything".to_vec();
    let as_leaf = Node::Leaf {
        content: content.clone(),
    };
    let as_branch = Node::branch(content);
    assert_ne!(as_leaf.hash(), as_branch.hash());
}
