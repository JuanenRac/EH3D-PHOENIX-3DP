//! Reading every setting of a file that a person brings: a 3MF project of another program, or a profile saved as JSON or as `key = value` lines (INI). Nothing is interpreted
//! here: each key comes out with its values as text, and **everything that was not read is listed with the reason**, so nothing is lost in silence. The layouts were learned
//! from how such files look, never from the code of the programs that write them. Nothing is extracted to disk and nothing is written back.
//!
//! Limits against a file made to hurt: the package is opened with the limits of the program, a file of settings over [`MAX_METADATA_BYTES`] is not inflated, at most
//! [`MAX_PAIRS`] settings are read, keys and values are cut to [`MAX_KEY_CHARS`] and [`MAX_VALUE_CHARS`] (and the cut is told), and no control character comes out.

use crate::error::FormatError;
use crate::project3mf::{read_project, Producer, MAX_METADATA_BYTES};
use crate::zip::Archive;
use p3dp_core::json::{self, Value};
use p3dp_core::Limits;

/// The most settings read from one file.
pub const MAX_PAIRS: usize = 20_000;
/// The longest key kept.
pub const MAX_KEY_CHARS: usize = 120;
/// The longest value kept.
pub const MAX_VALUE_CHARS: usize = 2_000;
/// The most values of one setting kept (one per extruder).
pub const MAX_VALUES: usize = 64;

/// What kind of file the settings came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A 3MF package, with the program it seems to come from.
    Project(Producer),
    /// A JSON object.
    Json,
    /// Lines of `key = value`, with or without `[sections]`.
    Ini,
}

/// What happened to a value while it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairNote {
    /// The key, a value or the list of values was longer than the limit and was cut.
    Cut,
    /// The value is an object, a nested list or null: it is not read.
    Structured,
    /// The same key came before in the same part of the file; the first one counts.
    Repeated,
}

/// One setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The entry of the package or the `[section]` it is in; empty when there is none.
    pub part: String,
    pub key: String,
    /// The values as text: one, or one per extruder.
    pub values: Vec<String>,
    pub note: Option<PairNote>,
}

/// Why a part of the file was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// The shapes of the model: this reading is of settings.
    Geometry,
    /// Settings of single objects or plates.
    PerObject,
    /// G-code.
    Gcode,
    /// A picture.
    Picture,
    /// An entry this program does not know.
    Unknown,
    /// Larger than the limit of a file of settings.
    TooLarge,
    /// Damaged, not UTF-8 or not the layout expected.
    Damaged,
    /// A name that would climb out of a folder, absolute or with a backslash.
    UnsafeName,
    /// A name that appears twice in the package.
    Twice,
    /// A line with no `=`.
    BadLine,
}

/// Everything that was found.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsFile {
    pub kind: SourceKind,
    pub pairs: Vec<Pair>,
    /// The parts that were read.
    pub read: Vec<String>,
    /// The parts that were not, each with the reason.
    pub unread: Vec<(String, Unread)>,
}

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("settings", reason)
}

/// The text with no control character, cut to a length; the flag says whether it was cut.
fn plain(s: &str, max: usize) -> (String, bool) {
    let mut chars = s.chars().filter(|c| !c.is_control());
    let kept: String = chars.by_ref().take(max).collect();
    let cut = chars.next().is_some();
    (kept.trim().to_string(), cut)
}

struct Builder {
    pairs: Vec<Pair>,
    seen: std::collections::HashSet<(String, String)>,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            pairs: Vec::new(),
            seen: std::collections::HashSet::new(),
        }
    }

    fn push(
        &mut self,
        part: &str,
        key: &str,
        values: &[String],
        note: Option<PairNote>,
    ) -> Result<(), FormatError> {
        if self.pairs.len() >= MAX_PAIRS {
            return Err(bad(format!("more than {MAX_PAIRS} settings")));
        }
        let (key, key_cut) = plain(key, MAX_KEY_CHARS);
        let mut cut = key_cut || values.len() > MAX_VALUES;
        let values: Vec<String> = values
            .iter()
            .take(MAX_VALUES)
            .map(|v| {
                let (v, was_cut) = plain(v, MAX_VALUE_CHARS);
                cut |= was_cut;
                v
            })
            .collect();
        let part = plain(part, MAX_KEY_CHARS).0;
        let repeated = !self.seen.insert((part.clone(), key.clone()));
        let note = if repeated {
            Some(PairNote::Repeated)
        } else if cut {
            Some(PairNote::Cut)
        } else {
            note
        };
        self.pairs.push(Pair {
            part,
            key,
            values,
            note,
        });
        Ok(())
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.clone()),
        Value::Num(n) if n.is_finite() => Some(format!("{n}")),
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
        _ => None,
    }
}

fn json_into(text: &str, part: &str, out: &mut Builder) -> Result<(), FormatError> {
    let v = json::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|e| bad(format!("not JSON: {e}")))?;
    let Value::Obj(members) = v else {
        return Err(bad("the JSON file is not an object"));
    };
    for (key, value) in &members {
        let values: Option<Vec<String>> = match value {
            Value::Arr(a) => a.iter().map(scalar).collect(),
            other => scalar(other).map(|s| vec![s]),
        };
        match values {
            Some(values) => out.push(part, key, &values, None)?,
            None => out.push(part, key, &[], Some(PairNote::Structured))?,
        }
    }
    Ok(())
}

/// The values of one line: several, one per extruder, are separated by `,` or `;`. A value in quotes is one.
fn split_values(value: &str) -> Vec<String> {
    let v = value.trim();
    if let Some(quoted) = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return vec![quoted.to_string()];
    }
    v.split([',', ';'])
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect()
}

/// Lines of `key = value`. In the file of a package every line starts with `;`, which is taken off (`commented`); in a profile a line that starts with `;` or `#` is a comment.
fn lines_into(
    text: &str,
    part: &str,
    commented: bool,
    out: &mut Builder,
    unread: &mut Vec<(String, Unread)>,
) -> Result<(), FormatError> {
    let mut section = String::new();
    for (i, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let mut line = raw.trim();
        if commented {
            line = line.trim_start_matches(';').trim();
        } else if line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let heading = line.strip_prefix('[').and_then(|r| r.strip_suffix(']'));
        if let (false, Some(name)) = (commented, heading) {
            section = plain(name, MAX_KEY_CHARS).0;
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            // the heading that a program writes at the top of its file is not a setting
            if !commented && unread.len() < MAX_PAIRS {
                unread.push((format!("line {}", i + 1), Unread::BadLine));
            }
            continue;
        };
        let k = k.trim();
        if k.is_empty() {
            unread.push((format!("line {}", i + 1), Unread::BadLine));
            continue;
        }
        let here = if section.is_empty() { part } else { &section };
        out.push(here, k, &split_values(v), None)?;
    }
    Ok(())
}

fn unsafe_name(name: &str) -> bool {
    name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(char::is_control)
        || name.split('/').any(|p| p == "..")
}

/// The preset files a project may carry beside its settings: `Metadata/<process|filament|machine>_settings_<number>.config`.
fn is_preset_entry(lower: &str) -> bool {
    let Some(rest) = lower
        .strip_prefix("metadata/")
        .and_then(|r| r.strip_suffix(".config"))
    else {
        return false;
    };
    [
        "process_settings_",
        "filament_settings_",
        "machine_settings_",
    ]
    .iter()
    .any(|p| {
        rest.strip_prefix(p)
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

#[derive(Clone, Copy)]
enum Layout {
    Json,
    Commented,
}

fn layout_of(lower: &str) -> Result<Layout, Unread> {
    if lower == "metadata/project_settings.config" || is_preset_entry(lower) {
        Ok(Layout::Json)
    } else if lower == "metadata/slic3r_pe.config" {
        Ok(Layout::Commented)
    } else if lower.ends_with("model_settings.config") || lower.ends_with("slic3r_pe_model.config")
    {
        Err(Unread::PerObject)
    } else if lower.ends_with(".model") {
        Err(Unread::Geometry)
    } else if lower.ends_with(".gcode") || lower.ends_with(".bgcode") {
        Err(Unread::Gcode)
    } else if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Err(Unread::Picture)
    } else {
        Err(Unread::Unknown)
    }
}

fn read_package(bytes: &[u8], limits: &Limits) -> Result<SettingsFile, FormatError> {
    // the package has to be a project: a ZIP file of anything else is refused
    let producer = read_project(bytes, limits)
        .map(|p| p.producer)
        .unwrap_or(Producer::Plain);
    let archive = Archive::open(bytes, limits)?;
    if archive.find("3D/3dmodel.model").is_none() {
        return Err(bad("the package has no 3D model: it is not a 3MF project"));
    }
    let mut out = Builder::new();
    let mut read = Vec::new();
    let mut unread = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut budget = limits.max_inflated_bytes;
    for entry in &archive.entries {
        if entry.name.ends_with('/') {
            continue;
        }
        let shown = plain(&entry.name, MAX_KEY_CHARS).0;
        if unsafe_name(&entry.name) {
            unread.push((shown, Unread::UnsafeName));
            continue;
        }
        let lower = entry.name.to_ascii_lowercase();
        if !names.insert(lower.clone()) {
            unread.push((shown, Unread::Twice));
            continue;
        }
        let layout = match layout_of(&lower) {
            Ok(l) => l,
            Err(why) => {
                unread.push((shown, why));
                continue;
            }
        };
        // all the files of settings together are held to the limit of one entry, so many large ones cannot add up
        if entry.size() > MAX_METADATA_BYTES || entry.size() > budget {
            unread.push((shown, Unread::TooLarge));
            continue;
        }
        budget -= entry.size();
        let Some(text) = archive
            .read(entry)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
        else {
            unread.push((shown, Unread::Damaged));
            continue;
        };
        let before = out.pairs.len();
        let done = match layout {
            Layout::Json => json_into(&text, &shown, &mut out),
            Layout::Commented => lines_into(&text, &shown, true, &mut out, &mut unread),
        };
        match done {
            Ok(()) => read.push(shown),
            Err(e) if out.pairs.len() >= MAX_PAIRS => return Err(e),
            Err(_) => {
                out.pairs.truncate(before);
                unread.push((shown, Unread::Damaged));
            }
        }
    }
    Ok(SettingsFile {
        kind: SourceKind::Project(producer),
        pairs: out.pairs,
        read,
        unread,
    })
}

/// Reads the settings of a file. What it is, is told by its first bytes, not by its name. A text that has no setting at all is an error.
pub fn read_settings(bytes: &[u8], limits: &Limits) -> Result<SettingsFile, FormatError> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return read_package(bytes, limits);
    }
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(FormatError::TooLarge {
            bytes: bytes.len() as u64,
            max: MAX_METADATA_BYTES,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| bad("the file is not UTF-8 text"))?;
    if text.contains('\0') {
        return Err(bad("the file is not text"));
    }
    let mut out = Builder::new();
    let mut unread = Vec::new();
    let kind = if text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('{')
    {
        json_into(text, "", &mut out)?;
        SourceKind::Json
    } else {
        lines_into(text, "", false, &mut out, &mut unread)?;
        SourceKind::Ini
    };
    if out.pairs.is_empty() {
        return Err(bad("no setting was found"));
    }
    Ok(SettingsFile {
        kind,
        pairs: out.pairs,
        read: Vec::new(),
        unread,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::Writer;

    const MODEL: &str =
        "<?xml version=\"1.0\"?><model unit=\"millimeter\"><resources/><build/></model>";

    fn pack(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = Writer::new();
        z.add("3D/3dmodel.model", MODEL.as_bytes()).unwrap();
        for (n, d) in entries {
            z.add(n, d).unwrap();
        }
        z.finish()
    }

    fn read(bytes: &[u8]) -> SettingsFile {
        read_settings(bytes, &Limits::default()).unwrap()
    }

    fn pair<'a>(f: &'a SettingsFile, key: &str) -> &'a Pair {
        f.pairs.iter().find(|p| p.key == key).unwrap()
    }

    fn reason(f: &SettingsFile, name: &str) -> Unread {
        f.unread.iter().find(|(n, _)| n == name).unwrap().1
    }

    #[test]
    fn every_key_of_a_json_profile_comes_out() {
        let f = read(br#"{"name":"mine","layer_height":"0.2","nozzle_temperature":["210","215"],"flag":true,"n":3,"nested":{"a":1},"deep":[[1]],"nothing":null}"#);
        assert_eq!(f.kind, SourceKind::Json);
        assert_eq!(f.pairs.len(), 8);
        assert_eq!(pair(&f, "nozzle_temperature").values, ["210", "215"]);
        assert_eq!(pair(&f, "flag").values, ["1"]);
        assert_eq!(pair(&f, "n").values, ["3"]);
        for key in ["nested", "deep", "nothing"] {
            assert_eq!(pair(&f, key).note, Some(PairNote::Structured), "{key}");
            assert!(pair(&f, key).values.is_empty());
        }
    }

    #[test]
    fn the_lines_of_an_ini_keep_their_sections_and_what_is_not_a_setting_is_told() {
        let f = read(b"\xef\xbb\xbf# made by a person\r\n[print:fine]\r\nlayer_height = 0.1\r\nlayer_height = 0.3\r\nfilament_type = PLA;PETG\r\nnotes = \"a, b\"\r\nnot a setting\r\n = 1\r\n");
        assert_eq!(f.kind, SourceKind::Ini);
        assert_eq!(f.pairs[0].part, "print:fine");
        assert_eq!(f.pairs[0].values, ["0.1"]);
        assert_eq!(f.pairs[1].note, Some(PairNote::Repeated));
        assert_eq!(pair(&f, "filament_type").values, ["PLA", "PETG"]);
        assert_eq!(pair(&f, "notes").values, ["a, b"]);
        assert_eq!(f.unread.len(), 2);
        assert!(f.unread.iter().all(|u| u.1 == Unread::BadLine));
    }

    #[test]
    fn a_project_reads_its_settings_and_lists_everything_else() {
        let z = pack(&[
            (
                "Metadata/project_settings.config",
                br#"{"layer_height":"0.16"}"#,
            ),
            (
                "Metadata/filament_settings_1.config",
                br#"{"nozzle_temperature":["220"]}"#,
            ),
            ("Metadata/model_settings.config", b"<config/>"),
            ("Metadata/plate_1.png", b"png"),
            ("Metadata/plate_1.gcode", b"G28"),
            ("Metadata/other.txt", b"x"),
        ]);
        let f = read(&z);
        assert!(matches!(f.kind, SourceKind::Project(_)));
        assert_eq!(f.read.len(), 2);
        assert_eq!(f.pairs.len(), 2);
        assert_eq!(reason(&f, "3D/3dmodel.model"), Unread::Geometry);
        assert_eq!(
            reason(&f, "Metadata/model_settings.config"),
            Unread::PerObject
        );
        assert_eq!(reason(&f, "Metadata/plate_1.png"), Unread::Picture);
        assert_eq!(reason(&f, "Metadata/plate_1.gcode"), Unread::Gcode);
        assert_eq!(reason(&f, "Metadata/other.txt"), Unread::Unknown);
        assert_eq!(f.read.len() + f.unread.len(), 7);
    }

    #[test]
    fn the_lines_of_a_project_start_with_a_semicolon() {
        let z = pack(&[(
            "Metadata/Slic3r_PE.config",
            b"; generated by a slicer\n\n; layer_height = 0.25\n; temperature = 215,220\n",
        )]);
        let f = read(&z);
        assert_eq!(pair(&f, "layer_height").values, ["0.25"]);
        assert_eq!(pair(&f, "temperature").values, ["215", "220"]);
        assert!(f.unread.iter().all(|u| u.1 != Unread::BadLine));
    }

    #[test]
    fn a_damaged_entry_is_told_and_does_not_hide_the_others() {
        let z = pack(&[
            ("Metadata/project_settings.config", b"{\"a\":\"1\",broken"),
            ("Metadata/process_settings_1.config", br#"{"b":"2"}"#),
            ("Metadata/machine_settings_2.config", b"\xff\xfe"),
        ]);
        let f = read(&z);
        assert_eq!(f.pairs.len(), 1);
        assert_eq!(
            reason(&f, "Metadata/project_settings.config"),
            Unread::Damaged
        );
        assert_eq!(
            reason(&f, "Metadata/machine_settings_2.config"),
            Unread::Damaged
        );
    }

    #[test]
    fn a_file_of_settings_that_inflates_too_far_is_not_read() {
        let big = vec![b' '; (MAX_METADATA_BYTES + 1) as usize];
        let z = pack(&[("Metadata/project_settings.config", &big)]);
        let f = read(&z);
        assert_eq!(
            reason(&f, "Metadata/project_settings.config"),
            Unread::TooLarge
        );
        assert!(f.pairs.is_empty());
        let e = read_settings(&big, &Limits::default()).unwrap_err();
        assert!(matches!(e, FormatError::TooLarge { .. }));
    }

    #[test]
    fn a_bomb_and_too_many_entries_are_refused_by_the_limits_of_the_package() {
        let z = pack(&[("Metadata/a.txt", b"x"), ("Metadata/b.txt", b"y")]);
        let few = Limits {
            max_zip_entries: 2,
            ..Limits::default()
        };
        assert!(read_settings(&z, &few).is_err());
        let small = Limits {
            max_inflated_bytes: 4,
            ..Limits::default()
        };
        assert!(read_settings(&z, &small).is_err());
    }

    #[test]
    fn names_that_climb_out_are_never_read() {
        // the writer of the program refuses such names, so one is made by changing the bytes of a safe one
        let mut z = pack(&[("Metadata/xx/project_settings.config", br#"{"a":"1"}"#)]);
        let from = b"Metadata/xx/";
        let to = b"Metadata/../";
        let mut changed = 0;
        for i in 0..z.len() - from.len() {
            if &z[i..i + from.len()] == from {
                z[i..i + from.len()].copy_from_slice(to);
                changed += 1;
            }
        }
        assert_eq!(changed, 2);
        let f = read(&z);
        assert!(f.pairs.is_empty());
        assert_eq!(
            reason(&f, "Metadata/../project_settings.config"),
            Unread::UnsafeName
        );
        for name in ["/a", "a\\b", "c:/a", "a/../b", "a\nb"] {
            assert!(unsafe_name(name), "{name:?}");
        }
        assert!(!unsafe_name("Metadata/a..b.config"));
    }

    #[test]
    fn long_keys_long_values_and_control_characters_are_cut_and_said_so() {
        let long = "v".repeat(MAX_VALUE_CHARS + 5);
        let text = format!(
            "{{\"{}\":\"1\",\"b\":\"{long}\",\"c\":\"x\\ny\\u0000z\"}}",
            "k".repeat(MAX_KEY_CHARS + 1)
        );
        let f = read(text.as_bytes());
        assert_eq!(f.pairs[0].key.len(), MAX_KEY_CHARS);
        assert_eq!(f.pairs[0].note, Some(PairNote::Cut));
        assert_eq!(f.pairs[1].values[0].len(), MAX_VALUE_CHARS);
        assert_eq!(f.pairs[1].note, Some(PairNote::Cut));
        assert_eq!(f.pairs[2].values, ["xyz"]);
        let many = format!("{{\"a\":[{}]}}", vec!["1"; MAX_VALUES + 1].join(","));
        let f = read(many.as_bytes());
        assert_eq!(f.pairs[0].values.len(), MAX_VALUES);
        assert_eq!(f.pairs[0].note, Some(PairNote::Cut));
    }

    #[test]
    fn hostile_text_is_an_error_and_never_a_panic() {
        let too_many: String = (0..=MAX_PAIRS).map(|i| format!("k{i} = 1\n")).collect();
        let deep = format!("{{\"a\":{}", "[".repeat(10_000));
        let cases: [&[u8]; 12] = [
            b"",
            b"{",
            b"{}",
            b"[1,2]",
            b"{\"a\":NaN}",
            b"{\"a\":1e999}",
            b"no setting here",
            b"a = 1\0",
            b"\xff\xfea = 1",
            b"PK\x03\x04 cut short",
            deep.as_bytes(),
            too_many.as_bytes(),
        ];
        for bytes in cases {
            assert!(
                read_settings(bytes, &Limits::default()).is_err(),
                "{:?}",
                String::from_utf8_lossy(&bytes[..bytes.len().min(20)])
            );
        }
    }

    #[test]
    fn a_package_with_no_model_is_not_a_project() {
        let mut z = Writer::new();
        z.add("Metadata/project_settings.config", b"{}").unwrap();
        assert!(read_settings(&z.finish(), &Limits::default()).is_err());
    }

    #[test]
    fn no_damage_to_a_project_panics() {
        let z = pack(&[("Metadata/project_settings.config", br#"{"a":"1"}"#)]);
        for i in 0..z.len() {
            let mut d = z.clone();
            d[i] ^= 0x5a;
            let _ = read_settings(&d, &Limits::default());
            let _ = read_settings(&z[..i], &Limits::default());
        }
    }
}
