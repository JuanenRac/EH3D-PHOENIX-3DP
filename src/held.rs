//! What a slice took from the filament, held to the printer or refused, and why: one list for the command line and the window, in the eight languages. Nothing is cut down in
//! silence: a speed over the printer refuses the slice, and every figure that is held is named with who states it.

use crate::i18n::{fill, Lang, Msg};
use p3dp_profiles::filament::{FilamentProfile, Nozzle};
use p3dp_profiles::{suite_machines, MotionLimits, PrinterProfile};
use p3dp_slicer::machine_check::{self, Held, TooFast};
use p3dp_slicer::{Placement, SliceConfig, UserGcode};

/// One thing the person is told about the figures of a slice.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    /// The temperatures and the fan came from the filament.
    Filament {
        name: String,
        nozzle_c: f64,
        bed_c: f64,
        fan_percent: f64,
    },
    /// Advice of the check of the filament against the printer (in English, as the database gives it).
    Advice(String),
    /// The flow was held to what the maker of the filament gives.
    FilamentFlow(f64),
    /// The flow was held to the most the maker of the printer states.
    MachineFlow(f64),
    /// The time is estimated with the most acceleration the maker of the printer states.
    MachineAcceleration(f64),
    /// A speed over the highest of the printer: the slice is refused.
    TooFast(TooFast),
    /// The maker figures of this printer are not known here, so nothing is checked against them.
    NoFigures,
    /// Figures of the printer that nobody has checked on the page of its maker.
    Unchecked(String),
    /// The start or end G-code of the person is in the file.
    OwnGcode,
}

impl Note {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Note::Filament {
                name,
                nozzle_c,
                bed_c,
                fan_percent,
            } => fill(
                Msg::NoteFilament.tr(lang),
                &[
                    name,
                    &nozzle_c.to_string(),
                    &bed_c.to_string(),
                    &fan_percent.to_string(),
                ],
            ),
            Note::Advice(a) => format!("note: {a}"),
            Note::FilamentFlow(q) => fill(Msg::NoteFilamentFlow.tr(lang), &[&q.to_string()]),
            Note::MachineFlow(q) => fill(Msg::HeldMachineFlow.tr(lang), &[&q.to_string()]),
            Note::MachineAcceleration(a) => fill(Msg::FitAccel.tr(lang), &[&a.to_string()]),
            Note::TooFast(t) => {
                let n = |v: f64| format!("{}", (v * 10.0).round() / 10.0);
                fill(
                    Msg::FitSpeed.tr(lang),
                    &[t.key, &n(t.set_mm_s), &n(t.max_mm_s)],
                )
            }
            Note::NoFigures => Msg::FitNoFigures.tr(lang).to_string(),
            Note::Unchecked(list) => fill(Msg::FitUnchecked.tr(lang), &[list]),
            Note::OwnGcode => Msg::HeldOwnGcode.tr(lang).to_string(),
        }
    }

    /// Whether this is why a slice did not happen.
    pub fn refuses(&self) -> bool {
        matches!(self, Note::TooFast(_))
    }
}

/// The start and end G-code of the person, read from the files that are named (an empty name is no block). The lines are checked when the slice makes its G-code.
pub fn read_user_gcode(start: &str, end: &str) -> Result<UserGcode, String> {
    let read = |path: &str| -> Result<Option<String>, String> {
        let path = path.trim();
        if path.is_empty() {
            return Ok(None);
        }
        let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
        if meta.len() > p3dp_core::kv::MAX_BYTES as u64 {
            return Err(format!("{path}: the file is too large"));
        }
        std::fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("{path}: {e}"))
    };
    Ok(UserGcode {
        start: read(start)?,
        end: read(end)?,
    })
}

/// What a plan was sliced with, to tell when the plan on view is out of date.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    printer: String,
    /// The filament with its nozzle and whether that is a hardened one.
    filament: Option<(String, f64, bool)>,
    config: SliceConfig,
    start: String,
    end: String,
}

impl Recipe {
    pub fn new(
        printer: &str,
        filament: Option<(&str, Nozzle)>,
        config: &SliceConfig,
        start: &str,
        end: &str,
    ) -> Recipe {
        let mut config = config.clone();
        // the window lays the parts itself, so where they are placed is not a setting of the person
        config.placement = Placement::Origin;
        Recipe {
            printer: printer.to_string(),
            filament: filament.map(|(id, n)| (id.to_string(), n.diameter_mm, n.hardened)),
            config,
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        }
    }
}

/// The notes of the filament that was applied: its figures, its advice and the flow of its maker, which is held in the settings.
pub fn filament_notes(
    config: &mut SliceConfig,
    filament: &FilamentProfile,
    printer: &PrinterProfile,
    nozzle: Nozzle,
) -> Result<Vec<Note>, String> {
    let compat = config
        .apply_filament(filament, printer, nozzle)
        .map_err(|e| e.to_string())?;
    let mut out = vec![Note::Filament {
        name: filament.name.clone(),
        nozzle_c: config.nozzle_c,
        bed_c: config.bed_c,
        fan_percent: config.fan_percent,
    }];
    out.extend(
        compat
            .advice
            .iter()
            .map(|a| Note::Advice(a.text().to_string())),
    );
    if let Some(record) = crate::preflight::record(&filament.id) {
        if let Some(limit) = crate::preflight::hold_flow(record, config) {
            out.push(Note::FilamentFlow(limit));
        }
    }
    Ok(out)
}

/// Holds the settings to what the maker of the printer states. The first list is what was held and what is not known; a speed over the printer gives the second, with every one.
pub fn machine_notes(
    config: &mut SliceConfig,
    printer_id: &str,
) -> Result<Result<Vec<Note>, Vec<Note>>, String> {
    let suite = suite_machines().map_err(|e| e.to_string())?;
    let record = suite.find(printer_id);
    let limits = record.map(MotionLimits::of).unwrap_or_default();
    let fast = machine_check::too_fast(config, &limits);
    if !fast.is_empty() {
        return Ok(Err(fast.into_iter().map(Note::TooFast).collect()));
    }
    let mut out: Vec<Note> = machine_check::hold(config, &limits)
        .into_iter()
        .map(|h| match h {
            Held::Acceleration(a) => Note::MachineAcceleration(a),
            Held::Flow(q) => Note::MachineFlow(q),
        })
        .collect();
    if limits.max_speed_mm_s.is_none() && limits.max_flow_mm3_s.is_none() {
        out.push(Note::NoFigures);
    }
    if let Some(r) = record {
        let unchecked = r.unconfirmed();
        if !unchecked.is_empty() {
            out.push(Note::Unchecked(unchecked.join(", ")));
        }
    }
    Ok(Ok(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_profiles::filament::FilamentCatalog;
    use p3dp_profiles::Catalog;

    fn a1() -> PrinterProfile {
        Catalog::builtin()
            .unwrap()
            .find("bambu-lab-a1")
            .unwrap()
            .clone()
    }

    #[test]
    fn a_speed_over_the_printer_refuses_and_every_one_is_said_in_every_language() {
        let mut config = SliceConfig {
            travel_speed_mm_s: 900.0,
            infill_speed_mm_s: 600.0,
            ..SliceConfig::default()
        };
        let before = config.clone();
        let refused = machine_notes(&mut config, "bambu-lab-a1")
            .unwrap()
            .unwrap_err();
        assert_eq!(config, before, "nothing is cut down");
        assert_eq!(refused.len(), 2);
        assert!(refused.iter().all(Note::refuses));
        for lang in Lang::ALL {
            let all: Vec<String> = refused.iter().map(|n| n.text(lang)).collect();
            assert!(all.join(" ").contains("travel_speed_mm_s"), "{lang:?}");
            assert!(all.join(" ").contains("500"), "{lang:?}");
        }
    }

    #[test]
    fn what_is_held_names_its_figure_and_an_unknown_printer_says_it_is_not_checked() {
        let mut config = SliceConfig {
            max_volumetric_flow_mm3_s: 90.0,
            ..SliceConfig::default()
        };
        let notes = machine_notes(&mut config, "bambu-lab-a1").unwrap().unwrap();
        assert_eq!(config.max_volumetric_flow_mm3_s, 28.0);
        assert!(notes.contains(&Note::MachineFlow(28.0)), "{notes:?}");
        assert!(notes.iter().all(|n| !n.refuses()));
        assert!(Note::MachineFlow(28.0).text(Lang::En).contains("28 mm3/s"));
        let mut config = SliceConfig::default();
        let notes = machine_notes(&mut config, "prusa-mk4s").unwrap().unwrap();
        assert_eq!(notes, [Note::NoFigures]);
    }

    #[test]
    fn a_change_of_the_printer_the_filament_a_setting_or_a_block_makes_another_recipe() {
        let config = SliceConfig::default();
        let nozzle = Nozzle {
            diameter_mm: 0.4,
            hardened: false,
        };
        let base = Recipe::new("a", Some(("pla", nozzle)), &config, "", "");
        let mut placed = config.clone();
        placed.placement = Placement::Origin;
        assert_eq!(
            base,
            Recipe::new("a", Some(("pla", nozzle)), &placed, " ", "")
        );
        let mut thicker = config.clone();
        thicker.layer_height_mm = 0.28;
        let hard = Nozzle {
            hardened: true,
            ..nozzle
        };
        for other in [
            Recipe::new("b", Some(("pla", nozzle)), &config, "", ""),
            Recipe::new("a", Some(("petg", nozzle)), &config, "", ""),
            Recipe::new("a", Some(("pla", hard)), &config, "", ""),
            Recipe::new("a", None, &config, "", ""),
            Recipe::new("a", Some(("pla", nozzle)), &thicker, "", ""),
            Recipe::new("a", Some(("pla", nozzle)), &config, "s.gcode", ""),
            Recipe::new("a", Some(("pla", nozzle)), &config, "", "e.gcode"),
        ] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn the_blocks_of_the_person_are_read_or_the_file_that_cannot_be_read_is_named() {
        assert_eq!(read_user_gcode("", "  ").unwrap(), UserGcode::default());
        let file = format!(
            "{}/examples/start-prime-line.gcode",
            env!("CARGO_MANIFEST_DIR")
        );
        let user = read_user_gcode(&file, "").unwrap();
        assert!(user.start.unwrap().contains("G"));
        assert!(user.end.is_none());
        let e = read_user_gcode("", "/no/such/end.gcode").unwrap_err();
        assert!(e.contains("end.gcode"), "{e}");
    }

    #[test]
    fn a_filament_says_what_it_set_in_the_language_asked() {
        let filaments = FilamentCatalog::builtin().unwrap();
        let petg = filaments.find("generic-petg").unwrap();
        let mut config = SliceConfig::default();
        let nozzle = Nozzle {
            diameter_mm: 0.4,
            hardened: false,
        };
        let notes = filament_notes(&mut config, petg, &a1(), nozzle).unwrap();
        assert_eq!(
            notes[0].text(Lang::En),
            "filament Generic PETG: nozzle 235 C, bed 80 C, fan 40 %"
        );
        assert!(notes[0].text(Lang::Es).starts_with("filamento "));
        assert!(Note::OwnGcode.text(Lang::De).len() > 10);
    }
}
