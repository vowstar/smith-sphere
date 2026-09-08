//! Delimited-table reader for frequency, resistance, and reactance columns,
//! with optional reflection-coefficient columns.
//!
//! Loading happens in two steps so the interface can ask for information the
//! file does not carry, such as the frequency unit, instead of guessing.

use super::{FrequencyUnit, ParseError};
use crate::complex::Complex;
use crate::dataset::{CsvColumns, DataSource, Document, LoadNote, Sample, Trace, TraceOrigin};
use crate::i18n::Lang;
use crate::impedance::Impedance;

/// Column roles recovered from a table: frequency column, declared unit,
/// value columns, and a declared reference impedance.
type ColumnAssignment = (
    Option<usize>,
    Option<FrequencyUnit>,
    ValueColumns,
    Option<f64>,
);

/// Which value columns were recognized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueColumns {
    /// Resistance and reactance in ohms.
    Impedance { resistance: usize, reactance: usize },
    /// Reflection coefficient as real and imaginary parts.
    ReflectionRealImaginary { real: usize, imaginary: usize },
    /// Reflection coefficient as magnitude (linear or dB) and phase.
    ReflectionPolar {
        magnitude: usize,
        phase: usize,
        decibel: bool,
        radians: bool,
    },
}

impl ValueColumns {
    #[must_use]
    pub fn columns(self) -> CsvColumns {
        match self {
            Self::Impedance { .. } => CsvColumns::Impedance,
            Self::ReflectionRealImaginary { .. } => CsvColumns::ReflectionRealImaginary,
            Self::ReflectionPolar { decibel: true, .. } => CsvColumns::ReflectionDecibelPhase,
            Self::ReflectionPolar { .. } => CsvColumns::ReflectionMagnitudePhase,
        }
    }

    #[must_use]
    pub fn describe(self, lang: Lang) -> &'static str {
        self.columns().describe(lang)
    }

    #[must_use]
    pub fn is_reflection(self) -> bool {
        !matches!(self, Self::Impedance { .. })
    }
}

/// Result of inspecting a table before it is turned into a document.
#[derive(Clone, Debug, PartialEq)]
pub struct CsvLayout {
    pub file_name: String,
    pub header: Option<Vec<String>>,
    pub frequency_column: Option<usize>,
    /// Unit declared in the header, if any.
    pub frequency_unit: Option<FrequencyUnit>,
    pub values: ValueColumns,
    /// Reference impedance declared by a `Z0` column.
    pub declared_z0: Option<f64>,
    pub rows: Vec<Vec<String>>,
}

impl CsvLayout {
    /// Human-readable summary of what was found.
    #[must_use]
    pub fn summary(&self, lang: Lang) -> String {
        let frequency = match (self.frequency_column, self.frequency_unit) {
            (Some(_), Some(unit)) => format!(
                "{}{}{}",
                lang.pick("频率列（", "frequency column ("),
                unit.label(),
                lang.pick("）", ")")
            ),
            (Some(_), None) => lang
                .pick("频率列（未声明单位）", "frequency column (no unit stated)")
                .to_owned(),
            (None, _) => lang.pick("无频率列", "no frequency column").to_owned(),
        };
        let z0 = match self.declared_z0 {
            Some(z0) => format!(
                "{}{} Ω",
                lang.pick("，Z0 列 ", ", Z0 column "),
                crate::format::significant(z0, 6)
            ),
            None => String::new(),
        };
        match lang {
            Lang::Chinese => format!(
                "找到 {frequency}、{}{z0}，共 {} 行数据",
                self.values.describe(lang),
                self.rows.len()
            ),
            Lang::English => format!(
                "found {frequency}, {}{z0}, {} data rows",
                self.values.describe(lang),
                self.rows.len()
            ),
        }
    }

    /// True when the interface must ask for a frequency unit.
    #[must_use]
    pub fn needs_unit(&self) -> bool {
        self.frequency_column.is_some() && self.frequency_unit.is_none()
    }

    /// True when the interface must ask for a reference impedance.
    #[must_use]
    pub fn needs_reference(&self) -> bool {
        self.values.is_reflection() && self.declared_z0.is_none()
    }
}

/// Inspects delimited text and identifies its columns.
///
/// # Errors
///
/// Returns an error naming the missing or ambiguous columns.
pub fn inspect_csv(text: &str, file_name: &str, lang: Lang) -> Result<CsvLayout, ParseError> {
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.trim_start_matches('\u{feff}').trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
        .collect();
    if lines.is_empty() {
        return Err(
            ParseError::new(lang.pick("表格是空的。", "The table is empty.")).with_hint(lang.pick(
                "需要至少一行 frequency、R、X 数据。",
                "At least one frequency, R, X data row is required.",
            )),
        );
    }
    let delimiter = detect_delimiter(lines[0]);
    let split = |line: &str| -> Vec<String> {
        match delimiter {
            Some(c) => line
                .split(c)
                .map(|cell| cell.trim().trim_matches('"').to_owned())
                .collect(),
            None => line
                .split_whitespace()
                .map(|cell| cell.trim_matches('"').to_owned())
                .collect(),
        }
    };
    let first = split(lines[0]);
    let has_header = first.iter().any(|cell| cell.parse::<f64>().is_err());
    let (header, data_lines) = if has_header {
        (Some(first), &lines[1..])
    } else {
        (None, &lines[..])
    };
    let rows: Vec<Vec<String>> = data_lines.iter().map(|line| split(line)).collect();
    if rows.is_empty() {
        return Err(ParseError::new(lang.pick(
            "表格只有表头，没有数据行。",
            "The table has a header but no data rows.",
        )));
    }

    let (frequency_column, frequency_unit, values, declared_z0) = match &header {
        Some(header) => classify_header(header, &rows, lang)?,
        None => classify_headerless(rows[0].len(), lang)?,
    };

    Ok(CsvLayout {
        file_name: file_name.to_owned(),
        header,
        frequency_column,
        frequency_unit,
        values,
        declared_z0,
        rows,
    })
}

/// Builds a document from an inspected layout and the answers the interface
/// collected.
///
/// # Errors
///
/// Returns an error when a required unit or reference impedance is missing.
pub fn build_csv_document(
    layout: &CsvLayout,
    frequency_unit: Option<FrequencyUnit>,
    reference_z0: Option<f64>,
    lang: Lang,
) -> Result<Document, ParseError> {
    let unit = match (
        layout.frequency_column,
        layout.frequency_unit.or(frequency_unit),
    ) {
        (Some(_), Some(unit)) => Some(unit),
        (Some(_), None) => {
            return Err(ParseError::new(lang.pick(
                "表格没有声明频率单位。",
                "The table does not state a frequency unit.",
            ))
            .with_hint(lang.pick(
                "请选择 Hz、kHz、MHz 或 GHz。",
                "Choose Hz, kHz, MHz, or GHz.",
            )));
        }
        (None, _) => None,
    };
    let z0 = match layout.declared_z0.or(reference_z0) {
        Some(z0) if z0 > 0.0 && z0.is_finite() => z0,
        _ if layout.values.is_reflection() => {
            return Err(ParseError::new(lang.pick(
                "反射系数需要一个正的参考阻抗 Z0。",
                "Reflection coefficients need a positive reference impedance Z0.",
            )));
        }
        _ => reference_z0
            .filter(|z0| z0.is_finite() && *z0 > 0.0)
            .unwrap_or(50.0),
    };

    let mut notes = Vec::new();
    let mut skipped = 0usize;
    let samples: Vec<Sample> = layout
        .rows
        .iter()
        .map(|row| {
            let frequency = match (layout.frequency_column, unit) {
                (Some(column), Some(unit)) => match cell_number(row, column) {
                    Some(value) => Some(value * unit.multiplier()),
                    None => {
                        skipped += 1;
                        return Sample::gap(None);
                    }
                },
                _ => None,
            };
            let impedance = match layout.values {
                ValueColumns::Impedance {
                    resistance,
                    reactance,
                } => match (cell_number(row, resistance), cell_number(row, reactance)) {
                    (Some(r), Some(x)) => Some(Impedance::new(r, x)),
                    _ => None,
                },
                ValueColumns::ReflectionRealImaginary { real, imaginary } => {
                    match (cell_number(row, real), cell_number(row, imaginary)) {
                        (Some(re), Some(im)) => {
                            Some(Impedance::from_reflection(Complex::new(re, im), z0))
                        }
                        _ => None,
                    }
                }
                ValueColumns::ReflectionPolar {
                    magnitude,
                    phase,
                    decibel,
                    radians,
                } => match (cell_number(row, magnitude), cell_number(row, phase)) {
                    (Some(m), Some(p)) => {
                        let magnitude = if decibel { 10f64.powf(m / 20.0) } else { m };
                        let phase_deg = if radians { p.to_degrees() } else { p };
                        Some(Impedance::from_reflection(
                            Complex::from_polar_deg(magnitude, phase_deg),
                            z0,
                        ))
                    }
                    _ => None,
                },
            };
            match impedance {
                Some(impedance) => Sample {
                    frequency_hz: frequency,
                    impedance: Some(impedance),
                },
                None => {
                    skipped += 1;
                    Sample::gap(frequency)
                }
            }
        })
        .collect();

    if samples.iter().all(|sample| sample.impedance.is_none()) {
        return Err(ParseError::new(lang.pick(
            "没有一行数据能读成数值。",
            "No data row could be read as numbers.",
        ))
        .with_hint(lang.pick(
            "检查小数点、分隔符和列顺序。",
            "Check the decimal point, the delimiter, and the column order.",
        )));
    }
    if skipped > 0 {
        notes.push(LoadNote::CsvUnreadableRows(skipped));
    }
    if layout.frequency_column.is_none() {
        notes.push(LoadNote::CsvNoFrequencyColumn);
    }

    let (origin, label) = match layout.values {
        ValueColumns::Impedance { .. } => (TraceOrigin::ImpedanceColumns, "Z"),
        _ => (TraceOrigin::ReflectionColumns, "Γ"),
    };
    let mut document = Document::new(
        &layout.file_name,
        DataSource::Csv {
            file_name: layout.file_name.clone(),
            columns: layout.values.columns(),
            frequency_unit: unit,
            reference_z0: z0,
        },
        vec![Trace::new(label, samples, z0, origin)],
    );
    document.notes = notes;
    Ok(document)
}

fn detect_delimiter(line: &str) -> Option<char> {
    [',', '\t', ';', '|']
        .into_iter()
        .map(|c| (c, line.matches(c).count()))
        .filter(|(_, count)| *count > 0)
        .max_by_key(|(_, count)| *count)
        .map(|(c, _)| c)
}

fn cell_number(row: &[String], column: usize) -> Option<f64> {
    row.get(column)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Frequency,
    Resistance,
    Reactance,
    GammaReal,
    GammaImaginary,
    GammaMagnitude,
    GammaDecibel,
    GammaPhase,
    MagnitudeOnly,
    Reference,
    Unknown,
}

fn classify_header(
    header: &[String],
    rows: &[Vec<String>],
    lang: Lang,
) -> Result<ColumnAssignment, ParseError> {
    let roles: Vec<(usize, Role, &str)> = header
        .iter()
        .enumerate()
        .map(|(index, name)| (index, role_of(name), name.as_str()))
        .collect();
    let find = |role: Role| {
        roles
            .iter()
            .find(|(_, r, _)| *r == role)
            .map(|(index, _, _)| *index)
    };

    let frequency_column = find(Role::Frequency);
    let frequency_unit = frequency_column.and_then(|index| FrequencyUnit::find_in(&header[index]));
    let declared_z0 = find(Role::Reference)
        .and_then(|index| cell_number(&rows[0], index))
        .filter(|z0| *z0 > 0.0);

    let resistance = find(Role::Resistance);
    let reactance = find(Role::Reactance);
    let values = match (resistance, reactance) {
        (Some(resistance), Some(reactance)) => ValueColumns::Impedance {
            resistance,
            reactance,
        },
        (Some(_), None) => {
            return Err(ParseError::new(lang.pick(
                "找到了电阻 R 列，还需要电抗 X 列。",
                "Found a resistance R column but still need a reactance X column.",
            ))
            .with_hint(lang.pick(
                "表头可写为 X、reactance 或 Im(Z)。",
                "The header can read X, reactance, or Im(Z).",
            )));
        }
        (None, Some(_)) => {
            return Err(ParseError::new(lang.pick(
                "找到了电抗 X 列，还需要电阻 R 列。",
                "Found a reactance X column but still need a resistance R column.",
            ))
            .with_hint(lang.pick(
                "表头可写为 R、resistance 或 Re(Z)。",
                "The header can read R, resistance, or Re(Z).",
            )));
        }
        (None, None) => classify_reflection_columns(&roles, header, lang)?,
    };
    Ok((frequency_column, frequency_unit, values, declared_z0))
}

fn classify_reflection_columns(
    roles: &[(usize, Role, &str)],
    header: &[String],
    lang: Lang,
) -> Result<ValueColumns, ParseError> {
    let find = |role: Role| {
        roles
            .iter()
            .find(|(_, r, _)| *r == role)
            .map(|(index, _, _)| *index)
    };
    let phase = find(Role::GammaPhase);
    let magnitude = find(Role::GammaMagnitude);
    let decibel = find(Role::GammaDecibel);
    if let (Some(real), Some(imaginary)) = (find(Role::GammaReal), find(Role::GammaImaginary)) {
        return Ok(ValueColumns::ReflectionRealImaginary { real, imaginary });
    }
    if let Some(phase) = phase {
        let radians = header[phase].to_ascii_lowercase().contains("rad");
        if let Some(magnitude) = magnitude {
            return Ok(ValueColumns::ReflectionPolar {
                magnitude,
                phase,
                decibel: false,
                radians,
            });
        }
        if let Some(magnitude) = decibel {
            return Ok(ValueColumns::ReflectionPolar {
                magnitude,
                phase,
                decibel: true,
                radians,
            });
        }
        return Err(ParseError::new(lang.pick(
            "找到了相位列，还需要幅度列。",
            "Found a phase column but still need a magnitude column.",
        ))
        .with_hint(lang.pick(
            "表头可写为 mag、|S11| 或 dB。",
            "The header can read mag, |S11|, or dB.",
        )));
    }
    let magnitude_only = magnitude.or(decibel).or(find(Role::MagnitudeOnly));
    if let Some(index) = magnitude_only {
        let has_frequency = find(Role::Frequency).is_some();
        let name = &header[index];
        let message = match lang {
            Lang::Chinese => {
                let frequency_part = if has_frequency { "频率列和" } else { "" };
                format!(
                    "找到了{frequency_part}幅度列“{name}”，还需要相位列。只有幅度、VSWR 或回波损耗时缺少相位，无法确定唯一位置。"
                )
            }
            Lang::English => {
                let frequency_part = if has_frequency {
                    "a frequency column and "
                } else {
                    ""
                };
                format!(
                    "Found {frequency_part}a magnitude column \"{name}\" but still need a phase column. Magnitude, VSWR, or return loss without phase cannot fix a unique position."
                )
            }
        };
        return Err(ParseError::new(message).with_hint(lang.pick(
            "请补充 phase、angle 或 deg 列，或者改为提供 R、X 阻抗列。",
            "Add a phase, angle, or deg column, or provide R, X impedance columns instead.",
        )));
    }
    let names = header.join(lang.pick("、", ", "));
    Err(ParseError::new(match lang {
        Lang::Chinese => format!("无法识别表头：{names}。"),
        Lang::English => format!("Could not recognize the header: {names}."),
    })
    .with_hint(lang.pick(
        "需要 frequency、R、X 三列，或反射系数的实部/虚部、幅度/相位列。",
        "Provide frequency, R, X columns, or reflection real/imaginary or magnitude/phase columns.",
    )))
}

fn classify_headerless(columns: usize, lang: Lang) -> Result<ColumnAssignment, ParseError> {
    match columns {
        3 => Ok((
            Some(0),
            None,
            ValueColumns::Impedance {
                resistance: 1,
                reactance: 2,
            },
            None,
        )),
        2 => Err(ParseError::new(lang.pick(
            "表格只有两列且没有表头。",
            "The table has only two columns and no header.",
        ))
        .with_hint(lang.pick(
            "请添加表头（例如 frequency,R,X），或补充频率列。",
            "Add a header such as frequency,R,X, or add a frequency column.",
        ))),
        _ => Err(ParseError::new(match lang {
            Lang::Chinese => format!("表格有 {columns} 列但没有表头，无法确定各列含义。"),
            Lang::English => {
                format!(
                    "The table has {columns} columns and no header, so the columns are ambiguous."
                )
            }
        })
        .with_hint(lang.pick(
            "请在第一行添加表头，例如 frequency(MHz),R,X。",
            "Add a header on the first line, such as frequency(MHz),R,X.",
        ))),
    }
}

/// Normalizes a header cell: lowercase, units and decorations removed.
fn normalize(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let mut stripped = String::new();
    let mut depth = 0usize;
    for c in lower.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => stripped.push(c),
            _ => {}
        }
    }
    let stripped = stripped
        .replace("ohms", "")
        .replace("ohm", "")
        .replace('Ω', "")
        .replace("ghz", "")
        .replace("mhz", "")
        .replace("khz", "")
        .replace("hz", "");
    stripped
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn role_of(name: &str) -> Role {
    let key = normalize(name);
    let lower = name.to_ascii_lowercase();
    match key.as_str() {
        "f" | "freq" | "frequency" => return Role::Frequency,
        "r" | "re" | "real" | "res" | "resistance" | "rez" | "zre" | "zreal" | "realz" | "zr"
        | "rz" => {
            return Role::Resistance;
        }
        "x" | "im" | "imag" | "imaginary" | "reactance" | "imz" | "zim" | "zimag" | "imagz"
        | "zx" | "xz" => {
            return Role::Reactance;
        }
        "z0" | "zref" | "zo" | "ref" | "reference" => return Role::Reference,
        "vswr" | "swr" | "rl" | "returnloss" | "mag" | "magnitude" | "abs" => {
            return Role::MagnitudeOnly;
        }
        "db" => return Role::GammaDecibel,
        "phase" | "ang" | "angle" | "deg" | "arg" | "ph" | "theta" => return Role::GammaPhase,
        _ => {}
    }
    let prefixes = ["gamma", "s11", "s22", "refl", "reflection", "s"];
    for prefix in prefixes {
        if let Some(rest) = key.strip_prefix(prefix) {
            return match rest {
                "re" | "real" => Role::GammaReal,
                "im" | "imag" | "imaginary" => Role::GammaImaginary,
                "" | "mag" | "magnitude" | "abs" | "m" => Role::GammaMagnitude,
                "db" | "logmag" => Role::GammaDecibel,
                "ang" | "angle" | "phase" | "deg" | "arg" | "ph" | "rad" => Role::GammaPhase,
                _ => Role::Unknown,
            };
        }
        if let Some(rest) = key.strip_suffix(prefix) {
            return match rest {
                "re" | "real" => Role::GammaReal,
                "im" | "imag" | "imaginary" => Role::GammaImaginary,
                "mag" | "magnitude" | "abs" => Role::GammaMagnitude,
                "db" => Role::GammaDecibel,
                "ang" | "angle" | "phase" | "deg" | "arg" => Role::GammaPhase,
                _ => Role::Unknown,
            };
        }
    }
    if lower.contains("|s11|")
        || lower.contains("|s22|")
        || lower.contains("|gamma|")
        || lower.contains("|γ|")
    {
        return Role::GammaMagnitude;
    }
    Role::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finite(sample: &Sample) -> Complex {
        match sample.impedance {
            Some(Impedance::Finite(z)) => z,
            other => panic!("expected finite impedance, got {other:?}"),
        }
    }

    #[test]
    fn reads_frequency_resistance_reactance_with_unit_in_header() {
        let text = "frequency (MHz),R,X\n100,25,30\n200,50,0\n";
        let layout = inspect_csv(text, "a.csv", Lang::Chinese).expect("inspect");
        assert_eq!(layout.frequency_unit, Some(FrequencyUnit::MHz));
        assert!(!layout.needs_unit());
        let document = build_csv_document(&layout, None, None, Lang::Chinese).expect("build");
        let trace = &document.traces[0];
        assert_eq!(trace.samples[0].frequency_hz, Some(100e6));
        assert_eq!(finite(&trace.samples[0]), Complex::new(25.0, 30.0));
        assert_eq!(trace.source_z0, 50.0);
    }

    #[test]
    fn asks_for_a_unit_when_the_header_has_none() {
        let text = "freq;R;X\n1;10;20\n";
        let layout = inspect_csv(text, "a.csv", Lang::Chinese).expect("inspect");
        assert!(layout.needs_unit());
        let error = build_csv_document(&layout, None, None, Lang::Chinese).expect_err("must ask");
        assert!(error.message.contains("频率单位"));
        let document = build_csv_document(&layout, Some(FrequencyUnit::GHz), None, Lang::Chinese)
            .expect("build");
        assert_eq!(document.traces[0].samples[0].frequency_hz, Some(1e9));
    }

    #[test]
    fn headerless_three_columns_are_frequency_r_x() {
        let layout =
            inspect_csv("1\t10\t20\n2\t11\t21\n", "raw.txt", Lang::Chinese).expect("inspect");
        assert_eq!(layout.frequency_column, Some(0));
        assert!(layout.needs_unit());
        assert_eq!(
            layout.values,
            ValueColumns::Impedance {
                resistance: 1,
                reactance: 2
            }
        );
    }

    #[test]
    fn magnitude_without_phase_explains_what_is_missing() {
        let error =
            inspect_csv("freq(GHz),|S11|\n1,0.5\n", "m.csv", Lang::Chinese).expect_err("must fail");
        assert!(error.message.contains("相位"), "{error}");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("phase"))
        );
        let error =
            inspect_csv("freq(GHz),vswr\n1,1.5\n", "m.csv", Lang::Chinese).expect_err("must fail");
        assert!(error.message.contains("缺少相位"));
    }

    #[test]
    fn reflection_columns_need_a_reference_and_recover_impedance() {
        let text = "freq(GHz),s11_mag,s11_deg\n1,0.2,90\n";
        let layout = inspect_csv(text, "g.csv", Lang::Chinese).expect("inspect");
        assert!(layout.needs_reference());
        let document = build_csv_document(&layout, None, Some(50.0), Lang::Chinese).expect("build");
        let expected = Impedance::from_reflection(Complex::from_polar_deg(0.2, 90.0), 50.0);
        assert!(
            finite(&document.traces[0].samples[0]).distance(expected.finite().expect("finite"))
                < 1e-9
        );
        let text = "freq(GHz),Re(S11),Im(S11),Z0\n1,0.1,0.1,75\n";
        let layout = inspect_csv(text, "g.csv", Lang::Chinese).expect("inspect");
        assert_eq!(layout.declared_z0, Some(75.0));
        assert!(!layout.needs_reference());
        let document = build_csv_document(&layout, None, None, Lang::Chinese).expect("build");
        assert_eq!(document.traces[0].source_z0, 75.0);
    }

    #[test]
    fn unreadable_rows_become_gaps() {
        let text = "f(Hz),R,X\n1,1,1\n2,nan,1\n3,3,3\n";
        let layout = inspect_csv(text, "gap.csv", Lang::Chinese).expect("inspect");
        let document = build_csv_document(&layout, None, None, Lang::Chinese).expect("build");
        assert!(document.traces[0].samples[1].impedance.is_none());
        assert!(
            document
                .notes
                .iter()
                .any(|note| matches!(note, LoadNote::CsvUnreadableRows(_)))
        );
    }

    #[test]
    fn rejects_partial_impedance_columns() {
        let error = inspect_csv("f(Hz),R\n1,1\n", "x.csv", Lang::Chinese).expect_err("must fail");
        assert!(error.message.contains("电抗"));
        let error = inspect_csv("a,b,c\n1,2,3\n", "x.csv", Lang::Chinese).expect_err("must fail");
        assert!(error.message.contains("无法识别表头"));
    }
}
