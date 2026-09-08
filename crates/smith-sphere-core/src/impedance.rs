//! Physical impedance, normalized impedance, and reflection coefficient.
//!
//! Every quantity keeps an explicit representation of its singular value so
//! that an open circuit or a divergent reflection coefficient is never encoded
//! as `NaN` or infinity inside a `Complex`.

use crate::complex::Complex;
use serde::{Deserialize, Serialize};

/// Physical impedance in ohms.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Impedance {
    /// A finite impedance `R + jX`.
    Finite(Complex),
    /// An open circuit, `|Z| = infinity`.
    Open,
}

/// Impedance divided by the plotting reference impedance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Normalized {
    Finite(Complex),
    Infinite,
}

/// Reflection coefficient relative to a reference impedance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reflection {
    Finite(Complex),
    /// `Z = -Z0`, where `(Z - Z0)/(Z + Z0)` has a zero denominator.
    Divergent,
}

impl Impedance {
    #[must_use]
    pub const fn new(resistance: f64, reactance: f64) -> Self {
        Self::Finite(Complex::new(resistance, reactance))
    }

    /// Normalizes to a positive real reference impedance.
    #[must_use]
    pub fn normalized(self, z0: f64) -> Normalized {
        match self {
            Self::Finite(z) => {
                let normalized = z / z0;
                if normalized.is_finite() {
                    Normalized::Finite(normalized)
                } else {
                    Normalized::Infinite
                }
            }
            Self::Open => Normalized::Infinite,
        }
    }

    /// Computes `Gamma = (Z - Z0)/(Z + Z0)`.
    #[must_use]
    pub fn reflection(self, z0: f64) -> Reflection {
        match self {
            Self::Finite(z) => {
                let denominator = z + Complex::new(z0, 0.0);
                if denominator.is_zero() {
                    Reflection::Divergent
                } else {
                    let gamma = (z - Complex::new(z0, 0.0)) / denominator;
                    if gamma.is_finite() {
                        Reflection::Finite(gamma)
                    } else {
                        Reflection::Divergent
                    }
                }
            }
            Self::Open => Reflection::Finite(Complex::ONE),
        }
    }

    /// Recovers `Z = Z0 (1 + Gamma)/(1 - Gamma)`.
    #[must_use]
    pub fn from_reflection(gamma: Complex, z0: f64) -> Self {
        let denominator = Complex::ONE - gamma;
        if denominator.is_zero() {
            return Self::Open;
        }
        let z = (Complex::ONE + gamma) / denominator * z0;
        if z.is_finite() {
            Self::Finite(z)
        } else {
            Self::Open
        }
    }

    /// Resistance in ohms, `None` for an open circuit.
    #[must_use]
    pub fn resistance(self) -> Option<f64> {
        match self {
            Self::Finite(z) => Some(z.re),
            Self::Open => None,
        }
    }

    #[must_use]
    pub fn finite(self) -> Option<Complex> {
        match self {
            Self::Finite(z) => Some(z),
            Self::Open => None,
        }
    }
}

impl Reflection {
    #[must_use]
    pub fn finite(self) -> Option<Complex> {
        match self {
            Self::Finite(gamma) => Some(gamma),
            Self::Divergent => None,
        }
    }
}

impl Normalized {
    #[must_use]
    pub fn finite(self) -> Option<Complex> {
        match self {
            Self::Finite(z) => Some(z),
            Self::Infinite => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Complex, b: Complex) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn reflection_of_matched_load_is_zero() {
        assert_eq!(
            Impedance::new(50.0, 0.0).reflection(50.0),
            Reflection::Finite(Complex::ZERO)
        );
    }

    #[test]
    fn reflection_of_short_and_open() {
        assert_eq!(
            Impedance::new(0.0, 0.0).reflection(50.0),
            Reflection::Finite(Complex::new(-1.0, 0.0))
        );
        assert_eq!(
            Impedance::Open.reflection(50.0),
            Reflection::Finite(Complex::ONE)
        );
    }

    #[test]
    fn reflection_diverges_at_negative_reference() {
        assert_eq!(
            Impedance::new(-50.0, 0.0).reflection(50.0),
            Reflection::Divergent
        );
    }

    #[test]
    fn reflection_round_trip_recovers_impedance() {
        let z = Impedance::new(25.0, 30.0);
        let Reflection::Finite(gamma) = z.reflection(50.0) else {
            panic!("finite");
        };
        let Impedance::Finite(back) = Impedance::from_reflection(gamma, 50.0) else {
            panic!("finite");
        };
        assert!(close(back, Complex::new(25.0, 30.0)));
    }

    #[test]
    fn negative_resistance_has_reflection_magnitude_above_one() {
        let Reflection::Finite(gamma) = Impedance::new(-20.0, 10.0).reflection(50.0) else {
            panic!("finite");
        };
        assert!(gamma.abs() > 1.0);
        let Impedance::Finite(back) = Impedance::from_reflection(gamma, 50.0) else {
            panic!("finite");
        };
        assert!(close(back, Complex::new(-20.0, 10.0)));
    }

    #[test]
    fn unit_reflection_is_open() {
        assert_eq!(
            Impedance::from_reflection(Complex::ONE, 50.0),
            Impedance::Open
        );
    }
}
