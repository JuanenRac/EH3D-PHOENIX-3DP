//! From a sliced plan to the `.gcode.3mf` package of a printer of the Bambu family: the facts of the print, a picture of the plate drawn from the plan, and the G-code exactly as
//! it was sliced. The container is `p3dp_formats::gcode3mf`; no printer has been given one of these packages.

use crate::i18n::{Lang, Msg};
use p3dp_core::Dialect;
use p3dp_formats::gcode3mf::{self, Plate, SUFFIX};
use p3dp_formats::png_write::encode_rgba;
use p3dp_gcode::requires_package;
use p3dp_slicer::Plan;

/// The side of the picture of the plate and of the small one, in pixels.
pub const PICTURE_SIDE: u32 = 256;
pub const SMALL_SIDE: u32 = 128;

/// What the name of a file to write asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A loose G-code file.
    Loose,
    /// A `.gcode.3mf` package.
    Package,
    /// A `.3mf` name that is not a package of G-code: writing text into it would make a file nothing opens.
    Other3mf,
}

/// What the name of a file to write asks for.
pub fn target_of(path: &str) -> Target {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(SUFFIX) {
        Target::Package
    } else if lower.ends_with(".3mf") {
        Target::Other3mf
    } else {
        Target::Loose
    }
}

/// Whether a printer of this dialect takes the package.
pub fn takes_package(dialect: Dialect) -> bool {
    requires_package(dialect)
}

/// What to write for a file of this name: a loose G-code file, or the package for a name ending in `.gcode.3mf`. The second value is what the person is told about it. A package
/// for a printer that does not take one, and a `.3mf` name that is not a package, are refused.
pub fn file_for(
    path: &str,
    plan: &Plan,
    gcode: &str,
    facts: &Facts,
    lang: Lang,
) -> Result<(Vec<u8>, Option<Msg>), String> {
    let dialect = plan.machine.dialect;
    match target_of(path) {
        Target::Other3mf => Err(Msg::OutOther3mf.tr(lang).to_string()),
        Target::Package if !takes_package(dialect) => Err(Msg::OutNotBambu.tr(lang).to_string()),
        Target::Package => Ok((build(plan, gcode, facts)?, Some(Msg::PackageNotTried))),
        Target::Loose => Ok((
            gcode.as_bytes().to_vec(),
            takes_package(dialect).then_some(Msg::PackageLoose),
        )),
    }
}

/// Writes a file so that it is whole or not there: a temporary file beside it, then its name.
pub fn write_whole(path: &str, bytes: &[u8]) -> Result<(), String> {
    let tmp = format!("{path}.tmp-{}", std::process::id());
    std::fs::write(&tmp, bytes).map_err(|e| format!("{tmp}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{path}: {e}")
    })
}

/// A picture of the plate seen from above: the lines of every layer, brighter the higher they are, on the bed of the printer. RGBA, `side` by `side`.
pub fn draw_plate(plan: &Plan, side: u32) -> Vec<u8> {
    let side_px = side.max(1) as usize;
    let mut out = vec![0u8; side_px * side_px * 4];
    for px in out.chunks_mut(4) {
        px.copy_from_slice(&[24, 24, 28, 255]);
    }
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in plan
        .layers
        .iter()
        .flat_map(|l| &l.paths)
        .flat_map(|p| &p.points)
    {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if !lo.iter().chain(hi.iter()).all(|v| v.is_finite()) {
        return out;
    }
    // the print fills the picture with a small border, keeping its proportions
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-3);
    let scale = (side_px as f64 * 0.9) / span;
    let middle = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let half = side_px as f64 / 2.0;
    let top = plan.height_mm().max(1e-3);
    let last = side_px as f64 - 1.0;
    for layer in &plan.layers {
        let light = 0.35 + 0.65 * (layer.z_mm / top).clamp(0.0, 1.0);
        let colour = [
            (255.0 * light) as u8,
            (190.0 * light) as u8,
            (40.0 * light) as u8,
        ];
        for path in &layer.paths {
            let closing = path.closed.then(|| path.points.first()).flatten();
            let mut points = path.points.iter().chain(closing);
            let Some(mut from) = points.next() else {
                continue;
            };
            for to in points {
                // y grows upwards on the bed and downwards in a picture
                let at = |p: &[f64; 2]| {
                    [
                        half + (p[0] - middle[0]) * scale,
                        half - (p[1] - middle[1]) * scale,
                    ]
                };
                let (a, b) = (at(from), at(to));
                let steps = (b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil().max(1.0) as usize;
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let x = (a[0] + (b[0] - a[0]) * t).clamp(0.0, last) as usize;
                    let y = (a[1] + (b[1] - a[1]) * t).clamp(0.0, last) as usize;
                    let i = (y * side_px + x) * 4;
                    out[i..i + 3].copy_from_slice(&colour);
                }
                from = to;
            }
        }
    }
    out
}

/// What the package says of the print besides its G-code.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts<'a> {
    pub title: &'a str,
    /// The family of the plastic (`PLA`), empty when no filament was chosen.
    pub material: &'a str,
    pub density_g_cm3: f64,
}

/// The bytes of the package of a plan and the G-code made from it.
pub fn build(plan: &Plan, gcode: &str, facts: &Facts) -> Result<Vec<u8>, String> {
    let picture =
        |side: u32| encode_rgba(side, side, &draw_plate(plan, side)).map_err(|e| e.to_string());
    let (large, small) = (picture(PICTURE_SIDE)?, picture(SMALL_SIDE)?);
    let metres = plan.filament_mm() / 1000.0;
    // cubic millimetres times grams per cubic centimetre, over a thousand
    let weight_g = plan.extrusion_mm3() * facts.density_g_cm3 / 1000.0;
    gcode3mf::write(&Plate {
        gcode,
        title: facts.title,
        printer_id: &plan.machine.id,
        nozzle_mm: plan.config.nozzle_diameter_mm,
        predicted_seconds: plan.estimated_seconds().max(0.0).round() as u64,
        weight_g,
        filament_m: metres,
        filament_type: facts.material,
        picture: &large,
        picture_small: &small,
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::Limits;
    use p3dp_formats::project3mf::read_project;
    use p3dp_profiles::Catalog;
    use p3dp_slicer::{Machine, SliceConfig, VERSION};

    #[test]
    fn the_name_says_what_is_written() {
        assert_eq!(target_of("a.gcode"), Target::Loose);
        assert_eq!(target_of("a.GCODE.3MF"), Target::Package);
        assert_eq!(target_of("a.3mf"), Target::Other3mf);
        assert_eq!(target_of("a.gcode.3mf.gcode"), Target::Loose);
        assert!(takes_package(Dialect::Bambu));
        assert!(!takes_package(Dialect::Klipper) && !takes_package(Dialect::Marlin));
    }

    #[test]
    fn a_sliced_cube_goes_into_a_package_with_its_picture_and_its_figures() {
        let printer = Catalog::builtin()
            .unwrap()
            .find("bambu-lab-a1")
            .unwrap()
            .clone();
        let machine = Machine::from_profile(&printer).unwrap();
        let (plan, gcode) =
            p3dp_slicer::slice(&cube(20.0), &machine, &SliceConfig::default()).unwrap();
        assert!(gcode.contains(VERSION));
        let facts = Facts {
            title: "cube",
            material: "PLA",
            density_g_cm3: 1.24,
        };
        let bytes = build(&plan, &gcode, &facts).unwrap();
        let limits = Limits::default();
        let got = gcode3mf::read_gcode(&bytes, &limits).unwrap().unwrap();
        assert_eq!(got.text, gcode);
        assert_eq!(got.checksum, gcode3mf::Checksum::Agrees);
        let info = read_project(&bytes, &limits).unwrap();
        let seconds = info.plates[0].predicted_seconds.unwrap();
        assert!(seconds.abs_diff(plan.estimated_seconds() as u64) <= 1);
        let weight = info.plates[0].weight_g.unwrap();
        assert!(weight > 1.0 && weight < 12.0, "{weight}");

        let picture = draw_plate(&plan, 64);
        assert_eq!(picture.len(), 64 * 64 * 4);
        let drawn = picture.chunks(4).filter(|p| p[0] > 60).count();
        assert!(drawn > 500, "{drawn}");
        // the corner of the picture is outside the print
        assert_eq!(&picture[..4], &[24, 24, 28, 255]);
    }

    #[test]
    fn each_name_writes_its_kind_and_the_wrong_ones_are_refused() {
        let catalog = Catalog::builtin().unwrap();
        let sliced = |id: &str| {
            let machine = Machine::from_profile(catalog.find(id).unwrap()).unwrap();
            p3dp_slicer::slice(&cube(10.0), &machine, &SliceConfig::default()).unwrap()
        };
        let facts = Facts {
            title: "cube",
            material: "",
            density_g_cm3: 1.24,
        };
        let (bambu, g) = sliced("bambu-lab-p1s");
        let (bytes, told) = file_for("a.gcode.3mf", &bambu, &g, &facts, Lang::En).unwrap();
        assert_eq!(told, Some(Msg::PackageNotTried));
        assert!(bytes.starts_with(b"PK"));
        let (bytes, told) = file_for("a.gcode", &bambu, &g, &facts, Lang::En).unwrap();
        assert_eq!(
            (bytes, told),
            (g.clone().into_bytes(), Some(Msg::PackageLoose))
        );
        assert!(file_for("a.3mf", &bambu, &g, &facts, Lang::En).is_err());
        let (other, g) = sliced("creality-k1c");
        assert!(file_for("a.gcode.3mf", &other, &g, &facts, Lang::En).is_err());
        let (_, told) = file_for("a.gcode", &other, &g, &facts, Lang::En).unwrap();
        assert_eq!(told, None);
    }

    #[test]
    fn a_file_is_written_whole_and_a_folder_that_is_not_there_is_told() {
        let dir = std::env::temp_dir().join(format!("p3dp-whole-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.bin").to_string_lossy().into_owned();
        write_whole(&file, b"one").unwrap();
        write_whole(&file, b"two").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let lost = dir.join("no/a.bin").to_string_lossy().into_owned();
        assert!(write_whole(&lost, b"x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
