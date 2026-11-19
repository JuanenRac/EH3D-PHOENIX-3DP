//! The catalog: the printers that come with the program, read from a table of facts that is part of the source.

use crate::profile::{Connectivity, PrinterProfile, ProfileError, Technology};
use p3dp_core::{Dialect, TempLimits};

/// The table, one printer per line (see the comments at its top for the columns).
pub const DATA: &str = include_str!("../data/printers.tsv");

/// A set of profiles.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalog {
    pub profiles: Vec<PrinterProfile>,
}

fn field_num(line: usize, name: &str, v: &str) -> Result<f64, ProfileError> {
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| ProfileError(format!("line {line}: {name} {v:?} is not a number")))
}

impl Catalog {
    /// The catalog of the program.
    pub fn builtin() -> Result<Catalog, ProfileError> {
        Catalog::parse(DATA)
    }

    /// Reads a table of printers. A row that makes no sense, an id that is used twice or a row with the wrong number of columns is an error that names the line.
    pub fn parse(text: &str) -> Result<Catalog, ProfileError> {
        let mut profiles: Vec<PrinterProfile> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let ln = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let c: Vec<&str> = line.split('|').collect();
            if c.len() != 15 {
                return Err(ProfileError(format!(
                    "line {ln}: {} columns, expected 15",
                    c.len()
                )));
            }
            let technology = Technology::from_name(c[3]).ok_or_else(|| {
                ProfileError(format!(
                    "line {ln}: technology {:?} is not fdm or msla",
                    c[3]
                ))
            })?;
            let dialect = Dialect::from_name(c[8]).ok_or_else(|| {
                ProfileError(format!("line {ln}: dialect {:?} is not known", c[8]))
            })?;
            let mut connectivity = Vec::new();
            for part in c[9].split(',').filter(|p| !p.trim().is_empty()) {
                connectivity.push(Connectivity::from_name(part).ok_or_else(|| {
                    ProfileError(format!("line {ln}: connectivity {part:?} is not known"))
                })?);
            }
            let mut nozzles_mm = Vec::new();
            for part in c[7].split(',').filter(|p| !p.trim().is_empty()) {
                nozzles_mm.push(field_num(ln, "nozzle", part)?);
            }
            let limits = if technology == Technology::Fdm {
                Some(TempLimits {
                    nozzle_c: field_num(ln, "nozzle limit", c[11])?,
                    bed_c: field_num(ln, "bed limit", c[12])?,
                    chamber_c: if c[13].trim() == "-" {
                        None
                    } else {
                        Some(field_num(ln, "chamber limit", c[13])?)
                    },
                })
            } else {
                None
            };
            let verified = match c[14].trim() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(ProfileError(format!(
                        "line {ln}: verified {other:?} is not true or false"
                    )))
                }
            };
            let p = PrinterProfile {
                id: c[0].trim().to_string(),
                vendor: c[1].trim().to_string(),
                model: c[2].trim().to_string(),
                technology,
                volume_mm: [
                    field_num(ln, "x", c[4])?,
                    field_num(ln, "y", c[5])?,
                    field_num(ln, "z", c[6])?,
                ],
                nozzles_mm,
                dialect,
                connectivity,
                multimaterial: Some(c[10].trim().to_string()).filter(|m| !m.is_empty()),
                limits,
                verified,
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
            return Err(ProfileError("the catalog has no printers".to_string()));
        }
        Ok(Catalog { profiles })
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// The profile with an id (any case).
    pub fn find(&self, id: &str) -> Option<&PrinterProfile> {
        let want = id.trim().to_ascii_lowercase();
        self.profiles.iter().find(|p| p.id == want)
    }

    /// The profiles whose id, vendor or model contain a text (any case), in catalog order.
    pub fn search(&self, query: &str) -> Vec<&PrinterProfile> {
        let q = query.trim().to_ascii_lowercase();
        self.profiles
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.id.contains(&q)
                    || p.vendor.to_ascii_lowercase().contains(&q)
                    || p.model.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    /// The vendors, each once, in the order they first appear.
    pub fn vendors(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        for p in &self.profiles {
            if !v.contains(&p.vendor.as_str()) {
                v.push(&p.vendor);
            }
        }
        v
    }

    /// The profiles of one technology.
    pub fn technology(&self, t: Technology) -> Vec<&PrinterProfile> {
        self.profiles.iter().filter(|p| p.technology == t).collect()
    }

    /// The profiles that can be reached in a way.
    pub fn reachable_by(&self, c: Connectivity) -> Vec<&PrinterProfile> {
        self.profiles.iter().filter(|p| p.has(c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> Catalog {
        Catalog::builtin().unwrap()
    }

    #[test]
    fn the_catalog_reads_and_has_the_machines_of_the_owner() {
        let c = cat();
        assert!(c.len() >= 30 && c.len() <= 60, "{}", c.len());
        let a1 = c.find("bambu-lab-a1").unwrap();
        assert_eq!(a1.dialect, Dialect::Bambu);
        assert!(a1.has(Connectivity::BambuLan));
        assert_eq!(a1.volume_mm, [256.0, 256.0, 256.0]);
        let p1s = c.find("BAMBU-LAB-P1S").unwrap();
        assert_eq!(p1s.technology, Technology::Fdm);
        for id in [
            "prusa-mk4s",
            "prusa-core-one",
            "prusa-mini-plus",
            "creality-k1",
            "creality-ender-3-v3-ke",
            "voron-2-4-300",
            "klipper-generic",
            "anycubic-kobra-3",
            "elegoo-neptune-4-pro",
            "qidi-plus4",
        ] {
            assert!(c.find(id).is_some(), "{id}");
        }
    }

    #[test]
    fn nothing_is_verified_yet() {
        assert!(cat().profiles.iter().all(|p| !p.verified));
    }

    #[test]
    fn every_profile_is_coherent() {
        for p in &cat().profiles {
            p.validate().unwrap();
            // a printer that talks to Moonraker runs Klipper; Prusa's link is on Marlin; Bambu printers use the Bambu dialect
            if p.has(Connectivity::Moonraker) {
                assert_eq!(p.dialect, Dialect::Klipper, "{}", p.id);
            }
            if p.has(Connectivity::BambuLan) {
                assert_eq!(p.dialect, Dialect::Bambu, "{}", p.id);
            }
            if p.has(Connectivity::Duet) {
                assert_eq!(p.dialect, Dialect::Rrf, "{}", p.id);
            }
            if let Some(l) = p.limits {
                assert!(l.nozzle_c >= 200.0 && l.nozzle_c <= 400.0, "{}", p.id);
                assert!(l.bed_c >= 60.0 && l.bed_c <= 140.0, "{}", p.id);
            }
        }
    }

    #[test]
    fn both_technologies_and_all_dialects_are_there() {
        let c = cat();
        assert!(c.technology(Technology::Msla).len() >= 3);
        assert!(c.technology(Technology::Fdm).len() >= 30);
        for d in Dialect::ALL {
            assert!(c.profiles.iter().any(|p| p.dialect == d), "{d}");
        }
        assert!(c.reachable_by(Connectivity::Moonraker).len() >= 10);
        assert!(c.reachable_by(Connectivity::OctoPrint).len() >= 10);
    }

    #[test]
    fn search_and_vendors() {
        let c = cat();
        assert!(c.search("bambu").len() >= 4);
        assert!(c.search("K1").iter().any(|p| p.id == "creality-k1"));
        assert_eq!(c.search("").len(), c.len());
        assert!(c.search("nothing like this").is_empty());
        let v = c.vendors();
        assert!(v.contains(&"Bambu Lab") && v.contains(&"Prusa Research"));
        let mut sorted = v.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), v.len());
    }

    #[test]
    fn a_bad_table_is_refused_with_its_line() {
        let row = "a-1|V|M|fdm|10|10|10|0.4|marlin|serial||250|100|-|false";
        assert!(Catalog::parse(row).is_ok());
        for (bad, want) in [
            ("a-1|V|M|fdm|10|10|10|0.4|marlin|serial||250|100|-", "15"),
            (
                "a-1|V|M|sls|10|10|10|0.4|marlin|serial||250|100|-|false",
                "technology",
            ),
            (
                "a-1|V|M|fdm|10|10|10|0.4|grbl|serial||250|100|-|false",
                "dialect",
            ),
            (
                "a-1|V|M|fdm|10|10|10|0.4|marlin|pigeon||250|100|-|false",
                "pigeon",
            ),
            (
                "a-1|V|M|fdm|10|10|ten|0.4|marlin|serial||250|100|-|false",
                "number",
            ),
            (
                "a-1|V|M|fdm|10|10|10|0.4|marlin|serial||250|100|-|maybe",
                "verified",
            ),
            (
                "a-1|V|M|fdm|0|10|10|0.4|marlin|serial||250|100|-|false",
                "volume",
            ),
        ] {
            let e = Catalog::parse(bad).unwrap_err().to_string();
            assert!(e.contains(want), "{bad} -> {e}");
        }
        let twice = format!("{row}\n{row}");
        assert!(Catalog::parse(&twice)
            .unwrap_err()
            .to_string()
            .contains("twice"));
        assert!(Catalog::parse("# only comments\n").is_err());
    }
}
