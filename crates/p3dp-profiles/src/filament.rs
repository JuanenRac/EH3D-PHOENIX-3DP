//! The filaments: generic profiles of the six families of material that most printers are fed (PLA, PETG, ABS, ASA, TPU and PA), each with the ranges of temperature of its family, and the
//! check of a filament against a printer and a nozzle. Nothing is lowered or raised to make a pair fit: a pair that does not fit is a list of reasons, and a pair that does gives the
//! temperatures to use, which never go past the limit of the printer.
//!
//! The ranges are the middle of the road of each family, from the figures printed on generic spools and public data sheets; the label of the spool wins, and a profile file can
//! replace any row. None of them has been tried on a machine.

use crate::profile::{PrinterProfile, ProfileError, Technology};
use p3dp_core::kv;

/// The table of the program.
pub const DATA: &str = include_str!("../data/filaments.tsv");

/// The families of material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    Pla,
    Petg,
    Abs,
    Asa,
    Tpu,
    Pa,
}

impl Material {
    pub const ALL: [Material; 6] = [
        Material::Pla,
        Material::Petg,
        Material::Abs,
        Material::Asa,
        Material::Tpu,
        Material::Pa,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Material::Pla => "pla",
            Material::Petg => "petg",
            Material::Abs => "abs",
            Material::Asa => "asa",
            Material::Tpu => "tpu",
            Material::Pa => "pa",
        }
    }

    pub fn from_name(name: &str) -> Option<Material> {
        let want = name.trim().to_ascii_lowercase();
        Material::ALL.into_iter().find(|m| m.name() == want)
    }
}

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
    pub name: String,
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
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(ProfileError(format!(
                "filament {id}: the id must be lower case letters, digits and hyphens"
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
            "id = {}\nname = {}\nmaterial = {}\nnozzle_c = {}\nbed_c = {}\nfan_percent = {}\ndensity_g_cm3 = {}\ndiameter_mm = {}\nmin_nozzle_mm = {}\nabrasive = {}\nenclosure_advised = {}\ndirect_drive_advised = {}\nverified = {}\n",
            self.id,
            self.name,
            self.material.name(),
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
        const KNOWN: [&str; 13] = [
            "id",
            "name",
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
            material: Material::from_name(need("material")?).ok_or_else(|| {
                ProfileError(format!(
                    "filament {id}: material: not one of pla, petg, abs, asa, tpu, pa"
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
}

fn num(line: usize, name: &str, v: &str) -> Result<f64, ProfileError> {
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| ProfileError(format!("line {line}: {name} {v:?} is not a number")))
}

fn flag(line: usize, name: &str, v: &str) -> Result<bool, ProfileError> {
    match v.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ProfileError(format!(
            "line {line}: {name} {other:?} is not true or false"
        ))),
    }
}

impl FilamentCatalog {
    /// The filaments that come with the program.
    pub fn builtin() -> Result<FilamentCatalog, ProfileError> {
        FilamentCatalog::parse(DATA)
    }

    /// Reads a table of filaments; a row that makes no sense, a repeated id or a row with the wrong number of columns is an error that names the line.
    pub fn parse(text: &str) -> Result<FilamentCatalog, ProfileError> {
        let mut profiles: Vec<FilamentProfile> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let ln = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let c: Vec<&str> = line.split('|').collect();
            if c.len() != 19 {
                return Err(ProfileError(format!(
                    "line {ln}: {} columns, expected 19",
                    c.len()
                )));
            }
            let material = Material::from_name(c[2]).ok_or_else(|| {
                ProfileError(format!("line {ln}: the material {:?} is not known", c[2]))
            })?;
            let range = |name: &str, a: usize| -> Result<Range, ProfileError> {
                Ok(Range {
                    best: num(ln, name, c[a])?,
                    lowest: num(ln, name, c[a + 1])?,
                    highest: num(ln, name, c[a + 2])?,
                })
            };
            let p = FilamentProfile {
                id: c[0].trim().to_string(),
                name: c[1].trim().to_string(),
                material,
                nozzle_c: range("nozzle", 3)?,
                bed_c: range("bed", 6)?,
                fan_percent: range("fan", 9)?,
                density_g_cm3: num(ln, "density", c[12])?,
                diameter_mm: num(ln, "diameter", c[13])?,
                min_nozzle_mm: num(ln, "smallest nozzle", c[14])?,
                abrasive: flag(ln, "abrasive", c[15])?,
                enclosure_advised: flag(ln, "enclosure", c[16])?,
                direct_drive_advised: flag(ln, "direct drive", c[17])?,
                verified: flag(ln, "verified", c[18])?,
            };
            p.validate()
                .map_err(|e| ProfileError(format!("line {ln}: {e}")))?;
            if profiles.iter().any(|q| q.id == p.id) {
                return Err(ProfileError(format!(
                    "line {ln}: the id {} is used twice",
                    p.id
                )));
            }
            profiles.push(p);
        }
        if profiles.is_empty() {
            return Err(ProfileError("the table has no filaments".to_string()));
        }
        Ok(FilamentCatalog { profiles })
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// The filament with an id (any case).
    pub fn find(&self, id: &str) -> Option<&FilamentProfile> {
        let want = id.trim().to_ascii_lowercase();
        self.profiles.iter().find(|p| p.id == want)
    }

    /// The filaments of a material.
    pub fn of(&self, m: Material) -> Vec<&FilamentProfile> {
        self.profiles.iter().filter(|p| p.material == m).collect()
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
        assert!(e.contains("colour") && e.contains("line 14"), "{e}");
        // a flag left out is off, and the last line of a key wins
        let short = good.replace("verified = off\n", "");
        assert!(!FilamentProfile::from_text(&short).unwrap().verified);
        let hot = format!("{good}bed_c = 85, 70, 90\n");
        assert_eq!(FilamentProfile::from_text(&hot).unwrap().bed_c.best, 85.0);
    }

    #[test]
    fn the_six_families_are_there_with_ranges_that_make_sense() {
        let c = FilamentCatalog::builtin().unwrap();
        assert_eq!(c.len(), 12, "the six generic ones and six variants");
        for m in Material::ALL {
            assert!(!c.of(m).is_empty(), "{}", m.name());
            assert_eq!(
                c.of(m)
                    .iter()
                    .filter(|p| p.id == format!("generic-{}", m.name()))
                    .count(),
                1,
                "{}",
                m.name()
            );
            assert_eq!(Material::from_name(&m.name().to_uppercase()), Some(m));
        }
        assert_eq!(Material::from_name("wood"), None);
        for p in &c.profiles {
            p.validate().unwrap();
            assert!(!p.verified, "{}", p.id);
            assert_eq!(
                p.abrasive,
                p.id.ends_with("-cf") || p.id.ends_with("-gf"),
                "{}",
                p.id
            );
        }
        // the families by heat: PLA is the coolest, PA the hottest
        let t = |m| c.of(m)[0].nozzle_c.best;
        assert!(t(Material::Pla) < t(Material::Petg) && t(Material::Abs) < t(Material::Pa));
        assert!(
            c.of(Material::Abs)[0].enclosure_advised && c.of(Material::Tpu)[0].direct_drive_advised
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

    #[test]
    fn a_bad_table_is_refused_with_its_line() {
        let row =
            "x-pla|X PLA|pla|210|190|230|60|0|70|100|80|100|1.24|1.75|0.2|false|false|false|false";
        assert_eq!(FilamentCatalog::parse(row).unwrap().len(), 1);
        for (bad, want) in [
            ("x|X|pla|210", "columns"),
            (
                "x|X|wood|210|190|230|60|0|70|100|80|100|1.24|1.75|0.2|false|false|false|false",
                "material",
            ),
            (
                "x|X|pla|180|190|230|60|0|70|100|80|100|1.24|1.75|0.2|false|false|false|false",
                "range",
            ),
            (
                "x|X|pla|210|190|230|60|0|70|100|80|100|1.24|1.5|0.2|false|false|false|false",
                "diameter",
            ),
            (
                "x|X|pla|210|190|230|60|0|70|100|80|100|1.24|1.75|0.2|maybe|false|false|false",
                "abrasive",
            ),
            (
                "X Y|X|pla|210|190|230|60|0|70|100|80|100|1.24|1.75|0.2|false|false|false|false",
                "id",
            ),
            (
                "x|X|pla|210|190|230|60|0|70|100|80|100|1.24|1.75|9|false|false|false|false",
                "smallest nozzle",
            ),
        ] {
            let e = FilamentCatalog::parse(bad).unwrap_err().to_string();
            assert!(e.contains(want), "{bad} -> {e}");
        }
        assert!(FilamentCatalog::parse(&format!("{row}\n{row}"))
            .unwrap_err()
            .to_string()
            .contains("twice"));
        assert!(FilamentCatalog::parse("# nothing\n").is_err());
    }
}
