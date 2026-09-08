//! Human-readable formatting of frequencies, impedances, and reflections.

use crate::complex::Complex;
use crate::i18n::Lang;
use crate::impedance::{Impedance, Reflection};

/// Formats a frequency with an SI prefix, e.g. `1.250 GHz`.
#[must_use]
pub fn frequency(hz: f64) -> String {
    let abs = hz.abs();
    let (scaled, unit) = if abs >= 1e9 {
        (hz / 1e9, "GHz")
    } else if abs >= 1e6 {
        (hz / 1e6, "MHz")
    } else if abs >= 1e3 {
        (hz / 1e3, "kHz")
    } else {
        (hz, "Hz")
    };
    format!("{} {unit}", significant(scaled, 6))
}

/// Formats a real number with a bounded number of significant digits and no
/// trailing zeros.
#[must_use]
pub fn significant(value: f64, digits: usize) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if !value.is_finite() {
        return "∞".to_owned();
    }
    let magnitude = value.abs().log10().floor() as i32;
    let decimals = (digits as i32 - 1 - magnitude).clamp(0, 12) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// Formats `R + jX` with a unit.
#[must_use]
pub fn complex_ohms(z: Complex) -> String {
    let sign = if z.im < 0.0 { "−" } else { "+" };
    format!(
        "{} {sign} j{} Ω",
        significant(z.re, 5),
        significant(z.im.abs(), 5)
    )
}

/// Formats an impedance including the open-circuit case.
#[must_use]
pub fn impedance(z: Impedance, lang: Lang) -> String {
    match z {
        Impedance::Finite(z) => complex_ohms(z),
        Impedance::Open => lang.pick("开路 (|Z| = ∞)", "open (|Z| = ∞)").to_owned(),
    }
}

/// Formats a normalized impedance without a unit.
#[must_use]
pub fn normalized(z: Option<Complex>) -> String {
    match z {
        Some(z) => {
            let sign = if z.im < 0.0 { "−" } else { "+" };
            format!(
                "{} {sign} j{}",
                significant(z.re, 5),
                significant(z.im.abs(), 5)
            )
        }
        None => "∞".to_owned(),
    }
}

/// Formats magnitude and phase of a reflection coefficient.
#[must_use]
pub fn reflection(gamma: Reflection, lang: Lang) -> (String, String) {
    match gamma {
        Reflection::Divergent => (
            lang.pick("发散 (Z = −Z0)", "divergent (Z = −Z0)")
                .to_owned(),
            lang.pick("发散", "divergent").to_owned(),
        ),
        Reflection::Finite(g) => {
            let magnitude = significant(g.abs(), 5);
            let phase = if g.is_zero() {
                lang.pick("未定义 (Γ = 0)", "undefined (Γ = 0)").to_owned()
            } else {
                format!("{}°", significant(g.arg_deg(), 5))
            };
            (magnitude, phase)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_uses_si_prefixes() {
        assert_eq!(frequency(1.25e9), "1.25 GHz");
        assert_eq!(frequency(915e6), "915 MHz");
        assert_eq!(frequency(1500.0), "1.5 kHz");
        assert_eq!(frequency(12.5), "12.5 Hz");
    }

    #[test]
    fn significant_trims_noise() {
        assert_eq!(significant(25.0, 5), "25");
        assert_eq!(significant(0.000123456, 3), "0.000123");
        assert_eq!(significant(-4.56789, 3), "-4.57");
        assert_eq!(significant(123456.0, 3), "123456");
    }

    #[test]
    fn reflection_special_cases_are_spelled_out() {
        let (magnitude, phase) = reflection(Reflection::Finite(Complex::ZERO), Lang::Chinese);
        assert_eq!(magnitude, "0");
        assert!(phase.contains("未定义"));
        let (magnitude, phase) = reflection(Reflection::Divergent, Lang::Chinese);
        assert!(magnitude.contains("发散"));
        assert!(phase.contains("发散"));
        let (_, phase) = reflection(Reflection::Finite(Complex::ZERO), Lang::English);
        assert!(phase.contains("undefined"));
    }
}
