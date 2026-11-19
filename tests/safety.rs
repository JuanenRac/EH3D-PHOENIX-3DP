//! The promise that matters most: no temperature goes past the limits of the model, in any dialect, from any door. Every printer of the catalog, with a fixed seed.

use p3dp_core::machine::Zone;
use p3dp_core::rng::Rng;
use p3dp_core::Dialect;
use p3dp_gcode::{check_program, JobParams, Writer};
use p3dp_link::moonraker::MoonrakerDriver;
use p3dp_link::octoprint::OctoPrintDriver;
use p3dp_link::sim::{SimCore, SimMoonraker, SimOctoPrint};
use p3dp_link::{Command, PrinterDriver};
use p3dp_profiles::{Catalog, Connectivity, Technology};

fn fdm() -> Vec<p3dp_profiles::PrinterProfile> {
    Catalog::builtin()
        .unwrap()
        .profiles
        .into_iter()
        .filter(|p| p.technology == Technology::Fdm)
        .collect()
}

#[test]
fn the_writer_accepts_exactly_what_the_model_allows() {
    let mut rng = Rng::new(7_000_001);
    for p in fdm() {
        let l = p.limits.unwrap();
        for _ in 0..40 {
            let mut w = Writer::new(p.dialect, l, p.volume_mm).unwrap();
            let t = rng.range(-20.0, l.nozzle_c * 1.5);
            let ok = w.set_nozzle(t, false).is_ok();
            assert_eq!(ok, (0.0..=l.nozzle_c).contains(&t), "{} nozzle {t}", p.id);
            let b = rng.range(-20.0, l.bed_c * 1.5);
            let ok = w.set_bed(b, true).is_ok();
            assert_eq!(ok, (0.0..=l.bed_c).contains(&b), "{} bed {b}", p.id);
            let c = rng.range(0.0, 120.0);
            let ok = w.set_chamber(c, false).is_ok();
            assert_eq!(
                ok,
                l.chamber_c.is_some_and(|m| c <= m),
                "{} chamber {c}",
                p.id
            );
        }
    }
}

#[test]
fn the_checker_agrees_with_the_writer_on_every_printer() {
    let mut rng = Rng::new(7_000_002);
    for p in fdm() {
        let l = p.limits.unwrap();
        for _ in 0..30 {
            let job = JobParams {
                nozzle_c: rng.range(0.0, l.nozzle_c),
                bed_c: rng.range(0.0, l.bed_c),
                chamber_c: l.chamber_c.map(|m| rng.range(0.0, m)),
                fan_pct: rng.range(0.0, 100.0),
                estimated_minutes: Some(rng.below(1000) as u32),
            };
            let mut w = Writer::new(p.dialect, l, p.volume_mm).unwrap();
            w.preamble(&job).unwrap();
            w.layer(0, 0.2).unwrap();
            w.linear_move(
                Some(p.volume_mm[0] / 2.0),
                Some(p.volume_mm[1] / 2.0),
                Some(0.2),
                Some(1.0),
                Some(1800.0),
            )
            .unwrap();
            w.end_safe().unwrap();
            let text = w.finish().unwrap();
            assert!(check_program(&text, &l).is_empty(), "{}:\n{text}", p.id);
            // the same program asked to go a degree past the limit is refused by the writer and found by the checker
            let mut hot = job.clone();
            hot.nozzle_c = l.nozzle_c + 1.0;
            let mut w2 = Writer::new(p.dialect, l, p.volume_mm).unwrap();
            assert!(w2.preamble(&hot).is_err(), "{}", p.id);
            assert!(w2.lines().is_empty());
            assert_eq!(
                check_program(&format!("M104 S{}\n", l.nozzle_c + 1.0), &l).len(),
                1
            );
        }
    }
}

#[test]
fn every_dialect_of_the_catalog_has_a_writer_except_resin() {
    let c = Catalog::builtin().unwrap();
    for p in &c.profiles {
        let r = Writer::new(
            p.dialect,
            p.limits.unwrap_or(p3dp_core::TempLimits {
                nozzle_c: 1.0,
                bed_c: 1.0,
                chamber_c: None,
            }),
            p.volume_mm,
        );
        assert_eq!(r.is_ok(), p.dialect != Dialect::Msla, "{}", p.id);
    }
}

#[test]
fn the_drivers_of_every_reachable_printer_keep_to_its_limits() {
    let mut rng = Rng::new(7_000_003);
    for p in fdm() {
        let l = p.limits.unwrap();
        if p.has(Connectivity::Moonraker) {
            let core = SimCore::shared();
            let mut d = MoonrakerDriver::new(
                SimMoonraker::new(core.clone()),
                Default::default(),
                l,
                false,
            );
            storm(&mut d, &core, l.nozzle_c, l.bed_c, &mut rng);
            assert!(
                core.borrow().max_nozzle_target_seen <= l.nozzle_c,
                "{} moonraker",
                p.id
            );
            assert!(
                core.borrow().max_bed_target_seen <= l.bed_c,
                "{} moonraker",
                p.id
            );
        }
        if p.has(Connectivity::OctoPrint) {
            let core = SimCore::shared();
            let mut d =
                OctoPrintDriver::new(SimOctoPrint::new(core.clone()), Default::default(), l, true);
            storm(&mut d, &core, l.nozzle_c, l.bed_c, &mut rng);
            assert!(
                core.borrow().max_nozzle_target_seen <= l.nozzle_c,
                "{} octoprint",
                p.id
            );
            assert!(
                core.borrow().max_bed_target_seen <= l.bed_c,
                "{} octoprint",
                p.id
            );
        }
    }
}

fn storm(
    d: &mut dyn PrinterDriver,
    core: &p3dp_link::sim::Shared,
    nozzle: f64,
    bed: f64,
    rng: &mut Rng,
) {
    for _ in 0..60 {
        let zone = if rng.below(2) == 0 {
            Zone::Nozzle
        } else {
            Zone::Bed
        };
        let top = if zone == Zone::Nozzle { nozzle } else { bed };
        let _ = d.command(&Command::SetTemp {
            zone,
            target_c: rng.range(-10.0, top * 1.6),
        });
        let _ = d.command(&Command::Gcode {
            line: format!("M104 S{}", rng.range(0.0, top * 1.6)),
        });
        core.borrow_mut().advance(rng.range(0.0, 20.0));
        let _ = d.status();
    }
}
