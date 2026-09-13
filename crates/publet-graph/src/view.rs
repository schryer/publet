//! Typed views over the object types this crate indexes.
//!
//! An [`Object`] carries an untyped body. These views read the fields each
//! type is required to have, failing rather than defaulting when one is
//! absent, so a malformed object never enters the graph wearing a shape it
//! does not have.

use publet_core::{Cid, Object, cbor::Value};

use crate::GraphError;

/// A claim class (Section 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Class {
    /// A mathematical or logical statement, settled by proof checking.
    Formal,
    /// A claim about the observable world, settled by reproduction.
    Empirical,
    /// "X said Y", verified on the provenance of the attribution.
    Attributive,
    /// A stipulated meaning, described by usage rather than voted true.
    Definitional,
    /// An ought-claim. Not truth-apt.
    Normative,
    /// Poetry, fiction, testimony. Not truth-apt.
    Expressive,
    /// A primary-source record, verified on provenance and custody.
    Archival,
    /// An instruction or method, settled by reproduction reports.
    Procedural,
}

impl Class {
    /// Resolve the identifier used in a publet body.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "formal" => Self::Formal,
            "empirical" => Self::Empirical,
            "attributive" => Self::Attributive,
            "definitional" => Self::Definitional,
            "normative" => Self::Normative,
            "expressive" => Self::Expressive,
            "archival" => Self::Archival,
            "procedural" => Self::Procedural,
            _ => return None,
        })
    }

    /// The identifier used in a publet body.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Formal => "formal",
            Self::Empirical => "empirical",
            Self::Attributive => "attributive",
            Self::Definitional => "definitional",
            Self::Normative => "normative",
            Self::Expressive => "expressive",
            Self::Archival => "archival",
            Self::Procedural => "procedural",
        }
    }

    /// Whether a verdict annotation may target a publet of this class.
    ///
    /// Section 5.2: `definitional`, `normative`, and `expressive` are not
    /// truth-apt, and a verdict on one is a category error rather than a
    /// disagreement.
    #[must_use]
    pub fn accepts_verdict(self) -> bool {
        !matches!(
            self,
            Self::Definitional | Self::Normative | Self::Expressive
        )
    }

    /// Whether this class's content is itself the assertion (Section 5.7).
    ///
    /// The atomicity tests apply only here. The other four classes
    /// instruct, quote, record, or express, and "negating it yields exactly
    /// one coherent counter-claim" has no reading for any of them: a
    /// procedure does not negate, and negating an attribution denies that
    /// *X* said it, which says nothing about how many sentences *Y* has.
    #[must_use]
    pub fn content_is_assertion(self) -> bool {
        matches!(
            self,
            Self::Formal | Self::Empirical | Self::Definitional | Self::Normative
        )
    }

    /// Whether a verdict on this class is confined to provenance.
    ///
    /// Section 5.2: for `attributive` and `archival`, a verdict speaks to
    /// whether the attribution or the custody chain holds, never to whether
    /// the quoted or archived content is true. That claim is made by
    /// publishing an `empirical` publet asserting it, and contesting that.
    #[must_use]
    pub fn verdict_confined_to_provenance(self) -> bool {
        matches!(self, Self::Attributive | Self::Archival)
    }
}

/// A publet (Section 5).
#[derive(Debug, Clone)]
pub struct Publet {
    cid: Cid,
    class: Class,
    lang: String,
    content: String,
    depends: Vec<Cid>,
}

impl Publet {
    /// Read a publet from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not a publet or if a required
    /// body field is absent or ill-typed.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "publet" {
            return Err(GraphError::WrongKind {
                expected: "publet",
                found: object.kind().to_owned(),
            });
        }
        let body = object.body();
        let class_id = text(body.get("class"), "class")?;
        let class = Class::from_id(&class_id).ok_or_else(|| GraphError::UnknownClass {
            found: class_id.clone(),
        })?;
        let depends = match body.get("depends") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_text()
                        .ok_or(GraphError::BadField {
                            field: "depends",
                            expected: "an array of CID strings",
                        })
                        .and_then(|t| {
                            t.parse::<Cid>().map_err(|_| GraphError::BadField {
                                field: "depends",
                                expected: "an array of CID strings",
                            })
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(GraphError::BadField {
                    field: "depends",
                    expected: "an array",
                });
            }
            None => Vec::new(),
        };
        // Section 5.5: a measurement without a reproducible method is a
        // report of an experience. Requiring the method object makes its
        // absence visible rather than assumed.
        if class == Class::Empirical && !names_a_method(body.get("evidence")) {
            return Err(GraphError::EmpiricalWithoutMethod);
        }

        Ok(Self {
            cid,
            class,
            lang: text(body.get("lang"), "lang")?,
            content: text(body.get("content"), "content")?,
            depends,
        })
    }

    /// This publet's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// The claim class.
    #[must_use]
    pub fn class(&self) -> Class {
        self.class
    }

    /// The BCP 47 language tag.
    #[must_use]
    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// The assertion.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Publets whose meaning this one presupposes.
    #[must_use]
    pub fn depends(&self) -> &[Cid] {
        &self.depends
    }

    /// The term a `definitional` publet defines.
    ///
    /// Section 9.2 refers to `term(D)` without saying how it is obtained.
    /// This implementation reads `ext["term"]` when present, and otherwise
    /// takes the text before the first colon in `content`, which is the
    /// shape the specification's own example uses:
    ///
    /// ```text
    /// quantum (adj., physics): pertaining to the discrete quantization ...
    /// ```
    ///
    /// A parenthesized qualifier is dropped, so `quantum (adj., physics)`
    /// and `quantum (n.)` yield the same term and are therefore comparable
    /// -- which is the point of the computation. Returns `None` for classes
    /// other than `definitional`.
    #[must_use]
    pub fn term(&self, object: &Object) -> Option<String> {
        if self.class != Class::Definitional {
            return None;
        }
        if let Some(ext) = object.ext()
            && let Some(Value::Text(t)) = ext.get("term")
        {
            return Some(t.trim().to_lowercase());
        }
        let head = self.content.split(':').next()?;
        let without_qualifier = head.split('(').next().unwrap_or(head);
        let trimmed = without_qualifier.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }
}

/// Whether an evidence list carries an entry with `role: "method"`.
fn names_a_method(evidence: Option<&Value>) -> bool {
    let Some(Value::Array(entries)) = evidence else {
        return false;
    };
    entries
        .iter()
        .any(|entry| entry.get("role").and_then(Value::as_text) == Some("method"))
}

fn text(value: Option<&Value>, field: &'static str) -> Result<String, GraphError> {
    match value {
        Some(Value::Text(s)) => Ok(s.clone()),
        Some(_) => Err(GraphError::BadField {
            field,
            expected: "a text string",
        }),
        None => Err(GraphError::MissingField { field }),
    }
}

/// What a key declares itself to be (Section 10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Principal {
    /// A person. Only human principals may author, be counted toward a
    /// replication floor, or hold stewardship (R11).
    Human,
    /// An organization. It may sign, fund, and operate infrastructure.
    Organization,
    /// An automated system.
    Automated,
}

impl Principal {
    /// Resolve the identifier used in a key object.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "human" => Self::Human,
            "organization" => Self::Organization,
            "automated" => Self::Automated,
            _ => return None,
        })
    }

    /// The identifier used in a key object.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Organization => "organization",
            Self::Automated => "automated",
        }
    }

    /// Whether this principal may author and be counted.
    ///
    /// Institutions are immortal and humans are not. An organizational key
    /// accrues merit indefinitely and compounds authority across
    /// generations of staff who bear none of the consequences, so only
    /// human principals carry authorship weight (R11).
    #[must_use]
    pub fn is_human(self) -> bool {
        matches!(self, Self::Human)
    }
}

/// A key object (Section 10.1).
#[derive(Debug, Clone)]
pub struct Key {
    cid: Cid,
    principal: Principal,
}

impl Key {
    /// Read a key from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not a key, or if `principal`
    /// is absent or unrecognized. The field is required because every rule
    /// that turns on who may author depends on it, and a key that declines
    /// to say is a key no rule can be applied to.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "key" {
            return Err(GraphError::WrongKind {
                expected: "key",
                found: object.kind().to_owned(),
            });
        }
        let declared = text(object.body().get("principal"), "principal")?;
        let principal =
            Principal::from_id(&declared).ok_or_else(|| GraphError::UnknownPrincipal {
                found: declared.clone(),
            })?;
        Ok(Self { cid, principal })
    }

    /// This key's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// What the key declares itself to be.
    #[must_use]
    pub fn principal(&self) -> Principal {
        self.principal
    }
}

/// A relation kind (Section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RelationKind {
    /// `from` replaces `to`. Acyclic; constitutes lineage.
    Supersedes,
    /// `from` translates `to`.
    Translates,
    /// `from` presupposes `to`. Acyclic.
    Depends,
    /// `from` argues against `to`.
    Disputes,
    /// `from` argues for `to`.
    Supports,
    /// `from` and `to` express the same claim.
    Equivalent,
    /// The signer withdraws a prior assertion.
    Retracts,
    /// `from` copies or adapts `to`. Acyclic.
    DerivedFrom,
    /// `from` authorizes `to` to continue a line of work.
    Delegates,
}

impl RelationKind {
    /// Resolve the identifier used in a relation body.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "supersedes" => Self::Supersedes,
            "translates" => Self::Translates,
            "depends" => Self::Depends,
            "disputes" => Self::Disputes,
            "supports" => Self::Supports,
            "equivalent" => Self::Equivalent,
            "retracts" => Self::Retracts,
            "derived-from" => Self::DerivedFrom,
            "delegates" => Self::Delegates,
            _ => return None,
        })
    }

    /// The identifier used in a relation body.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Supersedes => "supersedes",
            Self::Translates => "translates",
            Self::Depends => "depends",
            Self::Disputes => "disputes",
            Self::Supports => "supports",
            Self::Equivalent => "equivalent",
            Self::Retracts => "retracts",
            Self::DerivedFrom => "derived-from",
            Self::Delegates => "delegates",
        }
    }

    /// Whether an edge of this kind closing a cycle must be rejected.
    ///
    /// Section 6: relations may form cycles in general -- mutual dispute is
    /// ordinary disagreement and `equivalent` cycles are how classes form.
    /// These three express a lineage or a presupposition, which a cycle
    /// would make incoherent.
    #[must_use]
    pub fn is_acyclic(self) -> bool {
        matches!(self, Self::Supersedes | Self::Depends | Self::DerivedFrom)
    }
}

/// A relation (Section 6).
#[derive(Debug, Clone)]
pub struct Relation {
    cid: Cid,
    author: Cid,
    kind: RelationKind,
    from: Cid,
    to: Cid,
}

impl Relation {
    /// Read a relation from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not a relation or if a
    /// required body field is absent, ill-typed, or an unknown kind.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "rel" {
            return Err(GraphError::WrongKind {
                expected: "rel",
                found: object.kind().to_owned(),
            });
        }
        let body = object.body();
        let kind_id = text(body.get("kind"), "kind")?;
        let kind = RelationKind::from_id(&kind_id).ok_or_else(|| GraphError::UnknownKind {
            found: kind_id.clone(),
        })?;
        Ok(Self {
            cid,
            author: object.author().clone(),
            kind,
            from: cid_field(body.get("from"), "from")?,
            to: cid_field(body.get("to"), "to")?,
        })
    }

    /// This relation's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// The key that authored the relation.
    #[must_use]
    pub fn author(&self) -> &Cid {
        &self.author
    }

    /// The relation kind.
    #[must_use]
    pub fn kind(&self) -> RelationKind {
        self.kind
    }

    /// The subject of the assertion.
    #[must_use]
    pub fn from(&self) -> &Cid {
        &self.from
    }

    /// The object of the assertion.
    #[must_use]
    pub fn to(&self) -> &Cid {
        &self.to
    }
}

fn cid_field(value: Option<&Value>, field: &'static str) -> Result<Cid, GraphError> {
    let text = text(value, field)?;
    text.parse().map_err(|_| GraphError::BadField {
        field,
        expected: "a CID string",
    })
}

/// An annotation (Section 7).
#[derive(Debug, Clone)]
pub struct Annotation {
    cid: Cid,
    author: Cid,
    kind: String,
    target: Cid,
    aspect: Option<String>,
}

impl Annotation {
    /// Read an annotation from a verified object.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object is not an annotation or if a
    /// required body field is absent or ill-typed.
    pub fn from_object(cid: Cid, object: &Object) -> Result<Self, GraphError> {
        if object.kind() != "ann" {
            return Err(GraphError::WrongKind {
                expected: "ann",
                found: object.kind().to_owned(),
            });
        }
        let body = object.body();
        Ok(Self {
            cid,
            author: object.author().clone(),
            kind: text(body.get("kind"), "kind")?,
            target: cid_field(body.get("target"), "target")?,
            aspect: match body.get("aspect") {
                Some(Value::Text(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(GraphError::BadField {
                        field: "aspect",
                        expected: "a text string",
                    });
                }
                None => None,
            },
        })
    }

    /// This annotation's identifier.
    #[must_use]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// The key that authored the annotation.
    #[must_use]
    pub fn author(&self) -> &Cid {
        &self.author
    }

    /// The annotation kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// What the annotation is about.
    #[must_use]
    pub fn target(&self) -> &Cid {
        &self.target
    }

    /// The narrowing aspect, if the annotation names one.
    #[must_use]
    pub fn aspect(&self) -> Option<&str> {
        self.aspect.as_deref()
    }
}
