//! Documents and lineage anchors (Sections 8 and 9.3).
//!
//! A document contains no assertions of its own. Its glosses are
//! presentational connective tissue, and an author who finds themselves
//! arguing in one is required by the design to publish a publet instead --
//! which drags implicit argument into the open where it can be cited and
//! contested.

use publet_core::{Cid, Object, cbor::Value};

use crate::{Graph, GraphError, Lineage};

/// How a document item refers to what it cites (Section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Bind {
    /// The citation is permanently fixed to one object.
    Object,
    /// The citation tracks a lineage; the head is resolved per viewpoint.
    Lineage,
}

impl Bind {
    /// Resolve the identifier used in a document item.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "object" => Self::Object,
            "lineage" => Self::Lineage,
            _ => return None,
        })
    }

    /// The identifier used in a document item.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Lineage => "lineage",
        }
    }
}

/// What a document does with the publet it cites.
///
/// The role is what makes a document's *use* explicit: including a publet
/// as a counterpoint is not endorsing it, and a composition-aware
/// evaluation can tell the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// The document asserts this.
    Assert,
    /// The document quotes it.
    Quote,
    /// The document sets it against something else.
    Contrast,
    /// The document supplies it as background.
    Background,
    /// The document presents it as an opposing view.
    Counterpoint,
}

impl Role {
    /// Resolve the identifier used in a document item.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "assert" => Self::Assert,
            "quote" => Self::Quote,
            "contrast" => Self::Contrast,
            "background" => Self::Background,
            "counterpoint" => Self::Counterpoint,
            _ => return None,
        })
    }

    /// The identifier used in a document item.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Assert => "assert",
            Self::Quote => "quote",
            Self::Contrast => "contrast",
            Self::Background => "background",
            Self::Counterpoint => "counterpoint",
        }
    }
}

/// One cited publet within a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What is cited: an object, or a lineage genesis.
    pub reference: Cid,
    /// Whether the citation is fixed or tracking.
    pub bind: Bind,
    /// The head the author actually read, for a tracking citation.
    pub at: Option<Cid>,
    /// How the document uses it.
    pub role: Role,
    /// Presentational connective tissue. Carries no claims.
    pub gloss: Option<String>,
}

/// A document manifest (Section 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    cid: Cid,
    title: String,
    items: Vec<Item>,
}

impl Document {
    /// Read a document from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not a document, if an item
    /// names an unknown role or binding, or if a lineage-bound item omits
    /// the head its author read.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "doc" {
            return Err(GraphError::WrongKind {
                expected: "doc",
                found: object.kind().to_owned(),
            });
        }
        let body = object.body();
        let title = body
            .get("title")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_owned();

        let mut items = Vec::new();
        if let Some(Value::Array(sections)) = body.get("sections") {
            for section in sections {
                let Some(Value::Array(entries)) = section.get("items") else {
                    continue;
                };
                for entry in entries {
                    items.push(parse_item(entry)?);
                }
            }
        }
        Ok(Self { cid, title, items })
    }

    /// This document's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// The title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The cited items, in order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Identifiers this document cites, in order.
    #[must_use]
    pub fn references(&self) -> Vec<Cid> {
        self.items.iter().map(|i| i.reference.clone()).collect()
    }
}

fn parse_item(entry: &Value) -> Result<Item, GraphError> {
    let reference: Cid = entry
        .get("ref")
        .and_then(Value::as_text)
        .and_then(|t| t.parse().ok())
        .ok_or(GraphError::BadField {
            field: "items.ref",
            expected: "a CID string",
        })?;

    let bind = match entry.get("bind").and_then(Value::as_text) {
        // Section 8: object binding is the default, so a document that says
        // nothing is citing one fixed object rather than tracking anything.
        None => Bind::Object,
        Some(id) => Bind::from_id(id).ok_or(GraphError::BadField {
            field: "items.bind",
            expected: "`object` or `lineage`",
        })?,
    };

    let at = entry
        .get("at")
        .and_then(Value::as_text)
        .and_then(|t| t.parse::<Cid>().ok());

    // A tracking citation without the head its author read gives a reader
    // only what is current, losing what the author actually saw -- which is
    // half of what the binding exists to provide.
    if bind == Bind::Lineage && at.is_none() {
        return Err(GraphError::LineageBindWithoutHead {
            reference: reference.to_string(),
        });
    }

    let role = match entry.get("role").and_then(Value::as_text) {
        None => Role::Assert,
        Some(id) => Role::from_id(id).ok_or(GraphError::BadField {
            field: "items.role",
            expected: "assert, quote, contrast, background, or counterpoint",
        })?,
    };

    Ok(Item {
        reference,
        bind,
        at,
        role,
        gloss: entry
            .get("gloss")
            .and_then(Value::as_text)
            .map(ToOwned::to_owned),
    })
}

/// A lineage anchor (Section 9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    cid: Cid,
    lineage: Cid,
    current: Cid,
    stewards: Vec<Cid>,
    threshold: u64,
}

impl Anchor {
    /// Read an anchor from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not an anchor or a required
    /// field is absent.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "anchor" {
            return Err(GraphError::WrongKind {
                expected: "anchor",
                found: object.kind().to_owned(),
            });
        }
        let body = object.body();
        let field = |name: &'static str| -> Result<Cid, GraphError> {
            body.get(name)
                .and_then(Value::as_text)
                .and_then(|t| t.parse().ok())
                .ok_or(GraphError::BadField {
                    field: name,
                    expected: "a CID string",
                })
        };
        Ok(Self {
            cid,
            lineage: field("lineage")?,
            current: field("current")?,
            stewards: match body.get("stewards") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| v.as_text().and_then(|t| t.parse().ok()))
                    .collect(),
                _ => Vec::new(),
            },
            threshold: body.get("threshold").and_then(Value::as_uint).unwrap_or(1),
        })
    }

    /// This anchor's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// The lineage it names, by genesis identifier.
    #[must_use]
    pub fn lineage(&self) -> &Cid {
        &self.lineage
    }

    /// The object it currently recommends.
    #[must_use]
    pub fn current(&self) -> &Cid {
        &self.current
    }

    /// The steward circle.
    #[must_use]
    pub fn stewards(&self) -> &[Cid] {
        &self.stewards
    }

    /// How many steward signatures an update requires.
    #[must_use]
    pub fn threshold(&self) -> u64 {
        self.threshold
    }

    /// Check that `current` is in the lineage this anchor names.
    ///
    /// An anchor pointing outside its own lineage is not a shortcut to the
    /// computation; it is a different claim wearing the shape of one.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::AnchorOutsideLineage`] if it is not.
    pub fn check_against(&self, graph: &Graph) -> Result<(), GraphError> {
        let view = graph.lineage(&self.lineage, Lineage::Full);
        if view.members.iter().any(|m| m == &self.current) {
            return Ok(());
        }
        Err(GraphError::AnchorOutsideLineage {
            current: self.current.to_string(),
            lineage: self.lineage.to_string(),
        })
    }
}
