//! Edge cases of the slicer: the plane geometry of a layer (areas, the inside of loops, the offset of a loop, the crossings of a loop with itself, the lines of the infill), the time
//! of a run of moves, every range and every rule between the settings, the choice of a filament, and the refusals of a plan. A model or a setting that is wrong is refused with a
//! reason and never cut down to pass. The random numbers come from a seeded generator, so every run sees the same shapes.

use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Dialect, Mesh, TempLimits};
use p3dp_profiles::{Catalog, FilamentProfile, Nozzle};
use p3dp_slicer::config::KEYS;
use p3dp_slicer::geom::{self, Loop};
use p3dp_slicer::support::distance_to_loops;
use p3dp_slicer::timing::{self, Block};
use p3dp_slicer::{plan, slice, Adhesion, Machine, SliceConfig, SliceError};

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn reversed(l: &Loop) -> Loop {
    l.iter().rev().copied().collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// ---------------------------------------------------------------- geometry

#[test]
fn the_area_of_a_loop_has_a_sign_that_follows_the_way_round() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    assert!(near(geom::area(&s), 100.0));
    assert!(near(geom::area(&reversed(&s)), -100.0));
}

#[test]
fn a_loop_of_fewer_than_three_points_has_no_area_and_no_panic() {
    assert_eq!(geom::area(&[]), 0.0);
    assert_eq!(geom::area(&[[3.0, 4.0]]), 0.0);
    assert_eq!(geom::area(&[[0.0, 0.0], [5.0, 5.0]]), 0.0);
    assert_eq!(geom::loop_length(&[]), 0.0);
    assert_eq!(geom::loop_length(&[[1.0, 1.0]]), 0.0);
}

#[test]
fn the_length_of_a_loop_comes_back_to_its_start() {
    assert!(near(geom::loop_length(&square(0.0, 0.0, 10.0, 10.0)), 40.0));
    assert!(near(geom::loop_length(&[[0.0, 0.0], [3.0, 4.0]]), 10.0));
    assert!(near(geom::dist([0.0, 0.0], [3.0, 4.0]), 5.0));
    assert_eq!(geom::dist([1.5, -2.0], [1.5, -2.0]), 0.0);
}

#[test]
fn the_box_around_loops_is_none_when_there_is_no_point() {
    assert_eq!(geom::bounds(&[]), None);
    assert_eq!(geom::bounds(&[Vec::new()]), None);
    let loops = vec![square(0.0, 0.0, 10.0, 10.0), square(-5.0, 2.0, 3.0, 20.0)];
    assert_eq!(geom::bounds(&loops), Some(([-5.0, 0.0], [10.0, 20.0])));
}

#[test]
fn a_point_is_inside_by_the_even_odd_rule_and_a_hole_is_outside() {
    let outer = square(0.0, 0.0, 10.0, 10.0);
    assert!(geom::inside([5.0, 5.0], std::slice::from_ref(&outer)));
    assert!(!geom::inside([15.0, 5.0], std::slice::from_ref(&outer)));
    assert!(!geom::inside([-1.0, -1.0], std::slice::from_ref(&outer)));
    let with_hole = vec![outer, reversed(&square(3.0, 3.0, 7.0, 7.0))];
    assert!(!geom::inside([5.0, 5.0], &with_hole));
    assert!(geom::inside([1.0, 1.0], &with_hole));
    assert!(!geom::inside([5.0, 5.0], &[]));
}

#[test]
fn a_point_is_never_inside_a_loop_that_is_a_point_or_a_line() {
    assert!(!geom::inside([0.0, 0.0], &[vec![[0.0, 0.0]]]));
    assert!(!geom::inside([1.0, 0.0], &[vec![[0.0, 0.0], [2.0, 0.0]]]));
    assert!(!geom::inside([1.0, 1.0], &[Vec::new()]));
}

#[test]
fn a_loop_that_crosses_itself_is_told_and_a_simple_one_is_not() {
    assert!(geom::self_crosses(&[
        [0.0, 0.0],
        [10.0, 10.0],
        [10.0, 0.0],
        [0.0, 10.0]
    ]));
    assert!(!geom::self_crosses(&square(0.0, 0.0, 10.0, 10.0)));
    assert!(!geom::self_crosses(&[[0.0, 0.0], [10.0, 0.0], [5.0, 8.0]]));
    assert!(!geom::self_crosses(&[]));
    assert!(!geom::self_crosses(&[[1.0, 1.0]]));
}

#[test]
fn a_bow_tie_is_cut_into_its_two_triangles_one_of_each_way_round() {
    let bow = [[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]];
    let ccw = geom::simple_loops(&bow, true);
    let cw = geom::simple_loops(&bow, false);
    assert_eq!((ccw.len(), cw.len()), (1, 1));
    assert!(near(geom::area(&ccw[0]), 25.0));
    assert!(near(geom::area(&cw[0]), -25.0));
}

#[test]
fn a_simple_loop_is_kept_whole_and_only_for_its_own_way_round() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    let kept = geom::simple_loops(&s, true);
    assert_eq!(kept.len(), 1);
    assert!(near(geom::area(&kept[0]), 100.0));
    assert!(geom::simple_loops(&s, false).is_empty());
    assert_eq!(geom::simple_loops(&reversed(&s), false).len(), 1);
}

#[test]
fn a_loop_with_no_area_or_too_few_points_gives_no_loop() {
    assert!(geom::simple_loops(&[], true).is_empty());
    assert!(geom::simple_loops(&[[0.0, 0.0], [1.0, 1.0]], true).is_empty());
    assert!(geom::simple_loops(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], true).is_empty());
    assert!(geom::simple_loops(&[[1.0, 1.0]; 5], true).is_empty());
}

#[test]
fn the_points_that_add_nothing_are_taken_out_of_a_loop() {
    let with_mid = vec![
        [0.0, 0.0],
        [5.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [0.0, 10.0],
    ];
    assert_eq!(
        geom::without_collinear(&with_mid),
        [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
    );
    let doubled = vec![
        [0.0, 0.0],
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [0.0, 10.0],
    ];
    assert_eq!(geom::without_collinear(&doubled).len(), 4);
    let closed = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]];
    assert_eq!(geom::without_collinear(&closed).len(), 3);
    assert!(geom::without_collinear(&[]).is_empty());
    assert_eq!(geom::without_collinear(&[[1.0, 1.0]]).len(), 1);
}

#[test]
fn a_square_is_inset_to_a_smaller_square() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    let in1 = geom::inset(&s, 1.0);
    assert_eq!(in1.len(), 1);
    assert!(near(geom::area(&in1[0]), 64.0));
    assert_eq!(geom::bounds(&in1), Some(([1.0, 1.0], [9.0, 9.0])));
    let same = geom::inset(&s, 0.0);
    assert!(near(geom::area(&same[0]), 100.0));
}

#[test]
fn an_inset_by_half_the_width_or_more_leaves_nothing() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    assert!(geom::inset(&s, 5.0).is_empty());
    assert!(geom::inset(&s, 6.0).is_empty());
    assert!(geom::inset(&s, 100.0).is_empty());
}

#[test]
fn a_round_pin_thinner_than_the_inset_leaves_nothing_and_a_fatter_one_leaves_a_loop() {
    let pin = |r: f64| -> Loop {
        (0..12)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 12.0;
                [r * a.cos(), r * a.sin()]
            })
            .collect()
    };
    assert!(geom::area(&pin(1.0)) > 0.0);
    assert_eq!(geom::inset(&pin(1.0), 0.5).len(), 1);
    assert!(geom::inset(&pin(1.0), 1.0).is_empty());
    assert!(geom::inset(&pin(1.0), 1.5).is_empty());
    let triangle: Loop = vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]];
    assert_eq!(geom::inset(&triangle, 0.5).len(), 1);
    assert!(geom::inset(&triangle, 3.0).is_empty());
}

#[test]
fn a_negative_inset_grows_the_loop_and_the_inset_of_a_hole_grows_the_hole() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    let out = geom::inset(&s, -1.0);
    assert!(near(geom::area(&out[0]), 144.0));
    let hole = geom::inset(&reversed(&s), 1.0);
    assert_eq!(hole.len(), 1);
    assert!(near(geom::area(&hole[0]), -144.0));
}

#[test]
fn an_inset_of_something_that_is_not_a_loop_or_by_a_number_that_is_not_a_number_is_nothing() {
    let s = square(0.0, 0.0, 10.0, 10.0);
    assert!(geom::inset(&s, f64::NAN).is_empty());
    assert!(geom::inset(&s, f64::INFINITY).is_empty());
    assert!(geom::inset(&[], 1.0).is_empty());
    assert!(geom::inset(&[[0.0, 0.0], [1.0, 1.0]], 0.1).is_empty());
    assert!(geom::inset(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], 0.1).is_empty());
}

#[test]
fn an_inset_never_gives_a_loop_that_is_bigger_than_the_one_it_came_from() {
    let mut rng = Rng::new(0x1A5E);
    for _ in 0..100 {
        let (w, h) = (rng.range(3.0, 40.0), rng.range(3.0, 40.0));
        let d = rng.range(0.05, 1.4);
        let s = square(0.0, 0.0, w, h);
        let in1 = geom::inset(&s, d);
        let total: f64 = in1.iter().map(|l| geom::area(l)).sum();
        assert!(total <= w * h + 1e-9);
        assert!(near(total, (w - 2.0 * d) * (h - 2.0 * d)), "{w} {h} {d}");
    }
}

#[test]
fn the_lines_of_an_infill_cross_the_square_from_side_to_side() {
    let s = vec![square(0.0, 0.0, 10.0, 10.0)];
    let lines = geom::scan_lines(&s, true, 3.0, 0.0);
    assert_eq!(lines.len(), 4);
    for (p, q) in &lines {
        assert_eq!((p[0], q[0]), (0.0, 10.0));
        assert_eq!(p[1], q[1]);
    }
    let up = geom::scan_lines(&s, false, 3.0, 0.0);
    assert_eq!(up.len(), 4);
    for (p, q) in &up {
        assert_eq!((p[1], q[1]), (0.0, 10.0));
        assert_eq!(p[0], q[0]);
    }
}

#[test]
fn a_hole_cuts_the_lines_that_cross_it_in_two() {
    let loops = vec![
        square(0.0, 0.0, 10.0, 10.0),
        reversed(&square(3.0, 3.0, 7.0, 7.0)),
    ];
    let lines = geom::scan_lines(&loops, true, 3.0, 0.0);
    assert_eq!(lines.len(), 6);
    for (p, q) in &lines {
        assert!(p[0] < q[0]);
    }
}

#[test]
fn the_lines_that_are_too_short_are_left_out_and_no_spacing_gives_no_line() {
    let s = vec![square(0.0, 0.0, 10.0, 10.0)];
    assert!(geom::scan_lines(&s, true, 3.0, 11.0).is_empty());
    for spacing in [0.0, -1.0, 1e-7, f64::NAN, f64::INFINITY] {
        assert!(
            geom::scan_lines(&s, true, spacing, 0.0).is_empty(),
            "{spacing}"
        );
    }
    assert!(geom::scan_lines(&[], true, 1.0, 0.0).is_empty());
    assert!(geom::scan_lines(&[Vec::new()], true, 1.0, 0.0).is_empty());
}

#[test]
fn the_distance_to_loops_is_to_the_nearest_edge_and_infinite_with_none() {
    let s = vec![square(0.0, 0.0, 10.0, 10.0)];
    assert!(near(distance_to_loops([5.0, 5.0], &s), 5.0));
    assert!(near(distance_to_loops([-3.0, 5.0], &s), 3.0));
    assert!(near(distance_to_loops([0.0, 0.0], &s), 0.0));
    assert!(near(distance_to_loops([13.0, 14.0], &s), 5.0));
    assert_eq!(distance_to_loops([1.0, 1.0], &[]), f64::INFINITY);
}

#[test]
fn any_points_given_to_the_geometry_never_stop_it() {
    let mut rng = Rng::new(0x6E0);
    for _ in 0..200 {
        let n = rng.below(12) as usize;
        let pts: Vec<[f64; 2]> = (0..n)
            .map(|_| [rng.range(-20.0, 20.0), rng.range(-20.0, 20.0)])
            .collect();
        let _ = geom::self_crosses(&pts);
        let _ = geom::simple_loops(&pts, true);
        let _ = geom::simple_loops(&pts, false);
        let _ = geom::without_collinear(&pts);
        let _ = geom::inset(&pts, rng.range(-2.0, 2.0));
        let _ = geom::scan_lines(std::slice::from_ref(&pts), rng.below(2) == 0, 1.5, 0.0);
    }
}

// ---------------------------------------------------------------- time

#[test]
fn a_block_between_two_points_has_its_length_and_its_direction() {
    let b = Block::planar([0.0, 0.0], [3.0, 4.0], 10.0);
    assert!(near(b.length_mm, 5.0));
    assert_eq!(b.top_speed_mm_s, 10.0);
    let d = b.direction.unwrap();
    assert!(near(d[0], 0.6) && near(d[1], 0.8));
    let still = Block::planar([1.0, 1.0], [1.0, 1.0], 10.0);
    assert_eq!((still.length_mm, still.direction), (0.0, None));
    let stop = Block::stop(2.0, 5.0);
    assert_eq!((stop.length_mm, stop.direction), (2.0, None));
}

#[test]
fn without_acceleration_the_time_is_the_length_over_the_speed() {
    let blocks = [Block::stop(10.0, 5.0), Block::stop(30.0, 10.0)];
    assert!(near(timing::flat_seconds(&blocks), 5.0));
    for bad_accel in [0.0, -5.0, f64::NAN, f64::INFINITY] {
        assert!(
            near(timing::seconds(&blocks, bad_accel, 5.0), 5.0),
            "{bad_accel}"
        );
    }
    assert_eq!(timing::flat_seconds(&[]), 0.0);
}

#[test]
fn the_blocks_that_cannot_be_driven_do_not_count() {
    let blocks = [
        Block::stop(f64::NAN, 5.0),
        Block::stop(10.0, 0.0),
        Block::stop(10.0, -3.0),
        Block::stop(f64::INFINITY, 5.0),
    ];
    assert_eq!(timing::flat_seconds(&blocks), 0.0);
    assert_eq!(timing::seconds(&blocks, 1000.0, 5.0), 0.0);
    assert_eq!(timing::seconds(&[], 1000.0, 5.0), 0.0);
    assert_eq!(timing::seconds(&[Block::stop(0.0, 5.0)], 1000.0, 5.0), 0.0);
}

#[test]
fn one_long_block_from_rest_to_rest_is_a_trapezoid_of_speed() {
    let t = timing::seconds(&[Block::stop(100.0, 100.0)], 1000.0, 5.0);
    assert!((t - 1.1).abs() < 1e-9, "{t}");
}

#[test]
fn one_short_block_is_a_triangle_of_speed_that_never_reaches_the_top() {
    let t = timing::seconds(&[Block::stop(1.0, 100.0)], 1000.0, 5.0);
    assert!((t - 2.0 * 1000f64.sqrt() / 1000.0).abs() < 1e-9, "{t}");
}

#[test]
fn a_straight_line_cut_in_two_takes_the_time_of_the_whole() {
    let whole = timing::seconds(
        &[Block::planar([0.0, 0.0], [100.0, 0.0], 100.0)],
        1000.0,
        5.0,
    );
    let halves = timing::seconds(
        &[
            Block::planar([0.0, 0.0], [50.0, 0.0], 100.0),
            Block::planar([50.0, 0.0], [100.0, 0.0], 100.0),
        ],
        1000.0,
        5.0,
    );
    assert!((whole - halves).abs() < 1e-9, "{whole} {halves}");
}

#[test]
fn a_turn_all_the_way_back_goes_down_to_the_corner_speed() {
    let there_and_back = [
        Block::planar([0.0, 0.0], [50.0, 0.0], 100.0),
        Block::planar([50.0, 0.0], [0.0, 0.0], 100.0),
    ];
    let slow = timing::seconds(&there_and_back, 1000.0, 0.5);
    let quick = timing::seconds(&there_and_back, 1000.0, 20.0);
    let stop_and_go = timing::stop_and_go_seconds(&there_and_back, 1000.0);
    assert!(quick < slow);
    assert!(slow <= stop_and_go + 1e-9);
    assert!(timing::flat_seconds(&there_and_back) < quick);
}

#[test]
fn a_machine_that_speeds_up_faster_is_never_slower() {
    let mut rng = Rng::new(0xACC);
    for _ in 0..100 {
        let mut at = [0.0, 0.0];
        let mut blocks = Vec::new();
        for _ in 0..(1 + rng.below(20)) {
            let to = [rng.range(0.0, 150.0), rng.range(0.0, 150.0)];
            blocks.push(Block::planar(at, to, rng.range(10.0, 150.0)));
            at = to;
        }
        let slow = timing::seconds(&blocks, 500.0, 5.0);
        let fast = timing::seconds(&blocks, 5000.0, 5.0);
        assert!(fast <= slow + 1e-9, "{fast} {slow}");
        assert!(slow >= timing::flat_seconds(&blocks) - 1e-9);
    }
}

// ---------------------------------------------------------------- the settings

fn set(key: &str, value: &str) -> Result<SliceConfig, String> {
    let mut c = SliceConfig::default();
    c.set(key, value).map(|()| c).map_err(|e| e.to_string())
}

/// A number setting takes its two ends and refuses what is just outside them, and a refusal changes nothing.
fn edges(key: &str, low: &str, high: &str, below: &str, above: &str) {
    for ok in [low, high] {
        let c = set(key, ok).unwrap_or_else(|e| panic!("{key} = {ok}: {e}"));
        assert_eq!(
            c.value_of(key).unwrap().parse::<f64>().unwrap(),
            ok.parse::<f64>().unwrap(),
            "{key}"
        );
    }
    for bad in [below, above, "x", "", "nan", "inf"] {
        let mut c = SliceConfig::default();
        let err = c.set(key, bad).unwrap_err().to_string();
        assert!(err.contains(key), "{key} = {bad}: {err}");
        assert_eq!(c, SliceConfig::default(), "{key} = {bad} changed something");
    }
}

#[test]
fn the_defaults_are_valid_and_the_text_reads_back_to_the_same_settings() {
    let c = SliceConfig::default();
    assert!(c.check().is_ok());
    assert_eq!(SliceConfig::from_text(&c.to_text()).unwrap(), c);
}

#[test]
fn every_key_of_the_slicer_has_a_value_and_takes_it_back() {
    assert_eq!(KEYS.len(), 86);
    let mut seen = std::collections::HashSet::new();
    let d = SliceConfig::default();
    for key in KEYS {
        assert!(seen.insert(key), "{key} twice");
        let v = d
            .value_of(key)
            .unwrap_or_else(|| panic!("{key} has no value"));
        let mut c = SliceConfig::default();
        c.set(key, &v)
            .unwrap_or_else(|e| panic!("{key} = {v}: {e}"));
        assert_eq!(c, d, "{key}");
    }
    assert!(d.value_of("colour").is_none());
    assert!(d.value_of("").is_none());
}

#[test]
fn the_text_of_the_settings_has_every_key_once_in_the_order_of_the_list() {
    let text = SliceConfig::default().to_text();
    let keys: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k))
        .collect();
    assert_eq!(keys, KEYS);
}

#[test]
fn a_key_that_is_not_a_setting_is_refused_by_name() {
    let mut c = SliceConfig::default();
    for key in ["colour", "", "Layer_Height_mm", "layer_height_mm "] {
        let e = c.set(key, "1").unwrap_err().to_string();
        assert!(e.contains("not a setting of the slicer"), "{key:?}: {e}");
    }
    assert_eq!(c, SliceConfig::default());
}

#[test]
fn the_ranges_of_the_layers_and_the_nozzle_have_their_edges() {
    edges("layer_height_mm", "0.05", "1", "0.04", "1.01");
    edges("first_layer_height_mm", "0.05", "1", "0.04", "1.01");
    edges("nozzle_diameter_mm", "0.1", "2", "0.09", "2.01");
    edges("line_width_mm", "0.1", "2.5", "0.09", "2.51");
    edges("filament_diameter_mm", "1", "3.5", "0.99", "3.51");
    edges("extrusion_multiplier", "0.5", "1.5", "0.49", "1.51");
}

#[test]
fn the_ranges_of_the_walls_and_the_infill_have_their_edges() {
    edges("perimeters", "1", "10", "0", "11");
    edges("top_layers", "0", "50", "-1", "51");
    edges("bottom_layers", "0", "50", "-1", "51");
    edges("infill_percent", "0", "100", "-0.1", "100.1");
    edges("width_variation_percent", "0", "50", "-0.1", "50.1");
}

#[test]
fn the_ranges_of_the_temperatures_and_the_fan_have_their_edges() {
    edges("nozzle_c", "0", "500", "-1", "500.5");
    edges("bed_c", "0", "200", "-1", "200.5");
    edges("chamber_c", "0", "120", "-1", "120.5");
    edges("fan_percent", "0", "100", "-1", "100.5");
    edges("fan_from_layer", "0", "100000", "-1", "100001");
    edges("bridge_fan_percent", "0", "100", "-1", "100.5");
    edges("fan_ramp_layers", "0", "1000", "-1", "1001");
}

#[test]
fn the_ranges_of_the_speeds_have_their_edges() {
    for key in [
        "first_layer_speed_mm_s",
        "perimeter_speed_mm_s",
        "external_perimeter_speed_mm_s",
        "infill_speed_mm_s",
        "travel_speed_mm_s",
        "max_speed_mm_s",
        "solid_infill_speed_mm_s",
        "bridge_speed_mm_s",
        "support_speed_mm_s",
        "min_speed_mm_s",
        "ironing_speed_mm_s",
    ] {
        edges(key, "1", "1000", "0.9", "1000.5");
    }
    edges("z_speed_mm_s", "0.1", "1000", "0.09", "1000.5");
    edges("retract_speed_mm_s", "1", "200", "0.9", "200.5");
    edges("accel_mm_s2", "50", "100000", "49", "100001");
    edges("corner_speed_mm_s", "0.5", "100", "0.4", "100.5");
}

#[test]
fn the_ranges_of_the_retraction_and_the_lifts_have_their_edges() {
    edges("retract_mm", "0", "20", "-0.1", "20.1");
    edges("retract_min_travel_mm", "0", "100", "-0.1", "100.1");
    edges("end_lift_mm", "0", "100", "-0.1", "100.1");
    edges("z_hop_mm", "0", "10", "-0.1", "10.1");
    edges("retract_full_travel_mm", "1", "500", "0.9", "500.5");
    edges("retract_min_percent", "0", "100", "-0.1", "100.1");
    edges("pressure_advance", "0", "2", "-0.1", "2.1");
}

#[test]
fn the_ranges_of_the_limits_have_their_edges() {
    edges("max_layers", "1", "100000", "0", "100001");
    edges("max_points_per_layer", "100", "10000000", "99", "10000001");
    edges("max_gcode_lines", "1000", "100000000", "999", "100000001");
    edges(
        "max_support_samples",
        "1000",
        "2000000000",
        "999",
        "2000000001",
    );
    edges("bed_margin_mm", "0", "100", "-0.1", "100.1");
}

#[test]
fn the_ranges_of_the_bridges_the_supports_and_the_adhesion_have_their_edges() {
    edges("bridge_flow_ratio", "0.5", "1.5", "0.49", "1.51");
    edges("max_bridge_span_mm", "0", "1000", "-1", "1001");
    edges("support_spacing_mm", "1", "20", "0.9", "20.5");
    edges("support_overhang_angle_deg", "1", "89", "0.9", "89.5");
    edges("support_xy_gap_mm", "0", "5", "-0.1", "5.1");
    edges("support_z_gap_layers", "0", "10", "-1", "11");
    edges("skirt_loops", "1", "20", "0", "21");
    edges("skirt_distance_mm", "0", "30", "-0.1", "30.1");
    edges("brim_width_mm", "0", "30", "-0.1", "30.1");
    edges("raft_layers", "1", "10", "0", "11");
    edges("raft_air_gap_mm", "0", "1", "-0.1", "1.1");
    edges("raft_margin_mm", "0", "20", "-0.1", "20.1");
}

#[test]
fn the_ranges_of_the_finishing_touches_have_their_edges() {
    edges("min_layer_time_s", "0", "600", "-1", "601");
    edges("ironing_flow_percent", "1", "30", "0.9", "30.5");
    edges("adaptive_min_layer_mm", "0.04", "1", "0.03", "1.01");
    edges("adaptive_max_layer_mm", "0.04", "1", "0.03", "1.01");
    edges("adaptive_cusp_mm", "0.005", "0.5", "0.004", "0.51");
    edges("elephant_foot_mm", "0", "2", "-0.1", "2.1");
    edges("max_volumetric_flow_mm3_s", "0", "100", "-0.1", "100.5");
}

#[test]
fn a_whole_number_setting_refuses_a_number_with_a_fraction() {
    for key in [
        "perimeters",
        "top_layers",
        "max_layers",
        "skirt_loops",
        "raft_layers",
        "fan_from_layer",
    ] {
        assert!(set(key, "2.5").is_err(), "{key}");
        assert!(set(key, "1e1").is_err(), "{key}");
    }
}

#[test]
fn the_words_of_a_setting_are_read_in_any_letter_case_and_the_others_are_refused() {
    let words: [(&str, &[&str]); 7] = [
        ("placement", &["center", "CENTRE", " Origin "]),
        ("thin_regions", &["skip", "REFUSE"]),
        ("infill_pattern", &["rectilinear", "Grid", "GYROID"]),
        ("seam", &["nearest", "Aligned", "REAR"]),
        ("supports", &["off", "BuildPlate", "everywhere"]),
        ("adhesion", &["none", "Skirt", "BRIM", "raft"]),
        ("vase_mode", &["on", "off", "TRUE", "0", "Yes"]),
    ];
    for (key, good) in words {
        for word in good {
            assert!(set(key, word).is_ok(), "{key} = {word}");
        }
        for bad in ["", "maybe", "centered", "2"] {
            assert!(set(key, bad).is_err(), "{key} = {bad}");
        }
    }
}

#[test]
fn a_line_of_the_file_that_is_wrong_is_refused_with_its_number() {
    for (text, line) in [
        ("layer_height_mm = 5\n", "line 1"),
        ("\n\nperimeters = 0\n", "line 3"),
        ("colour = red\n", "line 1"),
        ("infill_pattern = honeycomb\n", "line 1"),
        ("layer_height_mm\n", "line 1"),
    ] {
        let e = SliceConfig::from_text(text).unwrap_err().to_string();
        assert!(e.contains(line), "{text:?}: {e}");
    }
}

#[test]
fn the_last_line_of_a_key_wins_and_comments_are_ignored() {
    let c = SliceConfig::from_text("# a note\nperimeters = 3 # three\nperimeters = 4\n").unwrap();
    assert_eq!(c.perimeters, 4);
}

fn refused(text: &str) -> String {
    SliceConfig::from_text(text).unwrap_err().to_string()
}

#[test]
fn a_layer_taller_than_four_fifths_of_the_nozzle_is_refused() {
    assert!(refused("layer_height_mm = 0.4\n").contains("80% of the nozzle"));
    assert!(refused("first_layer_height_mm = 0.4\n").contains("80% of the nozzle"));
    assert!(
        SliceConfig::from_text("layer_height_mm = 0.32\nfirst_layer_height_mm = 0.32\n").is_ok()
    );
    assert!(SliceConfig::from_text(
        "nozzle_diameter_mm = 0.6\nline_width_mm = 0.6\nlayer_height_mm = 0.48\n"
    )
    .is_ok());
}

#[test]
fn a_line_width_outside_eighty_to_two_hundred_percent_of_the_nozzle_is_refused() {
    assert!(refused("line_width_mm = 0.31\n").contains("80% to 200%"));
    assert!(refused("line_width_mm = 0.81\n").contains("80% to 200%"));
    assert!(SliceConfig::from_text("line_width_mm = 0.32\n").is_ok());
    assert!(SliceConfig::from_text("line_width_mm = 0.8\n").is_ok());
}

#[test]
fn a_speed_above_the_top_speed_is_refused() {
    let e = refused("max_speed_mm_s = 30\n");
    assert!(e.contains("max_speed_mm_s"), "{e}");
    assert!(SliceConfig::from_text("max_speed_mm_s = 1000\ntravel_speed_mm_s = 1000\n").is_ok());
}

#[test]
fn the_width_of_a_line_that_varies_cannot_become_too_narrow() {
    assert!(refused("width_variation_percent = 50\n").contains("width_variation_percent"));
    assert!(SliceConfig::from_text("width_variation_percent = 40\n").is_ok());
}

#[test]
fn supports_cannot_be_closer_than_a_line_and_a_hop_cannot_be_under_half_a_layer() {
    let wide = "nozzle_diameter_mm = 1.5\nline_width_mm = 1.5\nsupports = everywhere\n";
    assert!(refused(&format!("{wide}support_spacing_mm = 1\n")).contains("support_spacing_mm"));
    assert!(SliceConfig::from_text(&format!("{wide}support_spacing_mm = 3\n")).is_ok());
    assert!(SliceConfig::from_text("support_spacing_mm = 1\n").is_ok());
    assert!(refused("z_hop_mm = 0.05\n").contains("z_hop_mm"));
    assert!(SliceConfig::from_text("z_hop_mm = 0.1\n").is_ok());
}

#[test]
fn a_brim_cannot_be_narrower_than_a_line() {
    assert!(refused("adhesion = brim\nbrim_width_mm = 0.3\n").contains("brim_width_mm"));
    assert!(SliceConfig::from_text("adhesion = brim\nbrim_width_mm = 0.45\n").is_ok());
    assert!(SliceConfig::from_text("adhesion = skirt\nbrim_width_mm = 0.3\n").is_ok());
}

#[test]
fn a_slowing_down_that_goes_above_the_top_speed_is_refused() {
    assert!(refused("min_layer_time_s = 10\nmin_speed_mm_s = 300\n").contains("min_speed_mm_s"));
    assert!(SliceConfig::from_text("min_speed_mm_s = 300\n").is_ok());
}

#[test]
fn a_vase_is_one_wall_and_refuses_what_does_not_go_with_it() {
    assert!(SliceConfig::from_text("vase_mode = on\n").is_ok());
    assert!(refused("vase_mode = on\nsupports = buildplate\n").contains("vase"));
    assert!(refused("vase_mode = on\nadhesion = raft\n").contains("vase"));
    assert!(refused("vase_mode = on\nironing = on\n").contains("vase"));
    assert!(refused("vase_mode = on\nadaptive_layers = on\n").contains("vase"));
}

#[test]
fn ironing_needs_a_top_and_a_speed_the_machine_can_do() {
    assert!(SliceConfig::from_text("ironing = on\n").is_ok());
    assert!(refused("ironing = on\ntop_layers = 0\n").contains("top_layers"));
    assert!(refused(
        "max_speed_mm_s = 100\ntravel_speed_mm_s = 100\nironing = on\nironing_speed_mm_s = 150\n"
    )
    .contains("ironing_speed_mm_s"));
}

#[test]
fn layers_that_change_their_height_have_their_own_rules() {
    assert!(SliceConfig::from_text("adaptive_layers = on\n").is_ok());
    assert!(
        refused("adaptive_layers = on\nadaptive_min_layer_mm = 0.3\n")
            .contains("adaptive_min_layer_mm")
    );
    assert!(
        refused("adaptive_layers = on\nadaptive_max_layer_mm = 0.4\n")
            .contains("adaptive_max_layer_mm")
    );
    assert!(SliceConfig::from_text("adaptive_max_layer_mm = 0.9\n").is_ok());
}

#[test]
fn a_flow_limit_under_one_and_a_foot_that_eats_the_wall_are_refused() {
    assert!(refused("max_volumetric_flow_mm3_s = 0.5\n").contains("max_volumetric_flow_mm3_s"));
    assert!(SliceConfig::from_text("max_volumetric_flow_mm3_s = 1\n").is_ok());
    assert!(refused("elephant_foot_mm = 0.9\n").contains("elephant_foot_mm"));
    assert!(SliceConfig::from_text("elephant_foot_mm = 0.85\n").is_ok());
}

#[test]
fn the_area_and_the_spacing_of_a_line_follow_from_its_width_and_height() {
    let c = SliceConfig::default();
    let area = 0.2 * (0.45 - 0.2) + std::f64::consts::PI * 0.2 * 0.2 / 4.0;
    assert!(near(c.line_area_mm2(0.2), area));
    assert!(near(c.line_area_for(0.45, 0.2), area));
    assert!(near(c.spacing_mm(0.2), area / 0.2));
    assert!(near(
        c.spacing_for(0.6, 0.2),
        c.line_area_for(0.6, 0.2) / 0.2
    ));
    assert!(near(
        c.filament_area_mm2(),
        std::f64::consts::PI * 0.875 * 0.875
    ));
    assert!(c.spacing_mm(0.2) < c.line_width_mm);
}

#[test]
fn the_area_of_a_line_grows_with_the_width_and_with_the_height() {
    let c = SliceConfig::default();
    assert!(c.line_area_for(0.5, 0.2) > c.line_area_for(0.45, 0.2));
    assert!(c.line_area_for(0.45, 0.25) > c.line_area_for(0.45, 0.2));
}

// ---------------------------------------------------------------- the filament

const PLA: &str = "id = test/pla\nname = Test PLA\nmaterial = pla\nnozzle_c = 210, 190, 230\nbed_c = 60, 50, 70\nfan_percent = 100, 80, 100\ndensity_g_cm3 = 1.24\ndiameter_mm = 1.75\nmin_nozzle_mm = 0.2\n";

fn pla() -> FilamentProfile {
    FilamentProfile::from_text(PLA).unwrap()
}

fn mk4s() -> p3dp_profiles::PrinterProfile {
    Catalog::builtin()
        .unwrap()
        .find("prusa-mk4s")
        .unwrap()
        .clone()
}

fn soft(d: f64) -> Nozzle {
    Nozzle {
        diameter_mm: d,
        hardened: false,
    }
}

#[test]
fn a_filament_that_goes_with_the_printer_sets_the_temperatures_the_fan_and_the_diameters() {
    let mut c = SliceConfig::default();
    c.apply_filament(&pla(), &mk4s(), soft(0.4)).unwrap();
    assert_eq!((c.nozzle_c, c.bed_c, c.fan_percent), (210.0, 60.0, 100.0));
    assert_eq!((c.filament_diameter_mm, c.nozzle_diameter_mm), (1.75, 0.4));
}

#[test]
fn a_filament_that_does_not_go_with_the_printer_is_refused_and_changes_nothing() {
    let mut abrasive = pla();
    abrasive.abrasive = true;
    let mut c = SliceConfig::default();
    let e = c
        .apply_filament(&abrasive, &mk4s(), soft(0.4))
        .unwrap_err()
        .to_string();
    assert!(e.contains("hardened"), "{e}");
    assert_eq!(c, SliceConfig::default());
    let e = c
        .apply_filament(&pla(), &mk4s(), soft(0.5))
        .unwrap_err()
        .to_string();
    assert!(e.contains("does not list"), "{e}");
    assert_eq!(c, SliceConfig::default());
    let hard = Nozzle {
        diameter_mm: 0.4,
        hardened: true,
    };
    assert!(c.apply_filament(&abrasive, &mk4s(), hard).is_ok());
}

#[test]
fn a_nozzle_that_leaves_the_settings_wrong_is_refused_and_changes_nothing() {
    let mut c = SliceConfig::default();
    let e = c.apply_filament(&pla(), &mk4s(), soft(0.6)).unwrap_err();
    assert!(matches!(e, SliceError::Config(_)), "{e}");
    assert_eq!(c, SliceConfig::default());
}

#[test]
fn a_filament_is_not_set_for_a_resin_printer() {
    let resin = Catalog::builtin()
        .unwrap()
        .find("elegoo-mars-4")
        .unwrap()
        .clone();
    let mut c = SliceConfig::default();
    assert!(c.apply_filament(&pla(), &resin, soft(0.4)).is_err());
    assert_eq!(c, SliceConfig::default());
}

// ---------------------------------------------------------------- the plan

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

fn with(change: impl FnOnce(&mut SliceConfig)) -> SliceConfig {
    let mut c = SliceConfig::default();
    change(&mut c);
    c
}

#[test]
fn a_cube_is_planned_in_layers_that_go_up_to_its_height() {
    let p = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
    assert!(
        p.layer_count() >= 99 && p.layer_count() <= 101,
        "{}",
        p.layer_count()
    );
    assert!((p.height_mm() - 20.0).abs() < 1e-6, "{}", p.height_mm());
    assert!((p.model_volume_mm3 - 8000.0).abs() < 1e-6);
    for w in p.layers.windows(2) {
        assert!(w[1].z_mm > w[0].z_mm);
        assert_eq!(w[1].index, w[0].index + 1);
    }
    assert!(p
        .layers
        .iter()
        .all(|l| l.height_mm > 0.0 && !l.paths.is_empty()));
    assert!(p.filament_mm() > 0.0);
}

#[test]
fn the_same_model_and_settings_give_the_same_plan_and_the_same_text() {
    let c = cube(15.0);
    let a = plan(&c, &machine(), &SliceConfig::default()).unwrap();
    let b = plan(&c, &machine(), &SliceConfig::default()).unwrap();
    assert_eq!(a, b);
    let (_, g1) = slice(&c, &machine(), &SliceConfig::default()).unwrap();
    let (_, g2) = slice(&c, &machine(), &SliceConfig::default()).unwrap();
    assert_eq!(g1, g2);
    assert!(g1.ends_with('\n') && g1.lines().count() > 100);
}

#[test]
fn taller_layers_give_fewer_of_them_and_more_infill_gives_more_plastic() {
    let m = cube(20.0);
    let base = plan(&m, &machine(), &SliceConfig::default()).unwrap();
    let tall = plan(&m, &machine(), &with(|c| c.layer_height_mm = 0.3)).unwrap();
    assert!(tall.layer_count() < base.layer_count());
    let thin = plan(&m, &machine(), &with(|c| c.infill_percent = 0.0)).unwrap();
    let dense = plan(&m, &machine(), &with(|c| c.infill_percent = 100.0)).unwrap();
    assert!(dense.filament_mm() > thin.filament_mm());
    let walls = plan(&m, &machine(), &with(|c| c.perimeters = 4)).unwrap();
    assert!(walls.filament_mm() > base.filament_mm());
}

#[test]
fn a_raft_adds_its_layers_under_the_model() {
    let m = cube(10.0);
    let plain = plan(&m, &machine(), &SliceConfig::default()).unwrap();
    let raft = plan(&m, &machine(), &with(|c| c.adhesion = Adhesion::Raft)).unwrap();
    assert_eq!(raft.raft_layers, 2);
    assert_eq!(plain.raft_layers, 0);
    assert_eq!(raft.layer_count(), plain.layer_count() + 2);
}

#[test]
fn a_temperature_above_what_the_printer_allows_is_refused_and_never_lowered() {
    let m = cube(10.0);
    for change in [
        with(|c| c.nozzle_c = 301.0),
        with(|c| c.bed_c = 101.0),
        with(|c| c.chamber_c = 40.0),
    ] {
        assert!(matches!(
            plan(&m, &machine(), &change),
            Err(SliceError::Temp(_))
        ));
    }
    assert!(plan(&m, &machine(), &with(|c| c.nozzle_c = 300.0)).is_ok());
    assert!(plan(&m, &machine(), &with(|c| c.bed_c = 0.0)).is_ok());
}

#[test]
fn a_resin_printer_is_refused() {
    let mut resin = machine();
    resin.dialect = Dialect::Msla;
    assert!(matches!(
        plan(&cube(10.0), &resin, &SliceConfig::default()),
        Err(SliceError::Machine(_))
    ));
}

#[test]
fn settings_that_do_not_go_together_are_refused_before_the_model_is_looked_at() {
    let bad = with(|c| c.layer_height_mm = 0.4);
    assert!(matches!(
        plan(&Mesh::new(), &machine(), &bad),
        Err(SliceError::Config(_))
    ));
}

#[test]
fn a_model_with_nothing_in_it_is_refused() {
    assert!(matches!(
        plan(&Mesh::new(), &machine(), &SliceConfig::default()),
        Err(SliceError::EmptyMesh)
    ));
    let flat = Mesh {
        vertices: cube(10.0).vertices,
        triangles: Vec::new(),
    };
    assert!(matches!(
        plan(&flat, &machine(), &SliceConfig::default()),
        Err(SliceError::EmptyMesh)
    ));
}

#[test]
fn a_model_with_a_hole_in_its_skin_is_refused_with_the_open_edges() {
    let mut m = cube(10.0);
    m.triangles.pop();
    match plan(&m, &machine(), &SliceConfig::default()) {
        Err(SliceError::OpenMesh { open_edges }) => assert!(open_edges > 0),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_model_that_faces_inwards_is_refused() {
    let mut m = cube(10.0);
    for t in &mut m.triangles {
        t.swap(1, 2);
    }
    assert!(matches!(
        plan(&m, &machine(), &SliceConfig::default()),
        Err(SliceError::InvertedMesh)
    ));
}

#[test]
fn a_model_taller_or_wider_than_the_printer_is_refused() {
    assert!(matches!(
        plan(&cube(300.0), &machine(), &SliceConfig::default()),
        Err(SliceError::DoesNotFit { .. })
    ));
    let mut small = machine();
    small.volume_mm = [30.0, 30.0, 256.0];
    assert!(matches!(
        plan(&cube(40.0), &small, &SliceConfig::default()),
        Err(SliceError::DoesNotFit { .. })
    ));
}

#[test]
fn more_layers_than_the_limit_are_refused_and_the_count_is_told() {
    let c = with(|c| c.max_layers = 10);
    match plan(&cube(20.0), &machine(), &c) {
        Err(SliceError::TooManyLayers { layers, max }) => {
            assert_eq!(max, 10);
            assert!(layers > 10);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_printer_profile_gives_a_machine_with_its_limits_and_a_resin_one_gives_none() {
    let catalog = Catalog::builtin().unwrap();
    for p in &catalog.profiles {
        match Machine::from_profile(p) {
            Ok(m) => {
                assert_eq!(m.id, p.id);
                assert_eq!(m.volume_mm, p.volume_mm);
                assert_eq!(Some(m.limits), p.limits);
                assert!(p.technology == p3dp_profiles::Technology::Fdm);
            }
            Err(SliceError::Machine(why)) => {
                assert!(
                    p.technology != p3dp_profiles::Technology::Fdm || p.limits.is_none(),
                    "{}: {why}",
                    p.id
                );
            }
            Err(other) => panic!("{}: {other:?}", p.id),
        }
    }
    let mut no_limits = mk4s();
    no_limits.limits = None;
    assert!(matches!(
        Machine::from_profile(&no_limits),
        Err(SliceError::Machine(_))
    ));
    assert!(Machine::from_profile(&mk4s()).is_ok());
}

#[test]
fn every_refusal_has_a_sentence() {
    let all = [
        SliceError::Config("a setting".into()),
        SliceError::Machine("a machine".into()),
        SliceError::EmptyMesh,
        SliceError::OpenMesh { open_edges: 3 },
        SliceError::InvertedMesh,
        SliceError::DoesNotFit {
            what: "too tall".into(),
        },
        SliceError::OpenLoop { layer: 2 },
        SliceError::TooManyLayers {
            layers: 11,
            max: 10,
        },
        SliceError::TooManyPoints {
            layer: 1,
            points: 9,
            max: 5,
        },
        SliceError::ThinRegion { layer: 4 },
        SliceError::TooManyLines { lines: 9, max: 5 },
    ];
    for e in &all {
        assert!(!e.to_string().is_empty(), "{e:?}");
    }
    assert_eq!(
        SliceError::Config("a setting".into()).to_string(),
        "a setting"
    );
    assert_eq!(
        SliceError::EmptyMesh.to_string(),
        "the model has no triangles"
    );
}
