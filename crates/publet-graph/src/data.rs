//! Data payloads, source evidence, and document views (Sections 4.7, 5.5,
//! 5.8, and 8).
//!
//! A claim about a set of values states what they are in `content` and
//! carries them in `data`. The split between the two is what lets the
//! values travel without losing the sentence that says what they mean, and
//! lets `content` stay one assertion the atomicity tests can read.
//!
//! Nothing here says how to show the values. That is a document's business
//! -- see [`View`] -- because one claim's data may be a full table in one
//! document and a chart in another, and neither rendering is an object.

use std::collections::BTreeMap;

use publet_core::{Cid, cbor::Value};

use crate::GraphError;

/// One column of a table: its name and, where its values count something,
/// the unit they count in, what they measure, and how sure they are.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Column {
    /// The column's name, non-empty and unique within its table.
    pub name: String,
    /// The unit every value in the column is in, if it has one.
    pub unit: Option<String>,
    /// The claims declaring the symbols in `unit` its system lacks, by
    /// symbol (Section 5.9.2).
    pub unit_defs: BTreeMap<String, Cid>,
    /// The quantity kind the values measure (Section 5.9.3).
    pub kind: Option<Cid>,
    /// The statistical measure its quantity cells follow (Section 5.9.4).
    pub measure: Option<Cid>,
    /// The PDC code of the quantity concept its values are of (Section
    /// 5.9.6), such as floor area.
    pub concept: Option<String>,
}

/// The name of the column whose cells say which singular each row is about
/// (Section 5.9.6).
pub const ABOUT: &str = "about";

impl Column {
    /// Every claim the column cites: its unit definitions, kind, and
    /// measure. Each must also be in the claim's `depends`.
    pub fn cited(&self) -> impl Iterator<Item = (&'static str, &Cid)> {
        self.unit_defs
            .values()
            .map(|c| ("data.columns.unit_defs", c))
            .chain(self.kind.iter().map(|c| ("data.columns.kind", c)))
            .chain(self.measure.iter().map(|c| ("data.columns.measure", c)))
    }
}

/// One cell of a table.
///
/// There is no float variant, for the reason Section 4.1 gives: a
/// non-integer quantity is a decimal string, and its unit is the column's.
/// A [`Cell::Quantity`] holds the parameters of its column's measure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Cell {
    /// Text, including decimal strings.
    Text(String),
    /// A non-negative integer.
    Uint(u64),
    /// A negative integer, stored as CBOR stores it: `-1 - n`.
    Nint(u64),
    /// No value.
    Null,
    /// A value with how sure its author is of it: a map from the column's
    /// measure's parameter names to text or integers (Section 5.9.4).
    Quantity(BTreeMap<String, Cell>),
}

/// A claim's `data` field (Section 5.8).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Data {
    /// Typed columns and rows of cells.
    Table {
        /// The columns, in order.
        columns: Vec<Column>,
        /// The rows, each exactly as wide as `columns`.
        rows: Vec<Vec<Cell>>,
    },
    /// A file, carried as a blob (Section 4.7).
    File {
        /// What the bytes are, as a media type.
        media: String,
        /// The blob's identifier.
        blob: Cid,
        /// How many bytes it has. A blob of any other length is refused.
        size: u64,
    },
}

impl Data {
    /// Read a `data` field.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::BadField`] if the value is neither shape, a
    /// column name is empty or repeated, a row's width differs from the
    /// number of columns, or a cell is not text, an integer, or null.
    pub fn from_value(value: &Value) -> Result<Self, GraphError> {
        let Value::Map(map) = value else {
            return Err(bad("data", "a map"));
        };
        if map.contains_key("columns") || map.contains_key("rows") {
            return table(map);
        }
        if map.contains_key("ref") {
            return file(map);
        }
        Err(bad(
            "data",
            "a table ({columns, rows}) or a file ({media, ref, size})",
        ))
    }

    /// The blob this data names, if it is a file.
    #[must_use]
    pub fn blob(&self) -> Option<(&Cid, u64)> {
        match self {
            Self::File { blob, size, .. } => Some((blob, *size)),
            Self::Table { .. } => None,
        }
    }

    /// Which singular each row is about, by row, for a table with an
    /// [`ABOUT`] column (Section 5.9.6). A null cell is about nothing named.
    #[must_use]
    pub fn about(&self) -> Vec<(usize, Cid)> {
        let Self::Table { columns, rows } = self else {
            return Vec::new();
        };
        let Some(index) = columns.iter().position(|c| c.name == ABOUT) else {
            return Vec::new();
        };
        rows.iter()
            .enumerate()
            .filter_map(|(r, row)| match row.get(index) {
                Some(Cell::Text(t)) => t.parse().ok().map(|cid| (r, cid)),
                _ => None,
            })
            .collect()
    }
}

fn table(map: &BTreeMap<String, Value>) -> Result<Data, GraphError> {
    let Some(Value::Array(raw_columns)) = map.get("columns") else {
        return Err(bad("data.columns", "an array of {name, unit}"));
    };
    let mut columns = Vec::with_capacity(raw_columns.len());
    for raw in raw_columns {
        let name = raw
            .get("name")
            .and_then(Value::as_text)
            .filter(|n| !n.is_empty())
            .ok_or(bad("data.columns.name", "a non-empty text string"))?;
        if columns.iter().any(|c: &Column| c.name == name) {
            return Err(bad(
                "data.columns.name",
                "unique within the table (Section 5.8)",
            ));
        }
        let unit = match raw.get("unit") {
            None | Some(Value::Null) => None,
            Some(Value::Text(u)) => Some(u.clone()),
            Some(_) => return Err(bad("data.columns.unit", "a text string")),
        };
        let unit_defs = match raw.get("unit_defs") {
            None => BTreeMap::new(),
            Some(Value::Map(defs)) if unit.is_some() => defs
                .iter()
                .map(|(symbol, cid)| {
                    cid.as_text()
                        .and_then(|t| t.parse::<Cid>().ok())
                        .map(|cid| (symbol.clone(), cid))
                        .ok_or(bad(
                            "data.columns.unit_defs",
                            "a map from symbol to the CID of its defining claim",
                        ))
                })
                .collect::<Result<_, _>>()?,
            Some(_) => {
                return Err(bad(
                    "data.columns.unit_defs",
                    "a map from symbol to CID, on a column with a `unit`",
                ));
            }
        };
        let column = Column {
            name: name.to_owned(),
            unit,
            unit_defs,
            kind: optional_cid(raw.get("kind"), "data.columns.kind")?,
            measure: optional_cid(raw.get("measure"), "data.columns.measure")?,
            concept: match raw.get("concept") {
                None => None,
                Some(Value::Text(code)) if !code.is_empty() => Some(code.clone()),
                Some(_) => return Err(bad("data.columns.concept", "a PDC code")),
            },
        };
        // Section 5.9.6: the about column names things, so it counts
        // nothing and is of no concept.
        if column.name == ABOUT
            && column
                != (Column {
                    name: ABOUT.to_owned(),
                    ..Column::default()
                })
        {
            return Err(bad(
                "data.columns",
                "an `about` column with no unit, kind, measure or concept (Section 5.9.6)",
            ));
        }
        columns.push(column);
    }
    if columns.is_empty() {
        return Err(bad("data.columns", "at least one column"));
    }

    let Some(Value::Array(raw_rows)) = map.get("rows") else {
        return Err(bad("data.rows", "an array of rows"));
    };
    let mut rows = Vec::with_capacity(raw_rows.len());
    for raw in raw_rows {
        let Value::Array(cells) = raw else {
            return Err(bad("data.rows", "an array of arrays"));
        };
        // A short row leaves a reader guessing which column is missing, and
        // a long one holds a value no column says the meaning of.
        if cells.len() != columns.len() {
            return Err(bad(
                "data.rows",
                "rows exactly as wide as the columns (Section 5.8)",
            ));
        }
        rows.push(
            cells
                .iter()
                .zip(&columns)
                .map(|(c, column)| {
                    if column.name == ABOUT {
                        about_cell(c)
                    } else {
                        cell(c, column.measure.is_some())
                    }
                })
                .collect::<Result<_, _>>()?,
        );
    }
    Ok(Data::Table { columns, rows })
}

/// A cell. In a column with a measure, every cell is a quantity or null
/// (Section 5.9.4); elsewhere, none is a quantity.
fn cell(value: &Value, measured: bool) -> Result<Cell, GraphError> {
    Ok(match (value, measured) {
        (Value::Null, _) => Cell::Null,
        (Value::Map(params), true) => Cell::Quantity(
            params
                .iter()
                .map(|(name, v)| {
                    let v = match v {
                        Value::Text(t) => Cell::Text(t.clone()),
                        Value::Uint(n) => Cell::Uint(*n),
                        _ => {
                            return Err(bad(
                                "data.rows",
                                "quantity parameters of text or unsigned integers",
                            ));
                        }
                    };
                    Ok((name.clone(), v))
                })
                .collect::<Result<_, _>>()?,
        ),
        (_, true) => {
            return Err(bad(
                "data.rows",
                "quantities or null in a column with a `measure` (Section 5.9.4)",
            ));
        }
        (Value::Text(t), false) => Cell::Text(t.clone()),
        (Value::Uint(n), false) => Cell::Uint(*n),
        (Value::Nint(n), false) => Cell::Nint(*n),
        (_, false) => {
            return Err(bad(
                "data.rows",
                "cells of text, integers, or null; a non-integer is a decimal string, \
                 and a quantity needs a column with a `measure`",
            ));
        }
    })
}

/// A cell of the about column: the CID of a singular, or null.
fn about_cell(value: &Value) -> Result<Cell, GraphError> {
    match value {
        Value::Null => Ok(Cell::Null),
        Value::Text(t) if t.parse::<Cid>().is_ok() => Ok(Cell::Text(t.clone())),
        _ => Err(bad(
            "data.rows",
            "the CID of a singular, or null, in the `about` column (Section 5.9.6)",
        )),
    }
}

fn optional_cid(value: Option<&Value>, field: &'static str) -> Result<Option<Cid>, GraphError> {
    match value {
        None => Ok(None),
        Some(v) => v
            .as_text()
            .and_then(|t| t.parse::<Cid>().ok())
            .map(Some)
            .ok_or(bad(field, "the CID of a definitional claim")),
    }
}

fn file(map: &BTreeMap<String, Value>) -> Result<Data, GraphError> {
    let media = map
        .get("media")
        .and_then(Value::as_text)
        .filter(|m| m.contains('/'))
        .ok_or(bad("data.media", "a media type such as `application/pdf`"))?;
    let blob = map
        .get("ref")
        .and_then(Value::as_text)
        .and_then(|t| t.parse::<Cid>().ok())
        .ok_or(bad("data.ref", "the CID of a blob"))?;
    let size = map
        .get("size")
        .and_then(Value::as_uint)
        .ok_or(bad("data.size", "the blob's length in bytes"))?;
    Ok(Data::File {
        media: media.to_owned(),
        blob,
        size,
    })
}

/// Where a claim's values were read from (Section 5.5, `role: "source"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// `external`, `blob`, or `claim`.
    pub kind: String,
    /// What was read: a repository, a table, a blob CID, or a claim CID.
    pub reference: String,
    /// The version read: a commit, a snapshot.
    pub revision: Option<String>,
    /// Where within `reference`: a path.
    pub locator: Option<String>,
    /// The query that selected the values.
    pub query: Option<String>,
}

/// The `source` entries of an evidence list, in order.
///
/// # Errors
///
/// Returns [`GraphError::BadField`] if a source entry lacks `kind` or
/// `ref`, or names a claim that is not a CID.
pub fn sources(evidence: Option<&Value>) -> Result<Vec<Source>, GraphError> {
    let Some(Value::Array(entries)) = evidence else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries {
        if entry.get("role").and_then(Value::as_text) != Some("source") {
            continue;
        }
        let kind = entry
            .get("kind")
            .and_then(Value::as_text)
            .ok_or(bad("evidence.kind", "`external`, `blob`, or `claim`"))?;
        let reference = entry
            .get("ref")
            .and_then(Value::as_text)
            .ok_or(bad("evidence.ref", "a CID or URI"))?;
        if matches!(kind, "claim" | "blob") && reference.parse::<Cid>().is_err() {
            return Err(bad("evidence.ref", "a CID, for a `claim` or `blob` source"));
        }
        let optional = |name: &str| {
            entry
                .get(name)
                .and_then(Value::as_text)
                .map(ToOwned::to_owned)
        };
        out.push(Source {
            kind: kind.to_owned(),
            reference: reference.to_owned(),
            revision: optional("revision"),
            locator: optional("locator"),
            query: optional("query"),
        });
    }
    Ok(out)
}

/// How a document shows the data an item cites (Section 8).
///
/// Presentational, like a gloss: it names a renderer and that renderer's
/// settings, and carries no claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The presentation, such as `table` or `bar-chart`.
    pub renderer: String,
    /// The renderer's settings.
    pub options: BTreeMap<String, Value>,
}

impl View {
    /// Read a document item's `view`.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::BadField`] if it is not a map naming a
    /// renderer, or if `options` is present and not a map.
    pub fn from_value(value: &Value) -> Result<Self, GraphError> {
        let renderer = value
            .get("renderer")
            .and_then(Value::as_text)
            .filter(|r| !r.is_empty())
            .ok_or(bad("items.view.renderer", "a non-empty text string"))?;
        let options = match value.get("options") {
            None => BTreeMap::new(),
            Some(Value::Map(m)) => m.clone(),
            Some(_) => return Err(bad("items.view.options", "a map")),
        };
        Ok(Self {
            renderer: renderer.to_owned(),
            options,
        })
    }
}

fn bad(field: &'static str, expected: &'static str) -> GraphError {
    GraphError::BadField { field, expected }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    const BLOB: &str = "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra";

    fn map(pairs: Vec<(&str, Value)>) -> Value {
        Value::Map(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn budget(rows: Vec<Value>) -> Value {
        map(vec![
            (
                "columns",
                Value::Array(vec![
                    map(vec![("name", text("item"))]),
                    map(vec![("name", text("spend")), ("unit", text("EUR"))]),
                ]),
            ),
            ("rows", Value::Array(rows)),
        ])
    }

    #[test]
    fn a_table_reads_its_columns_units_and_cells() {
        let data = Data::from_value(&budget(vec![Value::Array(vec![
            text("seed trays"),
            text("120.00"),
        ])]))
        .unwrap();
        let Data::Table { columns, rows } = data else {
            panic!("not a table");
        };
        assert_eq!(columns[0].unit, None);
        assert_eq!(columns[1].unit.as_deref(), Some("EUR"));
        assert_eq!(rows[0][1], Cell::Text("120.00".into()));
    }

    #[test]
    fn integers_and_null_are_cells() {
        let data = Data::from_value(&budget(vec![Value::Array(vec![
            Value::Null,
            Value::Uint(120),
        ])]));
        assert!(data.is_ok());
    }

    #[test]
    fn a_short_row_is_refused() {
        let err = Data::from_value(&budget(vec![Value::Array(vec![text("seed trays")])]));
        assert!(matches!(
            err,
            Err(GraphError::BadField {
                field: "data.rows",
                ..
            })
        ));
    }

    #[test]
    fn a_nested_cell_is_refused() {
        let err = Data::from_value(&budget(vec![Value::Array(vec![
            text("seed trays"),
            Value::Array(vec![]),
        ])]));
        assert!(err.is_err());
    }

    #[test]
    fn a_repeated_column_name_is_refused() {
        let value = map(vec![
            (
                "columns",
                Value::Array(vec![
                    map(vec![("name", text("spend"))]),
                    map(vec![("name", text("spend"))]),
                ]),
            ),
            ("rows", Value::Array(vec![])),
        ]);
        assert!(matches!(
            Data::from_value(&value),
            Err(GraphError::BadField {
                field: "data.columns.name",
                ..
            })
        ));
    }

    #[test]
    fn an_empty_column_name_is_refused() {
        let value = map(vec![
            ("columns", Value::Array(vec![map(vec![("name", text(""))])])),
            ("rows", Value::Array(vec![])),
        ]);
        assert!(Data::from_value(&value).is_err());
    }

    #[test]
    fn a_file_names_its_blob_media_and_size() {
        let value = map(vec![
            ("media", text("application/pdf")),
            ("ref", text(BLOB)),
            ("size", Value::Uint(2048)),
        ]);
        let data = Data::from_value(&value).unwrap();
        let (blob, size) = data.blob().unwrap();
        assert_eq!(blob.to_string(), BLOB);
        assert_eq!(size, 2048);
    }

    #[test]
    fn a_file_without_a_size_is_refused() {
        let value = map(vec![
            ("media", text("application/pdf")),
            ("ref", text(BLOB)),
        ]);
        assert!(Data::from_value(&value).is_err());
    }

    #[test]
    fn neither_shape_is_refused() {
        assert!(Data::from_value(&map(vec![("values", Value::Array(vec![]))])).is_err());
        assert!(Data::from_value(&text("a table")).is_err());
    }

    #[test]
    fn sources_are_the_source_entries_only() {
        let evidence = Value::Array(vec![
            map(vec![
                ("kind", text("claim")),
                ("role", text("method")),
                ("ref", text(BLOB)),
            ]),
            map(vec![
                ("kind", text("external")),
                ("role", text("source")),
                ("ref", text("https://github.com/schryer/business-plan")),
                ("revision", text("abc123")),
                ("locator", text("bizplan/plan/p0.py")),
            ]),
        ]);
        let found = sources(Some(&evidence)).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].revision.as_deref(), Some("abc123"));
        assert_eq!(found[0].locator.as_deref(), Some("bizplan/plan/p0.py"));
        assert_eq!(found[0].query, None);
    }

    #[test]
    fn a_claim_source_must_be_a_cid() {
        let evidence = Value::Array(vec![map(vec![
            ("kind", text("claim")),
            ("role", text("source")),
            ("ref", text("budget-p0")),
        ])]);
        assert!(sources(Some(&evidence)).is_err());
    }

    #[test]
    fn a_view_names_a_renderer_and_options() {
        let view = View::from_value(&map(vec![
            ("renderer", text("table")),
            ("options", map(vec![("totals", Value::Bool(true))])),
        ]))
        .unwrap();
        assert_eq!(view.renderer, "table");
        assert_eq!(view.options.get("totals"), Some(&Value::Bool(true)));
    }

    #[test]
    fn a_view_without_a_renderer_is_refused() {
        assert!(View::from_value(&map(vec![("options", map(vec![]))])).is_err());
    }
}
