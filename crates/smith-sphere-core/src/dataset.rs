//! Loaded measurement or model data: samples, traces, and their provenance.

use crate::i18n::Lang;
use crate::impedance::Impedance;
use crate::parse::FrequencyUnit;
use serde::{Deserialize, Serialize};

/// One frequency point. A missing impedance marks a gap that must not be
/// bridged by a line.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Frequency in hertz, `None` for data without a frequency axis.
    pub frequency_hz: Option<f64>,
    /// Physical impedance in ohms, `None` for an unreadable row.
    pub impedance: Option<Impedance>,
}

impl Sample {
    #[must_use]
    pub const fn new(frequency_hz: f64, impedance: Impedance) -> Self {
        Self {
            frequency_hz: Some(frequency_hz),
            impedance: Some(impedance),
        }
    }

    #[must_use]
    pub const fn gap(frequency_hz: Option<f64>) -> Self {
        Self {
            frequency_hz,
            impedance: None,
        }
    }
}

/// Which two-port reflection a trace was derived from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortVariant {
    S11,
    S22,
}

impl PortVariant {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::S11 => "S11",
            Self::S22 => "S22",
        }
    }

    #[must_use]
    pub fn port_number(self) -> u8 {
        match self {
            Self::S11 => 1,
            Self::S22 => 2,
        }
    }
}

/// How the physical impedance of a trace was obtained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TraceOrigin {
    /// Entered as `R + jX` by hand.
    ManualImpedance,
    /// Entered as a complex reflection coefficient by hand.
    ManualReflection,
    /// Frequency, R, X columns from a table.
    ImpedanceColumns,
    /// Reflection-coefficient columns from a table.
    ReflectionColumns,
    /// Touchstone one-port S parameter.
    OnePortS,
    /// Touchstone one-port normalized Z parameter.
    OnePortZ,
    /// Touchstone one-port normalized Y parameter.
    OnePortY,
    /// Touchstone two-port reflection at one port with the other port matched.
    TwoPortS(PortVariant),
    /// Analytic demonstration model.
    Demo,
}

/// A sequence of samples sharing one source reference impedance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// Short label such as `S11` or `Z`.
    pub label: String,
    pub samples: Vec<Sample>,
    /// Reference impedance declared by the source, used to recover physical
    /// impedance from S parameters. Independent of the plotting `Z0`.
    pub source_z0: f64,
    pub origin: TraceOrigin,
}

impl Trace {
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        samples: Vec<Sample>,
        source_z0: f64,
        origin: TraceOrigin,
    ) -> Self {
        Self {
            label: label.into(),
            samples,
            source_z0,
            origin,
        }
    }

    #[must_use]
    pub fn valid_sample_count(&self) -> usize {
        self.samples
            .iter()
            .filter(|sample| sample.impedance.is_some())
            .count()
    }

    #[must_use]
    pub fn has_frequency_axis(&self) -> bool {
        self.samples.len() > 1
            && self
                .samples
                .iter()
                .all(|sample| sample.frequency_hz.is_some())
    }
}

/// One of the built-in demonstration datasets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DemoKind {
    SeriesRlc,
    NegativeResistance,
    BoundaryCrossing,
    Landmarks,
}

impl DemoKind {
    /// Short title, without the demonstration prefix.
    #[must_use]
    pub fn title(self, lang: Lang) -> &'static str {
        match self {
            Self::SeriesRlc => lang.pick("串联 RLC 扫频", "Series RLC sweep"),
            Self::NegativeResistance => lang.pick("负电阻器件", "Negative-resistance device"),
            Self::BoundaryCrossing => lang.pick("跨越 R = 0 的轨迹", "R = 0 crossing"),
            Self::Landmarks => lang.pick("特征点集合", "Landmark points"),
        }
    }

    /// One-line description shown next to the title.
    #[must_use]
    pub fn description(self, lang: Lang) -> &'static str {
        match self {
            Self::SeriesRlc => lang.pick(
                "R = 25 Ω、L = 10 nH、C = 5 pF 串联，100 MHz 至 2 GHz，正电阻区。",
                "R = 25 Ω, L = 10 nH, C = 5 pF in series, 100 MHz to 2 GHz, positive resistance.",
            ),
            Self::NegativeResistance => lang.pick(
                "理想数学模型：−40 Ω 与 2 pF 并联后串联 2 nH，全程负电阻。",
                "Ideal model: −40 Ω in parallel with 2 pF, then 2 nH in series, negative throughout.",
            ),
            Self::BoundaryCrossing => lang.pick(
                "演示数据：电阻从 −30 Ω 线性变到 +30 Ω，轨迹穿过共享边界。",
                "Demo data: resistance sweeps −30 Ω to +30 Ω, crossing the shared boundary.",
            ),
            Self::Landmarks => lang.pick(
                "Z = 0、∞、±Z0、±jZ0 六个特征点（Z0 = 50 Ω），用于核对位置。",
                "The six landmarks Z = 0, ∞, ±Z0, ±jZ0 (Z0 = 50 Ω) for checking positions.",
            ),
        }
    }
}

/// Which value columns a table provided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsvColumns {
    Impedance,
    ReflectionRealImaginary,
    ReflectionMagnitudePhase,
    ReflectionDecibelPhase,
}

impl CsvColumns {
    #[must_use]
    pub fn describe(self, lang: Lang) -> &'static str {
        match self {
            Self::Impedance => lang.pick("R、X 阻抗列", "R, X impedance columns"),
            Self::ReflectionRealImaginary => lang.pick(
                "反射系数实部、虚部列",
                "reflection real and imaginary columns",
            ),
            Self::ReflectionMagnitudePhase => lang.pick(
                "反射系数幅度与相位列",
                "reflection magnitude and phase columns",
            ),
            Self::ReflectionDecibelPhase => lang.pick(
                "反射系数 dB 幅度与相位列",
                "reflection dB magnitude and phase columns",
            ),
        }
    }
}

/// The technical summary of a parsed Touchstone file, kept structured so it can
/// be shown in either language. The parameter, format, and unit labels are
/// standard notation that is the same in both languages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TouchstoneSummary {
    pub ports: u8,
    pub parameter: String,
    pub format: String,
    pub unit: String,
    pub reference_z0: f64,
    pub points: usize,
}

/// Where a document came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DataSource {
    Manual,
    /// Built-in demonstration data.
    Demo(DemoKind),
    Csv {
        file_name: String,
        columns: CsvColumns,
        frequency_unit: Option<FrequencyUnit>,
        reference_z0: f64,
    },
    Touchstone {
        file_name: String,
        summary: TouchstoneSummary,
    },
}

impl DataSource {
    #[must_use]
    pub fn kind_label(&self, lang: Lang) -> &'static str {
        match self {
            Self::Manual => lang.pick("手动输入", "Manual"),
            Self::Demo(_) => lang.pick("演示数据", "Demo data"),
            Self::Csv { .. } => lang.pick("CSV 表格", "CSV table"),
            Self::Touchstone { .. } => "Touchstone",
        }
    }

    #[must_use]
    pub fn detail(&self, lang: Lang) -> String {
        match self {
            Self::Manual => lang.pick("手动输入", "Manual entry").to_owned(),
            Self::Demo(kind) => kind.description(lang).to_owned(),
            Self::Csv {
                file_name,
                columns,
                frequency_unit,
                reference_z0,
            } => {
                let unit = frequency_unit
                    .map(|unit| {
                        format!(
                            "{}{}",
                            lang.pick("，频率单位 ", ", frequency unit "),
                            unit.label()
                        )
                    })
                    .unwrap_or_default();
                format!(
                    "{file_name}: {}{unit}{}{} Ω",
                    columns.describe(lang),
                    lang.pick("，参考阻抗 ", ", reference "),
                    crate::format::significant(*reference_z0, 6)
                )
            }
            Self::Touchstone { file_name, summary } => {
                format!("{file_name}: {}", summary_text(summary, lang))
            }
        }
    }

    #[must_use]
    pub fn is_demo(&self) -> bool {
        matches!(self, Self::Demo(_))
    }
}

fn summary_text(summary: &TouchstoneSummary, lang: Lang) -> String {
    let TouchstoneSummary {
        ports,
        parameter,
        format,
        unit,
        reference_z0,
        points,
    } = summary;
    let z0 = crate::format::significant(*reference_z0, 6);
    match lang {
        Lang::Chinese => format!(
            "{ports} 端口 {parameter} 参数，{format} 格式，频率单位 {unit}，参考阻抗 {z0} Ω，{points} 个频点"
        ),
        Lang::English => format!(
            "{ports}-port {parameter} parameters, {format} format, frequency unit {unit}, reference {z0} Ω, {points} points"
        ),
    }
}

/// A non-fatal note produced while loading a file, kept structured so it shows
/// in the current language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadNote {
    CsvUnreadableRows(usize),
    CsvNoFrequencyColumn,
    TouchstoneSecondOptionLine(usize),
    TouchstoneOptionAfterData(usize),
    TouchstoneNoiseBlockSkipped,
    TouchstoneFrequencyNotMonotonic(usize),
    TouchstoneDefaultOptions,
    LandmarksHaveNoFrequencyAxis,
}

impl LoadNote {
    #[must_use]
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::CsvUnreadableRows(count) => match lang {
                Lang::Chinese => format!("{count} 行无法读取，已作为断点处理，不会连线。"),
                Lang::English => {
                    format!("{count} rows could not be read and became breaks, not connected.")
                }
            },
            Self::CsvNoFrequencyColumn => lang
                .pick(
                    "表格没有频率列，各点按行顺序显示，不提供频率滑块。",
                    "The table has no frequency column, so points follow row order with no slider.",
                )
                .to_owned(),
            Self::TouchstoneSecondOptionLine(line) => match lang {
                Lang::Chinese => format!("第 {line} 行出现了第二个选项行，已忽略。"),
                Lang::English => format!("Line {line} has a second option line, which was ignored."),
            },
            Self::TouchstoneOptionAfterData(line) => match lang {
                Lang::Chinese => format!("第 {line} 行的选项行出现在数据之后，已忽略。"),
                Lang::English => {
                    format!("The option line on line {line} came after data and was ignored.")
                }
            },
            Self::TouchstoneNoiseBlockSkipped => lang
                .pick(
                    "检测到频率回落，其后的噪声参数段已忽略。",
                    "The frequency stepped back, so the trailing noise block was skipped.",
                )
                .to_owned(),
            Self::TouchstoneFrequencyNotMonotonic(line) => match lang {
                Lang::Chinese => format!("第 {line} 行的频率小于前一行，数据并非单调递增。"),
                Lang::English => {
                    format!("Line {line} has a lower frequency than the previous row, so the data is not increasing.")
                }
            },
            Self::TouchstoneDefaultOptions => lang
                .pick(
                    "文件没有 # 选项行，按 Touchstone 默认 GHz S MA R 50 解析。",
                    "The file has no # option line, so the Touchstone default GHz S MA R 50 is used.",
                )
                .to_owned(),
            Self::LandmarksHaveNoFrequencyAxis => lang
                .pick(
                    "特征点没有频率轴，因此不显示频率滑块。",
                    "Landmark points have no frequency axis, so no slider is shown.",
                )
                .to_owned(),
        }
    }
}

/// One loaded file, manual entry, or demonstration set. A document can hold
/// several trace variants (for example `S11` and `S22`) of which one is shown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub source: DataSource,
    pub traces: Vec<Trace>,
    /// When set, only this trace is shown and the interface offers a switch
    /// between the variants (for example `S11` and `S22`). When `None`, all
    /// traces are shown together.
    pub variant_selection: Option<usize>,
    /// Non-fatal notes produced while loading.
    pub notes: Vec<LoadNote>,
}

impl Document {
    /// The document title in the current language. Demonstration documents show
    /// a translated title; loaded files keep their file name.
    #[must_use]
    pub fn display_name(&self, lang: Lang) -> String {
        match &self.source {
            DataSource::Demo(kind) => {
                format!("{}{}", lang.pick("演示：", "Demo: "), kind.title(lang))
            }
            DataSource::Manual => lang.pick("手动输入", "Manual entry").to_owned(),
            _ => self.name.clone(),
        }
    }

    #[must_use]
    pub fn new(name: impl Into<String>, source: DataSource, traces: Vec<Trace>) -> Self {
        Self {
            name: name.into(),
            source,
            traces,
            variant_selection: None,
            notes: Vec::new(),
        }
    }

    /// Traces that are currently displayed, with their indices.
    pub fn visible_traces(&self) -> impl Iterator<Item = (usize, &Trace)> {
        self.traces.iter().enumerate().filter(move |(index, _)| {
            self.variant_selection
                .is_none_or(|selected| selected == *index)
        })
    }

    /// True when the document offers a choice between trace variants.
    #[must_use]
    pub fn has_variants(&self) -> bool {
        self.variant_selection.is_some() && self.traces.len() > 1
    }

    pub fn select_variant(&mut self, index: usize) {
        if index < self.traces.len() && self.variant_selection.is_some() {
            self.variant_selection = Some(index);
        }
    }

    /// The trace the frequency slider follows: the selected variant, or the
    /// first trace with a frequency axis.
    #[must_use]
    pub fn primary_trace_index(&self) -> usize {
        self.variant_selection.unwrap_or_else(|| {
            self.traces
                .iter()
                .position(Trace::has_frequency_axis)
                .unwrap_or(0)
        })
    }
}
