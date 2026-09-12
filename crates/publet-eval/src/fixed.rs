//! Fixed-point arithmetic scaled by 10^6.
//!
//! R8 requires that the same policy over the same snapshot yield
//! bit-identical results in every conformant implementation. Floating point
//! cannot provide that: results depend on rounding mode, on instruction
//! selection, on whether an intermediate stayed in a wider register. So the
//! evaluation layer has no floats at all, and this type is the only numeric
//! carrier it uses.
//!
//! The type deliberately implements no conversion from `f32` or `f64`. A
//! single `as f64` anywhere in a weight computation would reintroduce the
//! problem silently, and the absence of a conversion means the mistake does
//! not compile. The `guard-floats` check backs this at the file level.
//!
//! # Examples
//!
//! ```
//! use publet_eval::Fixed6;
//!
//! let whole = Fixed6::ONE;
//! let third = whole.mul_div(Fixed6::from_integer(1), Fixed6::from_integer(3));
//! // Truncating division: one third is 0.333333, never 0.3333334.
//! assert_eq!(third.to_string(), "0.333333");
//! ```

// Clippy's `integer_division` lint advises using floats to avoid precision
// loss. That advice is correct in general and wrong here: R8 requires
// bit-identical results across implementations, which floating point cannot
// give, so truncating integer division is the specified behaviour rather
// than an approximation of something better.
#![allow(clippy::integer_division)]

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Sub};

/// The scaling factor: six decimal places.
pub const SCALE: i128 = 1_000_000;

/// A number scaled by 10^6, with truncating division.
///
/// Backed by `i128` so that a multiplication of two scaled values cannot
/// overflow before the division that rescales it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Fixed6(i128);

impl Fixed6 {
    /// Zero.
    pub const ZERO: Self = Self(0);
    /// One.
    pub const ONE: Self = Self(SCALE);

    /// Construct from a raw scaled value.
    #[must_use]
    pub const fn from_scaled(raw: i128) -> Self {
        Self(raw)
    }

    /// Construct from a whole number.
    #[must_use]
    pub const fn from_integer(n: i64) -> Self {
        Self(n as i128 * SCALE)
    }

    /// The underlying scaled value.
    #[must_use]
    pub const fn to_scaled(self) -> i128 {
        self.0
    }

    /// `self * numerator / denominator`, truncating toward zero.
    ///
    /// The multiplication happens before the division and in `i128`, so
    /// precision is not lost to an intermediate rounding that a different
    /// implementation might place elsewhere.
    ///
    /// Returns [`Self::ZERO`] when `denominator` is zero, which is the
    /// arithmetic identity this layer wants: a key that confers no weight
    /// at all confers none to anyone.
    #[must_use]
    pub fn mul_div(self, numerator: Self, denominator: Self) -> Self {
        if denominator.0 == 0 {
            return Self::ZERO;
        }
        // (a*S * b*S) / (c*S) = (a*b/c)*S, already at the right scale.
        Self(self.0.saturating_mul(numerator.0) / denominator.0)
    }

    /// `self * other`, truncating toward zero.
    #[must_use]
    pub fn times(self, other: Self) -> Self {
        Self(self.0.saturating_mul(other.0) / SCALE)
    }

    /// `self / other`, truncating toward zero.
    ///
    /// Returns [`Self::ZERO`] when `other` is zero.
    #[must_use]
    pub fn ratio(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::ZERO;
        }
        Self(self.0.saturating_mul(SCALE) / other.0)
    }

    /// The smaller of two values.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }

    /// Whether this value is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Halve repeatedly, for exponential decay.
    ///
    /// Computes `self * 2^(-numerator/denominator)` by applying whole
    /// halvings and then a linear interpolation of the remainder. Exact
    /// exponentiation would need a transcendental function, which cannot be
    /// made bit-identical across platforms; this approximation is fully
    /// specified by integer operations and therefore can.
    #[must_use]
    pub fn halve_fractional(self, numerator: u64, denominator: u64) -> Self {
        if denominator == 0 || self.0 == 0 {
            return self;
        }
        let whole = numerator / denominator;
        let mut value = self.0;
        // Beyond 127 halvings the result is zero in this representation.
        for _ in 0..whole.min(127) {
            value /= 2;
        }
        if whole >= 127 {
            return Self(0);
        }
        let remainder = numerator % denominator;
        if remainder == 0 {
            return Self(value);
        }
        // Linear interpolation between value and value/2 over the fraction.
        let frac = i128::from(remainder);
        let den = i128::from(denominator);
        Self(value - (value * frac) / (den * 2))
    }
}

impl Add for Fixed6 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl Sub for Fixed6 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl Sum for Fixed6 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl fmt::Display for Fixed6 {
    /// Render with exactly six decimal places, so that output is stable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0;
        let magnitude = self.0.unsigned_abs();
        let whole = magnitude / SCALE.unsigned_abs();
        let frac = magnitude % SCALE.unsigned_abs();
        if negative {
            write!(f, "-")?;
        }
        write!(f, "{whole}.{frac:06}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div_keeps_the_scale() {
        let two = Fixed6::from_integer(2);
        let three = Fixed6::from_integer(3);
        // 2 * 2 / 3
        assert_eq!(two.mul_div(two, three).to_string(), "1.333333");
        // Multiplying before dividing preserves precision a naive
        // (self/den)*num would lose.
        let small = Fixed6::from_scaled(3);
        assert_eq!(
            small.mul_div(Fixed6::from_integer(1000), Fixed6::from_integer(3)),
            Fixed6::from_scaled(1000)
        );
    }

    #[test]
    fn truncates_toward_zero_rather_than_rounding() {
        let one = Fixed6::ONE;
        let three = Fixed6::from_integer(3);
        assert_eq!(one.ratio(three).to_string(), "0.333333");
        let two = Fixed6::from_integer(2);
        // 2/3 truncates to 0.666666, not 0.666667.
        assert_eq!(two.ratio(three).to_string(), "0.666666");
    }

    #[test]
    fn division_by_zero_yields_zero() {
        assert_eq!(Fixed6::ONE.ratio(Fixed6::ZERO), Fixed6::ZERO);
        assert_eq!(Fixed6::ONE.mul_div(Fixed6::ONE, Fixed6::ZERO), Fixed6::ZERO);
    }

    #[test]
    fn display_is_stable_to_six_places() {
        assert_eq!(Fixed6::ZERO.to_string(), "0.000000");
        assert_eq!(Fixed6::ONE.to_string(), "1.000000");
        assert_eq!(Fixed6::from_scaled(1).to_string(), "0.000001");
        assert_eq!(Fixed6::from_scaled(-1).to_string(), "-0.000001");
    }

    #[test]
    fn halving_is_exact_on_whole_periods() {
        let one = Fixed6::ONE;
        assert_eq!(one.halve_fractional(0, 10), one);
        assert_eq!(one.halve_fractional(10, 10).to_string(), "0.500000");
        assert_eq!(one.halve_fractional(20, 10).to_string(), "0.250000");
        assert_eq!(one.halve_fractional(1000, 10), Fixed6::ZERO);
    }

    #[test]
    fn addition_saturates_rather_than_wrapping() {
        let big = Fixed6::from_scaled(i128::MAX);
        assert_eq!(big + big, Fixed6::from_scaled(i128::MAX));
    }
}
