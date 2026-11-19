//! From a model to a program to a printer, all in memory, through the public doors of the crates.

use eh3d_phoenix_3dp::app_state::{demo_device, AppState};
use eh3d_phoenix_3dp::cli;
use eh3d_phoenix_3dp::config::AppConfig;
use eh3d_phoenix_3dp::i18n::Lang;
use eh3d_phoenix_3dp::project::Project;
use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Limits, Mesh, Nm, Point3, Secret};
use p3dp_formats::{read_mesh, write_mesh, Format};
use p3dp_gcode::{check_program, JobParams, Writer};
use p3dp_link::moonraker::MoonrakerDriver;
use p3dp_link::sim::{SimCore, SimMoonraker};
use p3dp_link::{JobState, PrinterDriver};
use p3dp_profiles::{Catalog, Technology};

#[test]
fn a_model_becomes_a_program_that_a_simulated_printer_runs() {
    let cat = Catalog::builtin().unwrap();
    let p = cat.find("creality-k1").unwrap();
    let l = p.limits.unwrap();
    let model = read_mesh(
        &write_mesh(&cube(20.0), Format::Stl, "").unwrap(),
        None,
        &Limits::default(),
    )
    .unwrap();
    assert!(p.fits(&model));
    let mut w = Writer::new(p.dialect, l, p.volume_mm).unwrap();
    w.header("PHOENIX-3DP", eh3d_phoenix_3dp::VERSION, &p.id);
    w.preamble(&JobParams {
        nozzle_c: 215.0,
        bed_c: 60.0,
        chamber_c: None,
        fan_pct: 100.0,
        estimated_minutes: None,
    })
    .unwrap();
    w.layer(0, 0.2).unwrap();
    w.linear_move(Some(10.0), Some(10.0), Some(0.2), Some(0.5), Some(1200.0))
        .unwrap();
    w.end_safe().unwrap();
    let program = w.finish().unwrap();
    assert!(program.contains("PRINT_START BED=60 EXTRUDER=215"));
    assert!(check_program(&program, &l).is_empty());
    let core = SimCore::shared();
    core.borrow_mut().total_s = 60.0;
    let mut d = MoonrakerDriver::new(SimMoonraker::new(core.clone()), Secret::default(), l, false);
    d.upload("model.gcode", program.as_bytes(), true).unwrap();
    assert_eq!(d.status().unwrap().state, JobState::Printing);
    core.borrow_mut().advance(61.0);
    assert_eq!(d.status().unwrap().state, JobState::Finished);
    assert_eq!(core.borrow().files[0].1, program.as_bytes());
}

#[test]
fn a_chain_of_conversions_keeps_the_volume() {
    let mut rng = Rng::new(31_415);
    for _ in 0..25 {
        let side = rng.range(1.0, 150.0);
        let mut m = cube(side);
        let (dx, dy, dz) = (
            rng.range(-80.0, 80.0),
            rng.range(-80.0, 80.0),
            rng.range(0.0, 80.0),
        );
        for v in &mut m.vertices {
            *v = Point3::new(
                v.x + Nm::from_mm(dx).unwrap(),
                v.y + Nm::from_mm(dy).unwrap(),
                v.z + Nm::from_mm(dz).unwrap(),
            );
        }
        let want = m.volume_mm3();
        let mut current: Mesh = m;
        for f in [Format::ThreeMf, Format::Obj, Format::Stl, Format::ThreeMf] {
            let bytes = write_mesh(&current, f, "chain").unwrap();
            current = read_mesh(&bytes, None, &Limits::default()).unwrap();
            assert_eq!(current.triangles.len(), 12);
            assert!(
                (current.volume_mm3() - want).abs() / want < 1e-3,
                "{f:?}: {} against {want}",
                current.volume_mm3()
            );
        }
    }
}

#[test]
fn a_project_keeps_its_printer_through_the_package() {
    let cat = Catalog::builtin().unwrap();
    for id in [
        "bambu-lab-a1",
        "bambu-lab-p1s",
        "prusa-mk4s",
        "voron-2-4-300",
    ] {
        let p = Project::new("Test <part> & 1", id, cube(10.0));
        let back = Project::from_3mf(&p.to_3mf().unwrap(), &Limits::default()).unwrap();
        assert_eq!(back.printer_id, id);
        assert!(cat.find(&back.printer_id).is_some());
        assert_eq!(back.name, "Test <part> & 1");
    }
}

#[test]
fn every_language_of_the_command_line_answers() {
    for l in Lang::ALL {
        let r = cli::run(
            &[
                "printers".to_string(),
                "--search".to_string(),
                "bambu".to_string(),
                "--lang".to_string(),
                l.code().to_string(),
            ],
            None,
        );
        assert_eq!(r.code, 0);
        assert!(r.out.contains("bambu-lab-p1s"));
        let last = r.out.lines().last().unwrap();
        assert!(!last.is_empty());
    }
}

#[test]
fn the_window_state_reports_every_printer_without_a_panic() {
    let cat = Catalog::builtin().unwrap();
    let mut s = AppState::new(AppConfig::default(), Lang::Pt).unwrap();
    for p in &cat.profiles {
        assert!(s.select_printer(&p.id));
        assert!(!s.printer_report().is_empty(), "{}", p.id);
        let d = demo_device(p);
        assert_eq!(d.is_ok(), p.technology == Technology::Fdm, "{}", p.id);
    }
}

#[test]
fn the_bambu_printers_of_the_owner_are_there_and_marked_for_what_they_are() {
    let cat = Catalog::builtin().unwrap();
    for id in ["bambu-lab-a1", "bambu-lab-p1s"] {
        let p = cat.find(id).unwrap();
        assert!(!p.verified);
        assert!(p3dp_gcode::requires_package(p.dialect));
        let mut s = AppState::new(AppConfig::default(), Lang::En).unwrap();
        s.select_printer(id);
        assert!(s
            .printer_report()
            .iter()
            .any(|l| l.contains("experimental")));
    }
}
