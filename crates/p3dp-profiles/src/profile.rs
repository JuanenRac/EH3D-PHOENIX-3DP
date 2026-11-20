//! One printer profile and the file in which an owner writes or edits one.

use p3dp_core::kv;
use p3dp_core::{Dialect, Mesh, TempLimits};
use std::fmt;

/// How a printer can be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Connectivity {
    Moonraker,
    OctoPrint,
    PrusaLink,
    Duet,
    BambuLan,
    BambuCloud,
    ElegooLink,
    Serial,
    SdCard,
}

impl Connectivity {
    pub const ALL: [Connectivity; 9] = [
        Connectivity::Moonraker,
        Connectivity::OctoPrint,
        Connectivity::PrusaLink,
        Connectivity::Duet,
        Connectivity::BambuLan,
        Connectivity::BambuCloud,
        Connectivity::ElegooLink,
        Connectivity::Serial,
        Connectivity::SdCard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Connectivity::Moonraker => "moonraker",
            Connectivity::OctoPrint => "octoprint",
            Connectivity::PrusaLink => "prusalink",
            Connectivity::Duet => "duet",
            Connectivity::BambuLan => "bambu-lan",
            Connectivity::BambuCloud => "bambu-cloud",
            Connectivity::ElegooLink => "elegoolink",
            Connectivity::Serial => "serial",
            Connectivity::SdCard => "sd",
        }
    }

    pub fn from_name(name: &str) -> Option<Connectivity> {
        let n = name.trim().to_ascii_lowercase();
        Connectivity::ALL.into_iter().find(|c| c.name() == n)
    }
}

/// The technology of the printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Technology {
    /// Melted filament.
    Fdm,
    /// Resin cured by a masked screen.
    Msla,
}

impl Technology {
    pub fn name(self) -> &'static str {
        match self {
            Technology::Fdm => "fdm",
            Technology::Msla => "msla",
        }
    }

    pub fn from_name(name: &str) -> Option<Technology> {
        match name.trim().to_ascii_lowercase().as_str() {
            "fdm" => Some(Technology::Fdm),
            "msla" => Some(Technology::Msla),
            _ => None,
        }
    }
}

/// Why a profile or a catalog was refused: it names the line or the profile and the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileError(pub String);

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProfileError {}

/// Where the temperature limits of a profile come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitSource {
    /// The figure that the maker publishes for the model.
    Maker,
    /// The conservative limits of the program, used where the figure of the maker was not taken: they refuse more than the printer could take, and a profile file can raise them.
    Conservative,
    /// The owner wrote them in a profile file.
    Owner,
}

impl LimitSource {
    pub fn name(self) -> &'static str {
        match self {
            LimitSource::Maker => "maker",
            LimitSource::Conservative => "default",
            LimitSource::Owner => "owner",
        }
    }

    pub fn from_name(name: &str) -> Option<LimitSource> {
        match name.trim().to_ascii_lowercase().as_str() {
            "maker" => Some(LimitSource::Maker),
            "default" | "conservative" => Some(LimitSource::Conservative),
            "owner" => Some(LimitSource::Owner),
            _ => None,
        }
    }
}

/// A printer model.
#[derive(Debug, Clone, PartialEq)]
pub struct PrinterProfile {
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub technology: Technology,
    /// The build volume in millimetres (x, y, z).
    pub volume_mm: [f64; 3],
    /// The nozzles the maker offers, in millimetres (empty for resin).
    pub nozzles_mm: Vec<f64>,
    pub dialect: Dialect,
    pub connectivity: Vec<Connectivity>,
    /// What the printer has for several materials or colours, if anything.
    pub multimaterial: Option<String>,
    /// The temperature limits of the model (`None` for resin printers).
    pub limits: Option<TempLimits>,
    /// Where those limits come from.
    pub limit_source: LimitSource,
    /// Whether the profile has been tried on a machine. Every profile of the catalog starts as false.
    pub verified: bool,
}

fn bad(id: &str, reason: &str) -> ProfileError {
    ProfileError(format!("profile {id}: {reason}"))
}

impl PrinterProfile {
    /// Checks that the profile makes sense: a positive volume, the right fields for its technology, limits that are finite and not absurd.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let id = &self.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(bad(
                id,
                "the id must be lower case letters, digits and hyphens",
            ));
        }
        if self.vendor.is_empty() || self.model.is_empty() {
            return Err(bad(id, "the vendor and the model are needed"));
        }
        if self
            .volume_mm
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 5000.0)
        {
            return Err(bad(
                id,
                "the volume must be between 0 and 5000 mm on every axis",
            ));
        }
        match self.technology {
            Technology::Fdm => {
                if self.dialect == Dialect::Msla {
                    return Err(bad(id, "a filament printer cannot have the msla dialect"));
                }
                if self.nozzles_mm.is_empty()
                    || self
                        .nozzles_mm
                        .iter()
                        .any(|n| !n.is_finite() || *n < 0.1 || *n > 2.0)
                {
                    return Err(bad(id, "the nozzles must be between 0.1 and 2 mm"));
                }
                let Some(l) = self.limits else {
                    return Err(bad(id, "a filament printer needs temperature limits"));
                };
                if !(l.nozzle_c.is_finite() && l.nozzle_c > 0.0 && l.nozzle_c <= 600.0) {
                    return Err(bad(id, "the nozzle limit must be between 0 and 600 C"));
                }
                if !(l.bed_c.is_finite() && l.bed_c > 0.0 && l.bed_c <= 200.0) {
                    return Err(bad(id, "the bed limit must be between 0 and 200 C"));
                }
                if let Some(c) = l.chamber_c {
                    if !(c.is_finite() && c > 0.0 && c <= 200.0) {
                        return Err(bad(id, "the chamber limit must be between 0 and 200 C"));
                    }
                }
            }
            Technology::Msla => {
                if self.dialect != Dialect::Msla {
                    return Err(bad(id, "a resin printer must have the msla dialect"));
                }
                if self.limits.is_some() || !self.nozzles_mm.is_empty() {
                    return Err(bad(
                        id,
                        "a resin printer has no nozzle and no heater limits",
                    ));
                }
            }
        }
        if self.connectivity.is_empty() {
            return Err(bad(id, "at least one way of connecting is needed"));
        }
        Ok(())
    }

    pub fn has(&self, c: Connectivity) -> bool {
        self.connectivity.contains(&c)
    }

    /// The nozzle the profile starts with: 0.4 mm if the maker offers it, else the first.
    pub fn default_nozzle_mm(&self) -> Option<f64> {
        if self.nozzles_mm.contains(&0.4) {
            Some(0.4)
        } else {
            self.nozzles_mm.first().copied()
        }
    }

    /// Whether a mesh fits in the build volume.
    pub fn fits(&self, mesh: &Mesh) -> bool {
        mesh.fits_in(self.volume_mm)
    }

    /// The name that a person sees.
    pub fn title(&self) -> String {
        if self.model.starts_with(&self.vendor) {
            self.model.clone()
        } else {
            format!("{} {}", self.vendor, self.model)
        }
    }

    /// The profile as the text of a profile file (`from_text` reads it back).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "id = {}\nvendor = {}\nmodel = {}\ntechnology = {}\n",
            self.id,
            self.vendor,
            self.model,
            self.technology.name()
        ));
        s.push_str(&format!(
            "volume_x_mm = {}\nvolume_y_mm = {}\nvolume_z_mm = {}\n",
            self.volume_mm[0], self.volume_mm[1], self.volume_mm[2]
        ));
        let nz: Vec<String> = self.nozzles_mm.iter().map(|n| n.to_string()).collect();
        s.push_str(&format!(
            "nozzles_mm = {}\ndialect = {}\n",
            nz.join(","),
            self.dialect.name()
        ));
        let cn: Vec<&str> = self.connectivity.iter().map(|c| c.name()).collect();
        s.push_str(&format!("connectivity = {}\n", cn.join(",")));
        s.push_str(&format!(
            "multimaterial = {}\n",
            self.multimaterial.as_deref().unwrap_or("")
        ));
        if let Some(l) = self.limits {
            s.push_str(&format!(
                "nozzle_max_c = {}\nbed_max_c = {}\n",
                l.nozzle_c, l.bed_c
            ));
            if let Some(c) = l.chamber_c {
                s.push_str(&format!("chamber_max_c = {c}\n"));
            }
        }
        s.push_str(&format!("limit_source = {}\n", self.limit_source.name()));
        s.push_str(&format!(
            "verified = {}\n",
            if self.verified { "on" } else { "off" }
        ));
        s
    }

    /// Reads a profile file: lines of `key = value`. Every key of `to_text` is understood; an unknown key, a missing one that the technology needs or a value out of range
    /// is an error that names it.
    pub fn from_text(text: &str) -> Result<PrinterProfile, ProfileError> {
        let entries = kv::parse(text).map_err(|e| ProfileError(e.to_string()))?;
        let get = |k: &str| entries.iter().find(|e| e.key == k).map(|e| e.value.clone());
        const KNOWN: [&str; 15] = [
            "id",
            "vendor",
            "model",
            "technology",
            "volume_x_mm",
            "volume_y_mm",
            "volume_z_mm",
            "nozzles_mm",
            "dialect",
            "connectivity",
            "multimaterial",
            "nozzle_max_c",
            "bed_max_c",
            "chamber_max_c",
            "limit_source",
        ];
        for e in &entries {
            if e.key != "verified" && !KNOWN.contains(&e.key.as_str()) {
                return Err(ProfileError(format!(
                    "line {}: the key {} is not known",
                    e.line, e.key
                )));
            }
        }
        let need = |k: &str| get(k).ok_or_else(|| ProfileError(format!("the key {k} is missing")));
        let id = need("id")?;
        let num = |k: &str, lo: f64, hi: f64| -> Result<f64, ProfileError> {
            kv::float_in(k, &need(k)?, lo, hi).map_err(|e| ProfileError(e.to_string()))
        };
        let technology = Technology::from_name(&need("technology")?)
            .ok_or_else(|| bad(&id, "technology must be fdm or msla"))?;
        let dialect = Dialect::from_name(&need("dialect")?)
            .ok_or_else(|| bad(&id, "the dialect is not known"))?;
        let mut connectivity = Vec::new();
        for part in need("connectivity")?
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            connectivity.push(Connectivity::from_name(part).ok_or_else(|| {
                bad(&id, &format!("the way of connecting {part:?} is not known"))
            })?);
        }
        let mut nozzles_mm = Vec::new();
        for part in get("nozzles_mm")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            nozzles_mm.push(
                kv::float_in("nozzles_mm", part, 0.1, 2.0)
                    .map_err(|e| ProfileError(e.to_string()))?,
            );
        }
        let limits = match technology {
            Technology::Fdm => Some(TempLimits {
                nozzle_c: num("nozzle_max_c", 1.0, 600.0)?,
                bed_c: num("bed_max_c", 1.0, 200.0)?,
                chamber_c: match get("chamber_max_c") {
                    Some(v) if !v.is_empty() => Some(
                        kv::float_in("chamber_max_c", &v, 1.0, 200.0)
                            .map_err(|e| ProfileError(e.to_string()))?,
                    ),
                    _ => None,
                },
            }),
            Technology::Msla => None,
        };
        let verified = match get("verified") {
            Some(v) => kv::on_off("verified", &v).map_err(|e| ProfileError(e.to_string()))?,
            None => false,
        };
        let limit_source = match get("limit_source") {
            Some(v) => LimitSource::from_name(&v)
                .ok_or_else(|| bad(&id, "limit_source must be maker, default or owner"))?,
            None => LimitSource::Owner,
        };
        let p = PrinterProfile {
            id: id.clone(),
            vendor: need("vendor")?,
            model: need("model")?,
            technology,
            volume_mm: [
                num("volume_x_mm", 1.0, 5000.0)?,
                num("volume_y_mm", 1.0, 5000.0)?,
                num("volume_z_mm", 1.0, 5000.0)?,
            ],
            nozzles_mm,
            dialect,
            connectivity,
            multimaterial: get("multimaterial").filter(|m| !m.is_empty()),
            limits,
            limit_source,
            verified,
        };
        p.validate()?;
        Ok(p)
    }
}

/// One line of a profile that differs between two profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub key: String,
    pub from: String,
    pub to: String,
}

/// Something a person should know about an edit that was accepted.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A temperature limit was raised. `above_maker` says that the old figure was the one the maker states for the model, so the new one is higher than the maker allows.
    LimitRaised {
        key: &'static str,
        from: f64,
        to: f64,
        above_maker: bool,
    },
    /// The profile said it had been tried on a machine; an edited profile has not been, so it says `verified = off` now.
    VerificationCleared,
}

/// A profile after an edit, with what the person should be told.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    pub profile: PrinterProfile,
    pub notices: Vec<Notice>,
}

/// The keys of a profile an edit may not touch: they say which model it is.
const LOCKED: [&str; 4] = ["id", "vendor", "model", "technology"];
/// The keys that mean nothing to a resin printer.
const FDM_ONLY: [&str; 4] = ["nozzles_mm", "nozzle_max_c", "bed_max_c", "chamber_max_c"];

impl PrinterProfile {
    /// The lines in which two profiles differ, as `(key, from, to)` in the order of a profile file (a key that only one has shows an empty text on the other side).
    pub fn diff(&self, other: &PrinterProfile) -> Vec<Change> {
        let a = self.lines();
        let b = other.lines();
        let mut out = Vec::new();
        for (k, v) in &a {
            let to = b
                .iter()
                .find(|(k2, _)| k2 == k)
                .map_or("", |(_, v2)| v2.as_str());
            if v != to {
                out.push(Change {
                    key: k.clone(),
                    from: v.clone(),
                    to: to.to_string(),
                });
            }
        }
        for (k, v) in &b {
            if !a.iter().any(|(k2, _)| k2 == k) && !v.is_empty() {
                out.push(Change {
                    key: k.clone(),
                    from: String::new(),
                    to: v.clone(),
                });
            }
        }
        out
    }

    fn lines(&self) -> Vec<(String, String)> {
        self.to_text()
            .lines()
            .filter_map(|l| l.split_once(" = "))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Applies edits (`key`, `value`) to a copy of the profile and checks the result as a whole. The id, the vendor, the model and the technology cannot be edited; a key that does not
    /// exist for this kind of printer is an error that names it; a value out of range is an error that names the key. A limit that is changed makes the limits the owner's
    /// (`limit_source = owner`), a raised limit comes back as a notice (and says if it is past the figure of the maker), and an edit of anything clears `verified`: an edited
    /// profile has not been tried.
    pub fn edited(&self, edits: &[(&str, &str)]) -> Result<Edited, ProfileError> {
        let mut lines = self.lines();
        let mut explicit_source = false;
        for (key, value) in edits {
            if LOCKED.contains(key) {
                return Err(ProfileError(format!(
                    "{key}: it says which model the profile is and cannot be edited (copy the profile under another id instead)"
                )));
            }
            if self.technology == Technology::Msla && FDM_ONLY.contains(key) {
                return Err(ProfileError(format!(
                    "{key}: a resin printer has no nozzle and no heater limits"
                )));
            }
            if *key == "limit_source" {
                explicit_source = true;
            }
            match lines.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value.trim().to_string(),
                None if matches!(*key, "chamber_max_c" | "multimaterial") => {
                    lines.push((key.to_string(), value.trim().to_string()));
                }
                None => return Err(ProfileError(format!("{key}: not a key of this profile"))),
            }
        }
        let text_of = |lines: &[(String, String)]| {
            lines
                .iter()
                .map(|(k, v)| format!("{k} = {v}\n"))
                .collect::<String>()
        };
        let mut p = PrinterProfile::from_text(&text_of(&lines))?;
        let mut notices = Vec::new();
        if let (Some(old), Some(new)) = (self.limits, p.limits) {
            let pairs = [
                ("nozzle_max_c", old.nozzle_c, new.nozzle_c),
                ("bed_max_c", old.bed_c, new.bed_c),
                (
                    "chamber_max_c",
                    old.chamber_c.unwrap_or(0.0),
                    new.chamber_c.unwrap_or(0.0),
                ),
            ];
            let mut changed = false;
            for (key, from, to) in pairs {
                if (from - to).abs() > f64::EPSILON {
                    changed = true;
                }
                if to > from {
                    notices.push(Notice::LimitRaised {
                        key,
                        from,
                        to,
                        above_maker: self.limit_source == LimitSource::Maker,
                    });
                }
            }
            if changed && !explicit_source {
                p.limit_source = LimitSource::Owner;
            }
        }
        if p != *self && self.verified {
            p.verified = false;
            notices.push(Notice::VerificationCleared);
        }
        // `verified` can only come off by an edit: a profile is never made verified by editing it
        if p.verified && !self.verified {
            return Err(ProfileError(
                "verified: a profile becomes verified by a report from a machine, not by an edit"
                    .to_string(),
            ));
        }
        Ok(Edited {
            profile: p,
            notices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    fn catalog_profile(id: &str) -> PrinterProfile {
        crate::catalog::Catalog::builtin()
            .unwrap()
            .find(id)
            .unwrap()
            .clone()
    }

    #[test]
    fn raising_a_limit_past_the_maker_is_allowed_and_told_and_makes_the_limits_the_owners() {
        // the A1 mini states a bed of 80 C; its owner may raise it by hand to 100
        let a1 = catalog_profile("bambu-lab-a1-mini");
        assert_eq!(a1.limits.unwrap().bed_c, 80.0);
        assert_eq!(a1.limit_source, LimitSource::Maker);
        let e = a1.edited(&[("bed_max_c", "100")]).unwrap();
        assert_eq!(e.profile.limits.unwrap().bed_c, 100.0);
        assert_eq!(e.profile.limit_source, LimitSource::Owner);
        assert!(!e.profile.verified);
        assert_eq!(
            e.notices,
            vec![Notice::LimitRaised {
                key: "bed_max_c",
                from: 80.0,
                to: 100.0,
                above_maker: true
            }]
        );
        // the nozzle limit did not change, so there is no notice about it
        assert_eq!(
            e.profile.limits.unwrap().nozzle_c,
            a1.limits.unwrap().nozzle_c
        );
        // lowering a limit is quiet
        let low = a1.edited(&[("bed_max_c", "60")]).unwrap();
        assert!(low.notices.is_empty());
        assert_eq!(low.profile.limit_source, LimitSource::Owner);
        // an edit that changes no limit leaves the source as it was
        let same = a1.edited(&[("bed_max_c", "80")]).unwrap();
        assert_eq!(same.profile.limit_source, LimitSource::Maker);
        assert!(same.notices.is_empty());
    }

    #[test]
    fn an_edit_that_cannot_be_applied_names_the_key_and_changes_nothing() {
        let a1 = catalog_profile("bambu-lab-a1-mini");
        for (key, value, want) in [
            ("id", "other", "id"),
            ("model", "X", "model"),
            ("technology", "msla", "technology"),
            ("colour", "red", "colour"),
            ("bed_max_c", "900", "bed_max_c"),
            ("bed_max_c", "hot", "bed_max_c"),
            ("nozzles_mm", "9", "nozzles_mm"),
            ("connectivity", "carrier-pigeon", "carrier-pigeon"),
            ("verified", "on", "verified"),
        ] {
            let e = a1.edited(&[(key, value)]).unwrap_err().to_string();
            assert!(e.contains(want), "{key}={value} -> {e}");
        }
        let resin = crate::catalog::Catalog::builtin()
            .unwrap()
            .technology(Technology::Msla)[0]
            .clone();
        let e = resin
            .edited(&[("nozzle_max_c", "250")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("nozzle_max_c") && e.contains("resin"), "{e}");
        assert!(resin.edited(&[("multimaterial", "none")]).is_ok());
    }

    #[test]
    fn an_edit_clears_verified_and_the_chamber_limit_can_be_added() {
        let mut p = catalog_profile("bambu-lab-a1");
        p.verified = true;
        let e = p.edited(&[("chamber_max_c", "45")]).unwrap();
        assert!(!e.profile.verified);
        assert!(e.notices.contains(&Notice::VerificationCleared));
        assert_eq!(e.profile.limits.unwrap().chamber_c, Some(45.0));
        assert!(e.notices.iter().any(|n| matches!(
            n,
            Notice::LimitRaised {
                key: "chamber_max_c",
                ..
            }
        )));
        // no edits at all: the same profile, still verified
        let none = p.edited(&[]).unwrap();
        assert_eq!(none.profile, p);
        assert!(none.notices.is_empty());
    }

    #[test]
    fn the_difference_of_two_profiles_lists_the_lines_that_changed() {
        let a1 = catalog_profile("bambu-lab-a1-mini");
        assert!(a1.diff(&a1).is_empty());
        let b = a1
            .edited(&[("bed_max_c", "100"), ("nozzles_mm", "0.4")])
            .unwrap()
            .profile;
        let d = a1.diff(&b);
        let keys: Vec<&str> = d.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, vec!["nozzles_mm", "bed_max_c", "limit_source"]);
        assert_eq!(
            d[1],
            Change {
                key: "bed_max_c".into(),
                from: "80".into(),
                to: "100".into()
            }
        );
        assert_eq!(d[2].from, "maker");
        // a line only one of them has
        let mut c = a1.clone();
        c.limits.as_mut().unwrap().chamber_c = Some(40.0);
        let d = a1.diff(&c);
        assert_eq!(
            d,
            vec![Change {
                key: "chamber_max_c".into(),
                from: String::new(),
                to: "40".into()
            }]
        );
        let back = c.diff(&a1);
        assert_eq!(back[0].from, "40");
        assert_eq!(back[0].to, "");
    }

    fn sample() -> PrinterProfile {
        PrinterProfile {
            id: "test-printer".into(),
            vendor: "Test".into(),
            model: "Printer".into(),
            technology: Technology::Fdm,
            volume_mm: [200.0, 200.0, 200.0],
            nozzles_mm: vec![0.4, 0.6],
            dialect: Dialect::Marlin,
            connectivity: vec![Connectivity::OctoPrint, Connectivity::Serial],
            multimaterial: None,
            limits: Some(TempLimits {
                nozzle_c: 260.0,
                bed_c: 100.0,
                chamber_c: Some(50.0),
            }),
            limit_source: LimitSource::Owner,
            verified: false,
        }
    }

    #[test]
    fn the_source_of_the_limits_is_named_and_a_file_without_it_is_the_owners() {
        for s in [
            LimitSource::Maker,
            LimitSource::Conservative,
            LimitSource::Owner,
        ] {
            assert_eq!(LimitSource::from_name(s.name()), Some(s));
        }
        assert_eq!(
            LimitSource::from_name(" CONSERVATIVE "),
            Some(LimitSource::Conservative)
        );
        assert_eq!(LimitSource::from_name("guess"), None);
        let text = sample().to_text().replace("limit_source = owner\n", "");
        assert_eq!(
            PrinterProfile::from_text(&text).unwrap().limit_source,
            LimitSource::Owner
        );
        let bad = sample()
            .to_text()
            .replace("limit_source = owner", "limit_source = guess");
        assert!(PrinterProfile::from_text(&bad).is_err());
    }

    #[test]
    fn a_profile_round_trips_through_its_file() {
        let p = sample();
        assert_eq!(PrinterProfile::from_text(&p.to_text()).unwrap(), p);
        let mut q = sample();
        q.limits = Some(TempLimits {
            chamber_c: None,
            ..q.limits.unwrap()
        });
        q.multimaterial = Some("AMS".into());
        q.verified = true;
        q.limit_source = LimitSource::Maker;
        assert_eq!(PrinterProfile::from_text(&q.to_text()).unwrap(), q);
    }

    #[test]
    fn a_profile_that_makes_no_sense_is_refused() {
        let mut p = sample();
        p.volume_mm[1] = 0.0;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.limits = None;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.dialect = Dialect::Msla;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.id = "Bad Id".into();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.nozzles_mm = vec![];
        assert!(p.validate().is_err());
        let mut p = sample();
        p.limits = Some(TempLimits {
            nozzle_c: f64::NAN,
            ..p.limits.unwrap()
        });
        assert!(p.validate().is_err());
        let mut p = sample();
        p.connectivity.clear();
        assert!(p.validate().is_err());
    }

    #[test]
    fn a_profile_file_names_what_is_wrong() {
        let good = sample().to_text();
        for (from, to, want) in [
            ("dialect = marlin", "dialect = grbl", "dialect"),
            ("nozzle_max_c = 260", "nozzle_max_c = 900", "nozzle_max_c"),
            (
                "connectivity = octoprint,serial",
                "connectivity = octoprint,pigeon",
                "pigeon",
            ),
            ("volume_x_mm = 200", "volume_x_mm = nan", "volume_x_mm"),
            (
                "id = test-printer",
                "id = test-printer\nsurprise = 1",
                "surprise",
            ),
        ] {
            let e = PrinterProfile::from_text(&good.replace(from, to))
                .unwrap_err()
                .to_string();
            assert!(e.contains(want), "{e}");
        }
        assert!(PrinterProfile::from_text("id = a\n").is_err());
    }

    #[test]
    fn fits_and_default_nozzle() {
        let p = sample();
        assert!(p.fits(&cube(100.0)));
        assert!(!p.fits(&cube(250.0)));
        assert_eq!(p.default_nozzle_mm(), Some(0.4));
        let mut q = sample();
        q.nozzles_mm = vec![0.6, 0.8];
        assert_eq!(q.default_nozzle_mm(), Some(0.6));
        assert_eq!(p.title(), "Test Printer");
        assert!(p.has(Connectivity::Serial) && !p.has(Connectivity::Moonraker));
    }

    #[test]
    fn names_round_trip() {
        for c in Connectivity::ALL {
            assert_eq!(Connectivity::from_name(c.name()), Some(c));
        }
        assert_eq!(Technology::from_name("MSLA"), Some(Technology::Msla));
        assert_eq!(Technology::from_name("sls"), None);
    }
}
