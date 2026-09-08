//! The sample files under `examples/` must reproduce the analytic models
//! regardless of the format they are stored in.

use smith_sphere_core::dataset::LoadNote;
use smith_sphere_core::demo::{
    boundary_crossing_impedance, linear_sweep, negative_resistance_impedance, series_rlc_impedance,
};
use smith_sphere_core::parse::csv::{build_csv_document, inspect_csv};
use smith_sphere_core::parse::touchstone::parse_touchstone;
use smith_sphere_core::{Complex, Document, FrequencyUnit, Impedance, Lang};

const RLC_MA: &str = include_str!("../../../examples/series_rlc_ma.s1p");
const RLC_RI: &str = include_str!("../../../examples/series_rlc_ri.s1p");
const RLC_DB: &str = include_str!("../../../examples/series_rlc_db.s1p");
const RLC_CSV: &str = include_str!("../../../examples/series_rlc.csv");
const NEGATIVE: &str = include_str!("../../../examples/negative_resistance.s1p");
const CROSSING_TSV: &str = include_str!("../../../examples/crossing_r_zero.tsv");
const CROSSING_NO_UNIT: &str = include_str!("../../../examples/crossing_no_unit.csv");
const TWO_PORT: &str = include_str!("../../../examples/two_port_demo.s2p");
const MAGNITUDE_ONLY: &str = include_str!("../../../examples/magnitude_only.csv");
const V2: &str = include_str!("../../../examples/touchstone_v2_unsupported.s1p");
const KC901V_OPEN: &str = include_str!("../../../examples/kc901v_open_s11.s1p");

fn assert_matches_model(
    document: &Document,
    trace: usize,
    frequencies: &[f64],
    model: impl Fn(f64) -> Complex,
) {
    let samples = &document.traces[trace].samples;
    assert_eq!(samples.len(), frequencies.len(), "{}", document.name);
    for (sample, &frequency) in samples.iter().zip(frequencies) {
        let expected = model(frequency);
        let actual = match sample.impedance {
            Some(Impedance::Finite(z)) => z,
            other => panic!("expected finite impedance, got {other:?}"),
        };
        let frequency_error =
            (sample.frequency_hz.expect("frequency") - frequency).abs() / frequency;
        assert!(frequency_error < 1e-7, "frequency {frequency}");
        let tolerance = 1e-6 * expected.abs().max(1.0);
        assert!(
            actual.distance(expected) < tolerance,
            "{} at {frequency}: {actual:?} vs {expected:?}",
            document.name
        );
    }
}

#[test]
fn series_rlc_is_identical_across_ma_ri_db_and_csv() {
    let frequencies = linear_sweep(100e6, 2e9, 77);
    let model = |f: f64| series_rlc_impedance(f, 25.0, 10e-9, 5e-12);
    for (name, text) in [("ma", RLC_MA), ("ri", RLC_RI), ("db", RLC_DB)] {
        let document = parse_touchstone(
            text,
            &format!("series_rlc_{name}.s1p"),
            Some(1),
            Lang::Chinese,
        )
        .expect("parse");
        assert_matches_model(&document, 0, &frequencies, model);
    }
    let layout = inspect_csv(RLC_CSV, "series_rlc.csv", Lang::Chinese).expect("inspect");
    assert_eq!(layout.frequency_unit, Some(FrequencyUnit::MHz));
    let document = build_csv_document(&layout, None, Some(50.0), Lang::Chinese).expect("build");
    assert_matches_model(&document, 0, &frequencies, model);
}

#[test]
fn negative_resistance_file_matches_the_ideal_model() {
    let frequencies = linear_sweep(100e6, 3e9, 59);
    let document = parse_touchstone(NEGATIVE, "negative_resistance.s1p", Some(1), Lang::Chinese)
        .expect("parse");
    assert_matches_model(&document, 0, &frequencies, negative_resistance_impedance);
    assert!(
        document.traces[0]
            .samples
            .iter()
            .all(|sample| { matches!(sample.impedance, Some(Impedance::Finite(z)) if z.re < 0.0) })
    );
}

#[test]
fn crossing_tables_match_the_model_and_the_unitless_one_asks_for_a_unit() {
    let frequencies = linear_sweep(1e9, 2e9, 40);
    let layout = inspect_csv(CROSSING_TSV, "crossing_r_zero.tsv", Lang::Chinese).expect("inspect");
    assert_eq!(layout.frequency_unit, Some(FrequencyUnit::GHz));
    let document = build_csv_document(&layout, None, Some(50.0), Lang::Chinese).expect("build");
    assert_matches_model(&document, 0, &frequencies, boundary_crossing_impedance);

    let layout =
        inspect_csv(CROSSING_NO_UNIT, "crossing_no_unit.csv", Lang::Chinese).expect("inspect");
    assert!(layout.needs_unit());
    assert!(build_csv_document(&layout, None, Some(50.0), Lang::Chinese).is_err());
    let document = build_csv_document(&layout, Some(FrequencyUnit::GHz), Some(50.0), Lang::Chinese)
        .expect("build");
    assert_matches_model(&document, 0, &frequencies, boundary_crossing_impedance);
}

#[test]
fn two_port_file_exposes_both_ports_and_skips_noise_data() {
    let frequencies = linear_sweep(1e9, 2e9, 21);
    let document =
        parse_touchstone(TWO_PORT, "two_port_demo.s2p", Some(2), Lang::Chinese).expect("parse");
    assert_eq!(document.traces.len(), 2);
    assert_eq!(document.variant_selection, Some(0));
    assert_matches_model(&document, 0, &frequencies, |f| {
        series_rlc_impedance(f, 25.0, 10e-9, 5e-12)
    });
    assert_matches_model(&document, 1, &frequencies, boundary_crossing_impedance);
    assert!(
        document
            .notes
            .iter()
            .any(|note| matches!(note, LoadNote::TouchstoneNoiseBlockSkipped))
    );
}

#[test]
fn defective_inputs_are_rejected_with_reasons() {
    let error = inspect_csv(MAGNITUDE_ONLY, "magnitude_only.csv", Lang::Chinese)
        .expect_err("magnitude only");
    assert!(error.message.contains("相位"));
    let error = parse_touchstone(V2, "touchstone_v2_unsupported.s1p", Some(1), Lang::Chinese)
        .expect_err("v2");
    assert!(error.message.contains("2.0"));
}

/// A real KC901V sweep of an open port, exported by kcsdi-rs. The reflection
/// stays near the rim, and calibration noise pushes a few low-frequency points
/// just outside |Γ| = 1, which is exactly what the negative chart must show.
#[test]
fn real_kc901v_sweep_loads_and_keeps_points_outside_the_rim() {
    let document = parse_touchstone(KC901V_OPEN, "kc901v_open_s11.s1p", Some(1), Lang::English)
        .expect("parse");
    let trace = &document.traces[0];
    assert_eq!(trace.samples.len(), 201);
    assert_eq!(trace.source_z0, 50.0);
    assert_eq!(trace.samples[0].frequency_hz, Some(5e3));
    assert_eq!(trace.samples[200].frequency_hz, Some(650e6));
    let outside = trace
        .samples
        .iter()
        .filter(|sample| {
            sample
                .impedance
                .and_then(|z| z.reflection(50.0).finite())
                .is_some_and(|gamma| gamma.abs() > 1.0)
        })
        .count();
    assert_eq!(outside, 8, "low-frequency points just past the rim");
    // The readout SmithSphere shows at the sample nearest 104.004 MHz.
    let sample = trace
        .samples
        .iter()
        .min_by(|a, b| {
            let distance = |s: &&smith_sphere_core::Sample| {
                (s.frequency_hz.unwrap_or(f64::MAX) - 104.004e6).abs()
            };
            distance(a).total_cmp(&distance(b))
        })
        .expect("samples");
    assert!((sample.frequency_hz.expect("frequency") - 104.004e6).abs() < 1e3);
    let z = sample.impedance.and_then(|z| z.finite()).expect("finite");
    assert!(
        (z.re - 94.908).abs() < 5e-3 && (z.im - 155.43).abs() < 5e-3,
        "{z:?}"
    );
}
