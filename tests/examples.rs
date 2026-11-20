//! The files of examples/ work with the commands the README shows.

use eh3d_phoenix_3dp::cli::run;
use eh3d_phoenix_3dp::config::AppConfig;
use std::path::PathBuf;

fn path(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("examples");
    p.push(name);
    p.to_string_lossy().into_owned()
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_cube_reads_in_both_formats_and_means_the_same() {
    for f in ["cube-20mm.stl", "cube-20mm.obj"] {
        let r = run(&args(&["info", &path(f), "--printer", "prusa-mk4s"]), None);
        assert_eq!(r.code, 0, "{f}: {}", r.err);
        assert!(
            r.out.contains("Triangles: 12")
                && r.out.contains("Volume: 8.00")
                && r.out.contains("20.0 x 20.0 x 20.0")
                && r.out.contains("fits"),
            "{f}: {}",
            r.out
        );
    }
}

#[test]
fn the_settings_example_is_valid_and_says_what_the_defaults_are() {
    let text = std::fs::read_to_string(path("phoenix-3dp.cfg")).unwrap();
    let c = AppConfig::from_text(&text).unwrap();
    assert_eq!(c.default_printer, "bambu-lab-a1");
    assert_eq!(c.limits, p3dp_core::Limits::default());
    assert_eq!(c.server, p3dp_server_bridge::Settings::default());
    // the keys of the window say what the defaults are, and the example has none of the limits of a printer set
    let d = AppConfig::default();
    assert_eq!(
        (
            c.theme,
            c.grid_mm,
            c.arrange_gap_mm,
            c.arrange_on_add,
            c.ui_hints
        ),
        (
            d.theme,
            d.grid_mm,
            d.arrange_gap_mm,
            d.arrange_on_add,
            d.ui_hints
        )
    );
    assert!(c.limit_overrides.is_empty());
    let r = run(
        &args(&[
            "printers",
            "--config",
            &path("phoenix-3dp.cfg"),
            "--search",
            "k1",
        ]),
        None,
    );
    assert_eq!(r.code, 0, "{}", r.err);
}

#[test]
fn the_profile_example_checks_the_two_programs() {
    let ok = run(
        &args(&[
            "check-profile",
            "--file",
            &path("my-printer.cfg"),
            "--gcode",
            &path("job-safe.gcode"),
        ]),
        None,
    );
    assert_eq!(ok.code, 0, "{}", ok.err);
    assert!(ok.out.contains("Corner Klipper") && ok.out.contains("no temperature goes past"));
    let bad = run(
        &args(&[
            "check-profile",
            "--file",
            &path("my-printer.cfg"),
            "--gcode",
            &path("job-too-hot.gcode"),
        ]),
        None,
    );
    assert_eq!(bad.code, 1);
    assert!(
        bad.out.contains("line 3") && bad.out.contains("line 4"),
        "{}",
        bad.out
    );
}

#[test]
fn convert_runs_on_the_examples() {
    let out = std::env::temp_dir().join(format!("p3dp_examples_{}.3mf", std::process::id()));
    let out_s = out.to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&out);
    let r = run(
        &args(&[
            "convert",
            &path("cube-20mm.stl"),
            &out_s,
            "--printer",
            "bambu-lab-a1",
        ]),
        None,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(run(&args(&["info", &out_s]), None).code, 0);
    let _ = std::fs::remove_file(&out);
}
