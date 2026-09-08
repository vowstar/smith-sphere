//! Touchstone 1.x reader for `.s1p` and `.s2p` files.
//!
//! Supported: the `#` option line (frequency unit, `S`/`Z`/`Y` parameter type,
//! `MA`/`DB`/`RI` format, `R n`), `!` comments, data rows split across lines,
//! and the trailing noise-parameter block of two-port files, which is skipped.
//! Touchstone 2.0 keywords, mixed-mode data, and more than two ports are
//! rejected with an explicit message.

use super::{FrequencyUnit, ParseError};
use crate::complex::Complex;
use crate::dataset::{
    DataSource, Document, LoadNote, PortVariant, Sample, TouchstoneSummary, Trace, TraceOrigin,
};
use crate::i18n::Lang;
use crate::impedance::Impedance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    S,
    Y,
    Z,
    G,
    H,
}

impl Parameter {
    fn label(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::G => "G",
            Self::H => "H",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    MagnitudeAngle,
    DecibelAngle,
    RealImaginary,
}

impl Format {
    fn label(self) -> &'static str {
        match self {
            Self::MagnitudeAngle => "MA",
            Self::DecibelAngle => "DB",
            Self::RealImaginary => "RI",
        }
    }

    fn to_complex(self, a: f64, b: f64) -> Complex {
        match self {
            Self::MagnitudeAngle => Complex::from_polar_deg(a, b),
            Self::DecibelAngle => Complex::from_polar_deg(10f64.powf(a / 20.0), b),
            Self::RealImaginary => Complex::new(a, b),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Options {
    unit: FrequencyUnit,
    parameter: Parameter,
    format: Format,
    reference: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            unit: FrequencyUnit::GHz,
            parameter: Parameter::S,
            format: Format::MagnitudeAngle,
            reference: 50.0,
        }
    }
}

/// Parses Touchstone 1.x text. `port_hint` comes from the file extension.
///
/// # Errors
///
/// Returns a user-facing error for unsupported versions, parameter types,
/// malformed numbers, or an inconsistent number of values.
pub fn parse_touchstone(
    text: &str,
    file_name: &str,
    port_hint: Option<u8>,
    lang: Lang,
) -> Result<Document, ParseError> {
    if let Some(ports) = port_hint
        && ports != 1
        && ports != 2
    {
        return Err(ParseError::new(match lang {
            Lang::Chinese => format!("{ports} 端口 Touchstone 文件暂不支持。"),
            Lang::English => format!("{ports}-port Touchstone files are not supported yet."),
        })
        .with_hint(lang.pick(
            "当前支持 .s1p 和 .s2p（Touchstone 1.x）。",
            "Only .s1p and .s2p (Touchstone 1.x) are supported.",
        )));
    }

    let mut options: Option<Options> = None;
    let mut notes = Vec::new();
    // Data rows are assembled line by line so that a trailing noise-parameter
    // block, whose rows have a different width, can be recognized and skipped.
    let mut pending: Vec<f64> = Vec::new();
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut ports: Option<u8> = port_hint;
    let mut previous_frequency: Option<f64> = None;
    let mut first_data_line: Option<usize> = None;
    let mut noise_skipped = false;

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim_start_matches('\u{feff}');
        let content = line.split('!').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        if content.starts_with('[') {
            return Err(unsupported_keyword(content, lang));
        }
        if let Some(option_text) = content.strip_prefix('#') {
            let parsed = parse_options(option_text, line_number, lang)?;
            if options.is_some() {
                notes.push(LoadNote::TouchstoneSecondOptionLine(line_number));
            } else if first_data_line.is_some() {
                notes.push(LoadNote::TouchstoneOptionAfterData(line_number));
            } else {
                options = Some(parsed);
            }
            continue;
        }
        if noise_skipped {
            continue;
        }
        let mut values = Vec::new();
        for token in content.split_whitespace() {
            let value = parse_number(token).ok_or_else(|| {
                ParseError::new(match lang {
                    Lang::Chinese => format!("第 {line_number} 行无法读取数值“{token}”。"),
                    Lang::English => {
                        format!("Line {line_number} has a value that is not a number: \"{token}\".")
                    }
                })
                .with_hint(lang.pick(
                    "Touchstone 数据行只能包含数字，注释需以 ! 开头。",
                    "Touchstone data rows hold numbers only; comments start with !.",
                ))
            })?;
            values.push(value);
        }
        first_data_line.get_or_insert(line_number);

        if ports.is_none() {
            ports = Some(infer_ports_from_line(values.len(), line_number, lang)?);
        }
        let stride = 1 + 2 * usize::from(ports.unwrap_or(1)).pow(2);

        if pending.is_empty() {
            let unit = options.unwrap_or_default().unit;
            let frequency = values[0] * unit.multiplier();
            if let Some(previous) = previous_frequency
                && frequency < previous
            {
                if ports == Some(2) {
                    notes.push(LoadNote::TouchstoneNoiseBlockSkipped);
                    noise_skipped = true;
                    continue;
                }
                notes.push(LoadNote::TouchstoneFrequencyNotMonotonic(line_number));
            }
            previous_frequency = Some(frequency);
        }
        pending.extend(values);
        if pending.len() == stride {
            rows.push(std::mem::take(&mut pending));
        } else if pending.len() > stride {
            let port_count = ports.unwrap_or(1);
            return Err(ParseError::new(match lang {
                Lang::Chinese => format!(
                    "第 {line_number} 行附近的数值个数超过每个频点的 {stride} 个（{port_count} 端口）。"
                ),
                Lang::English => format!(
                    "Near line {line_number} there are more values than the {stride} per frequency point ({port_count} ports)."
                ),
            })
            .with_hint(lang.pick(
                "检查扩展名与端口数是否一致，或数据行是否被错误拆分。",
                "Check that the extension matches the port count and that rows are not split wrong.",
            )));
        }
    }

    if !pending.is_empty() {
        let stride = 1 + 2 * usize::from(ports.unwrap_or(1)).pow(2);
        let missing = stride - pending.len();
        return Err(ParseError::new(match lang {
            Lang::Chinese => format!("最后一个频点不完整，缺少 {missing} 个数值。"),
            Lang::English => {
                format!("The last frequency point is incomplete, missing {missing} values.")
            }
        })
        .with_hint(lang.pick(
            "检查文件是否被截断。",
            "Check whether the file was truncated.",
        )));
    }

    let options_declared = options.is_some();
    let options = options.unwrap_or_default();
    if !options_declared {
        notes.push(LoadNote::TouchstoneDefaultOptions);
    }
    if rows.is_empty() {
        return Err(
            ParseError::new(lang.pick("没有找到任何数据行。", "No data rows were found."))
                .with_hint(lang.pick(
                    "检查文件是否只包含注释或选项行。",
                    "Check whether the file has only comments or option lines.",
                )),
        );
    }
    let ports = ports.unwrap_or(1);

    match (ports, options.parameter) {
        (_, Parameter::S) | (1, Parameter::Z | Parameter::Y) => {}
        (2, Parameter::Z | Parameter::Y) => {
            let parameter = options.parameter.label();
            return Err(ParseError::new(match lang {
                Lang::Chinese => format!("两端口 {parameter} 参数矩阵不能直接当作端口反射系数。"),
                Lang::English => {
                    format!(
                        "A two-port {parameter} matrix cannot be read directly as port reflections."
                    )
                }
            })
            .with_hint(lang.pick(
                "请提供 S 参数的 .s2p 文件；Z11/Z22 不是匹配终接下的端口阻抗。",
                "Provide an S-parameter .s2p file; Z11/Z22 are not the matched-port impedances.",
            )));
        }
        (_, Parameter::G | Parameter::H) => {
            let parameter = options.parameter.label();
            return Err(ParseError::new(match lang {
                Lang::Chinese => format!("{parameter} 参数暂不支持。"),
                Lang::English => format!("{parameter} parameters are not supported yet."),
            })
            .with_hint(lang.pick(
                "当前支持 S 参数，以及一端口 Z、Y 参数。",
                "Supported: S parameters, and one-port Z and Y parameters.",
            )));
        }
        _ => {}
    }

    let mut traces = Vec::new();
    let unit = options.unit;
    let reference = options.reference;
    match ports {
        1 => {
            let (origin, label) = match options.parameter {
                Parameter::S => (TraceOrigin::OnePortS, "S11"),
                Parameter::Z => (TraceOrigin::OnePortZ, "Z11"),
                Parameter::Y => (TraceOrigin::OnePortY, "Y11"),
                Parameter::G | Parameter::H => unreachable!("rejected above"),
            };
            let samples = rows
                .iter()
                .map(|row| {
                    let value = options.format.to_complex(row[1], row[2]);
                    let impedance = match options.parameter {
                        Parameter::S => Impedance::from_reflection(value, reference),
                        Parameter::Z => Impedance::Finite(value * reference),
                        Parameter::Y => {
                            if value.is_zero() {
                                Impedance::Open
                            } else {
                                Impedance::Finite(Complex::new(reference, 0.0) / value)
                            }
                        }
                        Parameter::G | Parameter::H => unreachable!("rejected above"),
                    };
                    sample(row[0] * unit.multiplier(), impedance)
                })
                .collect();
            traces.push(Trace::new(label, samples, reference, origin));
        }
        2 => {
            // Touchstone 1.x two-port order is S11 S21 S12 S22.
            for (variant, offset) in [(PortVariant::S11, 1), (PortVariant::S22, 7)] {
                let samples = rows
                    .iter()
                    .map(|row| {
                        let gamma = options.format.to_complex(row[offset], row[offset + 1]);
                        sample(
                            row[0] * unit.multiplier(),
                            Impedance::from_reflection(gamma, reference),
                        )
                    })
                    .collect();
                traces.push(Trace::new(
                    variant.label(),
                    samples,
                    reference,
                    TraceOrigin::TwoPortS(variant),
                ));
            }
        }
        _ => unreachable!("validated above"),
    }

    let summary = TouchstoneSummary {
        ports,
        parameter: options.parameter.label().to_owned(),
        format: options.format.label().to_owned(),
        unit: unit.label().to_owned(),
        reference_z0: reference,
        points: rows.len(),
    };
    let mut document = Document::new(
        file_name,
        DataSource::Touchstone {
            file_name: file_name.to_owned(),
            summary,
        },
        traces,
    );
    document.notes = notes;
    if ports == 2 {
        document.variant_selection = Some(0);
    }
    Ok(document)
}

fn sample(frequency_hz: f64, impedance: Impedance) -> Sample {
    let valid = match impedance {
        Impedance::Finite(z) => z.is_finite(),
        Impedance::Open => true,
    };
    if valid && frequency_hz.is_finite() {
        Sample::new(frequency_hz, impedance)
    } else {
        Sample::gap(Some(frequency_hz))
    }
}

fn unsupported_keyword(content: &str, lang: Lang) -> ParseError {
    let keyword = content
        .split(']')
        .next()
        .unwrap_or(content)
        .trim_start_matches('[')
        .trim();
    let lower = keyword.to_ascii_lowercase();
    let what = if lower.starts_with("version") {
        lang.pick("Touchstone 2.0 文件", "a Touchstone 2.0 file")
    } else if lower.contains("mixed") {
        lang.pick("混合模（mixed-mode）数据", "mixed-mode data")
    } else {
        lang.pick(
            "带 [关键字] 的 Touchstone 2.x 文件",
            "a Touchstone 2.x file with [keywords]",
        )
    };
    ParseError::new(match lang {
        Lang::Chinese => format!("检测到{what}（“[{keyword}]”），当前版本不支持。"),
        Lang::English => format!("Detected {what} (\"[{keyword}]\"), which this version does not support."),
    })
    .with_hint(lang.pick(
        "当前支持 Touchstone 1.x 的 .s1p 和 .s2p 单端模式 S 参数。可用 Touchstone 1.x 格式重新导出。",
        "Supported: Touchstone 1.x .s1p and .s2p single-ended S parameters. Re-export as Touchstone 1.x.",
    ))
}

fn parse_options(option_text: &str, line_number: usize, lang: Lang) -> Result<Options, ParseError> {
    let mut options = Options::default();
    let tokens: Vec<&str> = option_text.split_whitespace().collect();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].to_ascii_lowercase();
        match token.as_str() {
            "hz" | "khz" | "mhz" | "ghz" => {
                options.unit = FrequencyUnit::parse(&token).unwrap_or(FrequencyUnit::GHz);
            }
            "s" => options.parameter = Parameter::S,
            "y" => options.parameter = Parameter::Y,
            "z" => options.parameter = Parameter::Z,
            "g" => options.parameter = Parameter::G,
            "h" => options.parameter = Parameter::H,
            "ma" => options.format = Format::MagnitudeAngle,
            "db" => options.format = Format::DecibelAngle,
            "ri" => options.format = Format::RealImaginary,
            "r" => {
                index += 1;
                let value = tokens
                    .get(index)
                    .and_then(|t| parse_number(t))
                    .filter(|v| *v > 0.0 && v.is_finite());
                match value {
                    Some(reference) => options.reference = reference,
                    None => {
                        return Err(ParseError::new(match lang {
                            Lang::Chinese => {
                                format!("第 {line_number} 行的参考阻抗 R 后面缺少正数。")
                            }
                            Lang::English => format!(
                                "Line {line_number} has no positive number after the reference R."
                            ),
                        })
                        .with_hint(
                            lang.pick("示例：# GHz S MA R 50", "Example: # GHz S MA R 50"),
                        ));
                    }
                }
            }
            _ => {
                let original = tokens[index];
                return Err(ParseError::new(match lang {
                    Lang::Chinese => format!("第 {line_number} 行的选项“{original}”无法识别。"),
                    Lang::English => format!(
                        "Line {line_number} has an option that is not recognized: \"{original}\"."
                    ),
                })
                .with_hint(lang.pick(
                    "Touchstone 1.x 选项行示例：# GHz S MA R 50",
                    "Touchstone 1.x option line example: # GHz S MA R 50",
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn parse_number(token: &str) -> Option<f64> {
    token.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// Chooses between one and two ports from the width of the first data line.
///
/// Touchstone 1.x one-port rows always hold three values. Two-port rows hold
/// nine values, possibly wrapped; a wrapped first line still starts with at
/// least five values (frequency plus two pairs).
fn infer_ports_from_line(values: usize, line_number: usize, lang: Lang) -> Result<u8, ParseError> {
    match values {
        3 => Ok(1),
        9 | 5 | 7 => Ok(2),
        _ => Err(ParseError::new(match lang {
            Lang::Chinese => format!("第 {line_number} 行有 {values} 个数值，无法判断端口数。"),
            Lang::English => {
                format!("Line {line_number} has {values} values, so the port count is unclear.")
            }
        })
        .with_hint(lang.pick(
            "请使用 .s1p 或 .s2p 扩展名，或在粘贴时选择格式。",
            "Use a .s1p or .s2p extension, or pick the format when pasting.",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S1P_MA: &str = "! one-port\n# MHz S MA R 50\n100 0.5 45\n200 0.5 -45\n";

    fn first_trace(document: &Document) -> &Trace {
        &document.traces[0]
    }

    fn finite(sample: &Sample) -> Complex {
        match sample.impedance {
            Some(Impedance::Finite(z)) => z,
            other => panic!("expected finite impedance, got {other:?}"),
        }
    }

    #[test]
    fn reads_one_port_magnitude_angle() {
        let document = parse_touchstone(S1P_MA, "a.s1p", Some(1), Lang::Chinese).expect("parse");
        let trace = first_trace(&document);
        assert_eq!(trace.samples.len(), 2);
        assert_eq!(trace.samples[0].frequency_hz, Some(100e6));
        let expected = Impedance::from_reflection(Complex::from_polar_deg(0.5, 45.0), 50.0);
        assert!(finite(&trace.samples[0]).distance(expected.finite().expect("finite")) < 1e-9);
        assert_eq!(trace.source_z0, 50.0);
        assert_eq!(trace.origin, TraceOrigin::OnePortS);
    }

    #[test]
    fn ri_ma_and_db_forms_are_equivalent() {
        let gamma = Complex::from_polar_deg(0.6, 120.0);
        let ma = "# GHz S MA R 75\n1 0.6 120\n".to_owned();
        let db = format!("# GHz S DB R 75\n1 {} 120\n", 20.0 * 0.6f64.log10());
        let ri = format!("# GHz S RI R 75\n1 {} {}\n", gamma.re, gamma.im);
        let expected = Impedance::from_reflection(gamma, 75.0)
            .finite()
            .expect("finite");
        for text in [ma, db, ri] {
            let document = parse_touchstone(&text, "x.s1p", Some(1), Lang::Chinese).expect("parse");
            let z = finite(&first_trace(&document).samples[0]);
            assert!(z.distance(expected) < 1e-9, "{text}: {z:?} vs {expected:?}");
            assert_eq!(first_trace(&document).source_z0, 75.0);
        }
    }

    #[test]
    fn two_port_uses_s11_s21_s12_s22_order_and_skips_noise_block() {
        let text = "# GHz S RI R 50\n\
                    1 0.1 0.2  0.9 0.0  0.8 0.0  -0.3 0.4\n\
                    2 0.2 0.2  0.9 0.0\n  0.8 0.0  -0.3 -0.4\n\
                    ! noise parameters\n\
                    1 1.0 0.5 30 0.2\n\
                    2 1.2 0.4 40 0.3\n";
        let document = parse_touchstone(text, "amp.s2p", Some(2), Lang::Chinese).expect("parse");
        assert_eq!(document.traces.len(), 2);
        assert_eq!(document.traces[0].samples.len(), 2);
        let s11 = Impedance::from_reflection(Complex::new(0.2, 0.2), 50.0)
            .finite()
            .expect("finite");
        assert!(finite(&document.traces[0].samples[1]).distance(s11) < 1e-9);
        let s22 = Impedance::from_reflection(Complex::new(-0.3, -0.4), 50.0)
            .finite()
            .expect("finite");
        assert!(finite(&document.traces[1].samples[1]).distance(s22) < 1e-9);
        assert!(
            document
                .notes
                .iter()
                .any(|note| matches!(note, LoadNote::TouchstoneNoiseBlockSkipped))
        );
        assert_eq!(
            document.traces[1].origin,
            TraceOrigin::TwoPortS(PortVariant::S22)
        );
    }

    #[test]
    fn rejects_touchstone_two_point_zero() {
        let text = "[Version] 2.0\n# GHz S MA R 50\n[Number of Ports] 1\n1 0.5 0\n";
        let error = parse_touchstone(text, "x.s1p", Some(1), Lang::Chinese).expect_err("must fail");
        assert!(error.message.contains("2.0"));
        assert!(error.hint.is_some());
    }

    #[test]
    fn rejects_unknown_option_and_bad_numbers() {
        let error = parse_touchstone(
            "# GHz S MA R 50 XX\n1 0.5 0\n",
            "x.s1p",
            Some(1),
            Lang::Chinese,
        )
        .expect_err("must fail");
        assert!(error.message.contains("XX"));
        let error = parse_touchstone(
            "# GHz S MA R 50\n1 abc 0\n",
            "x.s1p",
            Some(1),
            Lang::Chinese,
        )
        .expect_err("must fail");
        assert!(error.message.contains("abc"));
        let error = parse_touchstone(
            "# GHz S MA R 50\n1 0.5 0\n",
            "x.s3p",
            Some(3),
            Lang::Chinese,
        )
        .expect_err("must fail");
        assert!(error.message.contains("3 端口"));
    }

    #[test]
    fn missing_option_line_uses_documented_defaults() {
        let document = parse_touchstone("1 1 0\n", "x.s1p", Some(1), Lang::Chinese).expect("parse");
        assert!(
            document
                .notes
                .iter()
                .any(|note| matches!(note, LoadNote::TouchstoneDefaultOptions))
        );
        assert_eq!(first_trace(&document).samples[0].frequency_hz, Some(1e9));
        assert_eq!(
            first_trace(&document).samples[0].impedance,
            Some(Impedance::Open)
        );
    }

    #[test]
    fn frequency_units_scale_to_hertz() {
        for (unit, expected) in [("Hz", 1.0), ("kHz", 1e3), ("MHz", 1e6), ("GHz", 1e9)] {
            let text = format!("# {unit} S RI R 50\n1 0 0\n");
            let document = parse_touchstone(&text, "x.s1p", Some(1), Lang::Chinese).expect("parse");
            assert_eq!(
                first_trace(&document).samples[0].frequency_hz,
                Some(expected)
            );
        }
    }

    #[test]
    fn one_port_z_and_y_parameters_are_denormalized() {
        let z_doc = parse_touchstone(
            "# MHz Z RI R 50\n1 0.5 0.6\n",
            "x.s1p",
            Some(1),
            Lang::Chinese,
        )
        .expect("parse");
        assert!(finite(&z_doc.traces[0].samples[0]).distance(Complex::new(25.0, 30.0)) < 1e-9);
        let y_doc = parse_touchstone("# MHz Y RI R 50\n1 2 0\n", "x.s1p", Some(1), Lang::Chinese)
            .expect("parse");
        assert!(finite(&y_doc.traces[0].samples[0]).distance(Complex::new(25.0, 0.0)) < 1e-9);
        let error = parse_touchstone(
            "# MHz Z RI R 50\n1 1 0 1 0 1 0 1 0\n",
            "x.s2p",
            Some(2),
            Lang::Chinese,
        )
        .expect_err("must fail");
        assert!(error.message.contains("Z"));
    }

    #[test]
    fn infers_port_count_for_pasted_text() {
        let one = "# GHz S RI R 50\n1 0 0\n2 0 0\n3 0 0\n";
        let document = parse_touchstone(one, "pasted", None, Lang::Chinese).expect("parse");
        assert_eq!(document.traces.len(), 1);
        assert_eq!(document.traces[0].samples.len(), 3);
        let two = "# GHz S RI R 50\n1 0.1 0 0.9 0 0.9 0 0.2 0\n2 0.1 0 0.9 0\n 0.9 0 0.2 0\n";
        let document = parse_touchstone(two, "pasted", None, Lang::Chinese).expect("parse");
        assert_eq!(document.traces.len(), 2);
        assert_eq!(document.traces[0].samples.len(), 2);
    }
}
