//! The keyboard shortcuts of the window, without the window: the actions that have one, the chords (`Ctrl+Shift+S`), the scheme of the family of programs of the suite and a scheme
//! of its own, and the changes a person writes in the settings file (`key_open = Ctrl+Alt+O`, `key_slice = none`). Two actions on one chord is an error that names both, a chord
//! that cannot be read is an error that names the line, and nothing is guessed. `Ctrl` is the command key on a Mac.

use crate::i18n::{Lang, Msg};

/// What a shortcut can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Open,
    Save,
    Slice,
    NextTab,
    PreviousTab,
    GoPrepare,
    GoPreview,
    GoDevice,
    GoProject,
    Refresh,
    ResetView,
    FitView,
    LayerUp,
    LayerDown,
    EmergencyStop,
    ShowShortcuts,
}

impl Action {
    pub const ALL: [Action; 16] = [
        Action::Open,
        Action::Save,
        Action::Slice,
        Action::NextTab,
        Action::PreviousTab,
        Action::GoPrepare,
        Action::GoPreview,
        Action::GoDevice,
        Action::GoProject,
        Action::Refresh,
        Action::ResetView,
        Action::FitView,
        Action::LayerUp,
        Action::LayerDown,
        Action::EmergencyStop,
        Action::ShowShortcuts,
    ];

    /// The name in the settings file (`key_` is put in front of it).
    pub fn name(self) -> &'static str {
        match self {
            Action::Open => "open",
            Action::Save => "save",
            Action::Slice => "slice",
            Action::NextTab => "next_tab",
            Action::PreviousTab => "previous_tab",
            Action::GoPrepare => "go_prepare",
            Action::GoPreview => "go_preview",
            Action::GoDevice => "go_device",
            Action::GoProject => "go_project",
            Action::Refresh => "refresh",
            Action::ResetView => "reset_view",
            Action::FitView => "fit_view",
            Action::LayerUp => "layer_up",
            Action::LayerDown => "layer_down",
            Action::EmergencyStop => "emergency_stop",
            Action::ShowShortcuts => "show_shortcuts",
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        let n = name.trim().to_ascii_lowercase();
        Action::ALL.into_iter().find(|a| a.name() == n)
    }

    /// The text that names the action in the list of shortcuts.
    pub fn msg(self) -> Msg {
        match self {
            Action::Open => Msg::Open,
            Action::Save => Msg::Save,
            Action::Slice => Msg::SliceNow,
            Action::NextTab => Msg::NextTab,
            Action::PreviousTab => Msg::PreviousTab,
            Action::GoPrepare => Msg::TabPrepare,
            Action::GoPreview => Msg::TabPreview,
            Action::GoDevice => Msg::TabDevice,
            Action::GoProject => Msg::TabProject,
            Action::Refresh => Msg::Refresh,
            Action::ResetView => Msg::ViewReset,
            Action::FitView => Msg::ViewFit,
            Action::LayerUp => Msg::LayerUp,
            Action::LayerDown => Msg::LayerDown,
            Action::EmergencyStop => Msg::EmergencyStop,
            Action::ShowShortcuts => Msg::ShortcutsTitle,
        }
    }
}

/// The names of the keys that a chord may hold.
fn key_names() -> Vec<String> {
    let mut v: Vec<String> = ('A'..='Z').map(|c| c.to_string()).collect();
    v.extend(('0'..='9').map(|c| c.to_string()));
    v.extend((1..=12).map(|n| format!("F{n}")));
    v.extend(
        [
            "Up", "Down", "Left", "Right", "PageUp", "PageDown", "Home", "End", "Tab", "Escape",
            "Enter", "Space", "Delete",
        ]
        .map(String::from),
    );
    v
}

/// A key with the modifiers held down.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// One of the names of [`key_names`], in the spelling they are listed in.
    pub key: String,
}

impl Chord {
    pub fn new(ctrl: bool, alt: bool, shift: bool, key: &str) -> Chord {
        Chord {
            ctrl,
            alt,
            shift,
            key: key.to_string(),
        }
    }

    /// Reads `Ctrl+Shift+S`: modifiers and a key joined by `+`, in any case and order, with spaces allowed around the signs.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mods) = match parts.split_last() {
            Some((k, m)) if !k.is_empty() => (*k, m),
            _ => {
                return Err(format!(
                    "{text:?} is not a chord: write modifiers and a key, as Ctrl+S"
                ))
            }
        };
        let mut chord = Chord::new(false, false, false, "");
        for m in mods {
            let flag = match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" => &mut chord.ctrl,
                "alt" | "option" => &mut chord.alt,
                "shift" => &mut chord.shift,
                other => return Err(format!("{other:?} is not a modifier (Ctrl, Alt or Shift)")),
            };
            if *flag {
                return Err(format!("{text:?} names a modifier twice"));
            }
            *flag = true;
        }
        let wanted = key.to_ascii_lowercase();
        chord.key = key_names()
            .into_iter()
            .find(|k| k.to_ascii_lowercase() == wanted)
            .ok_or_else(|| format!("{key:?} is not a key the shortcuts know"))?;
        Ok(chord)
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

/// The two schemes that come with the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchemeId {
    /// The one the programs of the family share.
    #[default]
    Family,
    /// The program's own: the Alt key for what is done by the menu, so the keys of a text box are never in the way.
    Own,
}

impl SchemeId {
    pub const ALL: [SchemeId; 2] = [SchemeId::Family, SchemeId::Own];

    pub fn name(self) -> &'static str {
        match self {
            SchemeId::Family => "family",
            SchemeId::Own => "own",
        }
    }

    pub fn from_name(name: &str) -> Option<SchemeId> {
        let n = name.trim().to_ascii_lowercase();
        SchemeId::ALL.into_iter().find(|s| s.name() == n)
    }

    pub fn msg(self) -> Msg {
        match self {
            SchemeId::Family => Msg::SchemeFamily,
            SchemeId::Own => Msg::SchemeOwn,
        }
    }

    fn default_chord(self, a: Action) -> Chord {
        let c = |ctrl, alt, shift, key: &str| Chord::new(ctrl, alt, shift, key);
        match self {
            SchemeId::Family => match a {
                Action::Open => c(true, false, false, "O"),
                Action::Save => c(true, false, false, "S"),
                Action::Slice => c(true, false, false, "R"),
                Action::NextTab => c(true, false, false, "Tab"),
                Action::PreviousTab => c(true, false, true, "Tab"),
                Action::GoPrepare => c(true, false, false, "1"),
                Action::GoPreview => c(true, false, false, "2"),
                Action::GoDevice => c(true, false, false, "3"),
                Action::GoProject => c(true, false, false, "4"),
                Action::Refresh => c(false, false, false, "F5"),
                Action::ResetView => c(false, false, false, "Home"),
                Action::FitView => c(true, false, false, "0"),
                Action::LayerUp => c(false, false, false, "Up"),
                Action::LayerDown => c(false, false, false, "Down"),
                Action::EmergencyStop => c(true, false, true, "E"),
                Action::ShowShortcuts => c(false, false, false, "F1"),
            },
            SchemeId::Own => match a {
                Action::Open => c(false, true, false, "O"),
                Action::Save => c(false, true, false, "S"),
                Action::Slice => c(false, true, false, "L"),
                Action::NextTab => c(false, true, false, "Right"),
                Action::PreviousTab => c(false, true, false, "Left"),
                Action::GoPrepare => c(false, true, false, "1"),
                Action::GoPreview => c(false, true, false, "2"),
                Action::GoDevice => c(false, true, false, "3"),
                Action::GoProject => c(false, true, false, "4"),
                Action::Refresh => c(false, false, false, "F5"),
                Action::ResetView => c(false, true, false, "Home"),
                Action::FitView => c(false, true, false, "F"),
                Action::LayerUp => c(false, false, false, "PageUp"),
                Action::LayerDown => c(false, false, false, "PageDown"),
                Action::EmergencyStop => c(false, true, true, "X"),
                Action::ShowShortcuts => c(false, false, false, "F1"),
            },
        }
    }
}

/// The shortcuts in force: one chord or none for each action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub base: SchemeId,
    bindings: Vec<(Action, Option<Chord>)>,
}

impl Scheme {
    /// A scheme as it comes with the program.
    pub fn of(base: SchemeId) -> Scheme {
        Scheme {
            base,
            bindings: Action::ALL
                .into_iter()
                .map(|a| (a, Some(base.default_chord(a))))
                .collect(),
        }
    }

    /// A scheme with the changes of a person: pairs of the name of an action (without `key_`) and a chord, or `none` to take the shortcut off. An unknown action, a chord that
    /// cannot be read or two actions on one chord is an error.
    pub fn with_changes(base: SchemeId, changes: &[(String, String)]) -> Result<Scheme, String> {
        let mut s = Scheme::of(base);
        for (name, value) in changes {
            let action =
                Action::from_name(name).ok_or_else(|| format!("key_{name}: no such action"))?;
            let chord = if value.trim().eq_ignore_ascii_case("none") {
                None
            } else {
                Some(Chord::parse(value).map_err(|e| format!("key_{name}: {e}"))?)
            };
            if let Some(slot) = s.bindings.iter_mut().find(|(a, _)| *a == action) {
                slot.1 = chord;
            }
        }
        s.check()?;
        Ok(s)
    }

    /// Fails when two actions are on one chord.
    pub fn check(&self) -> Result<(), String> {
        for (i, (a, ca)) in self.bindings.iter().enumerate() {
            let Some(ca) = ca else { continue };
            for (b, cb) in &self.bindings[i + 1..] {
                if cb.as_ref() == Some(ca) {
                    return Err(format!(
                        "{} is the shortcut of both {} and {}",
                        ca,
                        a.name(),
                        b.name()
                    ));
                }
            }
        }
        Ok(())
    }

    /// The chord of an action, if it has one.
    pub fn chord(&self, a: Action) -> Option<&Chord> {
        self.bindings
            .iter()
            .find(|(x, _)| *x == a)
            .and_then(|(_, c)| c.as_ref())
    }

    /// The action a chord stands for.
    pub fn action_for(&self, chord: &Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, c)| c.as_ref() == Some(chord))
            .map(|(a, _)| *a)
    }

    /// The lines of the list of shortcuts: the action and its chord (an empty text when it has none).
    pub fn rows(&self) -> Vec<(Action, String)> {
        self.bindings
            .iter()
            .map(|(a, c)| (*a, c.as_ref().map(Chord::to_string).unwrap_or_default()))
            .collect()
    }

    /// The list of shortcuts as lines of text in a language, `Open: Ctrl+O`: the name of each action is its own (no two actions share a name) and an action with no chord shows `-`.
    pub fn rows_text(&self, lang: Lang) -> Vec<String> {
        self.rows()
            .into_iter()
            .map(|(a, chord)| {
                let chord = if chord.is_empty() {
                    "-".to_string()
                } else {
                    chord
                };
                format!("{}: {chord}", a.msg().tr(lang))
            })
            .collect()
    }

    /// The lines of the changes against the scheme it began from, as they are written in the settings file.
    pub fn changes(&self) -> Vec<(String, String)> {
        let base = Scheme::of(self.base);
        self.bindings
            .iter()
            .zip(&base.bindings)
            .filter(|(mine, theirs)| mine.1 != theirs.1)
            .map(|((a, c), _)| {
                (
                    format!("key_{}", a.name()),
                    c.as_ref().map_or("none".to_string(), Chord::to_string),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_is_read_in_any_case_and_order_and_written_in_one_way() {
        for (text, want) in [
            ("ctrl+s", "Ctrl+S"),
            ("  Shift + CTRL + tab ", "Ctrl+Shift+Tab"),
            ("alt+shift+ctrl+f5", "Ctrl+Alt+Shift+F5"),
            ("Cmd+O", "Ctrl+O"),
            ("pageup", "PageUp"),
            ("F12", "F12"),
            ("7", "7"),
        ] {
            let c = Chord::parse(text).unwrap();
            assert_eq!(c.to_string(), want, "{text}");
            assert_eq!(Chord::parse(&c.to_string()).unwrap(), c);
        }
        for bad in [
            "",
            "+",
            "Ctrl+",
            "Ctrl+Ctrl+S",
            "Super+S",
            "Ctrl+F13",
            "Ctrl+SS",
            "S+Ctrl",
            "Ctrl+Shift+Shift+A",
        ] {
            assert!(Chord::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn both_schemes_bind_every_action_to_a_different_chord() {
        for id in SchemeId::ALL {
            let s = Scheme::of(id);
            assert!(s.check().is_ok(), "{}", id.name());
            for a in Action::ALL {
                let c = s
                    .chord(a)
                    .unwrap_or_else(|| panic!("{} has no chord in {}", a.name(), id.name()));
                assert_eq!(s.action_for(c), Some(a));
                assert_eq!(Chord::parse(&c.to_string()).unwrap(), *c);
            }
            assert!(s.changes().is_empty());
            assert_eq!(s.rows().len(), 16);
        }
        // the schemes are two: most of the actions are on other chords
        let (f, o) = (Scheme::of(SchemeId::Family), Scheme::of(SchemeId::Own));
        let different = Action::ALL
            .into_iter()
            .filter(|a| f.chord(*a) != o.chord(*a))
            .count();
        assert!(different >= 12, "{different}");
        // the family scheme has the usual chords of a program with files
        assert_eq!(f.chord(Action::Open).unwrap().to_string(), "Ctrl+O");
        assert_eq!(f.chord(Action::Save).unwrap().to_string(), "Ctrl+S");
    }

    #[test]
    fn the_emergency_stop_is_never_a_key_alone_in_any_scheme() {
        for id in SchemeId::ALL {
            let c = Scheme::of(id)
                .chord(Action::EmergencyStop)
                .cloned()
                .unwrap();
            assert!(c.shift && (c.ctrl || c.alt), "{}: {c}", id.name());
        }
    }

    #[test]
    fn a_person_changes_one_shortcut_takes_one_off_and_the_rest_stay() {
        let changes = vec![
            ("open".to_string(), "Ctrl+Alt+O".to_string()),
            ("refresh".to_string(), "none".to_string()),
        ];
        let s = Scheme::with_changes(SchemeId::Family, &changes).unwrap();
        assert_eq!(s.chord(Action::Open).unwrap().to_string(), "Ctrl+Alt+O");
        assert!(s.chord(Action::Refresh).is_none());
        assert_eq!(s.chord(Action::Save).unwrap().to_string(), "Ctrl+S");
        assert_eq!(s.action_for(&Chord::parse("ctrl+o").unwrap()), None);
        // the changes are written back as the file has them
        assert_eq!(
            s.changes(),
            vec![
                ("key_open".to_string(), "Ctrl+Alt+O".to_string()),
                ("key_refresh".to_string(), "none".to_string())
            ]
        );
        let rows = s.rows();
        assert_eq!(
            rows.iter().find(|(a, _)| *a == Action::Refresh).unwrap().1,
            ""
        );
    }

    #[test]
    fn two_actions_on_one_chord_a_chord_that_is_not_one_and_an_action_that_is_not_one_are_errors() {
        let err = |name: &str, value: &str| {
            Scheme::with_changes(SchemeId::Family, &[(name.to_string(), value.to_string())])
                .unwrap_err()
        };
        let e = err("open", "Ctrl+S");
        assert!(
            e.contains("Ctrl+S") && e.contains("open") && e.contains("save"),
            "{e}"
        );
        assert!(err("open", "Ctrl+").contains("key_open"));
        assert!(err("opn", "Ctrl+P").contains("no such action"));
        assert!(err("slice", "Hyper+R").contains("not a modifier"));
        // moving a shortcut onto one that was taken off is fine
        let both = vec![
            ("save".to_string(), "none".to_string()),
            ("open".to_string(), "Ctrl+S".to_string()),
        ];
        assert!(Scheme::with_changes(SchemeId::Family, &both).is_ok());
    }

    #[test]
    fn the_names_of_actions_and_schemes_are_read_back() {
        for a in Action::ALL {
            assert_eq!(Action::from_name(a.name()), Some(a));
            assert_eq!(Action::from_name(&a.name().to_uppercase()), Some(a));
        }
        let mut names: Vec<&str> = Action::ALL.iter().map(|a| a.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 16);
        for s in SchemeId::ALL {
            assert_eq!(SchemeId::from_name(s.name()), Some(s));
        }
        assert_eq!(SchemeId::from_name("other"), None);
        assert_eq!(SchemeId::default(), SchemeId::Family);
        // every action has a text in the eight languages
        for a in Action::ALL {
            for l in crate::i18n::Lang::ALL {
                assert!(!a.msg().tr(l).is_empty());
            }
        }
    }

    #[test]
    fn no_two_actions_have_the_same_name_in_the_list_of_shortcuts_in_any_language() {
        for l in Lang::ALL {
            let mut names: Vec<&str> = Action::ALL.iter().map(|a| a.msg().tr(l)).collect();
            names.sort_unstable();
            let before = names.len();
            names.dedup();
            assert_eq!(
                names.len(),
                before,
                "two actions share a name in {}",
                l.code()
            );
        }
        assert_eq!(Action::NextTab.msg().tr(Lang::En), "Next tab");
        assert_eq!(Action::PreviousTab.msg().tr(Lang::En), "Previous tab");
        assert_eq!(Action::PreviousTab.msg().tr(Lang::Pt), "Separador anterior");
        assert_eq!(Action::LayerDown.msg().tr(Lang::En), "Layer down");
    }

    #[test]
    fn the_lines_of_the_list_show_the_name_and_the_chord_and_a_dash_for_no_chord() {
        let changes = vec![("refresh".to_string(), "none".to_string())];
        let s = Scheme::with_changes(SchemeId::Family, &changes).unwrap();
        let en = s.rows_text(Lang::En);
        assert_eq!(en.len(), 16);
        assert!(en.contains(&"Open: Ctrl+O".to_string()));
        assert!(en.contains(&"Previous tab: Ctrl+Shift+Tab".to_string()));
        assert!(en.contains(&"Next tab: Ctrl+Tab".to_string()));
        assert!(en.contains(&"Prepare: Ctrl+1".to_string()));
        assert!(en.contains(&"Refresh: -".to_string()));
        let mut seen = en.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 16, "no line twice");
    }
}
