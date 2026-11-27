//! The branch `3dp/` of an `.eh3d` project: one file with a branch for each program of the suite (the format is the crate `eh3d-project` of EH3D-COMMON).
//!
//! This program writes only `3dp/`. It keeps there the parts of the bed with their placement, the settings of the slice, the printer, the filament, the G-code of the slice and the
//! start and end G-code of the person. What it
//! takes from another branch is a copy with a reference that remembers the content hash and the format of its source, so that a later change of the source is told as out of date. Every
//! other branch, known or not, keeps its bytes. Nothing in a project is run and nothing here is sent to a printer.

use std::collections::BTreeMap;
use std::path::Path;

use eh3d_project::appbranch::{
    self, counted, extension_of, file_name, read_bounded, BranchError, BranchKit, BranchResult,
    Conversion, Notice, WorkingCopy,
};
use eh3d_project::meta::ConversionReport;
use p3dp_core::{Limits, Mesh};
use p3dp_formats::project3mf::read_project;
use p3dp_formats::read_mesh;
use p3dp_formats::threemf::{self, Meta};
use p3dp_gcode::template::{MAX_LINES, MAX_LINE_CHARS};
use p3dp_gcode::{check_program, read_motion, risky_commands, Outside, Risk, Violation};
use p3dp_profiles::PrinterProfile;
use p3dp_slicer::{SliceConfig, UserGcode};
use serde_json::{json, Value};

use crate::objects::{MAX_OBJECTS, SCALE_MAX, SCALE_MIN};

pub use eh3d_project::appbranch::{Identity, Listing, Row, EXTENSION};

/// The branch this program owns.
pub const BRANCH: &str = "3dp";
/// The format of the branch, `major.minor`.
pub const FORMAT: &str = "1.0";
/// The record of the branch that says which of its files is what.
pub const MANIFEST: &str = "3dp.json";
/// Where the parts of the bed and their placement are written.
pub const PLATE: &str = "plate/plate.json";
const PLATE_DIR: &str = "plate/";
const PROGRAM_DIR: &str = "programs/";
const SETTINGS: &str = "settings/slice.txt";
/// Where the printer profile of the person is kept, and the name of the record that says so.
const PROFILE: (&str, &str) = ("printer_profile", "profiles/printer.txt");
/// The largest printer profile that goes into a project or is read from one.
pub const MAX_PROFILE_BYTES: usize = 16 << 10;
/// Where the start and the end G-code of the person are kept, and the names of the record that say so.
const START_BLOCK: (&str, &str) = ("start_gcode", "blocks/start.gcode");
const END_BLOCK: (&str, &str) = ("end_gcode", "blocks/end.gcode");
/// The largest start or end G-code that goes into a project or is read from one.
pub const MAX_BLOCK_BYTES: usize = 256 << 10;
/// The farthest a part may be placed from the corner of the bed, in millimetres.
const MAX_PLACE_MM: f64 = 10_000.0;

const KIT: BranchKit<'static> = BranchKit::new(BRANCH, FORMAT, MANIFEST, env!("CARGO_PKG_VERSION"));

/// One part of the bed: the mesh as it was loaded and where the person put it.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub mesh: Mesh,
    /// The middle of the part on the bed; `None` for a part that was never placed (the window puts it in the middle).
    pub pos: Option<[f64; 2]>,
    pub rot_deg: [f64; 3],
    pub scale: [f64; 3],
}

impl Part {
    /// A part as it is loaded: not placed, not turned, at its own size.
    pub fn loaded(name: &str, mesh: Mesh) -> Part {
        Part {
            name: name.to_string(),
            mesh,
            pos: None,
            rot_deg: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// A G-code program on its way into the branch: the name of its file and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What [`save_contents`] puts into the branch: only what is there replaces what the branch holds.
#[derive(Debug, Default, Clone)]
pub struct Contents {
    pub parts: Option<Vec<Part>>,
    pub settings: Option<SliceConfig>,
    /// The id of the printer profile.
    pub printer: Option<String>,
    /// The id of the filament.
    pub filament: Option<String>,
    pub program: Option<Held>,
    /// The start G-code of the person; an empty text takes it out of the branch.
    pub start_gcode: Option<String>,
    /// The end G-code of the person; an empty text takes it out of the branch.
    pub end_gcode: Option<String>,
    /// The printer profile of the person, as the text of a profile file; an empty text takes it out of the branch.
    pub printer_profile: Option<String>,
}

impl Contents {
    /// Whether there is nothing to put.
    pub fn is_empty(&self) -> bool {
        self.parts.is_none()
            && self.settings.is_none()
            && self.printer.is_none()
            && self.filament.is_none()
            && self.program.is_none()
            && self.start_gcode.is_none()
            && self.end_gcode.is_none()
            && self.printer_profile.is_none()
    }
}

/// What the branch `3dp/` holds.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    pub parts: Vec<Part>,
    pub settings: Option<SliceConfig>,
    pub printer: Option<String>,
    pub filament: Option<String>,
    /// The G-code of the work, as a name inside the branch.
    pub program: Option<String>,
    /// The start and end G-code of the person. They are checked against the printer when a slice uses them, as the ones named by file are.
    pub blocks: UserGcode,
    /// The printer profile the project carries. It can only lower the limits of the same printer (see [`PrinterProfile::tightened_by`]).
    pub profile: Option<PrinterProfile>,
    /// The text of that profile, as it is kept.
    pub profile_text: Option<String>,
    /// The report of the last import.
    pub report: Option<ConversionReport>,
    /// What is out of date or wrong.
    pub warnings: Vec<String>,
    /// The files written out, when the branch was taken out.
    pub written: Option<usize>,
    /// The bytes of every file of the branch, by its name inside it.
    pub files: BTreeMap<String, Vec<u8>>,
}

fn describe(e: BranchError) -> String {
    e.to_string()
}

fn said(notices: Vec<Notice>) -> Vec<String> {
    notices.iter().map(ToString::to_string).collect()
}

fn base_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// Whether a name is the one of a model the program reads (a `.gcode.3mf` package holds a program, not a model).
pub fn is_model(name: &str) -> bool {
    !name.to_ascii_lowercase().ends_with(".gcode.3mf")
        && matches!(extension_of(name).as_str(), "stl" | "obj" | "3mf")
}

/// Whether a name is the one of a G-code program.
pub fn is_program(name: &str) -> bool {
    matches!(extension_of(name).as_str(), "gcode" | "gco" | "g")
}

/// Reads a model with the limits of the program; one that cannot be printed from is refused with the reason.
pub fn mesh_of(name: &str, bytes: &[u8], limits: &Limits) -> Result<Mesh, String> {
    let mesh =
        read_mesh(bytes, Some(base_name(name)), limits).map_err(|e| format!("{name}: {e}"))?;
    if let Some(e) = mesh.validate(limits).errors().next() {
        return Err(format!("{name}: {e}"));
    }
    Ok(mesh)
}

impl Opened {
    /// The name and the bytes of the G-code program of the work.
    pub fn program_file(&self) -> Option<(&str, &[u8])> {
        let rel = self.program.as_deref()?;
        self.files.get(rel).map(|b| (rel, b.as_slice()))
    }
}

/// What the temperatures of a program come to.
#[derive(Debug, Clone, PartialEq)]
pub enum Heat {
    /// The file is not text, so nothing of it was looked at.
    NotText,
    /// No printer with temperature limits was given.
    NoPrinter,
    /// Every temperature command that is known was compared with the limits of the printer; these go past them.
    Checked {
        printer: String,
        violations: Vec<Violation>,
    },
}

/// Where the moves of a program go, against the build volume of a printer.
#[derive(Debug, Clone, PartialEq)]
pub struct Moves {
    /// The first lines that go past the volume.
    pub outside: Vec<Outside>,
    pub outside_count: usize,
    /// Moves that could not be followed.
    pub unfollowed: usize,
}

/// What there is to say about a G-code program of a project. Nothing of it is run, changed or sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub name: String,
    pub lines: usize,
    pub heat: Heat,
    /// `None` when there is no printer to hold the moves to, or the file is not text.
    pub moves: Option<Moves>,
    /// The commands that act on the machine itself.
    pub risks: Vec<Risk>,
}

/// Counts the lines of a program, compares its temperatures with the limits of a printer and its moves with the build volume, and lists the commands that act on the machine.
pub fn review(name: &str, bytes: &[u8], printer: Option<&PrinterProfile>) -> Review {
    let text = String::from_utf8_lossy(bytes);
    let heat = if bytes.contains(&0) {
        Heat::NotText
    } else if let Some((printer, limits)) =
        printer.and_then(|p| p.limits.map(|l| (p.id.clone(), l)))
    {
        Heat::Checked {
            printer,
            violations: check_program(&text, &limits),
        }
    } else {
        Heat::NoPrinter
    };
    let is_text = !bytes.contains(&0);
    let volume = match &heat {
        Heat::Checked { .. } => printer.map(|p| p.volume_mm),
        _ => None,
    };
    let moves = volume.map(|v| {
        let m = read_motion(&text, Some(v));
        Moves {
            outside: m.outside,
            outside_count: m.outside_count,
            unfollowed: m.unfollowed,
        }
    });
    Review {
        name: name.to_string(),
        lines: text.lines().count(),
        heat,
        moves,
        risks: if is_text {
            risky_commands(&text)
        } else {
            Vec::new()
        },
    }
}

/// The profile of a printer file of the person: read whole and checked, or refused.
pub fn profile_of(text: &str) -> Result<PrinterProfile, String> {
    if text.len() > MAX_PROFILE_BYTES {
        return Err("the printer profile is too large".to_string());
    }
    let profile = PrinterProfile::from_text(text).map_err(|e| format!("printer profile: {e}"))?;
    profile
        .validate()
        .map_err(|e| format!("printer profile: {e}"))?;
    Ok(profile)
}

/// Refuses a start or end G-code that no slice would take: too large, too long or with a control character.
fn check_block_text(what: &str, text: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("{what} G-code: {why}"));
    if text.len() > MAX_BLOCK_BYTES {
        return bad("the file is too large");
    }
    if text.lines().count() > MAX_LINES {
        return bad("it has too many lines");
    }
    if text
        .lines()
        .any(|l| l.trim().chars().count() > MAX_LINE_CHARS)
    {
        return bad("a line is too long");
    }
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n'))
    {
        return bad("it has a control character");
    }
    Ok(())
}

/// One line of the record of the plate.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    file: String,
    name: String,
    pos: Option<[f64; 2]>,
    rot_deg: [f64; 3],
    scale: [f64; 3],
}

fn numbers<const N: usize>(v: &Value, lo: f64, hi: f64) -> Option<[f64; N]> {
    let list = v.as_array()?;
    let found: Vec<f64> = list
        .iter()
        .filter_map(Value::as_f64)
        .filter(|n| (lo..=hi).contains(n))
        .collect();
    if found.len() != list.len() {
        return None;
    }
    found.try_into().ok()
}

/// The record of the plate, every number checked against its range: nothing is cut down to fit.
fn entries_of(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let bad = |what: &str| format!("{BRANCH}/{PLATE}: {what}");
    let v: Value = serde_json::from_slice(bytes).map_err(|e| bad(&e.to_string()))?;
    let list = v["parts"]
        .as_array()
        .ok_or_else(|| bad("it has no parts"))?;
    if list.len() > MAX_OBJECTS {
        return Err(bad("it holds more parts than a bed can"));
    }
    list.iter()
        .map(|p| {
            let text = |key: &str| p[key].as_str().map(str::to_string);
            let pos = if p["pos"].is_null() {
                None
            } else {
                Some(
                    numbers::<2>(&p["pos"], -MAX_PLACE_MM, MAX_PLACE_MM)
                        .ok_or_else(|| bad("a position is not valid"))?,
                )
            };
            Ok(Entry {
                file: text("file").ok_or_else(|| bad("a part has no file"))?,
                name: text("name").ok_or_else(|| bad("a part has no name"))?,
                pos,
                rot_deg: numbers::<3>(&p["rot"], -360.0, 360.0)
                    .ok_or_else(|| bad("a turn is not valid"))?,
                scale: numbers::<3>(&p["scale"], SCALE_MIN, SCALE_MAX)
                    .ok_or_else(|| bad("a scale is not valid"))?,
            })
        })
        .collect()
}

fn entries_bytes(entries: &[Entry]) -> Vec<u8> {
    let parts: Vec<Value> = entries
        .iter()
        .map(|e| json!({"file": e.file, "name": e.name, "pos": e.pos, "rot": e.rot_deg, "scale": e.scale}))
        .collect();
    format!("{:#}\n", json!({ "parts": parts })).into_bytes()
}

fn entries_in(copy: &WorkingCopy) -> Vec<Entry> {
    copy.file(PLATE)
        .and_then(|b| entries_of(b).ok())
        .unwrap_or_default()
}

/// Writes the parts of the bed. A part whose mesh is the one of an imported or taken file refers to that file, so its report or its reference stays with it; the others are written as
/// packages of the program, one for each different mesh. A taken file that no part uses any more goes, with its reference.
fn write_plate(copy: &mut WorkingCopy, parts: &[Part], limits: &Limits) -> BranchResult<()> {
    let refused = BranchError::Refused;
    let before = entries_in(copy);
    let mut known: Vec<(String, Mesh)> = copy
        .files()
        .iter()
        .filter(|(rel, _)| rel.starts_with("origin/") || rel.starts_with(appbranch::TAKEN_DIR))
        .filter(|(rel, _)| is_model(rel))
        .filter_map(|(rel, bytes)| mesh_of(rel, bytes, limits).ok().map(|m| (rel.clone(), m)))
        .collect();
    let old: Vec<String> = copy
        .files()
        .keys()
        .filter(|n| n.starts_with(PLATE_DIR))
        .cloned()
        .collect();
    for rel in old {
        copy.remove(&rel);
    }
    let mut entries = Vec::with_capacity(parts.len());
    let mut written = 0;
    for part in parts {
        let found = known
            .iter()
            .find(|(_, m)| *m == part.mesh)
            .map(|(r, _)| r.clone());
        let file = match found {
            Some(rel) => rel,
            None => {
                written += 1;
                let rel = format!("{PLATE_DIR}part-{written:02}.3mf");
                let meta = Meta {
                    title: part.name.clone(),
                    printer: String::new(),
                };
                let bytes = threemf::write(&part.mesh, &meta)
                    .map_err(|e| refused(format!("{}: {e}", part.name)))?;
                copy.put(&rel, bytes)?;
                known.push((rel.clone(), part.mesh.clone()));
                rel
            }
        };
        entries.push(Entry {
            file,
            name: part.name.clone(),
            pos: part.pos,
            rot_deg: part.rot_deg,
            scale: part.scale,
        });
    }
    for gone in before {
        if gone.file.starts_with(appbranch::TAKEN_DIR)
            && entries.iter().all(|e| e.file != gone.file)
        {
            copy.remove(&gone.file);
        }
    }
    let record = entries_bytes(&entries);
    // what could not be read back is not written
    entries_of(&record).map_err(refused)?;
    copy.put(PLATE, record)?;
    copy.set("plate", PLATE);
    Ok(())
}

/// Takes the G-code of the work out of the record (the file of an import stays with its report).
fn drop_program(copy: &mut WorkingCopy) {
    if let Some(rel) = copy.text("program") {
        if rel.starts_with(PROGRAM_DIR) || rel.starts_with(appbranch::TAKEN_DIR) {
            copy.remove(&rel);
        }
        copy.set("program", "");
        copy.forget_missing(&["program"]);
    }
}

fn parts_of(files: &BTreeMap<String, Vec<u8>>, limits: &Limits) -> Result<Vec<Part>, String> {
    let Some(record) = files.get(PLATE) else {
        return Ok(Vec::new());
    };
    let mut read: BTreeMap<String, Mesh> = BTreeMap::new();
    entries_of(record)?
        .into_iter()
        .map(|e| {
            let mesh = match read.get(&e.file) {
                Some(m) => m.clone(),
                None => {
                    let bytes = files.get(&e.file).ok_or_else(|| {
                        format!("{BRANCH}/{}: the part is not in the branch", e.file)
                    })?;
                    let mesh = mesh_of(&e.file, bytes, limits)?;
                    read.insert(e.file.clone(), mesh.clone());
                    mesh
                }
            };
            Ok(Part {
                name: e.name,
                mesh,
                pos: e.pos,
                rot_deg: e.rot_deg,
                scale: e.scale,
            })
        })
        .collect()
}

fn opened_of(copy: &WorkingCopy, limits: &Limits) -> Result<Opened, String> {
    let files = copy.files().clone();
    let settings = match copy.text("settings").and_then(|rel| files.get(&rel)) {
        Some(bytes) => Some(
            SliceConfig::from_text(&String::from_utf8_lossy(bytes))
                .map_err(|e| format!("{BRANCH}/{SETTINGS}: {e}"))?,
        ),
        None => None,
    };
    let block = |(key, name): (&str, &str)| -> Result<Option<String>, String> {
        let Some(bytes) = copy.text(key).and_then(|rel| files.get(&rel)) else {
            return Ok(None);
        };
        let bad = |why: &str| format!("{BRANCH}/{name}: {why}");
        if bytes.len() > MAX_BLOCK_BYTES {
            return Err(bad("the file is too large"));
        }
        String::from_utf8(bytes.clone())
            .map(Some)
            .map_err(|_| bad("it is not text"))
    };
    let blocks = UserGcode {
        start: block(START_BLOCK)?,
        end: block(END_BLOCK)?,
    };
    // a profile that cannot be read does not close the project: it is told and left out
    let mut warnings = Vec::new();
    let mut profile = None;
    let mut profile_text = None;
    if let Some(bytes) = copy.text(PROFILE.0).and_then(|rel| files.get(&rel)) {
        let text = String::from_utf8_lossy(bytes).into_owned();
        match profile_of(&text) {
            Ok(found) => {
                profile = Some(found);
                profile_text = Some(text);
            }
            Err(e) => warnings.push(format!("{BRANCH}/{}: {e}", PROFILE.1)),
        }
    }
    Ok(Opened {
        profile,
        profile_text,
        blocks,
        parts: parts_of(&files, limits)?,
        settings,
        printer: copy.text("printer"),
        filament: copy.text("filament"),
        program: copy.text("program"),
        report: copy.report().map_err(describe)?,
        warnings,
        written: None,
        files,
    })
}

/// What the branch `3dp/` of the project at `path` holds; a project without it is refused.
pub fn held(path: &Path, limits: &Limits) -> Result<Opened, String> {
    let (_, copy, warnings) = KIT.open(path).map_err(describe)?;
    let mut opened = opened_of(&copy, limits)?;
    opened.warnings.splice(0..0, warnings);
    Ok(opened)
}

/// What the project at `path` holds in the branch now; nothing when there is no file or no branch.
fn held_or_nothing(path: &Path, limits: &Limits) -> Result<Opened, String> {
    if !path.exists() {
        return Ok(Opened::default());
    }
    let mut project = appbranch::open_project(path).map_err(describe)?;
    let copy = KIT.load(&mut project).map_err(describe)?;
    opened_of(&copy, limits)
}

/// Leaves out what the branch already holds, so a file taken from another branch stays where it is with its reference.
pub fn changes(mut contents: Contents, held: &Opened) -> Contents {
    if contents.parts.as_ref() == Some(&held.parts) {
        contents.parts = None;
    }
    if contents.settings.as_ref().map(SliceConfig::to_text)
        == held.settings.as_ref().map(SliceConfig::to_text)
    {
        contents.settings = None;
    }
    if contents.printer == held.printer {
        contents.printer = None;
    }
    if contents.filament == held.filament {
        contents.filament = None;
    }
    let same_program = contents.program.as_ref().is_some_and(|p| {
        held.program
            .as_ref()
            .and_then(|rel| held.files.get(rel))
            .is_some_and(|b| *b == p.bytes)
    });
    if same_program {
        contents.program = None;
    }
    if contents.start_gcode.as_deref() == Some(held.blocks.start.as_deref().unwrap_or_default()) {
        contents.start_gcode = None;
    }
    if contents.end_gcode.as_deref() == Some(held.blocks.end.as_deref().unwrap_or_default()) {
        contents.end_gcode = None;
    }
    if contents.printer_profile.as_deref() == Some(held.profile_text.as_deref().unwrap_or_default())
    {
        contents.printer_profile = None;
    }
    contents
}

/// Puts the work into the branch `3dp/` of the project at `path`, creating the project when it is not there. What is not given stays as it was, except that the G-code of a work whose
/// parts, settings, printer, filament or start and end G-code changed goes, because it is no longer the one of the work. Everything is checked before anything is written. The answer is `None` when
/// the branch holds all of it already, and then nothing is written.
pub fn save_contents(
    path: &Path,
    contents: &Contents,
    identity: &Identity,
    limits: &Limits,
) -> Result<Option<Vec<String>>, String> {
    if contents.is_empty() {
        return Err("there is nothing to save: give a model, settings, a printer, a filament or a G-code program".into());
    }
    for (what, text) in [
        ("start", &contents.start_gcode),
        ("end", &contents.end_gcode),
    ] {
        if let Some(text) = text {
            check_block_text(what, text)?;
        }
    }
    if let Some(text) = contents
        .printer_profile
        .as_deref()
        .filter(|t| !t.is_empty())
    {
        profile_of(text)?;
    }
    if contents
        .parts
        .as_ref()
        .is_some_and(|p| p.len() > MAX_OBJECTS)
    {
        return Err(format!("a bed holds at most {MAX_OBJECTS} parts"));
    }
    let new = changes(contents.clone(), &held_or_nothing(path, limits)?);
    if new.is_empty() {
        return Ok(None);
    }
    let program = match &new.program {
        Some(p) => {
            let rel = format!("{PROGRAM_DIR}{}", p.name);
            KIT.check_name(&rel).map_err(describe)?;
            Some((rel, p.bytes.clone()))
        }
        None => None,
    };
    let (_, notices) = KIT
        .update(path, identity, |_, copy| {
            let of_the_work = new.parts.is_some()
                || new.settings.is_some()
                || new.printer.is_some()
                || new.filament.is_some()
                || new.start_gcode.is_some()
                || new.end_gcode.is_some();
            for ((key, rel), text) in [(START_BLOCK, &new.start_gcode), (END_BLOCK, &new.end_gcode)]
            {
                let Some(text) = text else { continue };
                copy.remove(rel);
                if text.is_empty() {
                    copy.set(key, rel);
                    copy.forget_missing(&[key]);
                } else {
                    copy.put(rel, text.clone().into_bytes())?;
                    copy.set(key, rel);
                }
            }
            if let Some(text) = &new.printer_profile {
                let (key, rel) = PROFILE;
                copy.remove(rel);
                if text.is_empty() {
                    copy.set(key, rel);
                    copy.forget_missing(&[key]);
                } else {
                    copy.put(rel, text.clone().into_bytes())?;
                    copy.set(key, rel);
                }
            }
            if let Some(parts) = &new.parts {
                write_plate(copy, parts, limits)?;
            }
            if let Some(settings) = &new.settings {
                copy.put(SETTINGS, settings.to_text().into_bytes())?;
                copy.set("settings", SETTINGS);
            }
            if let Some(id) = &new.printer {
                copy.set("printer", id);
            }
            if let Some(id) = &new.filament {
                copy.set("filament", id);
            }
            match program {
                Some((rel, bytes)) => {
                    drop_program(copy);
                    copy.put(&rel, bytes)?;
                    copy.set("program", &rel);
                }
                None if of_the_work => drop_program(copy),
                None => {}
            }
            Ok(())
        })
        .map_err(describe)?;
    Ok(Some(said(notices)))
}

/// What an import of one file gives: the report and, for a model, its mesh.
fn convert(
    name: &str,
    bytes: &[u8],
    limits: &Limits,
) -> Result<(ConversionReport, Option<Mesh>), String> {
    let mut read = Vec::new();
    let mut not_read = Vec::new();
    let mut export_losses = Vec::new();
    let (format, mesh) = if is_model(name) {
        let mesh = mesh_of(name, bytes, limits)?;
        read.push(counted("triangles", mesh.triangles.len(), None));
        read.push(counted("vertices", mesh.vertices.len(), None));
        if extension_of(name) == "3mf" {
            if let Ok(info) = read_project(bytes, limits) {
                if !info.plates.is_empty() {
                    not_read.push(counted(
                        "plates",
                        info.plates.len(),
                        Some("the plates of other programs are not read"),
                    ));
                }
                if !info.settings.is_empty() {
                    not_read.push(counted(
                        "settings",
                        info.settings.len(),
                        Some("the settings of a project are read by the command import"),
                    ));
                }
            }
            export_losses.push(counted(
                "3mf",
                1,
                Some("the program writes the mesh; colours and settings stay in the original only"),
            ));
        }
        (extension_of(name), Some(mesh))
    } else if is_program(name) {
        let lines = String::from_utf8_lossy(bytes).lines().count();
        if lines == 0 {
            return Err(format!("{name}: it is empty"));
        }
        read.push(counted("lines", lines, None));
        ("gcode".to_string(), None)
    } else {
        return Err(format!(
            "{name}: it is not an STL, an OBJ, a 3MF model or a G-code program"
        ));
    };
    let report = appbranch::conversion_report(
        "phoenix-3dp",
        env!("CARGO_PKG_VERSION"),
        &format,
        name,
        bytes,
        Conversion {
            read,
            not_read,
            export_losses,
        },
    );
    Ok((report, mesh))
}

/// Imports a model or a G-code program of another program: the original is kept byte for byte under `3dp/origin/` with a report of the conversion. A model becomes the part of the bed;
/// a program becomes the G-code of the work and the parts stay.
pub fn import_file(
    path: &Path,
    source: &Path,
    identity: &Identity,
    limits: &Limits,
) -> Result<(ConversionReport, Vec<String>), String> {
    let name = file_name(source).map_err(describe)?;
    let bytes = read_bounded(source, limits.max_file_bytes).map_err(describe)?;
    let (report, mesh) = convert(&name, &bytes, limits)?;
    // the parts that referred to the original that is replaced are read before it goes
    let before = held_or_nothing(path, limits)?.parts;
    let notices = KIT
        .import(path, identity, bytes, &report, |copy, original| {
            match mesh {
                Some(mesh) => {
                    write_plate(copy, &[Part::loaded(&name, mesh)], limits)?;
                    drop_program(copy);
                }
                None => {
                    if !before.is_empty() {
                        write_plate(copy, &before, limits)?;
                    }
                    drop_program(copy);
                    copy.set("program", original);
                }
            }
            Ok(())
        })
        .map_err(describe)?;
    Ok((report, said(notices)))
}

/// What kind of thing a file taken from another branch is.
fn artifact_of(name: &str) -> &'static str {
    if is_program(name) {
        "toolpath"
    } else if is_model(name) || matches!(extension_of(name).as_str(), "step" | "stp") {
        "mesh3d"
    } else {
        "file"
    }
}

/// Copies a file of another branch into `3dp/taken/`, with a reference that remembers the state of its source. A model the program reads is added to the bed (or stays where the one
/// taken before under that name was) and a G-code program becomes the one of the work. A file of another kind is only kept.
pub fn take(
    path: &Path,
    branch: &str,
    rel: &str,
    limits: &Limits,
) -> Result<(String, Vec<String>), String> {
    let target = format!("{}{branch}/{rel}", appbranch::TAKEN_DIR);
    let before = held_or_nothing(path, limits)?;
    let listed = match before.files.get(PLATE) {
        Some(record) => entries_of(record)?,
        None => Vec::new(),
    };
    let full = listed.len() >= MAX_OBJECTS && listed.iter().all(|e| e.file != target);
    let check = |bytes: &[u8]| -> BranchResult<()> {
        if bytes.len() as u64 > limits.max_file_bytes {
            return Err(BranchError::Refused(format!(
                "{branch}/{rel}: the file is too large"
            )));
        }
        if is_model(rel) {
            if full {
                return Err(BranchError::Refused(format!(
                    "{branch}/{rel}: the bed holds all it can"
                )));
            }
            mesh_of(rel, bytes, limits).map_err(BranchError::Refused)?;
        }
        Ok(())
    };
    let artifact = artifact_of(rel);
    let mut failed = None;
    let (target, notices) = KIT
        .take(path, branch, rel, artifact, check, |copy, target| {
            if artifact == "toolpath" {
                drop_program(copy);
                copy.set("program", target);
            } else if is_model(target) {
                let mut entries = entries_in(copy);
                if entries.iter().all(|e| e.file != target) {
                    entries.push(Entry {
                        file: target.to_string(),
                        name: base_name(target).to_string(),
                        pos: None,
                        rot_deg: [0.0; 3],
                        scale: [1.0; 3],
                    });
                }
                match copy.put(PLATE, entries_bytes(&entries)) {
                    Ok(()) => copy.set("plate", PLATE),
                    Err(e) => failed = Some(e.to_string()),
                }
                drop_program(copy);
            }
        })
        .map_err(describe)?;
    match failed {
        Some(why) => Err(why),
        None => Ok((target, said(notices))),
    }
}

/// The branches of a project, with what is out of date.
pub fn list(path: &Path) -> Result<Listing, String> {
    appbranch::list(path).map_err(describe)
}

/// Opens the branch `3dp/`: reads and checks what it holds and, with `into`, writes its files there (a file that is there is never written over).
pub fn open(path: &Path, into: Option<&Path>, limits: &Limits) -> Result<Opened, String> {
    let mut opened = held(path, limits)?;
    if let Some(dir) = into {
        opened.written = Some(extract(path, BRANCH, dir)?);
    }
    Ok(opened)
}

/// Writes the files of any branch into a folder, byte for byte (a file that is there is never written over).
pub fn extract(path: &Path, branch: &str, dir: &Path) -> Result<usize, String> {
    appbranch::extract(path, branch, dir).map_err(describe)
}
