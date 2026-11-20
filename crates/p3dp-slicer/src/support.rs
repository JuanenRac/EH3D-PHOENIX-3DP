//! Simple supports. The part is sampled on a square grid of columns; a column whose material starts over empty space and has no lower material inside the cone that the overhang
//! angle allows gets lines of support under it, down to the bed or to the part below, with a gap of a few layers at the top and a clearance from the walls at the sides. The lines
//! of a layer run along x on the even layers and along y on the odd ones, so they make a mesh. It is a first, plain support: no interface layers, no tree, no painting.
//!
//! Nothing is cut down to make it work: an overhang that would have to stand on the part while the supports are limited to the bed is an error, and so is a decision that would take
//! more samples than the limit.

use crate::config::{SliceConfig, Supports};
use crate::error::SliceError;
use crate::geom::{inside, Loop, P};
use crate::plan::{Kind, Path};
use std::collections::BTreeSet;

/// The distance from a point to the nearest edge of a set of closed loops (infinite when there are none).
pub fn distance_to_loops(p: P, loops: &[Loop]) -> f64 {
    let mut best = f64::INFINITY;
    for l in loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            let t = if len2 < 1e-18 {
                0.0
            } else {
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
            };
            let q = [a[0] + t * dx, a[1] + t * dy];
            best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt());
        }
    }
    best
}

/// The support paths of every layer (empty when the supports are off), in no particular order.
pub(crate) fn support_paths(
    outlines: &[Vec<Loop>],
    heights: &[f64],
    cfg: &SliceConfig,
) -> Result<Vec<Vec<Path>>, SliceError> {
    let n = outlines.len();
    let mut out: Vec<Vec<Path>> = vec![Vec::new(); n];
    if cfg.supports == Supports::Off || n < 2 {
        return Ok(out);
    }
    let all: Vec<Loop> = outlines.iter().flatten().cloned().collect();
    let Some((lo, hi)) = crate::geom::bounds(&all) else {
        return Ok(out);
    };
    let pitch = cfg.support_spacing_mm;
    let (x0, x1) = (
        (lo[0] / pitch).ceil() as i64,
        (hi[0] / pitch).floor() as i64,
    );
    let (y0, y1) = (
        (lo[1] / pitch).ceil() as i64,
        (hi[1] / pitch).floor() as i64,
    );
    let columns = ((x1 - x0 + 1).max(0) as u64) * ((y1 - y0 + 1).max(0) as u64);
    let samples = columns.saturating_mul(n as u64);
    if samples > u64::from(cfg.max_support_samples) {
        return Err(SliceError::TooManySupportSamples {
            samples,
            max: u64::from(cfg.max_support_samples),
        });
    }
    let mut tops = Vec::with_capacity(n);
    let mut z = 0.0;
    for h in heights {
        z += h;
        tops.push(z);
    }
    let reach = cfg.support_overhang_angle_deg.to_radians().tan();
    let diagonal = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2)).sqrt();
    let gap = cfg.support_z_gap_layers as usize;
    let mut cells: Vec<BTreeSet<(i64, i64)>> = vec![BTreeSet::new(); n];
    for ix in x0..=x1 {
        for iy in y0..=y1 {
            let pt = [ix as f64 * pitch, iy as f64 * pitch];
            let f: Vec<bool> = outlines.iter().map(|o| inside(pt, o)).collect();
            if !f.iter().any(|b| *b) {
                continue;
            }
            for j in 1..n {
                if !(f[j] && !f[j - 1]) {
                    continue;
                }
                // held by lower material inside the cone of the angle? Material straight below, past some empty layers, is not holding it: that is where a support lands
                let mut held = false;
                for k in (0..j).rev() {
                    let dz = tops[j] - tops[k];
                    if f[k] {
                        break;
                    }
                    if distance_to_loops(pt, &outlines[k]) <= dz * reach {
                        held = true;
                        break;
                    }
                    if dz * reach > diagonal {
                        break;
                    }
                }
                if held || j < 1 + gap {
                    continue;
                }
                for k in (0..=j - 1 - gap).rev() {
                    if f[k] {
                        if cfg.supports == Supports::BuildPlate {
                            return Err(SliceError::UnsupportedOverhang { layer: j });
                        }
                        break;
                    }
                    if distance_to_loops(pt, &outlines[k]) >= cfg.support_xy_gap_mm {
                        cells[k].insert((ix, iy));
                    }
                }
            }
        }
    }
    for (k, set) in cells.iter().enumerate() {
        out[k] = rows(set, k, &outlines[k], pitch, cfg);
    }
    Ok(out)
}

/// The lines of one layer: runs of neighbouring cells along x (even layers) or y (odd ones), cut where the line would touch the part.
fn rows(
    set: &BTreeSet<(i64, i64)>,
    layer: usize,
    outline: &[Loop],
    pitch: f64,
    cfg: &SliceConfig,
) -> Vec<Path> {
    let along_x = layer.is_multiple_of(2);
    // (row, column) and back
    let rc = |c: (i64, i64)| if along_x { (c.1, c.0) } else { (c.0, c.1) };
    let at = |row: i64, col: i64| -> P {
        if along_x {
            [col as f64 * pitch, row as f64 * pitch]
        } else {
            [row as f64 * pitch, col as f64 * pitch]
        }
    };
    let clear =
        |p: P| !inside(p, outline) && distance_to_loops(p, outline) >= cfg.support_xy_gap_mm;
    let mut sorted: Vec<(i64, i64)> = set.iter().map(|c| rc(*c)).collect();
    sorted.sort_unstable();
    let mut paths = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let (row, first) = sorted[i];
        let mut last = first;
        let mut j = i + 1;
        while j < sorted.len() && sorted[j].0 == row && sorted[j].1 == last + 1 {
            let (a, b) = (at(row, last), at(row, last + 1));
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            if !clear(mid) {
                break;
            }
            last += 1;
            j += 1;
        }
        let (a, b) = (at(row, first), at(row, last));
        let (a, b) = if first == last {
            // a lone column: a short line across it
            let d = pitch * 0.4;
            if along_x {
                ([a[0] - d, a[1]], [a[0] + d, a[1]])
            } else {
                ([a[0], a[1] - d], [a[0], a[1] + d])
            }
        } else {
            (a, b)
        };
        if clear(a) && clear(b) {
            paths.push(Path {
                kind: Kind::Support,
                points: vec![a, b],
                closed: false,
                width_mm: cfg.line_width_mm,
            });
        }
        i = j;
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn cfg(mode: Supports) -> SliceConfig {
        SliceConfig {
            supports: mode,
            ..SliceConfig::default()
        }
    }

    /// A stem 4 x 4 for five layers with a plate 30 x 6 over it for five more.
    fn tee() -> Vec<Vec<Loop>> {
        let mut layers = Vec::new();
        for _ in 0..5 {
            layers.push(vec![rect(0.0, 0.0, 4.0, 4.0)]);
        }
        for _ in 0..5 {
            layers.push(vec![rect(0.0, 0.0, 30.0, 6.0)]);
        }
        layers
    }

    #[test]
    fn the_distance_to_a_loop_is_to_its_nearest_edge() {
        let l = vec![rect(0.0, 0.0, 10.0, 10.0)];
        assert!((distance_to_loops([5.0, 5.0], &l) - 5.0).abs() < 1e-12);
        assert!((distance_to_loops([13.0, 14.0], &l) - 5.0).abs() < 1e-12);
        assert!((distance_to_loops([3.0, -2.0], &l) - 2.0).abs() < 1e-12);
        assert!(distance_to_loops([0.0, 0.0], &[]).is_infinite());
    }

    #[test]
    fn nothing_is_made_when_the_supports_are_off_or_nothing_hangs() {
        let layers = tee();
        let heights = vec![0.2; layers.len()];
        let off = support_paths(&layers, &heights, &cfg(Supports::Off)).unwrap();
        assert!(off.iter().all(|l| l.is_empty()));
        let plain: Vec<Vec<Loop>> = (0..8).map(|_| vec![rect(0.0, 0.0, 20.0, 20.0)]).collect();
        let none = support_paths(&plain, &[0.2; 8], &cfg(Supports::Everywhere)).unwrap();
        assert!(none.iter().all(|l| l.is_empty()));
    }

    #[test]
    fn a_plate_over_a_stem_gets_lines_under_it_and_none_beside_the_stem_or_in_the_gap() {
        let layers = tee();
        let heights = vec![0.2; layers.len()];
        let c = SliceConfig {
            support_spacing_mm: 1.5,
            ..cfg(Supports::BuildPlate)
        };
        let s = support_paths(&layers, &heights, &c).unwrap();
        // the plate starts at layer 5; the gap is one layer, so the supports reach layer 3 and the layer 4 (under the plate) has none
        assert!(s[4].is_empty());
        for (k, paths) in s.iter().enumerate().take(4) {
            assert!(!paths.is_empty(), "layer {k}");
        }
        assert!(s[5..].iter().all(|l| l.is_empty()));
        for k in 0..4 {
            for p in &s[k] {
                assert_eq!(p.kind, Kind::Support);
                for q in &p.points {
                    // inside the shadow of the plate and clear of the stem by the gap
                    assert!(
                        q[0] >= 0.0 && q[0] <= 30.0 && q[1] >= 0.0 && q[1] <= 6.0,
                        "{q:?}"
                    );
                    assert!(
                        distance_to_loops(*q, &layers[k]) >= c.support_xy_gap_mm - 1e-9
                            && !inside(*q, &layers[k]),
                        "{q:?}"
                    );
                }
            }
        }
        // the layers turn their lines: the even ones along x and the odd ones along y
        let along = |l: &Vec<Path>| l.iter().all(|p| p.points[0][1] == p.points[1][1]);
        let across = |l: &Vec<Path>| l.iter().all(|p| p.points[0][0] == p.points[1][0]);
        assert!(along(&s[0]) && across(&s[1]) && along(&s[2]));
    }

    #[test]
    fn a_gentle_slope_needs_no_support_and_a_steep_one_does() {
        // each layer reaches out by `step` more than the one below
        let slope = |step: f64| -> Vec<Vec<Loop>> {
            (0..30)
                .map(|i| vec![rect(0.0, 0.0, 6.0 + step * i as f64, 6.0)])
                .collect()
        };
        let heights = vec![0.2; 30];
        let c = SliceConfig {
            support_spacing_mm: 1.0,
            ..cfg(Supports::Everywhere)
        };
        // 0.23 mm out per 0.2 mm up is about 49 degrees: under the 50 of the default
        let gentle = support_paths(&slope(0.23), &heights, &c).unwrap();
        assert!(gentle.iter().all(|l| l.is_empty()));
        // 0.55 mm per 0.2 mm is about 70 degrees (a step that does not fall on the grid of the samples)
        let steep = support_paths(&slope(0.55), &heights, &c).unwrap();
        assert!(steep.iter().any(|l| !l.is_empty()));
        // and a flatter limit makes the gentle one need them
        let strict = SliceConfig {
            support_overhang_angle_deg: 20.0,
            ..c
        };
        let flat = support_paths(&slope(0.23), &heights, &strict).unwrap();
        assert!(flat.iter().any(|l| !l.is_empty()));
    }

    #[test]
    fn an_overhang_that_would_stand_on_the_part_is_refused_when_the_supports_are_for_the_bed_only()
    {
        // a wide base, a pillar on one side, and a plate on top that reaches out over the base
        let mut layers: Vec<Vec<Loop>> = Vec::new();
        for _ in 0..4 {
            layers.push(vec![rect(0.0, 0.0, 30.0, 6.0)]);
        }
        for _ in 0..2 {
            layers.push(vec![rect(0.0, 0.0, 10.0, 6.0)]);
        }
        for _ in 0..4 {
            layers.push(vec![rect(0.0, 0.0, 30.0, 6.0)]);
        }
        let heights = vec![0.2; layers.len()];
        let bed = SliceConfig {
            support_spacing_mm: 1.5,
            ..cfg(Supports::BuildPlate)
        };
        assert!(matches!(
            support_paths(&layers, &heights, &bed),
            Err(SliceError::UnsupportedOverhang { layer: 6 })
        ));
        let anywhere = SliceConfig {
            support_spacing_mm: 1.5,
            support_z_gap_layers: 0,
            ..cfg(Supports::Everywhere)
        };
        let s = support_paths(&layers, &heights, &anywhere).unwrap();
        assert!(!s[4].is_empty() && !s[5].is_empty() && s[3].is_empty());
    }

    #[test]
    fn too_many_samples_are_refused_with_their_number() {
        let layers = tee();
        let heights = vec![0.2; layers.len()];
        let c = SliceConfig {
            max_support_samples: 1000,
            support_spacing_mm: 1.0,
            ..cfg(Supports::Everywhere)
        };
        let big: Vec<Vec<Loop>> = (0..200)
            .map(|_| vec![rect(0.0, 0.0, 100.0, 100.0)])
            .collect();
        assert!(matches!(
            support_paths(&big, &vec![0.2; 200], &c),
            Err(SliceError::TooManySupportSamples { max: 1000, .. })
        ));
        let roomy = SliceConfig {
            max_support_samples: 100_000,
            ..c
        };
        assert!(support_paths(&layers, &heights, &roomy).is_ok());
    }
}
