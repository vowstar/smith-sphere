//! Input parsers: Touchstone 1.x and delimited tables.

pub mod csv;
pub mod touchstone;

use serde::{Deserialize, Serialize};

/// User-facing parse failure with a concrete reason and a suggested next step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub hint: Option<String>,
}

impl ParseError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

/// Frequency unit of a file column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrequencyUnit {
    Hz,
    KHz,
    MHz,
    GHz,
}

impl FrequencyUnit {
    pub const ALL: [Self; 4] = [Self::Hz, Self::KHz, Self::MHz, Self::GHz];

    #[must_use]
    pub fn multiplier(self) -> f64 {
        match self {
            Self::Hz => 1.0,
            Self::KHz => 1e3,
            Self::MHz => 1e6,
            Self::GHz => 1e9,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Hz => "Hz",
            Self::KHz => "kHz",
            Self::MHz => "MHz",
            Self::GHz => "GHz",
        }
    }

    /// Parses a unit token such as `MHz` or `ghz`.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "hz" => Some(Self::Hz),
            "khz" => Some(Self::KHz),
            "mhz" => Some(Self::MHz),
            "ghz" => Some(Self::GHz),
            _ => None,
        }
    }

    /// Finds a unit word embedded in a longer header such as `freq (MHz)`.
    #[must_use]
    pub fn find_in(text: &str) -> Option<Self> {
        let lower = text.to_ascii_lowercase();
        for (needle, unit) in [
            ("ghz", Self::GHz),
            ("mhz", Self::MHz),
            ("khz", Self::KHz),
            ("hz", Self::Hz),
        ] {
            let mut search_from = 0;
            while let Some(position) = lower[search_from..].find(needle) {
                let start = search_from + position;
                let end = start + needle.len();
                let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
                let after_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    return Some(unit);
                }
                search_from = end;
            }
        }
        None
    }
}

/// File format inferred from a name or from pasted content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectedFormat {
    /// Touchstone with the port count from the extension when known.
    Touchstone { ports: Option<u8> },
    /// Delimited table.
    Csv,
}

/// Detects the most likely format from the file name and the first lines.
#[must_use]
pub fn detect_format(file_name: Option<&str>, text: &str) -> DetectedFormat {
    if let Some(name) = file_name {
        let lower = name.to_ascii_lowercase();
        if let Some(ports) = touchstone_ports_from_name(&lower) {
            return DetectedFormat::Touchstone { ports: Some(ports) };
        }
        if lower.ends_with(".ts") {
            return DetectedFormat::Touchstone { ports: None };
        }
        if lower.ends_with(".csv") || lower.ends_with(".tsv") || lower.ends_with(".txt") {
            return DetectedFormat::Csv;
        }
    }
    let looks_touchstone = text
        .lines()
        .map(str::trim_start)
        .take(64)
        .any(|line| line.starts_with('#') || line.starts_with('!') || line.starts_with('['));
    if looks_touchstone {
        DetectedFormat::Touchstone { ports: None }
    } else {
        DetectedFormat::Csv
    }
}

/// Port count encoded in a Touchstone extension such as `.s2p`.
#[must_use]
pub fn touchstone_ports_from_name(lower_name: &str) -> Option<u8> {
    let extension = lower_name.rsplit('.').next()?;
    let digits = extension.strip_prefix('s')?.strip_suffix('p')?;
    digits.parse::<u8>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_units_in_headers() {
        assert_eq!(
            FrequencyUnit::find_in("Frequency (MHz)"),
            Some(FrequencyUnit::MHz)
        );
        assert_eq!(FrequencyUnit::find_in("freq_ghz"), Some(FrequencyUnit::GHz));
        assert_eq!(FrequencyUnit::find_in("f [Hz]"), Some(FrequencyUnit::Hz));
        assert_eq!(FrequencyUnit::find_in("frequency"), None);
        assert_eq!(FrequencyUnit::find_in("mhz2"), None);
    }

    #[test]
    fn detects_formats() {
        assert_eq!(
            detect_format(Some("a.s2p"), ""),
            DetectedFormat::Touchstone { ports: Some(2) }
        );
        assert_eq!(
            detect_format(Some("A.S1P"), ""),
            DetectedFormat::Touchstone { ports: Some(1) }
        );
        assert_eq!(
            detect_format(Some("x.csv"), "# GHz S MA R 50"),
            DetectedFormat::Csv
        );
        assert_eq!(
            detect_format(None, "! comment\n# GHz S MA R 50\n1 0.5 30"),
            DetectedFormat::Touchstone { ports: None }
        );
        assert_eq!(detect_format(None, "freq,R,X\n1,2,3"), DetectedFormat::Csv);
    }
}
