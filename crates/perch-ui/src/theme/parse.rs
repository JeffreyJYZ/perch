//! The theme file itself: three anchors and two optional states, in one TOML file.
//!
//! The error is one line, because every caller puts it in a pane: a multi-line TOML report is
//! useless in a row beside a theme name.

use rune::{Color, color::hex, oklch};

use crate::kit::Anchors;

/// A theme file, as written.
#[derive(serde::Deserialize)]
struct File {
    bg: String,
    ink: String,
    signal: String,
    ok: Option<String>,
    fail: Option<String>,
}

/// The anchors a theme file names.
pub fn anchors(text: &str) -> Result<Anchors, String> {
    let file: File = toml::from_str(text).map_err(|error| last_line(&error.to_string()))?;
    Ok(Anchors {
        bg: colour("bg", &file.bg)?,
        ink: colour("ink", &file.ink)?,
        signal: colour("signal", &file.signal)?,
        ok: file
            .ok
            .as_deref()
            .map(|text| colour("ok", text))
            .transpose()?,
        fail: file
            .fail
            .as_deref()
            .map(|text| colour("fail", text))
            .transpose()?,
    })
}

/// The line of a `toml` report that says what is wrong: its last non-empty one.
fn last_line(report: &str) -> String {
    report
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("it will not parse")
        .to_string()
}

/// `oklch(0.165 0.002 265)`, or a `#rrggbb` converted to the same colour.
fn colour(field: &str, text: &str) -> Result<Color, String> {
    let value = text.trim();
    if let Some(colour) = hex(value) {
        return Ok(colour);
    }
    if let Some(colour) = oklch_value(value) {
        return Ok(colour);
    }
    Err(format!(
        "{field}: {value:?} is not oklch(0.165 0.002 265) or #rrggbb"
    ))
}

/// The three numbers of an `oklch(...)`, commas and spaces both accepted, degrees for hue and an
/// optional `%` on the lightness because that is how a colour picker writes it.
fn oklch_value(text: &str) -> Option<Color> {
    let inner = text.strip_prefix("oklch(")?.strip_suffix(')')?;
    let numbers: Vec<f32> = inner
        .split([' ', ',', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| match part.strip_suffix('%') {
            Some(percent) => percent.parse::<f32>().ok().map(|value| value / 100.0),
            None => part.parse::<f32>().ok(),
        })
        .collect::<Option<_>>()?;
    match numbers[..] {
        [l, c, h] => Some(oklch(l, c, h)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{colour, last_line};
    use rune::oklch;

    #[test]
    fn both_notations_are_the_same_colour_space() {
        assert_eq!(
            colour("bg", "oklch(0.2 0.02 40)").unwrap(),
            oklch(0.2, 0.02, 40.0)
        );
        assert_eq!(
            colour("bg", "oklch(0.2, 0.02, 40)").unwrap(),
            oklch(0.2, 0.02, 40.0)
        );
        assert_eq!(
            colour("bg", "#ff8800").unwrap(),
            rune::color::hex("#ff8800").unwrap()
        );
    }

    #[test]
    fn a_value_that_is_neither_names_its_field_and_the_shapes_it_wants() {
        let error = colour("signal", "amber").unwrap_err();
        assert!(error.starts_with("signal:"), "{error}");
        assert!(error.contains("oklch("), "{error}");
        assert!(error.contains("#rrggbb"), "{error}");
    }

    #[test]
    fn a_report_is_cut_to_the_line_that_says_what_is_wrong() {
        assert_eq!(
            last_line("TOML parse error at line 2, column 1\n  |\n2 | ink = 3\nmissing field `bg`"),
            "missing field `bg`"
        );
    }
}
