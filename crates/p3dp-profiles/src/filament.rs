//! The filaments: generic profiles of the six families of material that most printers are fed (PLA, PETG, ABS, ASA, TPU and PA), each with the ranges of temperature of its family, and the
//! check of a filament against a printer and a nozzle. Nothing is lowered or raised to make a pair fit: a pair that does not fit is a list of reasons, and a pair that does gives the
//! temperatures to use, which never go past the limit of the printer.
//!
//! The ranges are the middle of the road of each family, from the figures printed on generic spools and public data sheets; the label of the spool wins, and a profile file can
//! replace any row. None of them has been tried on a machine.

use crate::profile::{PrinterProfile, ProfileError, Technology};
use eh3d_printmaterials::model::Filament as Record;
use eh3d_printmaterials::query::Database;
use p3dp_core::kv;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The polymers of the database of the suite (the same type, so a program that reads that database and this catalog speaks of the same materials).
pub use eh3d_printmaterials::model::Material;

/// A range with the value to start from: `lowest <= best <= highest`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub best: f64,
    pub lowest: f64,
    pub highest: f64,
}

impl Range {
    fn check(&self, what: &str, id: &str, max: f64) -> Result<(), ProfileError> {
        let ok = [self.best, self.lowest, self.highest]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= max)
            && self.lowest <= self.best
            && self.best <= self.highest;
        if ok {
            Ok(())
        } else {
            Err(ProfileError(format!(
                "filament {id}: the range of {what} ({} / {} / {}) is not 0 to {max} with the lowest first and the highest last",
                self.lowest, self.best, self.highest
            )))
        }
    }
}

/// A filament.
#[derive(Debug, Clone, PartialEq)]
pub struct FilamentProfile {
    pub id: String,
    /// The name to show: the maker and the product (a profile of the generic family has no maker in it).
    pub name: String,
    /// The maker, as the database writes it (empty for a profile that has none).
    pub manufacturer: String,
    pub material: Material,
    pub nozzle_c: Range,
    pub bed_c: Range,
    pub fan_percent: Range,
    pub density_g_cm3: f64,
    pub diameter_mm: f64,
    /// The smallest nozzle the filament goes through without trouble.
    pub min_nozzle_mm: f64,
    /// Whether it wears a brass nozzle (filled with carbon or glass).
    pub abrasive: bool,
    pub enclosure_advised: bool,
    pub direct_drive_advised: bool,
    pub verified: bool,
}

/// The nozzle that is on the printer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nozzle {
    pub diameter_mm: f64,
    pub hardened: bool,
}

/// Why a filament cannot be used on a printer with a nozzle.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    /// The printer does not get hot enough: the lowest temperature of the filament and the limit of the printer.
    NozzleTooCold { lowest_c: f64, limit_c: f64 },
    /// The bed does not get hot enough.
    BedTooCold { lowest_c: f64, limit_c: f64 },
    /// The printer does not come with that nozzle.
    NozzleNotOffered { diameter_mm: f64 },
    /// The nozzle is smaller than the filament goes through.
    NozzleTooSmall { diameter_mm: f64, smallest_mm: f64 },
    /// The filament is abrasive and the nozzle is not hardened.
    NeedsHardenedNozzle,
    /// A resin printer takes no filament.
    NotAFilamentPrinter,
}

impl Issue {
    pub fn text(&self) -> String {
        match self {
            Issue::NozzleTooCold { lowest_c, limit_c } => format!(
                "the filament needs the nozzle at {lowest_c} C at the least and the limit of the printer is {limit_c} C"
            ),
            Issue::BedTooCold { lowest_c, limit_c } => format!(
                "the filament needs the bed at {lowest_c} C at the least and the limit of the printer is {limit_c} C"
            ),
            Issue::NozzleNotOffered { diameter_mm } => {
                format!("the printer does not list a nozzle of {diameter_mm} mm")
            }
            Issue::NozzleTooSmall {
                diameter_mm,
                smallest_mm,
            } => format!(
                "a nozzle of {diameter_mm} mm is smaller than the {smallest_mm} mm that this filament goes through"
            ),
            Issue::NeedsHardenedNozzle => {
                "the filament wears a brass nozzle: use a hardened one".to_string()
            }
            Issue::NotAFilamentPrinter => "a resin printer takes no filament".to_string(),
        }
    }
}

/// What a person should look at although the pair works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice {
    /// The filament prints better in a warm closed space and the profile has no heated chamber (the printer may still be closed).
    Enclosure,
    /// The filament is soft and goes best through a direct drive extruder, which the profile does not say the printer has.
    DirectDrive,
}

impl Advice {
    pub fn text(self) -> &'static str {
        match self {
            Advice::Enclosure => {
                "it prints better in a closed, warm space and the profile has no heated chamber"
            }
            Advice::DirectDrive => {
                "it is soft and goes best through a direct drive extruder, which the profile does not say"
            }
        }
    }
}

/// What a filament comes to on a printer.
#[derive(Debug, Clone, PartialEq)]
pub struct Compatibility {
    pub issues: Vec<Issue>,
    pub advice: Vec<Advice>,
    /// The temperatures and the fan to use, never above the limits of the printer; `None` where an issue stops them.
    pub nozzle_c: Option<f64>,
    pub bed_c: Option<f64>,
    pub fan_percent: f64,
}

impl Compatibility {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }
}

impl FilamentProfile {
    /// Checks that a row makes sense.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let id = &self.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-/._+".contains(&b))
        {
            return Err(ProfileError(format!(
                "filament {id}: the id must be lower case letters, digits, hyphens, dots, plus signs, underscores and slashes"
            )));
        }
        self.nozzle_c.check("the nozzle", id, 500.0)?;
        self.bed_c.check("the bed", id, 200.0)?;
        self.fan_percent.check("the fan", id, 100.0)?;
        if !(self.density_g_cm3.is_finite() && (0.5..=5.0).contains(&self.density_g_cm3)) {
            return Err(ProfileError(format!(
                "filament {id}: the density is not 0.5 to 5"
            )));
        }
        if ![1.75, 2.85, 3.0].contains(&self.diameter_mm) {
            return Err(ProfileError(format!(
                "filament {id}: the diameter is 1.75, 2.85 or 3 mm"
            )));
        }
        if !(self.min_nozzle_mm.is_finite() && (0.1..=2.0).contains(&self.min_nozzle_mm)) {
            return Err(ProfileError(format!(
                "filament {id}: the smallest nozzle is not 0.1 to 2 mm"
            )));
        }
        Ok(())
    }

    /// The profile as the text of a profile file (`from_text` reads it back).
    pub fn to_text(&self) -> String {
        let r = |a: &Range| format!("{}, {}, {}", a.best, a.lowest, a.highest);
        let onoff = |b: bool| if b { "on" } else { "off" };
        format!(
            "id = {}\nname = {}\n{}material = {}\nnozzle_c = {}\nbed_c = {}\nfan_percent = {}\ndensity_g_cm3 = {}\ndiameter_mm = {}\nmin_nozzle_mm = {}\nabrasive = {}\nenclosure_advised = {}\ndirect_drive_advised = {}\nverified = {}\n",
            self.id,
            self.name,
            if self.manufacturer.is_empty() {
                String::new()
            } else {
                format!("manufacturer = {}\n", self.manufacturer)
            },
            self.material.as_str(),
            r(&self.nozzle_c),
            r(&self.bed_c),
            r(&self.fan_percent),
            self.density_g_cm3,
            self.diameter_mm,
            self.min_nozzle_mm,
            onoff(self.abrasive),
            onoff(self.enclosure_advised),
            onoff(self.direct_drive_advised),
            onoff(self.verified)
        )
    }

    /// Reads a profile file. Every key of `to_text` is understood; an unknown key, a missing one or a value out of range is an error that names the key. The last line of a key wins.
    pub fn from_text(text: &str) -> Result<FilamentProfile, ProfileError> {
        const KNOWN: [&str; 14] = [
            "id",
            "name",
            "manufacturer",
            "material",
            "nozzle_c",
            "bed_c",
            "fan_percent",
            "density_g_cm3",
            "diameter_mm",
            "min_nozzle_mm",
            "abrasive",
            "enclosure_advised",
            "direct_drive_advised",
            "verified",
        ];
        let entries = kv::parse(text).map_err(|e| ProfileError(e.to_string()))?;
        for e in &entries {
            if !KNOWN.contains(&e.key.as_str()) {
                return Err(ProfileError(format!(
                    "line {}: the key {} is not known",
                    e.line, e.key
                )));
            }
        }
        let get = |k: &str| {
            entries
                .iter()
                .rev()
                .find(|e| e.key == k)
                .map(|e| e.value.as_str())
        };
        let need = |k: &str| get(k).ok_or_else(|| ProfileError(format!("the key {k} is missing")));
        let num = |k: &str, lo: f64, hi: f64| -> Result<f64, ProfileError> {
            kv::float_in(k, need(k)?, lo, hi).map_err(|e| ProfileError(e.to_string()))
        };
        let range = |k: &str, hi: f64| -> Result<Range, ProfileError> {
            let parts: Vec<&str> = need(k)?.split(',').map(str::trim).collect();
            let [best, lowest, highest] = parts.as_slice() else {
                return Err(ProfileError(format!(
                    "{k}: three numbers are needed: the best, the lowest and the highest"
                )));
            };
            let one =
                |v: &str| kv::float_in(k, v, 0.0, hi).map_err(|e| ProfileError(e.to_string()));
            Ok(Range {
                best: one(best)?,
                lowest: one(lowest)?,
                highest: one(highest)?,
            })
        };
        let flag = |k: &str| -> Result<bool, ProfileError> {
            match get(k) {
                Some(v) => kv::on_off(k, v).map_err(|e| ProfileError(e.to_string())),
                None => Ok(false),
            }
        };
        let id = need("id")?.to_string();
        let p = FilamentProfile {
            name: need("name")?.to_string(),
            manufacturer: get("manufacturer").unwrap_or("").to_string(),
            material: Material::from_name(need("material")?).ok_or_else(|| {
                ProfileError(format!(
                    "filament {id}: material: not one of the polymers of the database (pla, petg, abs, asa, tpu, tpe, pa, pc, pp, pva, bvoh, hips, pet, ppa, pps, peek, pei, other)"
                ))
            })?,
            nozzle_c: range("nozzle_c", 500.0)?,
            bed_c: range("bed_c", 200.0)?,
            fan_percent: range("fan_percent", 100.0)?,
            density_g_cm3: num("density_g_cm3", 0.5, 5.0)?,
            diameter_mm: num("diameter_mm", 1.75, 3.0)?,
            min_nozzle_mm: num("min_nozzle_mm", 0.1, 2.0)?,
            abrasive: flag("abrasive")?,
            enclosure_advised: flag("enclosure_advised")?,
            direct_drive_advised: flag("direct_drive_advised")?,
            verified: flag("verified")?,
            id,
        };
        p.validate()?;
        Ok(p)
    }

    /// Checks the filament against a printer and a nozzle.
    pub fn check(&self, printer: &PrinterProfile, nozzle: Nozzle) -> Compatibility {
        let mut c = Compatibility {
            issues: Vec::new(),
            advice: Vec::new(),
            nozzle_c: None,
            bed_c: None,
            fan_percent: self.fan_percent.best,
        };
        let limits = match (printer.technology, printer.limits) {
            (Technology::Fdm, Some(l)) => l,
            _ => {
                c.issues.push(Issue::NotAFilamentPrinter);
                return c;
            }
        };
        if limits.nozzle_c < self.nozzle_c.lowest {
            c.issues.push(Issue::NozzleTooCold {
                lowest_c: self.nozzle_c.lowest,
                limit_c: limits.nozzle_c,
            });
        } else {
            c.nozzle_c = Some(self.nozzle_c.best.min(limits.nozzle_c));
        }
        if limits.bed_c < self.bed_c.lowest {
            c.issues.push(Issue::BedTooCold {
                lowest_c: self.bed_c.lowest,
                limit_c: limits.bed_c,
            });
        } else {
            c.bed_c = Some(self.bed_c.best.min(limits.bed_c));
        }
        if !printer
            .nozzles_mm
            .iter()
            .any(|n| (n - nozzle.diameter_mm).abs() < 1e-9)
        {
            c.issues.push(Issue::NozzleNotOffered {
                diameter_mm: nozzle.diameter_mm,
            });
        }
        if nozzle.diameter_mm < self.min_nozzle_mm - 1e-9 {
            c.issues.push(Issue::NozzleTooSmall {
                diameter_mm: nozzle.diameter_mm,
                smallest_mm: self.min_nozzle_mm,
            });
        }
        if self.abrasive && !nozzle.hardened {
            c.issues.push(Issue::NeedsHardenedNozzle);
        }
        if self.enclosure_advised && limits.chamber_c.is_none() {
            c.advice.push(Advice::Enclosure);
        }
        if self.direct_drive_advised {
            c.advice.push(Advice::DirectDrive);
        }
        c
    }
}

/// A set of filaments.
#[derive(Debug, Clone, PartialEq)]
pub struct FilamentCatalog {
    pub profiles: Vec<FilamentProfile>,
    /// The identifiers this program used before it read the database of the suite, each with the identifier that stands for it now (`generic-pla` is `generic/generic-pla`), so a settings file or a
    /// project that names an old one still finds its filament.
    pub aliases: Vec<(String, String)>,
}

type DbRange = eh3d_printmaterials::model::Range;

/// A range of the database as the three figures this program works with, or nothing when the record gives no lowest and highest.
fn range_of(r: Option<DbRange>) -> Option<Range> {
    let r = r?;
    let (lo, hi) = (r.min?, r.max?);
    if lo > hi {
        return None;
    }
    Some(Range {
        best: r.recommended.unwrap_or((lo + hi) / 2.0).clamp(lo, hi),
        lowest: lo,
        highest: hi,
    })
}

/// The profile of a record of the database. What the record does not say (the temperatures and the fan of a filament whose maker publishes none, the density) comes from the generic profile of the
/// same polymer; a record with nothing to take from, or with a value out of range, gives nothing.
fn profile_of(f: &Record, generic: &HashMap<Material, &Record>) -> Option<FilamentProfile> {
    let base = generic.get(&f.material).copied();
    let nozzle_c = range_of(f.nozzle_c).or_else(|| range_of(base.and_then(|b| b.nozzle_c)))?;
    let bed_c = range_of(f.bed_c).or_else(|| range_of(base.and_then(|b| b.bed_c)))?;
    let fan_percent = range_of(f.fan_pct).or_else(|| range_of(base.and_then(|b| b.fan_pct)))?;
    let density_g_cm3 = f
        .density_g_cm3
        .or_else(|| base.and_then(|b| b.density_g_cm3))?;
    let manufacturer = f.manufacturer.trim().to_string();
    let own_name = manufacturer.is_empty()
        || manufacturer.eq_ignore_ascii_case("generic")
        || f.name
            .to_ascii_lowercase()
            .starts_with(&manufacturer.to_ascii_lowercase());
    let p = FilamentProfile {
        id: f.id.clone(),
        name: if own_name {
            f.name.clone()
        } else {
            format!("{manufacturer} {}", f.name)
        },
        manufacturer,
        material: f.material,
        nozzle_c,
        bed_c,
        fan_percent,
        density_g_cm3,
        diameter_mm: f.diameter_or_default(None).unwrap_or(1.75),
        min_nozzle_mm: f
            .min_nozzle_mm
            .or_else(|| base.and_then(|b| b.min_nozzle_mm))
            .unwrap_or(0.2),
        abrasive: f.needs_hardened_nozzle(),
        enclosure_advised: f
            .enclosure_advised
            .or_else(|| base.and_then(|b| b.enclosure_advised))
            .unwrap_or(false),
        direct_drive_advised: f
            .direct_drive_advised
            .or_else(|| base.and_then(|b| b.direct_drive_advised))
            .unwrap_or(false),
        verified: f.verified,
    };
    p.validate().ok()?;
    Some(p)
}

static BUILTIN: OnceLock<Result<FilamentCatalog, String>> = OnceLock::new();

impl FilamentCatalog {
    /// The filaments of the database of the suite (`eh3d-printmaterials`), the only source of this program: the generic profiles of the six families and every filament a maker publishes with
    /// its temperatures. A record that gives no temperature takes the one of the generic profile of its polymer; one that has none to take, or a value out of range, is left out.
    /// The catalog is made once and copied after that.
    pub fn builtin() -> Result<FilamentCatalog, ProfileError> {
        BUILTIN
            .get_or_init(|| {
                let db = Database::embedded().map_err(|e| {
                    e.first()
                        .map_or("the database of materials is not valid".to_string(), |x| {
                            x.to_string()
                        })
                })?;
                Ok(FilamentCatalog::from_database(&db))
            })
            .clone()
            .map_err(ProfileError)
    }

    /// The catalog of a database: the profiles in the order of the database, and the old identifiers (the `source_ref` of the generic profiles) as aliases.
    pub fn from_database(db: &Database) -> FilamentCatalog {
        let is_generic = |f: &Record| f.manufacturer.eq_ignore_ascii_case("generic");
        let generic: HashMap<Material, &Record> = db
            .filaments()
            .iter()
            .filter(|f| {
                is_generic(f)
                    && f.source_ref.as_deref() == Some(&format!("generic-{}", f.material.as_str()))
            })
            .map(|f| (f.material, f))
            .collect();
        let mut profiles = Vec::new();
        let mut aliases = Vec::new();
        for f in db.filaments() {
            let Some(p) = profile_of(f, &generic) else {
                continue;
            };
            if is_generic(f) {
                if let Some(old) = f
                    .source_ref
                    .as_deref()
                    .filter(|r| r.starts_with("generic-"))
                {
                    aliases.push((old.to_string(), p.id.clone()));
                }
            }
            profiles.push(p);
        }
        FilamentCatalog { profiles, aliases }
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// The filament with an id (any case), or with one of the identifiers the program used before the database.
    pub fn find(&self, id: &str) -> Option<&FilamentProfile> {
        let want = id.trim().to_ascii_lowercase();
        let want = self
            .aliases
            .iter()
            .find(|(old, _)| *old == want)
            .map_or(want, |(_, new)| new.clone());
        self.profiles.iter().find(|p| p.id == want)
    }

    /// The filaments of a material.
    pub fn of(&self, m: Material) -> Vec<&FilamentProfile> {
        self.profiles.iter().filter(|p| p.material == m).collect()
    }

    /// The makers that have a filament in the catalog, in alphabetical order (upper and lower case count the same).
    pub fn manufacturers(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .profiles
            .iter()
            .map(|p| p.manufacturer.as_str())
            .filter(|m| !m.is_empty())
            .collect();
        v.sort_by_key(|m| m.to_ascii_lowercase());
        v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use p3dp_core::TempLimits;

    fn fil(id: &str) -> FilamentProfile {
        FilamentCatalog::builtin()
            .unwrap()
            .find(id)
            .unwrap()
            .clone()
    }

    fn printer(id: &str) -> PrinterProfile {
        Catalog::builtin().unwrap().find(id).unwrap().clone()
    }

    const N04: Nozzle = Nozzle {
        diameter_mm: 0.4,
        hardened: false,
    };

    #[test]
    fn the_variants_with_fibre_fill_wear_a_brass_nozzle_and_are_refused_on_one() {
        let c = FilamentCatalog::builtin().unwrap();
        for id in [
            "generic-pla-cf",
            "generic-petg-cf",
            "generic-asa-cf",
            "generic-abs-gf",
            "generic-pa-cf",
        ] {
            let f = c.find(id).unwrap();
            assert!(f.abrasive, "{id}");
            assert!(f.min_nozzle_mm >= 0.4, "{id}");
            assert!(!f.verified, "{id}");
        }
        assert!(!c.find("generic-pla-plus").unwrap().abrasive);
        let a1 = printer("bambu-lab-a1");
        let brass = fil("generic-pla-cf").check(&a1, N04);
        assert!(
            brass.issues.contains(&Issue::NeedsHardenedNozzle),
            "{:?}",
            brass.issues
        );
        let hard = fil("generic-pla-cf").check(
            &a1,
            Nozzle {
                diameter_mm: 0.4,
                hardened: true,
            },
        );
        assert!(hard.ok(), "{:?}", hard.issues);
    }

    #[test]
    fn a_filament_profile_file_reads_back_what_was_written_and_a_mistake_names_the_key() {
        for p in FilamentCatalog::builtin().unwrap().profiles {
            assert_eq!(
                FilamentProfile::from_text(&p.to_text()).unwrap(),
                p,
                "{}",
                p.id
            );
        }
        let good = fil("generic-petg").to_text();
        for (from, to, want) in [
            ("material = petg", "material = wood", "material"),
            (
                "nozzle_c = 235, 220, 250",
                "nozzle_c = 235, 220",
                "nozzle_c",
            ),
            (
                "nozzle_c = 235, 220, 250",
                "nozzle_c = 300, 220, 250",
                "range of the nozzle",
            ),
            ("density_g_cm3 = 1.27", "density_g_cm3 = 9", "density_g_cm3"),
            ("diameter_mm = 1.75", "diameter_mm = 2", "diameter"),
            ("abrasive = off", "abrasive = sometimes", "abrasive"),
            ("min_nozzle_mm = 0.2\n", "", "min_nozzle_mm"),
        ] {
            let text = good.replacen(from, to, 1);
            assert_ne!(text, good, "{from}");
            let e = FilamentProfile::from_text(&text).unwrap_err().to_string();
            assert!(e.contains(want), "{from} -> {e}");
        }
        let e = FilamentProfile::from_text(&format!("{good}colour = red\n"))
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("colour") && e.contains(&format!("line {}", good.lines().count() + 1)),
            "{e}"
        );
        // a flag left out is off, and the last line of a key wins
        let short = good.replace("verified = off\n", "");
        assert!(!FilamentProfile::from_text(&short).unwrap().verified);
        let hot = format!("{good}bed_c = 85, 70, 90\n");
        assert_eq!(FilamentProfile::from_text(&hot).unwrap().bed_c.best, 85.0);
    }

    /// The twelve filaments this program carried before it read the database of the suite: (old id, material, nozzle, bed, fan, density, smallest nozzle, abrasive, enclosure, direct drive).
    #[allow(clippy::type_complexity)]
    const OLD_TABLE: [(
        &str,
        Material,
        [f64; 3],
        [f64; 3],
        [f64; 3],
        f64,
        f64,
        bool,
        bool,
        bool,
    ); 12] = [
        (
            "generic-pla",
            Material::Pla,
            [210.0, 190.0, 230.0],
            [60.0, 0.0, 70.0],
            [100.0, 80.0, 100.0],
            1.24,
            0.2,
            false,
            false,
            false,
        ),
        (
            "generic-petg",
            Material::Petg,
            [235.0, 220.0, 250.0],
            [80.0, 70.0, 90.0],
            [40.0, 30.0, 60.0],
            1.27,
            0.2,
            false,
            false,
            false,
        ),
        (
            "generic-abs",
            Material::Abs,
            [245.0, 230.0, 260.0],
            [100.0, 90.0, 110.0],
            [10.0, 0.0, 30.0],
            1.04,
            0.3,
            false,
            true,
            false,
        ),
        (
            "generic-asa",
            Material::Asa,
            [250.0, 235.0, 260.0],
            [100.0, 90.0, 110.0],
            [20.0, 0.0, 30.0],
            1.07,
            0.3,
            false,
            true,
            false,
        ),
        (
            "generic-tpu",
            Material::Tpu,
            [225.0, 210.0, 240.0],
            [50.0, 0.0, 60.0],
            [60.0, 30.0, 100.0],
            1.21,
            0.4,
            false,
            false,
            true,
        ),
        (
            "generic-pa",
            Material::Pa,
            [260.0, 240.0, 280.0],
            [80.0, 70.0, 100.0],
            [10.0, 0.0, 30.0],
            1.14,
            0.4,
            false,
            true,
            false,
        ),
        (
            "generic-pla-plus",
            Material::Pla,
            [215.0, 200.0, 235.0],
            [60.0, 0.0, 70.0],
            [100.0, 70.0, 100.0],
            1.24,
            0.2,
            false,
            false,
            false,
        ),
        (
            "generic-pla-cf",
            Material::Pla,
            [220.0, 200.0, 240.0],
            [60.0, 0.0, 70.0],
            [100.0, 70.0, 100.0],
            1.30,
            0.4,
            true,
            false,
            false,
        ),
        (
            "generic-petg-cf",
            Material::Petg,
            [250.0, 230.0, 270.0],
            [80.0, 70.0, 90.0],
            [40.0, 20.0, 60.0],
            1.30,
            0.4,
            true,
            false,
            false,
        ),
        (
            "generic-asa-cf",
            Material::Asa,
            [255.0, 240.0, 270.0],
            [100.0, 90.0, 110.0],
            [20.0, 0.0, 30.0],
            1.12,
            0.4,
            true,
            true,
            false,
        ),
        (
            "generic-abs-gf",
            Material::Abs,
            [250.0, 235.0, 270.0],
            [100.0, 90.0, 110.0],
            [10.0, 0.0, 30.0],
            1.12,
            0.4,
            true,
            true,
            false,
        ),
        (
            "generic-pa-cf",
            Material::Pa,
            [280.0, 260.0, 300.0],
            [90.0, 70.0, 110.0],
            [10.0, 0.0, 30.0],
            1.20,
            0.4,
            true,
            true,
            false,
        ),
    ];

    #[test]
    fn the_twelve_old_filaments_are_found_by_their_old_id_with_the_values_they_had() {
        let c = FilamentCatalog::builtin().unwrap();
        assert!(
            c.len() > 400,
            "the database of the suite is the only source: {}",
            c.len()
        );
        assert_eq!(c.aliases.len(), 12);
        for (id, m, nozzle, bed, fan, density, smallest, abrasive, enclosure, direct) in OLD_TABLE {
            let p = c.find(id).unwrap_or_else(|| panic!("{id}"));
            let t = |r: Range| [r.best, r.lowest, r.highest];
            assert_eq!(p.material, m, "{id}");
            assert_eq!(t(p.nozzle_c), nozzle, "{id}");
            assert_eq!(t(p.bed_c), bed, "{id}");
            assert_eq!(t(p.fan_percent), fan, "{id}");
            assert_eq!(
                (p.density_g_cm3, p.diameter_mm, p.min_nozzle_mm),
                (density, 1.75, smallest),
                "{id}"
            );
            assert_eq!(
                (p.abrasive, p.enclosure_advised, p.direct_drive_advised),
                (abrasive, enclosure, direct),
                "{id}"
            );
            assert!(!p.verified, "{id}");
            // the id of the database finds it as well, and an upper case old id too
            assert!(p.id.starts_with("generic/"), "{}", p.id);
            assert_eq!(c.find(&p.id), Some(p));
            assert_eq!(c.find(&id.to_uppercase()), Some(p));
        }
        assert!(c.find("unobtainium").is_none());
    }

    #[test]
    fn the_database_gives_every_family_and_each_profile_makes_sense() {
        let c = FilamentCatalog::builtin().unwrap();
        for m in [
            Material::Pla,
            Material::Petg,
            Material::Abs,
            Material::Asa,
            Material::Tpu,
            Material::Pa,
        ] {
            assert!(c.of(m).len() > 1, "{}", m.as_str());
            assert_eq!(Material::from_name(&m.as_str().to_uppercase()), Some(m));
        }
        assert_eq!(Material::from_name("wood"), None);
        for p in &c.profiles {
            p.validate().unwrap();
            assert!(!p.verified, "{}", p.id);
            assert!(
                !p.name.trim().is_empty() && !p.manufacturer.is_empty(),
                "{}",
                p.id
            );
        }
        // the ids are unique, and the makers come from the database
        let mut ids: Vec<&str> = c.profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), c.len());
        let makers = c.manufacturers();
        assert!(
            makers.len() > 10 && makers.contains(&"Generic"),
            "{makers:?}"
        );
        // the families by heat: PLA is the coolest, PA the hottest
        let t = |id: &str| c.find(id).unwrap().nozzle_c.best;
        assert!(t("generic-pla") < t("generic-petg") && t("generic-abs") < t("generic-pa"));
        // a filled filament wears a brass nozzle
        assert!(c
            .profiles
            .iter()
            .any(|p| p.abrasive && p.id != "generic/generic-pla-with-carbon-fibre"));
    }

    #[test]
    fn a_record_without_temperatures_takes_the_ones_of_its_polymer_and_one_with_nothing_to_take_is_left_out(
    ) {
        let db = Database::embedded().unwrap();
        let generic = FilamentCatalog::from_database(&db);
        let records = db.filaments();
        let base: HashMap<Material, &Record> = records
            .iter()
            .filter(|f| {
                f.id.starts_with("generic/")
                    && f.source_ref.as_deref() == Some(&format!("generic-{}", f.material.as_str()))
            })
            .map(|f| (f.material, f))
            .collect();
        let mut r = records
            .iter()
            .find(|f| f.material == Material::Pla && !f.id.starts_with("generic/"))
            .unwrap()
            .clone();
        r.nozzle_c = None;
        r.bed_c = None;
        r.fan_pct = None;
        r.density_g_cm3 = None;
        let p = profile_of(&r, &base).unwrap();
        let g = generic.find("generic-pla").unwrap();
        assert_eq!(
            (p.nozzle_c, p.bed_c, p.fan_percent, p.density_g_cm3),
            (g.nozzle_c, g.bed_c, g.fan_percent, g.density_g_cm3)
        );
        // a polymer with no generic profile and no temperatures gives nothing
        let mut peek = r.clone();
        peek.material = Material::Peek;
        assert!(profile_of(&peek, &base).is_none());
        // a range the wrong way round, or a value out of range, gives nothing
        let mut bad = r.clone();
        bad.nozzle_c = Some(eh3d_printmaterials::model::Range {
            min: Some(300.0),
            max: Some(200.0),
            recommended: None,
        });
        bad.bed_c = None;
        assert_eq!(
            profile_of(&bad, &base).map(|p| p.nozzle_c),
            Some(g.nozzle_c)
        );
        let mut hot = r.clone();
        hot.nozzle_c = Some(eh3d_printmaterials::model::Range {
            min: Some(100.0),
            max: Some(900.0),
            recommended: None,
        });
        assert!(profile_of(&hot, &base).is_none());
        // no recommended value: the middle of the range
        let mut mid = r.clone();
        mid.nozzle_c = Some(eh3d_printmaterials::model::Range {
            min: Some(200.0),
            max: Some(220.0),
            recommended: None,
        });
        assert_eq!(profile_of(&mid, &base).unwrap().nozzle_c.best, 210.0);
        // a maker's name goes before the product unless the product already starts with it
        assert!(
            p.name.starts_with(&p.manufacturer)
                || r.name
                    .to_ascii_lowercase()
                    .starts_with(&p.manufacturer.to_ascii_lowercase())
        );
    }

    #[test]
    fn the_printers_of_the_owner_take_the_common_materials_within_their_limits() {
        for id in ["bambu-lab-a1", "bambu-lab-p1s"] {
            let p = printer(id);
            for f in [
                "generic-pla",
                "generic-petg",
                "generic-abs",
                "generic-asa",
                "generic-tpu",
                "generic-pa",
            ] {
                let c = fil(f).check(&p, N04);
                assert!(c.ok(), "{id} {f}: {:?}", c.issues);
                let l = p.limits.unwrap();
                assert!(c.nozzle_c.unwrap() <= l.nozzle_c && c.bed_c.unwrap() <= l.bed_c);
            }
        }
        let c = fil("generic-pla").check(&printer("bambu-lab-a1"), N04);
        assert_eq!(
            (c.nozzle_c, c.bed_c, c.fan_percent),
            (Some(210.0), Some(60.0), 100.0)
        );
    }

    #[test]
    fn a_pair_that_does_not_fit_is_a_list_of_reasons_and_never_a_lowered_temperature() {
        // a bed that stops at 80 C does not print ABS, whose bed starts at 90
        let mini = printer("bambu-lab-a1-mini");
        let c = fil("generic-abs").check(&mini, N04);
        assert_eq!(
            c.issues,
            vec![Issue::BedTooCold {
                lowest_c: 90.0,
                limit_c: 80.0
            }]
        );
        assert_eq!((c.bed_c, c.nozzle_c), (None, Some(245.0)));
        assert!(c.issues[0].text().contains("80"));
        // a printer whose limit is under the range of the filament
        let mut cold = printer("marlin-generic");
        cold.limits = Some(TempLimits {
            nozzle_c: 220.0,
            bed_c: 100.0,
            chamber_c: None,
        });
        let c = fil("generic-pa").check(&cold, N04);
        assert!(matches!(
            c.issues[0],
            Issue::NozzleTooCold { lowest_c, limit_c } if lowest_c == 240.0 && limit_c == 220.0
        ));
        assert_eq!(c.nozzle_c, None);
        // the temperature to use is the best one, cut at the limit of the printer
        assert_eq!(fil("generic-pla").check(&cold, N04).nozzle_c, Some(210.0));
        let mut warm = cold.clone();
        warm.limits = Some(TempLimits {
            nozzle_c: 200.0,
            bed_c: 50.0,
            chamber_c: None,
        });
        let c = fil("generic-pla").check(&warm, N04);
        assert_eq!((c.nozzle_c, c.bed_c), (Some(200.0), Some(50.0)));
    }

    #[test]
    fn the_nozzle_has_to_be_one_the_printer_offers_one_the_filament_goes_through_and_hard_enough() {
        let p = printer("bambu-lab-a1");
        let n = |d, h| Nozzle {
            diameter_mm: d,
            hardened: h,
        };
        assert!(fil("generic-pla").check(&p, n(0.2, false)).ok());
        let c = fil("generic-tpu").check(&p, n(0.2, false));
        assert_eq!(
            c.issues,
            vec![Issue::NozzleTooSmall {
                diameter_mm: 0.2,
                smallest_mm: 0.4
            }]
        );
        let c = fil("generic-pla").check(&p, n(0.5, false));
        assert_eq!(c.issues, vec![Issue::NozzleNotOffered { diameter_mm: 0.5 }]);
        let mut cf = fil("generic-pa");
        cf.abrasive = true;
        assert_eq!(
            cf.check(&p, n(0.4, false)).issues,
            vec![Issue::NeedsHardenedNozzle]
        );
        assert!(cf.check(&p, n(0.4, true)).ok());
    }

    #[test]
    fn what_helps_is_advised_without_refusing() {
        let p = printer("bambu-lab-a1");
        assert_eq!(
            fil("generic-tpu").check(&p, N04).advice,
            vec![Advice::DirectDrive]
        );
        assert_eq!(
            fil("generic-abs").check(&p, N04).advice,
            vec![Advice::Enclosure]
        );
        assert!(fil("generic-pla").check(&p, N04).advice.is_empty());
        assert!(Advice::Enclosure.text().contains("closed"));
        // a printer with a heated chamber needs no advice about one
        let x = printer("prusa-core-one");
        assert!(x.limits.unwrap().chamber_c.is_some());
        assert!(!fil("generic-asa")
            .check(&x, N04)
            .advice
            .contains(&Advice::Enclosure));
        // a resin printer takes no filament
        let resin = printer("elegoo-mars-4");
        let c = fil("generic-pla").check(&resin, N04);
        assert_eq!(c.issues, vec![Issue::NotAFilamentPrinter]);
        assert!(!c.ok() && c.issues[0].text().contains("resin"));
    }
}
