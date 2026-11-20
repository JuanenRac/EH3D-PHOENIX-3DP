//! The resins: generic profiles of the kinds of photopolymer resin that monochrome-screen printers are fed, each with the layer height, the exposures and the lift of its kind as a
//! starting point, and the resin a model needs. The figures are the middle of the road of each kind, written from what makers print for generic resins and from public data sheets: the
//! bottle in hand and an exposure test win, because the right exposure depends on the power of the light of the printer. Nothing here has been tried on a machine, so every row says
//! `verified = false`, and a profile file can replace any row.
//!
//! A resin profile is data like the others: it reads and writes a text file of `key = value` lines where a mistake names the key, and a row of the table that makes no sense is an error that
//! names the line.

use crate::filament::Range;
use crate::profile::{PrinterProfile, ProfileError, Technology};
use eh3d_printmaterials::model::{Resin as Record, ResinKind as DbKind, Solvent};
use eh3d_printmaterials::query::Database;
use p3dp_core::kv;
use std::sync::OnceLock;

/// The printer of the database whose exposures are the starting points of the generic resins: a monochrome screen printer in general.
pub const GENERIC_PRINTER: &str = "generic-msla";

/// The kind of a resin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResinKind {
    Standard,
    Tough,
    Flexible,
    Castable,
    Clear,
    HighDetail,
}

impl ResinKind {
    pub const ALL: [ResinKind; 6] = [
        ResinKind::Standard,
        ResinKind::Tough,
        ResinKind::Flexible,
        ResinKind::Castable,
        ResinKind::Clear,
        ResinKind::HighDetail,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ResinKind::Standard => "standard",
            ResinKind::Tough => "tough",
            ResinKind::Flexible => "flexible",
            ResinKind::Castable => "castable",
            ResinKind::Clear => "clear",
            ResinKind::HighDetail => "high-detail",
        }
    }

    pub fn from_name(name: &str) -> Option<ResinKind> {
        let want = name.trim().to_ascii_lowercase();
        ResinKind::ALL.into_iter().find(|k| k.name() == want)
    }
}

/// What the printed part is washed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wash {
    Alcohol,
    Water,
}

impl Wash {
    pub fn name(self) -> &'static str {
        match self {
            Wash::Alcohol => "alcohol",
            Wash::Water => "water",
        }
    }

    pub fn from_name(name: &str) -> Option<Wash> {
        match name.trim().to_ascii_lowercase().as_str() {
            "alcohol" => Some(Wash::Alcohol),
            "water" => Some(Wash::Water),
            _ => None,
        }
    }
}

/// A resin.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinProfile {
    pub id: String,
    pub name: String,
    pub kind: ResinKind,
    /// The height of a layer, in micrometres.
    pub layer_um: Range,
    /// The exposure of a normal layer, in seconds.
    pub normal_exposure_s: Range,
    /// The exposure of a bottom layer, in seconds.
    pub bottom_exposure_s: Range,
    pub bottom_layers: u32,
    /// How far the plate lifts between layers, in millimetres.
    pub lift_mm: f64,
    /// How fast it lifts, in millimetres a minute.
    pub lift_speed_mm_min: f64,
    /// The wavelength the resin cures at, in nanometres.
    pub wavelength_nm: f64,
    pub density_g_ml: f64,
    pub wash: Wash,
    pub verified: bool,
}

/// What a print takes from the bottle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResinUse {
    /// The resin in the part, in millilitres.
    pub part_ml: f64,
    /// The resin counting the waste (what the supports, the raft and the drips take), in millilitres.
    pub total_ml: f64,
    pub grams: f64,
    /// The cost of `total_ml` at the price given (0 when no price was given).
    pub cost: f64,
}

fn bad(id: &str, reason: impl AsRef<str>) -> ProfileError {
    ProfileError(format!("resin {id}: {}", reason.as_ref()))
}

fn check_range(id: &str, what: &str, r: &Range, max: f64) -> Result<(), ProfileError> {
    let ok = [r.best, r.lowest, r.highest]
        .iter()
        .all(|v| v.is_finite() && *v > 0.0 && *v <= max)
        && r.lowest <= r.best
        && r.best <= r.highest;
    if ok {
        Ok(())
    } else {
        Err(bad(
            id,
            format!(
                "the range of {what} ({} / {} / {}) is not above 0 and up to {max}, with the lowest first and the highest last",
                r.lowest, r.best, r.highest
            ),
        ))
    }
}

impl ResinProfile {
    /// Checks that a row makes sense.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let id = &self.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-/._+".contains(&b))
        {
            return Err(bad(
                id,
                "the id must be lower case letters, digits, hyphens, dots, plus signs, underscores and slashes",
            ));
        }
        if self.name.trim().is_empty() {
            return Err(bad(id, "the name is needed"));
        }
        check_range(id, "the layer height (um)", &self.layer_um, 200.0)?;
        check_range(
            id,
            "the exposure of a layer (s)",
            &self.normal_exposure_s,
            60.0,
        )?;
        check_range(
            id,
            "the exposure of a bottom layer (s)",
            &self.bottom_exposure_s,
            300.0,
        )?;
        if self.bottom_exposure_s.best < self.normal_exposure_s.best {
            return Err(bad(
                id,
                "the bottom layers must be exposed at least as long as the others, so the part sticks to the plate",
            ));
        }
        if !(1..=20).contains(&self.bottom_layers) {
            return Err(bad(id, "the bottom layers are 1 to 20"));
        }
        if !(self.lift_mm.is_finite() && (1.0..=20.0).contains(&self.lift_mm)) {
            return Err(bad(id, "the lift is 1 to 20 mm"));
        }
        if !(self.lift_speed_mm_min.is_finite() && (10.0..=600.0).contains(&self.lift_speed_mm_min))
        {
            return Err(bad(id, "the lift speed is 10 to 600 mm a minute"));
        }
        if !(self.wavelength_nm.is_finite() && (350.0..=500.0).contains(&self.wavelength_nm)) {
            return Err(bad(id, "the wavelength is 350 to 500 nm"));
        }
        if !(self.density_g_ml.is_finite() && (0.9..=1.6).contains(&self.density_g_ml)) {
            return Err(bad(id, "the density is 0.9 to 1.6 g/ml"));
        }
        Ok(())
    }

    /// Whether the resin can go in a printer: only a resin printer takes resin.
    pub fn usable_on(&self, printer: &PrinterProfile) -> Result<(), ProfileError> {
        if printer.technology == Technology::Msla {
            Ok(())
        } else {
            Err(bad(
                &self.id,
                format!(
                    "{} is a filament printer and takes no resin",
                    printer.title()
                ),
            ))
        }
    }

    /// The resin a part of this volume takes, with `waste_percent` more for supports, raft and drips, and its cost at a price per litre (0 for no price).
    pub fn used(
        &self,
        volume_mm3: f64,
        waste_percent: f64,
        price_per_litre: f64,
    ) -> Result<ResinUse, ProfileError> {
        if !(volume_mm3.is_finite() && volume_mm3 >= 0.0) {
            return Err(bad(
                &self.id,
                "the volume of the part is not a number of zero or more",
            ));
        }
        if !(waste_percent.is_finite() && (0.0..=500.0).contains(&waste_percent)) {
            return Err(bad(&self.id, "the waste is 0 to 500 percent"));
        }
        if !(price_per_litre.is_finite() && price_per_litre >= 0.0) {
            return Err(bad(&self.id, "the price is not a number of zero or more"));
        }
        let part_ml = volume_mm3 / 1000.0;
        let total_ml = part_ml * (1.0 + waste_percent / 100.0);
        Ok(ResinUse {
            part_ml,
            total_ml,
            grams: total_ml * self.density_g_ml,
            cost: total_ml / 1000.0 * price_per_litre,
        })
    }

    /// The profile as the text of a profile file (`from_text` reads it back).
    pub fn to_text(&self) -> String {
        let r = |a: &Range| format!("{}, {}, {}", a.best, a.lowest, a.highest);
        format!(
            "id = {}\nname = {}\nkind = {}\nlayer_um = {}\nnormal_exposure_s = {}\nbottom_exposure_s = {}\nbottom_layers = {}\nlift_mm = {}\nlift_speed_mm_min = {}\nwavelength_nm = {}\ndensity_g_ml = {}\nwash = {}\nverified = {}\n",
            self.id,
            self.name,
            self.kind.name(),
            r(&self.layer_um),
            r(&self.normal_exposure_s),
            r(&self.bottom_exposure_s),
            self.bottom_layers,
            self.lift_mm,
            self.lift_speed_mm_min,
            self.wavelength_nm,
            self.density_g_ml,
            self.wash.name(),
            if self.verified { "on" } else { "off" }
        )
    }

    /// Reads a profile file. Every key of `to_text` is understood; an unknown key, a missing one or a value out of range is an error that names the key.
    pub fn from_text(text: &str) -> Result<ResinProfile, ProfileError> {
        const KNOWN: [&str; 13] = [
            "id",
            "name",
            "kind",
            "layer_um",
            "normal_exposure_s",
            "bottom_exposure_s",
            "bottom_layers",
            "lift_mm",
            "lift_speed_mm_min",
            "wavelength_nm",
            "density_g_ml",
            "wash",
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
        let id = need("id")?.to_string();
        let p = ResinProfile {
            name: need("name")?.to_string(),
            kind: ResinKind::from_name(need("kind")?).ok_or_else(|| {
                bad(
                    &id,
                    "kind: not one of standard, tough, flexible, castable, clear, high-detail",
                )
            })?,
            layer_um: range("layer_um", 200.0)?,
            normal_exposure_s: range("normal_exposure_s", 60.0)?,
            bottom_exposure_s: range("bottom_exposure_s", 300.0)?,
            bottom_layers: kv::int_in("bottom_layers", need("bottom_layers")?, 1, 20)
                .map_err(|e| ProfileError(e.to_string()))? as u32,
            lift_mm: num("lift_mm", 1.0, 20.0)?,
            lift_speed_mm_min: num("lift_speed_mm_min", 10.0, 600.0)?,
            wavelength_nm: num("wavelength_nm", 350.0, 500.0)?,
            density_g_ml: num("density_g_ml", 0.9, 1.6)?,
            wash: Wash::from_name(need("wash")?)
                .ok_or_else(|| bad(&id, "wash: alcohol or water"))?,
            verified: match get("verified") {
                Some(v) => kv::on_off("verified", v).map_err(|e| ProfileError(e.to_string()))?,
                None => false,
            },
            id,
        };
        p.validate()?;
        Ok(p)
    }
}

/// A set of resins.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinCatalog {
    pub profiles: Vec<ResinProfile>,
    /// The identifiers this program used before it read the database of the suite, each with the identifier that stands for it now.
    pub aliases: Vec<(String, String)>,
}

impl ResinKind {
    /// The kind of this program that stands for a kind of the database: the six of the generic profiles, and the nearest one for the others (a hard resin and a high-temperature one are tough, a
    /// water-washable one is standard, and the rest that has no place here is standard too, with its own name in the database).
    pub fn of_database(kind: DbKind) -> ResinKind {
        match kind {
            DbKind::Hard | DbKind::AbsLike | DbKind::HighTemperature | DbKind::Ceramic => {
                ResinKind::Tough
            }
            DbKind::Flexible => ResinKind::Flexible,
            DbKind::Castable => ResinKind::Castable,
            DbKind::Clear => ResinKind::Clear,
            DbKind::HighDetail => ResinKind::HighDetail,
            DbKind::Standard | DbKind::WaterWashable | DbKind::Dental | DbKind::Other => {
                ResinKind::Standard
            }
        }
    }
}

/// A range of the database, in the unit of this program (`scale` turns millimetres into micrometres), from the lowest, the best and the highest; the best is the value itself when the record
/// gives no bounds.
fn range_of(best: f64, lowest: Option<f64>, highest: Option<f64>, scale: f64) -> Range {
    // rounded to a millionth, so 0.05 mm is 50 um and not 50.00000000000001
    let to_unit = |v: f64| (v * scale * 1e6).round() / 1e6;
    Range {
        best: to_unit(best),
        lowest: to_unit(lowest.unwrap_or(best).min(best)),
        highest: to_unit(highest.unwrap_or(best).max(best)),
    }
}

/// The profile of a record of the database, from its exposure for the generic printer at the layer height it recommends. A resin with no such point, or a value out of range, gives nothing.
fn profile_of(r: &Record) -> Option<ResinProfile> {
    let layer = r.layer_mm?;
    let rec = layer.recommended?;
    let point = r
        .exposures
        .iter()
        .find(|e| e.printer == GENERIC_PRINTER && (e.layer_mm - rec).abs() < 1e-9)
        .or_else(|| r.exposures.iter().find(|e| e.printer == GENERIC_PRINTER))?;
    let wash = match r.wash.map(|w| w.solvent) {
        Some(Solvent::Water) => Wash::Water,
        _ => Wash::Alcohol,
    };
    let p = ResinProfile {
        id: r.id.clone(),
        name: r.name.clone(),
        kind: ResinKind::of_database(r.kind),
        layer_um: range_of(rec, layer.min, layer.max, 1000.0),
        normal_exposure_s: range_of(point.normal_s, point.normal_s_min, point.normal_s_max, 1.0),
        bottom_exposure_s: range_of(point.base_s?, point.base_s_min, point.base_s_max, 1.0),
        bottom_layers: point.base_layers?,
        lift_mm: point.lift_mm?,
        lift_speed_mm_min: point.lift_speed_mm_min?,
        wavelength_nm: r.wavelength_nm?,
        density_g_ml: r.density_g_cm3?,
        wash,
        verified: r.verified && point.verified,
    };
    p.validate().ok()?;
    Some(p)
}

static BUILTIN: OnceLock<Result<ResinCatalog, String>> = OnceLock::new();

impl ResinCatalog {
    /// The resins of the database of the suite (`eh3d-printmaterials`), the only source of this program. The catalog is made once and copied after that.
    pub fn builtin() -> Result<ResinCatalog, ProfileError> {
        BUILTIN
            .get_or_init(|| {
                let db = Database::embedded().map_err(|e| {
                    e.first()
                        .map_or("the database of materials is not valid".to_string(), |x| {
                            x.to_string()
                        })
                })?;
                Ok(ResinCatalog::from_database(&db))
            })
            .clone()
            .map_err(ProfileError)
    }

    /// The catalog of a database: the resins that have settings for the generic printer, in the order of the database, and the old identifiers (the `source_ref` of the generic profiles) as aliases.
    pub fn from_database(db: &Database) -> ResinCatalog {
        let mut profiles = Vec::new();
        let mut aliases = Vec::new();
        for r in db.resins() {
            let Some(p) = profile_of(r) else {
                continue;
            };
            if r.manufacturer.eq_ignore_ascii_case("generic") {
                if let Some(old) = r
                    .source_ref
                    .as_deref()
                    .filter(|s| s.starts_with("generic-"))
                {
                    aliases.push((old.to_string(), p.id.clone()));
                }
            }
            profiles.push(p);
        }
        ResinCatalog { profiles, aliases }
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// The resin with an id (any case), or with one of the identifiers the program used before the database.
    pub fn find(&self, id: &str) -> Option<&ResinProfile> {
        let want = id.trim().to_ascii_lowercase();
        let want = self
            .aliases
            .iter()
            .find(|(old, _)| *old == want)
            .map_or(want, |(_, new)| new.clone());
        self.profiles.iter().find(|p| p.id == want)
    }

    /// The resins of a kind.
    pub fn of(&self, kind: ResinKind) -> Vec<&ResinProfile> {
        self.profiles.iter().filter(|p| p.kind == kind).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn resin(id: &str) -> ResinProfile {
        ResinCatalog::builtin().unwrap().find(id).unwrap().clone()
    }

    #[test]
    fn the_built_in_resins_read_check_and_say_they_were_never_tried() {
        let c = ResinCatalog::builtin().unwrap();
        assert_eq!(c.len(), 7);
        for p in &c.profiles {
            p.validate().unwrap();
            assert!(!p.verified, "{}", p.id);
            assert!(p.layer_um.lowest <= p.layer_um.best && p.layer_um.best <= p.layer_um.highest);
        }
        for k in ResinKind::ALL {
            assert!(!c.of(k).is_empty(), "{}", k.name());
            assert_eq!(ResinKind::from_name(&k.name().to_uppercase()), Some(k));
        }
        assert_eq!(c.find("GENERIC-TOUGH").unwrap().kind, ResinKind::Tough);
        assert_eq!(c.find("generic-water-washable").unwrap().wash, Wash::Water);
        assert!(c.find("unobtainium").is_none());
        assert!(!c.is_empty());
        assert_eq!(Wash::from_name("Alcohol"), Some(Wash::Alcohol));
        assert_eq!(Wash::from_name("soap"), None);
    }

    #[test]
    fn a_resin_goes_only_in_a_resin_printer() {
        let cat = Catalog::builtin().unwrap();
        let r = resin("generic-standard");
        let fdm = cat.find("bambu-lab-a1-mini").unwrap();
        let e = r.usable_on(fdm).unwrap_err().to_string();
        assert!(
            e.contains("takes no resin") && e.contains("generic-standard"),
            "{e}"
        );
        let msla = cat.technology(Technology::Msla);
        assert!(!msla.is_empty());
        for p in msla {
            r.usable_on(p).unwrap();
        }
    }

    #[test]
    fn the_resin_a_part_takes_counts_the_waste_the_weight_and_the_cost() {
        let r = resin("generic-standard");
        // 10 ml of part, 20 % more for supports and drips, a litre at 40
        let u = r.used(10_000.0, 20.0, 40.0).unwrap();
        assert!((u.part_ml - 10.0).abs() < 1e-9);
        assert!((u.total_ml - 12.0).abs() < 1e-9);
        assert!((u.grams - 12.0 * 1.10).abs() < 1e-9);
        assert!((u.cost - 0.48).abs() < 1e-9);
        // no price, no cost
        assert_eq!(r.used(10_000.0, 0.0, 0.0).unwrap().cost, 0.0);
        assert_eq!(r.used(0.0, 0.0, 10.0).unwrap().total_ml, 0.0);
        for (v, w, p) in [
            (-1.0, 0.0, 0.0),
            (f64::NAN, 0.0, 0.0),
            (1.0, -1.0, 0.0),
            (1.0, 501.0, 0.0),
            (1.0, 0.0, -2.0),
            (1.0, 0.0, f64::INFINITY),
        ] {
            assert!(r.used(v, w, p).is_err(), "{v} {w} {p}");
        }
    }

    #[test]
    fn a_profile_file_reads_back_what_was_written_and_a_mistake_names_the_key() {
        for p in ResinCatalog::builtin().unwrap().profiles {
            assert_eq!(ResinProfile::from_text(&p.to_text()).unwrap(), p);
        }
        let tough = resin("generic-tough");
        let good = tough.to_text();
        for (from, to, want) in [
            ("kind = tough", "kind = jelly", "kind"),
            ("lift_mm = 6", "lift_mm = 99", "lift_mm"),
            ("layer_um = 50, 25, 100", "layer_um = 50, 25", "layer_um"),
            ("wash = alcohol", "wash = soap", "wash"),
            ("density_g_ml = 1.12", "density_g_ml = abc", "density_g_ml"),
            ("bottom_layers = 5", "bottom_layers = 0", "bottom_layers"),
            ("verified = off", "verified = maybe", "verified"),
            ("bottom_layers = 5\n", "", "bottom_layers"),
            (&format!("id = {}", tough.id), "id = Bad Id", "id"),
        ] {
            let text = good.replacen(from, to, 1);
            assert_ne!(text, good, "{from}");
            let e = ResinProfile::from_text(&text).unwrap_err().to_string();
            assert!(e.contains(want), "{from} -> {e}");
        }
        let e = ResinProfile::from_text(&format!("{good}colour = red\n"))
            .unwrap_err()
            .to_string();
        assert!(e.contains("colour") && e.contains("line 14"), "{e}");
        // the last line of a key wins, as in the other profile files
        let twice = format!("{good}lift_mm = 8\n");
        assert_eq!(ResinProfile::from_text(&twice).unwrap().lift_mm, 8.0);
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn the_old_identifiers_still_find_their_resin_and_the_values_are_the_ones_of_the_old_table() {
        let c = ResinCatalog::builtin().unwrap();
        assert_eq!(c.aliases.len(), 7);
        // (old id, kind, layer um best/lowest/highest, exposure, bottom exposure, bottom layers, lift, speed, wash)
        let want: [(
            &str,
            ResinKind,
            [f64; 3],
            [f64; 3],
            [f64; 3],
            u32,
            f64,
            f64,
            Wash,
        ); 3] = [
            (
                "generic-standard",
                ResinKind::Standard,
                [50.0, 25.0, 100.0],
                [2.5, 1.5, 4.0],
                [25.0, 15.0, 40.0],
                5,
                6.0,
                150.0,
                Wash::Alcohol,
            ),
            (
                "generic-flexible",
                ResinKind::Flexible,
                [50.0, 50.0, 100.0],
                [4.0, 3.0, 6.0],
                [30.0, 20.0, 45.0],
                6,
                5.0,
                90.0,
                Wash::Alcohol,
            ),
            (
                "generic-water-washable",
                ResinKind::Standard,
                [50.0, 25.0, 100.0],
                [2.8, 2.0, 4.5],
                [25.0, 15.0, 40.0],
                5,
                6.0,
                150.0,
                Wash::Water,
            ),
        ];
        for (id, kind, layer, normal, bottom, layers, lift, speed, wash) in want {
            let r = c.find(id).unwrap_or_else(|| panic!("{id}"));
            let t = |x: Range| [x.best, x.lowest, x.highest];
            assert_eq!(r.kind, kind, "{id}");
            assert_eq!(t(r.layer_um), layer, "{id}");
            assert_eq!(t(r.normal_exposure_s), normal, "{id}");
            assert_eq!(t(r.bottom_exposure_s), bottom, "{id}");
            assert_eq!(
                (r.bottom_layers, r.lift_mm, r.lift_speed_mm_min),
                (layers, lift, speed)
            );
            assert_eq!((r.wash, r.wavelength_nm), (wash, 405.0), "{id}");
            assert!(r.id.starts_with("generic/"), "{}", r.id);
            // the new identifier finds it as well
            assert_eq!(c.find(&r.id).map(|x| x.id.as_str()), Some(r.id.as_str()));
        }
    }

    #[test]
    fn a_record_of_the_database_that_makes_no_sense_gives_no_profile() {
        let db = Database::embedded().unwrap();
        let good = db.resins()[0].clone();
        assert!(profile_of(&good).is_some());
        let none = |f: &dyn Fn(&mut Record)| {
            let mut r = good.clone();
            f(&mut r);
            profile_of(&r).is_none()
        };
        assert!(none(&|r| r.wavelength_nm = Some(9000.0)));
        assert!(none(&|r| r.wavelength_nm = None));
        assert!(none(&|r| r.density_g_cm3 = Some(9.0)));
        assert!(none(&|r| r.layer_mm = None));
        assert!(none(&|r| r.exposures.clear()));
        assert!(none(
            &|r| r.exposures[0].printer = "another-printer".to_string()
        ));
        assert!(none(&|r| r.exposures[0].base_s = None));
        assert!(none(&|r| r.exposures[0].base_s = Some(0.1)));
        assert!(none(&|r| r.exposures[0].lift_mm = Some(99.0)));
        // the kinds of the database that have no place here go to the nearest one
        assert_eq!(ResinKind::of_database(DbKind::Dental), ResinKind::Standard);
        assert_eq!(ResinKind::of_database(DbKind::AbsLike), ResinKind::Tough);
        assert_eq!(ResinKind::of_database(DbKind::Ceramic), ResinKind::Tough);
    }
}
