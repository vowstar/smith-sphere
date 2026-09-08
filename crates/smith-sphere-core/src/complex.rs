//! Minimal complex arithmetic with the exact semantics the charts need.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Complex number with `f64` components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };
    pub const J: Self = Self { re: 0.0, im: 1.0 };

    #[must_use]
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Builds a value from magnitude and phase in degrees.
    #[must_use]
    pub fn from_polar_deg(magnitude: f64, phase_deg: f64) -> Self {
        let phase = phase_deg.to_radians();
        Self::new(magnitude * phase.cos(), magnitude * phase.sin())
    }

    #[must_use]
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    #[must_use]
    pub fn abs_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Phase in degrees within `(-180, 180]`.
    #[must_use]
    pub fn arg_deg(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }

    #[must_use]
    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }

    #[must_use]
    pub fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }

    /// Distance to another value.
    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        (self - other).abs()
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl Div<f64> for Complex {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self::new(self.re / rhs, self.im / rhs)
    }
}

impl Div for Complex {
    type Output = Self;

    /// Smith's scaled division, which avoids intermediate overflow.
    fn div(self, rhs: Self) -> Self {
        if rhs.re.abs() >= rhs.im.abs() {
            let ratio = rhs.im / rhs.re;
            let denominator = rhs.re + rhs.im * ratio;
            Self::new(
                (self.re + self.im * ratio) / denominator,
                (self.im - self.re * ratio) / denominator,
            )
        } else {
            let ratio = rhs.re / rhs.im;
            let denominator = rhs.re * ratio + rhs.im;
            Self::new(
                (self.re * ratio + self.im) / denominator,
                (self.im * ratio - self.re) / denominator,
            )
        }
    }
}

impl Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.re, -self.im)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn division_matches_textbook_result() {
        let quotient = Complex::new(1.0, 2.0) / Complex::new(3.0, -4.0);
        assert!((quotient.re - (-0.2)).abs() < 1e-12);
        assert!((quotient.im - 0.4).abs() < 1e-12);
    }

    #[test]
    fn polar_round_trip() {
        let value = Complex::from_polar_deg(2.0, 135.0);
        assert!((value.abs() - 2.0).abs() < 1e-12);
        assert!((value.arg_deg() - 135.0).abs() < 1e-9);
    }
}
