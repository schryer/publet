//! Versions: `MAJOR.MINOR.PATCH` arithmetic, and whether a version meets a
//! requirement.
//!
//! What a change is worth -- which kind of change bumps which part -- is a
//! release policy and belongs to whoever releases; this module only does
//! the arithmetic once that is decided ([`Version::bump`]).

use std::fmt;

/// A `MAJOR.MINOR.PATCH` version, ordered part by part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Changes when a published interface changes.
    pub major: u64,
    /// Changes when a feature is added.
    pub minor: u64,
    /// Changes when the same functionality is fixed.
    pub patch: u64,
}

/// Which part of a version a release raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bump {
    /// The patch part: `1.4.2` to `1.4.3`.
    Patch,
    /// The minor part, resetting patch: `1.4.2` to `1.5.0`.
    Minor,
    /// The major part, resetting the others: `1.4.2` to `2.0.0`.
    Major,
}

impl Version {
    /// Parse `MAJOR.MINOR.PATCH`.
    ///
    /// # Errors
    ///
    /// Returns a message if the text is not three dot-separated integers.
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.trim().split('.').collect();
        let bad = || format!("{text:?} is not MAJOR.MINOR.PATCH");
        let [major, minor, patch] = parts.as_slice() else {
            return Err(bad());
        };
        let num = |s: &str| s.parse::<u64>().map_err(|_| bad());
        Ok(Self {
            major: num(major)?,
            minor: num(minor)?,
            patch: num(patch)?,
        })
    }

    /// This version raised by `bump`.
    #[must_use]
    pub fn bump(self, bump: Bump) -> Self {
        match bump {
            Bump::Major => Self {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            },
            Bump::Minor => Self {
                minor: self.minor + 1,
                patch: 0,
                ..self
            },
            Bump::Patch => Self {
                patch: self.patch + 1,
                ..self
            },
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Whether `version` satisfies `requires`.
///
/// Versions here are any number of dot-separated parts, as tools print
/// them (`0.15`, `1.98.1`), and `requires` takes one of four forms:
///
/// - `^X.Y`, compatible: the same major part, and not below `X.Y`. Only a
///   major bump changes an interface, so `0.3.2` satisfies `^0.1`;
/// - `X.Y+`, a minimum: any version not below it, compared numerically part
///   by part;
/// - a prefix at a dot boundary: `0.15` accepts `0.15` and `0.15.2`, not
///   `0.150`;
/// - empty, which anything satisfies.
#[must_use]
pub fn satisfies(version: &str, requires: &str) -> bool {
    let parts =
        |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse::<u64>().ok()).collect() };
    if let Some(base) = requires.strip_prefix('^') {
        return match (parts(version), parts(base)) {
            (Some(have), Some(need)) => have.first() == need.first() && have >= need,
            _ => false,
        };
    }
    if let Some(minimum) = requires.strip_suffix('+') {
        return match (parts(version), parts(minimum)) {
            (Some(have), Some(need)) => have >= need,
            _ => false,
        };
    }
    requires.is_empty()
        || version == requires
        || version
            .strip_prefix(requires)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_parse_print_and_order() {
        assert_eq!(v("10.2.33").to_string(), "10.2.33");
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.x").is_err());
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("0.99.99"));
    }

    #[test]
    fn each_bump_raises_its_part_and_resets_the_lower_ones() {
        assert_eq!(v("1.4.2").bump(Bump::Patch), v("1.4.3"));
        assert_eq!(v("1.4.2").bump(Bump::Minor), v("1.5.0"));
        assert_eq!(v("1.4.2").bump(Bump::Major), v("2.0.0"));
        assert!(Bump::Major > Bump::Minor && Bump::Minor > Bump::Patch);
    }

    #[test]
    fn a_requirement_is_a_version_prefix_at_a_dot_boundary() {
        assert!(satisfies("0.15", "0.15"));
        assert!(satisfies("0.15.2", "0.15"));
        assert!(satisfies("0.15.2", ""));
        assert!(!satisfies("0.150", "0.15"));
        assert!(!satisfies("0.16.0", "0.15"));
        assert!(satisfies("1.98.1", "1.80+"));
        assert!(satisfies("22.22.1", "22+"));
        assert!(!satisfies("1.79.0", "1.80+"));
        assert!(!satisfies("abc", "1+"));
        assert!(satisfies("0.1.0", "^0.1"));
        assert!(satisfies("0.3.2", "^0.1"));
        assert!(!satisfies("1.0.0", "^0.1"));
        assert!(!satisfies("0.0.9", "^0.1"));
        assert!(satisfies("2.4.0", "^2.1"));
    }
}
