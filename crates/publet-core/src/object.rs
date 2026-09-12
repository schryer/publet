//! Objects and the verification type state (Sections 4.3 and 4.5).
//!
//! Parsing yields an [`Unverified<Object>`]. Only [`Verified<Object>`] is
//! accepted by the graph, evaluation, and store crates, and the sole way to
//! obtain one is [`Unverified::verify`], which checks the bytes against the
//! identifier the caller asked for. Indexing an object whose identity was
//! never confirmed is therefore not a mistake that can be made: no function
//! exists that would accept it.
//!
//! # Examples
//!
//! ```
//! use publet_core::{Cid, HashAlg, Object};
//!
//! const AUTHOR: &str =
//!     "pub:sha2-256:z7uu6enmjz5gfxa5jtqjx4kynsm3chepcvqtv5s5g7zfj4y2iwra";
//!
//! let bytes = Object::builder("publet", AUTHOR)
//!     .created("2026-09-12T10:00:00Z")
//!     .build()?;
//! let cid = Cid::of(&bytes, HashAlg::Sha2_256);
//!
//! let parsed = Object::parse(&bytes)?;
//! let verified = parsed.verify(&cid)?;
//! assert_eq!(verified.object().kind(), "publet");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::BTreeMap;

use thiserror::Error;

use crate::cbor::{self, Value};
use crate::{CanonError, Cid, CidError};

/// Reasons an object is not well formed.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum ObjectError {
    /// The bytes were not canonical CBOR.
    #[error(transparent)]
    Canonical(#[from] CanonError),

    /// The top-level item was not a map.
    #[error("an object must be a map, found {found}")]
    NotAMap {
        /// What the top-level item actually was.
        found: &'static str,
    },

    /// A required header field was absent.
    #[error("missing required field `{field}`")]
    MissingField {
        /// The absent field.
        field: &'static str,
    },

    /// A header field had the wrong type.
    #[error("field `{field}` must be {expected}")]
    WrongType {
        /// The offending field.
        field: &'static str,
        /// What the specification requires.
        expected: &'static str,
    },

    /// An unrecognized field appeared at the top level.
    #[error("unknown top-level field `{field}`; extensions belong in `ext`")]
    UnknownField {
        /// The offending field.
        field: String,
    },

    /// A CID-valued field did not parse.
    #[error("field `{field}`: {source}")]
    BadCid {
        /// The offending field.
        field: &'static str,
        /// Why it did not parse.
        #[source]
        source: CidError,
    },

    /// The bytes did not hash to the identifier the caller asked for.
    #[error("identifier mismatch: bytes hash to {actual}, not {expected}")]
    IdentifierMismatch {
        /// The identifier requested.
        expected: String,
        /// The identifier the bytes actually have.
        actual: String,
    },
}

/// A parsed object: its header, its body, and the bytes it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    protocol: String,
    kind: String,
    created: String,
    author: Cid,
    body: BTreeMap<String, Value>,
    prev: Option<Cid>,
    ext: Option<BTreeMap<String, Value>>,
    bytes: Vec<u8>,
}

const HEADER_FIELDS: [&str; 7] = ["author", "body", "created", "ext", "prev", "pub", "type"];

impl Object {
    /// Parse canonical bytes into an object whose identity is not yet checked.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError`] if the bytes are not canonical, if a required
    /// header field is absent or ill-typed, or if an unrecognized field
    /// appears at the top level.
    pub fn parse(bytes: &[u8]) -> Result<Unverified<Self>, ObjectError> {
        let value = cbor::decode(bytes)?;
        let Value::Map(map) = value else {
            return Err(ObjectError::NotAMap {
                found: describe(&value),
            });
        };

        for key in map.keys() {
            if !HEADER_FIELDS.contains(&key.as_str()) {
                return Err(ObjectError::UnknownField { field: key.clone() });
            }
        }

        let object = Self {
            protocol: text_field(&map, "pub")?,
            kind: text_field(&map, "type")?,
            created: text_field(&map, "created")?,
            author: cid_field(&map, "author")?,
            body: match map.get("body") {
                Some(Value::Map(m)) => m.clone(),
                Some(_) => {
                    return Err(ObjectError::WrongType {
                        field: "body",
                        expected: "a map",
                    });
                }
                None => return Err(ObjectError::MissingField { field: "body" }),
            },
            prev: match map.get("prev") {
                Some(_) => Some(cid_field(&map, "prev")?),
                None => None,
            },
            ext: match map.get("ext") {
                Some(Value::Map(m)) => Some(m.clone()),
                Some(_) => {
                    return Err(ObjectError::WrongType {
                        field: "ext",
                        expected: "a map",
                    });
                }
                None => None,
            },
            bytes: bytes.to_vec(),
        };
        Ok(Unverified(object))
    }

    /// Start building an object of `kind` authored by `author`.
    #[must_use]
    pub fn builder(kind: &str, author: &str) -> Builder {
        Builder {
            kind: kind.to_owned(),
            author: author.to_owned(),
            protocol: "1".to_owned(),
            created: None,
            body: BTreeMap::new(),
            prev: None,
            ext: None,
        }
    }

    /// The protocol major version.
    #[must_use]
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// The object type.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The author's claimed creation instant.
    ///
    /// Section 4.3: this is an unverified claim. Use a timestamp
    /// attestation where correctness depends on time.
    #[must_use]
    pub fn created(&self) -> &str {
        &self.created
    }

    /// The identifier of the signing key object.
    #[must_use]
    pub fn author(&self) -> &Cid {
        &self.author
    }

    /// The type-specific payload.
    #[must_use]
    pub fn body(&self) -> &BTreeMap<String, Value> {
        &self.body
    }

    /// The author's previous object, if this one names it.
    #[must_use]
    pub fn prev(&self) -> Option<&Cid> {
        self.prev.as_ref()
    }

    /// Extension fields, if present.
    #[must_use]
    pub fn ext(&self) -> Option<&BTreeMap<String, Value>> {
        self.ext.as_ref()
    }

    /// The canonical bytes this object was parsed from.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// An object whose identity has not been checked.
///
/// Deliberately opaque: the inner object is reachable only by verifying it,
/// or by [`Unverified::peek`] for diagnostics that must not feed the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unverified<T>(T);

impl Unverified<Object> {
    /// Confirm the bytes hash to `expected`.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::IdentifierMismatch`] if they do not.
    pub fn verify(self, expected: &Cid) -> Result<Verified<Object>, ObjectError> {
        if expected.verifies(&self.0.bytes) {
            Ok(Verified(self.0))
        } else {
            Err(ObjectError::IdentifierMismatch {
                expected: expected.to_string(),
                actual: Cid::of(&self.0.bytes, expected.alg()).to_string(),
            })
        }
    }

    /// Borrow the object without verifying it.
    ///
    /// For error messages and inspection tools only. Nothing that indexes,
    /// evaluates, or stores should take this path.
    #[must_use]
    pub fn peek(&self) -> &Object {
        &self.0
    }
}

/// An object whose bytes have been confirmed against its identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified<T>(T);

impl Verified<Object> {
    /// Borrow the verified object.
    #[must_use]
    pub fn object(&self) -> &Object {
        &self.0
    }

    /// Take ownership of the verified object.
    #[must_use]
    pub fn into_inner(self) -> Object {
        self.0
    }
}

/// Builds an object and emits its canonical bytes.
#[derive(Debug, Clone)]
pub struct Builder {
    kind: String,
    author: String,
    protocol: String,
    created: Option<String>,
    body: BTreeMap<String, Value>,
    prev: Option<String>,
    ext: Option<BTreeMap<String, Value>>,
}

impl Builder {
    /// Set the creation instant (RFC 3339 UTC, second precision).
    #[must_use]
    pub fn created(mut self, when: &str) -> Self {
        self.created = Some(when.to_owned());
        self
    }

    /// Insert a body field.
    #[must_use]
    pub fn field(mut self, key: &str, value: Value) -> Self {
        self.body.insert(key.to_owned(), value);
        self
    }

    /// Name the author's previous object.
    #[must_use]
    pub fn prev(mut self, cid: &str) -> Self {
        self.prev = Some(cid.to_owned());
        self
    }

    /// Emit canonical bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError`] if a CID-valued field does not parse or if
    /// no creation instant was set.
    pub fn build(self) -> Result<Vec<u8>, ObjectError> {
        let created = self
            .created
            .ok_or(ObjectError::MissingField { field: "created" })?;
        let author: Cid = self.author.parse().map_err(|source| ObjectError::BadCid {
            field: "author",
            source,
        })?;

        let mut map = BTreeMap::new();
        map.insert("author".to_owned(), Value::Text(author.to_string()));
        map.insert("body".to_owned(), Value::Map(self.body));
        map.insert("created".to_owned(), Value::Text(created));
        map.insert("pub".to_owned(), Value::Text(self.protocol));
        map.insert("type".to_owned(), Value::Text(self.kind));
        if let Some(prev) = self.prev {
            let parsed: Cid = prev.parse().map_err(|source| ObjectError::BadCid {
                field: "prev",
                source,
            })?;
            map.insert("prev".to_owned(), Value::Text(parsed.to_string()));
        }
        if let Some(ext) = self.ext {
            map.insert("ext".to_owned(), Value::Map(ext));
        }
        Ok(cbor::encode(&Value::Map(map)))
    }
}

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Uint(_) | Value::Nint(_) => "an integer",
        Value::Bytes(_) => "a byte string",
        Value::Text(_) => "a text string",
        Value::Array(_) => "an array",
        Value::Map(_) => "a map",
        Value::Bool(_) => "a boolean",
        Value::Null => "null",
    }
}

fn text_field(map: &BTreeMap<String, Value>, field: &'static str) -> Result<String, ObjectError> {
    match map.get(field) {
        Some(Value::Text(s)) => Ok(s.clone()),
        Some(_) => Err(ObjectError::WrongType {
            field,
            expected: "a text string",
        }),
        None => Err(ObjectError::MissingField { field }),
    }
}

fn cid_field(map: &BTreeMap<String, Value>, field: &'static str) -> Result<Cid, ObjectError> {
    let text = text_field(map, field)?;
    text.parse()
        .map_err(|source| ObjectError::BadCid { field, source })
}
