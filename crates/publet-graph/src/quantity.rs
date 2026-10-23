//! Definitions of units, kinds and measures, and calculations' signatures
//! (Section 5.9).
//!
//! A number in a table means nothing without its unit, what it measures,
//! how sure its author is of it, and how it was computed. Each of those is
//! a claim that columns cite, so each can be disputed and revised on its
//! own lineage. This module reads the two body fields that make them
//! checkable: `defines` on a `definitional` claim and `computes` on a
//! `procedural` one. Whether a column's unit actually measures its kind is
//! a question for a units system, which this crate does not carry: it reads
//! shapes, and leaves the arithmetic to an adapter.

use std::collections::BTreeMap;

use publet_algorithms::h3::Cell;
use publet_core::{Cid, cbor::Value};

use crate::GraphError;

/// What a `definitional` claim's `defines` field states (Section 5.9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Definition {
    /// A system of units, at a version: UCUM 2.2.
    System {
        /// The system's name.
        name: String,
        /// The version meant.
        version: String,
    },
    /// A base dimension the system lacks, named by the symbol of its unit.
    Dimension {
        /// `{tray}`, `[EUR]`.
        symbol: String,
    },
    /// A unit equal to `value` times `unit`.
    Unit {
        /// The new unit's symbol.
        symbol: String,
        /// The multiplier, a decimal string.
        value: String,
        /// The unit multiplied, in the system's syntax.
        unit: String,
    },
    /// A quantity kind: what a value measures, beyond its dimension.
    Kind {
        /// The kind's name.
        name: String,
        /// An integer exponent per base, by base symbol.
        dimension: BTreeMap<String, i64>,
        /// The units that may measure it; empty when any of the dimension
        /// may.
        units: Vec<String>,
    },
    /// A way of stating a value and how sure one is of it.
    Measure {
        /// The measure's name.
        name: String,
        /// What its parameters describe.
        describes: Describes,
        /// The parameters, in order. A quantity cell has exactly these keys.
        parameters: Vec<Parameter>,
    },
    /// A concept at a PDC code: a subject, a quantity, which narrows a
    /// kind, or a relation between quantities.
    Concept {
        /// The PDC code it is defined at. This is its `term(D)`.
        pdc: String,
        /// The concept's name, for a reader.
        name: String,
        /// What it includes and excludes. Required of a quantity concept.
        boundary: Option<Boundary>,
        /// Which form of concept it is.
        form: ConceptForm,
    },
    /// One particular thing, rather than a class of them: a basement, a
    /// growing box, a soil sample, a company (Section 5.9.6).
    Singular {
        /// The thing's name, unique among the parts of what it is part of.
        name: String,
        /// The PDC code of the class it belongs to.
        class: String,
        /// The singular it is a part of.
        part_of: Option<Cid>,
        /// The H3 cell it is in.
        location: Option<Cell>,
    },
}

/// The three forms of concept (Section 5.9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConceptForm {
    /// A class of things or topics, such as a UDC class: neither a
    /// quantity nor a relation.
    Subject,
    /// An idea a group of measurements measure.
    Quantity {
        /// The kind definition it narrows.
        kind: Cid,
    },
    /// A relation between quantity slots, solvable for a slot.
    Relation(ConceptRelation),
}

/// A relation concept's relation (Section 5.9.1). Unit-free: a slot's unit
/// comes from the kind it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRelation {
    /// The relation in words, for a reader.
    pub statement: String,
    /// The quantities it relates, by role.
    pub slots: Vec<Slot>,
    /// How the slots relate.
    pub form: RelationForm,
}

/// One quantity a relation relates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The slot's role, an identifier unique within the relation.
    pub role: String,
    /// What fills it.
    pub binds: Binding,
}

/// What fills a slot: a quantity concept or a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// The PDC code of a quantity concept.
    Concept(String),
    /// The CID of a kind definition.
    Kind(Cid),
}

/// How a relation's slots relate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelationForm {
    /// `1 = coefficient * product(slot ^ exponent)`, solvable for any slot.
    Product {
        /// A pure number, as a decimal string.
        coefficient: String,
        /// Each slot's exponent, by role, as a decimal string.
        exponents: BTreeMap<String, String>,
    },
    /// Solved only by the calculations that implement it.
    Explicit,
}

/// The slot of a relation concept a calculation solves for (`solves`,
/// Section 5.9.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solves {
    /// The relation concept's PDC code.
    pub concept: String,
    /// The role solved for.
    pub target: String,
    /// The roles it needs.
    pub requires: Vec<String>,
    /// The roles it can do without, each with the claim stating the value
    /// it takes when absent.
    pub optional: Vec<(String, Cid)>,
}

impl Solves {
    /// Read a `solves` field.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::BadField`] if `concept` is not a PDC code, a
    /// role is not an identifier or repeats, or an optional input's
    /// `default` is not a CID.
    pub fn from_value(value: &Value) -> Result<Self, GraphError> {
        let Value::Map(_) = value else {
            return Err(bad("computes.solves", "a map"));
        };
        let target = role(value.get("target"), "computes.solves.target")?;
        let requires = match value.get("requires") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|r| role(Some(r), "computes.solves.requires"))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(bad("computes.solves.requires", "an array of roles")),
        };
        let optional = match value.get("optional") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|o| {
                    Ok((
                        role(o.get("role"), "computes.solves.optional.role")?,
                        cid(
                            o.get("default").unwrap_or(&Value::Null),
                            "computes.solves.optional.default",
                            "the CID of the claim stating the default",
                        )?,
                    ))
                })
                .collect::<Result<_, GraphError>>()?,
            Some(_) => {
                return Err(bad(
                    "computes.solves.optional",
                    "an array of {role, default}",
                ));
            }
        };
        let mut seen = std::collections::BTreeSet::from([target.as_str()]);
        for r in requires.iter().chain(optional.iter().map(|(r, _)| r)) {
            if !seen.insert(r.as_str()) {
                return Err(bad("computes.solves", "each role once"));
            }
        }
        Ok(Self {
            concept: required_text(value, "concept", "computes.solves.concept")?,
            target,
            requires,
            optional,
        })
    }

    /// Every role it names, the target first.
    pub fn roles(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.target.as_str())
            .chain(self.requires.iter().map(String::as_str))
            .chain(self.optional.iter().map(|(r, _)| r.as_str()))
    }
}

/// A concept's boundary: one short sentence per line, so a line can be
/// added or struck, and two versions compared line by line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Boundary {
    /// What the concept takes in.
    pub includes: Vec<String>,
    /// What it leaves out.
    pub excludes: Vec<String>,
}

impl Definition {
    /// The name the definition goes by: its `symbol` or `name`, or a
    /// concept's PDC code. This is `term(D)` for Section 9.2, except for a
    /// singular, which defines a thing rather than a term and has none
    /// (Section 5.9.6): its name is returned for a reader.
    #[must_use]
    pub fn term(&self) -> &str {
        match self {
            Self::System { name, .. }
            | Self::Kind { name, .. }
            | Self::Measure { name, .. }
            | Self::Singular { name, .. } => name,
            Self::Dimension { symbol } | Self::Unit { symbol, .. } => symbol,
            Self::Concept { pdc, .. } => pdc,
        }
    }

    /// The claims this definition cites, each with the field citing it: a
    /// quantity concept's kind, and the kinds a relation concept's slots
    /// bind. Each must also be in `depends` (Section 5.9.1).
    #[must_use]
    pub fn cited(&self) -> Vec<(&'static str, &Cid)> {
        match self {
            Self::Concept {
                form: ConceptForm::Quantity { kind },
                ..
            } => vec![("defines.kind", kind)],
            Self::Concept {
                form: ConceptForm::Relation(r),
                ..
            } => r
                .slots
                .iter()
                .filter_map(|s| match &s.binds {
                    Binding::Kind(kind) => Some(("defines.relation.slots.kind", kind)),
                    Binding::Concept(_) => None,
                })
                .collect(),
            Self::Singular {
                part_of: Some(whole),
                ..
            } => vec![("defines.part_of", whole)],
            _ => Vec::new(),
        }
    }

    /// The `sort` this definition has on the wire.
    #[must_use]
    pub fn sort(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::Dimension { .. } => "dimension",
            Self::Unit { .. } => "unit",
            Self::Kind { .. } => "kind",
            Self::Measure { .. } => "measure",
            Self::Concept { .. } => "concept",
            Self::Singular { .. } => "singular",
        }
    }

    /// Read a `defines` field.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::BadField`] if `sort` is not one of the seven,
    /// a field is missing or ill-typed, a symbol is not `{...}` or
    /// `[...]`, a value is not a decimal string, an exponent is not an
    /// integer, a measure has no parameters or repeats one, or a concept
    /// has no PDC code or a boundary line that is not a non-empty string,
    /// or a singular has no name or class, a `part_of` that is not a CID,
    /// or a `location` whose `h3` is not an H3 cell identifier.
    pub fn from_value(value: &Value) -> Result<Self, GraphError> {
        let Value::Map(_) = value else {
            return Err(bad("defines", "a map with a `sort`"));
        };
        let sort = value
            .get("sort")
            .and_then(Value::as_text)
            .ok_or(bad("defines.sort", "a text string"))?;
        match sort {
            "system" => Ok(Self::System {
                name: required_text(value, "name", "defines.name")?,
                version: required_text(value, "version", "defines.version")?,
            }),
            "dimension" => Ok(Self::Dimension {
                symbol: symbol(value)?,
            }),
            "unit" => {
                let multiplier = required_text(value, "value", "defines.value")?;
                if !is_decimal(&multiplier) {
                    return Err(bad("defines.value", "a decimal string"));
                }
                Ok(Self::Unit {
                    symbol: symbol(value)?,
                    value: multiplier,
                    unit: required_text(value, "unit", "defines.unit")?,
                })
            }
            "kind" => Ok(Self::Kind {
                name: required_text(value, "name", "defines.name")?,
                dimension: dimension(value.get("dimension"))?,
                units: match value.get("units") {
                    None => Vec::new(),
                    Some(Value::Array(items)) if !items.is_empty() => items
                        .iter()
                        .map(|u| {
                            u.as_text()
                                .filter(|t| !t.is_empty())
                                .map(ToOwned::to_owned)
                                .ok_or(bad("defines.units", "an array of units"))
                        })
                        .collect::<Result<_, _>>()?,
                    Some(_) => return Err(bad("defines.units", "a non-empty array of units")),
                },
            }),
            "measure" => Ok(Self::Measure {
                name: required_text(value, "name", "defines.name")?,
                describes: match value.get("describes").and_then(Value::as_text) {
                    Some("exact") => Describes::Exact,
                    Some("variation") => Describes::Variation,
                    Some("uncertainty") => Describes::Uncertainty,
                    _ => {
                        return Err(bad(
                            "defines.describes",
                            "`exact`, `variation`, or `uncertainty`",
                        ));
                    }
                },
                parameters: parameters(value.get("parameters"))?,
            }),
            "concept" => {
                let pdc = required_text(value, "pdc", "defines.pdc")?;
                let name = required_text(value, "name", "defines.name")?;
                let bounds = value
                    .get("boundary")
                    .map(|b| boundary(Some(b)))
                    .transpose()?;
                let form = match (value.get("kind"), value.get("relation")) {
                    (Some(kind), None) => ConceptForm::Quantity {
                        kind: cid(kind, "defines.kind", "the CID of a kind definition")?,
                    },
                    (None, Some(r)) => ConceptForm::Relation(relation(r)?),
                    (None, None) => ConceptForm::Subject,
                    (Some(_), Some(_)) => {
                        return Err(bad(
                            "defines",
                            "a concept with a `kind` or a `relation`, not both",
                        ));
                    }
                };
                if bounds.is_none() && matches!(form, ConceptForm::Quantity { .. }) {
                    return Err(bad("defines.boundary", "a boundary on a quantity concept"));
                }
                Ok(Self::Concept {
                    pdc,
                    name,
                    boundary: bounds,
                    form,
                })
            }
            "singular" => singular(value),
            _ => Err(bad(
                "defines.sort",
                "`system`, `dimension`, `unit`, `kind`, `measure`, `concept`, or `singular`",
            )),
        }
    }
}

/// What a measure's parameters describe (Section 5.9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Describes {
    /// A value stated as exact: a count, a price on a receipt.
    Exact,
    /// Variation in what was measured: repeated measurements.
    Variation,
    /// Uncertainty in what is known: estimates, intervals.
    Uncertainty,
}

/// One parameter of a measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// The key a quantity cell gives it: `low`, `sd`, `n`.
    pub name: String,
    /// What its value is.
    pub kind: ParameterType,
    /// What it means, for a reader.
    pub meaning: String,
}

/// The type of a parameter's value in a quantity cell (Section 5.9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParameterType {
    /// A decimal string in the column's unit.
    Quantity,
    /// A decimal string from 0 to 1.
    Fraction,
    /// An unsigned integer.
    Count,
    /// The CID of a blob.
    Blob,
}

/// What a calculation takes and gives (`computes`, Section 5.9.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Computes {
    /// Named inputs, each with the kind it must measure.
    pub inputs: Vec<(String, Cid)>,
    /// Named outputs, each with the kind it measures.
    pub outputs: Vec<(String, Cid)>,
    /// The slot of a relation concept this calculation solves for.
    pub solves: Option<Solves>,
    /// How a result that is not exact is rounded.
    pub rounding: Option<String>,
    /// A simulation's seed.
    pub seed: Option<u64>,
}

impl Computes {
    /// Read a `computes` field.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::BadField`] if `inputs` or `outputs` is not an
    /// array of `{name, kind}` with a CID for each kind, a name repeats
    /// within one of them, `rounding` or `seed` is ill-typed, or `solves`
    /// is not a relation concept's code, a target role, and input roles
    /// that are identifiers, distinct from each other and from the target,
    /// with each optional input's default a claim.
    pub fn from_value(value: &Value) -> Result<Self, GraphError> {
        let Value::Map(_) = value else {
            return Err(bad("computes", "a map"));
        };
        Ok(Self {
            inputs: signature(value.get("inputs"), "computes.inputs")?,
            outputs: signature(value.get("outputs"), "computes.outputs")?,
            solves: value.get("solves").map(Solves::from_value).transpose()?,
            rounding: match value.get("rounding") {
                None => None,
                Some(Value::Text(t)) => Some(t.clone()),
                Some(_) => return Err(bad("computes.rounding", "a text string")),
            },
            seed: match value.get("seed") {
                None => None,
                Some(Value::Uint(n)) => Some(*n),
                Some(_) => return Err(bad("computes.seed", "an unsigned integer")),
            },
        })
    }

    /// The claims stating the defaults of the optional inputs it solves
    /// with (Section 5.9.5).
    pub fn defaults(&self) -> impl Iterator<Item = &Cid> {
        self.solves
            .iter()
            .flat_map(|s| s.optional.iter().map(|(_, default)| default))
    }

    /// Every kind the calculation names, inputs first.
    pub fn kinds(&self) -> impl Iterator<Item = &Cid> {
        self.inputs
            .iter()
            .chain(&self.outputs)
            .map(|(_, kind)| kind)
    }
}

/// Whether `text` is a decimal string: an optional minus sign, digits, and
/// an optional fraction. No exponent, no leading `+`, no bare point.
pub(crate) fn is_decimal(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (unsigned, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// A singular's fields (Section 5.9.1).
fn singular(value: &Value) -> Result<Definition, GraphError> {
    Ok(Definition::Singular {
        name: required_text(value, "name", "defines.name")?,
        class: required_text(value, "class", "defines.class")?,
        part_of: value
            .get("part_of")
            .map(|v| cid(v, "defines.part_of", "the CID of a singular"))
            .transpose()?,
        location: value.get("location").map(location).transpose()?,
    })
}

/// A singular's `location`: `{ h3: <cell> }`, the cell in canonical form.
fn location(value: &Value) -> Result<Cell, GraphError> {
    let Value::Map(map) = value else {
        return Err(bad("defines.location", "a map with an `h3` cell"));
    };
    if map.keys().any(|k| k != "h3") {
        return Err(bad("defines.location", "a map with only an `h3` cell"));
    }
    value
        .get("h3")
        .and_then(Value::as_text)
        .and_then(|t| t.parse().ok())
        .ok_or(bad(
            "defines.location.h3",
            "an H3 cell identifier in lowercase hexadecimal, such as `8a089b1a2177fff`",
        ))
}

fn required_text(value: &Value, key: &str, field: &'static str) -> Result<String, GraphError> {
    value
        .get(key)
        .and_then(Value::as_text)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(bad(field, "a non-empty text string"))
}

fn symbol(value: &Value) -> Result<String, GraphError> {
    let s = required_text(value, "symbol", "defines.symbol")?;
    let wrapped = |open: char, close: char| {
        s.len() > 2
            && s.starts_with(open)
            && s.ends_with(close)
            && s.get(1..s.len() - 1)
                .is_some_and(|inner| !inner.contains(['{', '}', '[', ']']))
    };
    if wrapped('{', '}') || wrapped('[', ']') {
        Ok(s)
    } else {
        Err(bad(
            "defines.symbol",
            "a brace annotation such as `{tray}` or a bracketed atom such as `[EUR]`",
        ))
    }
}

fn dimension(value: Option<&Value>) -> Result<BTreeMap<String, i64>, GraphError> {
    let Some(Value::Map(map)) = value else {
        return Err(bad("defines.dimension", "a map from base to exponent"));
    };
    let mut out = BTreeMap::new();
    for (base, exponent) in map {
        let n = match exponent {
            Value::Uint(n) => i64::try_from(*n).ok(),
            // CBOR stores -1 - n.
            Value::Nint(n) => i64::try_from(*n).ok().map(|n| -1 - n),
            _ => None,
        }
        .ok_or(bad("defines.dimension", "integer exponents"))?;
        if base.is_empty() {
            return Err(bad("defines.dimension", "non-empty base symbols"));
        }
        if n != 0 {
            out.insert(base.clone(), n);
        }
    }
    Ok(out)
}

fn cid(value: &Value, field: &'static str, expected: &'static str) -> Result<Cid, GraphError> {
    value
        .as_text()
        .and_then(|t| t.parse().ok())
        .ok_or(bad(field, expected))
}

/// Whether `text` is an identifier a code generator can use: a letter or
/// underscore, then letters, digits and underscores.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn role(value: Option<&Value>, field: &'static str) -> Result<String, GraphError> {
    value
        .and_then(Value::as_text)
        .filter(|t| is_identifier(t))
        .map(ToOwned::to_owned)
        .ok_or(bad(
            field,
            "a role: letters, digits and underscores, not starting with a digit",
        ))
}

/// Whether a decimal string is zero: `0`, `-0.00`.
fn is_zero(decimal: &str) -> bool {
    decimal.bytes().all(|b| matches!(b, b'0' | b'.' | b'-'))
}

fn relation(value: &Value) -> Result<ConceptRelation, GraphError> {
    let Value::Map(_) = value else {
        return Err(bad(
            "defines.relation",
            "a map of `statement`, `slots` and `form`",
        ));
    };
    let Some(Value::Array(items)) = value.get("slots") else {
        return Err(bad("defines.relation.slots", "an array of slots"));
    };
    let mut slots: Vec<Slot> = Vec::with_capacity(items.len());
    for item in items {
        let role = role(item.get("role"), "defines.relation.slots.role")?;
        if slots.iter().any(|s| s.role == role) {
            return Err(bad(
                "defines.relation.slots.role",
                "unique within the relation",
            ));
        }
        let binds = match (item.get("concept"), item.get("kind")) {
            (Some(c), None) => Binding::Concept(
                c.as_text()
                    .filter(|t| !t.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or(bad("defines.relation.slots.concept", "a PDC code"))?,
            ),
            (None, Some(k)) => Binding::Kind(cid(
                k,
                "defines.relation.slots.kind",
                "the CID of a kind definition",
            )?),
            _ => {
                return Err(bad(
                    "defines.relation.slots",
                    "slots naming a `concept` or a `kind`, not both",
                ));
            }
        };
        slots.push(Slot { role, binds });
    }
    if slots.len() < 2 {
        return Err(bad("defines.relation.slots", "at least two slots"));
    }
    let form = match value.get("form") {
        Some(Value::Text(t)) if t == "explicit" => RelationForm::Explicit,
        Some(Value::Map(_)) => product(value.get("form").and_then(|f| f.get("product")), &slots)?,
        _ => return Err(bad("defines.relation.form", "`explicit` or a `product`")),
    };
    Ok(ConceptRelation {
        statement: required_text(value, "statement", "defines.relation.statement")?,
        slots,
        form,
    })
}

fn product(value: Option<&Value>, slots: &[Slot]) -> Result<RelationForm, GraphError> {
    const FIELD: &str = "defines.relation.form.product";
    let Some(product @ Value::Map(_)) = value else {
        return Err(bad("defines.relation.form", "`explicit` or a `product`"));
    };
    let coefficient = required_text(product, "coefficient", FIELD)?;
    if !is_decimal(&coefficient) || is_zero(&coefficient) {
        return Err(bad(FIELD, "a non-zero decimal `coefficient`"));
    }
    let Some(Value::Map(raw)) = product.get("exponents") else {
        return Err(bad(FIELD, "a map of `exponents` by role"));
    };
    let mut exponents = BTreeMap::new();
    for (role, exponent) in raw {
        let e = match exponent {
            Value::Uint(n) => n.to_string(),
            // CBOR stores -1 - n.
            Value::Nint(n) => format!("-{}", u128::from(*n) + 1),
            Value::Text(t) if is_decimal(t) => t.clone(),
            _ => return Err(bad(FIELD, "integer or decimal-string exponents")),
        };
        if is_zero(&e) {
            return Err(bad(FIELD, "non-zero exponents"));
        }
        exponents.insert(role.clone(), e);
    }
    if exponents.len() != slots.len() || slots.iter().any(|s| !exponents.contains_key(&s.role)) {
        return Err(bad(FIELD, "an exponent for every slot, and only for slots"));
    }
    Ok(RelationForm::Product {
        coefficient,
        exponents,
    })
}

fn boundary(value: Option<&Value>) -> Result<Boundary, GraphError> {
    let Some(Value::Map(_)) = value else {
        return Err(bad(
            "defines.boundary",
            "a map of `includes` and `excludes`",
        ));
    };
    let lines = |key: &str| -> Result<Vec<String>, GraphError> {
        let Some(Value::Array(items)) = value.and_then(|v| v.get(key)) else {
            return Err(bad("defines.boundary", "`includes` and `excludes` arrays"));
        };
        items
            .iter()
            .map(|line| {
                line.as_text()
                    .filter(|t| !t.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or(bad("defines.boundary", "arrays of non-empty strings"))
            })
            .collect()
    };
    Ok(Boundary {
        includes: lines("includes")?,
        excludes: lines("excludes")?,
    })
}

fn parameters(value: Option<&Value>) -> Result<Vec<Parameter>, GraphError> {
    let Some(Value::Array(items)) = value else {
        return Err(bad(
            "defines.parameters",
            "an array of {name, type, meaning}",
        ));
    };
    let mut out: Vec<Parameter> = Vec::with_capacity(items.len());
    for item in items {
        let name = required_text(item, "name", "defines.parameters.name")?;
        if out.iter().any(|p| p.name == name) {
            return Err(bad("defines.parameters.name", "unique within the measure"));
        }
        let kind = match item.get("type").and_then(Value::as_text) {
            Some("quantity") => ParameterType::Quantity,
            Some("fraction") => ParameterType::Fraction,
            Some("count") => ParameterType::Count,
            Some("blob") => ParameterType::Blob,
            _ => {
                return Err(bad(
                    "defines.parameters.type",
                    "`quantity`, `fraction`, `count`, or `blob`",
                ));
            }
        };
        out.push(Parameter {
            name,
            kind,
            meaning: required_text(item, "meaning", "defines.parameters.meaning")?,
        });
    }
    if out.is_empty() {
        return Err(bad("defines.parameters", "at least one parameter"));
    }
    Ok(out)
}

fn signature(value: Option<&Value>, field: &'static str) -> Result<Vec<(String, Cid)>, GraphError> {
    let items = match value {
        None => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(bad(field, "an array of {name, kind}")),
    };
    let mut out: Vec<(String, Cid)> = Vec::with_capacity(items.len());
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_text)
            .filter(|n| !n.is_empty())
            .ok_or(bad(field, "entries with a non-empty `name`"))?;
        if out.iter().any(|(n, _)| n == name) {
            return Err(bad(field, "unique names"));
        }
        let kind = item
            .get("kind")
            .and_then(Value::as_text)
            .and_then(|t| t.parse::<Cid>().ok())
            .ok_or(bad(
                field,
                "entries whose `kind` is the CID of a kind claim",
            ))?;
        out.push((name.to_owned(), kind));
    }
    Ok(out)
}

fn bad(field: &'static str, expected: &'static str) -> GraphError {
    GraphError::BadField { field, expected }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND: &str = "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra";

    fn map(pairs: Vec<(&str, Value)>) -> Value {
        Value::Map(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    #[test]
    fn a_dimension_is_named_by_a_braced_or_bracketed_symbol() {
        for good in ["{tray}", "[EUR]"] {
            let d = Definition::from_value(&map(vec![
                ("sort", text("dimension")),
                ("symbol", text(good)),
            ]))
            .unwrap();
            assert_eq!(d.term(), good);
        }
        for bad in ["tray", "{}", "{a}{b}", "kg"] {
            assert!(
                Definition::from_value(&map(vec![
                    ("sort", text("dimension")),
                    ("symbol", text(bad)),
                ]))
                .is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_kind_reads_signed_exponents() {
        let d = Definition::from_value(&map(vec![
            ("sort", text("kind")),
            ("name", text("climate impact per tray")),
            (
                "dimension",
                map(vec![
                    ("g", Value::Uint(1)),
                    ("{CO2e}", Value::Uint(1)),
                    ("{tray}", Value::Nint(0)),
                ]),
            ),
            ("units", Value::Array(vec![text("kg{CO2e}/{tray}")])),
        ]))
        .unwrap();
        let Definition::Kind {
            dimension, units, ..
        } = d
        else {
            panic!("not a kind");
        };
        assert_eq!(dimension.get("{tray}"), Some(&-1));
        assert_eq!(units, ["kg{CO2e}/{tray}"]);
    }

    #[test]
    fn a_measure_names_typed_unique_parameters() {
        let param = |name: &str, kind: &str| {
            map(vec![
                ("name", text(name)),
                ("type", text(kind)),
                ("meaning", text("a bound")),
            ])
        };
        let measure = |params: Vec<Value>| {
            Definition::from_value(&map(vec![
                ("sort", text("measure")),
                ("name", text("three-point estimate")),
                ("describes", text("uncertainty")),
                ("parameters", Value::Array(params)),
            ]))
        };
        let d = measure(vec![
            param("low", "quantity"),
            param("likely", "quantity"),
            param("high", "quantity"),
        ])
        .unwrap();
        let Definition::Measure {
            parameters,
            describes,
            ..
        } = d
        else {
            panic!("not a measure");
        };
        assert_eq!(describes, Describes::Uncertainty);
        assert_eq!(parameters.len(), 3);
        assert!(measure(vec![param("low", "quantity"), param("low", "quantity")]).is_err());
        assert!(measure(vec![param("low", "float")]).is_err());
        assert!(measure(vec![]).is_err());
    }

    #[test]
    fn a_concept_goes_by_its_pdc_code_and_narrows_a_kind() {
        let lines = |items: &[&str]| Value::Array(items.iter().map(|t| text(t)).collect());
        let concept = |pairs: Vec<(&str, Value)>| {
            let mut all = vec![("sort", text("concept"))];
            all.extend(pairs);
            Definition::from_value(&map(all))
        };
        let bounds = || {
            map(vec![
                ("includes", lines(&["Indoor shelves lit only by LEDs"])),
                ("excludes", lines(&["Daylight greenhouses"])),
            ])
        };
        let d = concept(vec![
            ("pdc", text("631.544")),
            ("name", text("LED shelf electricity per area-day")),
            ("kind", text(KIND)),
            ("boundary", bounds()),
        ])
        .unwrap();
        assert_eq!(d.term(), "631.544");
        assert_eq!(d.sort(), "concept");
        let Definition::Concept {
            form: ConceptForm::Quantity { kind },
            boundary: Some(boundary),
            ..
        } = d
        else {
            panic!("not a quantity concept");
        };
        assert_eq!(kind.to_string(), KIND);
        assert_eq!(boundary.excludes, ["Daylight greenhouses"]);
        // A quantity concept states its boundary.
        assert!(
            concept(vec![
                ("pdc", text("631.544")),
                ("name", text("x")),
                ("kind", text(KIND)),
            ])
            .is_err()
        );
        // The PDC code is required, the kind is a CID, and each boundary
        // line is a non-empty string.
        assert!(
            concept(vec![
                ("name", text("x")),
                ("kind", text(KIND)),
                ("boundary", bounds())
            ])
            .is_err()
        );
        assert!(
            concept(vec![
                ("pdc", text("631.544")),
                ("name", text("x")),
                ("kind", text("energy")),
                ("boundary", bounds()),
            ])
            .is_err()
        );
        assert!(
            concept(vec![
                ("pdc", text("631.544")),
                ("name", text("x")),
                ("kind", text(KIND)),
                (
                    "boundary",
                    map(vec![("includes", lines(&[""])), ("excludes", lines(&[]))])
                ),
            ])
            .is_err()
        );
    }

    #[test]
    fn a_unit_is_a_decimal_multiple() {
        let unit = |value: &str| {
            Definition::from_value(&map(vec![
                ("sort", text("unit")),
                ("symbol", text("{flat}")),
                ("value", text(value)),
                ("unit", text("{tray}")),
            ]))
        };
        assert!(unit("6").is_ok());
        assert!(unit("0.125").is_ok());
        for bad in ["6e1", "+6", ".5", "5.", "six"] {
            assert!(unit(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_unknown_sort_is_refused() {
        assert!(Definition::from_value(&map(vec![("sort", text("currency"))])).is_err());
        assert!(Definition::from_value(&text("unit")).is_err());
    }

    #[test]
    fn a_calculation_names_its_kinds() {
        let entry = |name: &str| map(vec![("name", text(name)), ("kind", text(KIND))]);
        let c = Computes::from_value(&map(vec![
            (
                "inputs",
                Value::Array(vec![entry("energy"), entry("intensity")]),
            ),
            ("outputs", Value::Array(vec![entry("climate cost")])),
            ("rounding", text("half to even, 2 places")),
        ]))
        .unwrap();
        assert_eq!(c.kinds().count(), 3);
        assert_eq!(c.seed, None);
        assert!(
            Computes::from_value(&map(vec![(
                "inputs",
                Value::Array(vec![entry("energy"), entry("energy")])
            )]))
            .is_err()
        );
    }

    #[test]
    fn a_subject_concept_names_neither_a_kind_nor_a_relation() {
        let d = Definition::from_value(&map(vec![
            ("sort", text("concept")),
            ("pdc", text("631")),
            ("name", text("Agriculture")),
        ]))
        .unwrap();
        assert_eq!(d.term(), "631");
        assert!(d.cited().is_empty());
        assert!(matches!(
            d,
            Definition::Concept {
                form: ConceptForm::Subject,
                boundary: None,
                ..
            }
        ));
    }

    fn relation(slots: Vec<Value>, form: Value) -> Result<Definition, GraphError> {
        Definition::from_value(&map(vec![
            ("sort", text("concept")),
            ("pdc", text("631.544.9")),
            ("name", text("light use efficiency")),
            (
                "relation",
                map(vec![
                    ("statement", text("fresh mass is light times efficiency")),
                    ("slots", Value::Array(slots)),
                    ("form", form),
                ]),
            ),
        ]))
    }

    fn slot(role: &str, by: &str, what: &str) -> Value {
        map(vec![("role", text(role)), (by, text(what))])
    }

    fn product(coefficient: &str, exponents: Vec<(&str, Value)>) -> Value {
        map(vec![(
            "product",
            map(vec![
                ("coefficient", text(coefficient)),
                ("exponents", map(exponents)),
            ]),
        )])
    }

    #[test]
    fn a_relation_concept_binds_slots_by_role() {
        let slots = || {
            vec![
                slot("fresh_mass", "concept", "631.544.1"),
                slot("light", "kind", KIND),
                slot("efficiency", "concept", "631.544.2"),
            ]
        };
        let d = relation(
            slots(),
            product(
                "1",
                vec![
                    ("fresh_mass", Value::Uint(1)),
                    ("light", Value::Nint(0)),
                    ("efficiency", text("-1")),
                ],
            ),
        )
        .unwrap();
        assert_eq!(d.term(), "631.544.9");
        let Definition::Concept {
            form: ConceptForm::Relation(r),
            boundary: None,
            ..
        } = d
        else {
            panic!("not a relation concept");
        };
        assert_eq!(
            r.slots.get(1).map(|s| &s.binds),
            Some(&Binding::Kind(KIND.parse().unwrap()))
        );
        let RelationForm::Product { exponents, .. } = r.form else {
            panic!("not a product");
        };
        assert_eq!(exponents.get("light").map(String::as_str), Some("-1"));
        assert!(relation(slots(), text("explicit")).is_ok());
        // Every slot has a non-zero exponent, and only slots do.
        let partial = product("1", vec![("fresh_mass", Value::Uint(1))]);
        assert!(relation(slots(), partial).is_err());
        let zero = product(
            "1",
            vec![
                ("fresh_mass", Value::Uint(1)),
                ("light", Value::Uint(0)),
                ("efficiency", Value::Uint(1)),
            ],
        );
        assert!(relation(slots(), zero).is_err());
        // Roles are identifiers, unique, and bind a concept or a kind.
        for bad in [
            vec![
                slot("fresh mass", "kind", KIND),
                slot("light", "kind", KIND),
            ],
            vec![slot("light", "kind", KIND), slot("light", "kind", KIND)],
            vec![slot("light", "kind", KIND)],
            vec![slot("light", "kind", KIND), map(vec![("role", text("x"))])],
        ] {
            assert!(relation(bad, text("explicit")).is_err());
        }
        // A concept is a quantity or a relation, never both.
        assert!(
            Definition::from_value(&map(vec![
                ("sort", text("concept")),
                ("pdc", text("1")),
                ("name", text("x")),
                ("kind", text(KIND)),
                (
                    "relation",
                    map(vec![
                        ("statement", text("x")),
                        ("slots", Value::Array(slots())),
                        ("form", text("explicit")),
                    ]),
                ),
            ]))
            .is_err()
        );
    }

    #[test]
    fn a_calculation_solves_one_slot_and_states_its_defaults() {
        let solves =
            |pairs: Vec<(&str, Value)>| Computes::from_value(&map(vec![("solves", map(pairs))]));
        let roles = |names: &[&str]| Value::Array(names.iter().map(|n| text(n)).collect());
        let c = solves(vec![
            ("concept", text("631.544.9")),
            ("target", text("fresh_mass")),
            ("requires", roles(&["light"])),
            (
                "optional",
                Value::Array(vec![map(vec![
                    ("role", text("efficiency")),
                    ("default", text(KIND)),
                ])]),
            ),
        ])
        .unwrap();
        let s = c.solves.unwrap();
        assert_eq!(
            s.roles().collect::<Vec<_>>(),
            ["fresh_mass", "light", "efficiency"]
        );
        // An optional input states its default by a claim.
        assert!(
            solves(vec![
                ("concept", text("631.544.9")),
                ("target", text("fresh_mass")),
                (
                    "optional",
                    Value::Array(vec![map(vec![("role", text("efficiency"))])]),
                ),
            ])
            .is_err()
        );
        // The target is not also an input.
        assert!(
            solves(vec![
                ("concept", text("631.544.9")),
                ("target", text("light")),
                ("requires", roles(&["light"])),
            ])
            .is_err()
        );
    }
}
