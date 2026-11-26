//! The package a printer of the Bambu family is given: a 3MF package (a ZIP laid out by the Open Packaging Conventions, with the model part that the 3MF core specification asks
//! for) that carries the G-code of one plate beside it, in `Metadata/plate_1.gcode`, with its MD5 checksum, a picture of the plate and a few facts about the print. Such a file
//! is named `.gcode.3mf`.
//!
//! The maker does not publish the whole layout of these files. What is written here is the container of the standards and the few names that are needed to find the G-code and its
//! facts; nothing of another program was copied. No printer has been given one of these packages.

use crate::error::FormatError;
use crate::md5::md5_hex;
use crate::xml;
use crate::zip::{Archive, Writer};
use p3dp_core::Limits;

/// The G-code of the one plate.
pub const GCODE: &str = "Metadata/plate_1.gcode";
/// The checksum of the G-code, as hexadecimal digits.
pub const CHECKSUM: &str = "Metadata/plate_1.gcode.md5";
/// The picture of the plate and the small one.
pub const PICTURE: &str = "Metadata/plate_1.png";
pub const PICTURE_SMALL: &str = "Metadata/plate_1_small.png";
const SLICE_INFO: &str = "Metadata/slice_info.config";
const PLATE_SETTINGS: &str = "Metadata/model_settings.config";
const MODEL: &str = "3D/3dmodel.model";

/// The end of the name of such a file.
pub const SUFFIX: &str = ".gcode.3mf";

/// What goes into a package.
#[derive(Debug, Clone, PartialEq)]
pub struct Plate<'a> {
    pub gcode: &'a str,
    pub title: &'a str,
    /// The id of the printer in the catalog.
    pub printer_id: &'a str,
    pub nozzle_mm: f64,
    pub predicted_seconds: u64,
    pub weight_g: f64,
    pub filament_m: f64,
    /// The family of the plastic in capitals (`PLA`), empty when no filament was chosen.
    pub filament_type: &'a str,
    /// PNG files; an empty slice leaves the picture out.
    pub picture: &'a [u8],
    pub picture_small: &'a [u8],
}

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("gcode.3mf", reason)
}

fn finite(v: f64, what: &str) -> Result<f64, FormatError> {
    if v.is_finite() && v >= 0.0 {
        Ok(v)
    } else {
        Err(bad(format!("the {what} is not a figure")))
    }
}

/// The bytes of the package. The same plate gives the same bytes.
pub fn write(plate: &Plate) -> Result<Vec<u8>, FormatError> {
    if plate.gcode.trim().is_empty() {
        return Err(bad("there is no G-code"));
    }
    let weight = finite(plate.weight_g, "weight")?;
    let metres = finite(plate.filament_m, "length of filament")?;
    let nozzle = finite(plate.nozzle_mm, "nozzle")?;
    let title = xml::escape(plate.title);
    let printer = xml::escape(plate.printer_id);
    let material = xml::escape(plate.filament_type);
    let mut types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n",
    );
    for (ext, kind) in [
        (
            "rels",
            "application/vnd.openxmlformats-package.relationships+xml",
        ),
        (
            "model",
            "application/vnd.ms-package.3dmanufacturing-3dmodel+xml",
        ),
        ("png", "image/png"),
        ("gcode", "text/x.gcode"),
        ("config", "application/xml"),
        ("md5", "text/plain"),
    ] {
        types.push_str(&format!(
            " <Default Extension=\"{ext}\" ContentType=\"{kind}\"/>\n"
        ));
    }
    types.push_str("</Types>\n");
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n",
    );
    if !plate.picture.is_empty() {
        rels.push_str(&format!(
            " <Relationship Target=\"/{PICTURE}\" Id=\"rel1\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail\"/>\n"
        ));
    }
    rels.push_str("</Relationships>\n");
    let model = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n <metadata name=\"Title\">{title}</metadata>\n <metadata name=\"Application\">PHOENIX-3DP</metadata>\n <resources/>\n <build/>\n</model>\n"
    );
    let mut info = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n <plate>\n  <metadata key=\"index\" value=\"1\"/>\n  <metadata key=\"printer_model_id\" value=\"{printer}\"/>\n  <metadata key=\"nozzle_diameter\" value=\"{nozzle}\"/>\n  <metadata key=\"prediction\" value=\"{}\"/>\n  <metadata key=\"weight\" value=\"{weight:.2}\"/>\n",
        plate.predicted_seconds
    );
    info.push_str(&format!(
        "  <filament id=\"1\" type=\"{material}\" used_m=\"{metres:.2}\" used_g=\"{weight:.2}\"/>\n </plate>\n</config>\n"
    ));
    let mut settings = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n <plate>\n  <metadata key=\"plater_id\" value=\"1\"/>\n  <metadata key=\"gcode_file\" value=\"{GCODE}\"/>\n"
    );
    if !plate.picture.is_empty() {
        settings.push_str(&format!(
            "  <metadata key=\"thumbnail_file\" value=\"{PICTURE}\"/>\n"
        ));
    }
    settings.push_str(" </plate>\n</config>\n");
    let mut z = Writer::new();
    z.add("[Content_Types].xml", types.as_bytes())?;
    z.add("_rels/.rels", rels.as_bytes())?;
    z.add(MODEL, model.as_bytes())?;
    z.add(GCODE, plate.gcode.as_bytes())?;
    z.add(CHECKSUM, md5_hex(plate.gcode.as_bytes()).as_bytes())?;
    if !plate.picture.is_empty() {
        z.add(PICTURE, plate.picture)?;
    }
    if !plate.picture_small.is_empty() {
        z.add(PICTURE_SMALL, plate.picture_small)?;
    }
    z.add(SLICE_INFO, info.as_bytes())?;
    z.add(PLATE_SETTINGS, settings.as_bytes())?;
    Ok(z.finish())
}

/// Whether the checksum beside the G-code agrees with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checksum {
    Agrees,
    Differs,
    /// The package has no checksum file.
    Missing,
}

/// The G-code of a plate taken out of a package, with what its checksum says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlateGcode {
    pub name: String,
    pub text: String,
    pub checksum: Checksum,
}

/// The G-code of the first plate of a package (`Metadata/plate_N.gcode` with the lowest number); `None` when the package carries none.
pub fn read_gcode(bytes: &[u8], limits: &Limits) -> Result<Option<PlateGcode>, FormatError> {
    let archive = Archive::open(bytes, limits)?;
    let number = |name: &str| -> Option<u32> {
        name.strip_prefix("Metadata/plate_")?
            .strip_suffix(".gcode")?
            .parse()
            .ok()
    };
    let Some(entry) = archive
        .entries
        .iter()
        .filter(|e| number(&e.name).is_some())
        .min_by_key(|e| number(&e.name))
    else {
        return Ok(None);
    };
    let data = archive.read(entry)?;
    let text = String::from_utf8(data).map_err(|_| bad(format!("{} is not text", entry.name)))?;
    let checksum = match archive.find(&format!("{}.md5", entry.name)) {
        None => Checksum::Missing,
        Some(e) if e.size() > 128 => Checksum::Differs,
        Some(e) => {
            let written = archive.read(e)?;
            let written = String::from_utf8_lossy(&written);
            if written
                .trim()
                .eq_ignore_ascii_case(&md5_hex(text.as_bytes()))
            {
                Checksum::Agrees
            } else {
                Checksum::Differs
            }
        }
    };
    Ok(Some(PlateGcode {
        name: entry.name.clone(),
        text,
        checksum,
    }))
}

/// The largest PNG picture under `Metadata/` of a package; `None` when it has none.
pub fn read_picture(bytes: &[u8], limits: &Limits) -> Result<Option<Vec<u8>>, FormatError> {
    let archive = Archive::open(bytes, limits)?;
    let Some(entry) = archive
        .entries
        .iter()
        .filter(|e| {
            e.name.starts_with("Metadata/") && e.name.to_ascii_lowercase().ends_with(".png")
        })
        .max_by_key(|e| e.size())
    else {
        return Ok(None);
    };
    let data = archive.read(entry)?;
    if !data.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(bad(format!("{} is not a PNG picture", entry.name)));
    }
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project3mf::{read_project, Producer};

    const PROGRAM: &str = "G28\nG1 X10 Y10 F3000\nM400\n";

    fn plate(picture: &[u8]) -> Plate<'_> {
        Plate {
            gcode: PROGRAM,
            title: "a <part> & \"more\"",
            printer_id: "bambu-lab-a1",
            nozzle_mm: 0.4,
            predicted_seconds: 754,
            weight_g: 3.456,
            filament_m: 1.159,
            filament_type: "PLA",
            picture,
            picture_small: picture,
        }
    }

    fn png() -> Vec<u8> {
        crate::png_write::encode_rgba(2, 2, &[255; 16]).unwrap()
    }

    #[test]
    fn what_is_written_is_a_package_that_the_program_reads_back() {
        let picture = png();
        let bytes = write(&plate(&picture)).unwrap();
        assert_eq!(bytes, write(&plate(&picture)).unwrap(), "the same bytes");
        let limits = Limits::default();
        let got = read_gcode(&bytes, &limits).unwrap().unwrap();
        assert_eq!(got.name, GCODE);
        assert_eq!(got.text, PROGRAM);
        assert_eq!(got.checksum, Checksum::Agrees);
        assert_eq!(read_picture(&bytes, &limits).unwrap(), Some(picture));
        let info = read_project(&bytes, &limits).unwrap();
        assert_eq!(info.producer, Producer::Phoenix);
        assert_eq!(info.title, "a <part> & \"more\"");
        assert!(info.has_gcode);
        assert_eq!(info.plates.len(), 1);
        assert_eq!(info.plates[0].predicted_seconds, Some(754));
        assert_eq!(info.plates[0].weight_g, Some(3.46));
        assert_eq!(info.thumbnails.len(), 2);
    }

    #[test]
    fn the_parts_of_the_container_are_all_there_and_declared() {
        let picture = png();
        let bytes = write(&plate(&picture)).unwrap();
        let a = Archive::open(&bytes, &Limits::default()).unwrap();
        let types = a.read(a.find("[Content_Types].xml").unwrap()).unwrap();
        let types = String::from_utf8(types).unwrap();
        for e in &a.entries {
            if e.name != "[Content_Types].xml" {
                let ext = e.name.rsplit_once('.').unwrap().1;
                assert!(
                    types.contains(&format!("Extension=\"{ext}\"")),
                    "{} has no content type",
                    e.name
                );
            }
        }
        let rels = a.read(a.find("_rels/.rels").unwrap()).unwrap();
        let rels = String::from_utf8(rels).unwrap();
        assert!(rels.contains("/3D/3dmodel.model") && rels.contains(PICTURE));
        let none = write(&plate(&[])).unwrap();
        let a = Archive::open(&none, &Limits::default()).unwrap();
        assert!(a.find(PICTURE).is_none() && a.find(PICTURE_SMALL).is_none());
        let rels = a.read(a.find("_rels/.rels").unwrap()).unwrap();
        assert!(!String::from_utf8(rels).unwrap().contains("thumbnail"));
        assert_eq!(read_picture(&none, &Limits::default()).unwrap(), None);
    }

    #[test]
    fn a_checksum_that_differs_or_is_missing_is_told() {
        let pack = |entries: &[(&str, &[u8])]| {
            let mut z = Writer::new();
            for (n, d) in entries {
                z.add(n, d).unwrap();
            }
            z.finish()
        };
        let limits = Limits::default();
        let get = |b: Vec<u8>| read_gcode(&b, &limits).unwrap();
        let wrong = pack(&[(GCODE, b"G28\n"), (CHECKSUM, b"00")]);
        assert_eq!(get(wrong).unwrap().checksum, Checksum::Differs);
        let small = md5_hex(b"G28\n").to_lowercase();
        let lower = pack(&[(GCODE, b"G28\n"), (CHECKSUM, small.as_bytes())]);
        assert_eq!(get(lower).unwrap().checksum, Checksum::Agrees);
        let long = pack(&[(GCODE, b"G28\n"), (CHECKSUM, &[b'a'; 500])]);
        assert_eq!(get(long).unwrap().checksum, Checksum::Differs);
        let alone = pack(&[
            ("Metadata/plate_3.gcode", b"G1 Z3\n"),
            ("Metadata/plate_2.gcode", b"G1 Z2\n"),
        ]);
        let got = get(alone).unwrap();
        assert_eq!(
            (got.text.as_str(), got.checksum),
            ("G1 Z2\n", Checksum::Missing)
        );
        assert_eq!(get(pack(&[("other.gcode", b"G28\n")])), None);
        assert!(read_gcode(&pack(&[(GCODE, &[0xFF, 0xFE])]), &limits).is_err());
        assert!(read_gcode(b"not a package", &limits).is_err());
    }

    #[test]
    fn an_empty_program_and_figures_that_are_not_figures_are_refused() {
        let mut p = plate(&[]);
        p.gcode = "  \n";
        assert!(write(&p).is_err());
        for broken in [f64::NAN, f64::INFINITY, -1.0] {
            let mut p = plate(&[]);
            p.weight_g = broken;
            assert!(write(&p).is_err(), "{broken}");
            let mut p = plate(&[]);
            p.filament_m = broken;
            assert!(write(&p).is_err(), "{broken}");
        }
    }
}
