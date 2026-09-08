//! Built-in demonstration datasets. Every document produced here is labeled
//! as demo data and never presented as a measurement.

use crate::complex::Complex;
use crate::dataset::{DataSource, Document, Sample, Trace, TraceOrigin};
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
    pub fn title(self) -> &'static str {
        match self {
            Self::SeriesRlc => "串联 RLC 扫频",
            Self::NegativeResistance => "负电阻器件",
            Self::BoundaryCrossing => "跨越 R = 0 的轨迹",
            Self::Landmarks => "特征点集合",
        }
    }

    #[must_use]
    pub fn summary(self) -> &'static str {
        match self {
            Self::SeriesRlc => "R = 25 Ω、L = 10 nH、C = 5 pF 串联，100 MHz 至 2 GHz，正电阻区。",
            Self::NegativeResistance => "理想数学模型：−40 Ω 与 2 pF 并联后串联 2 nH，全程负电阻。",
            Self::BoundaryCrossing => "演示数据：电阻从 −30 Ω 线性变到 +30 Ω，轨迹穿过共享边界。",
            Self::Landmarks => "Z = 0、∞、±Z0、±jZ0 六个特征点（Z0 = 50 Ω），用于核对位置。",
        }
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

fn demo_document(name: &str, description: &str, label: &str, samples: Vec<Sample>) -> Document {
    Document::new(
        name,
        DataSource::Demo {
            description: description.to_owned(),
        },
        vec![Trace::new(label, samples, 50.0, TraceOrigin::Demo)],
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
    demo_document(
        "演示：串联 RLC",
        "演示数据。R = 25 Ω、L = 10 nH、C = 5 pF 三者串联，谐振约 712 MHz。",
        "Z",
        samples,
    )
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
    demo_document(
        "演示：负电阻器件",
        "演示数据，理想数学模型：−40 Ω 与 2 pF 并联，再串联 2 nH。负电阻不代表电路一定不稳定。",
        "Z",
        samples,
    )
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
    demo_document(
        "演示：跨越 R = 0",
        "演示数据，非实测：R 从 −30 Ω 线性变到 +30 Ω，X 从 −40 Ω 变到 +40 Ω。",
        "Z",
        samples,
    )
}

fn landmarks() -> Document {
    let points: [(&str, Impedance); 6] = [
        ("短路 Z = 0", Impedance::new(0.0, 0.0)),
        ("开路 Z = ∞", Impedance::Open),
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
        "演示：特征点",
        DataSource::Demo {
            description: "演示数据：六个无频率的特征点，Z0 = 50 Ω。".to_owned(),
        },
        traces,
    );
    document
        .notes
        .push("特征点没有频率轴，因此不显示频率滑块。".to_owned());
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
