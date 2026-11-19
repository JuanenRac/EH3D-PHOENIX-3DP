//! The command line: `printers`, `info`, `check-profile`, `convert` and `version`. `run` returns what to print and the exit code, so it is tested without a terminal.
//! Exit code 0 is success, 1 is a failure of the thing that was asked (a check that did not pass, a file that could not be read) and 2 is a mistake in the command.

use crate::app_state::{profile_report, AppState};
use crate::config::AppConfig;
use crate::i18n::{Lang, Msg};
use p3dp_formats::{write_mesh, Format};
use p3dp_gcode::check_program;
use p3dp_profiles::{PrinterProfile, Technology};
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
  phoenix-3dp version

Every command also takes --config FILE (the settings) and --lang CODE (en, es, de, fr, it, ja, zh, pt).
The models are STL, OBJ and 3MF; OUT is chosen by its extension (.stl, .obj, .3mf).";

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
        other => Outcome::usage(&format!("{other:?} is not a command")),
    }
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
        assert!(bambu.out.contains("5 printers"), "{}", bambu.out);
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
}
