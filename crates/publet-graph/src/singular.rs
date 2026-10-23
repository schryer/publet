//! Singulars, the facts about them, and the path keys that find them
//! (Section 5.9.6).
//!
//! A singular is one particular thing -- a basement, a growing box, a soil
//! sample, a company -- defined by a `definitional` claim with `defines.sort`
//! `singular`. A fact is a value in a table whose `about` column names the
//! singular and whose value column is of a quantity concept. A path key such
//! as `place.site.tallinn.basement.floor_area` is computed from the names in
//! those claims: the singular's class and the classes it narrows, the chain
//! of singulars it is part of, and the concept. Nothing is registered: two
//! readers holding the same claims compute the same paths.

use std::collections::BTreeSet;
use std::fmt;

use publet_algorithms::h3::Cell as H3Cell;
use publet_core::Cid;

use crate::{Cell, Column, ConceptForm, Data, Definition, Graph, Lineage, ProseClaim};

/// The most singulars a `part_of` chain may hold before it is refused.
/// Content addressing already makes a cycle impossible to construct; this
/// bounds the walk regardless.
const MAX_DEPTH: usize = 64;

/// Why a singular has no path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathError {
    /// The claim is not held, or does not define a singular.
    NotSingular {
        /// The claim.
        cid: String,
    },
    /// No concept at the class's code is in the singular's dependency
    /// closure, so the class has no name.
    NoClass {
        /// The class's PDC code.
        code: String,
    },
    /// More than one concept at one code is in the closure, each with a
    /// different name.
    ContestedClass {
        /// The PDC code.
        code: String,
    },
    /// The `part_of` chain is longer than any real one.
    TooDeep,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSingular { cid } => write!(f, "{cid} is not a singular held here"),
            Self::NoClass { code } => write!(
                f,
                "no concept at PDC code {code} is in the singular's dependency closure, so its class has no name (Section 5.9.6)"
            ),
            Self::ContestedClass { code } => write!(
                f,
                "the singular's dependency closure holds concepts of different names at PDC code {code}"
            ),
            Self::TooDeep => write!(f, "the `part_of` chain is longer than {MAX_DEPTH}"),
        }
    }
}

/// A path segment from a name: lowercase, with each run of characters that
/// are not letters or digits made one underscore, and none at either end.
/// `Floor area` is `floor_area`.
#[must_use]
pub fn segment(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut gap = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// One value about a singular: a cell in a column of a quantity concept, in
/// a row whose `about` cell names the singular.
#[derive(Debug, Clone)]
pub struct Fact<'g> {
    /// The claim stating it.
    pub claim: &'g ProseClaim,
    /// The singular the row is about.
    pub about: Cid,
    /// The row, from 0.
    pub row: usize,
    /// The value's column.
    pub column: &'g Column,
    /// The value.
    pub value: &'g Cell,
    /// The path segment of the column's concept.
    pub concept: String,
}

/// The fields of a singular's definition.
fn singular(graph: &Graph, cid: &Cid) -> Option<(String, String, Option<Cid>, Option<H3Cell>)> {
    match graph.prose_claim(cid)?.defines()? {
        Definition::Singular {
            name,
            class,
            part_of,
            location,
        } => Some((name.clone(), class.clone(), part_of.clone(), *location)),
        _ => None,
    }
}

/// The singular and every singular it is part of, the singular first.
fn chain(graph: &Graph, cid: &Cid) -> Result<Vec<Cid>, PathError> {
    let mut out = vec![cid.clone()];
    let mut current = cid.clone();
    loop {
        let Some((_, _, part_of, _)) = singular(graph, &current) else {
            return Err(PathError::NotSingular {
                cid: current.to_string(),
            });
        };
        let Some(whole) = part_of else {
            return Ok(out);
        };
        if out.len() >= MAX_DEPTH || out.contains(&whole) {
            return Err(PathError::TooDeep);
        }
        out.push(whole.clone());
        current = whole;
    }
}

/// The concepts in `claim`'s dependency closure, and the claim itself, by
/// PDC code, with their names.
fn concepts_in_closure(graph: &Graph, cid: &Cid) -> Vec<(String, String)> {
    let mut closure = graph.depends_closure(cid).unwrap_or_default();
    closure.push(cid.clone());
    closure
        .iter()
        .filter_map(|c| match graph.prose_claim(c)?.defines()? {
            Definition::Concept { pdc, name, .. } => Some((pdc.clone(), name.clone())),
            _ => None,
        })
        .collect()
}

/// The segment of the one name the concepts at `code` go by.
fn named(concepts: &[(String, String)], code: &str) -> Result<Option<String>, PathError> {
    let names: BTreeSet<String> = concepts
        .iter()
        .filter(|(pdc, _)| pdc == code)
        .map(|(_, name)| segment(name))
        .collect();
    let mut names = names.into_iter();
    match (names.next(), names.next()) {
        (None, _) => Ok(None),
        (Some(one), None) => Ok(Some(one)),
        (Some(_), Some(_)) => Err(PathError::ContestedClass {
            code: code.to_owned(),
        }),
    }
}

impl Graph {
    /// Every singular held, in identifier order.
    #[must_use]
    pub fn singulars(&self) -> Vec<&ProseClaim> {
        self.cids()
            .into_iter()
            .filter_map(|c| c.parse::<Cid>().ok())
            .filter_map(|c| self.prose_claim(&c))
            .filter(|c| matches!(c.defines(), Some(Definition::Singular { .. })))
            .collect()
    }

    /// The segments naming a singular's class: the names of the concepts at
    /// the class's code and at each broader code, broadest first, among the
    /// concepts in the outermost singular's dependency closure (Section
    /// 5.9.6). A code is broader than another when the other begins with it.
    ///
    /// # Errors
    ///
    /// Returns [`PathError`] if `cid` is not a singular, or the class code
    /// has no concept or a contested one in the closure.
    pub fn class_path(&self, cid: &Cid) -> Result<Vec<String>, PathError> {
        let chain = chain(self, cid)?;
        let root = chain.last().unwrap_or(cid);
        let Some((_, class, _, _)) = singular(self, root) else {
            return Err(PathError::NotSingular {
                cid: root.to_string(),
            });
        };
        let concepts = concepts_in_closure(self, root);
        if named(&concepts, &class)?.is_none() {
            return Err(PathError::NoClass { code: class });
        }
        let mut codes: Vec<&str> = concepts
            .iter()
            .map(|(pdc, _)| pdc.as_str())
            .filter(|pdc| class.starts_with(pdc))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        codes.sort_by_key(|c| c.len());
        codes
            .into_iter()
            .map(|code| named(&concepts, code).map(Option::unwrap_or_default))
            .collect()
    }

    /// A singular's path key without a concept: its class path, then the
    /// name of each singular from the outermost it is part of down to it
    /// (Section 5.9.6). `place.site.tallinn.basement`.
    ///
    /// # Errors
    ///
    /// As [`Graph::class_path`], and if a singular in the `part_of` chain is
    /// not held.
    pub fn singular_path(&self, cid: &Cid) -> Result<Vec<String>, PathError> {
        let mut path = self.class_path(cid)?;
        for part in chain(self, cid)?.iter().rev() {
            if let Some((name, ..)) = singular(self, part) {
                path.push(segment(&name));
            }
        }
        Ok(path)
    }

    /// Where a singular is for the spatial form of lookup: its own H3 cell,
    /// or else the cell of the nearest singular it is part of that has one.
    #[must_use]
    pub fn singular_location(&self, cid: &Cid) -> Option<H3Cell> {
        chain(self, cid)
            .ok()?
            .iter()
            .find_map(|c| singular(self, c).and_then(|(.., location)| location))
    }

    /// Every fact about any of `singulars`, from every claim held, in claim
    /// order: one per row about one of them and per column of a quantity
    /// concept that one of the claim's dependencies defines.
    #[must_use]
    pub fn facts_about(&self, singulars: &[Cid]) -> Vec<Fact<'_>> {
        let mut out = Vec::new();
        for cid in self.cids() {
            let Some(claim) = cid.parse::<Cid>().ok().and_then(|c| self.prose_claim(&c)) else {
                continue;
            };
            let Some(data @ Data::Table { columns, rows }) = claim.data() else {
                continue;
            };
            let about = data.about();
            if about.is_empty() {
                continue;
            }
            let concepts: Vec<(String, String)> = claim
                .depends()
                .iter()
                .filter_map(|d| match self.prose_claim(d)?.defines()? {
                    Definition::Concept {
                        pdc,
                        name,
                        form: ConceptForm::Quantity { .. },
                        ..
                    } => Some((pdc.clone(), name.clone())),
                    _ => None,
                })
                .collect();
            for (row, thing) in about {
                if !singulars.contains(&thing) {
                    continue;
                }
                for (index, column) in columns.iter().enumerate() {
                    let Some(code) = &column.concept else {
                        continue;
                    };
                    let Ok(Some(concept)) = named(&concepts, code) else {
                        continue;
                    };
                    let Some(value) = rows.get(row).and_then(|r| r.get(index)) else {
                        continue;
                    };
                    out.push(Fact {
                        claim,
                        about: thing.clone(),
                        row,
                        column,
                        value,
                        concept,
                    });
                }
            }
        }
        out
    }

    /// Every member of the lineages of `singulars`: a revised singular is
    /// the same thing, so facts about any version of it are about it.
    #[must_use]
    pub fn singular_versions(&self, singulars: &[Cid]) -> Vec<Cid> {
        let mut all: Vec<Cid> = Vec::new();
        for member in singulars
            .iter()
            .flat_map(|s| self.lineage(s, Lineage::Authoritative).members)
        {
            if !all.contains(&member) {
                all.push(member);
            }
        }
        all
    }

    /// The claims to read a lineage by: the anchored one, when every anchor
    /// held over the lineage recommends the same member (Section 9.3), or
    /// else the lineage's heads, of which more than one means it branched.
    #[must_use]
    pub fn lineage_heads(&self, claim: &Cid) -> Vec<Cid> {
        let view = self.lineage(claim, Lineage::Authoritative);
        let mut anchored: Vec<&Cid> = self
            .anchors()
            .filter(|a| a.lineage() == &view.genesis && view.members.contains(a.current()))
            .map(crate::Anchor::current)
            .collect();
        anchored.sort_by_key(ToString::to_string);
        anchored.dedup();
        match anchored.as_slice() {
            [one] => vec![(*one).clone()],
            _ => view.heads,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: segment
    #[test]
    fn a_segment_is_the_name_lowercased_with_runs_of_other_characters_one_underscore() {
        assert_eq!(segment("Floor area"), "floor_area");
        assert_eq!(segment("  Tallinn -- site (1) "), "tallinn_site_1");
        assert_eq!(segment("Kalamaja"), "kalamaja");
        assert_eq!(segment("Pärnu"), "pärnu");
        assert_eq!(segment("--"), "");
    }
}
