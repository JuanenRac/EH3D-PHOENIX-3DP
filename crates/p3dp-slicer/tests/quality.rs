//! The quality settings that are numbers of the printer and passes of the head: the pressure advance (written in the command of each firmware, within its range) and the ironing of the
//! top (one more pass over the solid skin with very little plastic). Shapes come from a fixed seed. Further down: the walls that hang, their order, the overlap of the infill,
//! the retractions (only across a wall, wiping), the temperatures of the first layer, the pauses, the progress marks and the length of the skirt.

use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Dialect, TempLimits};
use p3dp_gcode::check_program;
use p3dp_slicer::{plan, Kind, Machine, SliceConfig, SliceError};

fn machine(dialect: Dialect) -> Machine {
    Machine {
        id: "test-printer".into(),
        dialect,
        volume_mm: [256.0, 256.0, 256.0],
        limits: TempLimits {
            nozzle_c: 300.0,
            bed_c: 100.0,
            chamber_c: None,
        },
    }
}

fn mm(p: &p3dp_slicer::Plan, kind: Kind) -> f64 {
    p.layers
        .iter()
        .flat_map(|l| &l.paths)
        .filter(|x| x.kind == kind)
        .map(|x| x.length_mm())
        .sum()
}

#[test]
fn the_pressure_advance_is_written_once_in_the_command_of_each_firmware() {
    let cfg = SliceConfig {
        pressure_advance: 0.045,
        ..SliceConfig::default()
    };
    for (dialect, want) in [
        (Dialect::Klipper, "SET_PRESSURE_ADVANCE ADVANCE=0.045"),
        (Dialect::Marlin, "M900 K0.045"),
        (Dialect::Rrf, "M572 D0 S0.045"),
        (Dialect::Bambu, "M900 K0.045"),
    ] {
        let p = plan(&cube(10.0), &machine(dialect), &cfg).unwrap();
        let g = p.gcode("test").unwrap();
        assert_eq!(g.lines().filter(|l| *l == want).count(), 1, "{dialect:?}");
        // it is set before the first layer
        let at = g.lines().position(|l| l == want).unwrap();
        let first_layer = g
            .lines()
            .position(|l| l.contains("LAYER") || l.contains("layer_num"))
            .unwrap();
        assert!(at < first_layer, "{dialect:?}");
    }
    // off by default: nothing is written
    let p = plan(
        &cube(10.0),
        &machine(Dialect::Klipper),
        &SliceConfig::default(),
    )
    .unwrap();
    assert!(!p.gcode("test").unwrap().contains("PRESSURE_ADVANCE"));
}

#[test]
fn the_pressure_advance_and_the_ironing_keep_to_their_ranges() {
    for (text, want) in [
        ("pressure_advance = 2.5", "pressure_advance"),
        ("pressure_advance = -0.1", "pressure_advance"),
        ("ironing_flow_percent = 0", "ironing_flow_percent"),
        ("ironing_flow_percent = 31", "ironing_flow_percent"),
        ("ironing = maybe", "ironing"),
        ("ironing = on\nvase_mode = on", "vase"),
        ("ironing = on\ntop_layers = 0", "top_layers"),
        (
            "ironing = on\nironing_speed_mm_s = 500\nmax_speed_mm_s = 200",
            "ironing_speed_mm_s",
        ),
    ] {
        let e = SliceConfig::from_text(text).unwrap_err().to_string();
        assert!(e.contains(want), "{text:?} -> {e}");
    }
    let c = SliceConfig::from_text(
        "pressure_advance = 0.05\nironing = on\nironing_flow_percent = 12\n",
    )
    .unwrap();
    assert_eq!(SliceConfig::from_text(&c.to_text()).unwrap(), c);
}

#[test]
fn ironing_goes_over_the_top_skin_with_little_plastic() {
    let off = SliceConfig::default();
    let on = SliceConfig {
        ironing: true,
        ..SliceConfig::default()
    };
    let m = machine(Dialect::Klipper);
    let a = plan(&cube(20.0), &m, &off).unwrap();
    let b = plan(&cube(20.0), &m, &on).unwrap();
    assert_eq!(mm(&a, Kind::Ironing), 0.0);
    // the passes are on the top layer only, and as long as the skin they go over
    let top = b.layers.last().unwrap();
    assert!(top.paths.iter().any(|p| p.kind == Kind::Ironing));
    assert!(b.layers[..b.layers.len() - 1]
        .iter()
        .all(|l| l.paths.iter().all(|p| p.kind != Kind::Ironing)));
    assert!((mm(&b, Kind::Ironing) - top_solid(&b)).abs() < 1e-6);
    // the plastic of the pass is the share asked for of the plastic of the skin it goes over
    let extra = b.extrusion_mm3() - a.extrusion_mm3();
    let skin: f64 = top
        .paths
        .iter()
        .filter(|p| p.kind == Kind::SolidInfill)
        .map(|p| p.length_mm() * b.line_area_mm2(top.height_mm, p.kind, p.width_mm))
        .sum();
    assert!((extra - skin * 0.10).abs() < 1e-6, "{extra} {skin}");
    // it takes longer, and its own speed is used and checked against the limits
    assert!(b.estimated_seconds() > a.estimated_seconds());
    assert!((b.speed_mm_s(top.index, Kind::Ironing) - 20.0).abs() < 1e-9);
    let g = b.gcode("test").unwrap();
    assert!(g.contains("TYPE:ironing"));
    assert!(check_program(&g, &m.limits).is_empty());
}

fn top_solid(p: &p3dp_slicer::Plan) -> f64 {
    p.layers
        .last()
        .unwrap()
        .paths
        .iter()
        .filter(|x| x.kind == Kind::SolidInfill)
        .map(|x| x.length_mm())
        .sum()
}

#[test]
fn shapes_from_a_fixed_seed_keep_ironing_inside_the_part_and_the_bed() {
    let mut rng = Rng::new(0x0012_0377);
    let m = machine(Dialect::Marlin);
    for _ in 0..16 {
        let size = 8.0 + (rng.next_u64() % 40) as f64;
        let cfg = SliceConfig {
            ironing: true,
            ironing_flow_percent: 5.0 + (rng.next_u64() % 20) as f64,
            pressure_advance: (rng.next_u64() % 100) as f64 / 100.0,
            ..SliceConfig::default()
        };
        let p = plan(&cube(size), &m, &cfg).unwrap();
        let top = p.layers.last().unwrap();
        let solid: Vec<_> = top
            .paths
            .iter()
            .filter(|x| x.kind == Kind::SolidInfill)
            .collect();
        let iron: Vec<_> = top
            .paths
            .iter()
            .filter(|x| x.kind == Kind::Ironing)
            .collect();
        assert_eq!(solid.len(), iron.len(), "{size}");
        // every pass runs over a line of the skin, the other way round
        for (s, i) in solid.iter().rev().zip(&iron) {
            let mut back = s.points.clone();
            back.reverse();
            assert_eq!(back, i.points);
        }
        let g = p.gcode("test").unwrap();
        assert!(check_program(&g, &m.limits).is_empty());
    }
}

#[test]
fn a_vase_is_not_ironed_and_a_bad_pressure_advance_never_reaches_the_writer() {
    let vase = SliceConfig {
        vase_mode: true,
        ironing: true,
        ..SliceConfig::default()
    };
    let e = plan(&cube(20.0), &machine(Dialect::Klipper), &vase).unwrap_err();
    assert!(matches!(e, SliceError::Config(_)), "{e}");
    let wild = SliceConfig {
        pressure_advance: 9.0,
        ..SliceConfig::default()
    };
    // set by hand past the range of the file, the writer still refuses it
    let p = plan(&cube(10.0), &machine(Dialect::Klipper), &wild).unwrap();
    assert!(p.gcode("test").is_err());
}

// ---- shapes and lines

/// A solid whose bottom and top are two rectangles `(x0, y0, x1, y1)`, `height` apart: a wall of it leans where they differ.
fn slab(bottom: [f64; 4], top: [f64; 4], height: f64) -> p3dp_core::Mesh {
    let mut soup = Vec::new();
    let c = cube(1.0);
    for t in &c.triangles {
        soup.push(t.map(|i| {
            let v = c.vertices[i as usize].mm();
            let (r, z) = if v[2] > 0.5 {
                (top, height)
            } else {
                (bottom, 0.0)
            };
            [
                if v[0] > 0.5 { r[2] } else { r[0] },
                if v[1] > 0.5 { r[3] } else { r[1] },
                z,
            ]
        }));
    }
    p3dp_core::Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap()
}

fn settings(pairs: &[(&str, &str)]) -> SliceConfig {
    let mut c = SliceConfig::default();
    for (k, v) in pairs {
        c.set(k, v).unwrap();
    }
    c.check().unwrap();
    c
}

fn gcode_of(mesh: &p3dp_core::Mesh, dialect: Dialect, config: &SliceConfig) -> String {
    plan(mesh, &machine(dialect), config)
        .unwrap()
        .gcode("test")
        .unwrap()
}

/// The filament of the lines of a program that pull back (negative) or push, added up for each.
fn pulls(g: &str) -> (usize, f64) {
    let mut count = 0;
    let mut net = 0.0;
    for line in g.lines().filter(|l| l.starts_with("G1 ")) {
        let word = line
            .split_ascii_whitespace()
            .find_map(|w| w.strip_prefix('E'))
            .and_then(|v| v.parse::<f64>().ok());
        if let Some(e) = word {
            if e < 0.0 {
                count += 1;
            }
            net += e;
        }
    }
    (count, net)
}

#[test]
fn an_outer_wall_that_hangs_is_slower_with_the_fan_of_a_bridge_and_a_straight_one_stays_whole() {
    // the +x side leans out 0.4 mm a layer, which is more than half a line; the others do not
    let leaning = slab([0.0, 0.0, 10.0, 10.0], [0.0, 0.0, 14.0, 10.0], 2.0);
    let c = settings(&[("overhang_speed_mm_s", "10"), ("fan_percent", "30")]);
    let p = plan(&leaning, &machine(Dialect::Marlin), &c).unwrap();
    assert!(mm(&p, Kind::OverhangPerimeter) > 50.0);
    assert!(mm(&p, Kind::ExternalPerimeter) > 100.0);
    assert!(p.layers[0]
        .paths
        .iter()
        .all(|x| x.kind != Kind::OverhangPerimeter));
    assert!((p.speed_mm_s(5, Kind::OverhangPerimeter) - 10.0).abs() < 1e-9);
    let g = p.gcode("test").unwrap();
    let lines: Vec<&str> = g.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == "; TYPE:overhang perimeter")
        .expect("the walls that hang are named");
    assert!(
        lines[at + 1..at + 3].contains(&"M106 S255"),
        "{:?}",
        &lines[at..at + 3]
    );
    // the same length of wall as without the setting
    let whole = plan(&leaning, &machine(Dialect::Marlin), &SliceConfig::default()).unwrap();
    let outer = mm(&p, Kind::ExternalPerimeter) + mm(&p, Kind::OverhangPerimeter);
    assert!((outer - mm(&whole, Kind::ExternalPerimeter)).abs() < 1e-6);
    assert_eq!(mm(&whole, Kind::OverhangPerimeter), 0.0);
    // a cube has nothing that hangs: its walls stay closed loops
    let cube_plan = plan(&cube(10.0), &machine(Dialect::Marlin), &c).unwrap();
    assert_eq!(mm(&cube_plan, Kind::OverhangPerimeter), 0.0);
    assert!(cube_plan.layers[5]
        .paths
        .iter()
        .filter(|x| x.kind == Kind::ExternalPerimeter)
        .all(|x| x.closed));
    let mut fast = SliceConfig::default();
    fast.set("overhang_speed_mm_s", "900").unwrap();
    assert!(fast.check().is_err());
}

#[test]
fn the_outer_wall_goes_first_only_when_asked() {
    let first_wall = |c: &SliceConfig| {
        plan(&cube(10.0), &machine(Dialect::Marlin), c)
            .unwrap()
            .layers[5]
            .paths[0]
            .kind
    };
    assert_eq!(first_wall(&SliceConfig::default()), Kind::Perimeter);
    let outer = settings(&[("external_perimeters_first", "on")]);
    assert_eq!(first_wall(&outer), Kind::ExternalPerimeter);
    let a = plan(&cube(10.0), &machine(Dialect::Marlin), &outer).unwrap();
    let b = plan(
        &cube(10.0),
        &machine(Dialect::Marlin),
        &SliceConfig::default(),
    )
    .unwrap();
    assert!((a.extrusion_mm3() - b.extrusion_mm3()).abs() < 1e-6);
}

#[test]
fn the_overlap_makes_the_infill_reach_into_the_innermost_wall() {
    let span = |c: &SliceConfig| {
        let p = plan(&cube(20.0), &machine(Dialect::Marlin), c).unwrap();
        let xs: Vec<f64> = p.layers[50]
            .paths
            .iter()
            .filter(|x| x.kind == Kind::Infill)
            .flat_map(|x| x.points.iter().map(|q| q[0]))
            .collect();
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min)
    };
    let none = span(&SliceConfig::default());
    let some = span(&settings(&[("infill_overlap_percent", "30")]));
    // 30 % of 0.45 mm on each side, with the lines of the infill along x
    assert!((some - none - 0.27).abs() < 1e-6, "{none} {some}");
    assert!(SliceConfig::default()
        .set("infill_overlap_percent", "51")
        .is_err());
}

#[test]
fn a_travel_that_stays_inside_the_walls_is_not_retracted_when_asked() {
    let c = settings(&[("retract_only_crossing", "on")]);
    let (always, net) = pulls(&gcode_of(
        &cube(20.0),
        Dialect::Marlin,
        &SliceConfig::default(),
    ));
    let (fewer, net_fewer) = pulls(&gcode_of(&cube(20.0), Dialect::Marlin, &c));
    assert!(fewer < always, "{fewer} {always}");
    assert!(fewer > 0);
    assert!((net - net_fewer).abs() < 0.5, "{net} {net_fewer}");
    // two cubes apart: the travel from one to the other crosses a wall and is retracted
    let two = {
        let mut soup = Vec::new();
        for x in [0.0, 30.0] {
            let m = cube(8.0);
            for t in &m.triangles {
                soup.push(t.map(|i| {
                    let v = m.vertices[i as usize].mm();
                    [v[0] + x, v[1], v[2]]
                }));
            }
        }
        p3dp_core::Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap()
    };
    let p = plan(&two, &machine(Dialect::Marlin), &c).unwrap();
    let mut pulled = 0.0;
    let (moves, _) = p.layer_moves(&p.layers[5], [0.0, 0.0], &mut pulled);
    let mut here: [f64; 2] = [0.0, 0.0];
    let mut held = false;
    let mut long_ones = 0;
    for m in moves {
        match m {
            p3dp_slicer::Move::Retract(_) => held = true,
            p3dp_slicer::Move::Unretract(_) => held = false,
            p3dp_slicer::Move::Travel(to) => {
                if (to[0] - here[0]).abs() > 20.0 {
                    long_ones += 1;
                    assert!(held, "{here:?} to {to:?}");
                }
                here = to;
            }
            p3dp_slicer::Move::Line { to, .. } | p3dp_slicer::Move::Wipe { to, .. } => here = to,
            p3dp_slicer::Move::Z(_) => {}
        }
    }
    assert!(long_ones > 0);
}

#[test]
fn the_wipe_pulls_back_along_the_line_just_laid_and_takes_the_same_filament() {
    let c = settings(&[("wipe_mm", "2")]);
    let plain = gcode_of(&cube(20.0), Dialect::Marlin, &SliceConfig::default());
    let wiped = gcode_of(&cube(20.0), Dialect::Marlin, &c);
    let moving_pulls = |g: &str| {
        g.lines()
            .filter(|l| l.starts_with("G1 X") && l.contains(" E-"))
            .count()
    };
    assert_eq!(moving_pulls(&plain), 0);
    assert!(moving_pulls(&wiped) > 50);
    let (_, a) = pulls(&plain);
    let (_, b) = pulls(&wiped);
    assert!((a - b).abs() < 1e-6, "{a} {b}");
    assert!(check_program(&wiped, &machine(Dialect::Marlin).limits).is_empty());
    let moves = p3dp_gcode::read_motion(&wiped, Some([256.0, 256.0, 256.0]));
    assert_eq!(moves.outside_count, 0);
}

#[test]
fn the_first_layer_has_its_own_temperatures_and_they_keep_to_the_limits() {
    let c = settings(&[("first_layer_nozzle_c", "225"), ("first_layer_bed_c", "70")]);
    let g = gcode_of(&cube(5.0), Dialect::Marlin, &c);
    let lines: Vec<&str> = g.lines().collect();
    let at = |want: &str| {
        lines
            .iter()
            .position(|l| *l == want)
            .unwrap_or_else(|| panic!("no {want}"))
    };
    let second = at(";LAYER:1");
    assert!(at("M190 S70") < at(";LAYER:0") && at("M109 S225") < at(";LAYER:0"));
    assert!(at("M140 S60") > second || at("M140 S70") < second);
    assert!(lines[second..].contains(&"M140 S60"));
    assert!(lines[second..].contains(&"M104 S210"));
    // the same temperatures: nothing is set again
    let same = gcode_of(&cube(5.0), Dialect::Marlin, &SliceConfig::default());
    assert!(!same.contains("M104 S210"));
    let klipper = gcode_of(&cube(5.0), Dialect::Klipper, &c);
    assert!(klipper.contains("PRINT_START BED=70 EXTRUDER=225"));
    for (key, value) in [
        ("first_layer_nozzle_c", "301"),
        ("first_layer_bed_c", "101"),
    ] {
        let hot = settings(&[(key, value)]);
        assert!(matches!(
            plan(&cube(5.0), &machine(Dialect::Marlin), &hot),
            Err(SliceError::Temp(_))
        ));
    }
}

#[test]
fn the_pauses_are_written_before_their_layers_in_the_command_of_each_firmware() {
    let c = settings(&[("pause_at_layers", "3, 10")]);
    assert_eq!(c.value_of("pause_at_layers").as_deref(), Some("3,10"));
    assert_eq!(SliceConfig::from_text(&c.to_text()).unwrap(), c);
    for (dialect, command) in [
        (Dialect::Marlin, "M600"),
        (Dialect::Klipper, "PAUSE"),
        (Dialect::Rrf, "M226"),
        (Dialect::Bambu, "M400 U1"),
    ] {
        let g = gcode_of(&cube(5.0), dialect, &c);
        let lines: Vec<&str> = g.lines().collect();
        let at: Vec<usize> = (0..lines.len()).filter(|i| lines[*i] == command).collect();
        assert_eq!(at.len(), 2, "{dialect}");
        assert!(g.contains("; pause before layer 3") && g.contains("; pause before layer 10"));
        // the filament is back before the pause
        assert!(
            lines[at[0].saturating_sub(4)..at[0]]
                .iter()
                .any(|l| l.starts_with("G1 E-")),
            "{dialect}: {:?}",
            &lines[at[0] - 4..at[0]]
        );
    }
    for bad in ["1", "5,5", "7,3", "two", "2,"] {
        assert!(
            SliceConfig::default().set("pause_at_layers", bad).is_err(),
            "{bad}"
        );
    }
    let late = settings(&[("pause_at_layers", "26")]);
    assert!(matches!(
        plan(&cube(5.0), &machine(Dialect::Marlin), &late),
        Err(SliceError::Config(_))
    ));
}

#[test]
fn the_progress_marks_rise_to_the_end_and_can_be_turned_off() {
    let g = gcode_of(&cube(10.0), Dialect::Marlin, &SliceConfig::default());
    let marks: Vec<(u32, u32)> = g
        .lines()
        .filter_map(|l| l.strip_prefix("M73 P"))
        .map(|l| {
            let (p, r) = l.split_once(" R").unwrap();
            (p.parse().unwrap(), r.parse().unwrap())
        })
        .collect();
    assert!(marks.len() > 10);
    assert_eq!(marks.first().map(|m| m.0), Some(0));
    assert_eq!(marks.last(), Some(&(100, 0)));
    assert!(marks
        .windows(2)
        .all(|w| w[0].0 < w[1].0 && w[0].1 >= w[1].1));
    let off = settings(&[("progress_marks", "off")]);
    assert!(!gcode_of(&cube(10.0), Dialect::Marlin, &off).contains("M73"));
    let klipper = gcode_of(&cube(10.0), Dialect::Klipper, &SliceConfig::default());
    assert!(klipper.contains("SET_PRINT_STATS_INFO TOTAL_LAYER=50"));
    assert!(!gcode_of(&cube(10.0), Dialect::Rrf, &SliceConfig::default()).contains("M73"));
}

#[test]
fn a_skirt_gets_more_loops_until_it_is_long_enough_and_the_fit_knows_it() {
    let short = settings(&[("adhesion", "skirt")]);
    let long = settings(&[("adhesion", "skirt"), ("skirt_min_length_mm", "600")]);
    let a = plan(&cube(10.0), &machine(Dialect::Marlin), &short).unwrap();
    let b = plan(&cube(10.0), &machine(Dialect::Marlin), &long).unwrap();
    assert!(mm(&a, Kind::Adhesion) < 150.0);
    assert!(
        mm(&b, Kind::Adhesion) >= 600.0,
        "{}",
        mm(&b, Kind::Adhesion)
    );
    assert!(mm(&b, Kind::Adhesion) < 700.0);
    let need = |c: &SliceConfig| {
        p3dp_slicer::machine_check::fit([10.0; 3], [256.0; 3], c, None).needs_mm[0]
    };
    assert!(need(&long) > need(&short));
}

#[test]
fn a_raft_is_counted_when_the_lift_is_held_to_the_height_of_the_printer() {
    let low = Machine {
        volume_mm: [256.0, 256.0, 10.7],
        ..machine(Dialect::Marlin)
    };
    let c = settings(&[("adhesion", "raft"), ("z_hop_mm", "0.4")]);
    assert!(matches!(
        plan(&cube(10.0), &low, &c),
        Err(SliceError::LiftPastTop { .. })
    ));
    let flat = settings(&[("adhesion", "raft")]);
    assert!(plan(&cube(10.0), &low, &flat).is_ok());
}

#[test]
fn support_interface_layers_are_close_lines_under_the_overhang() {
    // a plate over two pillars
    let table = {
        let mut soup = Vec::new();
        for (m, shift) in [
            (
                slab([0.0, 0.0, 5.0, 10.0], [0.0, 0.0, 5.0, 10.0], 5.0),
                [0.0, 0.0, 0.0],
            ),
            (
                slab([25.0, 0.0, 30.0, 10.0], [25.0, 0.0, 30.0, 10.0], 5.0),
                [0.0, 0.0, 0.0],
            ),
            (
                slab([0.0, 0.0, 30.0, 10.0], [0.0, 0.0, 30.0, 10.0], 1.0),
                [0.0, 0.0, 5.0],
            ),
        ] {
            for t in &m.triangles {
                soup.push(t.map(|i| {
                    let v = m.vertices[i as usize].mm();
                    [v[0] + shift[0], v[1] + shift[1], v[2] + shift[2]]
                }));
            }
        }
        p3dp_core::Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap()
    };
    let plain = settings(&[("supports", "buildplate")]);
    let floor = settings(&[
        ("supports", "buildplate"),
        ("support_interface_layers", "2"),
        ("support_interface_spacing_mm", "0.6"),
    ]);
    let a = plan(&table, &machine(Dialect::Marlin), &plain).unwrap();
    let b = plan(&table, &machine(Dialect::Marlin), &floor).unwrap();
    let on = |p: &p3dp_slicer::Plan, layer: usize| -> f64 {
        p.layers[layer]
            .paths
            .iter()
            .filter(|x| x.kind == Kind::Support)
            .map(|x| x.length_mm())
            .sum()
    };
    // the plate starts at layer 25, with one layer of gap: the supports end at layer 23, and its two top layers are close
    assert!(
        on(&b, 23) > 3.0 * on(&a, 23),
        "{} {}",
        on(&b, 23),
        on(&a, 23)
    );
    assert!(on(&b, 22) > 3.0 * on(&a, 22));
    assert!((on(&b, 10) - on(&a, 10)).abs() < 1e-9);
    let narrow = {
        let mut c = SliceConfig::default();
        c.set("support_interface_layers", "1").unwrap();
        c.set("support_interface_spacing_mm", "0.2").unwrap();
        c
    };
    assert!(narrow.check().is_err());
}
