//! The settings that follow the shape and the plastic: layers whose height follows the lean of the walls, the first layer drawn in against the flared foot, and the limit of the plastic
//! the nozzle melts in a second. Shapes come from a fixed seed.

use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Dialect, Limits, Mesh, TempLimits};
use p3dp_gcode::check_program;
use p3dp_slicer::{plan, Kind, Machine, Plan, SliceConfig, SliceError};

fn machine() -> Machine {
    Machine {
        id: "test-printer".into(),
        dialect: Dialect::Klipper,
        volume_mm: [256.0, 256.0, 256.0],
        limits: TempLimits {
            nozzle_c: 300.0,
            bed_c: 100.0,
            chamber_c: None,
        },
    }
}

/// A sphere from rings and segments, with its lowest point on the bed.
fn sphere(r: f64, rings: usize, segs: usize) -> Mesh {
    let pt = |i: usize, j: usize| -> [f64; 3] {
        let phi = std::f64::consts::PI * i as f64 / rings as f64;
        let th = 2.0 * std::f64::consts::PI * (j % segs) as f64 / segs as f64;
        [
            r * phi.sin() * th.cos(),
            r * phi.sin() * th.sin(),
            r * phi.cos(),
        ]
    };
    let mut soup = Vec::new();
    for i in 0..rings {
        for j in 0..segs {
            let (a, b, c, d) = (pt(i, j), pt(i, j + 1), pt(i + 1, j + 1), pt(i + 1, j));
            if i == 0 {
                soup.push([a, d, c]);
            } else if i == rings - 1 {
                soup.push([a, c, b]);
            } else {
                soup.push([a, c, b]);
                soup.push([a, d, c]);
            }
        }
    }
    Mesh::from_soup(&soup, &Limits::default()).unwrap()
}

fn adaptive() -> SliceConfig {
    SliceConfig {
        adaptive_layers: true,
        ..SliceConfig::default()
    }
}

fn heights(p: &Plan) -> Vec<f64> {
    p.layers.iter().map(|l| l.height_mm).collect()
}

#[test]
fn adaptive_layers_follow_the_lean_of_a_dome_and_still_make_a_good_print() {
    let s = sphere(12.0, 36, 36);
    let p = plan(&s, &machine(), &adaptive()).unwrap();
    let h = heights(&p);
    assert!((h.iter().sum::<f64>() - 24.0).abs() < 1e-6);
    assert!((p.height_mm() - 24.0).abs() < 1e-6);
    // the heights change with the shape, inside what was allowed
    let (lo, hi) = h[1..h.len() - 1]
        .iter()
        .fold((f64::MAX, 0.0f64), |(a, b), x| (a.min(*x), b.max(*x)));
    assert!(lo >= 0.08 - 1e-9 && hi <= 0.28 + 1e-9, "{lo} {hi}");
    assert!(hi - lo > 0.1, "{lo} {hi}");
    // the tops of the layers go up by their heights
    let mut z = 0.0;
    for l in &p.layers {
        z += l.height_mm;
        assert!((l.z_mm - z).abs() < 1e-6);
    }
    // and the G-code of such a stack is a good program
    let g = p.gcode("test").unwrap();
    assert!(check_program(&g, &machine().limits).is_empty());
}

#[test]
fn adaptive_layers_are_off_by_default_and_a_straight_part_takes_the_thickest_layers() {
    let off = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
    assert!(heights(&off).iter().all(|h| (h - 0.2).abs() < 1e-9));
    let on = plan(&cube(20.0), &machine(), &adaptive()).unwrap();
    assert!(on.layer_count() < off.layer_count());
    assert!((on.height_mm() - 20.0).abs() < 1e-6);
}

#[test]
fn adaptive_settings_that_cannot_go_together_are_refused_and_nothing_is_changed() {
    for (text, want) in [
        (
            "adaptive_layers = on\nadaptive_min_layer_mm = 0.3\nadaptive_max_layer_mm = 0.2\n",
            "adaptive_min_layer_mm",
        ),
        (
            "adaptive_layers = on\nadaptive_max_layer_mm = 0.5\n",
            "adaptive_max_layer_mm",
        ),
        ("adaptive_layers = on\nvase_mode = on\n", "vase"),
        ("adaptive_cusp_mm = 0\n", "adaptive_cusp_mm"),
        ("elephant_foot_mm = 3\n", "elephant_foot_mm"),
        ("elephant_foot_mm = 0.9\n", "elephant_foot_mm"),
        (
            "max_volumetric_flow_mm3_s = 0.5\n",
            "max_volumetric_flow_mm3_s",
        ),
    ] {
        let e = SliceConfig::from_text(text).unwrap_err().to_string();
        assert!(e.contains(want), "{text:?} -> {e}");
    }
    let c = SliceConfig::from_text(
        "adaptive_layers = on\nelephant_foot_mm = 0.2\nmax_volumetric_flow_mm3_s = 12\n",
    )
    .unwrap();
    assert_eq!(SliceConfig::from_text(&c.to_text()).unwrap(), c);
}

#[test]
fn spheres_from_a_fixed_seed_give_stacks_that_add_up_and_keep_to_the_cusp() {
    let mut rng = Rng::new(0x5ee_d3d);
    for case in 0..8 {
        let r = rng.range(5.0, 25.0);
        let rings = 12 + rng.below(30) as usize;
        let segs = 12 + rng.below(30) as usize;
        let cusp = rng.range(0.03, 0.09);
        let c = SliceConfig {
            adaptive_cusp_mm: cusp,
            ..adaptive()
        };
        let p = plan(&sphere(r, rings, segs), &machine(), &c).unwrap();
        let h = heights(&p);
        assert!(
            (h.iter().sum::<f64>() - 2.0 * r).abs() < 1e-6,
            "case {case}"
        );
        assert!(h
            .iter()
            .all(|x| x.is_finite() && *x > 0.0 && *x <= 0.28 + 1e-9));
        // every layer has paths: nothing is skipped on the way
        assert!(p.layers.iter().all(|l| !l.paths.is_empty()), "case {case}");
    }
}

fn outer_box(l: &p3dp_slicer::Layer) -> ([f64; 2], [f64; 2]) {
    let mut lo = [f64::MAX; 2];
    let mut hi = [f64::MIN; 2];
    for p in l
        .paths
        .iter()
        .filter(|p| p.kind == Kind::ExternalPerimeter)
        .flat_map(|p| &p.points)
    {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

#[test]
fn the_first_layer_is_drawn_in_by_the_foot_and_the_others_are_not() {
    let plain = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
    let foot = plan(
        &cube(20.0),
        &machine(),
        &SliceConfig {
            elephant_foot_mm: 0.25,
            ..SliceConfig::default()
        },
    )
    .unwrap();
    let (plo, phi) = outer_box(&plain.layers[0]);
    let (flo, fhi) = outer_box(&foot.layers[0]);
    for k in 0..2 {
        assert!((flo[k] - plo[k] - 0.25).abs() < 1e-6, "{flo:?} {plo:?}");
        assert!((phi[k] - fhi[k] - 0.25).abs() < 1e-6);
    }
    // the second layer and every one after it are as they were
    // (the head comes to them from a different place, so the order of the paths may differ: the lines are the same)
    let lines = |l: &p3dp_slicer::Layer| -> Vec<(String, i64)> {
        let mut v: Vec<(String, i64)> = l
            .paths
            .iter()
            .map(|p| {
                (
                    p.kind.name().to_string(),
                    (p.length_mm() * 1e6).round() as i64,
                )
            })
            .collect();
        v.sort();
        v
    };
    for i in 1..plain.layers.len() {
        assert_eq!(lines(&plain.layers[i]), lines(&foot.layers[i]), "layer {i}");
    }
    // a raft takes the place of the foot: the part stands on the raft, not on the bed
    let rafted = |foot_mm: f64| SliceConfig {
        adhesion: p3dp_slicer::Adhesion::Raft,
        elephant_foot_mm: foot_mm,
        ..SliceConfig::default()
    };
    let a = plan(&cube(20.0), &machine(), &rafted(0.0)).unwrap();
    let b = plan(&cube(20.0), &machine(), &rafted(0.25)).unwrap();
    assert_eq!(a.layers, b.layers);
}

#[test]
fn a_foot_that_leaves_nothing_of_a_thin_part_is_told_as_a_thin_region() {
    let thin = {
        // a wall 1.2 mm thick
        let soup: Vec<[[f64; 3]; 3]> = cube(1.0)
            .triangles
            .iter()
            .map(|t| {
                [t[0], t[1], t[2]].map(|i| {
                    let m = cube(1.0).vertices[i as usize].mm();
                    [m[0] * 1.2, m[1] * 20.0, m[2] * 10.0]
                })
            })
            .collect();
        Mesh::from_soup(&soup, &Limits::default()).unwrap()
    };
    let c = SliceConfig {
        elephant_foot_mm: 0.5,
        thin_regions: p3dp_slicer::ThinRegions::Refuse,
        ..SliceConfig::default()
    };
    assert!(matches!(
        plan(&thin, &machine(), &c),
        Err(SliceError::ThinRegion { layer: 0 })
    ));
}

#[test]
fn no_line_asks_for_more_plastic_than_the_nozzle_melts() {
    let limit = 3.5;
    let free = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
    let held = plan(
        &cube(20.0),
        &machine(),
        &SliceConfig {
            max_volumetric_flow_mm3_s: limit,
            ..SliceConfig::default()
        },
    )
    .unwrap();
    let mut slowed = 0;
    for l in &held.layers {
        for p in &l.paths {
            let v = held.line_speed_mm_s(l.index, p.kind, p.width_mm, l.height_mm);
            let flow = v * held.line_area_mm2(l.height_mm, p.kind, p.width_mm);
            assert!(flow <= limit + 1e-9 || v <= 1.0 + 1e-9, "{flow}");
            if v < held.speed_mm_s(l.index, p.kind) - 1e-9 {
                slowed += 1;
            }
        }
    }
    assert!(slowed > 0);
    // the same plan, slower: more time, the same plastic
    assert!(held.estimated_seconds() > free.estimated_seconds());
    assert!((held.extrusion_mm3() - free.extrusion_mm3()).abs() < 1e-6);
    // with no limit the speed is the speed of the kind
    for l in &free.layers {
        for p in &l.paths {
            assert_eq!(
                free.line_speed_mm_s(l.index, p.kind, p.width_mm, l.height_mm),
                free.speed_mm_s(l.index, p.kind)
            );
        }
    }
    // and what is written is slower too: the largest feed of an extruding move in the G-code
    let feed = |p: &Plan| -> f64 {
        p.gcode("t")
            .unwrap()
            .lines()
            .filter(|l| l.starts_with("G1") && l.contains(" E") && l.contains(" F"))
            .filter_map(|l| {
                l.split(" F")
                    .nth(1)?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
            .fold(0.0f64, f64::max)
    };
    assert!(feed(&held) < feed(&free));
}

#[test]
fn the_time_of_a_print_is_told_by_group_and_adds_up_to_the_estimate() {
    use p3dp_slicer::report::{time_by_group, Group};
    let mut rng = Rng::new(0x71_3e_55);
    for case in 0..6 {
        let side = rng.range(8.0, 40.0);
        let c = SliceConfig {
            infill_percent: rng.range(5.0, 60.0),
            perimeters: 1 + rng.below(3) as u32,
            min_layer_time_s: if case % 2 == 0 { 0.0 } else { 6.0 },
            max_volumetric_flow_mm3_s: if case % 3 == 0 { 0.0 } else { 4.0 },
            ..SliceConfig::default()
        };
        let p = plan(&cube(side), &machine(), &c).unwrap();
        let s = p.summaries();
        let total: f64 = s.iter().map(|x| x.time_s).sum();
        let shares = time_by_group(&p, &s);
        let sum: f64 = shares.iter().map(|t| t.seconds).sum();
        assert!(
            (sum - total).abs() < 1e-6 * total.max(1.0),
            "case {case}: {sum} {total}"
        );
        assert!(shares
            .iter()
            .all(|t| t.seconds > 0.0 && t.seconds.is_finite()));
        // the groups come in a fixed order, the rest last, and a cube has no supports and no bridges
        assert_eq!(shares[0].group, Group::Perimeters);
        assert_eq!(shares.last().unwrap().group, Group::Other);
        assert!(shares
            .iter()
            .all(|t| t.group != Group::Supports && t.group != Group::Bridges));
        // the same plan gives the same figures
        assert_eq!(shares, time_by_group(&p, &s));
    }
}
