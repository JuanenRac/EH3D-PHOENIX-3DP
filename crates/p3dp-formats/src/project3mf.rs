//! Reading the projects that other programs save as 3MF, in read-only mode: which program made the package, which objects and plates it holds and a handful of the settings it was
//! sliced with. It reads only the small files of metadata that sit beside the model; the model itself (which these programs often split into component files) is not read here, and
//! nothing is written back. The layouts it knows were learned from how the packages look (names of entries, names of tags and keys), never from the code or the texts of those programs.
//!
//! Limits against a package made to hurt: the package is opened with the limits of the program (entries, inflated size, ratio); a file of metadata larger than
//! [`MAX_METADATA_BYTES`] is refused before it is inflated; at most [`MAX_OBJECTS`] objects and [`MAX_PLATES`] plates are read; every text is cut to a short length.

use crate::error::FormatError;
use crate::xml::{self, Tag};
use crate::zip::Archive;
use p3dp_core::json::{self, Value};
use p3dp_core::Limits;

/// The largest file of metadata that is read, in bytes once inflated.
pub const MAX_METADATA_BYTES: u64 = 8 << 20;
/// The most objects read from a project.
pub const MAX_OBJECTS: usize = 10_000;
/// The most plates read from a project.
pub const MAX_PLATES: usize = 1_000;
/// The longest text kept of a name or a value.
const MAX_TEXT: usize = 200;
/// How much of the start of the model is looked at for its metadata.
const MODEL_HEAD: usize = 64 << 10;

/// The program a package seems to come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Producer {
    /// A package with the files of the Prusa family of slicers.
    Prusa,
    /// A package with the files of Bambu Studio.
    Bambu,
    /// A package with the files of OrcaSlicer.
    Orca,
    /// A package written by this program.
    Phoenix,
    /// A plain 3MF with none of those files.
    Plain,
}

impl Producer {
    pub fn name(self) -> &'static str {
        match self {
            Producer::Prusa => "prusa",
            Producer::Bambu => "bambu",
            Producer::Orca => "orca",
            Producer::Phoenix => "phoenix",
            Producer::Plain => "plain",
        }
    }
}

/// An object of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub id: String,
    pub name: String,
    /// The extruder (or filament slot) the object is set to, when the project says.
    pub extruder: Option<u32>,
}

/// A plate of the project: the objects that are on it, and what the slicer predicted when it said.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateInfo {
    pub id: u32,
    pub name: String,
    /// The ids of the objects on the plate.
    pub object_ids: Vec<String>,
    pub predicted_seconds: Option<u64>,
    pub weight_g: Option<f64>,
}

/// What a project says of itself.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectInfo {
    pub producer: Producer,
    /// The text of the `Application` metadata, as it is.
    pub application: String,
    pub title: String,
    pub objects: Vec<ObjectInfo>,
    pub plates: Vec<PlateInfo>,
    /// Some of the settings, under the names of this program (see [`SETTING_KEYS`]); only the ones the project has.
    pub settings: Vec<(String, String)>,
    /// How many items the `build` of the model lists.
    pub build_items: usize,
    /// The names of the pictures in the package.
    pub thumbnails: Vec<String>,
    /// Whether the package carries G-code.
    pub has_gcode: bool,
    /// How many entries the package has.
    pub entries: usize,
}

impl ProjectInfo {
    /// A setting by the name of this program.
    pub fn setting(&self, key: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// The names under which the settings are given, in order.
pub const SETTING_KEYS: [&str; 12] = [
    "layer_height_mm",
    "first_layer_height_mm",
    "nozzle_diameter_mm",
    "perimeters",
    "infill_percent",
    "filament_type",
    "printer",
    "printer_profile",
    "print_profile",
    "filament_profile",
    "nozzle_c",
    "bed_c",
];

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("3mf", reason)
}

fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_string()
}

/// The text of an entry, refused before it is inflated when it is too large.
fn text_of(archive: &Archive, name: &str) -> Result<Option<String>, FormatError> {
    let Some(entry) = archive.find(name) else {
        return Ok(None);
    };
    if entry.size() > MAX_METADATA_BYTES {
        return Err(FormatError::TooLarge {
            bytes: entry.size(),
            max: MAX_METADATA_BYTES,
        });
    }
    let bytes = archive.read(entry)?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| bad(format!("{name} is not valid UTF-8")))
}

/// The `<metadata name="X">text</metadata>` lines at the head of the model.
fn model_metadata(model: &str) -> Vec<(String, String)> {
    let mut end = model.len().min(MODEL_HEAD);
    while !model.is_char_boundary(end) {
        end -= 1;
    }
    let head = &model[..end];
    let head = head.find("<resources").map_or(head, |i| &head[..i]);
    let mut out = Vec::new();
    let mut rest = head;
    while let Some(i) = rest.find("<metadata") {
        rest = &rest[i + "<metadata".len()..];
        let Some(close) = rest.find('>') else { break };
        let attrs = &rest[..close];
        let name = attrs
            .split("name=\"")
            .nth(1)
            .and_then(|a| a.split('"').next())
            .map(xml::unescape)
            .unwrap_or_default();
        if attrs.trim_end().ends_with('/') {
            continue;
        }
        let body = &rest[close + 1..];
        let Some(end_tag) = body.find("</metadata>") else {
            break;
        };
        if !name.is_empty() {
            out.push((clean(&name), clean(&xml::unescape(&body[..end_tag]))));
        }
        rest = &body[end_tag..];
    }
    out
}

/// The number of `<item` of the `<build>` of the model.
fn build_items(model: &str) -> usize {
    match model.rfind("<build") {
        Some(i) => model[i..].matches("<item").count(),
        None => 0,
    }
}

#[derive(Default)]
struct Walk {
    objects: Vec<ObjectInfo>,
    plates: Vec<PlateInfo>,
}

/// The objects and the plates of the file of settings of the Bambu and Orca packages and of the file of the Prusa ones. Both keep the facts as attributes `key` and `value` of
/// `metadata` tags, inside `object` (whose parts or volumes have names of their own) and `plate` with its `model_instance`.
fn read_config(text: &str) -> Result<Walk, FormatError> {
    let tags = xml::tags(text)?;
    let mut w = Walk::default();
    let mut object: Option<usize> = None;
    let mut plate: Option<PlateInfo> = None;
    let mut in_part = false;
    let mut in_instance = false;
    for t in &tags {
        let Tag {
            name,
            closing,
            self_closing,
            ..
        } = t;
        match (name.as_str(), *closing) {
            ("object", false) => {
                if w.objects.len() >= MAX_OBJECTS {
                    return Err(bad(format!("more than {MAX_OBJECTS} objects")));
                }
                w.objects.push(ObjectInfo {
                    id: clean(t.attr("id").unwrap_or("")),
                    name: String::new(),
                    extruder: None,
                });
                object = if *self_closing {
                    None
                } else {
                    Some(w.objects.len() - 1)
                };
            }
            ("object", true) => object = None,
            ("part" | "volume", false) => in_part = !*self_closing,
            ("part" | "volume", true) => in_part = false,
            ("plate", false) => {
                if w.plates.len() >= MAX_PLATES {
                    return Err(bad(format!("more than {MAX_PLATES} plates")));
                }
                let p = PlateInfo {
                    id: u32::try_from(w.plates.len() + 1).unwrap_or(u32::MAX),
                    name: String::new(),
                    object_ids: Vec::new(),
                    predicted_seconds: None,
                    weight_g: None,
                };
                if *self_closing {
                    w.plates.push(p);
                } else {
                    plate = Some(p);
                }
            }
            ("plate", true) => {
                if let Some(p) = plate.take() {
                    w.plates.push(p);
                }
            }
            ("model_instance", false) => in_instance = !*self_closing,
            ("model_instance", true) => in_instance = false,
            ("metadata", false) => {
                let (Some(key), Some(value)) = (t.attr("key"), t.attr("value")) else {
                    continue;
                };
                let value = clean(value);
                if in_instance {
                    if key == "object_id" {
                        if let Some(p) = plate.as_mut() {
                            if p.object_ids.len() < MAX_OBJECTS {
                                p.object_ids.push(value);
                            }
                        }
                    }
                } else if let Some(p) = plate.as_mut() {
                    match key {
                        "plater_id" | "index" => {
                            if let Ok(n) = value.parse() {
                                p.id = n;
                            }
                        }
                        "plater_name" | "name" => p.name = value,
                        "prediction" => {
                            p.predicted_seconds = value
                                .parse::<f64>()
                                .ok()
                                .filter(|v| *v >= 0.0 && v.is_finite())
                                .map(|v| v as u64)
                        }
                        "weight" => {
                            p.weight_g = value
                                .parse()
                                .ok()
                                .filter(|v: &f64| *v >= 0.0 && v.is_finite())
                        }
                        _ => {}
                    }
                } else if let (Some(i), false) = (object, in_part) {
                    let o = &mut w.objects[i];
                    match key {
                        "name" => o.name = value,
                        "extruder" => o.extruder = value.parse().ok(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(w)
}

/// The text of a value of the settings of Bambu and Orca: a string, or the first of a list of them.
fn first_text(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(clean(s)),
        Value::Num(n) if n.is_finite() => Some(format!("{n}")),
        Value::Arr(a) => a.first().and_then(first_text),
        _ => None,
    }
}

/// The settings of the JSON file (Bambu and Orca), under the names of this program.
fn settings_json(text: &str) -> Result<Vec<(String, String)>, FormatError> {
    let v = json::parse(text).map_err(|e| bad(format!("the settings are not JSON: {e}")))?;
    let table = [
        ("layer_height", "layer_height_mm"),
        ("initial_layer_print_height", "first_layer_height_mm"),
        ("nozzle_diameter", "nozzle_diameter_mm"),
        ("wall_loops", "perimeters"),
        ("sparse_infill_density", "infill_percent"),
        ("filament_type", "filament_type"),
        ("printer_model", "printer"),
        ("printer_settings_id", "printer_profile"),
        ("print_settings_id", "print_profile"),
        ("filament_settings_id", "filament_profile"),
        ("nozzle_temperature", "nozzle_c"),
        ("hot_plate_temp", "bed_c"),
    ];
    let mut out = Vec::new();
    for (from, to) in table {
        if let Some(t) = v.get(from).and_then(first_text) {
            let t = if to == "infill_percent" {
                t.trim_end_matches('%').to_string()
            } else {
                t
            };
            out.push((to.to_string(), t));
        }
    }
    Ok(out)
}

/// The settings of the `; key = value` file (Prusa), under the names of this program.
fn settings_lines(text: &str) -> Vec<(String, String)> {
    let table = [
        ("layer_height", "layer_height_mm"),
        ("first_layer_height", "first_layer_height_mm"),
        ("nozzle_diameter", "nozzle_diameter_mm"),
        ("perimeters", "perimeters"),
        ("fill_density", "infill_percent"),
        ("filament_type", "filament_type"),
        ("printer_model", "printer"),
        ("printer_settings_id", "printer_profile"),
        ("print_settings_id", "print_profile"),
        ("filament_settings_id", "filament_profile"),
        ("temperature", "nozzle_c"),
        ("bed_temperature", "bed_c"),
    ];
    let mut found: Vec<(String, String)> = Vec::new();
    for line in text.lines().take(100_000) {
        let line = line.trim().trim_start_matches(';').trim();
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        found.push((k.trim().to_string(), v.trim().to_string()));
    }
    let mut out = Vec::new();
    for (from, to) in table {
        if let Some((_, v)) = found.iter().find(|(k, _)| k == from) {
            // a list of values (one per extruder) gives its first
            let first = v
                .split([',', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"');
            let first = if to == "infill_percent" {
                first.trim_end_matches('%')
            } else {
                first
            };
            out.push((to.to_string(), clean(first)));
        }
    }
    out
}

/// Reads a project. A package that is not a package of 3MF, or that has no model, is an error; one made by a program this does not know is a plain project with what is in it.
pub fn read_project(bytes: &[u8], limits: &Limits) -> Result<ProjectInfo, FormatError> {
    let archive = Archive::open(bytes, limits)?;
    let model =
        text_of(&archive, "3D/3dmodel.model")?.ok_or_else(|| bad("the package has no 3D model"))?;
    let meta = model_metadata(&model);
    let meta_of = |k: &str| {
        meta.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let application = meta_of("Application");
    let title = meta_of("Title");
    let has = |name: &str| archive.find(name).is_some();
    let lower = application.to_ascii_lowercase();
    let producer = if lower.starts_with("bambustudio") {
        Producer::Bambu
    } else if lower.starts_with("orca") {
        Producer::Orca
    } else if lower.starts_with("prusaslicer")
        || lower.starts_with("slic3r")
        || has("Metadata/Slic3r_PE.config")
    {
        Producer::Prusa
    } else if lower.starts_with("phoenix-3dp") {
        Producer::Phoenix
    } else if has("Metadata/project_settings.config") && has("Metadata/model_settings.config") {
        // the files of the Bambu family with no name of the program: the nearest of the two
        Producer::Bambu
    } else {
        Producer::Plain
    };
    let mut walk = Walk::default();
    for config in [
        "Metadata/model_settings.config",
        "Metadata/Slic3r_PE_model.config",
    ] {
        if let Some(t) = text_of(&archive, config)? {
            walk = read_config(&t)?;
            break;
        }
    }
    let mut settings = Vec::new();
    if let Some(t) = text_of(&archive, "Metadata/project_settings.config")? {
        settings = settings_json(&t)?;
    } else if let Some(t) = text_of(&archive, "Metadata/Slic3r_PE.config")? {
        settings = settings_lines(&t);
    }
    // the prediction and the weight of each plate, when the package says
    if let Some(t) = text_of(&archive, "Metadata/slice_info.config")? {
        let info = read_config(&t)?;
        for (i, p) in info.plates.iter().enumerate() {
            let at = walk
                .plates
                .iter()
                .position(|q| q.id == p.id)
                .or((i < walk.plates.len()).then_some(i));
            if let Some(mine) = at.map(|k| &mut walk.plates[k]) {
                mine.predicted_seconds = mine.predicted_seconds.or(p.predicted_seconds);
                mine.weight_g = mine.weight_g.or(p.weight_g);
            }
        }
    }
    if walk.plates.is_empty() && !walk.objects.is_empty() {
        walk.plates.push(PlateInfo {
            id: 1,
            name: String::new(),
            object_ids: walk.objects.iter().map(|o| o.id.clone()).collect(),
            predicted_seconds: None,
            weight_g: None,
        });
    }
    let names: Vec<&str> = archive.entries.iter().map(|e| e.name.as_str()).collect();
    let thumbnails: Vec<String> = names
        .iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(".png") && n.starts_with("Metadata/"))
        .take(64)
        .map(|n| clean(n))
        .collect();
    let has_gcode = names.iter().any(|n| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".gcode") || l.ends_with(".bgcode")
    });
    Ok(ProjectInfo {
        producer,
        application,
        title,
        objects: walk.objects,
        plates: walk.plates,
        settings,
        build_items: build_items(&model),
        thumbnails,
        has_gcode,
        entries: archive.entries.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::Writer;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;

    const MODEL_HEAD_TEXT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n";

    fn model(app: &str, extra: &str) -> String {
        format!(
            "{MODEL_HEAD_TEXT} <metadata name=\"Application\">{app}</metadata>\n <metadata name=\"Title\">Test &amp; part</metadata>\n{extra} <resources>\n  <object id=\"1\" type=\"model\"><mesh><vertices/><triangles/></mesh></object>\n </resources>\n <build>\n  <item objectid=\"1\"/>\n  <item objectid=\"2\"/>\n </build>\n</model>\n"
        )
    }

    fn pack(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = Writer::new();
        for (n, d) in entries {
            z.add(n, d).unwrap();
        }
        z.finish()
    }

    fn prusa() -> Vec<u8> {
        let m = model(
            "PrusaSlicer-2.7.0",
            " <metadata name=\"slic3rpe:Version3mf\">1</metadata>\n",
        );
        let objects = "<?xml version=\"1.0\"?>\n<config>\n <object id=\"1\" instances_count=\"1\">\n  <metadata type=\"object\" key=\"name\" value=\"bracket\"/>\n  <metadata type=\"object\" key=\"extruder\" value=\"2\"/>\n  <volume firstid=\"0\" lastid=\"11\">\n   <metadata type=\"volume\" key=\"name\" value=\"a part name\"/>\n  </volume>\n </object>\n <object id=\"2\" instances_count=\"1\">\n  <metadata type=\"object\" key=\"name\" value=\"lid\"/>\n </object>\n</config>\n";
        let ini = "; generated by a slicer\n; layer_height = 0.15\n; first_layer_height = 0.2\n; nozzle_diameter = 0.4,0.4\n; perimeters = 3\n; fill_density = 15%\n; filament_type = PETG;PETG\n; printer_model = MK4S\n; printer_settings_id = \"Printer A\"\n; temperature = 240,240\n; bed_temperature = 85\n; unrelated = 1\n";
        pack(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("3D/3dmodel.model", m.as_bytes()),
            ("Metadata/Slic3r_PE.config", ini.as_bytes()),
            ("Metadata/Slic3r_PE_model.config", objects.as_bytes()),
            ("Metadata/thumbnail.png", b"\x89PNG"),
        ])
    }

    fn bambu_like(app: &str) -> Vec<u8> {
        let m = model(app, "");
        let settings = "<?xml version=\"1.0\"?>\n<config>\n <object id=\"2\">\n  <metadata key=\"name\" value=\"Cube\"/>\n  <metadata key=\"extruder\" value=\"1\"/>\n  <part id=\"1\" subtype=\"normal_part\">\n   <metadata key=\"name\" value=\"inner part\"/>\n  </part>\n </object>\n <object id=\"3\">\n  <metadata key=\"name\" value=\"Ring\"/>\n </object>\n <plate>\n  <metadata key=\"plater_id\" value=\"1\"/>\n  <metadata key=\"plater_name\" value=\"first\"/>\n  <model_instance>\n   <metadata key=\"object_id\" value=\"2\"/>\n   <metadata key=\"instance_id\" value=\"0\"/>\n  </model_instance>\n </plate>\n <plate>\n  <metadata key=\"plater_id\" value=\"2\"/>\n  <metadata key=\"plater_name\" value=\"\"/>\n  <model_instance>\n   <metadata key=\"object_id\" value=\"3\"/>\n  </model_instance>\n </plate>\n</config>\n";
        let project = "{\"layer_height\":\"0.16\",\"initial_layer_print_height\":\"0.2\",\"nozzle_diameter\":[\"0.4\"],\"wall_loops\":\"2\",\"sparse_infill_density\":\"18%\",\"filament_type\":[\"PLA\",\"PETG\"],\"printer_model\":\"Bambu Lab A1\",\"printer_settings_id\":\"Bambu Lab A1 0.4 nozzle\",\"nozzle_temperature\":[\"220\"],\"hot_plate_temp\":[\"55\"],\"other\":1}";
        let slice = "<?xml version=\"1.0\"?>\n<config>\n <plate>\n  <metadata key=\"index\" value=\"1\"/>\n  <metadata key=\"prediction\" value=\"3725\"/>\n  <metadata key=\"weight\" value=\"12.5\"/>\n </plate>\n <plate>\n  <metadata key=\"index\" value=\"2\"/>\n  <metadata key=\"prediction\" value=\"600\"/>\n </plate>\n</config>\n";
        pack(&[
            ("3D/3dmodel.model", m.as_bytes()),
            ("Metadata/model_settings.config", settings.as_bytes()),
            ("Metadata/project_settings.config", project.as_bytes()),
            ("Metadata/slice_info.config", slice.as_bytes()),
            ("Metadata/plate_1.png", b"\x89PNG"),
            ("Metadata/top_1.png", b"\x89PNG"),
            ("Metadata/plate_1.gcode", b"G28\n"),
        ])
    }

    #[test]
    fn a_project_of_the_prusa_family_gives_its_objects_and_settings() {
        let p = read_project(&prusa(), &Limits::default()).unwrap();
        assert_eq!(p.producer, Producer::Prusa);
        assert_eq!(p.application, "PrusaSlicer-2.7.0");
        assert_eq!(p.title, "Test & part");
        assert_eq!(p.objects.len(), 2);
        assert_eq!(
            (
                p.objects[0].id.as_str(),
                p.objects[0].name.as_str(),
                p.objects[0].extruder
            ),
            ("1", "bracket", Some(2))
        );
        assert_eq!(
            (p.objects[1].name.as_str(), p.objects[1].extruder),
            ("lid", None)
        );
        assert_eq!(
            p.plates.len(),
            1,
            "one plate is made up for a package with none"
        );
        assert_eq!(
            p.plates[0].object_ids,
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(p.setting("layer_height_mm"), Some("0.15"));
        assert_eq!(p.setting("nozzle_diameter_mm"), Some("0.4"));
        assert_eq!(p.setting("infill_percent"), Some("15"));
        assert_eq!(p.setting("filament_type"), Some("PETG"));
        assert_eq!(p.setting("printer_profile"), Some("Printer A"));
        assert_eq!(
            (p.setting("nozzle_c"), p.setting("bed_c")),
            (Some("240"), Some("85"))
        );
        assert_eq!(p.setting("unrelated"), None);
        assert_eq!(p.build_items, 2);
        assert_eq!(p.thumbnails, vec!["Metadata/thumbnail.png".to_string()]);
        assert!(!p.has_gcode);
        assert_eq!(p.entries, 5);
    }

    #[test]
    fn projects_of_bambu_and_orca_give_plates_with_their_prediction() {
        for (app, producer) in [
            ("BambuStudio-01.09.00.00", Producer::Bambu),
            ("OrcaSlicer-2.1.0", Producer::Orca),
        ] {
            let p = read_project(&bambu_like(app), &Limits::default()).unwrap();
            assert_eq!(p.producer, producer, "{app}");
            assert_eq!(p.objects.len(), 2);
            assert_eq!(
                (p.objects[0].name.as_str(), p.objects[0].extruder),
                ("Cube", Some(1))
            );
            assert_eq!(
                p.objects[1].name, "Ring",
                "the name of a part is not the name of the object"
            );
            assert_eq!(p.plates.len(), 2);
            assert_eq!((p.plates[0].id, p.plates[0].name.as_str()), (1, "first"));
            assert_eq!(p.plates[0].object_ids, vec!["2".to_string()]);
            assert_eq!(p.plates[1].object_ids, vec!["3".to_string()]);
            assert_eq!(
                (p.plates[0].predicted_seconds, p.plates[0].weight_g),
                (Some(3725), Some(12.5))
            );
            assert_eq!(
                (p.plates[1].predicted_seconds, p.plates[1].weight_g),
                (Some(600), None)
            );
            assert_eq!(p.setting("layer_height_mm"), Some("0.16"));
            assert_eq!(p.setting("infill_percent"), Some("18"));
            assert_eq!(p.setting("filament_type"), Some("PLA"));
            assert_eq!(p.setting("printer"), Some("Bambu Lab A1"));
            assert_eq!(
                (p.setting("nozzle_c"), p.setting("bed_c")),
                (Some("220"), Some("55"))
            );
            assert!(p.has_gcode);
            assert_eq!(p.thumbnails.len(), 2);
        }
    }

    #[test]
    fn a_package_of_this_program_and_a_plain_one_are_known_for_what_they_are() {
        let own = crate::threemf::write(
            &cube(10.0),
            &crate::threemf::Meta {
                title: "mine".into(),
                printer: "bambu-lab-a1".into(),
            },
        )
        .unwrap();
        let p = read_project(&own, &Limits::default()).unwrap();
        assert_eq!((p.producer, p.title.as_str()), (Producer::Phoenix, "mine"));
        assert!(p.objects.is_empty() && p.plates.is_empty() && p.settings.is_empty());
        assert_eq!(p.build_items, 1);
        let plain = pack(&[("3D/3dmodel.model", model("Some CAD 4.0", "").as_bytes())]);
        assert_eq!(
            read_project(&plain, &Limits::default()).unwrap().producer,
            Producer::Plain
        );
        // the layout of the Bambu family with an application name nobody knows
        let layout = pack(&[
            ("3D/3dmodel.model", model("Unknown", "").as_bytes()),
            ("Metadata/model_settings.config", b"<config/>"),
            ("Metadata/project_settings.config", b"{}"),
        ]);
        assert_eq!(
            read_project(&layout, &Limits::default()).unwrap().producer,
            Producer::Bambu
        );
    }

    #[test]
    fn what_is_not_a_project_is_an_error() {
        let l = Limits::default();
        assert!(read_project(b"not a zip", &l).is_err());
        assert!(read_project(&pack(&[("readme.txt", b"hello")]), &l).is_err());
        assert!(read_project(&pack(&[("3D/3dmodel.model", b"\xff\xfe\x00")]), &l).is_err());
        let bad_json = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            ("Metadata/project_settings.config", b"{not json"),
        ]);
        assert!(read_project(&bad_json, &l).is_err());
        let bad_xml = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            (
                "Metadata/model_settings.config",
                b"<config><object id=\"1\"",
            ),
        ]);
        assert!(read_project(&bad_xml, &l).is_err());
    }

    #[test]
    fn a_file_of_metadata_too_large_is_refused_before_it_is_read() {
        let big = vec![b' '; (MAX_METADATA_BYTES + 1) as usize];
        let pkg = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            ("Metadata/model_settings.config", &big),
        ]);
        let limits = Limits {
            max_inflated_bytes: 64 << 20,
            ..Limits::default()
        };
        assert!(matches!(
            read_project(&pkg, &limits),
            Err(FormatError::TooLarge { .. })
        ));
    }

    #[test]
    fn too_many_objects_or_plates_are_refused() {
        let mut objects = String::from("<config>");
        for i in 0..=MAX_OBJECTS {
            objects.push_str(&format!("<object id=\"{i}\"/>"));
        }
        objects.push_str("</config>");
        let pkg = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            ("Metadata/model_settings.config", objects.as_bytes()),
        ]);
        assert!(read_project(&pkg, &Limits::default()).is_err());
        let mut plates = String::from("<config>");
        for _ in 0..=MAX_PLATES {
            plates.push_str("<plate/>");
        }
        plates.push_str("</config>");
        let pkg = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            ("Metadata/model_settings.config", plates.as_bytes()),
        ]);
        assert!(read_project(&pkg, &Limits::default()).is_err());
    }

    #[test]
    fn texts_are_cut_and_cleaned_and_a_package_cut_or_damaged_never_panics() {
        let long = "x".repeat(5000);
        let cfg = format!("<config><object id=\"1\"><metadata key=\"name\" value=\"a{}b\u{7}\"/></object></config>", long);
        let pkg = pack(&[
            ("3D/3dmodel.model", model("BambuStudio-1", "").as_bytes()),
            ("Metadata/model_settings.config", cfg.as_bytes()),
        ]);
        let p = read_project(&pkg, &Limits::default()).unwrap();
        assert_eq!(p.objects[0].name.chars().count(), MAX_TEXT);
        assert!(!p.objects[0].name.contains('\u{7}'));
        let good = bambu_like("OrcaSlicer-2.1.0");
        let mut rng = Rng::new(0x3D3F_0001);
        for cut in (0..good.len()).step_by(7) {
            let _ = read_project(&good[..cut], &Limits::default());
        }
        for _ in 0..300 {
            let mut d = good.clone();
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(d.len() as u64) as usize;
                d[i] = rng.below(256) as u8;
            }
            let _ = read_project(&d, &Limits::default());
        }
    }
}
