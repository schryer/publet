//! Recursive Merkle trees over named, nested content -- the shape git's
//! own tree objects use (a directory's hash commits to its entries'
//! names and hashes, recursively), generalized past files and
//! directories to any leaf-or-branch structure.
//!
//! Neither of this crate's other two trees models nesting. [`crate::membership`]
//! is a sorted set: no entry contains another. [`crate::log`] is an
//! append-only sequence: entries have an order, not a hierarchy. A
//! source file's own definitions -- a method inside an `impl` block, a
//! function nested inside a function -- are a tree, not a set or a
//! sequence, and this is the third shape: every level of the hierarchy
//! commits to everything beneath it, so changing one deeply nested leaf
//! changes every ancestor's hash up to the root, the same
//! tamper-evidence property the other two trees give at their own
//! shape of structure.

use std::collections::BTreeMap;

use crate::log::{Hash, leaf_hash};

/// One position in the tree: a leaf holding raw content, or a branch
/// holding its own header bytes plus named children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Content hashed directly -- a function's exact source text, say.
    Leaf {
        /// The leaf's own bytes.
        content: Vec<u8>,
    },
    /// No content of its own beyond `header` (a signature line, a module
    /// path -- whatever names this branch without needing its children
    /// to render it), plus a set of named children, each itself a
    /// [`Node`]. Two branches with the same header and the same
    /// `(name, hash)` pairs -- regardless of child insertion order --
    /// hash identically; that is what makes moving a method's textual
    /// position in a file a no-op for its parent's hash.
    Branch {
        /// The branch's own identifying bytes, hashed but never
        /// expanded further -- there is nothing under `header` itself
        /// to recurse into.
        header: Vec<u8>,
        /// Children, keyed by name. A `BTreeMap` rather than a `Vec` of
        /// pairs so construction cannot accidentally depend on
        /// insertion order the way a hand-sorted `Vec` could if a
        /// caller forgot to sort it.
        children: BTreeMap<String, Node>,
    },
}

/// Domain-separation tag for a branch, distinct from [`crate::log`]'s
/// `0x00` (leaf) and `0x01` (interior binary node) so a hash from this
/// tree can never be mistaken for one from that one.
const BRANCH_TAG: u8 = 0x02;

impl Node {
    /// Build a leaf.
    #[must_use]
    pub fn leaf(content: impl Into<Vec<u8>>) -> Self {
        Self::Leaf {
            content: content.into(),
        }
    }

    /// Build a branch with no children yet.
    #[must_use]
    pub fn branch(header: impl Into<Vec<u8>>) -> Self {
        Self::Branch {
            header: header.into(),
            children: BTreeMap::new(),
        }
    }

    /// Add or replace a named child. Returns `self` for chaining.
    #[must_use]
    pub fn with_child(mut self, name: impl Into<String>, child: Self) -> Self {
        if let Self::Branch { children, .. } = &mut self {
            children.insert(name.into(), child);
        }
        self
    }

    /// Whether this is a branch (as opposed to a leaf).
    #[must_use]
    pub const fn is_branch(&self) -> bool {
        matches!(self, Self::Branch { .. })
    }

    /// This node's children, if it is a branch.
    #[must_use]
    pub fn children(&self) -> Option<&BTreeMap<String, Node>> {
        match self {
            Self::Branch { children, .. } => Some(children),
            Self::Leaf { .. } => None,
        }
    }

    /// The commitment: for a leaf, the hash of its content; for a
    /// branch, a hash of its header and every child's own `hash()`,
    /// paired with its name and length-prefixed so no concatenation of
    /// two different `(name, hash)` sequences can ever coincide.
    #[must_use]
    pub fn hash(&self) -> Hash {
        match self {
            Self::Leaf { content } => leaf_hash(content),
            Self::Branch { header, children } => {
                let mut buf = vec![BRANCH_TAG];
                buf.extend_from_slice(&(header.len() as u64).to_le_bytes());
                buf.extend_from_slice(header);
                // `children` is a BTreeMap, so this iterates in sorted
                // key order already -- the same order regardless of
                // insertion order, which is the whole point.
                for (name, child) in children {
                    let name_bytes = name.as_bytes();
                    buf.extend_from_slice(&(name_bytes.len() as u64).to_le_bytes());
                    buf.extend_from_slice(name_bytes);
                    buf.extend_from_slice(&child.hash());
                }
                leaf_hash(&buf)
            }
        }
    }

    /// Find the node at a `/`-separated path of child names, if it
    /// exists. `find("")` returns `self`.
    #[must_use]
    pub fn find(&self, path: &str) -> Option<&Node> {
        if path.is_empty() {
            return Some(self);
        }
        let (head, rest) = path.split_once('/').unwrap_or((path, ""));
        self.children()?.get(head)?.find(rest)
    }

    /// Every leaf's hash paired with its full `/`-joined path from this
    /// node, in sorted path order.
    #[must_use]
    pub fn leaves(&self) -> Vec<(String, Hash)> {
        let mut out = Vec::new();
        self.collect_leaves(String::new(), &mut out);
        out
    }

    fn collect_leaves(&self, prefix: String, out: &mut Vec<(String, Hash)>) {
        match self {
            Self::Leaf { .. } => out.push((prefix, self.hash())),
            Self::Branch { children, .. } => {
                for (name, child) in children {
                    let path = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{prefix}/{name}")
                    };
                    child.collect_leaves(path, out);
                }
            }
        }
    }
}
