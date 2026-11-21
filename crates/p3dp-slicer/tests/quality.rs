//! The quality settings that are numbers of the printer and passes of the head: the pressure advance (written in the command of each firmware, within its range) and the ironing of the
//! top (one more pass over the solid skin with very little plastic). Shapes come from a fixed seed.

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
