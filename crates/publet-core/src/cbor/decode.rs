//! Validating decoder for the Section 4.1 profile.
//!
//! Validation happens during parsing rather than as a second pass, so the
//! first violation is reported with its offset and no partially-normalized
//! value is ever constructed.

use std::collections::BTreeMap;

use unicode_normalization::{UnicodeNormalization as _, is_nfc};

use super::Value;
use crate::CanonError;

/// Maximum object size, from Section 4.5.
pub const MAX_OBJECT_BYTES: usize = 64 * 1024;

/// Default nesting limit.
///
/// Not a protocol constant. Object schemas nest a handful of levels, so a
/// generous ceiling still refuses the stack exhaustion that hostile input
/// would otherwise cause.
pub const DEFAULT_MAX_DEPTH: usize = 64;

/// Decode canonical CBOR, rejecting anything outside the profile.
///
/// # Errors
///
/// Returns [`CanonError`] naming the violated rule, with the byte offset at
/// which it was detected.
pub fn decode(input: &[u8]) -> Result<Value, CanonError> {
    decode_with_limit(input, MAX_OBJECT_BYTES, DEFAULT_MAX_DEPTH)
}

/// Decode with explicit size and nesting limits.
///
/// # Errors
///
/// As [`decode`], and [`CanonError::TooLarge`] if `input` exceeds `max_bytes`.
pub fn decode_with_limit(
    input: &[u8],
    max_bytes: usize,
    max_depth: usize,
) -> Result<Value, CanonError> {
    if input.len() > max_bytes {
        return Err(CanonError::TooLarge {
            size: input.len(),
            limit: max_bytes,
        });
    }
    let mut p = Parser {
        input,
        pos: 0,
        max_depth,
    };
    let value = p.item(0)?;
    let remaining = input.len().saturating_sub(p.pos);
    if remaining > 0 {
        return Err(CanonError::TrailingData { count: remaining });
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    max_depth: usize,
}

/// The argument of a CBOR head, plus how many extra bytes encoded it.
struct Head {
    major: u8,
    arg: u64,
    /// Offset of the initial byte, for error reporting.
    offset: usize,
}

impl Parser<'_> {
    fn byte(&mut self) -> Result<u8, CanonError> {
        let b = self
            .input
            .get(self.pos)
            .copied()
            .ok_or(CanonError::Truncated {
                offset: self.pos,
                needed: 1,
            })?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], CanonError> {
        let end = self.pos.checked_add(n).ok_or(CanonError::Truncated {
            offset: self.pos,
            needed: n,
        })?;
        let slice = self.input.get(self.pos..end).ok_or(CanonError::Truncated {
            offset: self.pos,
            needed: end.saturating_sub(self.input.len()),
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// Read a head, enforcing shortest-form argument encoding.
    fn head(&mut self) -> Result<Head, CanonError> {
        let offset = self.pos;
        let initial = self.byte()?;
        let major = initial >> 5;
        let info = initial & 0x1f;

        // Major type 7 reuses additional information 25, 26 and 27 for half,
        // single and double precision floats. They must be reported as
        // floats, not as non-shortest integers, so they are caught before
        // the argument is read and the shortest-form rule is applied.
        if major == 7 {
            if matches!(info, 25..=27) {
                return Err(CanonError::Float { offset });
            }
            if info == 31 {
                return Err(CanonError::IndefiniteLength { offset });
            }
            let arg = if info == 24 {
                u64::from(self.byte()?)
            } else if info <= 23 {
                u64::from(info)
            } else {
                return Err(CanonError::Unsupported {
                    offset,
                    detail: "reserved simple value",
                });
            };
            return Ok(Head { major, arg, offset });
        }

        let (arg, extra) = match info {
            0..=23 => (u64::from(info), 0usize),
            24 => (u64::from(self.byte()?), 1),
            25 => {
                let b = self.take(2)?;
                let mut buf = [0u8; 2];
                buf.copy_from_slice(b);
                (u64::from(u16::from_be_bytes(buf)), 2)
            }
            26 => {
                let b = self.take(4)?;
                let mut buf = [0u8; 4];
                buf.copy_from_slice(b);
                (u64::from(u32::from_be_bytes(buf)), 4)
            }
            27 => {
                let b = self.take(8)?;
                let mut buf = [0u8; 8];
                buf.copy_from_slice(b);
                (u64::from_be_bytes(buf), 8)
            }
            31 => return Err(CanonError::IndefiniteLength { offset }),
            _ => {
                return Err(CanonError::Unsupported {
                    offset,
                    detail: "reserved additional information",
                });
            }
        };

        // Shortest form: the argument must not fit in a smaller encoding.
        let minimal = match arg {
            0..=23 => 0usize,
            24..=0xff => 1,
            0x100..=0xffff => 2,
            0x1_0000..=0xffff_ffff => 4,
            _ => 8,
        };
        if extra != minimal {
            return Err(CanonError::NonShortestInteger {
                offset,
                value: arg,
                used: extra,
            });
        }

        Ok(Head { major, arg, offset })
    }

    fn text_at(&mut self, head: &Head) -> Result<String, CanonError> {
        let start = self.pos;
        let len = usize::try_from(head.arg).map_err(|_| CanonError::Truncated {
            offset: start,
            needed: usize::MAX,
        })?;
        let raw = self.take(len)?;
        let s = std::str::from_utf8(raw).map_err(|_| CanonError::InvalidUtf8 { offset: start })?;
        if !is_nfc(s) {
            return Err(CanonError::NotNfc {
                offset: start,
                text: s.nfc().collect(),
            });
        }
        Ok(s.to_owned())
    }

    fn item(&mut self, depth: usize) -> Result<Value, CanonError> {
        if depth > self.max_depth {
            return Err(CanonError::TooDeep {
                offset: self.pos,
                limit: self.max_depth,
            });
        }
        let head = self.head()?;
        match head.major {
            0 => Ok(Value::Uint(head.arg)),
            1 => Ok(Value::Nint(head.arg)),
            2 => {
                let len = usize::try_from(head.arg).map_err(|_| CanonError::Truncated {
                    offset: self.pos,
                    needed: usize::MAX,
                })?;
                Ok(Value::Bytes(self.take(len)?.to_vec()))
            }
            3 => Ok(Value::Text(self.text_at(&head)?)),
            4 => {
                let len = usize::try_from(head.arg).map_err(|_| CanonError::Truncated {
                    offset: self.pos,
                    needed: usize::MAX,
                })?;
                let mut items = Vec::new();
                items
                    .try_reserve(len.min(1024))
                    .map_err(|_| CanonError::TooLarge {
                        size: len,
                        limit: MAX_OBJECT_BYTES,
                    })?;
                for _ in 0..len {
                    items.push(self.item(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            5 => self.map(&head, depth),
            6 => Err(CanonError::Unsupported {
                offset: head.offset,
                detail: "tags are not part of the profile",
            }),
            7 => Self::simple(&head),
            _ => Err(CanonError::Unsupported {
                offset: head.offset,
                detail: "unknown major type",
            }),
        }
    }

    fn map(&mut self, head: &Head, depth: usize) -> Result<Value, CanonError> {
        let len = usize::try_from(head.arg).map_err(|_| CanonError::Truncated {
            offset: self.pos,
            needed: usize::MAX,
        })?;
        let mut out: BTreeMap<String, Value> = BTreeMap::new();
        let mut previous: Option<String> = None;

        for _ in 0..len {
            let key_offset = self.pos;
            let key_head = self.head()?;
            if key_head.major != 3 {
                return Err(CanonError::NonTextKey { offset: key_offset });
            }
            let key = self.text_at(&key_head)?;

            // Order is checked against the preceding key rather than by
            // sorting afterwards, so that the offset in the error points at
            // the key that broke the ordering.
            if let Some(prev) = &previous {
                match key.as_bytes().cmp(prev.as_bytes()) {
                    std::cmp::Ordering::Less => {
                        return Err(CanonError::UnsortedKeys {
                            offset: key_offset,
                            previous: prev.clone(),
                            current: key,
                        });
                    }
                    std::cmp::Ordering::Equal => {
                        return Err(CanonError::DuplicateKey {
                            offset: key_offset,
                            key,
                        });
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }

            let value = self.item(depth + 1)?;
            previous = Some(key.clone());
            out.insert(key, value);
        }
        Ok(Value::Map(out))
    }

    fn simple(head: &Head) -> Result<Value, CanonError> {
        match head.arg {
            20 => Ok(Value::Bool(false)),
            21 => Ok(Value::Bool(true)),
            22 => Ok(Value::Null),
            23 => Err(CanonError::Unsupported {
                offset: head.offset,
                detail: "undefined is not part of the profile",
            }),
            // Floats are rejected in `head`, before their argument is read.
            _ => Err(CanonError::Unsupported {
                offset: head.offset,
                detail: "simple value outside the profile",
            }),
        }
    }
}
