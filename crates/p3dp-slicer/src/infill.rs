//! The fill of the inside of a layer: solid skins where nothing covers a spot on the layers above or below, bridges where a skin hangs over nothing, and the sparse infill in the
//! pattern the settings ask for (rectilinear, grid or gyroid). Every pattern is made of polylines that are cut in short pieces; each piece is judged by its middle, and the pieces of
//! a kind that follow one another are joined again, so a straight line is one path and a wave is one path. The gyroid is the zero set of
//! `sin x cos y + sin y cos z + sin z cos x`, followed on a grid of squares and joined from the segments it leaves in each of them.

use crate::config::{InfillPattern, SliceConfig};
use crate::error::SliceError;
use crate::geom::{self, dist, inside, scan_lines, Loop, P};
use crate::plan::{Kind, Path};
use std::collections::HashMap;

/// The length of the curves of a gyroid cut, per unit of area, times the period: the period of a gyroid for a density is this over the density times the spacing of the lines.
/// It was measured on the cuts the code makes (see the test that measures it).
pub const GYROID_LENGTH_FACTOR: f64 = 2.40;

/// What a layer needs to know to be filled.
pub(crate) struct Fill<'a> {
    pub index: usize,
    pub height: f64,
    /// The height of the top of the layer.
    pub z: f64,
    pub cfg: &'a SliceConfig,
    pub outlines: &'a [Vec<Loop>],
    /// Inside the innermost perimeter.
    pub region: &'a [Loop],
    /// Whether a skin that hangs over nothing is a bridge (no supports hold it).
    pub bridges: bool,
}

/// Whether every one of `count` layers on one side of layer `index` covers the point.
pub(crate) fn covered(p: P, index: usize, count: u32, up: bool, outlines: &[Vec<Loop>]) -> bool {
    (1..=count as usize).all(|j| {
        let k = if up {
            index.checked_add(j)
        } else {
            index.checked_sub(j)
        };
        k.and_then(|k| outlines.get(k))
            .is_some_and(|o| inside(p, o))
    })
}

/// The points of a polyline where the points in a row that are not collinear with their neighbours are kept: a straight line made of many pieces is its two ends.
fn simplified(pts: Vec<P>) -> Vec<P> {
    if pts.len() < 3 {
        return pts;
    }
    let mut out: Vec<P> = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        let a = *out.last().unwrap_or(&pts[0]);
        let (b, c) = (pts[i], pts[i + 1]);
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        let dot = (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1]);
        if cross.abs() > 1e-9 || dot < 0.0 {
            out.push(b);
        }
    }
    out.push(pts[pts.len() - 1]);
    out
}

/// Cuts a polyline in pieces no longer than `piece`, judges each by its middle and gathers the runs of pieces of one kind.
fn split_polyline(
    pts: &[P],
    classify: &dyn Fn(P) -> Option<Kind>,
    piece: f64,
    runs: &mut Vec<(Kind, Vec<P>)>,
) {
    let mut cur: Option<(Kind, Vec<P>)> = None;
    let close = |cur: &mut Option<(Kind, Vec<P>)>, runs: &mut Vec<(Kind, Vec<P>)>| {
        if let Some((k, v)) = cur.take() {
            let v = simplified(v);
            if v.len() >= 2 {
                runs.push((k, v));
            }
        }
    };
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = ((dist(a, b) / piece).ceil() as usize).max(1);
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        for c in 0..n {
            let (t0, t1) = (c as f64 / n as f64, (c + 1) as f64 / n as f64);
            let (p0, p1) = (at(t0), at(t1));
            let mid = at((t0 + t1) / 2.0);
            match classify(mid) {
                Some(k) => match cur.as_mut() {
                    Some((ck, v)) if *ck == k => v.push(p1),
                    _ => {
                        close(&mut cur, runs);
                        cur = Some((k, vec![p0, p1]));
                    }
                },
                None => close(&mut cur, runs),
            }
        }
    }
    close(&mut cur, runs);
}

/// The polylines that the gyroid draws on the layer at height `z` over the box `lo`..`hi`, for a period in millimetres. `Err` carries the number of nodes the grid would need when
/// that is more than `max_nodes`.
pub fn gyroid_polylines(
    lo: P,
    hi: P,
    period: f64,
    z: f64,
    max_nodes: usize,
) -> Result<Vec<Vec<P>>, usize> {
    if !(period.is_finite() && period > 1e-3 && z.is_finite()) {
        return Ok(Vec::new());
    }
    let cell = (period / 10.0).max(0.1);
    let x0 = (lo[0] / cell).floor() * cell;
    let y0 = (lo[1] / cell).floor() * cell;
    let nx = (((hi[0] - x0) / cell).ceil() as usize).max(1);
    let ny = (((hi[1] - y0) / cell).ceil() as usize).max(1);
    let w = nx + 1;
    let nodes = w.saturating_mul(ny + 1);
    if nodes > max_nodes {
        return Err(nodes);
    }
    let k = std::f64::consts::TAU / period;
    let (sz, cz) = ((k * z).sin(), (k * z).cos());
    let f =
        |x: f64, y: f64| (k * x).sin() * (k * y).cos() + (k * y).sin() * cz + sz * (k * x).cos();
    let mut vals = Vec::with_capacity(nodes);
    for j in 0..=ny {
        for i in 0..=nx {
            vals.push(f(x0 + i as f64 * cell, y0 + j as f64 * cell));
        }
    }
    // the edges: a horizontal one (from node (i, j) to (i + 1, j)) is 2 * node, a vertical one (to (i, j + 1)) is 2 * node + 1
    let h = |i: usize, j: usize| ((j * w + i) * 2) as u64;
    let v = |i: usize, j: usize| ((j * w + i) * 2 + 1) as u64;
    let mut segments: Vec<(u64, u64)> = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = [
                vals[j * w + i],
                vals[j * w + i + 1],
                vals[(j + 1) * w + i + 1],
                vals[(j + 1) * w + i],
            ];
            let case = c
                .iter()
                .enumerate()
                .fold(0u8, |m, (b, x)| if *x > 0.0 { m | (1 << b) } else { m });
            let (bottom, right, top, left) = (h(i, j), v(i + 1, j), h(i, j + 1), v(i, j));
            let centre_in = (c[0] + c[1] + c[2] + c[3]) / 4.0 > 0.0;
            match case {
                1 | 14 => segments.push((left, bottom)),
                2 | 13 => segments.push((bottom, right)),
                3 | 12 => segments.push((left, right)),
                4 | 11 => segments.push((right, top)),
                6 | 9 => segments.push((bottom, top)),
                7 | 8 => segments.push((left, top)),
                5 => {
                    if centre_in {
                        segments.push((bottom, right));
                        segments.push((left, top));
                    } else {
                        segments.push((left, bottom));
                        segments.push((right, top));
                    }
                }
                10 => {
                    if centre_in {
                        segments.push((left, bottom));
                        segments.push((right, top));
                    } else {
                        segments.push((bottom, right));
                        segments.push((left, top));
                    }
                }
                _ => {}
            }
        }
    }
    let point = |id: u64| -> P {
        let node = (id / 2) as usize;
        let (i, j) = (node % w, node / w);
        let other = if id.is_multiple_of(2) { node + 1 } else { node + w };
        let (a, b) = (vals[node], vals[other]);
        let t = if (a - b).abs() < 1e-15 {
            0.5
        } else {
            a / (a - b)
        };
        let pa = [x0 + i as f64 * cell, y0 + j as f64 * cell];
        let pb = if id.is_multiple_of(2) {
            [pa[0] + cell, pa[1]]
        } else {
            [pa[0], pa[1] + cell]
        };
        [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t]
    };
    // joining the segments that share an edge
    let mut at_edge: HashMap<u64, Vec<usize>> = HashMap::new();
    for (n, s) in segments.iter().enumerate() {
        at_edge.entry(s.0).or_default().push(n);
        at_edge.entry(s.1).or_default().push(n);
    }
    let mut used = vec![false; segments.len()];
    let mut out: Vec<Vec<P>> = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut chain: std::collections::VecDeque<u64> =
            std::collections::VecDeque::from([segments[first].0, segments[first].1]);
        for front in [false, true] {
            loop {
                let end = if front {
                    chain[0]
                } else {
                    chain[chain.len() - 1]
                };
                let next = at_edge
                    .get(&end)
                    .and_then(|l| l.iter().copied().find(|n| !used[*n]));
                let Some(n) = next else { break };
                used[n] = true;
                let s = segments[n];
                let other = if s.0 == end { s.1 } else { s.0 };
                if front {
                    chain.push_front(other);
                } else {
                    chain.push_back(other);
                }
            }
        }
        out.push(chain.iter().map(|id| point(*id)).collect());
    }
    Ok(out)
}

/// The paths that fill a layer inside its perimeters, ordered so the head goes to the nearest end each time. `head` is where the head is and is left where the last path ends.
pub(crate) fn fill_paths(fill: &Fill, head: &mut P) -> Result<Vec<Path>, SliceError> {
    let cfg = fill.cfg;
    let s = cfg.spacing_mm(fill.height);
    let density = cfg.infill_percent / 100.0;
    let horizontal = fill.index.is_multiple_of(2);
    let min_len = (cfg.line_width_mm * 0.5).max(0.2);
    let all_solid = density >= 0.999;
    let needs_skin = |p: P| {
        (cfg.top_layers > 0 && !covered(p, fill.index, cfg.top_layers, true, fill.outlines))
            || (cfg.bottom_layers > 0
                && !covered(p, fill.index, cfg.bottom_layers, false, fill.outlines))
    };
    let solid_kind = |p: P| {
        let hangs = fill.bridges
            && fill.index > 0
            && !fill
                .outlines
                .get(fill.index - 1)
                .is_some_and(|below| inside(p, below));
        if hangs {
            Kind::Bridge
        } else {
            Kind::SolidInfill
        }
    };
    let piece = (4.0 * s).max(1.0);
    let mut runs: Vec<(Kind, Vec<P>)> = Vec::new();
    // the solid skins: lines that turn on each layer
    let skin_class = |p: P| (all_solid || needs_skin(p)).then(|| solid_kind(p));
    let skin_runs = |horizontal: bool| {
        let mut r: Vec<(Kind, Vec<P>)> = Vec::new();
        for (a, b) in scan_lines(fill.region, horizontal, s, min_len) {
            split_polyline(&[a, b], &skin_class, piece, &mut r);
        }
        r
    };
    let mut skins = skin_runs(horizontal);
    // a bridge is laid across the gap, from one side that holds it to the other: the direction that has more bridge anchored at both ends wins (the turn of the layers is kept when it ties)
    if fill.bridges && fill.index > 0 && skins.iter().any(|(k, _)| *k == Kind::Bridge) {
        let below = &fill.outlines[fill.index - 1];
        let anchored = |runs: &[(Kind, Vec<P>)]| -> f64 {
            runs.iter()
                .filter(|(k, pts)| *k == Kind::Bridge && pts.len() >= 2)
                .filter(|(_, pts)| {
                    let (a, b) = (pts[0], pts[pts.len() - 1]);
                    let len = dist(a, b).max(1e-9);
                    let u = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
                    let probe = 1.5 * s;
                    inside([a[0] - u[0] * probe, a[1] - u[1] * probe], below)
                        && inside([b[0] + u[0] * probe, b[1] + u[1] * probe], below)
                })
                .map(|(_, pts)| pts.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>())
                .sum()
        };
        let other = skin_runs(!horizontal);
        if anchored(&other) > anchored(&skins) + 1e-9 {
            skins = other;
        }
    }
    runs.extend(skins);
    // the sparse infill
    if !all_solid && density > 0.0 {
        let sparse_class = |p: P| (!needs_skin(p)).then_some(Kind::Infill);
        let spacing = s / density;
        match cfg.infill_pattern {
            InfillPattern::Rectilinear => {
                for (a, b) in scan_lines(fill.region, horizontal, spacing, min_len) {
                    split_polyline(&[a, b], &sparse_class, piece, &mut runs);
                }
            }
            InfillPattern::Grid => {
                for dir in [true, false] {
                    for (a, b) in scan_lines(fill.region, dir, spacing * 2.0, min_len) {
                        split_polyline(&[a, b], &sparse_class, piece, &mut runs);
                    }
                }
            }
            InfillPattern::Gyroid => {
                if let Some((lo, hi)) = geom::bounds(fill.region) {
                    let period = GYROID_LENGTH_FACTOR * spacing;
                    let max_nodes = (cfg.max_points_per_layer as usize).saturating_mul(8);
                    let lines =
                        gyroid_polylines(lo, hi, period, fill.z, max_nodes).map_err(|nodes| {
                            SliceError::TooManyPoints {
                                layer: fill.index,
                                points: nodes,
                                max: max_nodes,
                            }
                        })?;
                    let class =
                        |p: P| (inside(p, fill.region) && !needs_skin(p)).then_some(Kind::Infill);
                    for line in lines {
                        split_polyline(&line, &class, piece, &mut runs);
                    }
                }
            }
        }
    }
    if cfg.max_bridge_span_mm > 0.0 {
        for (kind, pts) in &runs {
            if *kind == Kind::Bridge {
                let span: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
                if span > cfg.max_bridge_span_mm + 1e-9 {
                    return Err(SliceError::BridgeTooLong {
                        layer: fill.index,
                        span_mm: span,
                        max_mm: cfg.max_bridge_span_mm,
                    });
                }
            }
        }
    }
    let paths = runs
        .into_iter()
        .map(|(kind, points)| Path {
            kind,
            points,
            closed: false,
            width_mm: cfg.line_width_mm,
        })
        .collect();
    Ok(order_paths(paths, head))
}

/// Puts open paths in the order of a head that always goes to the nearest end that is left, turning a path round when its far end is the nearer one.
pub(crate) fn order_paths(paths: Vec<Path>, head: &mut P) -> Vec<Path> {
    let mut left = paths;
    let mut out = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let mut best = (0usize, false, f64::MAX);
        for (i, p) in left.iter().enumerate() {
            let (Some(a), Some(b)) = (p.points.first(), p.points.last()) else {
                continue;
            };
            let (da, db) = (dist(*head, *a), dist(*head, *b));
            if da < best.2 {
                best = (i, false, da);
            }
            if db < best.2 {
                best = (i, true, db);
            }
        }
        let mut p = left.swap_remove(best.0);
        if best.1 {
            p.points.reverse();
        }
        if let Some(last) = p.points.last() {
            *head = *last;
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn length(lines: &[Vec<P>]) -> f64 {
        lines
            .iter()
            .map(|l| l.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>())
            .sum()
    }

    #[test]
    fn a_straight_line_cut_in_pieces_is_one_path_again() {
        let mut runs = Vec::new();
        split_polyline(
            &[[0.0, 0.0], [20.0, 0.0]],
            &|_| Some(Kind::Infill),
            1.0,
            &mut runs,
        );
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].1, vec![[0.0, 0.0], [20.0, 0.0]]);
        // a spot in the middle that is not wanted cuts it in two
        let mut runs = Vec::new();
        split_polyline(
            &[[0.0, 0.0], [20.0, 0.0]],
            &|p| ((p[0] - 10.0).abs() > 1.0).then_some(Kind::Infill),
            1.0,
            &mut runs,
        );
        assert_eq!(runs.len(), 2);
        // and a change of kind too
        let mut runs = Vec::new();
        split_polyline(
            &[[0.0, 0.0], [20.0, 0.0]],
            &|p| {
                Some(if p[0] < 10.0 {
                    Kind::Infill
                } else {
                    Kind::Bridge
                })
            },
            1.0,
            &mut runs,
        );
        assert_eq!(runs.len(), 2);
        assert_eq!((runs[0].0, runs[1].0), (Kind::Infill, Kind::Bridge));
    }

    #[test]
    fn a_bend_is_kept_and_a_straight_stretch_is_not() {
        let s = simplified(vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [2.0, 2.0],
        ]);
        assert_eq!(s, vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]]);
        // a point where the line goes back on itself is a corner
        let s = simplified(vec![[0.0, 0.0], [2.0, 0.0], [1.0, 0.0]]);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn the_gyroid_is_made_of_curves_inside_its_box_that_do_not_depend_on_the_box() {
        let a = gyroid_polylines([0.0, 0.0], [30.0, 30.0], 6.0, 1.3, 1_000_000).unwrap();
        assert!(!a.is_empty() && a.iter().all(|l| l.len() >= 2));
        for p in a.iter().flatten() {
            assert!(p[0].is_finite() && p[1].is_finite());
            assert!((-0.2..=30.2).contains(&p[0]) && (-0.2..=30.2).contains(&p[1]));
        }
        // the same curve through the same place, whatever the box: a point of the small box is on the curve of the large one
        let small = gyroid_polylines([10.0, 10.0], [14.0, 14.0], 6.0, 1.3, 1_000_000).unwrap();
        let k = std::f64::consts::TAU / 6.0;
        let (sz, cz) = ((k * 1.3f64).sin(), (k * 1.3f64).cos());
        for p in small.iter().flatten() {
            let f =
                (k * p[0]).sin() * (k * p[1]).cos() + (k * p[1]).sin() * cz + sz * (k * p[0]).cos();
            assert!(f.abs() < 0.2, "{f} at {p:?}");
        }
        // another height, another cut
        let b = gyroid_polylines([0.0, 0.0], [30.0, 30.0], 6.0, 2.9, 1_000_000).unwrap();
        assert_ne!(a, b);
        // a grid that would be too big is refused with its size, and a period that is no number gives nothing
        assert!(gyroid_polylines([0.0, 0.0], [300.0, 300.0], 1.0, 0.0, 1000).is_err());
        assert!(
            gyroid_polylines([0.0, 0.0], [30.0, 30.0], f64::NAN, 0.0, 1000)
                .unwrap()
                .is_empty()
        );
        assert!(gyroid_polylines([0.0, 0.0], [30.0, 30.0], 0.0, 0.0, 1000)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_length_of_the_curves_follows_the_period_the_code_works_out() {
        // the length per area times the period is a constant of the gyroid, the one the code uses to turn a density into a period
        let area = 60.0 * 60.0;
        for period in [4.0, 6.0, 9.0] {
            let mut total = 0.0;
            let mut n = 0.0;
            for z in [0.2, 0.9, 1.7, 2.4, 3.1] {
                let lines =
                    gyroid_polylines([0.0, 0.0], [60.0, 60.0], period, z, 10_000_000).unwrap();
                total += length(&lines) / area * period;
                n += 1.0;
            }
            let factor = total / n;
            assert!(
                (factor - GYROID_LENGTH_FACTOR).abs() / GYROID_LENGTH_FACTOR < 0.08,
                "period {period}: {factor}"
            );
        }
    }

    #[test]
    fn the_head_goes_to_the_nearest_end_and_turns_the_path_round_when_it_must() {
        let mk = |a: P, b: P| Path {
            kind: Kind::Infill,
            points: vec![a, b],
            closed: false,
            width_mm: 0.45,
        };
        let mut head = [0.0, 0.0];
        let out = order_paths(
            vec![mk([10.0, 0.0], [5.0, 0.0]), mk([20.0, 0.0], [30.0, 0.0])],
            &mut head,
        );
        assert_eq!(out[0].points, vec![[5.0, 0.0], [10.0, 0.0]]);
        assert_eq!(out[1].points, vec![[20.0, 0.0], [30.0, 0.0]]);
        assert_eq!(head, [30.0, 0.0]);
        let mut head = [0.0, 0.0];
        assert!(order_paths(Vec::new(), &mut head).is_empty());
    }

    #[test]
    fn what_a_layer_covers_above_and_below_decides_the_skin() {
        let big = vec![square(0.0, 0.0, 10.0, 10.0)];
        let small = vec![square(0.0, 0.0, 4.0, 4.0)];
        let layers = vec![big.clone(), big, small];
        assert!(covered([2.0, 2.0], 0, 2, true, &layers));
        assert!(!covered([8.0, 8.0], 0, 2, true, &layers));
        assert!(!covered([2.0, 2.0], 0, 3, true, &layers), "past the top");
        assert!(covered([2.0, 2.0], 2, 2, false, &layers));
        assert!(!covered([2.0, 2.0], 0, 1, false, &layers), "below the bed");
    }
}
