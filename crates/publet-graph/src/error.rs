//! Errors raised while typing objects and building the graph.

use publet_core::Cid;
use thiserror::Error;

use crate::view::RelationKind;

/// A reason an object could not be typed or admitted.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum GraphError {
    /// The object was of a different type than the view expected.
    #[error("expected a `{expected}` object, found `{found}`")]
    WrongKind {
        /// The type the view reads.
        expected: &'static str,
        /// The type the object declares.
        found: String,
    },

    /// A required body field was absent.
    #[error("missing required field `{field}`")]
    MissingField {
        /// The absent field.
        field: &'static str,
    },

    /// A body field had the wrong type or shape.
    #[error("field `{field}` must be {expected}")]
    BadField {
        /// The offending field.
        field: &'static str,
        /// What the specification requires.
        expected: &'static str,
    },

    /// The claim class is not one the specification defines.
    #[error("unknown claim class `{found}`")]
    UnknownClass {
        /// The unrecognized identifier.
        found: String,
    },

    /// The principal is not one the specification defines.
    #[error("unknown principal `{found}`; a key declares human, organization, or automated")]
    UnknownPrincipal {
        /// The unrecognized identifier.
        found: String,
    },

    /// An empirical publet named no method.
    #[error(
        "an `empirical` publet must carry an evidence entry with role `method`; \
         a measurement without a reproducible method is a report of an experience"
    )]
    EmpiricalWithoutMethod,

    /// The relation kind is not one the specification defines.
    #[error("unknown relation kind `{found}`")]
    UnknownKind {
        /// The unrecognized identifier.
        found: String,
    },

    /// An edge of an acyclic kind would close a cycle.
    #[error("`{kind}` edge from {from} to {to} closes a cycle")]
    CycleClosing {
        /// The kind that must stay acyclic.
        kind: &'static str,
        /// The edge's subject.
        from: String,
        /// The edge's object.
        to: String,
    },

    /// A verdict annotation targeted a class that is not truth-apt.
    #[error("a verdict may not target a `{class}` publet; it is not truth-apt")]
    VerdictOnNonTruthApt {
        /// The offending class.
        class: &'static str,
    },

    /// A verdict on a provenance-only class named another aspect.
    #[error(
        "a verdict on a `{class}` publet must have aspect `provenance`, found {found:?}; \
         a claim about the content is made by publishing an `empirical` publet"
    )]
    VerdictAspectNotProvenance {
        /// The class whose verdicts are confined to provenance.
        class: &'static str,
        /// The aspect the annotation actually named.
        found: Option<String>,
    },

    /// A `disputes` relation did not name grounds.
    #[error("a `disputes` relation must name a publet stating grounds")]
    DisputeWithoutGrounds,

    /// An object referenced an identifier the graph does not hold.
    #[error("{referrer} references {missing}, which is not in the graph")]
    DanglingReference {
        /// The object holding the reference.
        referrer: String,
        /// The identifier it names.
        missing: String,
    },
}

impl GraphError {
    /// Build a cycle error for an edge.
    pub(crate) fn cycle(kind: RelationKind, from: &Cid, to: &Cid) -> Self {
        Self::CycleClosing {
            kind: kind.id(),
            from: from.to_string(),
            to: to.to_string(),
        }
    }
}
