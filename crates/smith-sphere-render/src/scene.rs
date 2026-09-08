//! Presentation-independent geometry derived from loaded traces.

use smith_sphere_core::impedance::Normalized;
use smith_sphere_core::sphere::{negative_chart, positive_chart, region_of, sphere_point};
use smith_sphere_core::{Complex, Document, Impedance, Region, SpherePoint, Trace};
use std::f64::consts::FRAC_PI_2;

/// Identifies one sample in one trace of one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PointRef {
    pub document: usize,
    pub trace: usize,
    pub sample: usize,
}

/// Hover and fixed selection shared by every view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionState {
    pub selected: Option<PointRef>,
    pub hovered: Option<PointRef>,
}

/// One plotted sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlottedSample {
    pub sample: usize,
    pub frequency_hz: Option<f64>,
    pub impedance: Impedance,
    pub normalized: Normalized,
    pub sphere: SpherePoint,
    pub region: Region,
}

impl PlottedSample {
    /// True when the sample is drawn in the given planar chart.
    #[must_use]
    pub fn in_chart(&self, region: Region) -> bool {
        self.region == Region::Boundary || self.region == region
    }

    /// Planar position in the chart of `region`, when the sample belongs there.
    #[must_use]
    pub fn chart_position(&self, region: Region) -> Option<[f64; 2]> {
        if !self.in_chart(region) {
            return None;
        }
        Some(match region {
            Region::Negative => negative_chart(self.sphere),
            _ => positive_chart(self.sphere),
        })
    }
}

/// Vertex of a chart polyline: a real sample or an interpolated boundary
/// crossing between two samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChartVertex {
    Sample(usize),
    /// Linear interpolation between samples `after` and `after + 1` at `t`.
    Crossing {
        after: usize,
        t: f64,
    },
}

impl ChartVertex {
    #[must_use]
    pub fn is_interpolated(self) -> bool {
        matches!(self, Self::Crossing { .. })
    }
}

/// Connected run of vertices inside one planar chart.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartSegment {
    pub region: Region,
    pub vertices: Vec<(ChartVertex, [f64; 2])>,
}

/// A trace ready to draw in all three views.
#[derive(Clone, Debug, PartialEq)]
pub struct PlottedTrace {
    pub document: usize,
    pub trace: usize,
    pub color_index: usize,
    pub label: String,
    /// `None` marks a gap that must not be bridged.
    pub points: Vec<Option<PlottedSample>>,
    pub segments: Vec<ChartSegment>,
    /// Connected runs on the sphere, gaps removed.
    pub sphere_runs: Vec<Vec<(usize, SpherePoint)>>,
    /// Interpolated boundary crossings drawn on the sphere.
    pub crossings: Vec<SpherePoint>,
}

impl PlottedTrace {
    #[must_use]
    pub fn point(&self, sample: usize) -> Option<&PlottedSample> {
        self.points.get(sample).and_then(Option::as_ref)
    }

    pub fn valid_points(&self) -> impl Iterator<Item = &PlottedSample> {
        self.points.iter().filter_map(Option::as_ref)
    }
}

/// Plots every visible trace of a document against the plotting `z0`.
#[must_use]
pub fn plot_document(
    index: usize,
    document: &Document,
    z0: f64,
    first_color: usize,
) -> Vec<PlottedTrace> {
    document
        .visible_traces()
        .enumerate()
        .map(|(offset, (trace_index, trace))| {
            plot_trace(index, trace_index, trace, z0, first_color + offset)
        })
        .collect()
}

fn plot_trace(
    document: usize,
    trace_index: usize,
    trace: &Trace,
    z0: f64,
    color_index: usize,
) -> PlottedTrace {
    let points: Vec<Option<PlottedSample>> = trace
        .samples
        .iter()
        .enumerate()
        .map(|(sample, entry)| {
            let impedance = entry.impedance?;
            let normalized = impedance.normalized(z0);
            Some(PlottedSample {
                sample,
                frequency_hz: entry.frequency_hz,
                impedance,
                normalized,
                sphere: sphere_point(normalized),
                region: region_of(normalized),
            })
        })
        .collect();

    let (segments, crossings) = split_into_chart_segments(&points);
    let sphere_runs = sphere_runs(&points);

    PlottedTrace {
        document,
        trace: trace_index,
        color_index,
        label: trace.label.clone(),
        points,
        segments,
        sphere_runs,
        crossings,
    }
}

fn sphere_runs(points: &[Option<PlottedSample>]) -> Vec<Vec<(usize, SpherePoint)>> {
    let mut runs = Vec::new();
    let mut current: Vec<(usize, SpherePoint)> = Vec::new();
    for point in points {
        match point {
            Some(point) => current.push((point.sample, point.sphere)),
            None => {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// Splits a trace into runs that live entirely inside one chart. Boundary
/// samples join both charts; a sign change between two samples inserts an
/// interpolated crossing vertex in both charts.
fn split_into_chart_segments(
    points: &[Option<PlottedSample>],
) -> (Vec<ChartSegment>, Vec<SpherePoint>) {
    let mut segments = Vec::new();
    let mut crossings = Vec::new();
    let mut positive: Vec<(ChartVertex, [f64; 2])> = Vec::new();
    let mut negative: Vec<(ChartVertex, [f64; 2])> = Vec::new();

    let flush = |run: &mut Vec<(ChartVertex, [f64; 2])>,
                 region: Region,
                 segments: &mut Vec<ChartSegment>| {
        if !run.is_empty() {
            segments.push(ChartSegment {
                region,
                vertices: std::mem::take(run),
            });
        }
    };

    for (index, point) in points.iter().enumerate() {
        let Some(point) = point else {
            flush(&mut positive, Region::Positive, &mut segments);
            flush(&mut negative, Region::Negative, &mut segments);
            continue;
        };
        if point.in_chart(Region::Positive) {
            positive.push((
                ChartVertex::Sample(point.sample),
                positive_chart(point.sphere),
            ));
        } else {
            flush(&mut positive, Region::Positive, &mut segments);
        }
        if point.in_chart(Region::Negative) {
            negative.push((
                ChartVertex::Sample(point.sample),
                negative_chart(point.sphere),
            ));
        } else {
            flush(&mut negative, Region::Negative, &mut segments);
        }

        let next = points.get(index + 1).and_then(Option::as_ref);
        let Some(next) = next else { continue };
        let crossing = match (point.region, next.region) {
            (Region::Positive, Region::Negative) | (Region::Negative, Region::Positive) => {
                boundary_crossing(point, next)
            }
            _ => None,
        };
        if let Some((t, sphere)) = crossing {
            let vertex = ChartVertex::Crossing {
                after: point.sample,
                t,
            };
            crossings.push(sphere);
            let positive_position = positive_chart(sphere);
            let negative_position = negative_chart(sphere);
            if point.region == Region::Positive {
                positive.push((vertex, positive_position));
                flush(&mut positive, Region::Positive, &mut segments);
                negative.push((vertex, negative_position));
            } else {
                negative.push((vertex, negative_position));
                flush(&mut negative, Region::Negative, &mut segments);
                positive.push((vertex, positive_position));
            }
        }
    }
    flush(&mut positive, Region::Positive, &mut segments);
    flush(&mut negative, Region::Negative, &mut segments);
    (segments, crossings)
}

/// Linear interpolation of the normalized impedance to `r = 0`.
fn boundary_crossing(a: &PlottedSample, b: &PlottedSample) -> Option<(f64, SpherePoint)> {
    let za = a.normalized.finite()?;
    let zb = b.normalized.finite()?;
    let denominator = za.re - zb.re;
    if denominator == 0.0 {
        return None;
    }
    let t = (za.re / denominator).clamp(0.0, 1.0);
    let x = za.im + (zb.im - za.im) * t;
    let crossing = sphere_point(Normalized::Finite(Complex::new(0.0, x)));
    Some((t, crossing))
}

/// How much grid to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridDetail {
    Basic,
    Detailed,
}

/// One grid curve in planar chart coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct GridCurve {
    /// Normalized `|r|` or `x` value the curve represents.
    pub value: f64,
    pub major: bool,
    pub points: Vec<[f64; 2]>,
}

/// Planar grid shared by both charts.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartGrid {
    pub resistance: Vec<GridCurve>,
    pub reactance: Vec<GridCurve>,
}

const BASIC_RESISTANCE: [f64; 5] = [0.2, 0.5, 1.0, 2.0, 5.0];
const DETAILED_RESISTANCE: [f64; 8] = [0.1, 0.3, 0.4, 0.6, 0.8, 1.5, 3.0, 10.0];
const BASIC_REACTANCE: [f64; 5] = [0.2, 0.5, 1.0, 2.0, 5.0];
const DETAILED_REACTANCE: [f64; 8] = [0.1, 0.3, 0.4, 0.6, 0.8, 1.5, 3.0, 10.0];
const MAJOR_VALUES: [f64; 3] = [0.5, 1.0, 2.0];

/// Grid values for a detail level: `(resistance, reactance)`.
#[must_use]
pub fn grid_values(detail: GridDetail) -> (Vec<f64>, Vec<f64>) {
    let mut resistance = BASIC_RESISTANCE.to_vec();
    let mut reactance = BASIC_REACTANCE.to_vec();
    if detail == GridDetail::Detailed {
        resistance.extend(DETAILED_RESISTANCE);
        reactance.extend(DETAILED_REACTANCE);
    }
    resistance.sort_by(f64::total_cmp);
    reactance.sort_by(f64::total_cmp);
    (resistance, reactance)
}

/// Builds the planar grid by mapping constant-`r` and constant-`x` lines
/// through the sphere. The negative chart shares the same geometry with
/// negated resistance labels.
#[must_use]
pub fn chart_grid(detail: GridDetail) -> ChartGrid {
    let (resistance_values, reactance_values) = grid_values(detail);
    let resistance = resistance_values
        .iter()
        .map(|&r| GridCurve {
            value: r,
            major: MAJOR_VALUES.contains(&r),
            points: constant_resistance_line(r, 180)
                .into_iter()
                .map(positive_chart)
                .collect(),
        })
        .collect();
    let reactance = reactance_values
        .iter()
        .flat_map(|&x| [x, -x])
        .map(|x| GridCurve {
            value: x,
            major: MAJOR_VALUES.contains(&x.abs()),
            points: constant_reactance_half_line(x, 120)
                .into_iter()
                .map(positive_chart)
                .collect(),
        })
        .collect();
    ChartGrid {
        resistance,
        reactance,
    }
}

/// Sphere points along `Re(z) = r` for all `x`, closed through the open point.
#[must_use]
pub fn constant_resistance_line(r: f64, steps: usize) -> Vec<SpherePoint> {
    let mut points = Vec::with_capacity(steps + 2);
    points.push(SpherePoint::OPEN);
    for step in 1..steps {
        let phi = -FRAC_PI_2 + std::f64::consts::PI * step as f64 / steps as f64;
        let x = phi.tan();
        points.push(sphere_point(Normalized::Finite(Complex::new(r, x))));
    }
    points.push(SpherePoint::OPEN);
    points
}

/// Sphere points along `Im(z) = x` for `r >= 0`, ending at the open point.
#[must_use]
pub fn constant_reactance_half_line(x: f64, steps: usize) -> Vec<SpherePoint> {
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..steps {
        let phi = FRAC_PI_2 * step as f64 / steps as f64;
        let r = phi.tan();
        points.push(sphere_point(Normalized::Finite(Complex::new(r, x))));
    }
    points.push(SpherePoint::OPEN);
    points
}

/// Sphere points along `Im(z) = x` for all `r`, from open through both
/// hemispheres back to open.
#[must_use]
pub fn constant_reactance_full_line(x: f64, steps: usize) -> Vec<SpherePoint> {
    let mut points = Vec::with_capacity(steps + 2);
    points.push(SpherePoint::OPEN);
    for step in 1..steps {
        let phi = -FRAC_PI_2 + std::f64::consts::PI * step as f64 / steps as f64;
        let r = phi.tan();
        points.push(sphere_point(Normalized::Finite(Complex::new(r, x))));
    }
    points.push(SpherePoint::OPEN);
    points
}

/// Kind of a curve drawn on the sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SphereCurveKind {
    /// `Re(z) = value`, signed.
    Resistance(f64),
    /// `Im(z) = value`, signed.
    Reactance(f64),
    /// The shared boundary `r = 0`.
    Boundary,
    /// The real axis `x = 0`.
    RealAxis,
    /// The unit circle `|z| = 1`.
    UnitMagnitude,
}

/// One curve on the sphere.
#[derive(Clone, Debug, PartialEq)]
pub struct SphereCurve {
    pub kind: SphereCurveKind,
    pub points: Vec<SpherePoint>,
}

/// Builds the sphere grid from the same constant-`r` and constant-`x` lines.
#[must_use]
pub fn sphere_grid(detail: GridDetail) -> Vec<SphereCurve> {
    let (resistance_values, reactance_values) = grid_values(detail);
    let mut curves = Vec::new();
    curves.push(SphereCurve {
        kind: SphereCurveKind::Boundary,
        points: constant_resistance_line(0.0, 180),
    });
    curves.push(SphereCurve {
        kind: SphereCurveKind::RealAxis,
        points: constant_reactance_full_line(0.0, 180),
    });
    let unit: Vec<SpherePoint> = (0..=180)
        .map(|step| {
            let theta = std::f64::consts::TAU * step as f64 / 180.0;
            SpherePoint {
                u: 0.0,
                v: theta.sin(),
                w: theta.cos(),
            }
        })
        .collect();
    curves.push(SphereCurve {
        kind: SphereCurveKind::UnitMagnitude,
        points: unit,
    });
    for r in resistance_values {
        for signed in [r, -r] {
            curves.push(SphereCurve {
                kind: SphereCurveKind::Resistance(signed),
                points: constant_resistance_line(signed, 120),
            });
        }
    }
    for x in reactance_values {
        for signed in [x, -x] {
            curves.push(SphereCurve {
                kind: SphereCurveKind::Reactance(signed),
                points: constant_reactance_full_line(signed, 120),
            });
        }
    }
    curves
}

#[cfg(test)]
mod tests {
    use super::*;
    use smith_sphere_core::dataset::{DataSource, Sample, TraceOrigin};

    fn document(samples: Vec<Sample>) -> Document {
        Document::new(
            "t",
            DataSource::Manual,
            vec![Trace::new("Z", samples, 50.0, TraceOrigin::ManualImpedance)],
        )
    }

    fn sample(f: f64, r: f64, x: f64) -> Sample {
        Sample::new(f, Impedance::new(r, x))
    }

    #[test]
    fn crossing_traces_are_split_with_an_interpolated_boundary_vertex() {
        let doc = document(vec![
            sample(1.0, 30.0, 10.0),
            sample(2.0, -30.0, 30.0),
            sample(3.0, -60.0, 0.0),
        ]);
        let plotted = plot_document(0, &doc, 50.0, 0);
        assert_eq!(plotted.len(), 1);
        let trace = &plotted[0];
        assert_eq!(trace.segments.len(), 2);
        let positive = trace
            .segments
            .iter()
            .find(|s| s.region == Region::Positive)
            .expect("positive run");
        let negative = trace
            .segments
            .iter()
            .find(|s| s.region == Region::Negative)
            .expect("negative run");
        assert_eq!(positive.vertices.len(), 2);
        assert_eq!(negative.vertices.len(), 3);
        let (vertex, position) = positive.vertices[1];
        assert!(matches!(vertex, ChartVertex::Crossing { after: 0, t } if (t - 0.5).abs() < 1e-12));
        assert!(
            (position[0].hypot(position[1]) - 1.0).abs() < 1e-12,
            "crossing sits on the rim"
        );
        let (_, negative_position) = negative.vertices[0];
        assert!(
            (position[0] - negative_position[0]).abs() < 1e-12
                && (position[1] - negative_position[1]).abs() < 1e-12
        );
        assert_eq!(trace.crossings.len(), 1);
        assert!(trace.crossings[0].w.abs() < 1e-12);
        assert_eq!(trace.sphere_runs.len(), 1);
        assert_eq!(trace.sphere_runs[0].len(), 3);
    }

    #[test]
    fn gaps_break_every_view() {
        let doc = document(vec![
            sample(1.0, 10.0, 0.0),
            Sample::gap(Some(2.0)),
            sample(3.0, 20.0, 0.0),
        ]);
        let trace = &plot_document(0, &doc, 50.0, 0)[0];
        assert_eq!(trace.segments.len(), 2);
        assert!(trace.segments.iter().all(|s| s.vertices.len() == 1));
        assert_eq!(trace.sphere_runs.len(), 2);
        assert!(trace.crossings.is_empty());
    }

    #[test]
    fn boundary_samples_belong_to_both_charts_without_interpolation() {
        let doc = document(vec![
            sample(1.0, 20.0, 0.0),
            sample(2.0, 0.0, 50.0),
            sample(3.0, -20.0, 0.0),
        ]);
        let trace = &plot_document(0, &doc, 50.0, 0)[0];
        assert!(trace.crossings.is_empty());
        let positive = trace
            .segments
            .iter()
            .find(|s| s.region == Region::Positive)
            .expect("positive");
        let negative = trace
            .segments
            .iter()
            .find(|s| s.region == Region::Negative)
            .expect("negative");
        assert_eq!(positive.vertices.len(), 2);
        assert_eq!(negative.vertices.len(), 2);
        assert!(matches!(positive.vertices[1].0, ChartVertex::Sample(1)));
        assert!(matches!(negative.vertices[0].0, ChartVertex::Sample(1)));
    }

    #[test]
    fn changing_the_plot_reference_keeps_physical_impedance() {
        let doc = document(vec![sample(1.0, 25.0, 30.0)]);
        let at_50 = &plot_document(0, &doc, 50.0, 0)[0];
        let at_75 = &plot_document(0, &doc, 75.0, 0)[0];
        let a = at_50.point(0).expect("point");
        let b = at_75.point(0).expect("point");
        assert_eq!(a.impedance, b.impedance);
        assert_ne!(a.sphere, b.sphere);
        let Normalized::Finite(z) = b.normalized else {
            panic!("finite")
        };
        assert!((z.re - 25.0 / 75.0).abs() < 1e-12);
    }

    #[test]
    fn grid_curves_stay_inside_the_unit_disc_and_share_geometry() {
        let grid = chart_grid(GridDetail::Detailed);
        for curve in grid.resistance.iter().chain(grid.reactance.iter()) {
            for point in &curve.points {
                assert!(point[0].hypot(point[1]) <= 1.0 + 1e-9, "{:?}", curve.value);
                assert!(point[0].is_finite() && point[1].is_finite());
            }
        }
        // The same constant-|r| line mapped into the negative chart with -r
        // gives the same positions.
        for curve in &grid.resistance {
            let mirrored: Vec<[f64; 2]> = constant_resistance_line(-curve.value, 180)
                .into_iter()
                .map(negative_chart)
                .collect();
            for (a, b) in curve.points.iter().zip(mirrored.iter()) {
                assert!((a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn sphere_grid_points_are_on_the_sphere() {
        for curve in sphere_grid(GridDetail::Basic) {
            for point in curve.points {
                assert!((point.norm() - 1.0).abs() < 1e-9, "{:?}", curve.kind);
            }
        }
    }
}
