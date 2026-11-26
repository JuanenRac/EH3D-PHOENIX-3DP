//! Editing the temperature limits of a printer from the window, without the window: the draft a person types, the check of it against the rules of the profile, the warnings that go
//! with an accepted edit (a limit raised past what the maker states, a profile that is no longer marked as tried), and keeping the result in the settings so that it comes back the
//! next time. The limits of a model are the maker's unless the person changes them; a change is theirs and says so.

use crate::app_state::{AppState, Status};
use crate::config::{AppConfig, LimitOverride};
use crate::i18n::{fill, Lang, Msg};
use p3dp_profiles::{suite_overlay, Catalog, LimitSource, Notice, PrinterProfile, Technology};

/// What a person types: the three limits, in degrees Celsius. A chamber of `None` is a printer with no heated chamber.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitsDraft {
    pub nozzle_c: f64,
    pub bed_c: f64,
    pub chamber_c: Option<f64>,
}

/// Something to tell the person about an edit that was accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditWarning {
    /// A limit was raised past the figure the maker states for the model.
    PastMaker {
        key: &'static str,
        from: f64,
        to: f64,
    },
    /// A limit was raised (but the figure before was already the person's).
    Raised {
        key: &'static str,
        from: f64,
        to: f64,
    },
    /// The profile said it had been tried on a machine; an edited one has not been.
    NoLongerTried,
}

/// A draft that was accepted: the profile as it would be, and the warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub profile: PrinterProfile,
    pub warnings: Vec<EditWarning>,
}

impl EditWarning {
    /// The warning in a language: the key that changed, and what it means.
    pub fn text(self, lang: Lang) -> String {
        match self {
            EditWarning::PastMaker { key, from, to } => format!(
                "{} {}",
                fill(
                    Msg::NoticeLimitRaised.tr(lang),
                    &[key, &from.to_string(), &to.to_string()]
                ),
                Msg::NoticeAboveMaker.tr(lang)
            ),
            EditWarning::Raised { key, from, to } => fill(
                Msg::NoticeLimitRaised.tr(lang),
                &[key, &from.to_string(), &to.to_string()],
            ),
            EditWarning::NoLongerTried => Msg::NoticeVerificationCleared.tr(lang).to_string(),
        }
    }
}

impl LimitsDraft {
    /// The limits a profile has now. A resin printer has none.
    pub fn of(profile: &PrinterProfile) -> Option<LimitsDraft> {
        profile.limits.map(|l| LimitsDraft {
            nozzle_c: l.nozzle_c,
            bed_c: l.bed_c,
            chamber_c: l.chamber_c,
        })
    }

    /// Checks the draft against a profile: the edit is applied to a copy and the copy is checked as a whole by the rules of the profile. A resin printer has no such limits.
    pub fn check(&self, profile: &PrinterProfile) -> Result<Checked, String> {
        if profile.technology != Technology::Fdm {
            return Err("a resin printer has no nozzle and no heater limits".to_string());
        }
        for (what, v) in [("nozzle", self.nozzle_c), ("bed", self.bed_c)] {
            if !v.is_finite() {
                return Err(format!("{what}: not a number"));
            }
        }
        if self.chamber_c.is_some_and(|c| !c.is_finite()) {
            return Err("chamber: not a number".to_string());
        }
        let nozzle = format!("{}", self.nozzle_c);
        let bed = format!("{}", self.bed_c);
        let chamber = self.chamber_c.map_or(String::new(), |c| format!("{c}"));
        let edits = [
            ("nozzle_max_c", nozzle.as_str()),
            ("bed_max_c", bed.as_str()),
            ("chamber_max_c", chamber.as_str()),
        ];
        let edited = profile.edited(&edits).map_err(|e| e.to_string())?;
        let mut warnings = Vec::new();
        for n in &edited.notices {
            match *n {
                Notice::LimitRaised {
                    key,
                    from,
                    to,
                    above_maker,
                } => warnings.push(if above_maker {
                    EditWarning::PastMaker { key, from, to }
                } else {
                    EditWarning::Raised { key, from, to }
                }),
                Notice::VerificationCleared => warnings.push(EditWarning::NoLongerTried),
            }
        }
        Ok(Checked {
            profile: edited.profile,
            warnings,
        })
    }
}

/// Puts the limits the person set into a catalog. A line for a printer the catalog does not have, or that cannot be applied, is left out and named in the answer.
pub fn apply_overrides(catalog: &mut Catalog, overrides: &[LimitOverride]) -> Vec<String> {
    let mut skipped = Vec::new();
    for o in overrides {
        let Some(slot) = catalog.profiles.iter_mut().find(|p| p.id == o.printer_id) else {
            skipped.push(format!("{}: no such printer", o.printer_id));
            continue;
        };
        let draft = LimitsDraft {
            nozzle_c: o.nozzle_c,
            bed_c: o.bed_c,
            chamber_c: o.chamber_c,
        };
        match draft.check(slot) {
            Ok(c) => *slot = c.profile,
            Err(e) => skipped.push(format!("{}: {e}", o.printer_id)),
        }
    }
    skipped
}

impl AppState {
    /// Starts the state with the limits of the settings put in the catalog. A line that does not fit is told in the status, and the rest is used.
    pub fn with_limit_overrides(mut self) -> AppState {
        let overrides = self.config.limit_overrides.clone();
        let skipped = apply_overrides(&mut self.catalog, &overrides);
        if let Some(first) = skipped.first() {
            self.status = Status::Problem(first.clone());
        }
        self
    }

    /// Sets the limits of the printer that is chosen. The result is kept in the settings (so the caller can write them) and the warnings are returned. A refused draft changes
    /// nothing and the reason is the answer.
    pub fn set_printer_limits(&mut self, draft: &LimitsDraft) -> Result<Vec<EditWarning>, String> {
        let id = self.printer_id.clone();
        let Some(slot) = self.catalog.profiles.iter_mut().find(|p| p.id == id) else {
            return Err(format!("{id}: no such printer"));
        };
        let checked = draft.check(slot)?;
        *slot = checked.profile;
        let o = LimitOverride {
            printer_id: id.clone(),
            nozzle_c: draft.nozzle_c,
            bed_c: draft.bed_c,
            chamber_c: draft.chamber_c,
        };
        self.config.limit_overrides.retain(|x| x.printer_id != id);
        self.config.limit_overrides.push(o);
        Ok(checked.warnings)
    }

    /// Gives the printer that is chosen back the limits of the catalog (the maker's, or the careful ones the catalog starts with).
    pub fn reset_printer_limits(&mut self) -> bool {
        let id = self.printer_id.clone();
        let Ok(fresh) = suite_overlay().map(|o| o.catalog) else {
            return false;
        };
        let (Some(original), Some(slot)) = (
            fresh.find(&id).cloned(),
            self.catalog.profiles.iter_mut().find(|p| p.id == id),
        ) else {
            return false;
        };
        *slot = original;
        self.config.limit_overrides.retain(|x| x.printer_id != id);
        true
    }

    /// Whether the limits of the printer that is chosen are the person's.
    pub fn limits_are_mine(&self) -> bool {
        self.printer()
            .is_some_and(|p| p.limit_source == LimitSource::Owner)
    }
}

/// The lines of the settings that hold the limits of the printers, for a caller that merges them into the file.
pub fn override_lines(config: &AppConfig) -> Vec<(String, String)> {
    config
        .limit_overrides
        .iter()
        .map(|o| {
            (
                format!("{}{}", crate::config::LIMITS_KEY_PREFIX, o.printer_id),
                o.value_text(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    fn state() -> AppState {
        AppState::new(AppConfig::default(), Lang::En).unwrap()
    }

    #[test]
    fn a_draft_is_checked_by_the_rules_of_the_profile_and_a_raise_is_told() {
        let mut s = state();
        s.select_printer("bambu-lab-a1");
        let p = s.printer().unwrap().clone();
        let d = LimitsDraft::of(&p).unwrap();
        // no change: nothing to tell
        let same = d.check(&p).unwrap();
        assert_eq!(same.profile.limits, p.limits);
        assert!(same.warnings.is_empty());
        // a raise past the maker
        let hot = LimitsDraft {
            nozzle_c: d.nozzle_c + 20.0,
            ..d
        };
        let c = hot.check(&p).unwrap();
        assert!(c.warnings.iter().any(|w| matches!(
            w,
            EditWarning::PastMaker {
                key: "nozzle_max_c",
                ..
            }
        )));
        assert_eq!(c.profile.limit_source, LimitSource::Owner);
        // a lowering is no warning
        let cool = LimitsDraft {
            bed_c: d.bed_c - 10.0,
            ..d
        };
        assert!(cool.check(&p).unwrap().warnings.is_empty());
        // a chamber can be added
        let warm = LimitsDraft {
            chamber_c: Some(45.0),
            ..d
        };
        let c = warm.check(&p).unwrap();
        assert_eq!(c.profile.limits.unwrap().chamber_c, Some(45.0));
    }

    #[test]
    fn the_warnings_are_told_in_every_language_with_the_key_and_the_figures() {
        let past = EditWarning::PastMaker {
            key: "nozzle_max_c",
            from: 260.0,
            to: 290.0,
        };
        let raised = EditWarning::Raised {
            key: "bed_max_c",
            from: 100.0,
            to: 110.0,
        };
        for l in Lang::ALL {
            let t = past.text(l);
            assert!(
                t.contains("nozzle_max_c") && t.contains("260") && t.contains("290"),
                "{t}"
            );
            assert!(
                t.len() > raised.text(l).len(),
                "the one past the maker says more"
            );
            assert!(raised.text(l).contains("110") && !raised.text(l).contains("{}"));
            assert!(EditWarning::NoLongerTried.text(l).contains("verified"));
        }
        assert!(past.text(Lang::Es).contains("fabricante"));
    }

    #[test]
    fn nonsense_is_refused_with_the_reason_and_changes_nothing() {
        let mut s = state();
        s.select_printer("prusa-mk4s");
        let before = s.printer().unwrap().clone();
        let d = LimitsDraft::of(&before).unwrap();
        for bad in [
            LimitsDraft {
                nozzle_c: f64::NAN,
                ..d
            },
            LimitsDraft {
                bed_c: f64::INFINITY,
                ..d
            },
            LimitsDraft {
                chamber_c: Some(f64::NAN),
                ..d
            },
            LimitsDraft { nozzle_c: 0.0, ..d },
            LimitsDraft {
                nozzle_c: 900.0,
                ..d
            },
            LimitsDraft { bed_c: -5.0, ..d },
            LimitsDraft {
                chamber_c: Some(500.0),
                ..d
            },
        ] {
            assert!(s.set_printer_limits(&bad).is_err(), "{bad:?}");
        }
        assert_eq!(s.printer().unwrap(), &before);
        assert!(s.config.limit_overrides.is_empty());
        // a resin printer has none
        s.select_printer("elegoo-mars-4");
        assert!(LimitsDraft::of(s.printer().unwrap()).is_none());
        assert!(s.set_printer_limits(&d).is_err());
    }

    #[test]
    fn a_change_is_the_persons_is_kept_in_the_settings_and_can_be_undone() {
        let mut s = state();
        s.select_printer("prusa-mk4s");
        assert!(!s.limits_are_mine());
        let original = s.printer().unwrap().clone();
        let d = LimitsDraft {
            nozzle_c: 270.0,
            ..LimitsDraft::of(&original).unwrap()
        };
        s.set_printer_limits(&d).unwrap();
        assert!(s.limits_are_mine());
        assert_eq!(s.printer().unwrap().limits.unwrap().nozzle_c, 270.0);
        assert_eq!(s.config.limit_overrides.len(), 1);
        // a second edit of the same printer replaces the first
        s.set_printer_limits(&LimitsDraft {
            nozzle_c: 275.0,
            ..d
        })
        .unwrap();
        assert_eq!(s.config.limit_overrides.len(), 1);
        // the settings carry it to the next run
        let text = s.config.to_text(true);
        let mut again = AppState::new(AppConfig::from_text(&text).unwrap(), Lang::En)
            .unwrap()
            .with_limit_overrides();
        again.select_printer("prusa-mk4s");
        assert_eq!(again.printer().unwrap().limits.unwrap().nozzle_c, 275.0);
        let lines = override_lines(&s.config);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "printer_limits_prusa-mk4s");
        assert!(lines[0].1.starts_with("275,"), "{lines:?}");
        // and it can be given back
        assert!(s.reset_printer_limits());
        assert_eq!(s.printer().unwrap(), &original);
        assert!(s.config.limit_overrides.is_empty());
        assert!(!s.limits_are_mine());
    }

    #[test]
    fn a_line_for_a_printer_that_is_not_there_is_named_and_the_rest_is_used() {
        let cfg = AppConfig::from_text(
            "printer_limits_ghost = 250,80,none\nprinter_limits_prusa-mk4s = 270,100,none\nprinter_limits_elegoo-mars-4 = 250,80,none\n",
        )
        .unwrap();
        let s = AppState::new(cfg, Lang::En).unwrap().with_limit_overrides();
        assert!(s.status.is_problem());
        let mk4 = s.catalog.find("prusa-mk4s").unwrap();
        assert_eq!(mk4.limits.unwrap().nozzle_c, 270.0);
        assert!(s.catalog.find("elegoo-mars-4").unwrap().limits.is_none());
    }
}
