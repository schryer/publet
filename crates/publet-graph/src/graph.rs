//! The object graph: indices, lineage, closures, and equivalence classes.
//!
//! The graph accepts only [`Verified`] objects, so nothing whose identity
//! was never confirmed can be indexed. Edges of the acyclic kinds are
//! checked as they are added rather than in a later pass, which means a
//! cycle is reported against the edge that created it.

use std::collections::{BTreeMap, BTreeSet};

use publet_core::{Cid, Object, SigAlg, Verified, cbor::Value};

use crate::GraphError;
use crate::document::{Anchor, Document};
use crate::view::{Annotation, Class, ProseClaim, Relation, RelationKind};

/// The purpose both keys sign a rotating key object for (Section 10.5).
///
/// Section 10.5 says only that the new key object is "signed by both
/// keys"; a signature always covers a stated purpose (Section 4.4), and this
/// is the one a rotation is signed for, so a signature made for another
/// purpose cannot be taken for consent to a rotation.
pub const ROTATION_PURPOSE: &str = "rotation";

/// The most rotations followed from one key back to an earlier one. A chain
/// is bounded so that a crafted cycle or a very long chain cannot make a
/// query unbounded.
pub const MAX_ROTATIONS: usize = 64;

/// Which `supersedes` edges a lineage query should follow (R15, Section 6.1).
///
/// R15 splits the two levels apart: assertion, evidence and signature
/// operate on objects, whereas curation, stewardship and definitional drift
/// operate on lineages. This type is the lineage side of that split, and
/// the reason a query has to say which edges it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lineage {
    /// Only edges signed by a key that also signed the target: an author's
    /// own revision history.
    Authoritative,
    /// Additionally, third-party proposals to replace.
    Full,
}

/// A lineage view rooted at a genesis object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageView {
    /// The object with no outbound `supersedes` edge.
    pub genesis: Cid,
    /// Every object in the lineage, genesis first, in breadth-first order.
    pub members: Vec<Cid>,
    /// Members nothing supersedes. More than one means the lineage branched.
    pub heads: Vec<Cid>,
    /// Which edges were followed.
    pub mode: Lineage,
}

impl LineageView {
    /// Whether this lineage has more than one head.
    ///
    /// Section 6.1: branching is permitted, and selecting among heads is
    /// viewpoint-relative rather than a question this layer answers.
    #[must_use]
    pub fn is_branched(&self) -> bool {
        self.heads.len() > 1
    }
}

/// An indexed set of verified objects.
#[derive(Debug, Default)]
pub struct Graph {
    objects: BTreeMap<String, Object>,
    prose_claims: BTreeMap<String, ProseClaim>,
    relations: BTreeMap<String, Relation>,
    annotations: BTreeMap<String, Annotation>,
    documents: BTreeMap<String, Document>,
    anchors: BTreeMap<String, Anchor>,
    /// kind -> from -> to
    out_edges: BTreeMap<RelationKind, BTreeMap<String, BTreeSet<String>>>,
    /// kind -> to -> from
    in_edges: BTreeMap<RelationKind, BTreeMap<String, BTreeSet<String>>>,
    /// (kind, from, to) -> the relation objects asserting that edge.
    ///
    /// Needed because whether a `supersedes` edge is authoritative depends
    /// on who authored the relation and who authored its target, and both
    /// can arrive in any order. Resolving it here at query time rather than
    /// at insert time removes that ordering dependency.
    edge_relations: BTreeMap<(RelationKind, String, String), BTreeSet<String>>,
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a verified object, typing it and indexing its edges.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if the object cannot be typed, if a relation
    /// of an acyclic kind would close a cycle, or if an annotation violates
    /// the class rules of Section 5.2.
    pub fn insert(&mut self, cid: &Cid, object: Verified<Object>) -> Result<(), GraphError> {
        let object = object.into_inner();
        let key = cid.to_string();
        match object.kind() {
            "claim.prose" => {
                let claim = ProseClaim::from_object(cid.clone(), &object)?;
                self.prose_claims.insert(key.clone(), claim);
            }
            "claim.relation" => {
                let relation = Relation::from_object(cid.clone(), &object)?;
                self.index_relation(&relation)?;
                self.relations.insert(key.clone(), relation);
            }
            "claim.annotation" => {
                let annotation = Annotation::from_object(cid.clone(), &object)?;
                self.check_annotation(&annotation)?;
                self.annotations.insert(key.clone(), annotation);
            }
            "doc" => {
                let document = Document::from_object(cid.clone(), &object)?;
                self.documents.insert(key.clone(), document);
            }
            "anchor" => {
                let anchor = Anchor::from_object(cid.clone(), &object)?;
                self.anchors.insert(key.clone(), anchor);
            }
            _ => {}
        }
        self.objects.insert(key, object);
        Ok(())
    }

    fn index_relation(&mut self, relation: &Relation) -> Result<(), GraphError> {
        let kind = relation.kind();
        let from = relation.from().to_string();
        let to = relation.to().to_string();

        if kind.is_acyclic() && self.would_close_cycle(kind, &from, &to) {
            return Err(GraphError::cycle(kind, relation.from(), relation.to()));
        }

        self.out_edges
            .entry(kind)
            .or_default()
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.in_edges
            .entry(kind)
            .or_default()
            .entry(to.clone())
            .or_default()
            .insert(from.clone());

        self.edge_relations
            .entry((kind, from, to))
            .or_default()
            .insert(relation.cid().to_string());
        Ok(())
    }

    /// Whether any relation asserting `from -> to` carries author weight.
    ///
    /// Section 6: a `supersedes` edge is authoritative only when signed by a
    /// key that also signed the target -- the author's own revision history
    /// rather than a third party's proposal to replace.
    ///
    /// Evaluated on demand. Doing this on insert would make the answer
    /// depend on whether the relation or its target was loaded first, which
    /// is not a property of the graph.
    ///
    /// A fuller implementation also admits a detached signature over the
    /// target by the relation's author; that needs the signature index,
    /// which arrives with the store.
    fn edge_is_authoritative(&self, kind: RelationKind, from: &str, to: &str) -> bool {
        let Some(target) = self.objects.get(to) else {
            return false;
        };
        self.edge_relations
            .get(&(kind, from.to_owned(), to.to_owned()))
            .is_some_and(|cids| {
                cids.iter().any(|c| {
                    self.relations.get(c).is_some_and(|r| {
                        r.author() == target.author()
                            // Section 10.6: a `lineage` delegate continues
                            // the delegator's lineages with its authority.
                            || (kind == RelationKind::Supersedes
                                && self.delegates_lineage(target.author(), r.author()))
                            // Section 10.5: a key rotated from the author
                            // continues it.
                            || self.continues_key(target.author(), r.author())
                    })
                })
            })
    }

    /// Would adding `from -> to` create a cycle in this kind's edges?
    fn would_close_cycle(&self, kind: RelationKind, from: &str, to: &str) -> bool {
        if from == to {
            return true;
        }
        let Some(edges) = self.out_edges.get(&kind) else {
            return false;
        };
        // A cycle forms exactly when `from` is already reachable from `to`.
        publet_algorithms::graph::reaches(to, from, |current| {
            edges
                .get(current)
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        })
    }

    /// Reject annotations the class rules of Section 5.2 forbid.
    ///
    /// One rule over every grammar. A prose claim declares its class; a
    /// relation and an annotation take the class their `kind` carries
    /// (Sections 6 and 7.1); the same admissibility check runs over all
    /// three. Until Section 5.2 gained a `performative` class this needed
    /// a second rule written specially for relations, which was the same
    /// rule with different words.
    fn check_annotation(&self, annotation: &Annotation) -> Result<(), GraphError> {
        if annotation.kind() != "verdict" {
            return Ok(());
        }
        let Some(class) = self.class_of(&annotation.target().to_string()) else {
            // Either the target has not arrived yet -- `validate` enforces
            // the rule again once the graph is complete -- or it is an
            // object with no class, which nothing here judges.
            return Ok(());
        };
        Self::check_verdict(class, annotation.aspect())
    }

    /// The class of whatever `key` names, whichever grammar it is in.
    fn class_of(&self, key: &str) -> Option<Class> {
        if let Some(claim) = self.prose_claims.get(key) {
            return Some(claim.class());
        }
        if let Some(relation) = self.relations.get(key) {
            return Some(relation.kind().class());
        }
        self.annotations.get(key).and_then(Annotation::class)
    }

    fn check_verdict(class: Class, aspect: Option<&str>) -> Result<(), GraphError> {
        if !class.accepts_verdict() {
            return Err(GraphError::VerdictOnNonTruthApt { class: class.id() });
        }
        if class.verdict_confined_to_provenance() && aspect != Some("provenance") {
            return Err(GraphError::VerdictAspectNotProvenance {
                class: class.id(),
                found: aspect.map(str::to_owned),
            });
        }
        Ok(())
    }

    /// Re-check rules that depend on objects arriving in any order.
    ///
    /// Objects may be inserted before the things they refer to, so class
    /// rules whose subject is a target are checked here as well as on
    /// insert. Call once the graph is loaded.
    ///
    /// # Errors
    ///
    /// Returns the first [`GraphError`] found.
    pub fn validate(&self) -> Result<(), GraphError> {
        for annotation in self.annotations.values() {
            if annotation.kind() != "verdict" {
                continue;
            }
            if let Some(class) = self.class_of(&annotation.target().to_string()) {
                Self::check_verdict(class, annotation.aspect())?;
            }
        }
        for anchor in self.anchors.values() {
            anchor.check_against(self)?;
        }
        for relation in self.relations.values() {
            if relation.kind() == RelationKind::Disputes
                && !self.prose_claims.contains_key(&relation.from().to_string())
                && !self.objects.contains_key(&relation.from().to_string())
            {
                return Err(GraphError::DisputeWithoutGrounds);
            }
        }
        Ok(())
    }

    /// Every identifier in the graph, in sorted order.
    #[must_use]
    pub fn cids(&self) -> Vec<&str> {
        self.objects.keys().map(String::as_str).collect()
    }

    /// Borrow an object.
    #[must_use]
    pub fn object(&self, cid: &Cid) -> Option<&Object> {
        self.objects.get(&cid.to_string())
    }

    /// Borrow a typed document.
    #[must_use]
    pub fn document(&self, cid: &Cid) -> Option<&Document> {
        self.documents.get(&cid.to_string())
    }

    /// Every document, in identifier order.
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.documents.values()
    }

    /// Borrow a typed anchor.
    #[must_use]
    pub fn anchor(&self, cid: &Cid) -> Option<&Anchor> {
        self.anchors.get(&cid.to_string())
    }

    /// Every anchor held, in identifier order.
    pub fn anchors(&self) -> impl Iterator<Item = &Anchor> {
        self.anchors.values()
    }

    /// Every annotation targeting `target`, of any kind, in identifier
    /// order.
    #[must_use]
    pub fn annotations_of(&self, target: &Cid) -> Vec<&Annotation> {
        self.annotations
            .values()
            .filter(|a| a.target() == target)
            .collect()
    }

    /// Whether a `retracts` relation felicitously applies to `cid` (Section
    /// 6: retraction is authoritative only from a key that signed the
    /// target). Retraction never deletes (R12); this only says whether one
    /// such relation currently applies, so a reader can tell current
    /// standing from history rather than seeing everything ever filed.
    #[must_use]
    pub fn is_retracted(&self, cid: &Cid) -> bool {
        let Some(target) = self.object(cid) else {
            return false;
        };
        let author = target.author().to_string();
        self.incoming(RelationKind::Retracts, cid)
            .into_iter()
            .any(|from| {
                from.parse::<Cid>()
                    .ok()
                    .and_then(|c| self.object(&c))
                    .is_some_and(|r| r.author().to_string() == author)
            })
    }

    /// Whether the `supersedes` relation `relation` is felicitous: a
    /// revision rather than a proposal to replace (Section 6).
    ///
    /// It is when its author signed the object it supersedes, holds an
    /// immediate `lineage` delegation from that key (Section 10.6:
    /// delegation "transfers the capacity to continue -- to supersede"), or
    /// is a key rotated from it (Section 10.5: rotation "transfers
    /// continuity"; see [`Graph::continues_key`]).
    /// The delegation must itself be authored by the delegating key and
    /// not retracted. An `on-dormancy` delegation is not counted: whether
    /// the stated silence has elapsed is a fact about time this graph does
    /// not hold. Delegations are not followed transitively.
    #[must_use]
    pub fn supersedes_is_felicitous(&self, relation: &Cid) -> bool {
        let Some(rel) = self.relations.get(&relation.to_string()) else {
            return false;
        };
        if rel.kind() != RelationKind::Supersedes {
            return false;
        }
        let Some(target) = self.object(rel.to()) else {
            return false;
        };
        let original = target.author();
        rel.author() == original
            || self.delegates_lineage(original, rel.author())
            || self.continues_key(original, rel.author())
    }

    /// The key `successor` rotates from: the `rotates` field of its key
    /// object, counted only when that key object is signed, for
    /// [`ROTATION_PURPOSE`], by both the key it names and itself (Section
    /// 10.5). Each signature is a `sig` object in this graph, authored by
    /// the signing key, that verifies against the key's own public key.
    #[must_use]
    pub fn rotated_from(&self, successor: &Cid) -> Option<Cid> {
        let object = self.objects.get(&successor.to_string())?;
        if object.kind() != "key" {
            return None;
        }
        let previous: Cid = object.body().get("rotates")?.as_text()?.parse().ok()?;
        (self.signed_by(&previous, successor, ROTATION_PURPOSE)
            && self.signed_by(successor, successor, ROTATION_PURPOSE))
        .then_some(previous)
    }

    /// Whether `successor` is `original`, or reaches it through at most
    /// [`MAX_ROTATIONS`] rotations, each one counted by
    /// [`Graph::rotated_from`].
    #[must_use]
    pub fn continues_key(&self, original: &Cid, successor: &Cid) -> bool {
        let mut key = successor.clone();
        for _ in 0..=MAX_ROTATIONS {
            if &key == original {
                return true;
            }
            match self.rotated_from(&key) {
                Some(previous) => key = previous,
                None => return false,
            }
        }
        false
    }

    /// Whether a `sig` object in this graph, authored by the key `key`,
    /// signs the object `target` for `purpose` with a signature that
    /// verifies against the public key `key`'s own object states.
    fn signed_by(&self, key: &Cid, target: &Cid, purpose: &str) -> bool {
        let Some(key_object) = self.objects.get(&key.to_string()) else {
            return false;
        };
        let text =
            |o: &Object, k: &str| o.body().get(k).and_then(Value::as_text).map(str::to_owned);
        if key_object.kind() != "key" || text(key_object, "alg").as_deref() != Some("ed25519") {
            return false;
        }
        let Some(Value::Bytes(public)) = key_object.body().get("pubkey") else {
            return false;
        };
        let Some(target_object) = self.objects.get(&target.to_string()) else {
            return false;
        };
        let target_text = target.to_string();
        self.objects.values().any(|o| {
            o.kind() == "sig"
                && o.author() == key
                && text(o, "target").as_deref() == Some(target_text.as_str())
                && matches!(o.body().get("value"), Some(Value::Bytes(value))
                    if text(o, "purpose").is_some_and(|found| publet_core::verify(
                        SigAlg::Ed25519, public, value, &found, purpose, target_object.bytes(),
                    ).is_ok()))
        })
    }

    /// Whether `from` has delegated the `lineage` aspect to `to`,
    /// immediately, by a relation `from` itself authored and has not
    /// retracted (Section 10.6).
    #[must_use]
    pub fn delegates_lineage(&self, from: &Cid, to: &Cid) -> bool {
        self.relations.values().any(|r| {
            r.kind() == RelationKind::Delegates
                && r.from() == from
                && r.to() == to
                && r.author() == from
                && self.object(r.cid()).is_some_and(|o| {
                    let field = |k: &str| o.body().get(k).and_then(Value::as_text);
                    field("aspect") == Some("lineage")
                        && field("effective").is_none_or(|e| e == "immediate")
                })
                && !self.is_retracted(r.cid())
        })
    }

    /// Documents whose citations substantially overlap another's without
    /// declaring `derived-from` (Section 8).
    ///
    /// Forking is permitted and cannot be prevented; what the protocol can
    /// do is make an undeclared one conspicuous. Overlap is a signal, not a
    /// verdict: two documents on one subject will cite the same claims,
    /// which is why the threshold is high and the result is reported rather
    /// than enforced.
    #[must_use]
    pub fn undeclared_forks(&self, threshold_percent: u32) -> Vec<(String, String, u32)> {
        let mut out = Vec::new();
        let docs: Vec<&Document> = self.documents.values().collect();
        for (i, left) in docs.iter().enumerate() {
            let left_refs: BTreeSet<String> =
                left.references().iter().map(ToString::to_string).collect();
            if left_refs.is_empty() {
                continue;
            }
            for right in docs.iter().skip(i + 1) {
                let right_refs: BTreeSet<String> =
                    right.references().iter().map(ToString::to_string).collect();
                if right_refs.is_empty() {
                    continue;
                }
                let shared = left_refs.intersection(&right_refs).count();
                let smaller = left_refs.len().min(right_refs.len());
                // Integer division is intended: a percentage of citations
                // shared, truncated, is the signal. Precision beyond that
                // would imply the threshold means more than it does.
                #[allow(clippy::integer_division)]
                let percent = u32::try_from(shared * 100 / smaller).unwrap_or(0);
                if percent < threshold_percent {
                    continue;
                }
                if self.declares_derivation(left.cid(), right.cid()) {
                    continue;
                }
                out.push((left.cid().to_string(), right.cid().to_string(), percent));
            }
        }
        out
    }

    fn declares_derivation(&self, a: &Cid, b: &Cid) -> bool {
        self.out(RelationKind::DerivedFrom, a)
            .contains(&b.to_string())
            || self
                .out(RelationKind::DerivedFrom, b)
                .contains(&a.to_string())
    }

    /// Keys this key has publicly assumed accountability for (Section 10.7).
    ///
    /// The count is the check. A single assumption is indistinguishable
    /// from protecting a dissident or from fronting for a PR firm, and
    /// deliberately so; what separates them is visible only in aggregate,
    /// which is why assumptions are published rather than hidden.
    #[must_use]
    pub fn assumptions_by(&self, assumer: &Cid) -> Vec<crate::Assumption> {
        self.annotations
            .values()
            .filter(|a| a.kind() == "assumes-accountability")
            .filter_map(|a| self.objects.get(&a.cid().to_string()))
            .filter_map(crate::Assumption::from_object)
            .filter(|a| &a.assumer == assumer)
            .collect()
    }

    /// Every triage annotation targeting an object (Section 7.4).
    ///
    /// Exposed for display and for appeal, and read by nothing that
    /// computes. Triage may order attention; it may not decide anything.
    #[must_use]
    pub fn triage_of(&self, target: &Cid) -> Vec<crate::Triage> {
        self.annotations
            .values()
            .filter(|a| a.kind() == "triage" && a.target() == target)
            .filter_map(|a| self.objects.get(&a.cid().to_string()))
            .filter_map(crate::Triage::from_object)
            .collect()
    }

    /// Every term a `definitional` claim in this graph fixes.
    ///
    /// Returns *all* definitions of each term, and deliberately does not
    /// collapse them. Competing definitions for one string are the normal
    /// case rather than a conflict to resolve (Section 9.1), and returning
    /// a single winner would be this layer making a choice that belongs to
    /// a viewpoint.
    ///
    /// Sorted by term, then by identifier, so the output is stable.
    #[must_use]
    pub fn terms(&self) -> Vec<(String, Cid)> {
        let mut out = Vec::new();
        for cid_text in self.cids() {
            let Ok(cid) = cid_text.parse::<Cid>() else {
                continue;
            };
            let Some(claim) = self.prose_claim(&cid) else {
                continue;
            };
            let Some(object) = self.object(&cid) else {
                continue;
            };
            if let Some(term) = claim.term(object) {
                out.push((term, cid));
            }
        }
        out.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
        });
        out
    }

    /// Short citation tags filed against an object (Section 9.1).
    ///
    /// Anyone MAY tag any object (R6), so this can return more than one
    /// string; which a reader sees is resolved per viewpoint, never by
    /// precedence. Returned sorted and deduplicated -- two keys filing the
    /// same tag string is agreement, not two tags. A tag felicitously
    /// retracted (Section 6) is excluded, the same way `usage_of` and
    /// `subjects_of` exclude theirs -- see [`Self::is_retracted`].
    #[must_use]
    pub fn tags_of(&self, target: &Cid) -> Vec<String> {
        let mut out = Vec::new();
        for cid_text in self.cids() {
            let Ok(cid) = cid_text.parse::<Cid>() else {
                continue;
            };
            let Some(object) = self.object(&cid) else {
                continue;
            };
            if object.kind() != "claim.annotation" {
                continue;
            }
            let body = object.body();
            if body.get("kind").and_then(Value::as_text) != Some("tagged")
                || body.get("target").and_then(Value::as_text) != Some(&target.to_string())
            {
                continue;
            }
            if self.is_retracted(&cid) {
                continue;
            }
            let Some(value) = body.get("value") else {
                continue;
            };
            let Some(tag) = value.get("tag").and_then(Value::as_text) else {
                continue;
            };
            out.push(tag.to_owned());
        }
        out.sort();
        out.dedup();
        out
    }

    /// Usage citations filed against a definitional claim (Section 5.2).
    ///
    /// Each is a source and a locator naming where the sense was found. The
    /// citation is a pointer rather than a quotation, which is what lets a
    /// reader check it without the corpus reproducing what it cites. A
    /// citation felicitously retracted (Section 6) is excluded --
    /// see [`Self::is_retracted`].
    #[must_use]
    pub fn usage_of(&self, target: &Cid) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for cid_text in self.cids() {
            let Ok(cid) = cid_text.parse::<Cid>() else {
                continue;
            };
            let Some(object) = self.object(&cid) else {
                continue;
            };
            if object.kind() != "claim.annotation" {
                continue;
            }
            let body = object.body();
            if body.get("kind").and_then(Value::as_text) != Some("usage")
                || body.get("target").and_then(Value::as_text) != Some(&target.to_string())
            {
                continue;
            }
            if self.is_retracted(&cid) {
                continue;
            }
            let Some(value) = body.get("value") else {
                continue;
            };
            let Some(source) = value.get("source").and_then(Value::as_text) else {
                continue;
            };
            let locator = value
                .get("locator")
                .and_then(Value::as_text)
                .unwrap_or("")
                .to_owned();
            out.push((source.to_owned(), locator));
        }
        out.sort();
        out.dedup();
        out
    }

    /// Subjects this object is classified under (Section 9.1).
    ///
    /// Membership is an annotation, never a property of the object (R6), so
    /// competing taxonomies coexist as sets of `classifies` annotations by
    /// different keys and a viewpoint selects among them. Returned sorted
    /// and deduplicated: two keys classifying one object under one subject
    /// is agreement, not two memberships. A membership felicitously
    /// retracted (Section 6) is excluded -- see [`Self::is_retracted`].
    #[must_use]
    pub fn subjects_of(&self, target: &Cid) -> Vec<String> {
        let mut out = self.classification(|t, _| t == &target.to_string(), |_, s| s.to_owned());
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Objects classified under a subject.
    #[must_use]
    pub fn classified_under(&self, subject: &Cid) -> Vec<String> {
        let mut out = self.classification(|_, s| s == &subject.to_string(), |t, _| t.to_owned());
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Walk `classifies` annotations, selecting and projecting.
    fn classification(
        &self,
        keep: impl Fn(&String, &String) -> bool,
        project: impl Fn(&str, &str) -> String,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for cid_text in self.cids() {
            let Ok(cid) = cid_text.parse::<Cid>() else {
                continue;
            };
            let Some(object) = self.object(&cid) else {
                continue;
            };
            if object.kind() != "claim.annotation" {
                continue;
            }
            let body = object.body();
            if body.get("kind").and_then(Value::as_text) != Some("classifies") {
                continue;
            }
            let Some(target) = body.get("target").and_then(Value::as_text) else {
                continue;
            };
            let Some(subject) = body
                .get("value")
                .and_then(|v| v.get("subject"))
                .and_then(Value::as_text)
            else {
                continue;
            };
            if self.is_retracted(&cid) {
                continue;
            }
            let (target, subject) = (target.to_owned(), subject.to_owned());
            if keep(&target, &subject) {
                out.push(project(&target, &subject));
            }
        }
        out
    }

    /// Critique annotations targeting a document (Section 8).
    #[must_use]
    pub fn critiques_of(&self, target: &Cid) -> Vec<&Annotation> {
        self.annotations
            .values()
            .filter(|a| a.kind() == "critique" && a.target() == target)
            .collect()
    }

    /// Borrow a typed claim.
    #[must_use]
    pub fn prose_claim(&self, cid: &Cid) -> Option<&ProseClaim> {
        self.prose_claims.get(&cid.to_string())
    }

    /// Identifiers reached from `cid` by outbound edges of `kind`.
    #[must_use]
    pub fn out(&self, kind: RelationKind, cid: &Cid) -> Vec<String> {
        self.out_edges
            .get(&kind)
            .and_then(|m| m.get(&cid.to_string()))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Identifiers reaching `cid` by inbound edges of `kind`.
    ///
    /// This is the reverse-edge query that content addressing alone cannot
    /// answer, and it is complete here precisely because the caller holds
    /// the whole graph.
    #[must_use]
    pub fn incoming(&self, kind: RelationKind, cid: &Cid) -> Vec<String> {
        self.in_edges
            .get(&kind)
            .and_then(|m| m.get(&cid.to_string()))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The lineage containing `cid` (Section 6.1).
    ///
    /// Walks outbound `supersedes` edges to the genesis, then collects
    /// everything reachable back along inbound edges.
    #[must_use]
    pub fn lineage(&self, cid: &Cid, mode: Lineage) -> LineageView {
        let start = cid.to_string();
        let genesis = self.genesis_of(&start, mode);
        let members =
            publet_algorithms::graph::reachable(&genesis, |current| self.successors(current, mode));

        let heads = members
            .iter()
            .filter(|m| self.successors(m, mode).is_empty())
            .cloned()
            .collect();

        LineageView {
            genesis: genesis.parse().unwrap_or_else(|_| cid.clone()),
            members: members
                .iter()
                .filter_map(|m| m.parse::<Cid>().ok())
                .collect(),
            heads: {
                let h: Vec<String> = heads;
                h.iter().filter_map(|m| m.parse::<Cid>().ok()).collect()
            },
            mode,
        }
    }

    fn genesis_of(&self, start: &str, mode: Lineage) -> String {
        // Acyclicity means at most one path upward matters; where an object
        // supersedes several, the last in identifier order is taken, so the
        // result is deterministic.
        publet_algorithms::graph::root(start, |current| self.ancestors(current, mode).pop())
    }

    fn ancestors(&self, cid: &str, mode: Lineage) -> Vec<String> {
        let all: Vec<String> = self
            .out_edges
            .get(&RelationKind::Supersedes)
            .and_then(|m| m.get(cid))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        match mode {
            Lineage::Full => all,
            Lineage::Authoritative => all
                .into_iter()
                .filter(|to| self.edge_is_authoritative(RelationKind::Supersedes, cid, to))
                .collect(),
        }
    }

    fn successors(&self, cid: &str, mode: Lineage) -> Vec<String> {
        let all: Vec<String> = self
            .in_edges
            .get(&RelationKind::Supersedes)
            .and_then(|m| m.get(cid))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        match mode {
            Lineage::Full => all,
            Lineage::Authoritative => all
                .into_iter()
                .filter(|from| self.edge_is_authoritative(RelationKind::Supersedes, from, cid))
                .collect(),
        }
    }

    /// The transitive `depends` closure of a claim (Section 5.4).
    ///
    /// Includes both the in-body `depends` field and any `depends` relation
    /// objects. The result excludes `cid` itself.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::CycleClosing`] if the closure reaches `cid`,
    /// which Section 5.4 forbids.
    pub fn depends_closure(&self, cid: &Cid) -> Result<Vec<Cid>, GraphError> {
        let start = cid.to_string();
        let reached = publet_algorithms::graph::closure(&start, |current| {
            let mut next: BTreeSet<String> = BTreeSet::new();
            if let Some(claim) = self.prose_claims.get(current) {
                next.extend(claim.depends().iter().map(ToString::to_string));
            }
            if let Some(edges) = self
                .out_edges
                .get(&RelationKind::Depends)
                .and_then(|m| m.get(current))
            {
                next.extend(edges.iter().cloned());
            }
            next
        })
        .map_err(|_| GraphError::CycleClosing {
            kind: "depends",
            from: start.clone(),
            to: start.clone(),
        })?;
        Ok(reached
            .iter()
            .filter_map(|c| c.parse::<Cid>().ok())
            .collect())
    }

    /// Connected components over `equivalent` edges whose signer passes
    /// `trusted` (Section 11.6).
    ///
    /// The predicate is supplied rather than assumed, because whose
    /// equivalence assertions count is a viewpoint question this layer does
    /// not answer.
    #[must_use]
    pub fn equivalence_class(&self, cid: &Cid, trusted: &dyn Fn(&Cid) -> bool) -> Vec<Cid> {
        let mut members = publet_algorithms::graph::reachable(&cid.to_string(), |current| {
            let mut next = Vec::new();
            for relation in self.relations.values() {
                if relation.kind() != RelationKind::Equivalent || !trusted(relation.author()) {
                    continue;
                }
                let (from, to) = (relation.from().to_string(), relation.to().to_string());
                if from == current {
                    next.push(to);
                } else if to == current {
                    next.push(from);
                }
            }
            next
        });
        // In identifier order, not the order the edges were walked in.
        members.sort();
        members
            .iter()
            .filter_map(|s| s.parse::<Cid>().ok())
            .collect()
    }
}
