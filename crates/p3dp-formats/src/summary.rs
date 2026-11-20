//! What a file of G-code says about the print it holds, as numbers: the printer, the material, how much filament, the layer height, the nozzle and the time. The values come from the
//! metadata of a binary G-code file or from the comment lines of a plain one (`; key = value`), under the names that the slicers write in them. Nothing here is computed from the
//! G-code itself: a figure that the file does not state stays `None`, and a figure that cannot be read as a number is left out, never guessed.

/// The figures a file states about its print.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrintSummary {
    /// The model of the printer the file was made for.
    pub printer_model: Option<String>,
    /// The kind of filament (`PLA`, `PETG`...).
    pub filament_type: Option<String>,
    /// The filament the print uses, in millimetres.
    pub filament_mm: Option<f64>,
    /// The filament the print uses, in cubic centimetres.
    pub filament_cm3: Option<f64>,
    /// The filament the print uses, in grams.
    pub filament_g: Option<f64>,
    /// The cost the file states for the filament.
    pub filament_cost: Option<f64>,
    /// The height of a layer, in millimetres.
    pub layer_height_mm: Option<f64>,
    /// The diameter of the nozzle, in millimetres.
    pub nozzle_mm: Option<f64>,
    /// The time the file says the print takes, in seconds.
    pub seconds: Option<u64>,
}

/// The names under which a figure may be written, in the order they are looked for.
const PRINTER: [&str; 2] = ["printer_model", "printer_settings_id"];
const FILAMENT_TYPE: [&str; 1] = ["filament_type"];
const FILAMENT_MM: [&str; 2] = ["filament used [mm]", "filament_used_mm"];
const FILAMENT_CM3: [&str; 2] = ["filament used [cm3]", "filament_used_cm3"];
const FILAMENT_G: [&str; 3] = [
    "filament used [g]",
    "total filament used [g]",
    "filament_used_g",
];
const FILAMENT_COST: [&str; 2] = ["total filament cost", "filament cost"];
const LAYER_HEIGHT: [&str; 1] = ["layer_height"];
const NOZZLE: [&str; 2] = ["nozzle_diameter", "nozzle diameter"];
const TIME: [&str; 2] = [
    "estimated printing time (normal mode)",
    "estimated printing time",
];

/// The first number of a text like `12.5` or `0.4,0.4` (a list of nozzles gives the first); `None` when it does not start with a finite, non-negative number.
pub fn first_number(text: &str) -> Option<f64> {
    let head = text.split([',', ';', ' ']).next()?.trim();
    head.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

/// `1d 2h 3m 4s` (any of the parts, in that order) as seconds, or a plain count of seconds.
pub fn duration_seconds(text: &str) -> Option<u64> {
    let t = text.trim();
    if let Ok(n) = t.parse::<u64>() {
        return Some(n);
    }
    let mut total = 0u64;
    let mut seen = false;
    for part in t.split_whitespace() {
        // the unit is the last character, which may be more than one byte long
        let unit = part.chars().next_back()?;
        let n: u64 = part[..part.len() - unit.len_utf8()].parse().ok()?;
        let mult = match unit {
            'd' => 86_400,
            'h' => 3_600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        total = total.checked_add(n.checked_mul(mult)?)?;
        seen = true;
    }
    seen.then_some(total)
}

impl PrintSummary {
    /// Reads the figures from `(key, value)` pairs; the first pair that has a readable value for a name wins.
    pub fn from_pairs<'a, I>(pairs: I) -> PrintSummary
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let pairs: Vec<(&str, &str)> = pairs.into_iter().collect();
        let text = |names: &[&str]| -> Option<String> {
            names.iter().find_map(|n| {
                pairs
                    .iter()
                    .find(|(k, v)| k.eq_ignore_ascii_case(n) && !v.trim().is_empty())
                    .map(|(_, v)| v.trim().to_string())
            })
        };
        let number = |names: &[&str]| -> Option<f64> {
            names.iter().find_map(|n| {
                pairs
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case(n))
                    .find_map(|(_, v)| first_number(v))
            })
        };
        PrintSummary {
            printer_model: text(&PRINTER),
            filament_type: text(&FILAMENT_TYPE),
            filament_mm: number(&FILAMENT_MM),
            filament_cm3: number(&FILAMENT_CM3),
            filament_g: number(&FILAMENT_G),
            filament_cost: number(&FILAMENT_COST),
            layer_height_mm: number(&LAYER_HEIGHT),
            nozzle_mm: number(&NOZZLE),
            seconds: TIME.iter().find_map(|n| {
                pairs
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case(n))
                    .find_map(|(_, v)| duration_seconds(v))
            }),
        }
    }

    /// Whether the file stated nothing at all.
    pub fn is_empty(&self) -> bool {
        *self == PrintSummary::default()
    }

    /// The figures that are there, as `(name, text)` lines in a fixed order, for a report.
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        if let Some(v) = &self.printer_model {
            out.push(("printer", v.clone()));
        }
        if let Some(v) = &self.filament_type {
            out.push(("filament", v.clone()));
        }
        if let Some(v) = self.filament_mm {
            out.push(("filament_mm", format!("{v:.1}")));
        }
        if let Some(v) = self.filament_cm3 {
            out.push(("filament_cm3", format!("{v:.2}")));
        }
        if let Some(v) = self.filament_g {
            out.push(("filament_g", format!("{v:.2}")));
        }
        if let Some(v) = self.filament_cost {
            out.push(("filament_cost", format!("{v:.2}")));
        }
        if let Some(v) = self.layer_height_mm {
            out.push(("layer_height_mm", format!("{v}")));
        }
        if let Some(v) = self.nozzle_mm {
            out.push(("nozzle_mm", format!("{v}")));
        }
        if let Some(v) = self.seconds {
            out.push(("seconds", v.to_string()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_figures_a_file_states_are_read_under_the_names_slicers_write() {
        let s = PrintSummary::from_pairs([
            ("printer_model", "GENERIC1"),
            ("filament_type", "PLA"),
            ("filament used [mm]", "1234.5"),
            ("filament used [cm3]", "2.97"),
            ("filament used [g]", "3.68"),
            ("total filament cost", "0.07"),
            ("layer_height", "0.2"),
            ("nozzle_diameter", "0.4,0.4"),
            ("estimated printing time (normal mode)", "1h 2m 3s"),
        ]);
        assert_eq!(s.printer_model.as_deref(), Some("GENERIC1"));
        assert_eq!(s.filament_mm, Some(1234.5));
        assert_eq!(s.filament_g, Some(3.68));
        assert_eq!(s.nozzle_mm, Some(0.4), "a list of nozzles gives the first");
        assert_eq!(s.seconds, Some(3723));
        assert!(!s.is_empty());
        let l = s.lines();
        assert_eq!(l[0], ("printer", "GENERIC1".to_string()));
        assert_eq!(l.last().unwrap(), &("seconds", "3723".to_string()));
    }

    #[test]
    fn a_figure_that_is_not_stated_or_not_a_number_is_left_out_and_never_guessed() {
        let s = PrintSummary::from_pairs([
            ("filament used [mm]", "lots"),
            ("filament used [g]", "-4"),
            ("layer_height", "inf"),
            ("estimated printing time", "soon"),
            ("printer_model", "  "),
        ]);
        assert!(s.is_empty(), "{s:?}");
        assert!(PrintSummary::from_pairs(std::iter::empty()).is_empty());
        assert!(PrintSummary::default().lines().is_empty());
    }

    #[test]
    fn the_first_readable_value_wins_and_the_names_ignore_case() {
        let s = PrintSummary::from_pairs([
            ("Layer_Height", "x"),
            ("layer_height", "0.16"),
            ("layer_height", "0.3"),
            ("estimated printing time", "90"),
        ]);
        assert_eq!(s.layer_height_mm, Some(0.16));
        assert_eq!(s.seconds, Some(90), "a plain count of seconds is read");
    }

    #[test]
    fn durations_with_days_hours_minutes_and_seconds_add_up_and_nothing_else_is_read() {
        assert_eq!(duration_seconds("1d 2h 3m 4s"), Some(93_784));
        assert_eq!(duration_seconds("45m"), Some(2_700));
        assert_eq!(duration_seconds("5x"), None);
        // a unit that is more than one byte long is not a unit, and is no reason to panic
        assert_eq!(duration_seconds("5é"), None);
        assert_eq!(duration_seconds("1h 5分"), None);
        assert_eq!(duration_seconds("é"), None);
        assert_eq!(duration_seconds(""), None);
        assert_eq!(duration_seconds("99999999999999999999d"), None);
        assert_eq!(first_number("0.4,0.6"), Some(0.4));
        assert_eq!(first_number("NaN"), None);
    }
}
