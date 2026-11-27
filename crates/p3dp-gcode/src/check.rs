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

/// A word of a G, M or T command: its letter and the number after it (`Err` when what follows the letter is not a number).
type Word = (char, Result<f64, ()>);

/// The words of a line the way a firmware reads them: with or without blanks between them (`M104S210`).
fn words_of(code: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_alphabetic() {
            continue;
        }
        while chars.next_if(|d| d.is_ascii_whitespace()).is_some() {}
        let mut number = String::new();
        while let Some(d) = chars.next_if(|d| d.is_ascii_digit() || matches!(d, '+' | '-' | '.')) {
            number.push(d);
        }
        out.push((
            c.to_ascii_uppercase(),
            number.parse::<f64>().map_err(|_| ()),
        ));
    }
    out
}

/// What of a line is a command: without the comment, the line number (`N12`) and the checksum (`*45`).
fn command_of(raw: &str) -> &str {
    let code = raw.split(';').next().unwrap_or("");
    let code = code.split('*').next().unwrap_or("").trim();
    match code.strip_prefix(['N', 'n']) {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start(),
        _ => code,
    }
}

/// Whether the line is a G, M or T command (a letter and a number), and not a macro with a name of its own.
fn is_classic(code: &str) -> bool {
    let mut c = code.chars();
    c.next()
        .is_some_and(|c| matches!(c.to_ascii_uppercase(), 'G' | 'M' | 'T'))
        && c.find(|c| !c.is_ascii_whitespace())
            .is_some_and(|c| c.is_ascii_digit())
}

/// Every value given with `letter`: a firmware may take any of two words of the same letter, so each one is judged.
fn values(words: &[Word], letter: char) -> impl Iterator<Item = Result<f64, ()>> + '_ {
    words
        .iter()
        .skip(1)
        .filter(move |(l, _)| *l == letter)
        .map(|(_, v)| *v)
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

/// The temperatures a G, M or T command asks for: the target (`S`) and, where the command has one, the temperature to wait or stand by at (`R`).
fn classic(code: &str, limits: &TempLimits) -> Vec<TempError> {
    let words = words_of(code);
    let Some(&(letter, Ok(number))) = words.first() else {
        return Vec::new();
    };
    let (zone, letters): (Zone, &[char]) = match (letter, number as i64) {
        ('M', 104 | 109) => (Zone::Nozzle, &['S', 'R']),
        ('M', 140 | 190) => (Zone::Bed, &['S', 'R']),
        ('M', 141 | 191) => (Zone::Chamber, &['S', 'R']),
        // RepRapFirmware: the temperatures of a tool, only when the tool is named (`G10` alone retracts)
        ('M', 568) => (Zone::Nozzle, &['S', 'R']),
        ('G', 10) if values(&words, 'P').next().is_some() => (Zone::Nozzle, &['S', 'R']),
        // the tuning of a heater heats it: E-1 is the bed and E-2 the chamber
        ('M', 303) => {
            let zone = match values(&words, 'E').next() {
                Some(Ok(-1.0)) => Zone::Bed,
                Some(Ok(-2.0)) => Zone::Chamber,
                _ => Zone::Nozzle,
            };
            (zone, &['S'])
        }
        _ => return Vec::new(),
    };
    letters
        .iter()
        .flat_map(|&l| values(&words, l))
        .filter_map(|v| judge(limits, zone, v))
        .collect()
}

/// The temperatures a macro with a name of its own (Klipper) asks for.
fn named(code: &str, limits: &TempLimits) -> Vec<TempError> {
    let tokens: Vec<&str> = code.split_ascii_whitespace().collect();
    let mut found = Vec::new();
    match tokens[0].to_ascii_uppercase().as_str() {
        "SET_HEATER_TEMPERATURE" => {
            let heater = tokens.iter().skip(1).find_map(|t| {
                t.split_once('=')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("HEATER"))
                    .map(|(_, v)| v.to_ascii_lowercase())
            });
            let zone = match heater.as_deref() {
                Some("heater_bed" | "bed") => Some(Zone::Bed),
                Some(h) if h.contains("chamber") => Some(Zone::Chamber),
                Some(h)
                    if ["extruder", "hotend", "nozzle"]
                        .iter()
                        .any(|n| h.contains(n)) =>
                {
                    Some(Zone::Nozzle)
                }
                _ => None,
            };
            if let Some(z) = zone {
                found.extend(named_value(&tokens, "TARGET").and_then(|v| judge(limits, z, v)));
            }
        }
        "PRINT_START" | "START_PRINT" => {
            for (keys, zone) in [
                (
                    &["EXTRUDER", "EXTRUDER_TEMP", "HOTEND", "HOTEND_TEMP"][..],
                    Zone::Nozzle,
                ),
                (&["BED", "BED_TEMP"][..], Zone::Bed),
                (&["CHAMBER", "CHAMBER_TEMP"][..], Zone::Chamber),
            ] {
                for key in keys {
                    found.extend(named_value(&tokens, key).and_then(|v| judge(limits, zone, v)));
                }
            }
        }
        _ => {}
    }
    found
}

/// Checks a whole program. The commands it knows: `M104`, `M109` (nozzle), `M140`, `M190` (bed), `M141`, `M191` (chamber), `M303` (tuning), `G10 P.. S.. R..` and
/// `M568` (RepRapFirmware tool), `SET_HEATER_TEMPERATURE` and the temperature parameters of `PRINT_START` and `START_PRINT`. A command is read with or without blanks,
/// behind a line number and a checksum, and a carriage return ends a line. Comments are ignored. A value that is not a number is reported as such.
pub fn check_program(text: &str, limits: &TempLimits) -> Vec<Violation> {
    let mut out = Vec::new();
    let lines = text
        .split_terminator('\n')
        .flat_map(|l| l.strip_suffix('\r').unwrap_or(l).split('\r'));
    for (i, raw) in lines.enumerate() {
        let code = command_of(raw);
        if code.is_empty() {
            continue;
        }
        let found = if is_classic(code) {
            classic(code, limits)
        } else {
            named(code, limits)
        };
        for error in found {
            out.push(Violation {
                line: i + 1,
                command: raw.split(';').next().unwrap_or("").trim().to_string(),
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
