//! The object stream format (Section 2.3).
//!
//! Length-prefixed canonical CBOR frames. Deliberately not a self-describing
//! container: a pack is a sequence of objects whose identities are their
//! hashes, so the reader recomputes each identifier rather than trusting a
//! header that says what the bytes are.

use thiserror::Error;

/// Largest frame accepted, matching the object ceiling of Section 4.5.
const MAX_FRAME: usize = 64 * 1024;

/// Why a stream could not be read.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum WireError {
    /// The stream ended part-way through a frame.
    #[error("truncated stream: needed {needed} more byte(s)")]
    Truncated {
        /// How many further bytes the frame required.
        needed: usize,
    },
    /// A frame claimed a length above the object ceiling.
    #[error("frame of {size} bytes exceeds the {MAX_FRAME} byte object limit")]
    FrameTooLarge {
        /// The length the frame declared.
        size: usize,
    },
}

/// Encode objects as a pack.
#[must_use]
pub fn pack(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for object in objects {
        let len = u32::try_from(object.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(object);
    }
    out
}

/// Decode a pack into its objects.
///
/// # Errors
///
/// Returns [`WireError`] if the stream is truncated or a frame exceeds the
/// object size ceiling.
pub fn unpack(bytes: &[u8]) -> Result<Vec<Vec<u8>>, WireError> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let header = bytes
            .get(cursor..cursor + 4)
            .ok_or(WireError::Truncated { needed: 4 })?;
        let mut length = [0u8; 4];
        length.copy_from_slice(header);
        let size = u32::from_be_bytes(length) as usize;
        if size > MAX_FRAME {
            return Err(WireError::FrameTooLarge { size });
        }
        cursor += 4;
        let frame = bytes
            .get(cursor..cursor + size)
            .ok_or(WireError::Truncated {
                needed: (cursor + size).saturating_sub(bytes.len()),
            })?;
        out.push(frame.to_vec());
        cursor += size;
    }
    Ok(out)
}
