//! What to look at before printing, from what the database of materials of the suite says about the filament: the temperatures set against the ones the maker gives, the flow the slice asks
//! of the nozzle against the flow the filament takes, the drying the maker advises, and the nozzle, the enclosure and the extruder the filament asks for. Nothing here stops a print: it is a
//! list for the person, each line a warning or advice, in the eight languages. A filament that is not in the database (a profile file of the person) has no list.

use crate::i18n::{fill, Lang, Msg};
use eh3d_printmaterials::model::{Filament, Moisture};
use eh3d_printmaterials::query::Database;
use p3dp_slicer::SliceConfig;
use std::sync::OnceLock;

/// How serious a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Something that is set against what the maker says.
    Warning,
    /// Something that helps.
    Advice,
}

/// One thing to look at.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The nozzle is set outside the range of the filament.
    NozzleTemp { set_c: f64, min_c: f64, max_c: f64 },
    /// The bed is set outside the range of the filament.
    BedTemp { set_c: f64, min_c: f64, max_c: f64 },
    /// The slice asks for more flow than the filament takes, in cubic millimetres a second.
    Flow { asked: f64, limit: f64 },
    /// The maker advises drying it first.
    Dry { temp_c: f64, hours: f64 },
    /// It wears a brass nozzle and the nozzle is not hardened.
    Hardened,
    /// A closed printer is advised and this one has no heated chamber.
    Enclosure,
    /// A direct drive extruder is advised.
    DirectDrive,
}

impl Finding {
    /// A warning or advice.
    pub fn severity(&self) -> Severity {
        match self {
            Finding::NozzleTemp { .. }
            | Finding::BedTemp { .. }
            | Finding::Flow { .. }
            | Finding::Hardened => Severity::Warning,
            Finding::Dry { .. } | Finding::Enclosure | Finding::DirectDrive => Severity::Advice,
        }
    }

    /// The line in a language.
    pub fn text(&self, lang: Lang) -> String {
        let n = |v: f64| {
            if (v - v.round()).abs() < 1e-9 {
                format!("{v:.0}")
            } else {
                format!("{v:.1}")
            }
        };
        match self {
            Finding::NozzleTemp {
                set_c,
                min_c,
                max_c,
            } => fill(
                Msg::PfNozzleTemp.tr(lang),
                &[&n(*set_c), &n(*min_c), &n(*max_c)],
            ),
            Finding::BedTemp {
                set_c,
                min_c,
                max_c,
            } => fill(
                Msg::PfBedTemp.tr(lang),
                &[&n(*set_c), &n(*min_c), &n(*max_c)],
            ),
            Finding::Flow { asked, limit } => fill(Msg::PfFlow.tr(lang), &[&n(*asked), &n(*limit)]),
            Finding::Dry { temp_c, hours } => fill(Msg::PfDry.tr(lang), &[&n(*temp_c), &n(*hours)]),
            Finding::Hardened => Msg::PfHardened.tr(lang).to_string(),
            Finding::Enclosure => Msg::PfEnclosure.tr(lang).to_string(),
            Finding::DirectDrive => Msg::PfDirectDrive.tr(lang).to_string(),
        }
    }
}

/// The flow the slice asks of the nozzle at its fastest, in cubic millimetres a second: the section of a line of a layer times the fastest of the speeds that lay plastic (never above the
/// limit of speed of the slice).
pub fn peak_flow_mm3_s(config: &SliceConfig) -> f64 {
    let fastest = [
        config.perimeter_speed_mm_s,
        config.external_perimeter_speed_mm_s,
        config.infill_speed_mm_s,
        config.solid_infill_speed_mm_s,
    ]
    .into_iter()
    .fold(0.0_f64, f64::max)
    .min(config.max_speed_mm_s);
    config.line_area_mm2(config.layer_height_mm) * fastest
}

/// What to look at for a filament and a slice. `temps_set` says that the temperatures of the slice are the ones that will be used (there is a plan); a nozzle and a bed that are not yet set
/// are not compared. `chamber_heated` is a printer with a heated chamber, which needs no advice about an enclosure.
pub fn check(
    record: &Filament,
    config: &SliceConfig,
    temps_set: bool,
    hardened_nozzle: bool,
    chamber_heated: bool,
) -> Vec<Finding> {
    let mut out = Vec::new();
    if temps_set {
        if let Some((lo, hi)) = record.nozzle_range() {
            if !(lo..=hi).contains(&config.nozzle_c) {
                out.push(Finding::NozzleTemp {
                    set_c: config.nozzle_c,
                    min_c: lo,
                    max_c: hi,
                });
            }
        }
        if let Some((lo, hi)) = record.bed_range() {
            if !(lo..=hi).contains(&config.bed_c) {
                out.push(Finding::BedTemp {
                    set_c: config.bed_c,
                    min_c: lo,
                    max_c: hi,
                });
            }
        }
    }
    if let Some(limit) = record.mm3s_limit() {
        let asked = peak_flow_mm3_s(config);
        if asked > limit {
            out.push(Finding::Flow { asked, limit });
        }
    }
    if record.needs_hardened_nozzle() && !hardened_nozzle {
        out.push(Finding::Hardened);
    }
    if let Some(plan) = record.drying_plan() {
        if plan.moisture == Some(Moisture::High) {
            out.push(Finding::Dry {
                temp_c: plan.temp_c,
                hours: plan.hours,
            });
        }
    }
    if record.enclosure_advised == Some(true) && !chamber_heated {
        out.push(Finding::Enclosure);
    }
    if record.direct_drive_advised == Some(true) {
        out.push(Finding::DirectDrive);
    }
    out
}

static DATABASE: OnceLock<Option<Database>> = OnceLock::new();

/// The record of a filament of the database of the suite, by its identifier.
pub fn record(id: &str) -> Option<&'static Filament> {
    DATABASE
        .get_or_init(|| Database::embedded().ok())
        .as_ref()?
        .filament(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_printmaterials::model::Drying;

    fn pla() -> Filament {
        record("generic/generic-pla")
            .expect("the generic PLA")
            .clone()
    }

    fn config() -> SliceConfig {
        SliceConfig::default()
    }

    #[test]
    fn a_filament_set_inside_its_ranges_with_the_right_nozzle_has_nothing_to_look_at() {
        let mut c = config();
        c.nozzle_c = 210.0;
        c.bed_c = 60.0;
        assert_eq!(check(&pla(), &c, true, false, false), Vec::new());
        // temperatures that are not set yet are not compared
        c.nozzle_c = 400.0;
        assert!(check(&pla(), &c, false, false, false).is_empty());
    }

    #[test]
    fn a_temperature_outside_the_range_of_the_maker_is_a_warning_with_the_figures() {
        let mut c = config();
        c.nozzle_c = 260.0;
        c.bed_c = 110.0;
        let found = check(&pla(), &c, true, false, false);
        let r = pla();
        let (nlo, nhi) = r.nozzle_range().unwrap();
        let (blo, bhi) = r.bed_range().unwrap();
        assert_eq!(
            found,
            vec![
                Finding::NozzleTemp {
                    set_c: 260.0,
                    min_c: nlo,
                    max_c: nhi
                },
                Finding::BedTemp {
                    set_c: 110.0,
                    min_c: blo,
                    max_c: bhi
                },
            ]
        );
        assert!(found.iter().all(|f| f.severity() == Severity::Warning));
        let text = found[0].text(Lang::En);
        assert!(
            text.contains("260")
                && text.contains(&format!("{nlo:.0}"))
                && text.contains(&format!("{nhi:.0}")),
            "{text}"
        );
    }

    #[test]
    fn the_flow_the_slice_asks_is_compared_with_the_flow_the_filament_takes() {
        let mut c = config();
        c.perimeter_speed_mm_s = 100.0;
        c.external_perimeter_speed_mm_s = 80.0;
        c.infill_speed_mm_s = 200.0;
        c.solid_infill_speed_mm_s = 150.0;
        c.max_speed_mm_s = 500.0;
        let asked = peak_flow_mm3_s(&c);
        assert!((asked - c.line_area_mm2(c.layer_height_mm) * 200.0).abs() < 1e-9);
        // the speed limit of the slice caps it
        let mut capped = c.clone();
        capped.max_speed_mm_s = 100.0;
        assert!(peak_flow_mm3_s(&capped) < asked);
        let mut r = pla();
        r.max_flow_mm3_s = Some(asked - 1.0);
        let found = check(&r, &c, false, false, false);
        assert_eq!(
            found,
            vec![Finding::Flow {
                asked,
                limit: asked - 1.0
            }]
        );
        assert!(found[0].text(Lang::Es).contains("mm3/s"));
        // at the limit or under it, nothing; and a record with no limit says nothing
        r.max_flow_mm3_s = Some(asked);
        assert!(check(&r, &c, false, false, false).is_empty());
        r.max_flow_mm3_s = None;
        assert!(check(&r, &c, false, false, false).is_empty());
    }

    #[test]
    fn the_nozzle_the_enclosure_the_extruder_and_the_drying_come_from_the_record() {
        let mut r = pla();
        r.abrasive = Some(true);
        r.enclosure_advised = Some(true);
        r.direct_drive_advised = Some(true);
        r.drying = Some(Drying {
            temp_c: 70.0,
            hours: 6.0,
        });
        r.moisture = Some(Moisture::High);
        let c = config();
        let found = check(&r, &c, false, false, false);
        assert_eq!(
            found,
            vec![
                Finding::Hardened,
                Finding::Dry {
                    temp_c: 70.0,
                    hours: 6.0
                },
                Finding::Enclosure,
                Finding::DirectDrive,
            ]
        );
        assert_eq!(found[0].severity(), Severity::Warning);
        assert!(found[1..].iter().all(|f| f.severity() == Severity::Advice));
        // a hardened nozzle and a heated chamber take two of them away
        let better = check(&r, &c, false, true, true);
        assert_eq!(better.len(), 2);
        assert!(!better.contains(&Finding::Hardened) && !better.contains(&Finding::Enclosure));
        // a filament that takes water slowly is not told to be dried
        r.moisture = Some(Moisture::Low);
        assert!(!check(&r, &c, false, true, true)
            .iter()
            .any(|f| matches!(f, Finding::Dry { .. })));
    }

    #[test]
    fn every_line_is_written_in_the_eight_languages_with_its_figures() {
        let all = [
            Finding::NozzleTemp {
                set_c: 260.0,
                min_c: 190.0,
                max_c: 230.0,
            },
            Finding::BedTemp {
                set_c: 110.5,
                min_c: 0.0,
                max_c: 70.0,
            },
            Finding::Flow {
                asked: 15.25,
                limit: 12.0,
            },
            Finding::Dry {
                temp_c: 70.0,
                hours: 4.5,
            },
            Finding::Hardened,
            Finding::Enclosure,
            Finding::DirectDrive,
        ];
        for f in &all {
            let en = f.text(Lang::En);
            assert!(!en.contains("{}"), "{en}");
            for l in Lang::ALL {
                let t = f.text(l);
                assert!(
                    !t.trim().is_empty() && !t.contains("{}"),
                    "{f:?} in {}",
                    l.code()
                );
            }
        }
        // whole numbers have no decimals, the others have one
        assert!(all[1].text(Lang::En).contains("110.5") && all[1].text(Lang::En).contains(" 0 "));
        assert!(all[3].text(Lang::Pt).contains("4.5"));
    }

    #[test]
    fn the_database_gives_the_records_by_identifier_and_a_made_up_one_has_none() {
        assert!(record("generic/generic-petg").is_some());
        assert!(
            record("generic-petg").is_none(),
            "the old identifier is for the catalog, not for the record"
        );
        assert!(record("unobtainium").is_none());
    }
}
