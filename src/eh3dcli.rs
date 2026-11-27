//! The command `phoenix-3dp project list|save|import|open|take|extract FILE.eh3d ...`: the words after `project` for a project of the suite, apart from the window so that it can be
//! tried without one. Nothing here is sent to a printer.

use std::path::{Path, PathBuf};

use eh3d_project::appbranch::read_bounded;
use p3dp_core::Limits;
use p3dp_profiles::filament::FilamentCatalog;
use p3dp_profiles::suite_overlay;
use p3dp_slicer::SliceConfig;

use crate::eh3dfile::{self, Contents, Held, Identity, Part, BRANCH};

/// The words that make `project` work on a project of the suite and not read a file of another program.
pub const WORDS: [&str; 6] = ["list", "save", "import", "open", "take", "extract"];

/// The largest settings file that is read.
const MAX_SETTINGS_BYTES: u64 = 1 << 20;

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

/// What `save` puts into the branch, every file read and checked before anything is written.
fn contents_of(args: &mut Vec<String>, limits: &Limits) -> Result<Contents, String> {
    let model = flag(args, "--model")?;
    let settings = flag(args, "--settings")?;
    let printer = flag(args, "--printer")?;
    let filament = flag(args, "--filament")?;
    let program = flag(args, "--gcode")?;
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
        let catalog = suite_overlay().map_err(|e| e.to_string())?.catalog;
        let found = catalog
            .find(&id)
            .ok_or_else(|| format!("{id}: no such printer"))?;
        contents.printer = Some(found.id.clone());
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
    Ok(contents)
}

fn job(mut args: Vec<String>, limits: &Limits) -> Result<Done, String> {
    if args.is_empty() {
        return Err("project needs list, save, import, open, take or extract".into());
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
        }
        "open" => {
            let into = flag(&mut args, "--into")?.map(PathBuf::from);
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
        }
        "take" => {
            let [file, branch, name] =
                words::<3>(&args, "take needs FILE.eh3d, a branch and a file of it")?;
            let (target, said) = eh3dfile::take(Path::new(&file), &branch, &name, limits)?;
            notes(&mut done, &said);
            done.out = format!("{branch}/{name} was taken into {BRANCH}/{target}\n");
        }
        "extract" => {
            let [file, branch, dir] =
                words::<3>(&args, "extract needs FILE.eh3d, a branch and a folder")?;
            let count = eh3dfile::extract(Path::new(&file), &branch, Path::new(&dir))?;
            done.out = format!("{count} files written to {dir}\n");
        }
        other => {
            return Err(format!(
                "project {other}: it is list, save, import, open, take or extract"
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
