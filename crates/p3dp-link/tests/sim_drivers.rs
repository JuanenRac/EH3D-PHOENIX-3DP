//! The two drivers against the simulators, with the same scenarios: what the program sees must be the same whichever protocol is behind it.

use p3dp_core::machine::Zone;
use p3dp_core::rng::Rng;
use p3dp_core::{Secret, TempLimits};
use p3dp_link::moonraker::MoonrakerDriver;
use p3dp_link::octoprint::OctoPrintDriver;
use p3dp_link::retry::{CountingSleeper, Retrying};
use p3dp_link::sim::{Fault, Shared, SimCore, SimMoonraker, SimOctoPrint, SimState};
use p3dp_link::{Command, JobState, LinkError, PrinterDriver};

const LIMITS: TempLimits = TempLimits {
    nozzle_c: 300.0,
    bed_c: 100.0,
    chamber_c: Some(50.0),
};

type Pair = (Box<dyn PrinterDriver>, Shared);

fn moonraker(key: &str, raw: bool) -> Pair {
    let core = SimCore::shared();
    let d = MoonrakerDriver::new(
        SimMoonraker::new(core.clone()),
        Secret::new(key),
        LIMITS,
        raw,
    );
    (Box::new(d), core)
}

fn octoprint(key: &str, raw: bool) -> Pair {
    let core = SimCore::shared();
    let d = OctoPrintDriver::new(
        SimOctoPrint::new(core.clone()),
        Secret::new(key),
        LIMITS,
        raw,
    );
    (Box::new(d), core)
}

fn both(key: &str, raw: bool) -> Vec<(&'static str, Pair)> {
    vec![
        ("moonraker", moonraker(key, raw)),
        ("octoprint", octoprint(key, raw)),
    ]
}

#[test]
fn a_whole_job_from_upload_to_cancel() {
    for (name, (mut d, core)) in both("", false) {
        core.borrow_mut().total_s = 100.0;
        assert_eq!(d.status().unwrap().state, JobState::Idle, "{name}");
        d.upload("benchy.gcode", b"G28\nM84\n", true).unwrap();
        assert_eq!(
            d.list_files().unwrap(),
            vec!["benchy.gcode".to_string()],
            "{name}"
        );
        let s = d.status().unwrap();
        assert_eq!(s.state, JobState::Printing, "{name}");
        core.borrow_mut().advance(25.0);
        let s = d.status().unwrap();
        let job = s.job.expect("a job while printing");
        assert_eq!(job.file, "benchy.gcode", "{name}");
        assert!(
            (job.progress_pct - 25.0).abs() < 0.5,
            "{name}: {}",
            job.progress_pct
        );
        assert_eq!(job.elapsed_s, Some(25), "{name}");
        assert!(
            job.remaining_s.is_some_and(|r| (74..=76).contains(&r)),
            "{name}: {:?}",
            job.remaining_s
        );
        d.command(&Command::Pause).unwrap();
        assert_eq!(d.status().unwrap().state, JobState::Paused, "{name}");
        d.command(&Command::Resume).unwrap();
        assert_eq!(d.status().unwrap().state, JobState::Printing, "{name}");
        d.command(&Command::Cancel).unwrap();
        assert_eq!(core.borrow().state, SimState::Cancelled, "{name}");
        assert!(
            d.command(&Command::Pause).is_err(),
            "{name}: pausing nothing must fail"
        );
    }
}

#[test]
fn a_job_started_from_a_file_that_is_already_there() {
    for (name, (mut d, core)) in both("", false) {
        d.upload("a.gcode", b"G28\n", false).unwrap();
        assert_eq!(d.status().unwrap().state, JobState::Idle, "{name}");
        d.command(&Command::Start {
            file: "a.gcode".into(),
        })
        .unwrap();
        assert_eq!(core.borrow().state, SimState::Printing, "{name}");
        assert!(
            d.command(&Command::Start {
                file: "missing.gcode".into()
            })
            .is_err(),
            "{name}"
        );
        assert!(
            d.command(&Command::Start {
                file: "../etc.gcode".into()
            })
            .is_err(),
            "{name}"
        );
    }
}

#[test]
fn temperatures_follow_the_commands_and_the_limits_hold() {
    for (name, (mut d, core)) in both("", false) {
        d.command(&Command::SetTemp {
            zone: Zone::Nozzle,
            target_c: 215.0,
        })
        .unwrap();
        d.command(&Command::SetTemp {
            zone: Zone::Bed,
            target_c: 60.0,
        })
        .unwrap();
        for _ in 0..100 {
            core.borrow_mut().advance(1.0);
        }
        let s = d.status().unwrap();
        let n = s.temp(Zone::Nozzle).unwrap();
        assert_eq!((n.actual, n.target), (215.0, 215.0), "{name}");
        assert_eq!(s.temp(Zone::Bed).unwrap().target, 60.0, "{name}");
        let before = core.borrow().requests.len();
        for bad in [301.0, 1000.0, f64::NAN, -1.0] {
            let r = d.command(&Command::SetTemp {
                zone: Zone::Nozzle,
                target_c: bad,
            });
            assert!(matches!(r, Err(LinkError::Temp(_))), "{name} {bad}");
        }
        assert!(
            d.command(&Command::SetTemp {
                zone: Zone::Bed,
                target_c: 100.5
            })
            .is_err(),
            "{name}"
        );
        assert_eq!(
            core.borrow().requests.len(),
            before,
            "{name}: a refused command must send nothing"
        );
        assert!(core.borrow().max_nozzle_target_seen <= 300.0, "{name}");
    }
}

#[test]
fn the_chamber_depends_on_the_protocol() {
    let (mut d, core) = octoprint("", false);
    d.command(&Command::SetTemp {
        zone: Zone::Chamber,
        target_c: 40.0,
    })
    .unwrap();
    assert_eq!(core.borrow().chamber_target, 40.0);
    assert!(d
        .command(&Command::SetTemp {
            zone: Zone::Chamber,
            target_c: 51.0
        })
        .is_err());
    let (mut m, _) = moonraker("", false);
    assert!(matches!(
        m.command(&Command::SetTemp {
            zone: Zone::Chamber,
            target_c: 40.0
        }),
        Err(LinkError::Capability(_))
    ));
}

#[test]
fn an_emergency_stop_shuts_the_printer_down() {
    for (name, (mut d, core)) in both("", false) {
        d.command(&Command::SetTemp {
            zone: Zone::Nozzle,
            target_c: 200.0,
        })
        .unwrap();
        d.command(&Command::EmergencyStop).unwrap();
        assert_eq!(core.borrow().state, SimState::Shutdown, "{name}");
        core.borrow_mut().advance(1.0);
        assert!(d.status().unwrap().state.needs_attention(), "{name}");
        assert_eq!(core.borrow().nozzle_target, 0.0, "{name}");
    }
}

#[test]
fn the_key_is_needed_and_wrong_keys_are_refused() {
    for (name, (mut d, core)) in both("secret", false) {
        core.borrow_mut().api_key = "secret".into();
        assert!(d.status().is_ok(), "{name}");
    }
    for (name, (mut d, core)) in [
        ("moonraker", moonraker("wrong", false)),
        ("octoprint", octoprint("wrong", false)),
    ] {
        core.borrow_mut().api_key = "secret".into();
        assert_eq!(d.status().unwrap_err(), LinkError::Auth, "{name}");
        assert_eq!(d.list_files().unwrap_err(), LinkError::Auth, "{name}");
        assert_eq!(
            d.command(&Command::Pause).unwrap_err(),
            LinkError::Auth,
            "{name}"
        );
    }
}

#[test]
fn a_printer_that_cannot_be_reached_is_offline_not_an_error() {
    for (name, (mut d, core)) in both("", false) {
        core.borrow_mut().faults.push_back(Fault::NoConnection);
        assert_eq!(d.status().unwrap().state, JobState::Offline, "{name}");
        core.borrow_mut().faults.push_back(Fault::Timeout);
        assert_eq!(d.status().unwrap().state, JobState::Offline, "{name}");
        core.borrow_mut()
            .faults
            .push_back(Fault::Status(500, "boom".into()));
        assert!(
            matches!(d.status(), Err(LinkError::Http { status: 500, .. })),
            "{name}"
        );
        assert_eq!(d.status().unwrap().state, JobState::Idle, "{name}");
    }
}

#[test]
fn octoprint_without_a_connected_printer() {
    let (mut d, core) = octoprint("", false);
    core.borrow_mut().connected = false;
    let s = d.status().unwrap();
    assert_eq!(s.state, JobState::Offline);
    assert!(!s.alerts.is_empty());
}

#[test]
fn raw_gcode_is_off_unless_switched_on_and_keeps_to_the_limits() {
    for (name, (mut d, core)) in both("", false) {
        let r = d.command(&Command::Gcode { line: "G28".into() });
        assert!(matches!(r, Err(LinkError::Disabled(_))), "{name}");
        assert!(core.borrow().gcode_log.is_empty(), "{name}");
    }
    for (name, (mut d, core)) in both("", true) {
        d.command(&Command::Gcode { line: "G28".into() }).unwrap();
        assert_eq!(core.borrow().gcode_log, vec!["G28".to_string()], "{name}");
        assert!(
            d.command(&Command::Gcode {
                line: "M104 S350".into()
            })
            .is_err(),
            "{name}"
        );
        assert!(
            d.command(&Command::Gcode {
                line: "G28\nM104 S350".into()
            })
            .is_err(),
            "{name}"
        );
        assert_eq!(core.borrow().max_nozzle_target_seen, 0.0, "{name}");
    }
}

#[test]
fn both_protocols_tell_the_program_the_same_thing() {
    let (mut m, mc) = moonraker("", false);
    let (mut o, oc) = octoprint("", false);
    for (d, c) in [(&mut m, &mc), (&mut o, &oc)] {
        c.borrow_mut().total_s = 200.0;
        d.upload("same.gcode", b"G28\n", true).unwrap();
        d.command(&Command::SetTemp {
            zone: Zone::Nozzle,
            target_c: 180.0,
        })
        .unwrap();
        c.borrow_mut().advance(20.0);
    }
    let (a, b) = (m.status().unwrap(), o.status().unwrap());
    assert_eq!(a.state, b.state);
    assert_eq!(a.temp(Zone::Nozzle), b.temp(Zone::Nozzle));
    assert_eq!(a.temp(Zone::Bed), b.temp(Zone::Bed));
    let (ja, jb) = (a.job.unwrap(), b.job.unwrap());
    assert_eq!(ja.file, jb.file);
    assert!((ja.progress_pct - jb.progress_pct).abs() < 0.1);
    assert_eq!(ja.elapsed_s, jb.elapsed_s);
}

#[test]
fn retries_hide_a_flaky_network_for_reads_but_never_repeat_a_post() {
    let core = SimCore::shared();
    let flaky = Retrying::new(
        SimMoonraker::new(core.clone()),
        CountingSleeper::default(),
        3,
        11,
    );
    let mut d = MoonrakerDriver::new(flaky, Secret::default(), LIMITS, false);
    core.borrow_mut().faults.push_back(Fault::NoConnection);
    core.borrow_mut().faults.push_back(Fault::Timeout);
    assert_eq!(d.status().unwrap().state, JobState::Idle);
    assert_eq!(core.borrow().requests.len(), 3);
    // a timeout on a POST is not sent again
    core.borrow_mut().faults.push_back(Fault::Timeout);
    let before = core.borrow().requests.len();
    assert!(d.command(&Command::Pause).is_err());
    assert_eq!(core.borrow().requests.len(), before + 1);
}

#[test]
fn a_random_storm_of_commands_never_goes_past_the_limits() {
    let mut rng = Rng::new(20_261_118);
    for (name, (mut d, core)) in both("", true) {
        d.upload("storm.gcode", b"G28\n", false).unwrap();
        for _ in 0..1500 {
            let cmd = match rng.below(9) {
                0 => Command::Pause,
                1 => Command::Resume,
                2 => Command::Cancel,
                3 => Command::Start {
                    file: "storm.gcode".into(),
                },
                4 => Command::SetTemp {
                    zone: Zone::Nozzle,
                    target_c: rng.range(-50.0, 500.0),
                },
                5 => Command::SetTemp {
                    zone: Zone::Bed,
                    target_c: rng.range(-50.0, 200.0),
                },
                6 => Command::SetFan {
                    percent: rng.range(-20.0, 140.0),
                },
                7 => Command::Gcode {
                    line: format!("M104 S{}", rng.range(0.0, 600.0)),
                },
                _ => Command::SetTemp {
                    zone: Zone::Chamber,
                    target_c: rng.range(-5.0, 80.0),
                },
            };
            let _ = d.command(&cmd);
            core.borrow_mut().advance(rng.range(0.0, 30.0));
            let s = d.status().unwrap();
            assert_ne!(
                s.state,
                JobState::Emergency,
                "{name}: the firmware must never have been pushed to its own maximum"
            );
            let c = core.borrow();
            assert!(
                c.max_nozzle_target_seen <= 300.0,
                "{name}: {}",
                c.max_nozzle_target_seen
            );
            assert!(
                c.max_bed_target_seen <= 100.0,
                "{name}: {}",
                c.max_bed_target_seen
            );
            assert!(c.chamber_target <= 50.0, "{name}");
        }
    }
}
