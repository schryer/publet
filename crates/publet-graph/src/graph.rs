//! The object graph: indices, lineage, closures, and equivalence classes.
//!
//! The graph accepts only [`Verified`] objects, so nothing whose identity
//! was never confirmed can be indexed. Edges of the acyclic kinds are
//! checked as they are added rather than in a later pass, which means a
//! cycle is reported against the edge that created it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use publet_core::{Cid, Object, Verified, cbor::Value};

use crate::GraphError;
use crate::document::{Anchor, Document};
use crate::view::{Annotation, Class, Publet, Relation, RelationKind};

/// Which `supersedes` edges a lineage query should follow (Section 6.1).
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
    publets: BTreeMap<String, Publet>,
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
            "publet" => {
                let publet = Publet::from_object(cid.clone(), &object)?;
                self.publets.insert(key.clone(), publet);
            }
            "rel" => {
                let relation = Relation::from_object(cid.clone(), &object)?;
                self.index_relation(&relation)?;
                self.relations.insert(key.clone(), relation);
            }
            "ann" => {
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
                    self.relations
                        .get(c)
                        .is_some_and(|r| r.author() == target.author())
                })
            })
    }

    /// Would adding `from -> to` create a cycle in this kind's edges?
    fn would_close_cycle(&self, kind: RelationKind, from: &str, to: &str) -> bool {
        if from == to {
            return true;
        }
        // A cycle forms exactly when `from` is already reachable from `to`.
        let Some(edges) = self.out_edges.get(&kind) else {
            return false;
        };
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = VecDeque::from([to]);
        while let Some(current) = queue.pop_front() {
            if current == from {
                return true;
            }
            if !seen.insert(current) {
                continue;
            }
            if let Some(next) = edges.get(current) {
                queue.extend(next.iter().map(String::as_str));
            }
        }
        false
    }

    /// Reject annotations the class rules of Section 5.2 forbid.
    fn check_annotation(&self, annotation: &Annotation) -> Result<(), GraphError> {
        if annotation.kind() != "verdict" {
            return Ok(());
        }
        let Some(target) = self.publets.get(&annotation.target().to_string()) else {
            // The target may arrive later; the rule is enforced again by
            // `validate` once the graph is complete.
            return Ok(());
        };
        Self::check_verdict(target.class(), annotation.aspect())
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
            if let Some(target) = self.publets.get(&annotation.target().to_string()) {
                Self::check_verdict(target.class(), annotation.aspect())?;
            }
        }
        for anchor in self.anchors.values() {
            anchor.check_against(self)?;
        }
        for relation in self.relations.values() {
            if relation.kind() == RelationKind::Disputes
                && !self.publets.contains_key(&relation.from().to_string())
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

    /// Documents whose citations substantially overlap another's without
    /// declaring `derived-from` (Section 8).
    ///
    /// Forking is permitted and cannot be prevented; what the protocol can
    /// do is make an undeclared one conspicuous. Overlap is a signal, not a
    /// verdict: two documents on one subject will cite the same publets,
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

    /// Every term a `definitional` publet in this graph fixes.
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
            let Some(publet) = self.publet(&cid) else {
                continue;
            };
            let Some(object) = self.object(&cid) else {
                continue;
            };
            if let Some(term) = publet.term(object) {
                out.push((term, cid));
            }
        }
        out.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
        });
        out
    }

    /// Usage citations filed against a definitional publet (Section 5.2).
    ///
    /// Each is a source and a locator naming where the sense was found. The
    /// citation is a pointer rather than a quotation, which is what lets a
    /// reader check it without the corpus reproducing what it cites.
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
            if object.kind() != "ann" {
                continue;
            }
            let body = object.body();
            if body.get("kind").and_then(Value::as_text) != Some("usage")
                || body.get("target").and_then(Value::as_text) != Some(&target.to_string())
            {
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
    /// is agreement, not two memberships.
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
            if object.kind() != "ann" {
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

    /// Borrow a typed publet.
    #[must_use]
    pub fn publet(&self, cid: &Cid) -> Option<&Publet> {
        self.publets.get(&cid.to_string())
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

        let mut members = Vec::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([genesis.clone()]);
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            members.push(current.clone());
            for next in self.successors(&current, mode) {
                queue.push_back(next);
            }
        }

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
        let mut current = start.to_owned();
        let mut guard = BTreeSet::new();
        loop {
            if !guard.insert(current.clone()) {
                return current;
            }
            let mut ancestors = self.ancestors(&current, mode);
            match ancestors.pop() {
                // Acyclicity means at most one path upward matters; where an
                // object supersedes several, the lowest identifier is taken
                // so the result is deterministic.
                Some(next) => current = next,
                None => return current,
            }
        }
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

    /// The transitive `depends` closure of a publet (Section 5.4).
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
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([start.clone()]);
        let mut first = true;

        while let Some(current) = queue.pop_front() {
            if current == start && !first {
                return Err(GraphError::CycleClosing {
                    kind: "depends",
                    from: start.clone(),
                    to: start,
                });
            }
            first = false;
            if !seen.insert(current.clone()) {
                continue;
            }
            if current != start
                && let Ok(parsed) = current.parse::<Cid>()
            {
                out.push(parsed);
            }
            let mut next: BTreeSet<String> = BTreeSet::new();
            if let Some(publet) = self.publets.get(&current) {
                next.extend(publet.depends().iter().map(ToString::to_string));
            }
            if let Some(edges) = self
                .out_edges
                .get(&RelationKind::Depends)
                .and_then(|m| m.get(&current))
            {
                next.extend(edges.iter().cloned());
            }
            queue.extend(next);
        }
        Ok(out)
    }

    /// Connected components over `equivalent` edges whose signer passes
    /// `trusted` (Section 11.6).
    ///
    /// The predicate is supplied rather than assumed, because whose
    /// equivalence assertions count is a viewpoint question this layer does
    /// not answer.
    #[must_use]
    pub fn equivalence_class(&self, cid: &Cid, trusted: &dyn Fn(&Cid) -> bool) -> Vec<Cid> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([cid.to_string()]);
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            for relation in self.relations.values() {
                if relation.kind() != RelationKind::Equivalent || !trusted(relation.author()) {
                    continue;
                }
                let (from, to) = (relation.from().to_string(), relation.to().to_string());
                if from == current {
                    queue.push_back(to);
                } else if to == current {
                    queue.push_back(from);
                }
            }
        }
        seen.iter().filter_map(|s| s.parse::<Cid>().ok()).collect()
    }
}
