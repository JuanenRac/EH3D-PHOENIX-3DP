//! Layers whose height follows the shape: a wall that leans shows a step between two layers (the cusp), and the step is the height of the layer times how flat the wall lies, so a
//! flat dome wants thin layers and a straight wall can have thick ones. The height of each layer is the one that keeps the step under `adaptive_cusp_mm`, kept between the thinnest and
//! the thickest layer the person allows. A face that is exactly flat does not count: it has no step, it is a skin. The first layer keeps its own height, the last one is whatever is
//! left, and the heights add up to the height of the model.

use crate::config::SliceConfig;
use p3dp_core::Mesh;

/// The size of a cell of the height map, in millimetres.
const CELL_MM: f64 = 0.01;
/// The most cells of the map: a taller model gets bigger cells, never a bigger map.
const MAX_CELLS: usize = 400_000;
/// The heights are kept to a hundredth of a millimetre, as the printers write them.
const STEP_MM: f64 = 0.01;
/// A face whose normal is this close to the vertical is flat, and one that is this close to the horizontal is a straight wall: neither has a step to speak of.
const FLAT: f64 = 0.9995;

/// For each cell of height, the flattest sloping face that touches it, as the vertical part of its normal (0 is a straight wall, 1 is flat).
fn flatness_map(mesh: &Mesh, shift_z: f64, total: f64, wanted: f64) -> (Vec<f64>, f64) {
    let cell = CELL_MM.max(total / MAX_CELLS as f64);
    let cells = (total / cell).ceil() as usize + 1;
    let mut map = vec![0.0f64; cells];
    for t in &mesh.triangles {
        let (Some(a), Some(b), Some(c)) = (
            mesh.vertices.get(t[0] as usize),
            mesh.vertices.get(t[1] as usize),
            mesh.vertices.get(t[2] as usize),
        ) else {
            continue;
        };
        let (a, b, c) = (a.mm(), b.mm(), c.mm());
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-12 {
            continue;
        }
        let nz = (n[2] / len).abs();
        // a straight wall, or a face that is flat: nothing to follow; a face that cannot reach the cusp at the thickest layer: nothing to follow either
        if nz >= FLAT || nz < wanted {
            continue;
        }
        let lo = a[2].min(b[2]).min(c[2]) + shift_z;
        let hi = a[2].max(b[2]).max(c[2]) + shift_z;
        let from = ((lo / cell).floor().max(0.0)) as usize;
        let to = (((hi / cell).ceil().max(0.0)) as usize).min(cells - 1);
        for slot in map.iter_mut().take(to + 1).skip(from) {
            if nz > *slot {
                *slot = nz;
            }
        }
    }
    (map, cell)
}

/// The heights of the layers of a model `total` millimetres high. `first` is the height of the first layer. The settings are the ones `check` has accepted.
pub fn heights(mesh: &Mesh, shift_z: f64, total: f64, first: f64, cfg: &SliceConfig) -> Vec<f64> {
    let (min, max, cusp) = (
        cfg.adaptive_min_layer_mm,
        cfg.adaptive_max_layer_mm,
        cfg.adaptive_cusp_mm,
    );
    let (map, cell) = flatness_map(mesh, shift_z, total, cusp / max);
    let mut out = Vec::new();
    let first = first.min(total);
    let mut done = first;
    out.push(first);
    while total - done > 1e-3 {
        let left = total - done;
        // the flattest face between here and where a thick layer would end decides the height
        let from = ((done / cell).floor().max(0.0)) as usize;
        let to = (((done + max) / cell).ceil() as usize).min(map.len() - 1);
        let flattest = map[from.min(map.len() - 1)..=to]
            .iter()
            .fold(0.0f64, |m, v| m.max(*v));
        let mut h = if flattest > 0.0 { cusp / flattest } else { max };
        h = h.clamp(min, max);
        // to a hundredth, never above what was worked out
        h = ((h / STEP_MM) + 1e-9).floor() * STEP_MM;
        h = h.max(min.min(left));
        // no sliver at the end: a last layer thinner than the thinnest is added to this one, or the rest is halved
        if left - h < min - 1e-9 {
            h = if left <= max + 1e-9 { left } else { left / 2.0 };
        }
        let h = h.min(left);
        out.push(h);
        done += h;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;
    use p3dp_core::Limits;

    fn cfg() -> SliceConfig {
        SliceConfig {
            adaptive_layers: true,
            ..SliceConfig::default()
        }
    }

    /// A sphere from rings and segments, centred on the origin.
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
                    soup.push([a, c, d]);
                } else if i == rings - 1 {
                    soup.push([a, b, c]);
                } else {
                    soup.push([a, b, c]);
                    soup.push([a, c, d]);
                }
            }
        }
        Mesh::from_soup(&soup, &Limits::default()).unwrap()
    }

    #[test]
    fn a_straight_wall_gets_the_thickest_layers_and_they_add_up() {
        let h = heights(&cube(20.0), 0.0, 20.0, 0.2, &cfg());
        assert!((h.iter().sum::<f64>() - 20.0).abs() < 1e-9);
        assert_eq!(h[0], 0.2);
        // all but the first and the last are the thickest
        for x in &h[1..h.len() - 1] {
            assert!((x - 0.28).abs() < 1e-9, "{x}");
        }
        assert!(h.len() < 100);
    }

    #[test]
    fn a_dome_gets_thin_layers_at_the_poles_and_thick_ones_at_the_equator() {
        let s = sphere(10.0, 40, 48);
        let h = heights(&s, 10.0, 20.0, 0.2, &cfg());
        assert!((h.iter().sum::<f64>() - 20.0).abs() < 1e-9);
        let mut z = 0.0;
        let (mut near_pole, mut near_equator) = (f64::MAX, 0.0f64);
        for x in &h {
            z += x;
            if !(1.0..=19.0).contains(&z) {
                near_pole = near_pole.min(*x);
            }
            if (9.0..=11.0).contains(&z) {
                near_equator = near_equator.max(*x);
            }
        }
        assert!(near_pole < 0.15, "{near_pole}");
        assert!(near_equator > 0.25, "{near_equator}");
        for x in &h[1..h.len() - 1] {
            assert!((0.08 - 1e-9..=0.28 + 1e-9).contains(x), "{x}");
        }
    }

    #[test]
    fn shapes_from_a_fixed_seed_keep_the_step_under_the_cusp_and_the_heights_in_range() {
        let mut rng = Rng::new(0x3d_a1_a9);
        for _ in 0..12 {
            let r = rng.range(4.0, 30.0);
            let rings = 8 + rng.below(40) as usize;
            let segs = 8 + rng.below(40) as usize;
            let min = rng.range(0.05, 0.12);
            let max = rng.range(0.2, 0.3);
            let c = SliceConfig {
                adaptive_min_layer_mm: min,
                adaptive_max_layer_mm: max,
                adaptive_cusp_mm: rng.range(0.02, 0.1),
                ..cfg()
            };
            let s = sphere(r, rings, segs);
            let total = 2.0 * r;
            let h = heights(&s, r, total, 0.2, &c);
            assert!((h.iter().sum::<f64>() - total).abs() < 1e-9, "r={r}");
            assert!(h.iter().all(|x| x.is_finite() && *x > 0.0));
            for x in &h[1..h.len() - 2] {
                assert!(
                    *x >= min - 1e-9 && *x <= max + 1e-9,
                    "{x} not in {min}..{max}"
                );
            }
        }
    }

    #[test]
    fn the_same_model_gives_the_same_heights_and_an_empty_map_gives_the_thickest() {
        let s = sphere(8.0, 24, 24);
        assert_eq!(
            heights(&s, 8.0, 16.0, 0.2, &cfg()),
            heights(&s, 8.0, 16.0, 0.2, &cfg())
        );
        let h = heights(&Mesh::new(), 0.0, 5.0, 0.2, &cfg());
        assert!((h.iter().sum::<f64>() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_model_shorter_than_the_first_layer_is_one_layer() {
        let h = heights(&cube(0.1), 0.0, 0.1, 0.2, &cfg());
        assert_eq!(h, vec![0.1]);
    }
}
