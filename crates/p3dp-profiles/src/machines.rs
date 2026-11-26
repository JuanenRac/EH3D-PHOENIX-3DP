//! The catalog of printers of the suite (`eh3d-printers` of EH3D-COMMON) laid over the table of the program: for the models it covers, the figures that name a page of the maker
//! win, the models the table lacks are added, and every figure that differs between the two is kept as a [`Difference`] so nothing changes in silence.

use crate::catalog::Catalog;
use crate::profile::{Connectivity, LimitSource, PrinterProfile, ProfileError, Technology};
use eh3d_printers::{Adapter, Firmware, Printer};
use p3dp_core::{Dialect, TempLimits};

/// A figure of the table of the program that the catalog of the suite replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub id: String,
    pub field: &'static str,
    pub table: String,
    pub suite: String,
}

/// The table of the program with the catalog of the suite over it.
#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    pub catalog: Catalog,
    /// The models that only the catalog of the suite has.
    pub added: Vec<String>,
    pub differences: Vec<Difference>,
}

/// The catalog of printers of the suite.
pub fn suite_machines() -> Result<eh3d_printers::Catalog, ProfileError> {
    eh3d_printers::Catalog::embedded().map_err(|errors| {
        ProfileError(
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })
}

fn dialect_of(f: Firmware) -> Option<Dialect> {
    match f {
        Firmware::Bambu => Some(Dialect::Bambu),
        Firmware::Klipper => Some(Dialect::Klipper),
        Firmware::Marlin => Some(Dialect::Marlin),
        Firmware::Unknown => None,
    }
}

fn connectivity_of(a: Adapter) -> Option<Connectivity> {
    match a {
        Adapter::BambuLan => Some(Connectivity::BambuLan),
        Adapter::KlipperMoonraker => Some(Connectivity::Moonraker),
        Adapter::MarlinSerial => Some(Connectivity::Serial),
        Adapter::Octoprint => Some(Connectivity::OctoPrint),
        Adapter::Prusalink => Some(Connectivity::PrusaLink),
        Adapter::VendorProtocol | Adapter::Unknown => None,
    }
}

fn limits_of(p: &Printer) -> TempLimits {
    TempLimits {
        nozzle_c: p.nozzle_max_c,
        bed_c: p.bed_max_c,
        chamber_c: p.chamber_max_c,
    }
}

fn limits_text(l: Option<TempLimits>) -> String {
    l.map_or_else(
        || "-".to_string(),
        |l| {
            format!(
                "{}/{}/{}",
                l.nozzle_c,
                l.bed_c,
                l.chamber_c
                    .map_or_else(|| "-".to_string(), |c| c.to_string())
            )
        },
    )
}

/// A profile for a model that the table does not have; `None` when the record does not say enough to make one.
fn new_profile(p: &Printer) -> Option<PrinterProfile> {
    if p.nozzles_mm.is_empty() {
        return None;
    }
    Some(PrinterProfile {
        id: p.flat_id(),
        vendor: p.manufacturer.clone(),
        model: p.model.clone(),
        technology: Technology::Fdm,
        volume_mm: p.volume_mm,
        nozzles_mm: p.nozzles_mm.clone(),
        dialect: dialect_of(p.firmware)?,
        connectivity: vec![connectivity_of(p.adapter)?],
        multimaterial: p.multicolor.system.clone(),
        limits: Some(limits_of(p)),
        limit_source: LimitSource::Maker,
        verified: false,
    })
}

/// Lays the machines over a table.
pub fn overlay(mut catalog: Catalog, machines: &[Printer]) -> Result<Overlay, ProfileError> {
    let mut added = Vec::new();
    let mut differences = Vec::new();
    for m in machines {
        let id = m.flat_id();
        let Some(slot) = catalog.profiles.iter_mut().find(|p| p.id == id) else {
            if let Some(profile) = new_profile(m) {
                profile.validate()?;
                added.push(id);
                catalog.profiles.push(profile);
            }
            continue;
        };
        if slot.technology != Technology::Fdm {
            return Err(ProfileError(format!(
                "profile {id}: the catalog of the suite has a filament printer where the table has not"
            )));
        }
        let mut differ = |field: &'static str, table: String, suite: String| {
            if table != suite {
                differences.push(Difference {
                    id: id.clone(),
                    field,
                    table,
                    suite,
                });
            }
        };
        differ(
            "volume_mm",
            format!("{:?}", slot.volume_mm),
            format!("{:?}", m.volume_mm),
        );
        differ(
            "limits",
            limits_text(slot.limits),
            limits_text(Some(limits_of(m))),
        );
        slot.volume_mm = m.volume_mm;
        slot.limits = Some(limits_of(m));
        slot.limit_source = LimitSource::Maker;
        // a nozzle the table knows and the page of the maker did not list is kept: nothing is taken away
        for n in &m.nozzles_mm {
            if !slot.nozzles_mm.iter().any(|x| (x - n).abs() < 1e-9) {
                slot.nozzles_mm.push(*n);
            }
        }
        slot.nozzles_mm.sort_by(f64::total_cmp);
        if let Some(c) = connectivity_of(m.adapter) {
            if !slot.has(c) {
                slot.connectivity.push(c);
            }
        }
        slot.validate()?;
    }
    Ok(Overlay {
        catalog,
        added,
        differences,
    })
}

/// The table of the program with the catalog of the suite over it.
pub fn suite_overlay() -> Result<Overlay, ProfileError> {
    overlay(Catalog::builtin()?, suite_machines()?.printers())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_suite_adds_its_models_and_keeps_the_table() {
        let table = Catalog::builtin().unwrap();
        let over = suite_overlay().unwrap();
        assert_eq!(over.catalog.len(), table.len() + over.added.len());
        assert_eq!(
            over.added,
            [
                "artillery-sidewinder-x4-plus-s1",
                "artillery-sidewinder-x4-pro-s1"
            ]
        );
        for m in suite_machines().unwrap().printers() {
            let p = over.catalog.find(&m.id).unwrap();
            assert_eq!(p.volume_mm, m.volume_mm, "{}", p.id);
            assert_eq!(p.limit_source, LimitSource::Maker);
            assert!(!p.verified);
            p.validate().unwrap();
        }
        // a model the suite does not cover is as the table has it
        assert_eq!(over.catalog.find("prusa-mk4s"), table.find("prusa-mk4s"));
    }

    #[test]
    fn a_figure_that_differs_is_told_and_no_nozzle_is_lost() {
        let mut m = suite_machines()
            .unwrap()
            .find("creality-k1c")
            .unwrap()
            .clone();
        m.volume_mm = [219.0, 220.0, 250.0];
        m.nozzle_max_c = 290.0;
        let over = overlay(Catalog::builtin().unwrap(), std::slice::from_ref(&m)).unwrap();
        let fields: Vec<&str> = over.differences.iter().map(|d| d.field).collect();
        assert_eq!(fields, ["volume_mm", "limits"]);
        let p = over.catalog.find("creality-k1c").unwrap();
        assert!(p.nozzles_mm.contains(&0.4) && p.nozzles_mm.contains(&0.6));
        assert_eq!(p.limits.unwrap().nozzle_c, 290.0);
    }

    #[test]
    fn a_record_that_says_too_little_makes_no_profile() {
        let mut m = suite_machines()
            .unwrap()
            .find("artillery-sidewinder-x4-pro-s1")
            .unwrap()
            .clone();
        m.firmware = Firmware::Unknown;
        let over = overlay(Catalog::builtin().unwrap(), std::slice::from_ref(&m)).unwrap();
        assert!(over.added.is_empty());
        m.firmware = Firmware::Klipper;
        m.nozzles_mm.clear();
        let over = overlay(Catalog::builtin().unwrap(), std::slice::from_ref(&m)).unwrap();
        assert!(over.added.is_empty());
    }
}
