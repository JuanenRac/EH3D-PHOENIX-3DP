//! The start and end G-code a person writes for their own machine. The program writes its own start and end; these lines are added to them, never in their place, and every
//! line is checked before one of them is written: only the commands of the list below, temperatures inside the limits of the model, moves inside its build volume. A line that
//! is not understood refuses the whole block: nothing is skipped, changed or cut down.
//!
//! Placeholders in braces are replaced by the figures of the job: `{nozzle_c}`, `{bed_c}`, `{chamber_c}`, `{max_x}`, `{max_y}`, `{max_z}` and `{top_z}`.

use crate::writer::{num, GcodeError, Writer};
use p3dp_core::machine::Zone;
use p3dp_core::{Dialect, TempLimits};

/// The most lines of a block.
pub const MAX_LINES: usize = 200;
/// The most characters of a line.
pub const MAX_LINE_CHARS: usize = 200;
/// The most filament one line may move, in millimetres.
pub const MAX_E_MM: f64 = 100.0;
/// The longest pause one line may ask for, in seconds.
pub const MAX_PAUSE_S: f64 = 600.0;

/// Where a block goes: before the first layer or after the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Start,
    End,
}

impl Place {
    fn name(self) -> &'static str {
        match self {
            Place::Start => "start",
            Place::End => "end",
        }
    }
}

/// The figures of the job that a block may name and must respect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockJob {
    pub nozzle_c: f64,
    pub bed_c: f64,
    pub chamber_c: f64,
    /// The height of the print: a block at the end may not move the head below it.
    pub top_z_mm: f64,
}

struct Rules {
    limits: TempLimits,
    volume_mm: [f64; 3],
    place: Place,
    top_z_mm: f64,
    relative: bool,
}

/// The words of a command: the letter and what follows it.
fn word(token: &str) -> Option<(char, &str)> {
    let mut c = token.chars();
    let letter = c.next()?.to_ascii_uppercase();
    Some((letter, c.as_str()))
}

fn number(token: &str) -> Result<(char, f64), String> {
    let (letter, rest) = word(token).ok_or_else(|| "an empty word".to_string())?;
    let v = rest
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{token:?} is not a letter and a number"))?;
    Ok((letter, v))
}

impl Rules {
    fn numbers(&self, words: &[&str], allowed: &str) -> Result<Vec<(char, f64)>, String> {
        let mut out: Vec<(char, f64)> = Vec::new();
        for w in words {
            let (letter, v) = number(w)?;
            if !allowed.contains(letter) {
                return Err(format!(
                    "the word {w:?} is not one this command may have here"
                ));
            }
            if out.iter().any(|(l, _)| *l == letter) {
                return Err(format!("the word {letter} is there twice"));
            }
            out.push((letter, v));
        }
        Ok(out)
    }

    fn movement(&self, words: &[&str]) -> Result<(), String> {
        let found = self.numbers(words, "XYZEF")?;
        if found.is_empty() {
            return Err("a move with nothing to move".to_string());
        }
        for (letter, v) in found {
            match letter {
                'X' | 'Y' | 'Z' => {
                    let max = match letter {
                        'X' => self.volume_mm[0],
                        'Y' => self.volume_mm[1],
                        _ => self.volume_mm[2],
                    };
                    let ok = if self.relative {
                        v.abs() <= max
                    } else {
                        (0.0..=max).contains(&v)
                    };
                    if !ok {
                        return Err(format!(
                            "{letter}{} is outside the build volume (0 to {max})",
                            num(v)
                        ));
                    }
                    if letter == 'Z' && self.place == Place::End {
                        let low = if self.relative {
                            v < 0.0
                        } else {
                            v < self.top_z_mm - 1e-9
                        };
                        if low {
                            return Err(format!(
                                "Z{} would bring the head down into the print, which is {} mm high",
                                num(v),
                                num(self.top_z_mm)
                            ));
                        }
                    }
                }
                'E' => {
                    if v.abs() > MAX_E_MM {
                        return Err(format!(
                            "E{} moves more filament than {MAX_E_MM} mm in one line",
                            num(v)
                        ));
                    }
                }
                _ => {
                    if !(1.0..=100_000.0).contains(&v) {
                        return Err(format!("the feed {} is not from 1 to 100000", num(v)));
                    }
                }
            }
        }
        Ok(())
    }

    fn temperature(&self, words: &[&str], zone: Zone) -> Result<(), String> {
        let allowed = if zone == Zone::Nozzle { "ST" } else { "S" };
        let found = self.numbers(words, allowed)?;
        let Some((_, s)) = found.iter().find(|(l, _)| *l == 'S') else {
            return Err("a temperature is written with S".to_string());
        };
        self.limits.check(zone, *s).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn home(&self, words: &[&str]) -> Result<(), String> {
        let mut axes = Vec::new();
        for w in words {
            match w.to_ascii_uppercase().as_str() {
                "X" => axes.push('X'),
                "Y" => axes.push('Y'),
                "Z" => axes.push('Z'),
                _ => return Err(format!("G28 takes X, Y and Z, not {w:?}")),
            }
        }
        if self.place == Place::End && (axes.is_empty() || axes.contains(&'Z')) {
            return Err(
                "homing Z at the end would drive the head into the print: home X and Y only"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn line(&mut self, code: &str) -> Result<(), String> {
        let mut parts = code.split_ascii_whitespace();
        let Some(first) = parts.next() else {
            return Ok(());
        };
        let command = first.to_ascii_uppercase();
        let words: Vec<&str> = parts.collect();
        let bare = |words: &[&str]| {
            if words.is_empty() {
                Ok(())
            } else {
                Err(format!("{command} takes nothing after it"))
            }
        };
        match command.as_str() {
            "G0" | "G1" => self.movement(&words),
            "G4" => {
                let found = self.numbers(&words, "SP")?;
                let seconds: f64 = found
                    .iter()
                    .map(|(l, v)| if *l == 'P' { v / 1000.0 } else { *v })
                    .sum();
                if found.iter().any(|(_, v)| *v < 0.0) || seconds > MAX_PAUSE_S {
                    return Err(format!("a pause is from 0 to {MAX_PAUSE_S} seconds"));
                }
                Ok(())
            }
            "G28" => self.home(&words),
            "G90" => {
                bare(&words)?;
                self.relative = false;
                Ok(())
            }
            "G91" => {
                bare(&words)?;
                self.relative = true;
                Ok(())
            }
            // only the extruder may be given a new origin: a new origin of an axis would move the build volume
            // with no word at all some firmwares give every axis a new origin
            "G92" if words.is_empty() => Err("G92 needs the E word".to_string()),
            "G92" => self.numbers(&words, "E").map(|_| ()),
            "G21" | "G29" | "M82" | "M83" | "M84" | "M105" | "M400" => bare(&words),
            "M104" | "M109" => self.temperature(&words, Zone::Nozzle),
            "M140" | "M190" => self.temperature(&words, Zone::Bed),
            "M141" | "M191" => self.temperature(&words, Zone::Chamber),
            "M106" => {
                let found = self.numbers(&words, "SP")?;
                if found.iter().any(|(_, v)| !(0.0..=255.0).contains(v)) {
                    return Err("a fan is from 0 to 255".to_string());
                }
                Ok(())
            }
            "M107" => self.numbers(&words, "P").map(|_| ()),
            "M117" => Ok(()),
            "BED_MESH_CALIBRATE" | "BED_MESH_PROFILE" | "QUAD_GANTRY_LEVEL" | "Z_TILT_ADJUST" => {
                let plain = |s: &str| {
                    !s.is_empty()
                        && s.chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                };
                if words
                    .iter()
                    .all(|w| w.split_once('=').is_some_and(|(k, v)| plain(k) && plain(v)))
                {
                    Ok(())
                } else {
                    Err(format!("{command} takes words of the form NAME=value"))
                }
            }
            t if t
                .strip_prefix('T')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| n <= 15) =>
            {
                bare(&words)
            }
            other => Err(format!(
                "{other} is not a command that a start or end block may have"
            )),
        }
    }
}

fn filled(text: &str, job: &BlockJob, volume_mm: [f64; 3]) -> Result<String, String> {
    let values = [
        ("{nozzle_c}", job.nozzle_c),
        ("{bed_c}", job.bed_c),
        ("{chamber_c}", job.chamber_c),
        ("{max_x}", volume_mm[0]),
        ("{max_y}", volume_mm[1]),
        ("{max_z}", volume_mm[2]),
        ("{top_z}", job.top_z_mm),
    ];
    let mut out = text.to_string();
    for (name, value) in values {
        out = out.replace(name, &num(value));
    }
    if out.contains(['{', '}']) {
        return Err("a placeholder that the program does not know".to_string());
    }
    Ok(out)
}

/// The lines of a block, checked and with the placeholders replaced. The error names the block and the line.
pub fn check_block(
    text: &str,
    place: Place,
    job: &BlockJob,
    dialect: Dialect,
    limits: TempLimits,
    volume_mm: [f64; 3],
) -> Result<Vec<String>, GcodeError> {
    let refuse = |line: usize, reason: String| {
        GcodeError::Value(format!("{} G-code, line {line}: {reason}", place.name()))
    };
    if dialect == Dialect::Msla {
        return Err(refuse(0, "a resin printer takes no G-code".to_string()));
    }
    let count = text.lines().count();
    if count > MAX_LINES {
        return Err(refuse(
            MAX_LINES + 1,
            format!("more than {MAX_LINES} lines"),
        ));
    }
    let mut rules = Rules {
        limits,
        volume_mm,
        place,
        top_z_mm: job.top_z_mm,
        relative: false,
    };
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.chars().count() > MAX_LINE_CHARS {
            return Err(refuse(n, "the line is too long".to_string()));
        }
        if line.chars().any(|c| c.is_control() && c != '\t') {
            return Err(refuse(n, "a control character".to_string()));
        }
        let line = filled(line, job, volume_mm).map_err(|e| refuse(n, e))?;
        let code = line.split(';').next().unwrap_or("").trim();
        rules.line(code).map_err(|e| refuse(n, e))?;
        out.push(line);
    }
    Ok(out)
}

impl Writer {
    /// Adds a block of the person, checked first as a whole, and then puts back what the rest of the program takes for granted: millimetres, absolute positions and relative
    /// extrusion.
    pub fn user_block(
        &mut self,
        text: &str,
        place: Place,
        job: &BlockJob,
    ) -> Result<(), GcodeError> {
        let lines = check_block(
            text,
            place,
            job,
            self.dialect(),
            self.limits(),
            self.volume_mm(),
        )?;
        self.comment(&format!("your {} G-code", place.name()))?;
        for line in lines {
            self.raw_line(line);
        }
        self.comment(&format!("end of your {} G-code", place.name()))?;
        for line in ["G21", "G90", "M83"] {
            self.raw_line(line.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: TempLimits = TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    };
    const V: [f64; 3] = [256.0, 256.0, 256.0];
    const J: BlockJob = BlockJob {
        nozzle_c: 215.0,
        bed_c: 60.0,
        chamber_c: 0.0,
        top_z_mm: 20.0,
    };

    fn check(text: &str, place: Place) -> Result<Vec<String>, String> {
        check_block(text, place, &J, Dialect::Marlin, L, V).map_err(|e| e.to_string())
    }

    #[test]
    fn a_block_of_known_commands_inside_the_limits_is_taken_with_its_placeholders() {
        let lines = check(
            "; prime along the front\nG92 E0\nG1 Z0.3 F600\nG1 X5 Y5 F6000\nG1 X150 E9 F1200 ; {nozzle_c}\n\nM104 S{nozzle_c}\nM140 S{bed_c}\nG1 Z{top_z}\nG4 P500\nT0\nM117 ready",
            Place::Start,
        )
        .unwrap();
        assert_eq!(lines[4], "G1 X150 E9 F1200 ; 215");
        assert!(lines.contains(&"M104 S215".to_string()));
        assert!(lines.contains(&"G1 Z20".to_string()));
        assert_eq!(lines.len(), 11, "blank lines go");
        assert!(check("BED_MESH_PROFILE LOAD=default\nG28\nG29", Place::Start).is_ok());
    }

    #[test]
    fn a_line_that_leaves_the_machine_or_is_not_known_refuses_the_whole_block() {
        for bad in [
            "G1 X257",
            "G1 Y-1",
            "G1 Z999",
            "G1 X1 M104 S999",
            "G1X10",
            "G1",
            "G1 X1 X2",
            "G1 Xnan",
            "G1 E101",
            "G1 F0",
            "M104 S301",
            "M109 R200",
            "M190 S101",
            "M141 S40",
            "M104",
            "M302 P1",
            "M500",
            "M997",
            "G92 X0",
            "G2 X10 Y10 I5",
            "G4 S601",
            "M106 S300",
            "T16",
            "N10 G1 X1",
            "SET_KINEMATIC_POSITION Z=0",
            "BED_MESH_PROFILE LOAD={name}",
            "RESPOND MSG=hi",
        ] {
            let text = format!("G28\n{bad}");
            let e = check(&text, Place::Start).unwrap_err();
            assert!(e.starts_with("start G-code, line 2: "), "{bad}: {e}");
        }
        let long = "G4 S1\n".repeat(MAX_LINES + 1);
        assert!(check(&long, Place::Start).is_err());
    }

    #[test]
    fn relative_moves_are_bounded_too() {
        assert!(check("G91\nG1 X-10 Y10\nG90", Place::Start).is_ok());
        assert!(check("G91\nG1 X300", Place::Start).is_err());
        assert!(check("G91\nG90\nG1 X-1", Place::Start).is_err());
    }

    #[test]
    fn the_end_block_never_brings_the_head_down_into_the_print() {
        assert!(check("G1 Z30\nG1 X0 Y250\nG28 X Y\nG91\nG1 Z5", Place::End).is_ok());
        for bad in ["G1 Z19", "G91\nG1 Z-1", "G28", "G28 Z", "G28 X Z"] {
            assert!(check(bad, Place::End).is_err(), "{bad}");
        }
        assert!(check("G28\nG1 Z1", Place::Start).is_ok());
    }

    #[test]
    fn a_block_is_written_whole_or_not_at_all_and_the_modes_come_back() {
        let mut w = Writer::new(Dialect::Marlin, L, V).unwrap();
        assert!(w.user_block("G28\nM104 S999", Place::Start, &J).is_err());
        assert!(w.lines().is_empty());
        w.user_block("G91\nG1 Z2", Place::Start, &J).unwrap();
        assert_eq!(
            w.lines(),
            [
                "; your start G-code",
                "G91",
                "G1 Z2",
                "; end of your start G-code",
                "G21",
                "G90",
                "M83"
            ]
        );
        assert!(check_block("G28", Place::Start, &J, Dialect::Msla, L, V).is_err());
    }
}
