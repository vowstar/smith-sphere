//! Mapping of the complete impedance plane to the unit sphere and the two
//! planar circle charts.
//!
//! With `z = r + jx` and `d = r^2 + x^2 + 1`:
//!
//! ```text
//! u = (r^2 + x^2 - 1)/d
//! v = 2x/d
//! w = 2r/d
//! ```
//!
//! The positive-resistance chart is `p+ = (u + jv)/(1 + w) = (z - 1)/(z + 1)`,
//! the classic Smith chart. The negative-resistance chart is the mirrored
//! projection `p- = (u + jv)/(1 - w) = (conj(z) + 1)/(conj(z) - 1)`. It is a
//! compressed picture of the negative half-plane, not the reflection
//! coefficient.

use crate::complex::Complex;
use crate::impedance::Normalized;

/// Point on the unit sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpherePoint {
    pub u: f64,
    pub v: f64,
    pub w: f64,
}

impl SpherePoint {
    /// Short circuit, `Z = 0`.
    pub const SHORT: Self = Self {
        u: -1.0,
        v: 0.0,
        w: 0.0,
    };
    /// Open circuit, `Z = infinity`.
    pub const OPEN: Self = Self {
        u: 1.0,
        v: 0.0,
        w: 0.0,
    };
    /// `Z = +Z0`.
    pub const MATCH: Self = Self {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    };
    /// `Z = -Z0`.
    pub const NEGATIVE_MATCH: Self = Self {
        u: 0.0,
        v: 0.0,
        w: -1.0,
    };
    /// `Z = +jZ0`.
    pub const INDUCTIVE_UNIT: Self = Self {
        u: 0.0,
        v: 1.0,
        w: 0.0,
    };
    /// `Z = -jZ0`.
    pub const CAPACITIVE_UNIT: Self = Self {
        u: 0.0,
        v: -1.0,
        w: 0.0,
    };

    #[must_use]
    pub fn norm(self) -> f64 {
        (self.u * self.u + self.v * self.v + self.w * self.w).sqrt()
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        let du = self.u - other.u;
        let dv = self.v - other.v;
        let dw = self.w - other.w;
        (du * du + dv * dv + dw * dw).sqrt()
    }

    /// Normalizes to the sphere surface; used after interpolation.
    #[must_use]
    pub fn renormalized(self) -> Self {
        let n = self.norm();
        if n == 0.0 || !n.is_finite() {
            return self;
        }
        Self {
            u: self.u / n,
            v: self.v / n,
            w: self.w / n,
        }
    }
}

/// Which hemisphere, or the shared `R = 0` boundary, a point belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Positive,
    Negative,
    Boundary,
}

/// Relative tolerance for treating a resistance as exactly zero.
pub const BOUNDARY_TOLERANCE: f64 = 1e-12;

/// Classifies a normalized impedance.
#[must_use]
pub fn region_of(z: Normalized) -> Region {
    match z {
        Normalized::Infinite => Region::Boundary,
        Normalized::Finite(z) => {
            let scale = z.abs().max(1.0);
            if z.re > BOUNDARY_TOLERANCE * scale {
                Region::Positive
            } else if z.re < -BOUNDARY_TOLERANCE * scale {
                Region::Negative
            } else {
                Region::Boundary
            }
        }
    }
}

/// Maps a normalized impedance to the sphere.
#[must_use]
pub fn sphere_point(z: Normalized) -> SpherePoint {
    match z {
        Normalized::Infinite => SpherePoint::OPEN,
        Normalized::Finite(z) => sphere_from_parts(z.re, z.im),
    }
}

fn sphere_from_parts(r: f64, x: f64) -> SpherePoint {
    let magnitude_sq = r * r + x * x;
    if magnitude_sq.is_finite() {
        let d = magnitude_sq + 1.0;
        SpherePoint {
            u: (magnitude_sq - 1.0) / d,
            v: 2.0 * x / d,
            w: 2.0 * r / d,
        }
    } else {
        // Scale by the dominant component so the squares stay finite.
        let scale = r.abs().max(x.abs());
        let rs = r / scale;
        let xs = x / scale;
        let ms = rs * rs + xs * xs;
        let inverse_scale_sq = 1.0 / (scale * scale);
        let d = ms + inverse_scale_sq;
        SpherePoint {
            u: (ms - inverse_scale_sq) / d,
            v: 2.0 * xs / scale / d,
            w: 2.0 * rs / scale / d,
        }
    }
}

/// Positive-resistance chart coordinates, valid for `w >= 0`.
#[must_use]
pub fn positive_chart(point: SpherePoint) -> [f64; 2] {
    let denominator = 1.0 + point.w;
    [point.u / denominator, point.v / denominator]
}

/// Negative-resistance chart coordinates, valid for `w <= 0`.
#[must_use]
pub fn negative_chart(point: SpherePoint) -> [f64; 2] {
    let denominator = 1.0 - point.w;
    [point.u / denominator, point.v / denominator]
}

/// Classic Smith chart position `(z - 1)/(z + 1)` computed directly.
#[must_use]
pub fn classic_smith(z: Normalized) -> [f64; 2] {
    match z {
        Normalized::Infinite => [1.0, 0.0],
        Normalized::Finite(z) => {
            let p = (z - Complex::ONE) / (z + Complex::ONE);
            [p.re, p.im]
        }
    }
}

/// Mirrored negative chart position `(conj(z) + 1)/(conj(z) - 1)` computed directly.
#[must_use]
pub fn mirrored_negative(z: Normalized) -> [f64; 2] {
    match z {
        Normalized::Infinite => [1.0, 0.0],
        Normalized::Finite(z) => {
            let c = z.conj();
            let p = (c + Complex::ONE) / (c - Complex::ONE);
            [p.re, p.im]
        }
    }
}

/// Recovers the sphere point from a positive-chart position inside the unit
/// disc. Used to convert pointer positions back into impedances.
#[must_use]
pub fn sphere_from_positive_chart(p: [f64; 2]) -> SpherePoint {
    // p = (u + jv)/(1 + w) with u^2 + v^2 + w^2 = 1 gives |p|^2 = (1 - w)/(1 + w).
    let m = p[0] * p[0] + p[1] * p[1];
    let w = (1.0 - m) / (1.0 + m);
    let s = 1.0 + w;
    SpherePoint {
        u: p[0] * s,
        v: p[1] * s,
        w,
    }
}

/// Recovers the sphere point from a negative-chart position inside the unit disc.
#[must_use]
pub fn sphere_from_negative_chart(p: [f64; 2]) -> SpherePoint {
    let m = p[0] * p[0] + p[1] * p[1];
    let w = -(1.0 - m) / (1.0 + m);
    let s = 1.0 - w;
    SpherePoint {
        u: p[0] * s,
        v: p[1] * s,
        w,
    }
}

/// Recovers the normalized impedance from a sphere point.
#[must_use]
pub fn normalized_from_sphere(point: SpherePoint) -> Normalized {
    // u = (|z|^2 - 1)/d and 1 - u = 2/d, so d = 2/(1 - u).
    let denominator = 1.0 - point.u;
    if denominator <= 1e-15 {
        return Normalized::Infinite;
    }
    let d = 2.0 / denominator;
    Normalized::Finite(Complex::new(point.w * d / 2.0, point.v * d / 2.0))
}

/// Center and radius of the constant-`|r|` circle in either planar chart.
///
/// Both charts share the same grid geometry; the negative chart labels the
/// circle with `-|r|`.
#[must_use]
pub fn resistance_circle(r_abs: f64) -> ([f64; 2], f64) {
    ([r_abs / (1.0 + r_abs), 0.0], 1.0 / (1.0 + r_abs))
}

/// Center and radius of the constant-`x` circle in either planar chart.
#[must_use]
pub fn reactance_circle(x: f64) -> ([f64; 2], f64) {
    ([1.0, 1.0 / x], 1.0 / x.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(r: f64, x: f64) -> Normalized {
        Normalized::Finite(Complex::new(r, x))
    }

    fn close3(a: SpherePoint, b: SpherePoint) -> bool {
        a.distance(b) < 1e-12
    }

    fn close2(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-12
    }

    #[test]
    fn landmark_impedances_land_on_the_documented_sphere_points() {
        assert!(close3(sphere_point(n(0.0, 0.0)), SpherePoint::SHORT));
        assert!(close3(
            sphere_point(Normalized::Infinite),
            SpherePoint::OPEN
        ));
        assert!(close3(sphere_point(n(1.0, 0.0)), SpherePoint::MATCH));
        assert!(close3(
            sphere_point(n(-1.0, 0.0)),
            SpherePoint::NEGATIVE_MATCH
        ));
        assert!(close3(
            sphere_point(n(0.0, 1.0)),
            SpherePoint::INDUCTIVE_UNIT
        ));
        assert!(close3(
            sphere_point(n(0.0, -1.0)),
            SpherePoint::CAPACITIVE_UNIT
        ));
    }

    #[test]
    fn every_point_lies_on_the_unit_sphere() {
        let values = [
            (0.3, -2.5),
            (-0.7, 0.2),
            (12.0, 40.0),
            (-300.0, 0.001),
            (1e-9, 1e9),
            (1e200, -1e200),
            (-1e300, 1e-300),
        ];
        for (r, x) in values {
            let p = sphere_point(n(r, x));
            assert!((p.norm() - 1.0).abs() < 1e-12, "{r} {x}: {p:?}");
            assert!(p.u.is_finite() && p.v.is_finite() && p.w.is_finite());
        }
    }

    #[test]
    fn hemisphere_sign_follows_resistance_and_reactance() {
        let p = sphere_point(n(2.0, 3.0));
        assert!(p.w > 0.0 && p.v > 0.0);
        let q = sphere_point(n(-2.0, -3.0));
        assert!(q.w < 0.0 && q.v < 0.0);
    }

    #[test]
    fn positive_chart_matches_classic_smith_chart() {
        for (r, x) in [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.5, 0.5),
            (2.0, -3.0),
            (0.0, 1.0),
            (1e6, 0.0),
        ] {
            let z = n(r, x);
            assert!(
                close2(positive_chart(sphere_point(z)), classic_smith(z)),
                "{r} {x}"
            );
        }
        assert!(close2(positive_chart(SpherePoint::OPEN), [1.0, 0.0]));
    }

    #[test]
    fn negative_chart_matches_mirrored_formula() {
        for (r, x) in [
            (0.0, 0.0),
            (-1.0, 0.0),
            (-0.5, 0.5),
            (-2.0, -3.0),
            (0.0, 1.0),
            (-1e6, 0.0),
        ] {
            let z = n(r, x);
            assert!(
                close2(negative_chart(sphere_point(z)), mirrored_negative(z)),
                "{r} {x}"
            );
        }
    }

    #[test]
    fn charts_stay_inside_their_unit_disc() {
        for i in -40..=40 {
            for k in -40..=40 {
                let r = f64::from(i) * 0.37;
                let x = f64::from(k) * 0.53;
                let p = sphere_point(n(r, x));
                if r >= 0.0 {
                    let q = positive_chart(p);
                    assert!(q[0].hypot(q[1]) <= 1.0 + 1e-12, "{r} {x}");
                }
                if r <= 0.0 {
                    let q = negative_chart(p);
                    assert!(q[0].hypot(q[1]) <= 1.0 + 1e-12, "{r} {x}");
                }
            }
        }
    }

    #[test]
    fn shared_boundary_has_identical_positions_in_both_charts() {
        for k in -30..=30 {
            let x = f64::from(k) * 0.4;
            let p = sphere_point(n(0.0, x));
            let a = positive_chart(p);
            let b = negative_chart(p);
            assert!(close2(a, b), "{x}");
            assert!((a[0].hypot(a[1]) - 1.0).abs() < 1e-12);
        }
        assert!(close2(
            positive_chart(SpherePoint::OPEN),
            negative_chart(SpherePoint::OPEN)
        ));
    }

    #[test]
    fn both_charts_keep_inductive_up_and_short_left() {
        let inductive_positive = positive_chart(sphere_point(n(0.5, 1.0)));
        let inductive_negative = negative_chart(sphere_point(n(-0.5, 1.0)));
        assert!(inductive_positive[1] > 0.0 && inductive_negative[1] > 0.0);
        let short = sphere_point(n(0.0, 0.0));
        assert!(positive_chart(short)[0] < -0.999 && negative_chart(short)[0] < -0.999);
        assert!(close2(
            negative_chart(SpherePoint::NEGATIVE_MATCH),
            [0.0, 0.0]
        ));
    }

    #[test]
    fn chart_inverses_recover_the_sphere_point() {
        for (r, x) in [(0.4, 0.9), (3.0, -0.2), (0.0, 2.0)] {
            let p = sphere_point(n(r, x));
            assert!(close3(sphere_from_positive_chart(positive_chart(p)), p));
            let q = sphere_point(n(-r, x));
            assert!(close3(sphere_from_negative_chart(negative_chart(q)), q));
            let Normalized::Finite(back) = normalized_from_sphere(p) else {
                panic!("finite");
            };
            assert!(back.distance(Complex::new(r, x)) < 1e-9);
        }
        assert_eq!(
            normalized_from_sphere(SpherePoint::OPEN),
            Normalized::Infinite
        );
    }

    #[test]
    fn grid_circles_agree_with_the_mapping() {
        let (center, radius) = resistance_circle(0.5);
        for k in -20..=20 {
            let x = f64::from(k) * 0.3;
            let p = positive_chart(sphere_point(n(0.5, x)));
            assert!(((p[0] - center[0]).hypot(p[1] - center[1]) - radius).abs() < 1e-12);
            let q = negative_chart(sphere_point(n(-0.5, x)));
            assert!(((q[0] - center[0]).hypot(q[1] - center[1]) - radius).abs() < 1e-12);
        }
        let (center, radius) = reactance_circle(-2.0);
        for k in 0..=20 {
            let r = f64::from(k) * 0.4;
            let p = positive_chart(sphere_point(n(r, -2.0)));
            assert!(((p[0] - center[0]).hypot(p[1] - center[1]) - radius).abs() < 1e-12);
            let q = negative_chart(sphere_point(n(-r, -2.0)));
            assert!(((q[0] - center[0]).hypot(q[1] - center[1]) - radius).abs() < 1e-12);
        }
    }

    #[test]
    fn region_classification_uses_a_relative_tolerance() {
        assert_eq!(region_of(n(0.0, 3.0)), Region::Boundary);
        assert_eq!(region_of(n(1e-15, 3.0)), Region::Boundary);
        assert_eq!(region_of(n(1e-6, 3.0)), Region::Positive);
        assert_eq!(region_of(n(-1e-6, 3.0)), Region::Negative);
        assert_eq!(region_of(Normalized::Infinite), Region::Boundary);
    }
}
