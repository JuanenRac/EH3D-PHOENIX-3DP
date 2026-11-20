//! The lists of printers and of filaments of the window, with their filters, without the window. A filter only chooses what is shown: it never changes a profile, and the printer
//! that is chosen stays chosen when a filter hides it.

use p3dp_profiles::filament::{Compatibility, FilamentCatalog, FilamentProfile, Material, Nozzle};
use p3dp_profiles::{Catalog, PrinterProfile, Technology};

/// What chooses the printers that are listed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrinterFilter {
    /// Part of the id, the vendor or the model, in any case.
    pub search: String,
    pub technology: Option<Technology>,
    pub vendor: Option<String>,
    /// Only the printers a model of this size (x, y, z in millimetres) fits in.
    pub fits_size_mm: Option<[f64; 3]>,
}

impl PrinterFilter {
    /// Whether one profile passes.
    pub fn allows(&self, p: &PrinterProfile) -> bool {
        let q = self.search.trim().to_ascii_lowercase();
        let text = q.is_empty()
            || p.id.contains(&q)
            || p.vendor.to_ascii_lowercase().contains(&q)
            || p.model.to_ascii_lowercase().contains(&q);
        let tech = self.technology.is_none_or(|t| p.technology == t);
        let vendor = self.vendor.as_ref().is_none_or(|v| &p.vendor == v);
        let fits = self.fits_size_mm.is_none_or(|s| {
            // the model may be turned about the vertical axis, so the two sides on the plate are compared in the better order
            let (a, b) = if s[0] >= s[1] {
                (s[0], s[1])
            } else {
                (s[1], s[0])
            };
            let (px, py) = if p.volume_mm[0] >= p.volume_mm[1] {
                (p.volume_mm[0], p.volume_mm[1])
            } else {
                (p.volume_mm[1], p.volume_mm[0])
            };
            a <= px + 1e-9 && b <= py + 1e-9 && s[2] <= p.volume_mm[2] + 1e-9
        });
        text && tech && vendor && fits
    }

    /// The profiles that pass, in the order of the catalog.
    pub fn apply<'a>(&self, catalog: &'a Catalog) -> Vec<&'a PrinterProfile> {
        catalog.profiles.iter().filter(|p| self.allows(p)).collect()
    }

    /// The vendors worth listing for the technology chosen, each once.
    pub fn vendors<'a>(&self, catalog: &'a Catalog) -> Vec<&'a str> {
        let mut v: Vec<&str> = Vec::new();
        for p in &catalog.profiles {
            if self.technology.is_none_or(|t| p.technology == t) && !v.contains(&p.vendor.as_str())
            {
                v.push(&p.vendor);
            }
        }
        v
    }

    /// Whether any filter is on.
    pub fn is_active(&self) -> bool {
        !self.search.trim().is_empty()
            || self.technology.is_some()
            || self.vendor.is_some()
            || self.fits_size_mm.is_some()
    }

    /// Takes every filter off.
    pub fn clear(&mut self) {
        *self = PrinterFilter::default();
    }
}

/// What chooses the filaments that are listed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FilamentFilter {
    /// Words to look for in the name, the maker, the material and the id: every word has to be in at least one of them (`pla silk polymaker`).
    pub search: String,
    pub material: Option<Material>,
    /// Only the filaments of this maker (upper and lower case count the same).
    pub manufacturer: Option<String>,
    /// Only the filaments the printer can use with this nozzle: the printer chosen and the nozzle of the program.
    pub compatible_with: Option<Nozzle>,
}

/// A filament in the list, with how it goes with the printer when that is asked.
#[derive(Debug, Clone, PartialEq)]
pub struct FilamentRow<'a> {
    pub profile: &'a FilamentProfile,
    pub compatibility: Option<Compatibility>,
}

impl FilamentFilter {
    /// Whether every word is in the name, the maker, the material or the id of the filament.
    fn matches_words(&self, f: &FilamentProfile, words: &[String]) -> bool {
        if words.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            f.name,
            f.manufacturer,
            f.material.as_str(),
            f.id
        )
        .to_ascii_lowercase();
        words.iter().all(|w| haystack.contains(w.as_str()))
    }

    /// Whether any filter is on.
    pub fn is_active(&self) -> bool {
        !self.search.trim().is_empty()
            || self.material.is_some()
            || self.manufacturer.is_some()
            || self.compatible_with.is_some()
    }

    /// The filaments that pass for a printer, in the order of the catalog. The compatibility is worked out when it is asked for.
    pub fn apply<'a>(
        &self,
        filaments: &'a FilamentCatalog,
        printer: Option<&PrinterProfile>,
    ) -> Vec<FilamentRow<'a>> {
        let words: Vec<String> = self
            .search
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        filaments
            .profiles
            .iter()
            .filter(|f| self.matches_words(f, &words))
            .filter(|f| self.material.is_none_or(|m| f.material == m))
            .filter(|f| {
                self.manufacturer
                    .as_deref()
                    .is_none_or(|m| f.manufacturer.eq_ignore_ascii_case(m))
            })
            .filter_map(|f| {
                let compatibility = match (self.compatible_with, printer) {
                    (Some(nozzle), Some(p)) => {
                        let c = f.check(p, nozzle);
                        if !c.ok() {
                            return None;
                        }
                        Some(c)
                    }
                    _ => None,
                };
                Some(FilamentRow {
                    profile: f,
                    compatibility,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        Catalog::builtin().unwrap()
    }

    #[test]
    fn no_filter_shows_the_whole_catalog_in_its_order() {
        let c = catalog();
        let f = PrinterFilter::default();
        assert!(!f.is_active());
        let all = f.apply(&c);
        assert_eq!(all.len(), c.len());
        assert!(all.iter().zip(&c.profiles).all(|(a, b)| a.id == b.id));
    }

    #[test]
    fn the_filters_narrow_the_list_and_agree_with_the_search_of_the_catalog() {
        let c = catalog();
        let mut f = PrinterFilter {
            search: "  PRUSA ".to_string(),
            ..PrinterFilter::default()
        };
        let by_filter: Vec<&str> = f.apply(&c).iter().map(|p| p.id.as_str()).collect();
        let by_catalog: Vec<&str> = c.search("prusa").iter().map(|p| p.id.as_str()).collect();
        assert_eq!(by_filter, by_catalog);
        assert!(!by_filter.is_empty() && f.is_active());
        f.technology = Some(Technology::Msla);
        // both filters together: a subset of each
        let both = f.apply(&c);
        assert!(both.len() <= by_filter.len());
        assert!(both
            .iter()
            .all(|p| p.technology == Technology::Msla && by_filter.contains(&p.id.as_str())));
        f.search.clear();
        let resin = f.apply(&c);
        assert_eq!(resin.len(), c.technology(Technology::Msla).len());
        assert!(resin.iter().all(|p| p.technology == Technology::Msla));
        // the vendors on offer follow the technology
        let vendors = f.vendors(&c);
        assert!(!vendors.is_empty());
        f.vendor = Some(vendors[0].to_string());
        assert!(f.apply(&c).iter().all(|p| p.vendor == vendors[0]));
        f.clear();
        assert!(!f.is_active() && f.apply(&c).len() == c.len());
    }

    #[test]
    fn a_model_is_looked_for_a_bed_it_fits_even_turned_about() {
        let c = catalog();
        let big = PrinterFilter {
            fits_size_mm: Some([300.0, 300.0, 300.0]),
            ..PrinterFilter::default()
        };
        let small = PrinterFilter {
            fits_size_mm: Some([100.0, 100.0, 100.0]),
            ..PrinterFilter::default()
        };
        let (b, s) = (big.apply(&c), small.apply(&c));
        assert!(b.len() < s.len() && !b.is_empty());
        assert!(b
            .iter()
            .all(|p| p.volume_mm.iter().all(|v| *v >= 300.0 - 1e-9)));
        // 250 x 100 fits a bed of 100 x 250 when it is turned
        let p = c
            .profiles
            .iter()
            .find(|p| p.volume_mm[0] != p.volume_mm[1])
            .cloned();
        if let Some(p) = p {
            let (long, short) = (
                p.volume_mm[0].max(p.volume_mm[1]),
                p.volume_mm[0].min(p.volume_mm[1]),
            );
            let f = PrinterFilter {
                fits_size_mm: Some([short, long, 1.0]),
                ..PrinterFilter::default()
            };
            assert!(f.allows(&p));
            let too_tall = PrinterFilter {
                fits_size_mm: Some([1.0, 1.0, p.volume_mm[2] + 1.0]),
                ..PrinterFilter::default()
            };
            assert!(!too_tall.allows(&p));
        }
    }

    #[test]
    fn the_filaments_are_filtered_by_name_by_material_and_by_what_the_printer_can_use() {
        let fil = FilamentCatalog::builtin().unwrap();
        let c = catalog();
        let all = FilamentFilter::default().apply(&fil, None);
        assert_eq!(all.len(), fil.len());
        assert!(all.iter().all(|r| r.compatibility.is_none()));
        let pla = FilamentFilter {
            material: Some(Material::Pla),
            ..FilamentFilter::default()
        };
        let rows = pla.apply(&fil, None);
        assert_eq!(rows.len(), fil.of(Material::Pla).len());
        assert!(rows.iter().all(|r| r.profile.material == Material::Pla));
        let by_name = FilamentFilter {
            search: "PETG".to_string(),
            ..FilamentFilter::default()
        };
        let named = by_name.apply(&fil, None);
        assert!(!named.is_empty());
        assert!(named.iter().all(|r| {
            let p = r.profile;
            p.material == Material::Petg || p.name.to_ascii_lowercase().contains("petg")
        }));
        // a printer that cannot reach the heat of the engineering materials leaves them out
        let nozzle = Nozzle {
            diameter_mm: 0.4,
            hardened: false,
        };
        let ok_only = FilamentFilter {
            compatible_with: Some(nozzle),
            ..FilamentFilter::default()
        };
        let mini = c.find("bambu-lab-a1-mini").unwrap();
        let with = ok_only.apply(&fil, Some(mini));
        assert!(!with.is_empty() && with.len() <= fil.len());
        assert!(with
            .iter()
            .all(|r| r.compatibility.as_ref().is_some_and(Compatibility::ok)));
        // the temperatures to use never go past the limit of the printer
        let limit = mini.limits.unwrap().nozzle_c;
        assert!(with.iter().all(|r| r
            .compatibility
            .as_ref()
            .unwrap()
            .nozzle_c
            .is_none_or(|t| t <= limit)));
        // with no printer chosen the compatibility cannot be told and nothing is left out
        assert_eq!(ok_only.apply(&fil, None).len(), fil.len());
        // a resin printer takes none
        let resin = c.find("elegoo-mars-4").unwrap();
        assert!(ok_only.apply(&fil, Some(resin)).is_empty());
    }

    #[test]
    fn the_filaments_are_looked_for_by_maker_and_by_material_with_several_words() {
        let fil = FilamentCatalog::builtin().unwrap();
        let makers = fil.manufacturers();
        let maker = makers
            .iter()
            .find(|m| !m.eq_ignore_ascii_case("generic"))
            .unwrap();
        let by_maker = FilamentFilter {
            manufacturer: Some(maker.to_ascii_uppercase()),
            ..FilamentFilter::default()
        };
        let rows = by_maker.apply(&fil, None);
        assert!(!rows.is_empty() && by_maker.is_active());
        assert!(rows
            .iter()
            .all(|r| r.profile.manufacturer.eq_ignore_ascii_case(maker)));
        // the words of the search meet in the maker, the material and the name, in any order
        let words = FilamentFilter {
            search: format!("  {}   PLA ", maker.to_ascii_lowercase()),
            ..FilamentFilter::default()
        };
        let found = words.apply(&fil, None);
        assert!(found.len() <= rows.len());
        for r in &found {
            assert!(r.profile.manufacturer.eq_ignore_ascii_case(maker));
            let all = format!("{} {}", r.profile.name, r.profile.id).to_ascii_lowercase();
            assert!(
                r.profile.material == Material::Pla || all.contains("pla"),
                "{}",
                r.profile.id
            );
        }
        // a word that is nowhere finds nothing, and the old id still finds the generic PLA
        let none = FilamentFilter {
            search: "pla zzzz-nothing".to_string(),
            ..FilamentFilter::default()
        };
        assert!(none.apply(&fil, None).is_empty());
        let old = FilamentFilter {
            search: "generic pla".to_string(),
            material: Some(Material::Pla),
            ..FilamentFilter::default()
        };
        assert!(old
            .apply(&fil, None)
            .iter()
            .any(|r| r.profile.id == "generic/generic-pla"));
        assert!(!FilamentFilter::default().is_active());
    }
}
