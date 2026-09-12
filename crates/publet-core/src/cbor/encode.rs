//! Canonical encoder for the Section 4.1 profile.
//!
//! Encoding is total: every [`Value`] has exactly one representation, since
//! the type admits no floats, no tags, and no indefinite lengths, and its
//! maps are sorted by construction.

use super::Value;

/// Encode a value in canonical form.
///
/// The result round-trips: `decode(&encode(v))` yields `v`, and
/// `encode(&decode(b)?)` yields `b` for any canonical `b`.
#[must_use]
pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(&mut out, value);
    out
}

/// Write a CBOR head with the shortest argument encoding that fits.
///
/// The casts below are bounded by their match arms -- each arm has already
/// established that `arg` fits the target width -- so truncation is
/// unreachable. `u8::try_from` would need a fallback value that could only
/// ever mask a logic error, which is worse than an annotated cast.
#[allow(clippy::cast_possible_truncation)]
fn write_head(out: &mut Vec<u8>, major: u8, arg: u64) {
    let m = major << 5;
    match arg {
        0..=23 => out.push(m | (arg as u8)),
        24..=0xff => {
            out.push(m | 0x18);
            out.push(arg as u8);
        }
        0x100..=0xffff => {
            out.push(m | 0x19);
            out.extend_from_slice(&(arg as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(m | 0x1a);
            out.extend_from_slice(&(arg as u32).to_be_bytes());
        }
        _ => {
            out.push(m | 0x1b);
            out.extend_from_slice(&arg.to_be_bytes());
        }
    }
}

fn write_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Uint(n) => write_head(out, 0, *n),
        Value::Nint(n) => write_head(out, 1, *n),
        Value::Bytes(b) => {
            write_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            write_head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::Array(items) => {
            write_head(out, 4, items.len() as u64);
            for item in items {
                write_value(out, item);
            }
        }
        Value::Map(entries) => {
            write_head(out, 5, entries.len() as u64);
            // BTreeMap iterates in Ord order, which for String is by UTF-8
            // bytes -- exactly the ordering the profile requires.
            for (k, v) in entries {
                write_head(out, 3, k.len() as u64);
                out.extend_from_slice(k.as_bytes());
                write_value(out, v);
            }
        }
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Null => out.push(0xf6),
    }
}
