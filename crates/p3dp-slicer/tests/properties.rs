//! Properties of the slicer over shapes made from a fixed seed: the plastic laid down is the volume of the part, nothing leaves the bed, the head only goes up, and no temperature goes
//! past the limits of the machine. The G-code is read back by a small reader of the tests, not by the code that wrote it.

use p3dp_core::machine::Zone;
use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Dialect, Limits, Mesh, TempLimits};
use p3dp_gcode::check_program;
use p3dp_slicer::{
    plan, slice, Adhesion, InfillPattern, Kind, Machine, Placement, Seam, SliceConfig, SliceError,
    Supports,
};

const BED: [f64; 3] = [220.0, 220.0, 250.0];

fn machine(dialect: Dialect) -> Machine {
    Machine {
        id: "bench-printer".to_string(),
        dialect,
        volume_mm: BED,
        limits: TempLimits {
            nozzle_c: 300.0,
            bed_c: 110.0,
            chamber_c: None,
        },
    }
}

type Tri = [[f64; 3]; 3];

fn mesh_of(soup: &[Tri]) -> Mesh {
    Mesh::from_soup(soup, &Limits::default()).unwrap()
}

/// A box with its corner at the origin.
fn block(x: f64, y: f64, z: f64) -> Mesh {
    let c = cube(1.0);
    let soup: Vec<Tri> = c
        .triangles
        .iter()
        .map(|t| {
            t.map(|i| {
                let v = c.vertices[i as usize].mm();
                [v[0] * x, v[1] * y, v[2] * z]
            })
        })
        .collect();
    mesh_of(&soup)
}

fn ring_point(n: usize, i: usize, r: f64, z: f64) -> [f64; 3] {
    let a = std::f64::consts::TAU * (i % n) as f64 / n as f64;
    [r * a.cos(), r * a.sin(), z]
}

/// A prism whose section is a regular polygon of `n` sides (counter-clockwise from above), with a hole of radius `hole` when it is not zero.
fn prism(n: usize, radius: f64, hole: f64, height: f64) -> Mesh {
    let mut soup: Vec<Tri> = Vec::new();
    for i in 0..n {
        let (a0, a1) = (
            ring_point(n, i, radius, 0.0),
            ring_point(n, i + 1, radius, 0.0),
        );
        let (b0, b1) = (
            ring_point(n, i, radius, height),
            ring_point(n, i + 1, radius, height),
        );
        // the outer wall faces out
        soup.push([a0, a1, b1]);
        soup.push([a0, b1, b0]);
        if hole > 0.0 {
            let (c0, c1) = (ring_point(n, i, hole, 0.0), ring_point(n, i + 1, hole, 0.0));
            let (d0, d1) = (
                ring_point(n, i, hole, height),
                ring_point(n, i + 1, hole, height),
            );
            // the inner wall faces the axis
            soup.push([c0, d1, c1]);
            soup.push([c0, d0, d1]);
            // the bottom ring faces down, the top ring up
            soup.push([a0, c1, a1]);
            soup.push([a0, c0, c1]);
            soup.push([b0, b1, d1]);
            soup.push([b0, d1, d0]);
        } else {
            let (o, t) = ([0.0, 0.0, 0.0], [0.0, 0.0, height]);
            soup.push([o, a1, a0]);
            soup.push([t, b0, b1]);
        }
    }
    mesh_of(&soup)
}

/// What a walk through the G-code found.
#[derive(Default, Debug)]
struct Walk {
    moves: usize,
    extruding: usize,
    /// The net filament pushed, in millimetres (the retractions count as negative).
    filament_mm: f64,
    max_z: f64,
    z_goes_down: bool,
    outside_bed: Vec<String>,
    layers: usize,
}

fn word(line: &str, letter: char) -> Option<f64> {
    line.split_ascii_whitespace().skip(1).find_map(|t| {
        let mut c = t.chars();
        (c.next()? == letter).then(|| c.as_str().parse().ok())?
    })
}

fn walk(gcode: &str) -> Walk {
    let mut w = Walk::default();
    let (mut x, mut y, mut z) = (0.0f64, 0.0f64, 0.0f64);
    let mut absolute = true;
    for raw in gcode.lines() {
        if raw.starts_with(";LAYER:") || raw.starts_with("; layer_num") {
            w.layers += 1;
        }
        let line = raw.split(';').next().unwrap_or("").trim();
        let cmd = line.split_ascii_whitespace().next().unwrap_or("");
        match cmd {
            "G91" => absolute = false,
            "G90" => absolute = true,
            "G1" if absolute => {
                w.moves += 1;
                let (nx, ny, nz) = (word(line, 'X'), word(line, 'Y'), word(line, 'Z'));
                if let Some(v) = nz {
                    if v < z - 1e-9 {
                        w.z_goes_down = true;
                    }
                    z = v;
                }
                x = nx.unwrap_or(x);
                y = ny.unwrap_or(y);
                if x < 0.0 || x > BED[0] || y < 0.0 || y > BED[1] || z < 0.0 || z > BED[2] {
                    w.outside_bed.push(line.to_string());
                }
                w.max_z = w.max_z.max(z);
                if let Some(e) = word(line, 'E') {
                    w.filament_mm += e;
                    if e > 0.0 && (nx.is_some() || ny.is_some()) {
                        w.extruding += 1;
                    }
                }
            }
            _ => {}
        }
    }
    w
}

fn solid() -> SliceConfig {
    SliceConfig {
        infill_percent: 100.0,
        ..SliceConfig::default()
    }
}

fn check_everything(mesh: &Mesh, cfg: &SliceConfig, dialect: Dialect, what: &str) -> f64 {
    let m = machine(dialect);
    let (p, g) = slice(mesh, &m, cfg).unwrap_or_else(|e| panic!("{what}: {e}"));
    // the plastic of the plan is the volume of the part
    let ratio = p.extrusion_mm3() / mesh.volume_mm3();
    assert!(
        (0.93..=1.07).contains(&ratio),
        "{what}: the plan lays down {ratio:.3} of the volume"
    );
    // the file says what the plan says, to the rounding of the amounts
    let w = walk(&g);
    let from_file = w.filament_mm * cfg.filament_area_mm2();
    let from_plan = p.extrusion_mm3();
    assert!(
        (from_file - from_plan).abs() / from_plan < 0.002,
        "{what}: file {from_file:.2} plan {from_plan:.2}"
    );
    // nothing leaves the bed and the head only goes up
    assert!(
        w.outside_bed.is_empty(),
        "{what}: {:?}",
        w.outside_bed.first()
    );
    assert!(!w.z_goes_down, "{what}: z goes down");
    assert!(
        w.max_z >= p.height_mm() - 1e-3 && w.max_z <= p.height_mm() + cfg.end_lift_mm + 1e-3,
        "{what}: {}",
        w.max_z
    );
    assert_eq!(w.layers, p.layer_count(), "{what}");
    assert!(w.extruding > 0);
    // no temperature past the limits, read by the checker of the program
    let violations = check_program(&g, &m.limits);
    assert!(violations.is_empty(), "{what}: {:?}", violations.first());
    for l in g.lines() {
        let code = l.split(';').next().unwrap_or("");
        if code.starts_with("M104") || code.starts_with("M109") {
            assert!(
                m.limits
                    .check(Zone::Nozzle, word(code, 'S').unwrap())
                    .is_ok(),
                "{what}: {l}"
            );
        }
        if code.starts_with("M140") || code.starts_with("M190") {
            assert!(
                m.limits.check(Zone::Bed, word(code, 'S').unwrap()).is_ok(),
                "{what}: {l}"
            );
        }
    }
    ratio
}

#[test]
fn shapes_from_a_fixed_seed_keep_to_the_properties() {
    let mut rng = Rng::new(0x5EED_3D17);
    let heights = [0.1, 0.15, 0.2, 0.25, 0.3];
    for case in 0..24 {
        let h = heights[rng.below(heights.len() as u64) as usize];
        let cfg = SliceConfig {
            layer_height_mm: h,
            first_layer_height_mm: h.clamp(0.2, 0.3),
            perimeters: 1 + rng.below(4) as u32,
            top_layers: 2 + rng.below(4) as u32,
            bottom_layers: 2 + rng.below(4) as u32,
            line_width_mm: [0.4, 0.45, 0.5][rng.below(3) as usize],
            ..solid()
        };
        let tall = rng.range(2.0, 14.0);
        let (mesh, what) = match case % 3 {
            0 => {
                let (x, y) = (rng.range(14.0, 60.0), rng.range(14.0, 60.0));
                (
                    block(x, y, tall),
                    format!("case {case}: block {x:.1} x {y:.1} x {tall:.1}"),
                )
            }
            1 => {
                let (n, r) = (8 + rng.below(40) as usize, rng.range(8.0, 30.0));
                (
                    prism(n, r, 0.0, tall),
                    format!("case {case}: prism of {n} sides, radius {r:.1}, height {tall:.1}"),
                )
            }
            _ => {
                let r = rng.range(12.0, 30.0);
                let hole = r * rng.range(0.3, 0.6);
                let n = 12 + rng.below(30) as usize;
                (prism(n, r, hole, tall), format!("case {case}: ring of {n} sides, radius {r:.1}, hole {hole:.1}, height {tall:.1}"))
            }
        };
        assert!(
            mesh.open_edges() == 0 && mesh.volume_mm3() > 0.0,
            "{what}: the test shape is not closed"
        );
        check_everything(&mesh, &cfg, Dialect::Marlin, &what);
    }
}

#[test]
fn the_same_part_in_every_dialect_keeps_to_the_properties() {
    let part = prism(24, 15.0, 6.0, 6.0);
    let cfg = solid();
    for d in [
        Dialect::Marlin,
        Dialect::Klipper,
        Dialect::Rrf,
        Dialect::Bambu,
    ] {
        check_everything(&part, &cfg, d, &format!("{d}"));
    }
}

#[test]
fn the_dialects_write_their_own_start_and_end() {
    let cfg = SliceConfig::default();
    let b = block(20.0, 20.0, 2.0);
    let text = |d| slice(&b, &machine(d), &cfg).unwrap().1;
    let klipper = text(Dialect::Klipper);
    assert!(
        klipper.contains("PRINT_START BED=60 EXTRUDER=210")
            && klipper.trim_end().ends_with("PRINT_END")
    );
    assert!(
        klipper.contains(";LAYER:0") && klipper.contains("SET_PRINT_STATS_INFO CURRENT_LAYER=1")
    );
    let marlin = text(Dialect::Marlin);
    assert!(
        marlin.contains("M190 S60")
            && marlin.contains("M109 S210")
            && marlin.trim_end().ends_with("M84")
    );
    let rrf = text(Dialect::Rrf);
    assert!(rrf.contains("G10 P0 S210") && !rrf.contains("M109"));
    let bambu = text(Dialect::Bambu);
    assert!(bambu.contains("; layer_num: 0") && bambu.contains("M73 P0 R"));
    assert!(bambu.contains("must be sent inside a package"));
}

#[test]
fn the_estimate_of_the_time_and_the_summaries_agree_with_the_plan() {
    let b = block(30.0, 30.0, 6.0);
    let p = plan(&b, &machine(Dialect::Marlin), &SliceConfig::default()).unwrap();
    let sums = p.summaries();
    assert_eq!(sums.len(), p.layer_count());
    let total: f64 = sums.iter().map(|s| s.extrusion_mm3).sum();
    assert!((total - p.extrusion_mm3()).abs() < 1e-6);
    assert!(sums.iter().all(|s| s.time_s > 0.0 && s.paths > 0));
    assert!(
        sums[0].time_s > sums[10].time_s * 0.5,
        "the first layer is slow"
    );
    assert!((p.estimated_seconds() - sums.iter().map(|s| s.time_s).sum::<f64>()).abs() < 1e-9);
    assert!(
        p.estimated_seconds() > 60.0 && p.estimated_seconds() < 3600.0,
        "{}",
        p.estimated_seconds()
    );
    // faster speeds, shorter time
    let fast = SliceConfig {
        infill_speed_mm_s: 120.0,
        perimeter_speed_mm_s: 80.0,
        external_perimeter_speed_mm_s: 60.0,
        ..SliceConfig::default()
    };
    let q = plan(&b, &machine(Dialect::Marlin), &fast).unwrap();
    assert!(q.estimated_seconds() < p.estimated_seconds());
}

#[test]
fn the_skins_are_solid_where_nothing_covers_and_the_infill_is_sparse_where_it_is_covered() {
    // a block with a step: 30 x 30 x 4 below and 10 x 10 x 4 on top of it
    let mut soup: Vec<Tri> = Vec::new();
    for (lo, hi, z0, z1) in [
        ([0.0, 0.0], [30.0, 30.0], 0.0, 4.0),
        ([10.0, 10.0], [20.0, 20.0], 4.0, 8.0),
    ] {
        let c = cube(1.0);
        for t in &c.triangles {
            soup.push(t.map(|i| {
                let v = c.vertices[i as usize].mm();
                [
                    lo[0] + v[0] * (hi[0] - lo[0]),
                    lo[1] + v[1] * (hi[1] - lo[1]),
                    z0 + v[2] * (z1 - z0),
                ]
            }));
        }
    }
    // the two boxes touch on a face: the mesh is closed, with a shared internal face in two copies that cancel in the volume
    let mesh = mesh_of(&soup);
    assert_eq!(mesh.open_edges(), 0);
    let cfg = SliceConfig {
        top_layers: 3,
        bottom_layers: 3,
        infill_percent: 20.0,
        layer_height_mm: 0.2,
        ..SliceConfig::default()
    };
    let p = plan(&mesh, &machine(Dialect::Marlin), &cfg).unwrap();
    // layer 18 is under the last layers of the lower block: where the 10 x 10 step does not cover it the skin is solid
    let layer = &p.layers[18];
    let solid_len: f64 = layer
        .paths
        .iter()
        .filter(|x| x.kind == Kind::SolidInfill)
        .map(|x| x.length_mm())
        .sum();
    assert!(solid_len > 100.0, "{solid_len}");
    // the middle of the lower block, away from the step and from the top, is sparse
    let low = &p.layers[8];
    assert!(low.paths.iter().any(|x| x.kind == Kind::Infill));
}

#[test]
fn a_part_is_refused_and_never_cut_to_fit() {
    let cfg = SliceConfig::default();
    let m = machine(Dialect::Marlin);
    // wider than the bed
    assert!(matches!(
        slice(&block(230.0, 10.0, 5.0), &m, &cfg),
        Err(SliceError::DoesNotFit { .. })
    ));
    // higher than the printer
    assert!(matches!(
        slice(&block(10.0, 10.0, 260.0), &m, &cfg),
        Err(SliceError::DoesNotFit { .. })
    ));
    // a temperature over the limit
    let hot = SliceConfig {
        nozzle_c: 320.0,
        ..cfg.clone()
    };
    assert!(matches!(
        slice(&block(10.0, 10.0, 5.0), &m, &hot),
        Err(SliceError::Temp(_))
    ));
    // fits exactly: the full bed is allowed with no margin, and refused with one
    let exact = block(220.0, 220.0, 1.0);
    let origin = SliceConfig {
        placement: Placement::Center,
        ..cfg.clone()
    };
    assert!(slice(&exact, &m, &origin).is_ok());
    let margin = SliceConfig {
        bed_margin_mm: 0.5,
        ..cfg
    };
    assert!(matches!(
        slice(&exact, &m, &margin),
        Err(SliceError::DoesNotFit { .. })
    ));
}

#[test]
fn what_is_written_is_what_the_same_settings_wrote_before() {
    let b = prism(20, 12.0, 4.0, 3.0);
    let cfg = SliceConfig::default();
    let m = machine(Dialect::Klipper);
    assert_eq!(
        slice(&b, &m, &cfg).unwrap().1,
        slice(&b, &m, &cfg).unwrap().1
    );
}

/// The G-code read again: every feed rate, every fan value and the highest z.
fn feeds_and_fans(gcode: &str) -> (f64, Vec<f64>) {
    let mut max_feed = 0.0f64;
    let mut fans = Vec::new();
    for raw in gcode.lines() {
        let line = raw.split(';').next().unwrap_or("").trim();
        if let Some(f) = word(line, 'F') {
            max_feed = max_feed.max(f);
        }
        if line.starts_with("M106") {
            fans.push(word(line, 'S').unwrap_or(255.0));
        }
    }
    (max_feed, fans)
}

#[test]
fn every_pattern_seam_and_support_keeps_to_the_limits_on_shapes_from_a_fixed_seed() {
    let mut rng = Rng::new(0xF111_5EED);
    let patterns = [
        InfillPattern::Rectilinear,
        InfillPattern::Grid,
        InfillPattern::Gyroid,
    ];
    let seams = [Seam::Nearest, Seam::Aligned, Seam::Rear];
    let supports = [Supports::Off, Supports::BuildPlate, Supports::Everywhere];
    for case in 0..18 {
        let cfg = SliceConfig {
            infill_pattern: patterns[case % 3],
            seam: seams[(case / 3) % 3],
            supports: supports[(case / 2) % 3],
            infill_percent: [10.0, 20.0, 40.0][rng.below(3) as usize],
            perimeters: 1 + rng.below(3) as u32,
            z_hop_mm: if case % 2 == 0 { 0.4 } else { 0.0 },
            fan_percent: [0.0, 60.0, 100.0][rng.below(3) as usize],
            fan_ramp_layers: rng.below(5) as u32,
            width_variation_percent: if case % 4 == 0 { 20.0 } else { 0.0 },
            ..SliceConfig::default()
        };
        let tall = rng.range(3.0, 9.0);
        let what = format!("case {case}: {cfg:?}");
        let mesh = match case % 3 {
            0 => block(rng.range(16.0, 40.0), rng.range(16.0, 40.0), tall),
            1 => prism(10 + rng.below(30) as usize, rng.range(8.0, 25.0), 0.0, tall),
            _ => {
                let r = rng.range(14.0, 28.0);
                prism(
                    16 + rng.below(20) as usize,
                    r,
                    r * rng.range(0.3, 0.5),
                    tall,
                )
            }
        };
        let m = machine(Dialect::Klipper);
        let (p, g) = slice(&mesh, &m, &cfg).unwrap_or_else(|e| panic!("{what}: {e}"));
        // plastic: more than nothing and not more than a solid part
        let ratio = p.extrusion_mm3() / mesh.volume_mm3();
        assert!(ratio > 0.2 && ratio < 1.07, "{what}: {ratio}");
        let w = walk(&g);
        assert!(
            w.outside_bed.is_empty(),
            "{what}: {:?}",
            w.outside_bed.first()
        );
        assert!(
            w.max_z <= p.height_mm() + cfg.end_lift_mm + cfg.z_hop_mm + 1e-3,
            "{what}"
        );
        let violations = check_program(&g, &m.limits);
        assert!(violations.is_empty(), "{what}: {:?}", violations.first());
        let (max_feed, fans) = feeds_and_fans(&g);
        assert!(
            max_feed <= cfg.max_speed_mm_s * 60.0 + 1e-6,
            "{what}: feed {max_feed}"
        );
        assert!(
            fans.iter().all(|s| (0.0..=255.0).contains(s)),
            "{what}: {fans:?}"
        );
        // the file and the plan agree on the plastic
        let from_file = w.filament_mm * cfg.filament_area_mm2();
        assert!(
            (from_file - p.extrusion_mm3()).abs() / p.extrusion_mm3() < 0.003,
            "{what}"
        );
        assert!(
            p.estimated_seconds() >= p.flat_estimated_seconds(),
            "{what}"
        );
    }
}

/// A table: two pillars 5 mm wide and a plate over them, 30 x 10 x 6 mm, made from its profile in x and z. The plate hangs over 20 mm of nothing.
fn table_mesh() -> Mesh {
    let poly: [[f64; 2]; 8] = [
        [0.0, 0.0],
        [5.0, 0.0],
        [5.0, 5.0],
        [25.0, 5.0],
        [25.0, 0.0],
        [30.0, 0.0],
        [30.0, 6.0],
        [0.0, 6.0],
    ];
    let caps: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 7],
        [2, 3, 6],
        [2, 6, 7],
        [4, 5, 6],
        [4, 6, 3],
    ];
    let at = |i: usize, y: f64| [poly[i][0], y, poly[i][1]];
    let mut soup: Vec<[[f64; 3]; 3]> = Vec::new();
    for t in caps {
        soup.push([at(t[0], 0.0), at(t[1], 0.0), at(t[2], 0.0)]);
        soup.push([at(t[0], 10.0), at(t[2], 10.0), at(t[1], 10.0)]);
    }
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        soup.push([at(i, 0.0), at(j, 10.0), at(j, 0.0)]);
        soup.push([at(i, 0.0), at(i, 10.0), at(j, 10.0)]);
    }
    Mesh::from_soup(&soup, &Limits::default()).unwrap()
}

#[test]
fn a_table_is_printed_with_a_bridge_or_with_supports_and_refused_when_neither_can() {
    let table = table_mesh();
    let m = machine(Dialect::Marlin);
    let with_bridge = SliceConfig::default();
    let (p, g) = slice(&table, &m, &with_bridge).unwrap();
    assert!(g.contains("TYPE:bridge") && !g.contains("TYPE:support"));
    assert!(p
        .layers
        .iter()
        .flat_map(|l| &l.paths)
        .any(|x| x.kind == Kind::Bridge));
    let short = SliceConfig {
        max_bridge_span_mm: 8.0,
        ..SliceConfig::default()
    };
    assert!(matches!(
        slice(&table, &m, &short),
        Err(SliceError::BridgeTooLong { .. })
    ));
    let held = SliceConfig {
        supports: Supports::Everywhere,
        ..short
    };
    let (_, g) = slice(&table, &m, &held).unwrap();
    assert!(g.contains("TYPE:support") && !g.contains("TYPE:bridge"));
    let w = walk(&g);
    assert!(w.outside_bed.is_empty());
}

/// The plastic of the lines that are not the skirt, the brim or the raft.
fn part_mm3(p: &p3dp_slicer::Plan) -> f64 {
    p.layers
        .iter()
        .map(|l| {
            l.paths
                .iter()
                .filter(|x| !matches!(x.kind, Kind::Adhesion | Kind::Raft))
                .map(|x| x.length_mm() * p.line_area_mm2(l.height_mm, x.kind, x.width_mm))
                .sum::<f64>()
        })
        .sum()
}

fn kind_mm(p: &p3dp_slicer::Plan, kind: Kind) -> f64 {
    p.layers
        .iter()
        .flat_map(|l| &l.paths)
        .filter(|x| x.kind == kind)
        .map(|x| x.length_mm())
        .sum()
}

#[test]
fn skirt_brim_and_raft_keep_to_the_properties_on_shapes_from_a_fixed_seed() {
    let mut rng = Rng::new(0xAD4E_5104);
    let kinds = [Adhesion::Skirt, Adhesion::Brim, Adhesion::Raft];
    for case in 0..18 {
        let adhesion = kinds[case % 3];
        let cfg = SliceConfig {
            adhesion,
            skirt_loops: 1 + rng.below(3) as u32,
            skirt_distance_mm: rng.range(1.0, 6.0),
            brim_width_mm: rng.range(2.0, 8.0),
            raft_layers: 1 + rng.below(3) as u32,
            raft_margin_mm: rng.range(0.0, 6.0),
            raft_air_gap_mm: [0.0, 0.1, 0.2][rng.below(3) as usize],
            infill_percent: 100.0,
            ..SliceConfig::default()
        };
        let tall = rng.range(3.0, 7.0);
        let mesh = match case % 2 {
            0 => block(rng.range(14.0, 40.0), rng.range(14.0, 40.0), tall),
            _ => prism(12 + rng.below(24) as usize, rng.range(8.0, 25.0), 0.0, tall),
        };
        let what = format!("case {case}: {adhesion:?}");
        let m = machine(Dialect::Klipper);
        let (p, g) = slice(&mesh, &m, &cfg).unwrap_or_else(|e| panic!("{what}: {e}"));
        // the part is the part: the skirt, the brim and the raft are extra
        let ratio = part_mm3(&p) / mesh.volume_mm3();
        assert!((0.93..=1.07).contains(&ratio), "{what}: {ratio}");
        let w = walk(&g);
        assert!(
            w.outside_bed.is_empty(),
            "{what}: {:?}",
            w.outside_bed.first()
        );
        assert!(!w.z_goes_down, "{what}");
        assert_eq!(w.layers, p.layer_count(), "{what}");
        let from_file = w.filament_mm * cfg.filament_area_mm2();
        assert!(
            (from_file - p.extrusion_mm3()).abs() / p.extrusion_mm3() < 0.003,
            "{what}"
        );
        assert!(check_program(&g, &m.limits).is_empty(), "{what}");
        match adhesion {
            Adhesion::Skirt | Adhesion::Brim => {
                assert_eq!(p.raft_layers, 0);
                assert!(
                    kind_mm(&p, Kind::Adhesion) > 0.0 && kind_mm(&p, Kind::Raft) == 0.0,
                    "{what}"
                );
                // only the first layer has them
                assert!(
                    p.layers[1..]
                        .iter()
                        .all(|l| l.paths.iter().all(|x| x.kind != Kind::Adhesion)),
                    "{what}"
                );
                assert_eq!(p.layer_count() as f64, (tall / 0.2).ceil(), "{what}");
            }
            Adhesion::Raft => {
                let r = cfg.raft_layers as usize;
                assert_eq!(p.raft_layers, r, "{what}");
                assert!(
                    p.layers[..r]
                        .iter()
                        .all(|l| l.paths.iter().all(|x| x.kind == Kind::Raft)),
                    "{what}"
                );
                assert!(
                    p.layers[r..]
                        .iter()
                        .all(|l| l.paths.iter().all(|x| x.kind != Kind::Raft)),
                    "{what}"
                );
                // the part stands above the raft by its height and the air gap
                let raft_top = p.layers[r - 1].z_mm;
                let first = &p.layers[r];
                assert!(
                    (first.z_mm - (raft_top + cfg.raft_air_gap_mm + first.height_mm)).abs() < 1e-9,
                    "{what}"
                );
                assert!(
                    (p.height_mm() - (tall + raft_top + cfg.raft_air_gap_mm)).abs() < 2e-3,
                    "{what}"
                );
            }
            Adhesion::None => unreachable!(),
        }
    }
}

#[test]
fn the_skirt_or_the_raft_that_does_not_fit_is_refused() {
    let m = machine(Dialect::Klipper);
    // a part 200 mm wide on a bed 220 mm wide: a skirt 20 mm away cannot be laid
    let wide = block(200.0, 40.0, 5.0);
    let skirt = SliceConfig {
        adhesion: Adhesion::Skirt,
        skirt_distance_mm: 20.0,
        ..SliceConfig::default()
    };
    match plan(&wide, &m, &skirt) {
        Err(SliceError::DoesNotFit { what }) => assert!(what.contains("leaves the bed"), "{what}"),
        other => panic!("{other:?}"),
    }
    // a part as tall as the printer cannot stand on a raft
    let tall = block(20.0, 20.0, 249.5);
    let raft = SliceConfig {
        adhesion: Adhesion::Raft,
        raft_layers: 3,
        ..SliceConfig::default()
    };
    match plan(&tall, &m, &raft) {
        Err(SliceError::DoesNotFit { what }) => assert!(what.contains("with the raft"), "{what}"),
        other => panic!("{other:?}"),
    }
    // the same settings without the adhesion print
    assert!(plan(&tall, &m, &SliceConfig::default()).is_ok());
}

#[test]
fn a_vase_is_one_wall_with_a_solid_base_and_nothing_inside() {
    let m = machine(Dialect::Marlin);
    let cfg = SliceConfig {
        vase_mode: true,
        bottom_layers: 3,
        perimeters: 3,
        top_layers: 5,
        infill_percent: 20.0,
        ..SliceConfig::default()
    };
    let mesh = prism(48, 20.0, 0.0, 12.0);
    let (p, g) = slice(&mesh, &m, &cfg).unwrap();
    for l in &p.layers {
        let walls = l
            .paths
            .iter()
            .filter(|x| matches!(x.kind, Kind::ExternalPerimeter | Kind::Perimeter))
            .count();
        assert_eq!(walls, 1, "layer {}", l.index);
        let fill = l
            .paths
            .iter()
            .any(|x| matches!(x.kind, Kind::Infill | Kind::SolidInfill | Kind::Bridge));
        assert_eq!(fill, l.index < 3, "layer {}", l.index);
    }
    // the base is solid and the wall is a thin shell of the volume
    let ratio = p.extrusion_mm3() / mesh.volume_mm3();
    assert!(ratio < 0.2, "{ratio}");
    assert!(check_program(&g, &m.limits).is_empty());
    assert!(walk(&g).outside_bed.is_empty());
    // a part with a hole has two outlines: refused with the layer, and the plain print is not
    let ring = prism(32, 20.0, 8.0, 12.0);
    match plan(&ring, &m, &cfg) {
        Err(SliceError::VaseNeedsOneOutline { layer, outlines }) => {
            assert_eq!((layer, outlines), (3, 2));
        }
        other => panic!("{other:?}"),
    }
    assert!(plan(&ring, &m, &SliceConfig::default()).is_ok());
    // it cannot be asked for with supports or on a raft
    for bad in [
        SliceConfig {
            vase_mode: true,
            supports: Supports::Everywhere,
            ..SliceConfig::default()
        },
        SliceConfig {
            vase_mode: true,
            adhesion: Adhesion::Raft,
            ..SliceConfig::default()
        },
    ] {
        assert!(matches!(plan(&mesh, &m, &bad), Err(SliceError::Config(_))));
    }
}

#[test]
fn a_short_layer_is_slowed_down_to_the_time_asked_and_never_below_the_floor() {
    let m = machine(Dialect::Klipper);
    let base = SliceConfig::default();
    let slow = SliceConfig {
        min_layer_time_s: 15.0,
        min_speed_mm_s: 12.0,
        ..SliceConfig::default()
    };
    // a small tower: its layers are short
    let mesh = block(12.0, 12.0, 6.0);
    let (p0, _) = slice(&mesh, &m, &base).unwrap();
    let (p1, g1) = slice(&mesh, &m, &slow).unwrap();
    assert!(p0.layer_scales.iter().all(|s| *s == 1.0));
    assert!(p1.layer_scales[1..].iter().any(|s| *s < 1.0));
    assert_eq!(p1.layer_scales[0], 1.0, "the first layer is slow already");
    for l in &p1.layers {
        let line_time: f64 = l
            .paths
            .iter()
            .map(|x| x.length_mm() / p1.speed_mm_s(l.index, x.kind))
            .sum();
        // a slow kind raised to the floor takes the time of the others back: the layer is then as slow as the floor lets it be
        let at_floor = l.paths.iter().any(|x| {
            let b = p1.base_speed_mm_s(l.index, x.kind);
            b * p1.layer_scales[l.index] < slow.min_speed_mm_s.min(b) - 1e-12
        });
        if p1.layer_scales[l.index] < 1.0 {
            assert!(
                line_time >= slow.min_layer_time_s - 1e-6 || at_floor,
                "layer {}: {line_time}",
                l.index
            );
        }
        for x in &l.paths {
            let (v0, v1) = (
                p0.speed_mm_s(l.index, x.kind),
                p1.speed_mm_s(l.index, x.kind),
            );
            assert!(v1 <= v0 + 1e-12 && v1 >= slow.min_speed_mm_s.min(v0) - 1e-12);
        }
    }
    assert!(p1.estimated_seconds() > p0.estimated_seconds());
    // the file says what the plan says
    let (max_feed, _) = feeds_and_fans(&g1);
    assert!(max_feed <= slow.max_speed_mm_s * 60.0 + 1e-6);
    // a layer that is long enough is left alone
    let big = block(60.0, 60.0, 2.0);
    let (p2, _) = slice(
        &big,
        &m,
        &SliceConfig {
            min_layer_time_s: 5.0,
            ..SliceConfig::default()
        },
    )
    .unwrap();
    assert!(
        p2.layer_scales.iter().all(|s| *s == 1.0),
        "{:?}",
        p2.layer_scales
    );
}

#[test]
fn the_retraction_follows_the_travel_when_it_is_adaptive_and_the_plastic_is_the_same() {
    let m = machine(Dialect::Klipper);
    let base = SliceConfig {
        retract_mm: 2.0,
        retract_min_travel_mm: 1.0,
        ..SliceConfig::default()
    };
    let adaptive = SliceConfig {
        retract_adaptive: true,
        retract_full_travel_mm: 30.0,
        retract_min_percent: 25.0,
        ..base.clone()
    };
    let mesh = prism(30, 20.0, 8.0, 5.0);
    let (p0, g0) = slice(&mesh, &m, &base).unwrap();
    let (p1, g1) = slice(&mesh, &m, &adaptive).unwrap();
    let pulls = |g: &str| -> Vec<f64> {
        g.lines()
            .filter_map(|l| {
                let code = l.split(';').next().unwrap_or("");
                (code.starts_with("G1") && word(code, 'X').is_none() && word(code, 'Z').is_none())
                    .then(|| word(code, 'E'))
                    .flatten()
                    .filter(|e| *e < 0.0)
                    .map(|e| -e)
            })
            .collect()
    };
    let (a, b) = (pulls(&g0), pulls(&g1));
    assert!(!a.is_empty() && !b.is_empty());
    assert!(a.iter().all(|x| (x - 2.0).abs() < 1e-9));
    assert!(
        b.iter()
            .all(|x| *x <= 2.0 + 1e-9 && *x >= 2.0 * 0.25 - 1e-9),
        "{b:?}"
    );
    assert!(
        b.iter().any(|x| *x < 2.0 - 1e-6),
        "no short travel was found"
    );
    // what is pulled back is pushed again, so the net plastic is that of the plan in both
    for (p, g) in [(&p0, &g0), (&p1, &g1)] {
        let w = walk(g);
        let from_file = w.filament_mm * base.filament_area_mm2();
        assert!((from_file - p.extrusion_mm3()).abs() / p.extrusion_mm3() < 0.003);
    }
    // the function that decides it
    assert!((p1.retract_for_travel(0.0) - 0.5).abs() < 1e-9);
    assert!((p1.retract_for_travel(15.0) - 1.25).abs() < 1e-9);
    assert!((p1.retract_for_travel(500.0) - 2.0).abs() < 1e-9);
    assert!((p0.retract_for_travel(3.0) - 2.0).abs() < 1e-9);
}

#[test]
fn the_times_with_acceleration_lie_between_the_plain_time_and_the_stop_and_go_time() {
    let mut rng = Rng::new(0x713E_5EED);
    let m = machine(Dialect::Marlin);
    for case in 0..8 {
        let cfg = SliceConfig {
            accel_mm_s2: rng.range(300.0, 12_000.0),
            corner_speed_mm_s: rng.range(1.0, 20.0),
            infill_percent: [15.0, 40.0][rng.below(2) as usize],
            adhesion: [Adhesion::None, Adhesion::Brim, Adhesion::Raft][case % 3],
            ..SliceConfig::default()
        };
        let mesh = block(
            rng.range(15.0, 45.0),
            rng.range(15.0, 45.0),
            rng.range(2.0, 5.0),
        );
        let (p, g) = slice(&mesh, &m, &cfg).unwrap();
        let t = p.time_check();
        assert!(t.is_consistent(), "case {case}: {t:?}");
        assert!((t.estimate_s - p.estimated_seconds()).abs() < 1e-9);
        assert!((t.flat_s - p.flat_estimated_seconds()).abs() < 1e-9);
        // the minutes written in the file are the estimate rounded up
        let want = (t.estimate_s / 60.0).ceil() as u64;
        let said = g
            .lines()
            .find_map(|l| l.strip_prefix("M73 P0 R"))
            .and_then(|r| r.trim().parse::<u64>().ok());
        if let Some(said) = said {
            assert_eq!(said, want, "case {case}");
        }
        // the sum of the layers is the whole
        let sum: f64 = p.summaries().iter().map(|s| s.time_s).sum();
        assert!((sum - t.estimate_s).abs() < 1e-9);
    }
    // a stronger acceleration never makes it slower, and a huge one is the plain time
    let mesh = block(30.0, 30.0, 3.0);
    let at = |a: f64| {
        let cfg = SliceConfig {
            accel_mm_s2: a,
            ..SliceConfig::default()
        };
        slice(&mesh, &m, &cfg).unwrap().0.estimated_seconds()
    };
    let (slow, quick, huge) = (at(500.0), at(5000.0), at(100_000.0));
    assert!(slow > quick && quick >= huge);
    let flat = slice(&mesh, &m, &SliceConfig::default())
        .unwrap()
        .0
        .flat_estimated_seconds();
    assert!(huge >= flat && huge < flat * 1.5, "{huge} {flat}");
}
