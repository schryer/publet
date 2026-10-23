//! H3 cell identifiers and their hierarchy, without geometry.
//!
//! [H3](https://h3geo.org) divides the globe into hexagonal cells (and twelve
//! pentagons per resolution) at sixteen resolutions, 0 to 15, and names each
//! cell by a 64-bit identifier written in hexadecimal. A cell's identifier
//! holds its resolution, its base cell, and one digit per finer resolution,
//! so a cell's parent at any coarser resolution is computed from the
//! identifier alone: the digits below that resolution are cleared.
//!
//! That hierarchy is of identifiers, not of shapes. A hexagon's seven
//! children do not tile it exactly, so a point near a parent's edge can lie
//! in a child whose parent is the neighbouring cell. "Inside" here means
//! inside by identifier: [`Cell::contains`] says whether one cell is another's
//! ancestor, never whether one shape encloses another.
//!
//! Converting coordinates to cells takes geometry, and floating point, which
//! this crate does not do. Use the H3 library or one of its ports for that,
//! and this module for what follows from the identifiers.
//!
//! # Example
//!
//! ```
//! use publet_algorithms::h3::Cell;
//!
//! // A resolution-10 cell in Tallinn, and the resolution-7 cell it is in.
//! let cell: Cell = "8a089b1a2177fff".parse().unwrap();
//! let district = cell.parent(7).unwrap();
//! assert_eq!(district.to_string(), "87089b1a2ffffff");
//! assert!(district.contains(&cell));
//! assert!(!cell.contains(&district));
//! ```

use std::fmt;
use std::str::FromStr;

/// The finest resolution H3 has.
pub const MAX_RESOLUTION: u8 = 15;

/// How many base cells, the cells at resolution 0, there are.
const BASE_CELLS: u64 = 122;

/// The base cells that are pentagons.
const PENTAGONS: [u64; 12] = [4, 14, 24, 38, 49, 58, 63, 72, 83, 97, 107, 117];

/// The mode a cell identifier carries; other modes name edges and vertices.
const CELL_MODE: u64 = 1;

/// A digit beyond a cell's resolution.
const UNUSED: u64 = 7;

/// A valid H3 cell identifier.
///
/// Its text form is the canonical one H3 prints: lowercase hexadecimal with
/// no leading zeros, such as `8a089b1a2177fff`. Parsing accepts nothing
/// else, so one cell has one spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell(u64);

/// Why text is not a cell identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellError {
    /// Not canonical hexadecimal: empty, too long, uppercase, a leading
    /// zero, or a character that is not a hexadecimal digit.
    NotHex,
    /// Hexadecimal, but not the identifier of a cell: another mode, a set
    /// reserved bit, a base cell beyond 121, a digit out of range, or a
    /// pentagon's deleted direction.
    NotACell,
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotHex => "not canonical lowercase hexadecimal",
            Self::NotACell => "not the identifier of an H3 cell",
        })
    }
}

impl std::error::Error for CellError {}

impl Cell {
    /// The cell with this 64-bit identifier, if it names one.
    ///
    /// # Errors
    ///
    /// Returns [`CellError::NotACell`] if the bits do not name a valid cell.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// assert!(Cell::from_bits(0x8a08_9b1a_2177_fff).is_ok());
    /// assert!(Cell::from_bits(0).is_err());
    /// ```
    pub fn from_bits(bits: u64) -> Result<Self, CellError> {
        if bits >> 63 != 0 || (bits >> 59) & 0xf != CELL_MODE || (bits >> 56) & 0x7 != 0 {
            return Err(CellError::NotACell);
        }
        let resolution = (bits >> 52) & 0xf;
        let base = (bits >> 45) & 0x7f;
        if base >= BASE_CELLS {
            return Err(CellError::NotACell);
        }
        let mut leading = None;
        for r in 1..=u64::from(MAX_RESOLUTION) {
            let d = digit_at(bits, r);
            let used = r <= resolution;
            if used == (d == UNUSED) {
                return Err(CellError::NotACell);
            }
            if used && d != 0 && leading.is_none() {
                leading = Some(d);
            }
        }
        // A pentagon lacks one of the seven directions, the one H3 numbers
        // 1, so no descendant of one may turn that way first.
        if PENTAGONS.contains(&base) && leading == Some(1) {
            return Err(CellError::NotACell);
        }
        Ok(Self(bits))
    }

    /// The 64-bit identifier.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// let cell: Cell = "8009fffffffffff".parse().unwrap();
    /// assert_eq!(cell.bits(), 0x8009_ffff_ffff_fff);
    /// ```
    #[must_use]
    pub fn bits(self) -> u64 {
        self.0
    }

    /// The cell's resolution, from 0 (coarsest) to 15.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// let cell: Cell = "8a089b1a2177fff".parse().unwrap();
    /// assert_eq!(cell.resolution(), 10);
    /// ```
    #[must_use]
    pub fn resolution(self) -> u8 {
        // Four bits, so always below 16.
        u8::try_from((self.0 >> 52) & 0xf).unwrap_or(MAX_RESOLUTION)
    }

    /// The cell at `resolution` this cell is in, by identifier; the cell
    /// itself at its own resolution. `None` for a resolution finer than the
    /// cell's.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// let cell: Cell = "8a089b1a2177fff".parse().unwrap();
    /// assert_eq!(cell.parent(5).unwrap().to_string(), "85089b1bfffffff");
    /// assert_eq!(cell.parent(0).unwrap().to_string(), "8009fffffffffff");
    /// assert_eq!(cell.parent(10), Some(cell));
    /// assert_eq!(cell.parent(11), None);
    /// ```
    #[must_use]
    pub fn parent(self, resolution: u8) -> Option<Self> {
        let own = self.resolution();
        if resolution > own {
            return None;
        }
        let mut bits = (self.0 & !(0xf << 52)) | (u64::from(resolution) << 52);
        for r in u64::from(resolution) + 1..=u64::from(own) {
            bits |= UNUSED << digit_shift(r);
        }
        Some(Self(bits))
    }

    /// Whether `other` is this cell or one of its descendants, by
    /// identifier. This is not geometric containment: see the module
    /// documentation.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// let city: Cell = "85089b1bfffffff".parse().unwrap();
    /// let street: Cell = "8a089b1a2177fff".parse().unwrap();
    /// assert!(city.contains(&street));
    /// assert!(city.contains(&city));
    /// assert!(!street.contains(&city));
    /// ```
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        other.parent(self.resolution()) == Some(*self)
    }

    /// Whether the cell is one of the twelve pentagons at its resolution.
    ///
    /// # Example
    ///
    /// ```
    /// use publet_algorithms::h3::Cell;
    ///
    /// let pentagon: Cell = "8009fffffffffff".parse().unwrap();
    /// assert!(pentagon.is_pentagon());
    /// let hexagon: Cell = "8a089b1a2177fff".parse().unwrap();
    /// assert!(!hexagon.is_pentagon());
    /// ```
    #[must_use]
    pub fn is_pentagon(self) -> bool {
        let res = u64::from(self.resolution());
        PENTAGONS.contains(&((self.0 >> 45) & 0x7f)) && (1..=res).all(|r| digit_at(self.0, r) == 0)
    }
}

impl FromStr for Cell {
    type Err = CellError;

    fn from_str(text: &str) -> Result<Self, CellError> {
        let canonical = !text.is_empty()
            && text.len() <= 16
            && !text.starts_with('0')
            && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !canonical {
            return Err(CellError::NotHex);
        }
        let bits = u64::from_str_radix(text, 16).map_err(|_| CellError::NotHex)?;
        Self::from_bits(bits)
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

fn digit_shift(resolution: u64) -> u64 {
    (u64::from(MAX_RESOLUTION) - resolution) * 3
}

fn digit_at(bits: u64, resolution: u64) -> u64 {
    (bits >> digit_shift(resolution)) & 0x7
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // covers: Cell::from_bits, Cell::resolution
    #[test]
    fn digits_beyond_the_resolution_must_be_unused() {
        let cell: Cell = "8a089b1a2177fff".parse().unwrap();
        // Clear one unused digit's top bit: digit 11 becomes 3.
        let bits = cell.bits() & !(0x4 << digit_shift(11));
        assert_eq!(Cell::from_bits(bits), Err(CellError::NotACell));
    }

    // covers: Cell::parent, Cell::contains
    #[test]
    fn a_parent_contains_every_finer_ancestor_of_its_descendant() {
        let cell: Cell = "8a089b1a2177fff".parse().unwrap();
        for coarse in 0..=10 {
            let ancestor = cell.parent(coarse).unwrap();
            assert_eq!(ancestor.resolution(), coarse);
            for fine in coarse..=10 {
                assert!(ancestor.contains(&cell.parent(fine).unwrap()));
            }
        }
    }

    #[test]
    fn only_canonical_spellings_parse() {
        for bad in [
            "",
            "8A089B1A2177FFF",
            "08a089b1a2177fff",
            "8a089b1a2177fffff",
            "8a08 9b1a",
        ] {
            assert_eq!(bad.parse::<Cell>(), Err(CellError::NotHex), "{bad}");
        }
    }
}
