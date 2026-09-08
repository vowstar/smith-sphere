//! Parser for complex numbers written the way engineers type them.
//!
//! Accepted forms include `25+j30`, `25 - j30`, `25+30j`, `j30`, `-15i`,
//! `25`, `25, 30`, `(25, 30)`, and any of these with an `Ω` or `ohm` suffix.

use crate::complex::Complex;
use crate::i18n::Lang;

/// Parses a complex expression.
///
/// # Errors
///
/// Returns a user-facing message, in the given language, describing what could
/// not be read.
pub fn parse_complex(text: &str, lang: Lang) -> Result<Complex, String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace(['Ω', 'Ω'], "")
        .replace("ohms", "")
        .replace("ohm", "")
        .replace("Ohm", "")
        .replace("OHM", "")
        .trim_matches(|c| c == '(' || c == ')')
        .to_owned();

    if cleaned.is_empty() {
        return Err(lang
            .pick(
                "请输入一个数值，例如 25+j30",
                "Enter a value, such as 25+j30",
            )
            .to_owned());
    }

    if let Some((left, right)) = cleaned.split_once(',') {
        let re = parse_real(left).ok_or_else(|| match lang {
            Lang::Chinese => format!("无法读取实部“{left}”"),
            Lang::English => format!("Could not read the real part \"{left}\""),
        })?;
        let im = parse_real(right).ok_or_else(|| match lang {
            Lang::Chinese => format!("无法读取虚部“{right}”"),
            Lang::English => format!("Could not read the imaginary part \"{right}\""),
        })?;
        return Ok(Complex::new(re, im));
    }

    let mut terms = Vec::new();
    let mut start = 0;
    let bytes: Vec<char> = cleaned.chars().collect();
    for (index, &c) in bytes.iter().enumerate() {
        if index == 0 || (c != '+' && c != '-') {
            continue;
        }
        let previous = bytes[index - 1];
        if previous == 'e' || previous == 'E' {
            // Exponent sign such as 1e-3.
            let mantissa_present =
                index >= 2 && (bytes[index - 2].is_ascii_digit() || bytes[index - 2] == '.');
            if mantissa_present {
                continue;
            }
        }
        if previous == '+' || previous == '-' {
            continue;
        }
        terms.push(bytes[start..index].iter().collect::<String>());
        start = index;
    }
    terms.push(bytes[start..].iter().collect::<String>());

    if terms.len() > 2 {
        return Err(lang
            .pick(
                "表达式包含太多项，请使用 R+jX 的形式",
                "Too many terms; use the R+jX form",
            )
            .to_owned());
    }

    let mut real = None;
    let mut imaginary = None;
    for term in terms {
        let (value, is_imaginary) = parse_term(&term, lang)?;
        let slot = if is_imaginary {
            &mut imaginary
        } else {
            &mut real
        };
        if slot.is_some() {
            return Err(lang
                .pick(
                    "表达式包含重复的实部或虚部",
                    "The expression repeats the real or imaginary part",
                )
                .to_owned());
        }
        *slot = Some(value);
    }

    Ok(Complex::new(real.unwrap_or(0.0), imaginary.unwrap_or(0.0)))
}

fn parse_term(term: &str, lang: Lang) -> Result<(f64, bool), String> {
    let has_unit = |c: char| c == 'j' || c == 'J' || c == 'i' || c == 'I';
    let unit_count = term.chars().filter(|&c| has_unit(c)).count();
    if unit_count > 1 {
        return Err(match lang {
            Lang::Chinese => format!("项“{term}”包含多个虚数单位"),
            Lang::English => format!("Term \"{term}\" has more than one imaginary unit"),
        });
    }
    if unit_count == 0 {
        return parse_real(term)
            .map(|value| (value, false))
            .ok_or_else(|| match lang {
                Lang::Chinese => format!("无法读取“{term}”"),
                Lang::English => format!("Could not read \"{term}\""),
            });
    }

    let stripped: String = term.chars().filter(|&c| !has_unit(c)).collect();
    let unit_index = term.find(has_unit).unwrap_or(0);
    let valid_position = unit_index == term.len() - 1
        || unit_index == 0
        || (unit_index == 1 && (term.starts_with('+') || term.starts_with('-')));
    if !valid_position {
        return Err(match lang {
            Lang::Chinese => format!("虚数单位在“{term}”中的位置无法识别，请写成 j30 或 30j"),
            Lang::English => {
                format!("The imaginary unit is misplaced in \"{term}\"; write j30 or 30j")
            }
        });
    }
    let magnitude = match stripped.as_str() {
        "" | "+" => 1.0,
        "-" => -1.0,
        other => parse_real(other).ok_or_else(|| match lang {
            Lang::Chinese => format!("无法读取虚部“{term}”"),
            Lang::English => format!("Could not read the imaginary part \"{term}\""),
        })?,
    };
    Ok((magnitude, true))
}

fn parse_real(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Complex {
        parse_complex(text, Lang::Chinese).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    #[test]
    fn accepts_common_engineering_forms() {
        assert_eq!(ok("25+j30"), Complex::new(25.0, 30.0));
        assert_eq!(ok("25 - j30"), Complex::new(25.0, -30.0));
        assert_eq!(ok("25+30j"), Complex::new(25.0, 30.0));
        assert_eq!(ok("j30"), Complex::new(0.0, 30.0));
        assert_eq!(ok("-15i"), Complex::new(0.0, -15.0));
        assert_eq!(ok("25"), Complex::new(25.0, 0.0));
        assert_eq!(ok("-j"), Complex::new(0.0, -1.0));
        assert_eq!(ok("(25, 30)"), Complex::new(25.0, 30.0));
        assert_eq!(ok("25,30"), Complex::new(25.0, 30.0));
        assert_eq!(ok("1e2 + j1e-1 Ω"), Complex::new(100.0, 0.1));
        assert_eq!(ok("50 ohm"), Complex::new(50.0, 0.0));
        assert_eq!(ok("-20-j5"), Complex::new(-20.0, -5.0));
    }

    #[test]
    fn rejects_garbage_with_a_message() {
        assert!(parse_complex("", Lang::Chinese).is_err());
        assert!(parse_complex("abc", Lang::Chinese).is_err());
        assert!(parse_complex("1+2+3", Lang::Chinese).is_err());
        assert!(parse_complex("j1+j2", Lang::Chinese).is_err());
        assert!(parse_complex("2j5", Lang::Chinese).is_err());
    }
}
