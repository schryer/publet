//! Whether a tool's version meets a requirement.
//!
//! Tools print versions in many shapes -- `0.15`, `1.98.1`, `22.22.1` --
//! so versions here are any number of dot-separated integers rather than
//! strict semantic versions. For release arithmetic on strict
//! `MAJOR.MINOR.PATCH` versions, use the [`semver`](https://crates.io/crates/semver)
//! crate, as pubrel does.

/// Whether `version` satisfies `requires`.
///
/// `requires` takes one of four forms:
///
/// - `^X.Y`, compatible, as Cargo reads a caret requirement: not below
///   `X.Y`, and the same up to its first non-zero part. `^1.2` admits
///   `1.9.0` but not `2.0.0`; `^0.1` admits `0.1.9` but not `0.2.0`;
///   `^0.0.3` admits only `0.0.3`;
/// - `X.Y+`, a minimum: any version not below it, compared numerically
///   part by part;
/// - a prefix at a dot boundary: `0.15` admits `0.15` and `0.15.2`, not
///   `0.150`;
/// - empty, which anything satisfies.
///
/// A version or requirement with a part that is not a number satisfies
/// none of the numeric forms.
///
/// # Example
///
/// ```
/// use publet_algorithms::version::satisfies;
///
/// // Compatible, as Cargo reads it.
/// assert!(satisfies("1.98.1", "^1.80"));
/// assert!(!satisfies("2.0.0", "^1.80"));
/// assert!(satisfies("0.1.9", "^0.1"));
/// assert!(!satisfies("0.2.0", "^0.1"));
///
/// // A minimum, and a prefix.
/// assert!(satisfies("22.22.1", "22+"));
/// assert!(satisfies("0.15.2", "0.15"));
/// assert!(!satisfies("0.150", "0.15"));
/// ```
#[must_use]
pub fn satisfies(version: &str, requires: &str) -> bool {
    let parts =
        |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse::<u64>().ok()).collect() };
    if let Some(base) = requires.strip_prefix('^') {
        let (Some(have), Some(need)) = (parts(version), parts(base)) else {
            return false;
        };
        // Cargo: the parts up to and including the first non-zero one must
        // match exactly; with none non-zero, every part given must.
        let fixed = need
            .iter()
            .position(|&p| p != 0)
            .map_or(need.len(), |i| i + 1);
        let same = need
            .iter()
            .take(fixed)
            .enumerate()
            .all(|(i, part)| have.get(i).copied().unwrap_or(0) == *part);
        return same && have >= need;
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
mod tests {
    use super::*;

    #[test]
    fn a_requirement_is_a_version_prefix_at_a_dot_boundary() {
        assert!(satisfies("0.15", "0.15"));
        assert!(satisfies("0.15.2", "0.15"));
        assert!(satisfies("0.15.2", ""));
        assert!(!satisfies("0.150", "0.15"));
        assert!(!satisfies("0.16.0", "0.15"));
    }

    #[test]
    fn a_minimum_compares_part_by_part() {
        assert!(satisfies("1.98.1", "1.80+"));
        assert!(satisfies("22.22.1", "22+"));
        assert!(!satisfies("1.79.0", "1.80+"));
        assert!(!satisfies("abc", "1+"));
    }

    #[test]
    fn a_caret_is_compatible_as_cargo_reads_it() {
        // From 1.0: the major part is fixed.
        assert!(satisfies("2.4.0", "^2.1"));
        assert!(!satisfies("3.0.0", "^2.1"));
        assert!(!satisfies("2.0.9", "^2.1"));
        // Below 1.0: the minor part is fixed.
        assert!(satisfies("0.1.0", "^0.1"));
        assert!(satisfies("0.1.9", "^0.1"));
        assert!(!satisfies("0.3.2", "^0.1"));
        assert!(!satisfies("1.0.0", "^0.1"));
        assert!(!satisfies("0.0.9", "^0.1"));
        // Below 0.1: every part is fixed.
        assert!(satisfies("0.0.3", "^0.0.3"));
        assert!(!satisfies("0.0.4", "^0.0.3"));
        // All zero: the parts given are fixed.
        assert!(satisfies("0.0.7", "^0.0"));
        assert!(!satisfies("0.1.0", "^0.0"));
        assert!(!satisfies("x.1", "^0.1"));
    }
}
