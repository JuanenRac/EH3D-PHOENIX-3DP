//! What a printer has for several materials or colours, read from the words the catalog keeps (`AMS`, `CFS (4 colours, up to 16)`, `toolchanger (up to 5)`, `IDEX`...), as a number
//! of tools the slicer can be asked for. The words come from the data sheets of the makers; a system that is only optional (a kit the owner may or may not have fitted) is told as
//! such, and the slicer asks the owner to say it is there before it plans for it.

use crate::profile::PrinterProfile;

/// The family of the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiKind {
    /// A unit that feeds one nozzle from several spools (AMS, CFS, ACE, Box, Canvas, MMU).
    Feeder,
    /// Several heads on a carriage or a rack (a toolchanger, IDEX).
    Heads,
    /// Something the owner adds (a feeder or a toolchanger through the firmware).
    External,
}

impl MultiKind {
    pub fn name(self) -> &'static str {
        match self {
            MultiKind::Feeder => "feeder",
            MultiKind::Heads => "heads",
            MultiKind::External => "external",
        }
    }
}

/// What a printer can print with at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiCaps {
    pub kind: MultiKind,
    /// The most tools (spools or heads) one print may use.
    pub max_tools: u8,
    /// The system is a kit that may not be fitted: the owner has to say it is.
    pub optional: bool,
    /// The words of the catalog.
    pub text: String,
}

/// The first whole number that follows `marker` in `text`.
fn number_after(text: &str, marker: &str) -> Option<u8> {
    let at = text.find(marker)? + marker.len();
    let digits: String = text[at..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The first whole number in the text.
fn first_number(text: &str) -> Option<u8> {
    let digits: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// What the catalog words say a printer has. `None` when the profile says nothing (a printer with one tool).
pub fn caps_of(p: &PrinterProfile) -> Option<MultiCaps> {
    let text = p.multimaterial.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }
    let low = text.to_ascii_lowercase();
    let optional = low.contains("optional");
    let (kind, max_tools) = if low.starts_with("toolchanger") {
        (MultiKind::Heads, number_after(&low, "up to").unwrap_or(4))
    } else if low.starts_with("idex") {
        (MultiKind::Heads, 2)
    } else if low.starts_with("ams") && low.contains("units") {
        // four slots in a unit, and so many units
        let units = number_after(&low, "up to").unwrap_or(1);
        (MultiKind::Feeder, 4u8.saturating_mul(units))
    } else if low.starts_with("mmu") {
        (MultiKind::Feeder, 5)
    } else if low.starts_with("optional") {
        (MultiKind::External, 4)
    } else {
        let listed = if low.contains("up to") {
            number_after(&low, "up to")
        } else {
            first_number(&low)
        };
        (MultiKind::Feeder, listed.unwrap_or(4))
    };
    Some(MultiCaps {
        kind,
        max_tools: max_tools.clamp(2, 16),
        optional: optional || kind == MultiKind::External,
        text: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn caps(id: &str) -> Option<MultiCaps> {
        let c = Catalog::builtin().unwrap();
        caps_of(c.find(id).unwrap_or_else(|| panic!("no profile {id}")))
    }

    #[test]
    fn the_words_of_the_catalog_give_the_tools() {
        let a1 = caps("bambu-lab-a1").unwrap();
        assert_eq!(
            (a1.kind, a1.max_tools, a1.optional),
            (MultiKind::Feeder, 4, false)
        );
        let xl = caps("prusa-xl").unwrap();
        assert_eq!((xl.kind, xl.max_tools), (MultiKind::Heads, 5));
        let mk4 = caps("prusa-mk4s").unwrap();
        assert_eq!(
            (mk4.kind, mk4.max_tools, mk4.optional),
            (MultiKind::Feeder, 5, true)
        );
        assert!(caps("prusa-mk4s").unwrap().text.contains("MMU3"));
    }

    #[test]
    fn a_printer_with_no_words_has_one_tool() {
        let c = Catalog::builtin().unwrap();
        let none = c
            .technology(crate::profile::Technology::Fdm)
            .into_iter()
            .find(|p| p.multimaterial.is_none())
            .unwrap();
        assert!(caps_of(none).is_none());
    }

    #[test]
    fn every_row_of_the_catalog_with_words_gives_two_to_sixteen_tools() {
        let c = Catalog::builtin().unwrap();
        let mut with_words = 0;
        for p in c.technology(crate::profile::Technology::Fdm) {
            if let Some(m) = p.multimaterial.as_deref().filter(|m| !m.trim().is_empty()) {
                with_words += 1;
                let k = caps_of(p).unwrap_or_else(|| panic!("{} says {m} and gives nothing", p.id));
                assert!((2..=16).contains(&k.max_tools), "{}: {}", p.id, k.max_tools);
                assert_eq!(k.text, m.trim());
            }
        }
        assert!(with_words >= 20, "{with_words}");
    }

    #[test]
    fn a_kit_that_may_not_be_fitted_says_so() {
        let optional = Catalog::builtin()
            .unwrap()
            .technology(crate::profile::Technology::Fdm)
            .into_iter()
            .filter_map(caps_of)
            .filter(|k| k.optional)
            .count();
        assert!(optional >= 5, "{optional}");
        assert_eq!(MultiKind::External.name(), "external");
    }
}
