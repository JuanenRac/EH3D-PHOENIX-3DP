//! The command line: `printers`, `info`, `check-profile`, `convert`, `slice`, `project` and `version`. `run` returns what to print and the exit code, so it is tested without a terminal.
//! Exit code 0 is success, 1 is a failure of the thing that was asked (a check that did not pass, a file that could not be read) and 2 is a mistake in the command.

use crate::app_state::{profile_report, AppState};
use crate::config::AppConfig;
use crate::i18n::{Lang, Msg};
use p3dp_formats::project3mf::read_project;
use p3dp_formats::{bgcode, resin, resin_control};
use p3dp_formats::{write_mesh, Format};
use p3dp_gcode::check_program;
use p3dp_profiles::filament::{FilamentCatalog, Nozzle};
use p3dp_profiles::{PrinterProfile, Technology};
use p3dp_slicer::{Machine, SliceConfig};
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
  phoenix-3dp info MODEL [--printer ID]
  phoenix-3dp check-profile ID [--gcode FILE]
  phoenix-3dp check-profile --file PROFILE [--gcode FILE]
  phoenix-3dp convert IN OUT [--printer ID] [--force]
  phoenix-3dp filaments [--printer ID] [--nozzle MM] [--hardened]
  phoenix-3dp slice MODEL --printer ID [--settings FILE] [--filament ID] [--nozzle MM] [--hardened] [--out FILE.gcode] [--force]
  phoenix-3dp project FILE.3mf|FILE.bgcode|FILE.sl1 [--bottom-layers N] [--lift MM] [--light-power MW]
  phoenix-3dp version

Every command also takes --config FILE (the settings) and --lang CODE (en, es, de, fr, it, ja, zh, pt).
The models are STL, OBJ and 3MF; OUT is chosen by its extension (.stl, .obj, .3mf).
`slice` makes the layers and the G-code of a closed model for a printer of the catalog and refuses, never cuts down, what does not fit or goes past a limit;
without --out it only reports. `project` reads the objects, plates and settings of a 3MF project saved by another program, the header of a binary G-code file, or the header
of an open package of resin layers (.sl1) with an estimate of its time; all of it read only, and the closed formats of resin printers are refused with the reason.";

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
        "info" => info(rest, env_lang),
        "check-profile" => check_profile(rest, env_lang),
        "convert" => convert(rest, env_lang),
        "filaments" => filaments(rest, env_lang),
        "slice" => slice(rest, env_lang),
        "project" => project(rest, env_lang),
        other => Outcome::usage(&format!("{other:?} is not a command")),
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
    let p = match parse(args, &["file", "gcode", "config", "lang"], &[]) {
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
    let mut out = String::new();
    for l in profile_report(&profile, state.lang) {
        out.push_str(&l);
        out.push('\n');
    }
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

fn slice(args: &[String], env_lang: Option<&str>) -> Outcome {
    let p = match parse(
        args,
        &[
            "printer", "settings", "out", "config", "lang", "filament", "nozzle",
        ],
        &["force", "hardened"],
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
    let mut config = match p.flags.get("settings") {
        None => SliceConfig::default(),
        Some(path) => match read_limited(path, p3dp_core::kv::MAX_BYTES as u64)
            .and_then(|b| String::from_utf8(b).map_err(|_| format!("{path}: not UTF-8")))
            .and_then(|t| SliceConfig::from_text(&t).map_err(|e| format!("{path}: {e}")))
        {
            Ok(c) => c,
            Err(e) => return Outcome::fail(e),
        },
    };
    let mut notes = String::new();
    if let Some(fid) = p.flags.get("filament") {
        let nozzle = match nozzle_of(&p, &profile) {
            Ok(n) => n,
            Err(e) => return Outcome::usage(&e),
        };
        let filaments = match FilamentCatalog::builtin() {
            Ok(c) => c,
            Err(e) => return Outcome::fail(e.to_string()),
        };
        let Some(f) = filaments.find(fid) else {
            return Outcome::fail(format!(
                "{fid}: no such filament (see `phoenix-3dp filaments`)"
            ));
        };
        match config.apply_filament(f, &profile, nozzle) {
            Ok(compat) => {
                notes.push_str(&format!(
                    "filament {}: nozzle {} C, bed {} C, fan {} %\n",
                    f.name, config.nozzle_c, config.bed_c, config.fan_percent
                ));
                for a in compat.advice {
                    notes.push_str(&format!("note: {}\n", a.text()));
                }
            }
            Err(e) => return Outcome::fail(e.to_string()),
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
    let (plan, gcode) = match p3dp_slicer::slice(mesh, &machine, &config) {
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

/// The header of a binary G-code file, as lines.
fn bgcode_report(file: &str, bytes: &[u8], limits: &p3dp_core::Limits) -> Outcome {
    let h = match bgcode::read_header(bytes, limits) {
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
        &["config", "lang", "bottom-layers", "lift", "light-power"],
        &[],
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
    if bgcode::is_bgcode(&bytes) {
        return bgcode_report(file, &bytes, &state.config.limits);
    }
    let ext = file
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
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
}
