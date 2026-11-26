//! The command line: `printers`, `machines`, `machine`, `import`, `info`, `check-profile`, `convert`, `slice`, `project` and `version`. `run` returns what to print and the exit code, so it is tested without a terminal.
//! Exit code 0 is success, 1 is a failure of the thing that was asked (a check that did not pass, a file that could not be read) and 2 is a mistake in the command.

use crate::app_state::{profile_report, AppState};
use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::import::{self, Word as ImportWord};
use crate::machine_view;
use crate::report;
use eh3d_printers::{Level, Processes};
use p3dp_formats::picture::PictureFormat;
use p3dp_formats::project3mf::read_project;
use p3dp_formats::{bgcode, gcode_meta, resin, resin_control};
use p3dp_formats::{write_mesh, Format};
use p3dp_gcode::check_program;
use p3dp_profiles::filament::{FilamentCatalog, Nozzle};
use p3dp_profiles::{
    motion_limits, suite_machines, MotionLimits, PrinterProfile, ResinCatalog, ResinKind,
    Technology,
};
use p3dp_slicer::machine_check::{self, Held};
use p3dp_slicer::{Machine, SliceConfig, UserGcode};
use std::collections::HashMap;

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Outcome {
    fn ok(out: String) -> Outcome {
        Outcome {
            code: 0,
            out,
            err: String::new(),
        }
    }

    fn fail(err: String) -> Outcome {
        Outcome {
            code: 1,
            out: String::new(),
            err,
        }
    }

    fn usage(err: &str) -> Outcome {
        Outcome {
            code: 2,
            out: String::new(),
            err: format!("{err}\n\n{USAGE}"),
        }
    }
}

/// The help text.
pub const USAGE: &str = "phoenix-3dp: prepare and send 3D prints (simulators only: nothing here has run on a printer)

  phoenix-3dp printers [--vendor NAME] [--tech fdm|msla] [--search TEXT]
  phoenix-3dp machines [--search TEXT]
  phoenix-3dp machine ID [--nozzle MM] [--process draft|standard|fine|strong [--out FILE] [--force]]
  phoenix-3dp import FILE.3mf|FILE.json|FILE.ini [--out SETTINGS] [--force]
  phoenix-3dp info MODEL [--printer ID]
  phoenix-3dp check-profile ID [--gcode FILE]
  phoenix-3dp check-profile --file PROFILE [--gcode FILE]
  phoenix-3dp check-profile ID|--file PROFILE --set \"key=value;key=value\" [--save FILE] [--force]
  phoenix-3dp convert IN OUT [--printer ID] [--force]
  phoenix-3dp filaments [--printer ID] [--nozzle MM] [--hardened]
  phoenix-3dp resins [--kind KIND] [--printer ID] [--part-ml ML]
  phoenix-3dp fit MODEL --printer ID [--process draft|standard|fine|strong] [--settings FILE] [--filament ID] [--nozzle MM] [--hardened]
  phoenix-3dp slice MODEL --printer ID [--process LEVEL] [--settings FILE] [--filament ID] [--nozzle MM] [--hardened] [--start-gcode FILE] [--end-gcode FILE] [--report] [--out FILE.gcode] [--force]
  phoenix-3dp project FILE.3mf|FILE.bgcode|FILE.gcode|FILE.sl1 [--thumbnail FILE] [--force] [--bottom-layers N] [--lift MM] [--light-power MW]
  phoenix-3dp version

Every command also takes --config FILE (the settings) and --lang CODE (en, es, de, fr, it, ja, zh, pt).
The models are STL, OBJ and 3MF; OUT is chosen by its extension (.stl, .obj, .3mf).
`machines` and `machine` show the catalog of printers of the suite: what each maker publishes, with its pages, the filaments and processes to start from and, with --process,
a settings file for `slice`. `import` brings the settings of a 3MF project or of a JSON or INI profile of yours, tells every key it applied, did not apply, did not use
or did not read, and with --out writes the applied ones as a settings file. Nothing is sent to a printer.
`slice` makes the layers and the G-code of a closed model for a printer of the catalog and refuses, never cuts down, what does not fit or goes past a limit;
without --out it only reports; with --process it starts from that process for the nozzle (the keys of --settings win over it); the highest speed, acceleration and flow
the maker states are kept to. --start-gcode and --end-gcode add your own lines to the ones of the program, checked line by line against the limits and the volume. `fit` tells,
without slicing, whether the job fits and keeps to the machine. With --report it also tells the material, its cost, the time and the layers. `resins` lists the resins and, with --part-ml, what a part of that
volume takes. `check-profile --set` edits a copy of a profile (a limit past the figure of the maker is allowed and told) and --save writes it as a profile file. `project` reads the
objects, plates and settings of a 3MF project saved by another program, the header and thumbnails of a binary G-code file, the figures and thumbnails in the comments of a plain
G-code file, or the header of an open package of resin layers (.sl1) with an estimate of its time; all of it read only, and the closed formats of resin printers are refused with the reason.";

struct Parsed {
    positional: Vec<String>,
    flags: HashMap<String, String>,
}

/// Splits the arguments of a command into positionals, flags with a value and switches.
fn parse(args: &[String], with_value: &[&str], switches: &[&str]) -> Result<Parsed, String> {
    let mut positional = Vec::new();
    let mut flags = HashMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            if switches.contains(&name) {
                flags.insert(name.to_string(), String::new());
            } else if with_value.contains(&name) {
                let v = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
                if flags.insert(name.to_string(), v.clone()).is_some() {
                    return Err(format!("--{name} is given twice"));
                }
            } else {
                return Err(format!("--{name} is not an option of this command"));
            }
        } else {
            positional.push(a.clone());
        }
    }
    Ok(Parsed { positional, flags })
}

fn load_config(p: &Parsed) -> Result<AppConfig, String> {
    match p.flags.get("config") {
        None => Ok(AppConfig::default()),
        Some(path) => {
            let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
            if meta.len() > p3dp_core::kv::MAX_BYTES as u64 {
                return Err(format!("{path}: the settings file is too large"));
            }
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            AppConfig::from_text(&text).map_err(|e| format!("{path}: {e}"))
        }
    }
}

fn state_of(p: &Parsed, env_lang: Option<&str>) -> Result<AppState, String> {
    let config = load_config(p)?;
    // the order is: --lang, the settings file, the environment, English
    let lang = p
        .flags
        .get("lang")
        .and_then(|l| Lang::from_code(l))
        .or(config.lang)
        .unwrap_or_else(|| Lang::detect(None, env_lang));
    AppState::new(config, lang)
}

/// Runs a command. `env_lang` is the language of the environment (`PHOENIX_3DP_LANG` or `LANG`).
pub fn run(args: &[String], env_lang: Option<&str>) -> Outcome {
    let Some((cmd, rest)) = args.split_first() else {
        return Outcome::usage("no command given");
    };
    match cmd.as_str() {
        "help" | "--help" | "-h" => Outcome::ok(format!("{USAGE}\n")),
        "version" | "--version" | "-V" => Outcome::ok(format!("phoenix-3dp {}\n", crate::VERSION)),
        "printers" => printers(rest, env_lang),
        "machines" => machines(rest, env_lang),
        "machine" => machine(rest, env_lang),
        "import" => import_file(rest, env_lang),
        "info" => info(rest, env_lang),
        "check-profile" => check_profile(rest, env_lang),
        "convert" => convert(rest, env_lang),
        "filaments" => filaments(rest, env_lang),
        "resins" => resins(rest, env_lang),
        "fit" => fit_job(rest, env_lang),
        "slice" => slice(rest, env_lang),
        "project" => project(rest, env_lang),
        other => Outcome::usage(&format!("{other:?} is not a command")),
    }
}

/// Writes a text to a file: an existing file is replaced only with `--force`, and the file is whole or not there.
fn write_new(target: &str, text: &str, force: bool) -> Result<(), String> {
    if !force && std::path::Path::new(target).exists() {
        return Err(format!(
            "{target}: the file exists (use --force to replace it)"
        ));
    }
    let tmp = format!("{target}.tmp-{}", std::process::id());
    std::fs::write(&tmp, text.as_bytes()).map_err(|e| format!("{tmp}: {e}"))?;
    std::fs::rename(&tmp, target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{target}: {e}")
    })
}

fn text_of_lines(lines: Vec<String>) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn machines(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["search", "config", "lang"], &[]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    if !p.positional.is_empty() {
        return Outcome::usage("machines takes no file");
    }
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let catalog = match suite_machines() {
        Ok(c) => c,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let query = p.flags.get("search").cloned().unwrap_or_default();
    let found = catalog.search(&query);
    Outcome::ok(text_of_lines(machine_view::list_lines(&found, state.lang)))
}

fn machine(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &["nozzle", "process", "out", "config", "lang"],
        &["force"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [id] = p.positional.as_slice() else {
        return Outcome::usage("machine takes one printer id (see `phoenix-3dp machines`)");
    };
    if p.flags.contains_key("out") && !p.flags.contains_key("process") {
        return Outcome::usage("--out writes the settings of a process: give --process");
    }
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let catalog = match suite_machines() {
        Ok(c) => c,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let Some(printer) = catalog.find(id) else {
        return Outcome::fail(format!(
            "{id}: not in the catalog of the suite (see `phoenix-3dp machines`)"
        ));
    };
    let asked = match p.flags.get("nozzle").map(|v| v.parse::<f64>()) {
        None => None,
        Some(Ok(n)) => Some(n),
        Some(Err(_)) => return Outcome::usage("--nozzle takes a number of millimetres"),
    };
    let nozzle = match machine_view::nozzle_of(printer, asked) {
        Ok(n) => n,
        Err(e) => return Outcome::fail(e),
    };
    let Some(name) = p.flags.get("process") else {
        return match machine_view::show_lines(printer, nozzle, state.lang) {
            Ok(lines) => Outcome::ok(text_of_lines(lines)),
            Err(e) => Outcome::fail(e),
        };
    };
    let Some(level) = Level::from_name(name) else {
        return Outcome::usage("--process is draft, standard, fine or strong");
    };
    let text = match machine_view::process_settings(printer, level, nozzle) {
        Ok(t) => t,
        Err(e) => return Outcome::fail(e),
    };
    match p.flags.get("out") {
        None => Outcome::ok(text),
        Some(target) => match write_new(target, &text, p.flags.contains_key("force")) {
            Ok(()) => Outcome::ok(format!(
                "{}\n",
                fill(ImportWord::Written.tr(state.lang), &[target])
            )),
            Err(e) => Outcome::fail(e),
        },
    }
}

fn import_file(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["out", "config", "lang"], &["force"]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [file] = p.positional.as_slice() else {
        return Outcome::usage("import takes one project or profile file");
    };
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let bytes = match read_limited(file, state.config.limits.max_file_bytes) {
        Ok(b) => b,
        Err(e) => return Outcome::fail(e),
    };
    let report = match import::import(&bytes, &state.config.limits, &state.catalog) {
        Ok(r) => r,
        Err(e) => return Outcome::fail(format!("{file}: {e}")),
    };
    let mut out = text_of_lines(import::report_lines(&report, state.lang));
    let Some(target) = p.flags.get("out") else {
        return Outcome::ok(out);
    };
    // what was asked and cannot be done is a failure, and the report is still given
    let refused = |out: String, why: String| Outcome {
        code: 1,
        out,
        err: format!("{why}\n"),
    };
    if let Some(conflict) = &report.conflict {
        let why = fill(ImportWord::DoesNotHold.tr(state.lang), &[conflict]);
        return refused(out, why);
    }
    if report.applied.is_empty() {
        let why = ImportWord::NothingToWrite.tr(state.lang).to_string();
        return refused(out, why);
    }
    let same = match (std::fs::canonicalize(file), std::fs::canonicalize(target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => file == target,
    };
    if same {
        return refused(out, "the output is the file that was imported".to_string());
    }
    match write_new(
        target,
        &report.settings_text(),
        p.flags.contains_key("force"),
    ) {
        Ok(()) => {
            out.push_str(&fill(ImportWord::Written.tr(state.lang), &[target]));
            out.push('\n');
            Outcome::ok(out)
        }
        Err(e) => refused(out, e),
    }
}

/// The nozzle that `--nozzle` and `--hardened` name: the first one the printer lists when none is named.
fn nozzle_of(p: &Parsed, printer: &PrinterProfile) -> Result<Nozzle, String> {
    let diameter_mm = match p.flags.get("nozzle") {
        Some(v) => v
            .parse::<f64>()
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
            .ok_or_else(|| "--nozzle takes a number of millimetres".to_string())?,
        None => printer
            .nozzles_mm
            .iter()
            .copied()
            .find(|d| (d - 0.4).abs() < 1e-9)
            .or_else(|| printer.nozzles_mm.first().copied())
            .unwrap_or(0.4),
    };
    Ok(Nozzle {
        diameter_mm,
        hardened: p.flags.contains_key("hardened"),
    })
}

/// The filaments of the program, and what each one comes to on a printer when one is named.
fn filaments(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &["printer", "nozzle", "config", "lang"],
        &["hardened"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    if !p.positional.is_empty() {
        return Outcome::usage("filaments takes no file");
    }
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let printer = match p.flags.get("printer") {
        None => None,
        Some(id) => {
            if !state.select_printer(id) {
                return Outcome::fail(state.status.text(state.lang));
            }
            state.printer().cloned()
        }
    };
    let catalog = match FilamentCatalog::builtin() {
        Ok(c) => c,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let nozzle = match printer.as_ref().map(|pr| nozzle_of(&p, pr)) {
        Some(Err(e)) => return Outcome::usage(&e),
        Some(Ok(n)) => Some(n),
        None => None,
    };
    let mut out = String::new();
    for f in &catalog.profiles {
        out.push_str(&format!(
            "{:<14} {:<18} nozzle {}-{} C (best {}), bed {}-{} C (best {}), fan {} %, {} g/cm3, {} mm, not verified\n",
            f.id,
            f.name,
            f.nozzle_c.lowest,
            f.nozzle_c.highest,
            f.nozzle_c.best,
            f.bed_c.lowest,
            f.bed_c.highest,
            f.bed_c.best,
            f.fan_percent.best,
            f.density_g_cm3,
            f.diameter_mm
        ));
        if let (Some(pr), Some(n)) = (printer.as_ref(), nozzle) {
            let c = f.check(pr, n);
            if c.ok() {
                out.push_str(&format!(
                    "    on {} with a {} mm nozzle: nozzle {} C, bed {} C\n",
                    pr.title(),
                    n.diameter_mm,
                    c.nozzle_c.unwrap_or(0.0),
                    c.bed_c.unwrap_or(0.0)
                ));
            } else {
                for i in &c.issues {
                    out.push_str(&format!(
                        "    on {}: NOT POSSIBLE, {}\n",
                        pr.title(),
                        i.text()
                    ));
                }
            }
            for a in &c.advice {
                out.push_str(&format!("    note: {}\n", a.text()));
            }
        }
    }
    out.push_str("the ranges are typical for each family of material, not the figures of a spool, and none has been tried on a machine\n");
    Outcome::ok(out)
}

/// The resins of the program; with a printer, only if it is a resin printer; with a volume, what a part of that volume takes.
fn resins(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["kind", "printer", "part-ml", "config", "lang"], &[]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    if !p.positional.is_empty() {
        return Outcome::usage("resins takes no file");
    }
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let catalog = match ResinCatalog::builtin() {
        Ok(c) => c,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let kind = match p
        .flags
        .get("kind")
        .map(|k| ResinKind::from_name(k).ok_or(k))
    {
        Some(Err(k)) => {
            return Outcome::usage(&format!(
                "--kind {k:?} is not one of standard, tough, flexible, castable, clear, high-detail"
            ))
        }
        Some(Ok(k)) => Some(k),
        None => None,
    };
    let part_mm3 = match p.flags.get("part-ml").map(|v| v.parse::<f64>()) {
        Some(Ok(ml)) if ml.is_finite() && ml >= 0.0 => Some(ml * 1000.0),
        Some(_) => return Outcome::usage("--part-ml takes a number of millilitres, zero or more"),
        None => None,
    };
    if let Some(id) = p.flags.get("printer") {
        if !state.select_printer(id) {
            return Outcome::fail(state.status.text(state.lang));
        }
    }
    let printer = state
        .printer()
        .cloned()
        .filter(|_| p.flags.contains_key("printer"));
    let mut out = String::new();
    let mut n = 0;
    for r in catalog
        .profiles
        .iter()
        .filter(|r| kind.is_none_or(|k| r.kind == k))
    {
        if let Some(pr) = &printer {
            if let Err(e) = r.usable_on(pr) {
                return Outcome::fail(e.to_string());
            }
        }
        out.push_str(&format!(
            "{:<22} {:<30} {:<12} layer {}-{} um (best {}), exposure {} s ({} s for the first {} layers), lift {} mm at {} mm/min, {} nm, {} g/ml, wash with {}, not verified\n",
            r.id,
            r.name,
            r.kind.name(),
            r.layer_um.lowest,
            r.layer_um.highest,
            r.layer_um.best,
            r.normal_exposure_s.best,
            r.bottom_exposure_s.best,
            r.bottom_layers,
            r.lift_mm,
            r.lift_speed_mm_min,
            r.wavelength_nm,
            r.density_g_ml,
            r.wash.name()
        ));
        if let Some(v) = part_mm3 {
            match report::resin_line(r, v, &state.config, state.lang) {
                Ok(l) => out.push_str(&format!("    {l}\n")),
                Err(e) => return Outcome::fail(e),
            }
        }
        n += 1;
    }
    out.push_str(&format!("{n} resins\n"));
    out.push_str("the exposures are starting points for a monochrome-screen printer, not the figures of a bottle: they depend on the power of the light, and none has been tried on a machine\n");
    Outcome::ok(out)
}

fn printers(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["vendor", "tech", "search", "config", "lang"], &[]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    if !p.positional.is_empty() {
        return Outcome::usage("printers takes no file");
    }
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let tech = match p
        .flags
        .get("tech")
        .map(|t| Technology::from_name(t).ok_or(t))
    {
        Some(Err(t)) => return Outcome::usage(&format!("--tech {t:?} is not fdm or msla")),
        Some(Ok(t)) => Some(t),
        None => None,
    };
    let vendor = p.flags.get("vendor").map(|v| v.to_ascii_lowercase());
    let query = p.flags.get("search").cloned().unwrap_or_default();
    let mut out = String::new();
    let mut n = 0;
    for pr in state.catalog.search(&query) {
        if tech.is_some_and(|t| pr.technology != t)
            || vendor
                .as_ref()
                .is_some_and(|v| !pr.vendor.to_ascii_lowercase().contains(v.as_str()))
        {
            continue;
        }
        let conns: Vec<&str> = pr.connectivity.iter().map(|c| c.name()).collect();
        out.push_str(&format!(
            "{:<26} {:<34} {:<4} {:>3}x{:<3}x{:<3} {:<8} {:<26} {}\n",
            pr.id,
            pr.title(),
            pr.technology.name(),
            pr.volume_mm[0],
            pr.volume_mm[1],
            pr.volume_mm[2],
            pr.dialect.name(),
            conns.join(","),
            if pr.verified {
                "verified"
            } else {
                "not verified"
            }
        ));
        n += 1;
    }
    out.push_str(&format!(
        "{n} printers\n{}\n",
        Msg::SimulatorOnly.tr(state.lang)
    ));
    Outcome::ok(out)
}

fn info(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["printer", "config", "lang"], &[]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [file] = p.positional.as_slice() else {
        return Outcome::usage("info takes one model file");
    };
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    if let Some(id) = p.flags.get("printer") {
        if !state.select_printer(id) {
            return Outcome::fail(state.status.text(state.lang));
        }
    }
    if !state.load_model_file(file) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let mut out = String::new();
    for l in state.model_report() {
        out.push_str(&l);
        out.push('\n');
    }
    Outcome::ok(out)
}

fn check_profile(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &["file", "gcode", "config", "lang", "set", "save"],
        &["force"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let profile: PrinterProfile = match (p.positional.as_slice(), p.flags.get("file")) {
        ([id], None) => match state.catalog.find(id) {
            Some(pr) => pr.clone(),
            None => return Outcome::fail(format!("{id}: no such printer in the catalog")),
        },
        ([], Some(path)) => {
            let text = match std::fs::metadata(path)
                .map_err(|e| e.to_string())
                .and_then(|m| {
                    if m.len() > p3dp_core::kv::MAX_BYTES as u64 {
                        Err("the profile file is too large".to_string())
                    } else {
                        std::fs::read_to_string(path).map_err(|e| e.to_string())
                    }
                }) {
                Ok(t) => t,
                Err(e) => return Outcome::fail(format!("{path}: {e}")),
            };
            match PrinterProfile::from_text(&text) {
                Ok(pr) => pr,
                Err(e) => return Outcome::fail(format!("{path}: {e}")),
            }
        }
        _ => return Outcome::usage("check-profile takes an id or --file PROFILE"),
    };
    let original = profile.clone();
    let mut profile = profile;
    let mut notes = String::new();
    if let Some(spec) = p.flags.get("set") {
        let mut edits: Vec<(&str, &str)> = Vec::new();
        for part in spec.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let Some((k, v)) = part.split_once('=') else {
                return Outcome::usage(&format!(
                    "--set takes key=value pairs separated by semicolons, not {part:?}"
                ));
            };
            edits.push((k.trim(), v.trim()));
        }
        match original.edited(&edits) {
            Ok(e) => {
                for c in original.diff(&e.profile) {
                    let shown = |s: &str| {
                        if s.is_empty() {
                            "-".to_string()
                        } else {
                            s.to_string()
                        }
                    };
                    notes.push_str(&format!(
                        "changed: {}: {} -> {}\n",
                        c.key,
                        shown(&c.from),
                        shown(&c.to)
                    ));
                }
                for l in report::notice_lines(&e.notices, state.lang) {
                    notes.push_str(&format!("note: {l}\n"));
                }
                profile = e.profile;
            }
            Err(e) => return Outcome::fail(e.to_string()),
        }
    }
    if let Some(path) = p.flags.get("save") {
        if !p.flags.contains_key("force") && std::path::Path::new(path).exists() {
            return Outcome::fail(format!(
                "{path}: the file exists (use --force to replace it)"
            ));
        }
        let tmp = format!("{path}.tmp-{}", std::process::id());
        if let Err(e) = std::fs::write(&tmp, profile.to_text()) {
            return Outcome::fail(format!("{tmp}: {e}"));
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Outcome::fail(format!("{path}: {e}"));
        }
        notes.push_str(&format!("saved: {path}\n"));
    }
    let mut out = String::new();
    for l in profile_report(&profile, state.lang) {
        out.push_str(&l);
        out.push('\n');
    }
    out.push_str(&notes);
    let Some(gpath) = p.flags.get("gcode") else {
        return Outcome::ok(out);
    };
    let Some(limits) = profile.limits else {
        return Outcome::fail(format!(
            "{out}{gpath}: a resin printer takes layers, not G-code"
        ));
    };
    let size = match std::fs::metadata(gpath) {
        Ok(m) => m.len(),
        Err(e) => return Outcome::fail(format!("{gpath}: {e}")),
    };
    if size > state.config.limits.max_file_bytes {
        return Outcome::fail(format!(
            "{gpath}: the file is {size} bytes, more than the limit"
        ));
    }
    let text = match std::fs::read(gpath) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => return Outcome::fail(format!("{gpath}: {e}")),
    };
    let violations = check_program(&text, &limits);
    if violations.is_empty() {
        out.push_str(&format!(
            "{gpath}: no temperature goes past the limits of this model\n"
        ));
        return Outcome::ok(out);
    }
    for v in &violations {
        out.push_str(&format!(
            "{gpath}: line {}: {} ({})\n",
            v.line, v.error, v.command
        ));
    }
    Outcome {
        code: 1,
        out,
        err: format!(
            "{} line(s) go past the limits of {}\n",
            violations.len(),
            profile.id
        ),
    }
}

fn convert(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(args, &["printer", "config", "lang"], &["force"]) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [input, output] = p.positional.as_slice() else {
        return Outcome::usage("convert takes an input and an output file");
    };
    let Some(format) = Format::from_path(output) else {
        return Outcome::usage("the output must end in .stl, .obj or .3mf");
    };
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    if let Some(id) = p.flags.get("printer") {
        if !state.select_printer(id) {
            return Outcome::fail(state.status.text(state.lang));
        }
    }
    let same = match (std::fs::canonicalize(input), std::fs::canonicalize(output)) {
        (Ok(a), Ok(b)) => a == b,
        _ => input == output,
    };
    if same {
        return Outcome::fail("the output is the input file".to_string());
    }
    if !p.flags.contains_key("force") && std::path::Path::new(output).exists() {
        return Outcome::fail(format!(
            "{output}: the file exists (use --force to replace it)"
        ));
    }
    if !state.load_model_file(input) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let Some(mesh) = state.model.as_ref() else {
        return Outcome::fail("no model".to_string());
    };
    let bytes = if format == Format::ThreeMf && p.flags.contains_key("printer") {
        match state.project().map(|pr| pr.to_3mf()) {
            Some(Ok(b)) => b,
            Some(Err(e)) => return Outcome::fail(e.to_string()),
            None => return Outcome::fail("no model".to_string()),
        }
    } else {
        match write_mesh(mesh, format, &state.model_name) {
            Ok(b) => b,
            Err(e) => return Outcome::fail(e.to_string()),
        }
    };
    let tmp = format!("{output}.tmp-{}", std::process::id());
    if let Err(e) = std::fs::write(&tmp, &bytes) {
        return Outcome::fail(format!("{tmp}: {e}"));
    }
    if let Err(e) = std::fs::rename(&tmp, output) {
        let _ = std::fs::remove_file(&tmp);
        return Outcome::fail(format!("{output}: {e}"));
    }
    Outcome::ok(format!(
        "{} ({} triangles, {} bytes)\n",
        output,
        mesh.triangles.len(),
        bytes.len()
    ))
}

fn read_limited(path: &str, max: u64) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("{path}: {e}"))?
        .len();
    if size > max {
        return Err(format!(
            "{path}: the file is {size} bytes, more than the limit"
        ));
    }
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

/// The settings of a job and what was told while they were put together.
struct Job {
    config: SliceConfig,
    notes: String,
    density: f64,
}

fn read_text(path: &str) -> Result<String, String> {
    read_limited(path, p3dp_core::kv::MAX_BYTES as u64)
        .and_then(|b| String::from_utf8(b).map_err(|_| format!("{path}: not UTF-8")))
}

/// The settings of a job: the ones of the program, over them the process asked for, over that the settings file and over that the filament.
fn job_config(p: &Parsed, profile: &PrinterProfile) -> Result<Job, Outcome> {
    let mut config = SliceConfig::default();
    let nozzle = nozzle_of(p, profile).map_err(|e| Outcome::usage(&e))?;
    if let Some(name) = p.flags.get("process") {
        let Some(level) = Level::from_name(name) else {
            return Err(Outcome::usage(
                "--process is draft, standard, fine or strong",
            ));
        };
        let process = Processes::embedded()
            .and_then(|r| r.for_nozzle(level, nozzle.diameter_mm))
            .map_err(|e| Outcome::fail(e.to_string()))?;
        let filament_mm = suite_machines()
            .map_err(|e| Outcome::fail(e.to_string()))?
            .find(&profile.id)
            .and_then(|m| m.filament_diameter_mm);
        for (key, value) in machine_view::settings_of(&process, filament_mm) {
            config
                .set(key, &value)
                .map_err(|e| Outcome::fail(e.to_string()))?;
        }
    }
    if let Some(path) = p.flags.get("settings") {
        let text = read_text(path).map_err(Outcome::fail)?;
        let entries =
            p3dp_core::kv::parse(&text).map_err(|e| Outcome::fail(format!("{path}: {e}")))?;
        for e in entries {
            config
                .set(&e.key, &e.value)
                .map_err(|err| Outcome::fail(format!("{path}: line {}: {err}", e.line)))?;
        }
    }
    config.check().map_err(|e| Outcome::fail(e.to_string()))?;
    let mut notes = String::new();
    let mut density = report::DEFAULT_DENSITY_G_CM3;
    if let Some(fid) = p.flags.get("filament") {
        let filaments = FilamentCatalog::builtin().map_err(|e| Outcome::fail(e.to_string()))?;
        let Some(f) = filaments.find(fid) else {
            return Err(Outcome::fail(format!(
                "{fid}: no such filament (see `phoenix-3dp filaments`)"
            )));
        };
        density = f.density_g_cm3;
        let compat = config
            .apply_filament(f, profile, nozzle)
            .map_err(|e| Outcome::fail(e.to_string()))?;
        notes.push_str(&format!(
            "filament {}: nozzle {} C, bed {} C, fan {} %\n",
            f.name, config.nozzle_c, config.bed_c, config.fan_percent
        ));
        for a in compat.advice {
            notes.push_str(&format!("note: {}\n", a.text()));
        }
        if let Some(r) = crate::preflight::record(&f.id) {
            if let Some(limit) = crate::preflight::hold_flow(r, &mut config) {
                notes.push_str(&format!(
                    "flow: held to {limit} mm3/s, what the maker gives for the filament\n"
                ));
            }
        }
    }
    Ok(Job {
        config,
        notes,
        density,
    })
}

fn held_text(held: Held, lang: Lang) -> String {
    match held {
        Held::Acceleration(a) => fill(Msg::FitAccel.tr(lang), &[&a.to_string()]),
        Held::Flow(q) => fill(Msg::FitFlow.tr(lang), &[&q.to_string()]),
    }
}

/// Tells, without slicing, whether a model with its settings fits a printer and keeps to what its maker states. The exit code is 1 when it does not.
fn fit_job(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &[
            "printer", "settings", "config", "lang", "filament", "nozzle", "process",
        ],
        &["hardened"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [model] = p.positional.as_slice() else {
        return Outcome::usage("fit takes one model file");
    };
    let Some(id) = p.flags.get("printer") else {
        return Outcome::usage("fit needs --printer ID (see `phoenix-3dp printers`)");
    };
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    if !state.select_printer(id) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let Some(profile) = state.printer().cloned() else {
        return Outcome::fail(format!("{id}: no such printer in the catalog"));
    };
    if let Err(e) = Machine::from_profile(&profile) {
        return Outcome::fail(e.to_string());
    }
    let job = match job_config(&p, &profile) {
        Ok(j) => j,
        Err(e) => return e,
    };
    if !state.load_model_file(model) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let Some(size) = state.model.as_ref().and_then(|m| m.size_mm()) else {
        return Outcome::fail("no model".to_string());
    };
    let suite = match suite_machines() {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let record = suite.find(&profile.id);
    let limits = record.map(MotionLimits::of).unwrap_or_default();
    let lang = state.lang;
    let mut config = job.config;
    let n = |v: f64| format!("{}", (v * 10.0).round() / 10.0);
    let found = machine_check::fit(size, profile.volume_mm, &config, limits.default_height_mm);
    let v = profile.volume_mm;
    let mut lines = vec![
        format!("{model} - {}", profile.title()),
        fill(
            Msg::FitSize.tr(lang),
            &[
                &n(found.needs_mm[0]),
                &n(found.needs_mm[1]),
                &n(found.needs_mm[2]),
                &n(v[0]),
                &n(v[1]),
                &n(v[2]),
            ],
        ),
    ];
    let mut good = found.fits();
    if good {
        lines.push(Msg::FitYes.tr(lang).to_string());
    }
    for (axis, over) in ["x", "y", "z"].iter().zip(found.over_mm) {
        if over > 0.0 {
            lines.push(fill(Msg::FitNo.tr(lang), &[&n(over), axis]));
        }
    }
    if found.fits_turned {
        lines.push(Msg::FitTurned.tr(lang).to_string());
    }
    if let Some(h) = found.above_default_height_mm {
        lines.push(fill(
            Msg::FitDefaultHeight.tr(lang),
            &[&n(found.needs_mm[2]), &n(h)],
        ));
    }
    for t in machine_check::too_fast(&config, &limits) {
        good = false;
        lines.push(fill(
            Msg::FitSpeed.tr(lang),
            &[t.key, &n(t.set_mm_s), &n(t.max_mm_s)],
        ));
    }
    for h in machine_check::hold(&mut config, &limits) {
        lines.push(held_text(h, lang));
    }
    if !profile
        .nozzles_mm
        .iter()
        .any(|x| (x - config.nozzle_diameter_mm).abs() < 1e-9)
    {
        good = false;
        lines.push(fill(
            Msg::FitNozzle.tr(lang),
            &[&config.nozzle_diameter_mm.to_string()],
        ));
    }
    if let Some(d) = record.and_then(|r| r.filament_diameter_mm) {
        if (d - config.filament_diameter_mm).abs() > 1e-9 {
            good = false;
            lines.push(fill(
                Msg::FitFilamentDiameter.tr(lang),
                &[&d.to_string(), &config.filament_diameter_mm.to_string()],
            ));
        }
    }
    if limits.max_speed_mm_s.is_none() && limits.max_flow_mm3_s.is_none() {
        lines.push(Msg::FitNoFigures.tr(lang).to_string());
    }
    if let Some(r) = record {
        let unchecked = r.unconfirmed();
        if !unchecked.is_empty() {
            lines.push(fill(Msg::FitUnchecked.tr(lang), &[&unchecked.join(", ")]));
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out.push_str(&job.notes);
    out.push_str(&format!("{}\n", Msg::SimulatorOnly.tr(lang)));
    Outcome {
        code: i32::from(!good),
        out,
        err: String::new(),
    }
}

fn slice(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &[
            "printer",
            "settings",
            "out",
            "config",
            "lang",
            "filament",
            "nozzle",
            "process",
            "start-gcode",
            "end-gcode",
        ],
        &["force", "hardened", "report"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [model] = p.positional.as_slice() else {
        return Outcome::usage("slice takes one model file");
    };
    let Some(id) = p.flags.get("printer") else {
        return Outcome::usage("slice needs --printer ID (see `phoenix-3dp printers`)");
    };
    let mut state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    if !state.select_printer(id) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let Some(profile) = state.printer().cloned() else {
        return Outcome::fail(format!("{id}: no such printer in the catalog"));
    };
    let machine = match Machine::from_profile(&profile) {
        Ok(m) => m,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    let job = match job_config(&p, &profile) {
        Ok(j) => j,
        Err(e) => return e,
    };
    let Job {
        mut config,
        mut notes,
        density,
    } = job;
    let limits = match motion_limits(&profile.id) {
        Ok(l) => l,
        Err(e) => return Outcome::fail(e.to_string()),
    };
    match machine_check::apply(&mut config, &limits) {
        Ok(held) => {
            for h in held {
                notes.push_str(&format!("{}\n", held_text(h, state.lang)));
            }
        }
        Err(e) => return Outcome::fail(e.to_string()),
    }
    let mut user = UserGcode::default();
    for (flag, slot) in [
        ("start-gcode", &mut user.start),
        ("end-gcode", &mut user.end),
    ] {
        if let Some(path) = p.flags.get(flag) {
            match read_text(path) {
                Ok(text) => *slot = Some(text),
                Err(e) => return Outcome::fail(e),
            }
        }
    }
    if let Some(out) = p.flags.get("out") {
        if !p.flags.contains_key("force") && std::path::Path::new(out).exists() {
            return Outcome::fail(format!(
                "{out}: the file exists (use --force to replace it)"
            ));
        }
        let same = match (std::fs::canonicalize(model), std::fs::canonicalize(out)) {
            (Ok(a), Ok(b)) => a == b,
            _ => model == out,
        };
        if same {
            return Outcome::fail("the output is the model file".to_string());
        }
    }
    if !state.load_model_file(model) {
        return Outcome::fail(state.status.text(state.lang));
    }
    let Some(mesh) = state.model.as_ref() else {
        return Outcome::fail("no model".to_string());
    };
    let sliced = p3dp_slicer::plan(mesh, &machine, &config).and_then(|plan| {
        let gcode = plan.gcode_with(p3dp_slicer::VERSION, &user)?;
        Ok((plan, gcode))
    });
    let (plan, gcode) = match sliced {
        Ok(r) => r,
        Err(e) => return Outcome::fail(format!("{model}: {e}")),
    };
    let secs = plan.estimated_seconds();
    let mut out = format!(
        "{model} for {}: {} layers, {:.1} mm high, {:.2} m of filament, about {}h{:02}m (an estimate with the acceleration of the settings, not the planner of the printer)\n",
        profile.title(),
        plan.layer_count(),
        plan.height_mm(),
        plan.filament_mm() / 1000.0,
        (secs / 3600.0) as u64,
        ((secs % 3600.0) / 60.0) as u64
    );
    out.push_str(&notes);
    for w in &plan.warnings {
        let p3dp_slicer::Warning::ThinRegion { layer } = w;
        out.push_str(&format!(
            "warning: layer {layer} has a part thinner than the perimeters; it has fewer of them or none\n"
        ));
    }
    if p.flags.contains_key("report") {
        match report::slice_lines(&plan, density, &state.config, state.lang) {
            Ok(lines) => {
                for l in lines {
                    out.push_str(&l);
                    out.push('\n');
                }
            }
            Err(e) => return Outcome::fail(e),
        }
    }
    out.push_str(&format!("{}\n", Msg::SimulatorOnly.tr(state.lang)));
    let Some(target) = p.flags.get("out") else {
        return Outcome::ok(out);
    };
    let tmp = format!("{target}.tmp-{}", std::process::id());
    if let Err(e) = std::fs::write(&tmp, gcode.as_bytes()) {
        return Outcome::fail(format!("{tmp}: {e}"));
    }
    if let Err(e) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        return Outcome::fail(format!("{target}: {e}"));
    }
    out.push_str(&format!("{target} ({} lines)\n", gcode.lines().count()));
    Outcome::ok(out)
}

/// What `project` was asked to do with a thumbnail.
struct Thumb {
    /// The file to write the largest thumbnail to.
    out: Option<String>,
    force: bool,
}

/// Writes a picture taken out of a file. The name must end in the extension of the picture (a QOI picture in a `.png` would not open), and an existing file is not replaced without `--force`.
fn write_picture(
    path: &str,
    format: PictureFormat,
    data: &[u8],
    force: bool,
) -> Result<(), String> {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    if ext.as_deref().and_then(PictureFormat::from_name) != Some(format) {
        return Err(format!(
            "{path}: the picture is {}; name the file with .{}",
            format.extension().to_uppercase(),
            format.extension()
        ));
    }
    if !force && std::path::Path::new(path).exists() {
        return Err(format!(
            "{path}: the file exists (use --force to replace it)"
        ));
    }
    let tmp = format!("{path}.tmp-{}", std::process::id());
    std::fs::write(&tmp, data).map_err(|e| format!("{tmp}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{path}: {e}")
    })
}

/// The figures and thumbnails in the comments of a plain G-code file. Read only: no line is run, and only the start and the end of the file are looked at.
fn gcode_report(
    file: &str,
    bytes: &[u8],
    config: &AppConfig,
    lang: Lang,
    thumb: &Thumb,
) -> Outcome {
    let text = String::from_utf8_lossy(bytes);
    let limits = config.scan_limits();
    let mut out = format!(
        "{file}: plain G-code, {} bytes, {} lines\n",
        bytes.len(),
        text.lines().count()
    );
    let s = gcode_meta::summary(&text, &limits);
    for (k, v) in s.lines() {
        out.push_str(&format!("{k} = {v}\n"));
    }
    let scan = gcode_meta::thumbnails(&text, &limits);
    for (i, t) in scan.thumbnails.iter().enumerate() {
        out.push_str(&report::thumbnail_line(
            i + 1,
            t.width,
            t.height,
            t.format,
            t.data.len(),
            lang,
        ));
        out.push('\n');
    }
    for problem in &scan.problems {
        out.push_str(&format!("warning: {problem}\n"));
    }
    if let Some(path) = &thumb.out {
        let best = scan
            .thumbnails
            .iter()
            .enumerate()
            .fold(None::<(usize, u64)>, |best, (i, t)| {
                let area = u64::from(t.width) * u64::from(t.height);
                if best.is_none_or(|(_, a)| area > a) {
                    Some((i, area))
                } else {
                    best
                }
            });
        let Some((i, _)) = best else {
            return Outcome::fail(format!("{file}: no thumbnail that can be taken out"));
        };
        let t = &scan.thumbnails[i];
        if let Err(e) = write_picture(path, t.format, &t.data, thumb.force) {
            return Outcome::fail(e);
        }
        out.push_str(&format!(
            "{path}: the largest thumbnail ({} bytes) was written\n",
            t.data.len()
        ));
    }
    if s.is_empty() && scan.thumbnails.is_empty() {
        out.push_str("no figures of the print and no thumbnail in its comments\n");
    }
    out.push_str("read only: no line was run or changed\n");
    Outcome::ok(out)
}

/// The header of a binary G-code file, as lines.
fn bgcode_report(
    file: &str,
    bytes: &[u8],
    config: &AppConfig,
    lang: Lang,
    thumb: &Thumb,
) -> Outcome {
    let h = match bgcode::read_header(bytes, &config.limits) {
        Ok(h) => h,
        Err(e) => return Outcome::fail(format!("{file}: {e}")),
    };
    let mut out = format!(
        "{file}: binary G-code, version {}, {} blocks ({} of G-code, {} thumbnails), checksums {}\n",
        h.version,
        h.blocks.len(),
        h.count(bgcode::BlockKind::GCode),
        h.count(bgcode::BlockKind::Thumbnail),
        match h.checksum {
            bgcode::Checksum::None => "not used",
            bgcode::Checksum::Crc32 =>
                if h.all_checksums_ok() {
                    "all right"
                } else {
                    "WRONG on at least one block"
                },
        }
    );
    if let Some(s) = h.estimated_seconds() {
        out.push_str(&format!("the file says the print takes {s} s\n"));
    }
    for (kind, k, v) in &h.metadata {
        out.push_str(&format!("{}: {k} = {v}\n", kind.name()));
    }
    for (i, t) in h.thumbnails.iter().enumerate() {
        out.push_str(&report::thumbnail_line(
            i + 1,
            u32::from(t.width),
            u32::from(t.height),
            t.format,
            t.bytes,
            lang,
        ));
        out.push('\n');
    }
    if let Some(path) = &thumb.out {
        let Some((format, data)) = h
            .largest_thumbnail()
            .and_then(|(i, t)| h.thumbnail_data(bytes, i).map(|d| (t.format, d)))
        else {
            return Outcome::fail(format!("{file}: no thumbnail that can be taken out"));
        };
        if let Err(e) = write_picture(path, format, data, thumb.force) {
            return Outcome::fail(e);
        }
        out.push_str(&format!(
            "{path}: the largest thumbnail ({} bytes) was written\n",
            data.len()
        ));
    }
    out.push_str("read only: the blocks of G-code were not unpacked and nothing was changed\n");
    Outcome::ok(out)
}

/// The header of a file of resin layers and the time it takes to expose.
fn resin_report(file: &str, bytes: &[u8], p: &Parsed, limits: &p3dp_core::Limits) -> Outcome {
    let job = match resin::read_job(file, bytes, limits) {
        Ok(j) => j,
        Err(e) => return Outcome::fail(format!("{file}: {e}")),
    };
    let mut settings = resin::ResinSettings::default();
    if let Some(v) = p.flags.get("bottom-layers") {
        match v.parse::<u32>() {
            Ok(n) => settings.bottom_layers = n,
            Err(_) => return Outcome::usage("--bottom-layers takes a whole number"),
        }
    }
    if let Some(v) = p.flags.get("lift") {
        match v.parse::<f64>() {
            Ok(x) => settings.lift_mm = x,
            Err(_) => return Outcome::usage("--lift takes a number of millimetres"),
        }
    }
    let est = match resin::estimate(&job, &settings) {
        Ok(e) => e,
        Err(e) => return Outcome::fail(format!("{file}: {e}")),
    };
    // with the power of the light the layers are checked one by one against the limits
    let mut light = None;
    if let Some(v) = p.flags.get("light-power") {
        let power = match v.parse::<f64>() {
            Ok(x) => x,
            Err(_) => return Outcome::usage("--light-power takes a number of mW/cm2"),
        };
        match resin_control::control_layers(
            &job,
            &settings,
            power,
            &resin_control::SectionRule::default(),
            &resin_control::ResinLimits::default(),
            None,
        ) {
            Ok(r) => light = Some((power, r)),
            Err(e) => return Outcome::fail(format!("{file}: {e}")),
        }
    }
    let mut out = format!(
        "{file}: a package of {} layers of {} mm{}\nexposure {} s ({} s for the bottom layers, {} layers of transition)\n",
        job.layers,
        job.layer_height_mm,
        if job.material.is_empty() {
            String::new()
        } else {
            format!(", material {}", job.material)
        },
        job.exposure_s,
        job.bottom_exposure_s,
        job.transition_layers
    );
    out.push_str(&format!(
        "estimate: {:.0} s of light and {:.0} s of moving and waiting, {:.0} s in all, with {} bottom layers and a lift of {} mm (an estimate, not the planner of the printer)\n",
        est.exposure_s, est.movement_s, est.total_s, settings.bottom_layers, settings.lift_mm
    ));
    if let Some((power, r)) = &light {
        let most = r.layers.iter().map(|c| c.dose_mj_cm2).fold(0.0, f64::max);
        out.push_str(&format!(
            "light: {power} mW/cm2, {:.0} mJ/cm2 in all and {most:.1} at most on one layer; every layer is inside the limits of exposure, lift, speed, waiting and dose\n",
            r.total_dose_mj_cm2
        ));
    }
    if let Some(s) = job.stated_seconds {
        out.push_str(&format!("the file says the print takes {s} s\n"));
    }
    out.push_str("read only: no layer image was decoded and nothing was changed\n");
    Outcome::ok(out)
}

fn project(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &[
            "config",
            "lang",
            "bottom-layers",
            "lift",
            "light-power",
            "thumbnail",
        ],
        &["force"],
    ) {
        Ok(p) => p,
        Err(e) => return Outcome::usage(&e),
    };
    let [file] = p.positional.as_slice() else {
        return Outcome::usage("project takes one 3MF file");
    };
    let state = match state_of(&p, env_lang) {
        Ok(s) => s,
        Err(e) => return Outcome::fail(e),
    };
    let bytes = match read_limited(file, state.config.limits.max_file_bytes) {
        Ok(b) => b,
        Err(e) => return Outcome::fail(e),
    };
    let thumb = Thumb {
        out: p.flags.get("thumbnail").cloned(),
        force: p.flags.contains_key("force"),
    };
    if bgcode::is_bgcode(&bytes) {
        return bgcode_report(file, &bytes, &state.config, state.lang, &thumb);
    }
    let ext = file
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ["gcode", "gco", "g"].contains(&ext.as_str()) {
        return gcode_report(file, &bytes, &state.config, state.lang, &thumb);
    }
    if resin::OPEN_EXTENSIONS.contains(&ext.as_str())
        || resin::CLOSED_EXTENSIONS.contains(&ext.as_str())
    {
        return resin_report(file, &bytes, &p, &state.config.limits);
    }
    let info = match read_project(&bytes, &state.config.limits) {
        Ok(i) => i,
        Err(e) => return Outcome::fail(format!("{file}: {e}")),
    };
    let mut out = format!(
        "{file}: a project of the {} family ({}), {} entries, {} items on the build\n",
        info.producer.name(),
        if info.application.is_empty() {
            "no application named"
        } else {
            &info.application
        },
        info.entries,
        info.build_items
    );
    for o in &info.objects {
        out.push_str(&format!(
            "object {}: {}{}\n",
            o.id,
            o.name,
            o.extruder
                .map_or(String::new(), |e| format!(" (extruder {e})"))
        ));
    }
    for pl in &info.plates {
        out.push_str(&format!(
            "plate {}{}: objects {}{}{}\n",
            pl.id,
            if pl.name.is_empty() {
                String::new()
            } else {
                format!(" ({})", pl.name)
            },
            pl.object_ids.join(", "),
            pl.predicted_seconds
                .map_or(String::new(), |s| format!(", predicted {s} s")),
            pl.weight_g.map_or(String::new(), |g| format!(", {g} g"))
        ));
    }
    for (k, v) in &info.settings {
        out.push_str(&format!("{k} = {v}\n"));
    }
    if info.has_gcode {
        out.push_str("the package carries G-code\n");
    }
    out.push_str("read only: nothing was changed and the model itself was not read\n");
    Outcome::ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    fn tmp(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("p3dp_cli_{}_{name}", std::process::id()));
        p.to_string_lossy().into_owned()
    }

    fn write_cube(name: &str, f: Format) -> String {
        let path = tmp(name);
        std::fs::write(&path, write_mesh(&cube(10.0), f, "c").unwrap()).unwrap();
        path
    }

    #[test]
    fn the_catalog_of_the_suite_is_listed_shown_and_gives_a_process_to_slice_with() {
        let list = run(&args("machines --search artillery"), None);
        assert_eq!(list.code, 0, "{}", list.err);
        assert!(
            list.out.contains("artillery/sidewinder-x4-pro-s1"),
            "{}",
            list.out
        );
        assert!(!list.out.contains("bambu"));
        let shown = run(&args("machine bambu-lab/a1 --lang es"), None);
        assert_eq!(shown.code, 0, "{}", shown.err);
        assert!(shown.out.contains("https://bambulab.com/en/a1/tech-specs"));
        assert!(
            shown
                .out
                .contains("Volumen de impresión: 256 x 256 x 256 mm"),
            "{}",
            shown.out
        );
        let file = tmp("process.cfg");
        let _ = std::fs::remove_file(&file);
        let made = run(
            &args(&format!("machine creality-k1c --process fine --out {file}")),
            None,
        );
        assert_eq!(made.code, 0, "{}", made.err);
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(SliceConfig::from_text(&text).unwrap().layer_height_mm, 0.1);
        let again = run(
            &args(&format!(
                "machine creality-k1c --process draft --out {file}"
            )),
            None,
        );
        assert_eq!(again.code, 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        let _ = std::fs::remove_file(&file);
        assert_eq!(run(&args("machine no-such-printer"), None).code, 1);
        assert_eq!(
            run(&args("machine bambu-lab-a1 --nozzle 0.5"), None).code,
            1
        );
        assert_eq!(run(&args("machine bambu-lab-a1 --nozzle x"), None).code, 2);
        assert_eq!(
            run(&args("machine bambu-lab-a1 --process best"), None).code,
            2
        );
        assert_eq!(run(&args("machine bambu-lab-a1 --out x.cfg"), None).code, 2);
        assert_eq!(run(&args("machine"), None).code, 2);
        // the models the suite adds are printers of the program too, in both forms of the id
        let sliced = run(
            &args("filaments --printer artillery/sidewinder-x4-plus-s1"),
            None,
        );
        assert_eq!(sliced.code, 0, "{}", sliced.err);
    }

    #[test]
    fn import_reports_writes_only_what_it_applied_and_never_replaces_the_source() {
        let source = tmp("profile.ini");
        let target = tmp("imported.cfg");
        let _ = std::fs::remove_file(&target);
        std::fs::write(
            &source,
            "layer_height = 0.16\nperimeters = 3\nprinthost_apikey = HIDDEN-9\n",
        )
        .unwrap();
        let report = run(&args(&format!("import {source}")), None);
        assert_eq!(report.code, 0, "{}", report.err);
        assert!(
            report.out.contains("layer_height_mm = 0.16"),
            "{}",
            report.out
        );
        assert!(report.out.contains("printhost_apikey"));
        assert!(!report.out.contains("HIDDEN-9"));
        assert!(!std::path::Path::new(&target).exists());
        let written = run(&args(&format!("import {source} --out {target}")), None);
        assert_eq!(written.code, 0, "{}", written.err);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.contains("perimeters = 3") && !text.contains("HIDDEN-9"));
        let kept = run(&args(&format!("import {source} --out {target}")), None);
        assert_eq!(kept.code, 1);
        let over = run(
            &args(&format!("import {source} --out {source} --force")),
            None,
        );
        assert_eq!(over.code, 1);
        assert!(std::fs::read_to_string(&source)
            .unwrap()
            .contains("HIDDEN-9"));
        std::fs::write(&source, "nozzle_diameter = 0.2\nlayer_height = 0.4\n").unwrap();
        let bad = run(
            &args(&format!("import {source} --out {target} --force")),
            None,
        );
        assert_eq!(bad.code, 1);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), text);
        std::fs::write(&source, "name = nothing of ours\n").unwrap();
        let nothing = run(
            &args(&format!("import {source} --out {target} --force")),
            None,
        );
        assert_eq!(nothing.code, 1);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), text);
        std::fs::write(&source, [0xffu8, 0xfe, 0x00]).unwrap();
        assert_eq!(run(&args(&format!("import {source}")), None).code, 1);
        assert_eq!(run(&args("import"), None).code, 2);
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn version_and_help_and_mistakes() {
        let v = run(&args("version"), None);
        assert_eq!(
            (v.code, v.out.as_str()),
            (0, format!("phoenix-3dp {}\n", crate::VERSION).as_str())
        );
        assert_eq!(run(&args("--help"), None).code, 0);
        assert!(run(&args("--help"), None).out.contains("check-profile"));
        let none = run(&[], None);
        assert_eq!(none.code, 2);
        assert!(none.err.contains("no command"));
        assert_eq!(run(&args("frobnicate"), None).code, 2);
    }

    #[test]
    fn the_list_of_printers_filters() {
        let all = run(&args("printers"), None);
        assert_eq!(all.code, 0);
        assert!(
            all.out.contains("bambu-lab-a1 ")
                && all.out.contains("bambu-lab-p1s ")
                && all.out.contains("Simulator only")
        );
        let n_all: usize = all
            .out
            .lines()
            .rev()
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(n_all >= 30);
        let resin = run(&args("printers --tech msla"), None);
        assert!(resin.out.contains("elegoo-mars-4") && !resin.out.contains("bambu-lab-a1"));
        let bambu = run(&args("printers --vendor bambu"), None);
        assert!(bambu.out.contains("11 printers"), "{}", bambu.out);
        let none = run(&args("printers --search zzzz"), None);
        assert!(none.out.contains("0 printers"));
        assert_eq!(run(&args("printers --tech sls"), None).code, 2);
        assert_eq!(run(&args("printers --bogus 1"), None).code, 2);
        assert_eq!(run(&args("printers extra"), None).code, 2);
        assert_eq!(run(&args("printers --vendor"), None).code, 2);
    }

    #[test]
    fn the_language_comes_from_the_flag_then_the_file_then_the_environment() {
        assert!(run(&args("printers --lang es"), Some("de"))
            .out
            .contains("Solo simulador"));
        assert!(run(&args("printers"), Some("de_DE.UTF-8"))
            .out
            .contains("Nur Simulator"));
        let cfg = tmp("lang.cfg");
        std::fs::write(&cfg, "lang = fr\n").unwrap();
        let a = vec!["printers".to_string(), "--config".to_string(), cfg.clone()];
        assert!(run(&a, Some("de")).out.contains("Simulateur"));
        let mut b = a.clone();
        b.extend(["--lang".to_string(), "ja".to_string()]);
        assert!(run(&b, Some("de")).out.contains("シミュレーター"));
        let _ = std::fs::remove_file(&cfg);
    }

    #[test]
    fn a_bad_settings_file_is_a_failure_that_names_the_file() {
        let cfg = tmp("bad.cfg");
        std::fs::write(&cfg, "server_retries = 99\n").unwrap();
        let r = run(
            &["printers".to_string(), "--config".to_string(), cfg.clone()],
            None,
        );
        assert_eq!(r.code, 1);
        assert!(r.err.contains("server_retries") && r.err.contains("line 1"));
        let _ = std::fs::remove_file(&cfg);
        assert_eq!(
            run(&args("printers --config /no/such/file.cfg"), None).code,
            1
        );
    }

    #[test]
    fn info_reads_the_model_and_the_printer() {
        let f = write_cube("info.stl", Format::Stl);
        let r = run(
            &[
                "info".to_string(),
                f.clone(),
                "--printer".to_string(),
                "bambu-lab-a1-mini".to_string(),
            ],
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("Triangles: 12") && r.out.contains("fits"),
            "{}",
            r.out
        );
        let bad = run(
            &[
                "info".to_string(),
                f.clone(),
                "--printer".to_string(),
                "ghost".to_string(),
            ],
            None,
        );
        assert_eq!(bad.code, 1);
        let missing = run(&args("info /no/such/model.stl"), None);
        assert_eq!(missing.code, 1);
        assert_eq!(run(&args("info"), None).code, 2);
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn check_profile_by_id_and_by_file() {
        let r = run(&args("check-profile prusa-core-one"), None);
        assert_eq!(r.code, 0);
        assert!(r.out.contains("Nozzle: 290 C") && r.out.contains("Chamber: 55 C"));
        assert_eq!(run(&args("check-profile ghost"), None).code, 1);
        let file = tmp("mine.profile");
        let p = p3dp_profiles::Catalog::builtin()
            .unwrap()
            .find("creality-k1")
            .unwrap()
            .clone();
        std::fs::write(&file, p.to_text()).unwrap();
        let r = run(
            &[
                "check-profile".to_string(),
                "--file".to_string(),
                file.clone(),
            ],
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(r.out.contains("Creality K1"));
        std::fs::write(&file, "id = x\n").unwrap();
        assert_eq!(
            run(
                &[
                    "check-profile".to_string(),
                    "--file".to_string(),
                    file.clone()
                ],
                None
            )
            .code,
            1
        );
        let _ = std::fs::remove_file(&file);
        assert_eq!(run(&args("check-profile"), None).code, 2);
    }

    #[test]
    fn check_profile_finds_the_lines_past_the_limits() {
        let g = tmp("job.gcode");
        std::fs::write(&g, "G28\nM104 S215\nM140 S60\n").unwrap();
        let ok = run(
            &[
                "check-profile".to_string(),
                "bambu-lab-a1".to_string(),
                "--gcode".to_string(),
                g.clone(),
            ],
            None,
        );
        assert_eq!(ok.code, 0, "{}", ok.err);
        std::fs::write(&g, "G28\nM104 S350\nM140 S60\nM140 S130\n").unwrap();
        let bad = run(
            &[
                "check-profile".to_string(),
                "bambu-lab-a1".to_string(),
                "--gcode".to_string(),
                g.clone(),
            ],
            None,
        );
        assert_eq!(bad.code, 1);
        assert!(
            bad.out.contains("line 2") && bad.out.contains("line 4") && !bad.out.contains("line 3"),
            "{}",
            bad.out
        );
        assert!(bad.err.contains("2 line(s)"));
        let resin = run(
            &[
                "check-profile".to_string(),
                "elegoo-mars-4".to_string(),
                "--gcode".to_string(),
                g.clone(),
            ],
            None,
        );
        assert_eq!(resin.code, 1);
        let _ = std::fs::remove_file(&g);
    }

    #[test]
    fn convert_between_every_pair_of_formats() {
        for (from, to) in [
            (Format::Stl, "obj"),
            (Format::Obj, "3mf"),
            (Format::ThreeMf, "stl"),
            (Format::Stl, "3mf"),
        ] {
            let input = write_cube(&format!("in_{to}.{}", from.extension()), from);
            let output = tmp(&format!("out.{to}"));
            let _ = std::fs::remove_file(&output);
            let r = run(
                &["convert".to_string(), input.clone(), output.clone()],
                None,
            );
            assert_eq!(r.code, 0, "{}", r.err);
            let back = run(&["info".to_string(), output.clone()], None);
            assert!(back.out.contains("Triangles: 12"), "{}", back.out);
            let _ = std::fs::remove_file(&input);
            let _ = std::fs::remove_file(&output);
        }
    }

    #[test]
    fn convert_does_not_replace_or_overwrite_the_input() {
        let input = write_cube("keep.stl", Format::Stl);
        let output = tmp("exists.obj");
        std::fs::write(&output, "mine").unwrap();
        let r = run(
            &["convert".to_string(), input.clone(), output.clone()],
            None,
        );
        assert_eq!(r.code, 1);
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "mine");
        let forced = run(
            &[
                "convert".to_string(),
                input.clone(),
                output.clone(),
                "--force".to_string(),
            ],
            None,
        );
        assert_eq!(forced.code, 0);
        assert!(std::fs::read_to_string(&output)
            .unwrap()
            .starts_with("# PHOENIX-3DP"));
        let same = run(&["convert".to_string(), input.clone(), input.clone()], None);
        assert_eq!(same.code, 1);
        assert_eq!(
            run(
                &["convert".to_string(), input.clone(), tmp("x.gcode")],
                None
            )
            .code,
            2
        );
        assert_eq!(run(&args("convert one.stl"), None).code, 2);
        assert!(std::fs::read(&input).is_ok());
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn convert_to_3mf_can_name_the_printer() {
        let input = write_cube("pr.stl", Format::Stl);
        let output = tmp("pr.3mf");
        let _ = std::fs::remove_file(&output);
        let r = run(
            &[
                "convert".to_string(),
                input.clone(),
                output.clone(),
                "--printer".to_string(),
                "prusa-mk4s".to_string(),
            ],
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        let p = crate::project::Project::from_3mf(
            &std::fs::read(&output).unwrap(),
            &p3dp_core::Limits::default(),
        )
        .unwrap();
        assert_eq!(p.printer_id, "prusa-mk4s");
        let bad = run(
            &[
                "convert".to_string(),
                input.clone(),
                tmp("pr2.3mf"),
                "--printer".to_string(),
                "ghost".to_string(),
            ],
            None,
        );
        assert_eq!(bad.code, 1);
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn damaged_models_are_failures_not_crashes() {
        let bad = tmp("bad.stl");
        std::fs::write(&bad, [1u8, 2, 3, 4, 5]).unwrap();
        assert_eq!(run(&["info".to_string(), bad.clone()], None).code, 1);
        assert_eq!(
            run(
                &["convert".to_string(), bad.clone(), tmp("never.obj")],
                None
            )
            .code,
            1
        );
        assert!(!std::path::Path::new(&tmp("never.obj")).exists());
        let _ = std::fs::remove_file(&bad);
    }
    #[test]
    fn slice_reports_writes_and_refuses() {
        let model = write_cube("slice_cube.stl", Format::Stl);
        // without --out it only reports
        let r = run(
            &args(&format!("slice {model} --printer bambu-lab-a1")),
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("50 layers")
                && r.out.contains("Bambu Lab A1")
                && r.out.contains("Simulator only"),
            "{}",
            r.out
        );
        // with --out it writes G-code that the checker of the program accepts
        let out = tmp("slice_cube.gcode");
        let w = run(
            &args(&format!("slice {model} --printer bambu-lab-a1 --out {out}")),
            None,
        );
        assert_eq!(w.code, 0, "{}", w.err);
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("M73 P0 R") && text.contains("; layer_num: 49"));
        let limits = p3dp_profiles::Catalog::builtin()
            .unwrap()
            .find("bambu-lab-a1")
            .unwrap()
            .limits
            .unwrap();
        assert!(check_program(&text, &limits).is_empty());
        // it does not replace a file without --force
        assert_eq!(
            run(
                &args(&format!("slice {model} --printer bambu-lab-a1 --out {out}")),
                None
            )
            .code,
            1
        );
        assert_eq!(
            run(
                &args(&format!(
                    "slice {model} --printer bambu-lab-a1 --out {out} --force"
                )),
                None
            )
            .code,
            0
        );
        assert_eq!(
            run(
                &args(&format!(
                    "slice {model} --printer bambu-lab-a1 --out {model} --force"
                )),
                None
            )
            .code,
            1
        );
        // a temperature over the limit in the settings is refused
        let cfg = tmp("slice_hot.cfg");
        std::fs::write(&cfg, "nozzle_c = 400\n").unwrap();
        let hot = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --settings {cfg}"
            )),
            None,
        );
        assert_eq!(hot.code, 1);
        assert!(
            hot.err.contains("400") && hot.err.contains("above the limit"),
            "{}",
            hot.err
        );
        // a setting out of its range is refused with the line
        std::fs::write(&cfg, "perimeters = 2\ninfill_percent = 500\n").unwrap();
        let bad = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --settings {cfg}"
            )),
            None,
        );
        assert!(bad.code == 1 && bad.err.contains("line 2"), "{}", bad.err);
        // mistakes of the command
        assert_eq!(run(&args(&format!("slice {model}")), None).code, 2);
        assert_eq!(run(&args("slice --printer bambu-lab-a1"), None).code, 2);
        assert_eq!(
            run(&args(&format!("slice {model} --printer nope")), None).code,
            1
        );
        // a resin printer takes no G-code
        let resin = run(
            &args(&format!("slice {model} --printer elegoo-mars-4")),
            None,
        );
        assert!(
            resin.code == 1 && resin.err.contains("not a filament printer"),
            "{}",
            resin.err
        );
        for f in [&model, &out, &cfg] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn filaments_are_listed_and_checked_against_a_printer_and_a_nozzle() {
        let r = run(&args("filaments"), None);
        assert_eq!(r.code, 0, "{}", r.err);
        for id in [
            "generic-pla",
            "generic-petg",
            "generic-abs",
            "generic-asa",
            "generic-tpu",
            "generic-pa",
        ] {
            assert!(r.out.contains(id), "{id}");
        }
        assert!(r.out.contains("none has been tried on a machine"));
        let r = run(&args("filaments --printer bambu-lab-a1-mini"), None);
        assert_eq!(r.code, 0);
        assert!(
            r.out.contains("Generic ABS")
                && r.out.contains("NOT POSSIBLE")
                && r.out.contains("the limit of the printer is 80")
        );
        assert!(r
            .out
            .contains("on Bambu Lab A1 mini with a 0.4 mm nozzle: nozzle 210 C, bed 60 C"));
        assert!(r.out.contains("direct drive"));
        let r = run(&args("filaments --printer bambu-lab-a1 --nozzle 0.5"), None);
        assert!(r.out.contains("does not list a nozzle of 0.5"));
        assert_eq!(
            run(&args("filaments --nozzle x --printer bambu-lab-a1"), None).code,
            2
        );
        assert_eq!(run(&args("filaments extra"), None).code, 2);
        assert_eq!(run(&args("filaments --printer nope"), None).code, 1);
    }

    #[test]
    fn slice_holds_the_flow_to_what_the_maker_gives_for_the_filament_and_says_so() {
        let model = tmp("flow_cube.stl");
        std::fs::write(&model, write_mesh(&cube(10.0), Format::Stl, "").unwrap()).unwrap();
        for id in ["generic-tpu", "generic-pla", "generic-petg"] {
            let r = run(
                &args(&format!(
                    "slice {model} --printer bambu-lab-a1 --filament {id}"
                )),
                None,
            );
            assert_eq!(r.code, 0, "{id}: {}", r.err);
            let limit = crate::preflight::record(&format!("generic/{id}"))
                .and_then(|f| f.mm3s_limit())
                .filter(|l| *l >= 1.0);
            match limit {
                Some(l) => assert!(
                    r.out.contains(&format!("flow: held to {l} mm3/s")),
                    "{id}: {}",
                    r.out
                ),
                None => assert!(!r.out.contains("flow: held"), "{id}: {}", r.out),
            }
        }
    }

    #[test]
    fn slice_takes_the_temperatures_from_a_filament_and_refuses_a_pair_that_does_not_fit() {
        let model = tmp("filament_cube.stl");
        std::fs::write(&model, write_mesh(&cube(10.0), Format::Stl, "").unwrap()).unwrap();
        let r = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --filament generic-petg"
            )),
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out
                .contains("filament Generic PETG: nozzle 235 C, bed 80 C, fan 40 %"),
            "{}",
            r.out
        );
        let r = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --filament generic-tpu"
            )),
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(r.out.contains("note: it is soft"), "{}", r.out);
        let r = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1-mini --filament generic-abs"
            )),
            None,
        );
        assert_eq!(r.code, 1);
        assert!(
            r.err.contains("does not go with") && r.err.contains("bed"),
            "{}",
            r.err
        );
        let r = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --filament nope"
            )),
            None,
        );
        assert_eq!(r.code, 1);
        assert!(r.err.contains("no such filament"));
        // the nozzle is part of the check: a 0.6 nozzle needs wider lines than the default settings have
        let r = run(
            &args(&format!(
                "slice {model} --printer bambu-lab-a1 --filament generic-pla --nozzle 0.6"
            )),
            None,
        );
        assert_eq!(r.code, 1);
        assert!(r.err.contains("line_width_mm"), "{}", r.err);
        assert_eq!(
            run(
                &args(&format!(
                    "slice {model} --printer bambu-lab-a1 --filament generic-pla --nozzle big"
                )),
                None
            )
            .code,
            2
        );
        let _ = std::fs::remove_file(&model);
    }

    #[test]
    fn project_reads_the_header_of_binary_gcode_and_of_a_package_of_resin_layers() {
        // a binary G-code file: the header, one block of plain text metadata and one block of G-code
        let meta = b"printer_model=GENERIC\nestimated printing time (normal mode)=1h 1m 1s\n";
        let mut f = b"GCDE".to_vec();
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&0u16.to_le_bytes());
        f.extend_from_slice(&[3, 0, 0, 0]);
        f.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        f.extend_from_slice(&[0, 0]);
        f.extend_from_slice(meta);
        f.extend_from_slice(&[1, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        f.extend_from_slice(&[0u8; 8]);
        let path = tmp("a.bgcode");
        std::fs::write(&path, &f).unwrap();
        let r = run(&args(&format!("project {path}")), None);
        assert_eq!(r.code, 0, "{}", r.out);
        assert!(r.out.contains("binary G-code") && r.out.contains("3661 s"));
        assert!(r.out.contains("printer_model = GENERIC"));
        // cut short: a refusal with the file named
        std::fs::write(&path, &f[..30]).unwrap();
        assert_eq!(run(&args(&format!("project {path}")), None).code, 1);
        let _ = std::fs::remove_file(&path);
        // a package of layers
        let mut z = p3dp_formats::zip::Writer::new();
        z.add(
            "config.ini",
            b"expTime = 2\nexpTimeFirst = 30\nlayerHeight = 0.05\nnumFade = 0\nmaterialName = Test\n",
        )
        .unwrap();
        for i in 0..20 {
            z.add(&format!("m{i:05}.png"), b"x").unwrap();
        }
        let sl1 = tmp("a.sl1");
        std::fs::write(&sl1, z.finish()).unwrap();
        let r = run(
            &args(&format!("project {sl1} --bottom-layers 2 --lift 4")),
            None,
        );
        assert_eq!(r.code, 0, "{}", r.out);
        assert!(r.out.contains("20 layers of 0.05 mm") && r.out.contains("material Test"));
        assert!(r.out.contains("with 2 bottom layers and a lift of 4 mm"));
        assert_eq!(
            run(&args(&format!("project {sl1} --lift far")), None).code,
            2
        );
        assert_eq!(
            run(&args(&format!("project {sl1} --bottom-layers x")), None).code,
            2
        );
        // with the power of the light, the doses are told and the limits are checked
        let r = run(&args(&format!("project {sl1} --light-power 3")), None);
        assert_eq!(r.code, 0, "{}", r.out);
        assert!(
            r.out.contains("light: 3 mW/cm2") && r.out.contains("90.0 at most"),
            "{}",
            r.out
        );
        let r = run(&args(&format!("project {sl1} --light-power 100")), None);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("dose_mj_cm2"), "{}", r.err);
        assert_eq!(
            run(&args(&format!("project {sl1} --light-power strong")), None).code,
            2
        );
        // the same bytes under a closed name are refused with the reason
        let ctb = tmp("a.ctb");
        std::fs::copy(&sl1, &ctb).unwrap();
        let r = run(&args(&format!("project {ctb}")), None);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("not open"), "{}", r.err);
        let _ = std::fs::remove_file(&sl1);
        let _ = std::fs::remove_file(&ctb);
    }

    #[test]
    fn project_reads_a_package_and_refuses_what_is_not_one() {
        let own = tmp("project_own.3mf");
        std::fs::write(
            &own,
            write_mesh(&cube(10.0), Format::ThreeMf, "mine").unwrap(),
        )
        .unwrap();
        let r = run(&args(&format!("project {own}")), None);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("phoenix family") && r.out.contains("read only"),
            "{}",
            r.out
        );
        let junk = tmp("project_junk.3mf");
        std::fs::write(&junk, b"not a package").unwrap();
        assert_eq!(run(&args(&format!("project {junk}")), None).code, 1);
        assert_eq!(run(&args("project"), None).code, 2);
        assert_eq!(run(&args("project a b"), None).code, 2);
        assert_eq!(run(&args("project /no/such/file.3mf"), None).code, 1);
        for f in [&own, &junk] {
            let _ = std::fs::remove_file(f);
        }
    }

    /// The base64 text of a small PNG header (12 bytes: the signature and 1 2 3 4).
    const PNG_B64: &str = "iVBORw0KGgoBAgME";
    const PNG_BYTES: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

    fn gcode_with_thumbnail(name: &str) -> String {
        let path = tmp(name);
        let text = format!(
            "; generated by a test\n; thumbnail begin 16x16 16\n; {PNG_B64}\n; thumbnail end\nG28\nG1 X1 Y1 E1\n; filament used [g] = 3.5\n; estimated printing time (normal mode) = 2m 3s\n"
        );
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn project_reads_the_figures_and_thumbnails_of_plain_gcode_and_takes_the_thumbnail_out() {
        let path = gcode_with_thumbnail("t1.gcode");
        let r = run(&args(&format!("project {path}")), None);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("plain G-code") && r.out.contains("filament_g = 3.50"),
            "{}",
            r.out
        );
        assert!(r.out.contains("seconds = 123"), "{}", r.out);
        assert!(
            r.out.contains("Thumbnail 1: 16 x 16 PNG, 12 bytes"),
            "{}",
            r.out
        );
        assert!(r.out.contains("read only"), "{}", r.out);
        // in another language
        let es = run(&args(&format!("project {path} --lang es")), None);
        assert!(
            es.out.contains("Miniatura 1: 16 x 16 PNG, 12 bytes"),
            "{}",
            es.out
        );
        // the thumbnail is taken out under a name that matches its format, and never over a file without --force
        let png = tmp("t1_out.png");
        let _ = std::fs::remove_file(&png);
        let r = run(&args(&format!("project {path} --thumbnail {png}")), None);
        assert_eq!(r.code, 0, "{}{}", r.out, r.err);
        assert_eq!(std::fs::read(&png).unwrap(), PNG_BYTES);
        let again = run(&args(&format!("project {path} --thumbnail {png}")), None);
        assert_eq!(again.code, 1);
        assert!(again.err.contains("use --force"), "{}", again.err);
        assert_eq!(
            run(
                &args(&format!("project {path} --thumbnail {png} --force")),
                None
            )
            .code,
            0
        );
        let wrong = tmp("t1_out.jpg");
        let w = run(&args(&format!("project {path} --thumbnail {wrong}")), None);
        assert_eq!(w.code, 1);
        assert!(
            w.err.contains("name the file with .png") && !std::path::Path::new(&wrong).exists(),
            "{}",
            w.err
        );
        for f in [&path, &png] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn plain_gcode_with_nothing_in_its_comments_says_so_and_a_bad_thumbnail_is_a_warning() {
        let path = tmp("t2.gcode");
        std::fs::write(&path, "G28\nG1 X1 Y1 E1\n").unwrap();
        let r = run(&args(&format!("project {path}")), None);
        assert_eq!(r.code, 0);
        assert!(
            r.out.contains("no figures of the print and no thumbnail"),
            "{}",
            r.out
        );
        let none = run(
            &args(&format!("project {path} --thumbnail {}", tmp("t2_out.png"))),
            None,
        );
        assert_eq!(none.code, 1);
        assert!(
            none.err.contains("no thumbnail that can be taken out"),
            "{}",
            none.err
        );
        std::fs::write(
            &path,
            format!("; thumbnail begin 16x16 99\n; {PNG_B64}\n; thumbnail end\n"),
        )
        .unwrap();
        let r = run(&args(&format!("project {path}")), None);
        assert!(
            r.out.contains("warning: line 1") && r.out.contains("says 99"),
            "{}",
            r.out
        );
        // the limits of the settings file are honoured: a head of 1 KiB does not reach a thumbnail after 2 KiB of comments
        let far = format!(
            "{}; thumbnail begin 16x16 16\n; {PNG_B64}\n; thumbnail end\n",
            "; filler\n".repeat(400)
        );
        std::fs::write(&path, far).unwrap();
        let cfg = tmp("t2.cfg");
        std::fs::write(&cfg, "limit_gcode_head_kib = 1\n").unwrap();
        let near = run(&args(&format!("project {path} --config {cfg}")), None);
        assert!(!near.out.contains("Thumbnail 1"), "{}", near.out);
        let wide = run(&args(&format!("project {path}")), None);
        assert!(wide.out.contains("Thumbnail 1"), "{}", wide.out);
        for f in [&path, &cfg] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn project_lists_the_thumbnails_of_binary_gcode_and_takes_the_largest_out() {
        let crc = p3dp_formats::crc32::crc32;
        let block = |kind: u16, params: &[u8], data: &[u8]| {
            let mut b = Vec::new();
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&0u16.to_le_bytes());
            b.extend_from_slice(&(data.len() as u32).to_le_bytes());
            b.extend_from_slice(params);
            b.extend_from_slice(data);
            let c = crc(&b);
            b.extend_from_slice(&c.to_le_bytes());
            b
        };
        let mut f = b"GCDE".to_vec();
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&block(5, &[0, 0, 16, 0, 16, 0], &PNG_BYTES));
        let mut big = b"qoif".to_vec();
        big.extend_from_slice(&[7; 10]);
        f.extend_from_slice(&block(5, &[2, 0, 0, 1, 0, 1], &big));
        f.extend_from_slice(&block(1, &[0, 0], b"G28 G1"));
        let path = tmp("t3.bgcode");
        std::fs::write(&path, &f).unwrap();
        let r = run(&args(&format!("project {path}")), None);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("Thumbnail 1: 16 x 16 PNG, 12 bytes"),
            "{}",
            r.out
        );
        assert!(
            r.out.contains("Thumbnail 2: 256 x 256 QOI, 14 bytes"),
            "{}",
            r.out
        );
        // the largest is the QOI one: a .png name is refused, a .qoi name works
        let bad = tmp("t3_out.png");
        let w = run(&args(&format!("project {path} --thumbnail {bad}")), None);
        assert_eq!(w.code, 1);
        assert!(w.err.contains("name the file with .qoi"), "{}", w.err);
        let qoi = tmp("t3_out.qoi");
        let _ = std::fs::remove_file(&qoi);
        assert_eq!(
            run(&args(&format!("project {path} --thumbnail {qoi}")), None).code,
            0
        );
        assert_eq!(std::fs::read(&qoi).unwrap(), big);
        for f in [&path, &qoi] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn slice_with_report_tells_the_material_the_time_and_the_layers_in_the_language_asked() {
        let model = write_cube("report_cube.stl", Format::Stl);
        let base = format!("slice {model} --printer prusa-mk4s --report");
        let r = run(&args(&base), None);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(
            r.out.contains("Material: ") && r.out.contains("Cost of the filament: "),
            "{}",
            r.out
        );
        assert!(
            r.out.contains("Time: ") && r.out.contains("Layers under 5 s: "),
            "{}",
            r.out
        );
        assert!(
            r.out.contains("Slowest layers: ") && r.out.contains("Travel without printing: "),
            "{}",
            r.out
        );
        let without = run(&args(&format!("slice {model} --printer prusa-mk4s")), None);
        assert!(
            !without.out.contains("Material: "),
            "the report is asked for"
        );
        let es = run(&args(&format!("{base} --lang es")), None);
        assert!(
            es.out.contains("Coste del filamento: ") && es.out.contains("Capas más lentas: "),
            "{}",
            es.out
        );
        // the density of the chosen filament weighs the material: PETG is denser than the PLA of the default
        let grams = |text: &str| -> f64 {
            let l = text.lines().find(|l| l.starts_with("Material: ")).unwrap();
            l.rsplit(", ")
                .next()
                .unwrap()
                .trim_end_matches(" g")
                .parse()
                .unwrap()
        };
        let petg = run(&args(&format!("{base} --filament generic-petg")), None);
        assert_eq!(petg.code, 0, "{}{}", petg.out, petg.err);
        assert!(grams(&petg.out) > grams(&r.out) * 0.95);
        let _ = std::fs::remove_file(&model);
    }

    #[test]
    fn the_resins_are_listed_by_kind_for_a_resin_printer_and_with_what_a_part_takes() {
        let r = run(&args("resins"), None);
        assert_eq!(r.code, 0, "{}", r.err);
        for id in [
            "generic-standard",
            "generic-water-washable",
            "generic-tough",
            "generic-flexible",
            "generic-castable",
            "generic-clear",
            "generic-high-detail",
        ] {
            assert!(r.out.contains(id), "{id}");
        }
        assert!(
            r.out.contains("7 resins") && r.out.contains("none has been tried on a machine"),
            "{}",
            r.out
        );
        let tough = run(&args("resins --kind tough"), None);
        assert!(
            tough.out.contains("1 resins") && !tough.out.contains("generic-clear"),
            "{}",
            tough.out
        );
        assert_eq!(run(&args("resins --kind jelly"), None).code, 2);
        assert_eq!(run(&args("resins --part-ml -3"), None).code, 2);
        assert_eq!(run(&args("resins extra"), None).code, 2);
        let part = run(&args("resins --kind standard --part-ml 10"), None);
        assert!(
            part.out
                .contains("Resin: 10.00 ml in the part, 12.00 ml with the waste"),
            "{}",
            part.out
        );
        // a filament printer takes no resin
        let fdm = run(&args("resins --printer bambu-lab-a1"), None);
        assert_eq!(fdm.code, 1);
        assert!(fdm.err.contains("takes no resin"), "{}", fdm.err);
        let cat = p3dp_profiles::Catalog::builtin().unwrap();
        let msla = cat.technology(Technology::Msla)[0].id.clone();
        let ok = run(&args(&format!("resins --printer {msla}")), None);
        assert_eq!(ok.code, 0, "{}", ok.err);
    }

    #[test]
    fn check_profile_edits_a_copy_tells_what_changed_and_saves_it_as_a_profile_file() {
        // the owner of a Bambu Lab A1 mini raises the bed limit by hand, past the 80 C of the maker
        let r = run(
            &args("check-profile bambu-lab-a1-mini --set bed_max_c=100"),
            None,
        );
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(r.out.contains("changed: bed_max_c: 80 -> 100"), "{}", r.out);
        assert!(
            r.out.contains("changed: limit_source: maker -> owner"),
            "{}",
            r.out
        );
        assert!(
            r.out.contains("note: bed_max_c raised from 80 C to 100 C"),
            "{}",
            r.out
        );
        assert!(r.out.contains("above what the maker states"), "{}", r.out);
        assert!(r.out.contains("Bed: 100 C"), "{}", r.out);
        let pt = run(
            &args("check-profile bambu-lab-a1-mini --set bed_max_c=100 --lang pt"),
            None,
        );
        assert!(
            pt.out.contains("aumentado de 80 C para 100 C"),
            "{}",
            pt.out
        );
        // a mistake names the key and changes nothing
        for (set, want, code) in [
            ("colour=red", "colour", 1),
            ("id=other", "id", 1),
            ("bed_max_c=999", "bed_max_c", 1),
            ("bed_max_c", "key=value", 2),
        ] {
            let e = run(
                &args(&format!("check-profile bambu-lab-a1-mini --set {set}")),
                None,
            );
            assert_eq!(e.code, code, "{set}: {}{}", e.out, e.err);
            assert!(e.err.contains(want), "{set}: {}", e.err);
        }
        // saved, read back by the same command, and not over a file without --force
        let file = tmp("edited_a1_mini.cfg");
        let _ = std::fs::remove_file(&file);
        let s = run(&args(&format!("check-profile bambu-lab-a1-mini --set bed_max_c=100;nozzle_max_c=290 --save {file}")), None);
        assert_eq!(s.code, 0, "{}{}", s.out, s.err);
        assert!(
            s.out.contains("saved: ") && s.out.contains("changed: nozzle_max_c: 300 -> 290"),
            "{}",
            s.out
        );
        let back = run(&args(&format!("check-profile --file {file}")), None);
        assert_eq!(back.code, 0, "{}{}", back.out, back.err);
        assert!(
            back.out.contains("Bed: 100 C") && back.out.contains("Nozzle: 290 C"),
            "{}",
            back.out
        );
        let again = run(
            &args(&format!("check-profile bambu-lab-a1-mini --save {file}")),
            None,
        );
        assert_eq!(again.code, 1);
        assert!(again.err.contains("use --force"), "{}", again.err);
        assert_eq!(
            run(
                &args(&format!(
                    "check-profile bambu-lab-a1-mini --save {file} --force"
                )),
                None
            )
            .code,
            0
        );
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn fit_tells_whether_a_job_keeps_to_the_machine_in_the_language_asked() {
        let model = format!("{}/examples/cube-20mm.stl", env!("CARGO_MANIFEST_DIR"));
        let args = |extra: &[&str]| {
            let mut v = vec!["fit".to_string(), model.clone()];
            v.extend(extra.iter().map(|s| s.to_string()));
            v
        };
        let ok = run(
            &args(&["--printer", "bambu-lab-a1", "--process", "fine"]),
            None,
        );
        assert_eq!(ok.code, 0, "{}{}", ok.out, ok.err);
        assert!(
            ok.out.contains("It fits") && ok.out.contains("28 mm3/s"),
            "{}",
            ok.out
        );
        let es = run(&args(&["--printer", "bambu-lab-p1s", "--lang", "es"]), None);
        assert!(
            es.out.contains("Cabe") && es.out.contains("filament_diameter_mm"),
            "{}",
            es.out
        );
        let unknown = run(&args(&["--printer", "prusa-mk4s"]), None);
        assert_eq!(unknown.code, 0, "{}", unknown.err);
        assert!(unknown.out.contains("are not checked"), "{}", unknown.out);
        assert_eq!(run(&args(&[]), None).code, 2);
        assert_eq!(
            run(
                &args(&["--printer", "bambu-lab-a1", "--process", "best"]),
                None
            )
            .code,
            2
        );
    }

    #[test]
    fn slice_takes_a_process_and_your_own_start_and_end_or_refuses_them_whole() {
        let dir = std::env::temp_dir().join(format!("p3dp-user-gcode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = format!("{}/examples/cube-20mm.stl", env!("CARGO_MANIFEST_DIR"));
        let start = format!(
            "{}/examples/start-prime-line.gcode",
            env!("CARGO_MANIFEST_DIR")
        );
        let end = format!("{}/examples/end-park.gcode", env!("CARGO_MANIFEST_DIR"));
        let out = dir.join("cube.gcode").to_string_lossy().into_owned();
        let run_with = |start: &str, printer: &str| {
            let v = [
                "slice",
                model.as_str(),
                "--printer",
                printer,
                "--process",
                "draft",
                "--start-gcode",
                start,
                "--end-gcode",
                end.as_str(),
                "--out",
                out.as_str(),
                "--force",
            ];
            run(&v.map(String::from), None)
        };
        for printer in ["bambu-lab-a1", "creality-ender-3-v3-se", "creality-k1c"] {
            let r = run_with(&start, printer);
            assert_eq!(r.code, 0, "{printer}: {}", r.err);
            let text = std::fs::read_to_string(&out).unwrap();
            let at = |s: &str| text.find(s).unwrap_or_else(|| panic!("{printer}: {s}"));
            assert!(at("; your start G-code") < at("TYPE:"));
            assert!(at("TYPE:") < at("; your end G-code"));
            assert!(text.contains("layers: ") && text.contains("of 0.24 mm"));
        }
        let bad = dir.join("bad.gcode");
        std::fs::write(&bad, "G28\nM104 S999\n").unwrap();
        let _ = std::fs::remove_file(&out);
        let r = run_with(&bad.to_string_lossy(), "bambu-lab-a1");
        assert_eq!(r.code, 1);
        assert!(r.err.contains("start G-code, line 2"), "{}", r.err);
        assert!(!std::path::Path::new(&out).exists(), "nothing is written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
