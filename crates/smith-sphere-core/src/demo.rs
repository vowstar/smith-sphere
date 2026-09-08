//! Built-in demonstration datasets. Every document produced here is labeled
//! as demo data and never presented as a measurement.

use crate::complex::Complex;
use crate::dataset::{DataSource, DemoKind, Document, LoadNote, Sample, Trace, TraceOrigin};
use crate::i18n::Lang;
use crate::impedance::Impedance;
use std::f64::consts::PI;

/// Identifier of a bundled example.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Example {
    SeriesRlc,
    NegativeResistance,
    BoundaryCrossing,
    Landmarks,
}

impl Example {
    pub const ALL: [Self; 4] = [
        Self::SeriesRlc,
        Self::NegativeResistance,
        Self::BoundaryCrossing,
        Self::Landmarks,
    ];

    #[must_use]
    pub fn kind(self) -> DemoKind {
        match self {
            Self::SeriesRlc => DemoKind::SeriesRlc,
            Self::NegativeResistance => DemoKind::NegativeResistance,
            Self::BoundaryCrossing => DemoKind::BoundaryCrossing,
            Self::Landmarks => DemoKind::Landmarks,
        }
    }

    #[must_use]
    pub fn title(self, lang: Lang) -> &'static str {
        self.kind().title(lang)
    }

    #[must_use]
    pub fn summary(self, lang: Lang) -> &'static str {
        self.kind().description(lang)
    }

    #[must_use]
    pub fn build(self) -> Document {
        match self {
            Self::SeriesRlc => series_rlc(),
            Self::NegativeResistance => negative_resistance(),
            Self::BoundaryCrossing => boundary_crossing(),
            Self::Landmarks => landmarks(),
        }
    }
}

/// Frequencies spaced linearly from `start` to `stop` inclusive.
#[must_use]
pub fn linear_sweep(start_hz: f64, stop_hz: f64, points: usize) -> Vec<f64> {
    (0..points)
        .map(|index| {
            if points <= 1 {
                start_hz
            } else {
                start_hz + (stop_hz - start_hz) * index as f64 / (points - 1) as f64
            }
        })
        .collect()
}

/// Series R, L, C impedance at a frequency.
#[must_use]
pub fn series_rlc_impedance(
    frequency_hz: f64,
    resistance: f64,
    inductance: f64,
    capacitance: f64,
) -> Complex {
    let omega = 2.0 * PI * frequency_hz;
    Complex::new(resistance, omega * inductance - 1.0 / (omega * capacitance))
}

fn demo_document(kind: DemoKind, samples: Vec<Sample>) -> Document {
    Document::new(
        kind.title(Lang::English),
        DataSource::Demo(kind),
        vec![Trace::new("Z", samples, 50.0, TraceOrigin::Demo)],
    )
}

fn series_rlc() -> Document {
    let samples = linear_sweep(100e6, 2e9, 77)
        .into_iter()
        .map(|f| {
            Sample::new(
                f,
                Impedance::Finite(series_rlc_impedance(f, 25.0, 10e-9, 5e-12)),
            )
        })
        .collect();
    demo_document(DemoKind::SeriesRlc, samples)
}

/// Ideal negative-resistance model: `-40 Ω || 2 pF`, then `2 nH` in series.
#[must_use]
pub fn negative_resistance_impedance(frequency_hz: f64) -> Complex {
    let omega = 2.0 * PI * frequency_hz;
    let g = Complex::new(1.0 / -40.0, omega * 2e-12);
    Complex::ONE / g + Complex::new(0.0, omega * 2e-9)
}

fn negative_resistance() -> Document {
    let samples = linear_sweep(100e6, 3e9, 59)
        .into_iter()
        .map(|f| Sample::new(f, Impedance::Finite(negative_resistance_impedance(f))))
        .collect();
    demo_document(DemoKind::NegativeResistance, samples)
}

/// Demonstration trajectory whose resistance changes sign at 1.5 GHz.
#[must_use]
pub fn boundary_crossing_impedance(frequency_hz: f64) -> Complex {
    let t = (frequency_hz - 1e9) / 1e9;
    Complex::new(-30.0 + 60.0 * t, -40.0 + 80.0 * t * t)
}

fn boundary_crossing() -> Document {
    let samples = linear_sweep(1e9, 2e9, 40)
        .into_iter()
        .map(|f| Sample::new(f, Impedance::Finite(boundary_crossing_impedance(f))))
        .collect();
    demo_document(DemoKind::BoundaryCrossing, samples)
}

fn landmarks() -> Document {
    // Landmark labels use notation that reads the same in every language.
    let points: [(&str, Impedance); 6] = [
        ("Z = 0", Impedance::new(0.0, 0.0)),
        ("Z = ∞", Impedance::Open),
        ("Z = +Z0", Impedance::new(50.0, 0.0)),
        ("Z = −Z0", Impedance::new(-50.0, 0.0)),
        ("Z = +jZ0", Impedance::new(0.0, 50.0)),
        ("Z = −jZ0", Impedance::new(0.0, -50.0)),
    ];
    let traces = points
        .into_iter()
        .map(|(label, impedance)| {
            Trace::new(
                label,
                vec![Sample {
                    frequency_hz: None,
                    impedance: Some(impedance),
                }],
                50.0,
                TraceOrigin::Demo,
            )
        })
        .collect();
    let mut document = Document::new(
        DemoKind::Landmarks.title(Lang::English),
        DataSource::Demo(DemoKind::Landmarks),
        traces,
    );
    document.notes.push(LoadNote::LandmarksHaveNoFrequencyAxis);
    document
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::{Region, region_of};

    #[test]
    fn series_rlc_resonates_near_712_megahertz() {
        let resonance = 1.0 / (2.0 * PI * (10e-9f64 * 5e-12).sqrt());
        assert!((resonance - 711.8e6).abs() < 1e6);
        let z = series_rlc_impedance(resonance, 25.0, 10e-9, 5e-12);
        assert!(z.im.abs() < 1e-6);
        assert_eq!(z.re, 25.0);
    }

    #[test]
    fn examples_are_labeled_as_demo_and_stay_in_their_regions() {
        for example in Example::ALL {
            let document = example.build();
            assert!(document.source.is_demo(), "{}", document.name);
            assert!(!document.traces.is_empty());
        }
        let rlc = Example::SeriesRlc.build();
        assert!(rlc.traces[0].samples.iter().all(|sample| {
            region_of(sample.impedance.expect("finite").normalized(50.0)) == Region::Positive
        }));
        let negative = Example::NegativeResistance.build();
        assert!(negative.traces[0].samples.iter().all(|sample| {
            region_of(sample.impedance.expect("finite").normalized(50.0)) == Region::Negative
        }));
        let crossing = Example::BoundaryCrossing.build();
        let regions: Vec<Region> = crossing.traces[0]
            .samples
            .iter()
            .map(|sample| region_of(sample.impedance.expect("finite").normalized(50.0)))
            .collect();
        assert!(regions.contains(&Region::Positive) && regions.contains(&Region::Negative));
    }
}
