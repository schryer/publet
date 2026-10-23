//! Check a claim's quantities against the claims they cite (Section 5.9).
//!
//! `publet-graph` reads the shapes: a column's `unit_defs`, `kind` and
//! `measure`, quantity cells, and the `defines` of definitional claims. It
//! cannot say whether `kW.h/{tray}` measures "electrical energy per tray",
//! because that takes a units system. This crate is the adapter between
//! the two: it gathers the definitions in a claim's dependency closure,
//! hands them to [pubunit] as its definitions format, and checks each
//! column's unit, its kind, and its quantity cells' keys and values.
//!
//! pubunit is pinned to one release, and the system it implements is
//! named by that release: a claim whose `system` claim names another
//! system or version is not checked, and its units are reported as
//! unchecked rather than passed (Section 5.9.2).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use publet_core::{Cid, cbor::Value};
use publet_graph::{
    Binding, Cell, Column, ConceptForm, Data, Definition, Graph, ParameterType, ProseClaim,
};
use pubunit::{Registry, Term};
use serde_json::json;

/// Something found while checking a claim's quantities.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Problem {
    /// A cited claim is not in the graph, so nothing that depends on it
    /// can be checked (Section 5.4).
    Unavailable {
        /// The claim's identifier.
        cid: String,
    },
    /// The claim lists more than one `system` claim in `depends`.
    MoreThanOneSystem,
    /// The claim's system is not the one this checker implements.
    UnknownSystem {
        /// The system named.
        name: String,
        /// The version named.
        version: String,
    },
    /// A column has a unit but the claim names no system: the unit is
    /// undefined, and shown so.
    Undefined {
        /// The column.
        column: String,
    },
    /// The definitions in the closure contradict each other, such as two
    /// claims declaring one symbol.
    Definitions {
        /// pubunit's reason.
        message: String,
    },
    /// A cited claim does not define what the column cites it for.
    WrongDefinition {
        /// The column.
        column: String,
        /// What it was cited as: `{tray}`, `kind`, `measure`.
        cited_as: String,
        /// The claim.
        cid: String,
    },
    /// A column's unit is not well formed, or names a symbol nothing
    /// declares.
    Unit {
        /// The column.
        column: String,
        /// pubunit's reason.
        message: String,
    },
    /// A column's unit uses a declared symbol its `unit_defs` does not map.
    NotInUnitDefs {
        /// The column.
        column: String,
        /// The symbol.
        symbol: String,
    },
    /// A column's unit does not measure its kind.
    Kind {
        /// The column.
        column: String,
        /// pubunit's reason.
        message: String,
    },
    /// A quantity cell's keys are not the measure's parameters.
    Keys {
        /// The column.
        column: String,
        /// The row, from 0.
        row: usize,
        /// The parameters the measure names.
        expected: Vec<String>,
        /// The keys the cell has.
        found: Vec<String>,
    },
    /// A quantity cell's value does not have its parameter's type.
    Value {
        /// The column.
        column: String,
        /// The row, from 0.
        row: usize,
        /// The parameter.
        parameter: String,
        /// What it must be.
        expected: &'static str,
    },
    /// A column named as one of its calculation's outputs names a
    /// different kind (Section 5.9.5).
    DerivedKind {
        /// The column.
        column: String,
    },
    /// A PDC code the claim names has no concept claim in its `depends`
    /// (Sections 5.9.1 and 5.9.5).
    ConceptNotInDepends {
        /// The code.
        code: String,
    },
    /// The concepts in `depends` at a code are not of the form the claim
    /// names it for.
    WrongConcept {
        /// The code.
        code: String,
        /// The form it must have: `quantity` or `relation`.
        expected: &'static str,
    },
    /// A claim cited as a singular, by a singular's `part_of` or a row's
    /// `about` cell, does not define one (Section 5.9.6).
    NotSingular {
        /// Where it was cited: `defines.part_of` or `about`.
        cited_as: &'static str,
        /// The claim.
        cid: String,
    },
    /// A calculation names a role its relation concept has no slot for.
    UnknownRole {
        /// The relation concept's code.
        code: String,
        /// The role.
        role: String,
    },
}

impl Problem {
    /// Whether Section 5.9 requires an implementation to reject the claim.
    /// The rest are reported: an unavailable or unknown definition leaves a
    /// value unchecked, never passed.
    #[must_use]
    pub fn rejects(&self) -> bool {
        !matches!(
            self,
            Self::Unavailable { .. }
                | Self::UnknownSystem { .. }
                | Self::Undefined { .. }
                | Self::DerivedKind { .. }
        )
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { cid } => {
                write!(f, "{cid} is not held, so what cites it is unchecked")
            }
            Self::MoreThanOneSystem => write!(
                f,
                "`depends` lists more than one units system (Section 5.9.2)"
            ),
            Self::UnknownSystem { name, version } => write!(
                f,
                "units are in {name} {version}, which this checker does not implement, so they are unchecked"
            ),
            Self::Undefined { column } => write!(
                f,
                "column `{column}`: its unit is undefined, since the claim names no units system (Section 5.9.2)"
            ),
            Self::Definitions { message } => write!(
                f,
                "the definitions this claim depends on conflict: {message}"
            ),
            Self::WrongDefinition {
                column,
                cited_as,
                cid,
            } => {
                write!(
                    f,
                    "column `{column}`: {cid}, cited as {cited_as}, does not define it"
                )
            }
            Self::Unit { column, message } | Self::Kind { column, message } => {
                write!(f, "column `{column}`: {message}")
            }
            Self::NotInUnitDefs { column, symbol } => write!(
                f,
                "column `{column}`: {symbol} is declared by a claim, so `unit_defs` must map it (Section 5.9.2)"
            ),
            Self::Keys {
                column,
                row,
                expected,
                found,
            } => write!(
                f,
                "column `{column}`, row {row}: a quantity has exactly its measure's parameters ({}), found ({}) (Section 5.9.4)",
                expected.join(", "),
                found.join(", ")
            ),
            Self::Value {
                column,
                row,
                parameter,
                expected,
            } => {
                write!(
                    f,
                    "column `{column}`, row {row}: `{parameter}` must be {expected}"
                )
            }
            Self::DerivedKind { column } => write!(
                f,
                "column `{column}` is an output of its calculation, which gives a different kind (Section 5.9.5)"
            ),
            Self::ConceptNotInDepends { code } => write!(
                f,
                "PDC code {code} is named, so `depends` must list a concept claim at it (Section 5.9.1)"
            ),
            Self::WrongConcept { code, expected } => write!(
                f,
                "PDC code {code} must name a {expected} concept, and the concept `depends` lists there is not one"
            ),
            Self::NotSingular { cited_as, cid } => write!(
                f,
                "{cid}, cited as {cited_as}, does not define a singular (Section 5.9.6)"
            ),
            Self::UnknownRole { code, role } => write!(
                f,
                "role `{role}` is not a slot of the relation concept at {code} (Section 5.9.5)"
            ),
        }
    }
}

/// The units system this checker implements: UCUM, at the version the
/// pinned pubunit reads.
#[must_use]
pub fn system() -> (&'static str, &'static str) {
    ("UCUM", Registry::ucum_source().ucum_version)
}

/// Check `claim`'s table against the claims its columns cite, as held in
/// `graph`. A claim without a table, or whose columns cite nothing and
/// have no units, has nothing to check.
#[must_use]
pub fn check(graph: &Graph, claim: &ProseClaim) -> Vec<Problem> {
    let mut problems = Vec::new();
    check_concepts(graph, claim, &mut problems);
    check_singulars(graph, claim, &mut problems);
    let Some(Data::Table { columns, rows }) = claim.data() else {
        return problems;
    };
    let defs = definitions(graph, claim, &mut problems);

    let systems: Vec<(&str, &str)> = claim
        .depends()
        .iter()
        .filter_map(|cid| match defs.get(&cid.to_string()) {
            Some(Definition::System { name, version }) => Some((name.as_str(), version.as_str())),
            _ => None,
        })
        .collect();
    let registry = match systems.as_slice() {
        [] => None,
        [one] if *one == system() => match registry(&defs) {
            Ok(r) => Some(r),
            Err(message) => {
                problems.push(Problem::Definitions { message });
                None
            }
        },
        [(name, version)] => {
            problems.push(Problem::UnknownSystem {
                name: (*name).to_owned(),
                version: (*version).to_owned(),
            });
            None
        }
        _ => {
            problems.push(Problem::MoreThanOneSystem);
            None
        }
    };
    let declared: BTreeSet<&str> = defs
        .values()
        .filter_map(|d| match d {
            Definition::Dimension { symbol } | Definition::Unit { symbol, .. } => {
                Some(symbol.as_str())
            }
            _ => None,
        })
        .collect();

    for (index, column) in columns.iter().enumerate() {
        if let Some(unit) = &column.unit {
            match &registry {
                Some(reg) => check_unit(reg, &defs, &declared, column, unit, &mut problems),
                None if systems.is_empty() => problems.push(Problem::Undefined {
                    column: column.name.clone(),
                }),
                None => {}
            }
        }
        if let Some(measure) = &column.measure {
            let cells = rows.iter().map(|r| r.get(index));
            check_cells(&defs, column, measure, cells, &mut problems);
        }
    }
    check_derivation(graph, claim, columns, &mut problems);
    problems
}

/// Check the PDC codes a concept's slots or a calculation's `solves` name
/// against the concept claims in `depends` (Sections 5.9.1 and 5.9.5).
fn check_concepts(graph: &Graph, claim: &ProseClaim, problems: &mut Vec<Problem>) {
    let mut named: Vec<(&str, &'static str)> = Vec::new();
    if let Some(Definition::Concept {
        form: ConceptForm::Relation(r),
        ..
    }) = claim.defines()
    {
        named.extend(r.slots.iter().filter_map(|s| match &s.binds {
            Binding::Concept(code) => Some((code.as_str(), "quantity")),
            Binding::Kind(_) => None,
        }));
    }
    if let Some(Definition::Singular { class, .. }) = claim.defines() {
        named.push((class.as_str(), "subject"));
    }
    if let Some(Data::Table { columns, .. }) = claim.data() {
        named.extend(
            columns
                .iter()
                .filter_map(|c| c.concept.as_deref().map(|code| (code, "quantity"))),
        );
    }
    let solves = claim.computes().and_then(|c| c.solves.as_ref());
    if let Some(s) = solves {
        named.push((s.concept.as_str(), "relation"));
    }
    if named.is_empty() {
        return;
    }
    let mut unheld = false;
    let mut held: Vec<(&str, &ConceptForm)> = Vec::new();
    for dep in claim.depends() {
        let Some(c) = graph.prose_claim(dep) else {
            unheld = true;
            problems.push(Problem::Unavailable {
                cid: dep.to_string(),
            });
            continue;
        };
        if let Some(Definition::Concept { pdc, form, .. }) = c.defines() {
            held.push((pdc.as_str(), form));
        }
    }
    for (code, expected) in named {
        let at: Vec<&ConceptForm> = held
            .iter()
            .filter(|(pdc, _)| *pdc == code)
            .map(|(_, f)| *f)
            .collect();
        if at.is_empty() {
            if !unheld {
                problems.push(Problem::ConceptNotInDepends {
                    code: code.to_owned(),
                });
            }
            continue;
        }
        let fits = |f: &&ConceptForm| match expected {
            "quantity" => matches!(f, ConceptForm::Quantity { .. }),
            "subject" => matches!(f, ConceptForm::Subject),
            _ => matches!(f, ConceptForm::Relation(_)),
        };
        if !at.iter().any(fits) {
            problems.push(Problem::WrongConcept {
                code: code.to_owned(),
                expected,
            });
            continue;
        }
        if let Some(s) = solves.filter(|s| s.concept == code) {
            let relations = at.iter().filter_map(|f| match f {
                ConceptForm::Relation(r) => Some(r),
                _ => None,
            });
            let missing = |r: &publet_graph::ConceptRelation| {
                s.roles()
                    .find(|role| !r.slots.iter().any(|slot| slot.role == *role))
                    .map(ToOwned::to_owned)
            };
            let all: Vec<Option<String>> = relations.map(missing).collect();
            if !all.iter().any(Option::is_none)
                && let Some(Some(role)) = all.into_iter().next()
            {
                problems.push(Problem::UnknownRole {
                    code: code.to_owned(),
                    role,
                });
            }
        }
    }
}

/// Check that what a singular is part of, and what each row is about, is a
/// singular (Section 5.9.6). Whether either is in `depends` is the
/// loader's to check; one not held is unchecked, never passed.
fn check_singulars(graph: &Graph, claim: &ProseClaim, problems: &mut Vec<Problem>) {
    let mut cited: Vec<(&'static str, Cid)> = Vec::new();
    if let Some(Definition::Singular {
        part_of: Some(whole),
        ..
    }) = claim.defines()
    {
        cited.push(("defines.part_of", whole.clone()));
    }
    if let Some(data) = claim.data() {
        cited.extend(data.about().into_iter().map(|(_, c)| ("about", c)));
    }
    for (cited_as, cid) in cited {
        match graph.prose_claim(&cid) {
            None => problems.push(Problem::Unavailable {
                cid: cid.to_string(),
            }),
            Some(c) if !matches!(c.defines(), Some(Definition::Singular { .. })) => {
                problems.push(Problem::NotSingular {
                    cited_as,
                    cid: cid.to_string(),
                });
            }
            Some(_) => {}
        }
    }
}

/// Every definition in the claim's dependency closure, by CID.
fn definitions(
    graph: &Graph,
    claim: &ProseClaim,
    problems: &mut Vec<Problem>,
) -> BTreeMap<String, Definition> {
    let mut closure: BTreeMap<String, Cid> = BTreeMap::new();
    for dep in claim.depends() {
        closure.insert(dep.to_string(), dep.clone());
        if let Ok(more) = graph.depends_closure(dep) {
            closure.extend(more.into_iter().map(|c| (c.to_string(), c)));
        }
    }
    let mut out = BTreeMap::new();
    for cid in closure.into_values() {
        match graph.prose_claim(&cid) {
            Some(c) => {
                if let Some(d) = c.defines() {
                    out.insert(cid.to_string(), d.clone());
                }
            }
            None => problems.push(Problem::Unavailable {
                cid: cid.to_string(),
            }),
        }
    }
    out
}

/// A pubunit registry declaring every dimension, unit and kind in `defs`.
///
/// Kinds are named by their claim's CID, so two kinds that share a name --
/// two generations of one lineage, or two parties' -- never collide; the
/// name a reader sees is the claim's own.
fn registry(defs: &BTreeMap<String, Definition>) -> Result<Registry, String> {
    let mut dimensions = Vec::new();
    let mut units = Vec::new();
    let mut kinds = Vec::new();
    for (cid, d) in defs {
        match d {
            Definition::Dimension { symbol } => {
                dimensions.push(json!({ "symbol": symbol, "name": symbol, "id": cid }));
            }
            Definition::Unit {
                symbol,
                value,
                unit,
            } => {
                units.push((symbol.clone(), json!({ "symbol": symbol, "name": symbol, "value": value, "unit": unit, "id": cid })));
            }
            Definition::Kind {
                dimension,
                units: allowed,
                ..
            } => {
                let mut kind = json!({ "name": cid, "dimension": dimension, "id": cid });
                if !allowed.is_empty()
                    && let Some(map) = kind.as_object_mut()
                {
                    map.insert("units".to_owned(), json!(allowed));
                }
                kinds.push(kind);
            }
            _ => {}
        }
    }
    let defs = pubunit::Definitions::from_json(
        &json!({ "dimensions": dimensions, "units": ordered(units), "kinds": kinds }).to_string(),
    )
    .map_err(|e| e.to_string())?;
    Registry::with_definitions(&defs).map_err(|e| e.to_string())
}

/// Units in an order where each comes after the units its own definition
/// uses, as pubunit requires. A unit naming one that never resolves is
/// left last, and pubunit says why.
fn ordered(mut pending: Vec<(String, serde_json::Value)>) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut placed: BTreeSet<String> = BTreeSet::new();
    while !pending.is_empty() {
        let symbols: BTreeSet<String> = pending.iter().map(|(s, _)| s.clone()).collect();
        let (ready, rest): (Vec<_>, Vec<_>) = pending.into_iter().partition(|(_, v)| {
            let unit = v
                .get("unit")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            symbols
                .iter()
                .all(|s| placed.contains(s) || !unit.contains(s.as_str()))
        });
        if ready.is_empty() {
            out.extend(rest.into_iter().map(|(_, v)| v));
            break;
        }
        for (s, v) in ready {
            placed.insert(s);
            out.push(v);
        }
        pending = rest;
    }
    out
}

fn check_unit(
    reg: &Registry,
    defs: &BTreeMap<String, Definition>,
    declared: &BTreeSet<&str>,
    column: &Column,
    unit: &str,
    problems: &mut Vec<Problem>,
) {
    let name = || column.name.clone();
    for (symbol, cid) in &column.unit_defs {
        match defs.get(&cid.to_string()) {
            Some(Definition::Dimension { symbol: s } | Definition::Unit { symbol: s, .. })
                if s == symbol => {}
            Some(_) => problems.push(Problem::WrongDefinition {
                column: name(),
                cited_as: symbol.clone(),
                cid: cid.to_string(),
            }),
            None => {}
        }
    }
    let term = match pubunit::parse(unit) {
        Ok(t) => t,
        Err(e) => {
            problems.push(Problem::Unit {
                column: name(),
                message: e.to_string(),
            });
            return;
        }
    };
    let mut used = BTreeSet::new();
    symbols(&term, &mut used);
    for symbol in used {
        if declared.contains(symbol.as_str()) && !column.unit_defs.contains_key(&symbol) {
            problems.push(Problem::NotInUnitDefs {
                column: name(),
                symbol,
            });
        }
    }
    if let Err(e) = reg.validate(unit) {
        problems.push(Problem::Unit {
            column: name(),
            message: e.to_string(),
        });
        return;
    }
    if let Some(kind) = &column.kind {
        match defs.get(&kind.to_string()) {
            Some(Definition::Kind {
                name: kind_name, ..
            }) => {
                if let Err(e) = reg.check_kind(unit, &kind.to_string()) {
                    problems.push(Problem::Kind {
                        column: name(),
                        message: e.to_string().replace(&kind.to_string(), kind_name),
                    });
                }
            }
            Some(_) => problems.push(Problem::WrongDefinition {
                column: name(),
                cited_as: "kind".to_owned(),
                cid: kind.to_string(),
            }),
            None => {}
        }
    }
}

/// The symbols a unit names that a claim might declare: bracketed atoms,
/// and annotations whether standing alone or after a unit.
fn symbols(term: &Term, out: &mut BTreeSet<String>) {
    match term {
        Term::Symbol {
            symbol, annotation, ..
        } => {
            if symbol.starts_with('[') {
                out.insert(symbol.clone());
            }
            if let Some(a) = annotation {
                out.insert(format!("{{{a}}}"));
            }
        }
        Term::Factor {
            annotation: Some(a),
            ..
        }
        | Term::Annotation(a) => {
            out.insert(format!("{{{a}}}"));
        }
        Term::Mul(a, b) | Term::Div(a, b) => {
            symbols(a, out);
            symbols(b, out);
        }
        Term::Recip(a) | Term::Group(a) => symbols(a, out),
        _ => {}
    }
}

fn check_cells<'a>(
    defs: &BTreeMap<String, Definition>,
    column: &Column,
    measure: &Cid,
    cells: impl Iterator<Item = Option<&'a Cell>>,
    problems: &mut Vec<Problem>,
) {
    let parameters = match defs.get(&measure.to_string()) {
        Some(Definition::Measure { parameters, .. }) => parameters,
        Some(_) => {
            problems.push(Problem::WrongDefinition {
                column: column.name.clone(),
                cited_as: "measure".to_owned(),
                cid: measure.to_string(),
            });
            return;
        }
        None => return,
    };
    let mut expected: Vec<String> = parameters.iter().map(|p| p.name.clone()).collect();
    expected.sort();
    for (row, cell) in cells.enumerate() {
        let Some(Cell::Quantity(params)) = cell else {
            continue;
        };
        let found: Vec<String> = params.keys().cloned().collect();
        if found != expected {
            problems.push(Problem::Keys {
                column: column.name.clone(),
                row,
                expected: expected.clone(),
                found,
            });
            continue;
        }
        for p in parameters {
            let ok = match (p.kind, params.get(&p.name)) {
                (ParameterType::Quantity, Some(Cell::Text(t))) => is_decimal(t),
                (ParameterType::Fraction, Some(Cell::Text(t))) => {
                    is_decimal(t) && !t.starts_with('-') && fraction_at_most_one(t)
                }
                (ParameterType::Count, Some(Cell::Uint(_))) => true,
                (ParameterType::Blob, Some(Cell::Text(t))) => t.parse::<Cid>().is_ok(),
                _ => false,
            };
            if !ok {
                problems.push(Problem::Value {
                    column: column.name.clone(),
                    row,
                    parameter: p.name.clone(),
                    expected: match p.kind {
                        ParameterType::Quantity => "a decimal string in the column's unit",
                        ParameterType::Fraction => "a decimal string from 0 to 1",
                        ParameterType::Count => "an unsigned integer",
                        _ => "the CID of a blob",
                    },
                });
            }
        }
    }
}

/// Section 5.9.5: a column named as an output of the claim's calculation
/// should measure the kind the calculation says it gives.
fn check_derivation(
    graph: &Graph,
    claim: &ProseClaim,
    columns: &[Column],
    problems: &mut Vec<Problem>,
) {
    let Some(Value::Array(evidence)) = graph
        .object(claim.cid())
        .and_then(|o| o.body().get("evidence"))
    else {
        return;
    };
    let derivation = evidence.iter().find_map(|e| {
        (e.get("role").and_then(Value::as_text) == Some("derivation"))
            .then(|| e.get("ref").and_then(Value::as_text)?.parse::<Cid>().ok())
            .flatten()
    });
    let Some(computes) = derivation
        .and_then(|d| graph.prose_claim(&d))
        .and_then(ProseClaim::computes)
    else {
        return;
    };
    for column in columns {
        let output = computes
            .outputs
            .iter()
            .find(|(name, _)| *name == column.name);
        if let Some((_, kind)) = output
            && column.kind.as_ref() != Some(kind)
        {
            problems.push(Problem::DerivedKind {
                column: column.name.clone(),
            });
        }
    }
}

/// A decimal string, as Section 4.1 has it: digits with an optional
/// minus sign and fraction, no exponent.
fn is_decimal(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |(w, f)| (w, Some(f)));
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

fn fraction_at_most_one(text: &str) -> bool {
    text.parse::<pubunit::Rational>()
        .is_ok_and(|r| r <= pubunit::Rational::one())
}
