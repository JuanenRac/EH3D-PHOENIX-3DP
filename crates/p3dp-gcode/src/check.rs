//! The checker of a program: it reads G-code that someone else wrote (or that this program wrote earlier) and finds every temperature that goes beyond the limits of a model.

use p3dp_core::machine::{TempError, Zone};
use p3dp_core::TempLimits;

/// A line that asks for a temperature that is not allowed.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    /// The line number, starting at 1.
    pub line: usize,
    /// The command as it was written.
    pub command: String,
    pub error: TempError,
}

fn value_of(tokens: &[&str], letter: char) -> Option<Result<f64, ()>> {
    tokens.iter().skip(1).find_map(|t| {
        let mut c = t.chars();
        if c.next()?.eq_ignore_ascii_case(&letter) {
            Some(c.as_str().parse::<f64>().map_err(|_| ()))
        } else {
            None
        }
    })
}

fn named_value(tokens: &[&str], key: &str) -> Option<Result<f64, ()>> {
    tokens.iter().skip(1).find_map(|t| {
        let (k, v) = t.split_once('=')?;
        if k.eq_ignore_ascii_case(key) {
            Some(v.parse::<f64>().map_err(|_| ()))
        } else {
            None
        }
    })
}

fn judge(limits: &TempLimits, zone: Zone, v: Result<f64, ()>) -> Option<TempError> {
    match v {
        Ok(n) => limits.check(zone, n).err(),
        Err(()) => Some(TempError::NotFinite(zone)),
    }
}

/// Checks a whole program. The commands it knows: `M104`, `M109` (nozzle), `M140`, `M190` (bed), `M141`, `M191` (chamber), `G10 P.. S..` (RepRapFirmware tool),
/// `SET_HEATER_TEMPERATURE` and the parameters `BED`, `EXTRUDER`, `CHAMBER` of `PRINT_START`. Comments are ignored. A value that is not a number is reported as such.
pub fn check_program(text: &str, limits: &TempLimits) -> Vec<Violation> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let code = raw.split(';').next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = code.split_ascii_whitespace().collect();
        let cmd = tokens[0].to_ascii_uppercase();
        let mut found: Vec<TempError> = Vec::new();
        match cmd.as_str() {
            "M104" | "M109" => {
                found.extend(value_of(&tokens, 'S').and_then(|v| judge(limits, Zone::Nozzle, v)))
            }
            "M140" | "M190" => {
                found.extend(value_of(&tokens, 'S').and_then(|v| judge(limits, Zone::Bed, v)))
            }
            "M141" | "M191" => {
                found.extend(value_of(&tokens, 'S').and_then(|v| judge(limits, Zone::Chamber, v)))
            }
            "G10" => {
                if value_of(&tokens, 'P').is_some() {
                    found.extend(
                        value_of(&tokens, 'S').and_then(|v| judge(limits, Zone::Nozzle, v)),
                    );
                }
            }
            "SET_HEATER_TEMPERATURE" => {
                let heater = tokens.iter().skip(1).find_map(|t| {
                    t.split_once('=')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("HEATER"))
                        .map(|(_, v)| v.to_ascii_lowercase())
                });
                let zone = match heater.as_deref() {
                    Some(h) if h.starts_with("extruder") => Some(Zone::Nozzle),
                    Some("heater_bed") => Some(Zone::Bed),
                    Some(h) if h.contains("chamber") => Some(Zone::Chamber),
                    _ => None,
                };
                if let Some(z) = zone {
                    found.extend(named_value(&tokens, "TARGET").and_then(|v| judge(limits, z, v)));
                }
            }
            "PRINT_START" => {
                for (key, zone) in [
                    ("EXTRUDER", Zone::Nozzle),
                    ("BED", Zone::Bed),
                    ("CHAMBER", Zone::Chamber),
                ] {
                    found.extend(named_value(&tokens, key).and_then(|v| judge(limits, zone, v)));
                }
            }
            _ => {}
        }
        for error in found {
            out.push(Violation {
                line: i + 1,
                command: code.to_string(),
                error,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{JobParams, Writer};
    use p3dp_core::rng::Rng;
    use p3dp_core::Dialect;

    const L: TempLimits = TempLimits {
        nozzle_c: 290.0,
        bed_c: 120.0,
        chamber_c: Some(55.0),
    };

    #[test]
    fn finds_every_kind_of_command() {
        let prog = "M104 S300\nM109 S290\nM140 S121\nM190 S120\nM141 S56\nG10 P0 S295\nG10 S999\nSET_HEATER_TEMPERATURE HEATER=extruder TARGET=291\nSET_HEATER_TEMPERATURE HEATER=heater_bed TARGET=100\nPRINT_START BED=130 EXTRUDER=200\nM104 Sabc\nM104 S-5\nM104 S0 ; fine\n; M104 S999\n";
        let v = check_program(prog, &L);
        let lines: Vec<usize> = v.iter().map(|x| x.line).collect();
        assert_eq!(lines, vec![1, 3, 5, 6, 8, 10, 11, 12]);
        assert!(matches!(v[0].error, TempError::Above { limit, .. } if limit == 290.0));
        assert!(matches!(v[6].error, TempError::NotFinite(_)));
        assert!(matches!(v[7].error, TempError::Negative(..)));
    }

    #[test]
    fn a_clean_program_has_no_violations() {
        assert!(check_program("G28\nM104 S200\nM140 S60\nG1 X10 Y10 E1\nM84\n", &L).is_empty());
        assert!(check_program("", &L).is_empty());
    }

    #[test]
    fn a_printer_without_a_chamber_refuses_chamber_heat() {
        let l = TempLimits {
            chamber_c: None,
            ..L
        };
        assert_eq!(check_program("M141 S30\nM141 S0\n", &l).len(), 1);
    }

    #[test]
    fn what_the_writer_writes_always_passes_the_checker() {
        let mut r = Rng::new(20_260_101);
        for _ in 0..200 {
            let d = [
                Dialect::Marlin,
                Dialect::Klipper,
                Dialect::Rrf,
                Dialect::Bambu,
            ][r.below(4) as usize];
            let mut w = Writer::new(d, L, [250.0, 250.0, 250.0]).unwrap();
            let job = JobParams {
                nozzle_c: r.range(0.0, 290.0),
                bed_c: r.range(0.0, 120.0),
                chamber_c: if r.below(2) == 0 {
                    None
                } else {
                    Some(r.range(0.0, 55.0))
                },
                fan_pct: r.range(0.0, 100.0),
                estimated_minutes: Some(r.below(5000) as u32),
            };
            w.preamble(&job).unwrap();
            w.set_nozzle(r.range(0.0, 290.0), r.below(2) == 0).unwrap();
            w.set_bed(r.range(0.0, 120.0), r.below(2) == 0).unwrap();
            w.end_safe().unwrap();
            let text = w.finish().unwrap();
            assert!(check_program(&text, &L).is_empty(), "{d}\n{text}");
        }
    }

    #[test]
    fn what_the_writer_refuses_the_checker_also_finds() {
        let mut r = Rng::new(9);
        for _ in 0..100 {
            let over = r.range(290.01, 600.0);
            let mut w = Writer::new(Dialect::Marlin, L, [250.0; 3]).unwrap();
            assert!(w.set_nozzle(over, false).is_err());
            let hand = format!("M104 S{over}\n");
            assert_eq!(check_program(&hand, &L).len(), 1);
        }
    }

    #[test]
    fn garbage_never_panics() {
        let mut r = Rng::new(123);
        for _ in 0..300 {
            let n = r.below(80) as usize;
            let s: String = (0..n)
                .map(|_| {
                    [
                        "M104 ",
                        "S",
                        "G10 ",
                        "P0 ",
                        "=",
                        "TARGET",
                        "\n",
                        ";",
                        "HEATER",
                        "SET_HEATER_TEMPERATURE ",
                        "nan",
                        "1e999",
                        "-",
                        " ",
                        "PRINT_START ",
                    ][r.below(15) as usize]
                })
                .collect();
            let _ = check_program(&s, &L);
        }
    }
}
