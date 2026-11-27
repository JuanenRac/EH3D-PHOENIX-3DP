//! The command `phoenix-3dp project list|save|import|open|take|extract FILE.eh3d ...`: the words after `project` for a project of the suite, apart from the window so that it can be
//! tried without one. Nothing here is sent to a printer.

use std::path::{Path, PathBuf};

use eh3d_project::appbranch::read_bounded;
use p3dp_core::Limits;
use p3dp_gcode::read_motion;
use p3dp_profiles::filament::FilamentCatalog;
use p3dp_profiles::{suite_overlay, Bound, PrinterProfile};
use p3dp_slicer::SliceConfig;

use crate::eh3dfile::{self, Contents, Heat, Held, Identity, Part, BRANCH};

/// The words that make `project` work on a project of the suite and not read a file of another program.
pub const WORDS: [&str; 7] = [
    "list", "save", "import", "open", "take", "extract", "layers",
];

/// The largest settings file that is read.
const MAX_SETTINGS_BYTES: u64 = 1 << 20;
/// The most lines past the limits that are listed one by one.
const MAX_LISTED: usize = 20;

/// What the words gave: what goes to the standard output and error, and the exit code (0 clean, 1 something to read, 2 error).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Done {
    pub out: String,
    pub err: String,
    pub code: i32,
}

fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(i) = args.iter().position(|a| a == name) else {
        return Ok(None);
    };
    let value = args
        .get(i + 1)
        .ok_or_else(|| format!("{name} needs a value"))?
        .clone();
    args.drain(i..=i + 1);
    Ok(Some(value))
}

fn words<const N: usize>(args: &[String], what: &str) -> Result<[String; N], String> {
    if let Some(option) = args.iter().find(|a| a.starts_with("--")) {
        return Err(format!("{option}: unknown option"));
    }
    <[String; N]>::try_from(args.to_vec()).map_err(|_| format!("project {what}"))
}

fn identity(args: &mut Vec<String>) -> Result<Identity, String> {
    Ok(Identity {
        name: flag(args, "--name")?,
        id: flag(args, "--id")?,
    })
}

fn notes(done: &mut Done, list: &[String]) {
    for note in list {
        done.err.push_str(&format!("note: {note}\n"));
    }
}

fn printer_of(id: &str) -> Result<PrinterProfile, String> {
    let catalog = suite_overlay().map_err(|e| e.to_string())?.catalog;
    catalog
        .find(id)
        .cloned()
        .ok_or_else(|| format!("{id}: no such printer"))
}

fn bounds_text(list: &[Bound]) -> String {
    list.iter()
        .map(|b| format!("{} {} (yours {})", b.key, b.other, b.own))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Says what the printer profile of a project does to the one of the person, and gives the printer to check against: the one of the person, lowered where the project's is
/// lower. A profile of a printer that is not the one checked against changes nothing.
fn with_profile(
    done: &mut Done,
    own: PrinterProfile,
    theirs: Option<&PrinterProfile>,
) -> PrinterProfile {
    let Some(theirs) = theirs.filter(|t| t.id == own.id) else {
        return own;
    };
    let t = own.tightened_by(theirs);
    if !t.lowered.is_empty() {
        done.out.push_str(&format!(
            "the profile of the project lowers: {}\n",
            bounds_text(&t.lowered)
        ));
    }
    if !t.refused.is_empty() {
        done.err.push_str(&format!(
            "note: the profile of the project asks for more than yours and is not followed there: {}\n",
            bounds_text(&t.refused)
        ));
    }
    t.profile
}

/// Compares the temperatures and the moves of the G-code of the work with the limits and the volume of the printer that is asked for or, without one, of the printer of the
/// project, and lists the commands that act on the machine. Anything found is something to read: the exit code is 1. Nothing is changed.
fn check_program_of(
    done: &mut Done,
    file: &str,
    asked: Option<&PrinterProfile>,
    limits: &Limits,
) -> Result<(), String> {
    let opened = eh3dfile::held(Path::new(file), limits)?;
    let Some((name, bytes)) = opened.program_file() else {
        return Ok(());
    };
    let own = opened.printer.as_deref().and_then(|id| printer_of(id).ok());
    let printer = asked
        .cloned()
        .or(own)
        .map(|p| with_profile(done, p, opened.profile.as_ref()));
    let review = eh3dfile::review(name, bytes, printer.as_ref());
    done.out
        .push_str(&format!("program {name}: {} line(s)\n", review.lines));
    match review.heat {
        Heat::NotText => done
            .out
            .push_str("temperatures not checked: the program is not a text file\n"),
        Heat::NoPrinter => done.out.push_str(
            "temperatures not checked: no printer with temperature limits is known (use --printer ID)\n",
        ),
        Heat::Checked {
            printer,
            violations,
        } if violations.is_empty() => done.out.push_str(&format!(
            "no temperature command goes past the limits of {printer}\n"
        )),
        Heat::Checked {
            printer,
            violations,
        } => {
            for v in violations.iter().take(MAX_LISTED) {
                done.out
                    .push_str(&format!("line {}: {} ({})\n", v.line, v.error, v.command));
            }
            if violations.len() > MAX_LISTED {
                done.out
                    .push_str(&format!("and {} more\n", violations.len() - MAX_LISTED));
            }
            done.err.push_str(&format!(
                "{} line(s) of the program go past the limits of {printer}\n",
                violations.len()
            ));
            done.code = 1;
        }
    }
    if let Some(moves) = &review.moves {
        if moves.outside_count == 0 {
            done.out.push_str("no move goes past the build volume\n");
        } else {
            for o in moves.outside.iter().take(MAX_LISTED) {
                done.out.push_str(&format!(
                    "line {}: {}{:.3} is outside the build volume\n",
                    o.line, o.axis, o.value_mm
                ));
            }
            done.err.push_str(&format!(
                "{} line(s) of the program leave the build volume\n",
                moves.outside_count
            ));
            done.code = 1;
        }
        if moves.unfollowed > 0 {
            done.out.push_str(&format!(
                "{} move(s) could not be followed and are not checked\n",
                moves.unfollowed
            ));
        }
    }
    if !review.risks.is_empty() {
        for r in review.risks.iter().take(MAX_LISTED) {
            done.out.push_str(&format!(
                "line {}: {} ({})\n",
                r.line,
                r.kind.name(),
                r.command
            ));
        }
        done.err.push_str(&format!(
            "{} line(s) of the program act on the machine itself\n",
            review.risks.len()
        ));
        done.code = 1;
    }
    Ok(())
}

/// The layers of the G-code of the work: their height, the lines that lay plastic and the filament they take; with a number, one layer only.
fn layers_of(
    done: &mut Done,
    file: &str,
    only: Option<usize>,
    asked: Option<&PrinterProfile>,
    limits: &Limits,
) -> Result<(), String> {
    let opened = eh3dfile::held(Path::new(file), limits)?;
    let Some((name, bytes)) = opened.program_file() else {
        return Err(format!("{file}: the project has no G-code"));
    };
    if bytes.contains(&0) {
        return Err(format!("{name}: it is not a text file"));
    }
    let own = asked
        .cloned()
        .or_else(|| opened.printer.as_deref().and_then(|id| printer_of(id).ok()))
        .map(|p| with_profile(done, p, opened.profile.as_ref()));
    let motion = read_motion(&String::from_utf8_lossy(bytes), own.map(|p| p.volume_mm));
    if let Some(n) = only {
        if n == 0 || n > motion.layers.len() {
            return Err(format!("--layer is from 1 to {}", motion.layers.len()));
        }
    }
    done.out.push_str(&format!(
        "program {name}: {} layer(s), {:.1} mm of filament\n",
        motion.layers.len(),
        motion.filament_mm
    ));
    for (i, l) in motion.layers.iter().enumerate() {
        if only.is_some_and(|n| n != i + 1) {
            continue;
        }
        done.out.push_str(&format!(
            "layer {}: z {:.3} mm, {} line(s) of {:.1} mm, {} travel(s), {:.2} mm of filament\n",
            i + 1,
            l.z_mm,
            l.extrusions.len(),
            l.length_mm,
            l.travels.len(),
            l.filament_mm
        ));
    }
    if motion.outside_count > 0 {
        done.err.push_str(&format!(
            "{} line(s) of the program leave the build volume\n",
            motion.outside_count
        ));
        done.code = 1;
    }
    if motion.unfollowed > 0 {
        done.out.push_str(&format!(
            "{} move(s) could not be followed\n",
            motion.unfollowed
        ));
    }
    Ok(())
}

/// What `save` puts into the branch, every file read and checked before anything is written.
fn contents_of(args: &mut Vec<String>, limits: &Limits) -> Result<Contents, String> {
    let model = flag(args, "--model")?;
    let settings = flag(args, "--settings")?;
    let printer = flag(args, "--printer")?;
    let filament = flag(args, "--filament")?;
    let program = flag(args, "--gcode")?;
    let start = flag(args, "--start-gcode")?;
    let end = flag(args, "--end-gcode")?;
    let profile = flag(args, "--printer-file")?;
    let read =
        |file: &str, limit: u64| read_bounded(Path::new(file), limit).map_err(|e| e.to_string());
    let name_of =
        |file: &str| eh3d_project::appbranch::file_name(Path::new(file)).map_err(|e| e.to_string());
    let mut contents = Contents::default();
    if let Some(file) = &model {
        let name = name_of(file)?;
        if !eh3dfile::is_model(&name) {
            return Err(format!(
                "{file}: --model takes an STL, an OBJ or a 3MF model"
            ));
        }
        let mesh = eh3dfile::mesh_of(&name, &read(file, limits.max_file_bytes)?, limits)?;
        contents.parts = Some(vec![Part::loaded(&name, mesh)]);
    }
    if let Some(file) = &settings {
        // only the keys of the slicer are taken, so nothing else of the person (such as the token of the server) goes into a project
        let text = String::from_utf8_lossy(&read(file, MAX_SETTINGS_BYTES)?).into_owned();
        contents.settings =
            Some(SliceConfig::from_text(&text).map_err(|e| format!("{file}: {e}"))?);
    }
    if let Some(id) = printer {
        contents.printer = Some(printer_of(&id)?.id);
    }
    if let Some(id) = filament {
        let catalog = FilamentCatalog::builtin().map_err(|e| e.to_string())?;
        let found = catalog
            .find(&id)
            .ok_or_else(|| format!("{id}: no such filament"))?;
        contents.filament = Some(found.id.clone());
    }
    if let Some(file) = &program {
        let name = name_of(file)?;
        if !eh3dfile::is_program(&name) {
            return Err(format!("{file}: --gcode takes a .gcode program"));
        }
        contents.program = Some(Held {
            name,
            bytes: read(file, limits.max_file_bytes)?,
        });
    }
    for (file, slot) in [
        (&start, &mut contents.start_gcode),
        (&end, &mut contents.end_gcode),
    ] {
        if let Some(file) = file {
            let bytes = read(file, eh3dfile::MAX_BLOCK_BYTES as u64)?;
            *slot = Some(String::from_utf8(bytes).map_err(|_| format!("{file}: it is not text"))?);
        }
    }
    if let Some(file) = &profile {
        let bytes = read(file, eh3dfile::MAX_PROFILE_BYTES as u64)?;
        let text = String::from_utf8(bytes).map_err(|_| format!("{file}: it is not text"))?;
        // an empty file takes the profile out
        if !text.is_empty() {
            let id = eh3dfile::profile_of(&text)
                .map_err(|e| format!("{file}: {e}"))?
                .id;
            contents.printer.get_or_insert(id);
        }
        contents.printer_profile = Some(text);
    }
    Ok(contents)
}

fn job(mut args: Vec<String>, limits: &Limits) -> Result<Done, String> {
    if args.is_empty() {
        return Err("project needs list, save, import, open, take, extract or layers".into());
    }
    let what = args.remove(0);
    let mut done = Done::default();
    match what.as_str() {
        "list" => {
            let json = match args.iter().position(|a| a == "--json") {
                Some(i) => {
                    args.remove(i);
                    true
                }
                None => false,
            };
            let [file] = words::<1>(&args, "list needs FILE.eh3d")?;
            let listing = eh3dfile::list(Path::new(&file))?;
            if json {
                let rows: Vec<serde_json::Value> = listing
                    .rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "id": r.id, "app": r.app, "format": r.format, "app_version": r.app_version,
                            "files": r.files, "hash": r.hash, "notes": r.notes,
                        })
                    })
                    .collect();
                let document = serde_json::json!({
                    "name": listing.name, "id": listing.id, "branches": rows, "warnings": listing.warnings,
                });
                done.out = format!("{document:#}\n");
            } else {
                done.out
                    .push_str(&format!("project {} ({})\n", listing.name, listing.id));
                for r in &listing.rows {
                    done.out.push_str(&format!(
                        "  {:<16} {:>4} files  format {:<6} {} {}  {}\n",
                        r.id,
                        r.files,
                        r.format,
                        r.app,
                        r.app_version,
                        r.notes.join(", ")
                    ));
                }
                done.out
                    .push_str(&format!("{} branch(es)\n", listing.rows.len()));
                notes(&mut done, &listing.warnings);
            }
            done.code = i32::from(!listing.warnings.is_empty());
        }
        "save" => {
            let who = identity(&mut args)?;
            let contents = contents_of(&mut args, limits)?;
            let [file] = words::<1>(&args, "save needs FILE.eh3d")?;
            match eh3dfile::save_contents(Path::new(&file), &contents, &who, limits)? {
                Some(said) => {
                    notes(&mut done, &said);
                    done.out = format!("the branch {BRANCH} of {file} was saved\n");
                }
                None => done.out = format!("nothing has changed: {file} was not written\n"),
            }
        }
        "import" => {
            let who = identity(&mut args)?;
            let asked = flag(&mut args, "--printer")?
                .map(|id| printer_of(&id))
                .transpose()?;
            let [file, source] = words::<2>(&args, "import needs FILE.eh3d and a file")?;
            let (report, said) =
                eh3dfile::import_file(Path::new(&file), Path::new(&source), &who, limits)?;
            for c in &report.read {
                done.out
                    .push_str(&format!("read: {} {}\n", c.count, c.item));
            }
            for c in &report.not_read {
                done.out.push_str(&format!(
                    "not read: {} {} ({})\n",
                    c.count,
                    c.item,
                    c.reason.as_deref().unwrap_or("-")
                ));
            }
            for c in &report.export_losses {
                done.out.push_str(&format!(
                    "lost on export: {}\n",
                    c.reason.as_deref().unwrap_or(&c.item)
                ));
            }
            notes(&mut done, &said);
            done.out.push_str(&format!(
                "{} was imported into {file}; the original is kept\n",
                report.source.name
            ));
            done.code = i32::from(!report.not_read.is_empty());
            if eh3dfile::is_program(&report.source.name) {
                check_program_of(&mut done, &file, asked.as_ref(), limits)?;
            }
        }
        "open" => {
            let into = flag(&mut args, "--into")?.map(PathBuf::from);
            let asked = flag(&mut args, "--printer")?
                .map(|id| printer_of(&id))
                .transpose()?;
            let [file] = words::<1>(&args, "open needs FILE.eh3d")?;
            let opened = eh3dfile::open(Path::new(&file), into.as_deref(), limits)?;
            done.out
                .push_str(&format!("parts {}\n", opened.parts.len()));
            for part in &opened.parts {
                done.out.push_str(&format!(
                    "part {}: {} triangles\n",
                    part.name,
                    part.mesh.triangles.len()
                ));
            }
            for (what, value) in [
                ("printer", &opened.printer),
                ("filament", &opened.filament),
                ("program", &opened.program),
            ] {
                done.out
                    .push_str(&format!("{what} {}\n", value.as_deref().unwrap_or("none")));
            }
            done.out.push_str(&format!(
                "settings {}\n",
                if opened.settings.is_some() {
                    "yes"
                } else {
                    "none"
                }
            ));
            for (what, block) in [("start", &opened.blocks.start), ("end", &opened.blocks.end)] {
                done.out.push_str(&match block {
                    Some(text) => format!("{what} G-code {} line(s)\n", text.lines().count()),
                    None => format!("{what} G-code none\n"),
                });
            }
            if let Some(profile) = &opened.profile {
                done.out
                    .push_str(&format!("printer profile {}\n", profile.id));
            }
            if let Some(report) = &opened.report {
                done.out.push_str(&format!(
                    "imported {} ({}), {} item(s) not read\n",
                    report.source.name,
                    report.source.format,
                    report.not_read.len()
                ));
            }
            if let (Some(count), Some(dir)) = (opened.written, &into) {
                done.out
                    .push_str(&format!("{count} files written to {}\n", dir.display()));
            }
            notes(&mut done, &opened.warnings);
            done.code = i32::from(!opened.warnings.is_empty());
            check_program_of(&mut done, &file, asked.as_ref(), limits)?;
        }
        "take" => {
            let asked = flag(&mut args, "--printer")?
                .map(|id| printer_of(&id))
                .transpose()?;
            let [file, branch, name] =
                words::<3>(&args, "take needs FILE.eh3d, a branch and a file of it")?;
            let (target, said) = eh3dfile::take(Path::new(&file), &branch, &name, limits)?;
            notes(&mut done, &said);
            done.out = format!("{branch}/{name} was taken into {BRANCH}/{target}\n");
            if eh3dfile::is_program(&name) {
                check_program_of(&mut done, &file, asked.as_ref(), limits)?;
            }
        }
        "extract" => {
            let [file, branch, dir] =
                words::<3>(&args, "extract needs FILE.eh3d, a branch and a folder")?;
            let count = eh3dfile::extract(Path::new(&file), &branch, Path::new(&dir))?;
            done.out = format!("{count} files written to {dir}\n");
        }
        "layers" => {
            let asked = flag(&mut args, "--printer")?
                .map(|id| printer_of(&id))
                .transpose()?;
            let only = flag(&mut args, "--layer")?
                .map(|v| {
                    v.parse::<usize>()
                        .map_err(|_| "--layer takes a number".to_string())
                })
                .transpose()?;
            let [file] = words::<1>(&args, "layers needs FILE.eh3d")?;
            layers_of(&mut done, &file, only, asked.as_ref(), limits)?;
        }
        other => {
            return Err(format!(
                "project {other}: it is list, save, import, open, take, extract or layers"
            ))
        }
    }
    Ok(done)
}

/// Runs the words after `project`, with the limits of the settings of the program.
pub fn run(args: &[String], limits: &Limits) -> Done {
    job(args.to_vec(), limits).unwrap_or_else(|why| Done {
        out: String::new(),
        err: format!("phoenix-3dp: {why}\n"),
        code: 2,
    })
}
