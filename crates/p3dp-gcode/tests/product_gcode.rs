//! The G-code writer and the checker at their exact edges: the lines of each dialect, the values that are refused with nothing written, and the commands the checker reads.

use p3dp_core::machine::{TempError, Zone};
use p3dp_core::{Dialect, TempLimits};
use p3dp_gcode::writer::num;
use p3dp_gcode::{check_program, required_macros, requires_package, GcodeError, JobParams, Writer};

const LIMITS: TempLimits = TempLimits {
    nozzle_c: 300.0,
    bed_c: 100.0,
    chamber_c: None,
};

const WARM: TempLimits = TempLimits {
    nozzle_c: 300.0,
    bed_c: 100.0,
    chamber_c: Some(60.0),
};

const VOLUME: [f64; 3] = [220.0, 210.0, 250.0];

fn writer(d: Dialect) -> Writer {
    Writer::new(d, LIMITS, VOLUME).unwrap()
}

fn text(w: &Writer) -> Vec<&str> {
    w.lines().iter().map(String::as_str).collect()
}

fn job(nozzle: f64, bed: f64, chamber: Option<f64>) -> JobParams {
    JobParams {
        nozzle_c: nozzle,
        bed_c: bed,
        chamber_c: chamber,
        fan_pct: 0.0,
        estimated_minutes: None,
    }
}

// ---- the numbers ----

#[test]
fn num_writes_the_shortest_text_with_three_decimals() {
    assert_eq!(num(0.0), "0");
    assert_eq!(num(100.0), "100");
    assert_eq!(num(1000.0), "1000");
    assert_eq!(num(20.5), "20.5");
    assert_eq!(num(0.2), "0.2");
    assert_eq!(num(-1.5), "-1.5");
    assert_eq!(num(0.0004), "0");
    assert_eq!(num(-0.0004), "0");
}

// ---- making a writer ----

#[test]
fn a_resin_printer_gets_no_gcode_writer() {
    let e = Writer::new(Dialect::Msla, LIMITS, VOLUME).unwrap_err();
    assert!(matches!(e, GcodeError::Dialect(_)));
    assert!(e.to_string().contains("layers"));
}

#[test]
fn a_build_volume_that_is_not_positive_and_finite_is_refused() {
    for bad in [
        [0.0, 210.0, 250.0],
        [220.0, -1.0, 250.0],
        [220.0, 210.0, f64::NAN],
        [f64::INFINITY, 210.0, 250.0],
    ] {
        let e = Writer::new(Dialect::Marlin, LIMITS, bad).unwrap_err();
        assert!(matches!(e, GcodeError::Value(_)), "{bad:?}");
    }
}

#[test]
fn every_dialect_but_resin_makes_a_writer_that_knows_its_dialect() {
    for d in Dialect::ALL {
        let made = Writer::new(d, LIMITS, VOLUME);
        assert_eq!(made.is_ok(), d != Dialect::Msla, "{d}");
        if let Ok(w) = made {
            assert_eq!(w.dialect(), d);
            assert!(w.lines().is_empty());
        }
    }
}

#[test]
fn macros_and_packages_are_asked_by_the_dialects_that_need_them() {
    assert_eq!(
        required_macros(Dialect::Klipper),
        ["PRINT_START", "PRINT_END"]
    );
    for d in [Dialect::Marlin, Dialect::Rrf, Dialect::Bambu, Dialect::Msla] {
        assert!(required_macros(d).is_empty(), "{d}");
    }
    assert!(requires_package(Dialect::Bambu));
    for d in [
        Dialect::Marlin,
        Dialect::Klipper,
        Dialect::Rrf,
        Dialect::Msla,
    ] {
        assert!(!requires_package(d), "{d}");
    }
}

#[test]
fn the_error_of_a_temperature_is_shown_as_the_temperature_error() {
    let e: GcodeError = TempError::NoChamber(40.0).into();
    assert_eq!(e.to_string(), TempError::NoChamber(40.0).to_string());
    assert_eq!(e, GcodeError::Temp(TempError::NoChamber(40.0)));
}

// ---- temperatures ----

#[test]
fn nozzle_lines_of_marlin_klipper_and_bambu() {
    for d in [Dialect::Marlin, Dialect::Klipper, Dialect::Bambu] {
        let mut w = writer(d);
        w.set_nozzle(210.0, false).unwrap();
        w.set_nozzle(210.0, true).unwrap();
        w.set_nozzle(0.0, true).unwrap();
        assert_eq!(text(&w), ["M104 S210", "M109 S210", "M104 S0"], "{d}");
    }
}

#[test]
fn nozzle_lines_of_reprapfirmware_wait_with_a_separate_command() {
    let mut w = writer(Dialect::Rrf);
    w.set_nozzle(210.0, false).unwrap();
    w.set_nozzle(215.0, true).unwrap();
    w.set_nozzle(0.0, true).unwrap();
    assert_eq!(
        text(&w),
        ["G10 P0 S210", "G10 P0 S215", "M116", "G10 P0 S0"]
    );
}

#[test]
fn nozzle_limit_is_exact_and_a_refused_value_writes_nothing() {
    let mut w = writer(Dialect::Marlin);
    w.set_nozzle(300.0, false).unwrap();
    assert_eq!(w.lines().len(), 1);
    for bad in [300.001, -0.001, f64::NAN, f64::INFINITY] {
        assert!(matches!(w.set_nozzle(bad, true), Err(GcodeError::Temp(_))));
    }
    assert_eq!(w.lines().len(), 1);
}

#[test]
fn bed_lines_wait_only_when_something_is_heated() {
    let mut w = writer(Dialect::Marlin);
    w.set_bed(60.0, false).unwrap();
    w.set_bed(60.0, true).unwrap();
    w.set_bed(0.0, true).unwrap();
    assert_eq!(text(&w), ["M140 S60", "M190 S60", "M140 S0"]);
    assert!(w.set_bed(100.5, false).is_err());
    assert!(w.set_bed(100.0, false).is_ok());
}

#[test]
fn chamber_lines_exist_only_on_a_printer_with_a_chamber() {
    let mut none = writer(Dialect::Marlin);
    none.set_chamber(0.0, true).unwrap();
    assert!(none.lines().is_empty());
    assert!(matches!(
        none.set_chamber(30.0, false),
        Err(GcodeError::Temp(TempError::NoChamber(_)))
    ));
    let mut w = Writer::new(Dialect::Marlin, WARM, VOLUME).unwrap();
    w.set_chamber(40.0, false).unwrap();
    w.set_chamber(40.0, true).unwrap();
    w.set_chamber(0.0, true).unwrap();
    assert_eq!(text(&w), ["M141 S40", "M191 S40", "M141 S0"]);
    assert!(w.set_chamber(60.5, false).is_err());
}

// ---- the fan ----

#[test]
fn the_fan_is_scaled_to_255_and_rounded() {
    let mut w = writer(Dialect::Marlin);
    for p in [100.0, 50.0, 1.0, 0.1, 0.0] {
        w.set_fan(p).unwrap();
    }
    assert_eq!(
        text(&w),
        ["M106 S255", "M106 S128", "M106 S3", "M107", "M107"]
    );
}

#[test]
fn the_fan_of_bambu_names_its_part_fan_and_that_of_rrf_never_uses_m107() {
    let mut b = writer(Dialect::Bambu);
    b.set_fan(100.0).unwrap();
    b.set_fan(0.0).unwrap();
    assert_eq!(text(&b), ["M106 P1 S255", "M107"]);
    let mut r = writer(Dialect::Rrf);
    r.set_fan(0.0).unwrap();
    r.set_fan(100.0).unwrap();
    assert_eq!(text(&r), ["M106 S0", "M106 S255"]);
}

#[test]
fn the_fan_edges_are_exact() {
    let mut w = writer(Dialect::Marlin);
    for bad in [100.000_1, -0.000_1, f64::NAN, f64::INFINITY, -1.0] {
        assert!(matches!(w.set_fan(bad), Err(GcodeError::Value(_))), "{bad}");
    }
    assert!(w.lines().is_empty());
}

// ---- tools and pressure advance ----

#[test]
fn tools_go_from_zero_to_fifteen() {
    let mut w = writer(Dialect::Marlin);
    w.select_tool(0).unwrap();
    w.select_tool(15).unwrap();
    assert!(w.select_tool(16).is_err());
    assert!(w.select_tool(255).is_err());
    assert_eq!(text(&w), ["T0", "T15"]);
}

#[test]
fn pressure_advance_has_a_line_for_each_family() {
    let mut k = writer(Dialect::Klipper);
    k.set_pressure_advance(0.05).unwrap();
    assert_eq!(text(&k), ["SET_PRESSURE_ADVANCE ADVANCE=0.05"]);
    let mut r = writer(Dialect::Rrf);
    r.set_pressure_advance(0.05).unwrap();
    assert_eq!(text(&r), ["M572 D0 S0.05"]);
    let mut m = writer(Dialect::Marlin);
    m.set_pressure_advance(0.0).unwrap();
    m.set_pressure_advance(2.0).unwrap();
    assert_eq!(text(&m), ["M900 K0", "M900 K2"]);
}

#[test]
fn pressure_advance_refuses_what_is_out_of_range_or_not_a_number() {
    let mut w = writer(Dialect::Klipper);
    for bad in [2.001, -0.001, f64::NAN, f64::INFINITY] {
        assert!(w.set_pressure_advance(bad).is_err(), "{bad}");
    }
    assert!(w.lines().is_empty());
}

// ---- moves ----

#[test]
fn a_move_writes_only_the_words_that_were_given() {
    let mut w = writer(Dialect::Marlin);
    w.linear_move(Some(10.0), Some(20.5), Some(0.2), Some(0.5), Some(1800.0))
        .unwrap();
    w.linear_move(None, None, None, None, Some(1500.0)).unwrap();
    w.linear_move(None, None, None, Some(-1.5), None).unwrap();
    w.linear_move(Some(0.0), None, None, None, None).unwrap();
    assert_eq!(
        text(&w),
        [
            "G1 X10 Y20.5 Z0.2 E0.5 F1800",
            "G1 F1500",
            "G1 E-1.5",
            "G1 X0"
        ]
    );
}

#[test]
fn a_move_with_nothing_in_it_is_an_error() {
    let mut w = writer(Dialect::Marlin);
    let e = w.linear_move(None, None, None, None, None).unwrap_err();
    assert!(e.to_string().contains("at least one"));
    assert!(w.lines().is_empty());
}

#[test]
fn the_build_volume_is_inclusive_on_every_axis() {
    let mut w = writer(Dialect::Marlin);
    w.linear_move(Some(220.0), Some(210.0), Some(250.0), None, None)
        .unwrap();
    assert!(w
        .linear_move(Some(220.001), None, None, None, None)
        .is_err());
    assert!(w
        .linear_move(None, Some(210.001), None, None, None)
        .is_err());
    assert!(w
        .linear_move(None, None, Some(250.001), None, None)
        .is_err());
    assert!(w.linear_move(Some(-0.001), None, None, None, None).is_err());
    assert!(w
        .linear_move(None, None, Some(f64::NAN), None, None)
        .is_err());
    assert_eq!(w.lines().len(), 1);
}

#[test]
fn a_refused_axis_stops_the_whole_move() {
    let mut w = writer(Dialect::Marlin);
    let r = w.linear_move(Some(10.0), Some(999.0), None, Some(1.0), Some(1000.0));
    assert!(r.is_err());
    assert!(w.lines().is_empty());
}

#[test]
fn extrusion_and_feed_have_exact_limits() {
    let mut w = writer(Dialect::Marlin);
    w.linear_move(None, None, None, Some(100_000.0), Some(1.0))
        .unwrap();
    w.linear_move(None, None, None, Some(-100_000.0), Some(100_000.0))
        .unwrap();
    for e in [100_000.1, -100_000.1, f64::NAN, f64::INFINITY] {
        assert!(
            w.linear_move(None, None, None, Some(e), None).is_err(),
            "{e}"
        );
    }
    for f in [0.99, 100_000.1, 0.0, -5.0, f64::NAN] {
        assert!(
            w.linear_move(None, None, None, None, Some(f)).is_err(),
            "{f}"
        );
    }
    assert_eq!(w.lines().len(), 2);
}

// ---- layers ----

#[test]
fn layer_markers_of_each_family() {
    let mut m = writer(Dialect::Marlin);
    m.layer(3, 0.6).unwrap();
    assert_eq!(text(&m), [";LAYER:3"]);
    let mut k = writer(Dialect::Klipper);
    k.layer(3, 0.6).unwrap();
    assert_eq!(
        text(&k),
        [";LAYER:3", "SET_PRINT_STATS_INFO CURRENT_LAYER=4"]
    );
    let mut b = writer(Dialect::Bambu);
    b.layer(3, 0.6).unwrap();
    assert_eq!(text(&b), ["; layer_num: 3", "; Z_HEIGHT: 0.6"]);
}

#[test]
fn a_layer_above_the_volume_or_below_zero_is_refused() {
    let mut w = writer(Dialect::Marlin);
    w.layer(0, 0.0).unwrap();
    w.layer(1, 250.0).unwrap();
    assert!(w.layer(2, 250.001).is_err());
    assert!(w.layer(2, -0.001).is_err());
    assert!(w.layer(2, f64::NAN).is_err());
    assert_eq!(w.lines().len(), 2);
}

// ---- the start and the end of a job ----

#[test]
fn the_start_of_a_marlin_job_heats_the_nozzle_in_two_steps() {
    let mut w = writer(Dialect::Marlin);
    w.preamble(&job(210.0, 60.0, None)).unwrap();
    assert_eq!(
        text(&w),
        [
            "G21",
            "G90",
            "M83",
            "M140 S60",
            "M104 S150",
            "G28",
            "M190 S60",
            "M109 S210",
            "M107"
        ]
    );
}

#[test]
fn a_cool_nozzle_is_not_raised_to_the_first_step() {
    let mut w = writer(Dialect::Marlin);
    w.preamble(&job(100.0, 0.0, None)).unwrap();
    assert!(text(&w).contains(&"M104 S100"));
    assert!(!text(&w).contains(&"M104 S150"));
    assert!(text(&w).contains(&"M140 S0"));
}

#[test]
fn the_start_of_a_reprapfirmware_job() {
    let mut w = writer(Dialect::Rrf);
    w.preamble(&job(210.0, 60.0, None)).unwrap();
    assert_eq!(
        text(&w),
        [
            "G21",
            "G90",
            "M83",
            "M140 S60",
            "G10 P0 S150",
            "G28",
            "M190 S60",
            "G10 P0 S210",
            "M116",
            "M106 S0"
        ]
    );
}

#[test]
fn the_start_of_a_klipper_job_is_one_macro_call() {
    let mut w = writer(Dialect::Klipper);
    w.preamble(&job(210.0, 60.0, None)).unwrap();
    assert_eq!(
        text(&w),
        ["G21", "G90", "M83", "PRINT_START BED=60 EXTRUDER=210"]
    );
    let mut c = Writer::new(Dialect::Klipper, WARM, VOLUME).unwrap();
    c.preamble(&job(210.0, 60.0, Some(40.0))).unwrap();
    assert_eq!(
        c.lines().last().map(String::as_str),
        Some("PRINT_START BED=60 EXTRUDER=210 CHAMBER=40")
    );
    let mut z = Writer::new(Dialect::Klipper, WARM, VOLUME).unwrap();
    z.preamble(&job(210.0, 60.0, Some(0.0))).unwrap();
    assert!(!z.lines().last().unwrap().contains("CHAMBER"));
}

#[test]
fn a_bambu_job_tells_the_minutes_it_will_take() {
    let mut w = writer(Dialect::Bambu);
    let mut j = job(210.0, 60.0, None);
    j.estimated_minutes = Some(95);
    w.preamble(&j).unwrap();
    assert_eq!(text(&w)[3], "M73 P0 R95");
    let mut m = writer(Dialect::Marlin);
    m.preamble(&j).unwrap();
    assert!(!text(&m).iter().any(|l| l.starts_with("M73")));
}

#[test]
fn a_chamber_job_warms_the_chamber_twice_in_order() {
    let mut w = Writer::new(Dialect::Marlin, WARM, VOLUME).unwrap();
    w.preamble(&job(210.0, 60.0, Some(40.0))).unwrap();
    let t = text(&w);
    let first = t.iter().position(|l| *l == "M141 S40").unwrap();
    let wait = t.iter().position(|l| *l == "M191 S40").unwrap();
    assert!(first < wait);
}

#[test]
fn a_job_with_a_value_over_the_limits_writes_nothing_at_all() {
    let cases = [
        job(300.5, 60.0, None),
        job(210.0, 100.5, None),
        job(210.0, 60.0, Some(10.0)),
        job(f64::NAN, 60.0, None),
        JobParams {
            fan_pct: 101.0,
            ..job(210.0, 60.0, None)
        },
        JobParams {
            fan_pct: f64::NAN,
            ..job(210.0, 60.0, None)
        },
    ];
    for j in cases {
        let mut w = writer(Dialect::Marlin);
        assert!(w.preamble(&j).is_err(), "{j:?}");
        assert!(w.lines().is_empty(), "{j:?}");
    }
}

#[test]
fn the_end_of_a_job_cools_everything_and_stops_the_motors() {
    let mut m = writer(Dialect::Marlin);
    m.end_safe().unwrap();
    assert_eq!(
        text(&m),
        [
            "G91",
            "G1 E-2 F1800",
            "G90",
            "M104 S0",
            "M140 S0",
            "M107",
            "M84"
        ]
    );
    let mut c = Writer::new(Dialect::Marlin, WARM, VOLUME).unwrap();
    c.end_safe().unwrap();
    assert!(text(&c).contains(&"M141 S0"));
    let mut r = writer(Dialect::Rrf);
    r.end_safe().unwrap();
    assert_eq!(r.lines().last().map(String::as_str), Some("M0"));
    let mut k = writer(Dialect::Klipper);
    k.end_safe().unwrap();
    assert_eq!(text(&k), ["PRINT_END"]);
}

// ---- the file ----

#[test]
fn the_header_lists_the_dialect_the_limits_and_what_the_printer_needs() {
    let mut b = writer(Dialect::Bambu);
    b.header("gen", "1.2.3", "x1c");
    b.comment("body").unwrap();
    let file = b.finish().unwrap();
    assert!(file.starts_with("; generator: gen 1.2.3\n; dialect: bambu\n; printer: x1c\n"));
    assert!(file.contains("; temperature limits: nozzle 300 C, bed 100 C\n"));
    assert!(file.contains("inside a package"));
    assert!(file.ends_with("\n\n; body\n"));
    let mut k = writer(Dialect::Klipper);
    k.header("gen", "1", "");
    let file = k.finish().unwrap();
    assert!(!file.contains("printer:"));
    assert!(file.contains("needs the macro PRINT_START"));
    assert!(file.contains("needs the macro PRINT_END"));
}

#[test]
fn a_program_with_no_header_starts_at_the_first_line() {
    let mut w = writer(Dialect::Marlin);
    w.select_tool(1).unwrap();
    assert_eq!(w.finish().unwrap(), "T1\n");
    assert_eq!(writer(Dialect::Marlin).finish().unwrap(), "");
}

#[test]
fn a_comment_cannot_smuggle_in_a_second_line() {
    let mut w = writer(Dialect::Marlin);
    assert!(w.comment("a\nM104 S900").is_err());
    assert!(w.comment("a\rb").is_err());
    assert!(w.lines().is_empty());
    w.comment("fine").unwrap();
    assert_eq!(text(&w), ["; fine"]);
}

#[test]
fn a_zero_byte_or_a_very_long_line_is_refused_when_the_file_is_made() {
    let mut w = writer(Dialect::Marlin);
    w.comment("a\0b").unwrap();
    assert!(matches!(w.finish(), Err(GcodeError::File(_))));
    let mut long = writer(Dialect::Marlin);
    long.comment(&"x".repeat(1022)).unwrap();
    assert!(long.clone().finish().is_ok());
    long.comment(&"x".repeat(1023)).unwrap();
    assert!(matches!(long.finish(), Err(GcodeError::File(_))));
}

// ---- the checker ----

#[test]
fn the_checker_finds_each_heating_command_over_its_limit() {
    let prog = "M104 S301\nM109 S301\nM140 S101\nM190 S101\nM104 S300\nM140 S100\n";
    let v = check_program(prog, &LIMITS);
    let lines: Vec<usize> = v.iter().map(|x| x.line).collect();
    assert_eq!(lines, [1, 2, 3, 4]);
    assert!(matches!(
        v[0].error,
        TempError::Above {
            zone: Zone::Nozzle,
            ..
        }
    ));
    assert!(matches!(
        v[3].error,
        TempError::Above {
            zone: Zone::Bed,
            ..
        }
    ));
}

#[test]
fn the_checker_ignores_comments_blank_lines_and_other_commands() {
    let prog = "; M104 S999\n\n   \nG1 X5 S999\nM104 S200 ; M104 S999\nM106 S255\n";
    assert!(check_program(prog, &LIMITS).is_empty());
    assert!(check_program("", &LIMITS).is_empty());
}

#[test]
fn the_checker_reads_lower_case_and_keeps_the_command_as_written() {
    let v = check_program("  m104 s500  \n", &LIMITS);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].command, "m104 s500");
    assert_eq!(v[0].line, 1);
}

#[test]
fn the_checker_reports_a_value_that_is_not_a_number() {
    let v = check_program("M104 Sabc\nM140 S\nM104 Snan\nM104 Sinf\n", &LIMITS);
    assert_eq!(v.len(), 4);
    assert!(v.iter().all(|x| matches!(x.error, TempError::NotFinite(_))));
}

#[test]
fn the_checker_finds_the_s_word_wherever_it_stands() {
    let v = check_program("M104 T0 S400\nM104 S400 T0\nM104 T0\n", &LIMITS);
    let lines: Vec<usize> = v.iter().map(|x| x.line).collect();
    assert_eq!(lines, [1, 2]);
}

#[test]
fn the_checker_reads_the_chamber_and_refuses_heat_on_a_printer_without_one() {
    let v = check_program("M141 S40\nM191 S0\nM141 S61\n", &LIMITS);
    assert_eq!(v.len(), 2);
    assert!(matches!(v[0].error, TempError::NoChamber(_)));
    let w = check_program("M141 S60\nM141 S61\n", &WARM);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].line, 2);
}

#[test]
fn the_checker_reads_the_tool_temperature_of_reprapfirmware_only_with_a_tool() {
    let v = check_program("G10 P0 S400\nG10 S400\nG10 P1 S300\n", &LIMITS);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].line, 1);
}

#[test]
fn the_checker_reads_the_heaters_of_klipper() {
    let prog = "SET_HEATER_TEMPERATURE HEATER=extruder TARGET=400\n\
                SET_HEATER_TEMPERATURE HEATER=extruder1 TARGET=301\n\
                SET_HEATER_TEMPERATURE HEATER=heater_bed TARGET=101\n\
                set_heater_temperature heater=Heater_Bed target=100\n\
                SET_HEATER_TEMPERATURE HEATER=chamber_temp TARGET=70\n\
                SET_HEATER_TEMPERATURE HEATER=fan TARGET=900\n\
                SET_HEATER_TEMPERATURE HEATER=extruder TARGET=abc\n";
    let v = check_program(prog, &WARM);
    let lines: Vec<usize> = v.iter().map(|x| x.line).collect();
    assert_eq!(lines, [1, 2, 3, 5, 7]);
}

#[test]
fn the_checker_reads_the_parameters_of_the_start_macro() {
    let v = check_program(
        "PRINT_START BED=101 EXTRUDER=301 CHAMBER=61\nPRINT_START BED=60 EXTRUDER=210\n",
        &WARM,
    );
    let zones: Vec<Zone> = v
        .iter()
        .filter_map(|x| match x.error {
            TempError::Above { zone, .. } => Some(zone),
            _ => None,
        })
        .collect();
    assert_eq!(zones, [Zone::Nozzle, Zone::Bed, Zone::Chamber]);
    assert!(v.iter().all(|x| x.line == 1));
}

#[test]
fn every_line_the_writer_makes_passes_the_checker_at_the_limits() {
    for d in [
        Dialect::Marlin,
        Dialect::Klipper,
        Dialect::Rrf,
        Dialect::Bambu,
    ] {
        let mut w = Writer::new(d, WARM, VOLUME).unwrap();
        w.preamble(&job(300.0, 100.0, Some(60.0))).unwrap();
        w.set_nozzle(300.0, true).unwrap();
        w.set_bed(100.0, true).unwrap();
        w.set_chamber(60.0, true).unwrap();
        w.end_safe().unwrap();
        let file = w.finish().unwrap();
        assert!(check_program(&file, &WARM).is_empty(), "{d}");
    }
}

#[test]
fn the_checker_never_panics_on_odd_lines() {
    let odd = "\u{0}\n;;;\nM104\nM104 S\nG10 P\nSET_HEATER_TEMPERATURE\nPRINT_START BED=\nPRINT_START =5\n\u{e9}\u{4e2d} S5\n";
    let _ = check_program(odd, &LIMITS);
}
