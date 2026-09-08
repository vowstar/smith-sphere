//! Loaded measurement or model data: samples, traces, and their provenance.

use crate::impedance::Impedance;
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

/// Where a document came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DataSource {
    Manual,
    /// Built-in demonstration data with a human-readable description.
    Demo {
        description: String,
    },
    Csv {
        file_name: String,
        detail: String,
    },
    Touchstone {
        file_name: String,
        detail: String,
    },
}

impl DataSource {
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Manual => "手动输入",
            Self::Demo { .. } => "演示数据",
            Self::Csv { .. } => "CSV 表格",
            Self::Touchstone { .. } => "Touchstone",
        }
    }

    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Manual => "手动输入".to_owned(),
            Self::Demo { description } => description.clone(),
            Self::Csv { file_name, detail } | Self::Touchstone { file_name, detail } => {
                format!("{file_name}: {detail}")
            }
        }
    }

    #[must_use]
    pub fn is_demo(&self) -> bool {
        matches!(self, Self::Demo { .. })
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
    pub notes: Vec<String>,
}

impl Document {
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
