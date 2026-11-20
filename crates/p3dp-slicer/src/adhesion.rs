//! What helps the first layer to hold to the bed: a skirt (loops apart from the part), a brim (loops that touch it) and a raft (layers under it). They are paths like any other, so
//! the preview shows them, the time counts them and the G-code writes them; and a loop that would leave the bed is caught by the same check as every other path.

use crate::config::SliceConfig;
use crate::geom::{area, inset, scan_lines, Loop, P};
use crate::infill::order_paths;
use crate::plan::{Kind, Path};

/// The outer loops of a layer moved away from the material by `d` (a hole is left alone: the loops of a brim do not go into a hole).
pub fn grown(outline: &[Loop], d: f64) -> Vec<Loop> {
    outline
        .iter()
        .filter(|l| area(l) > 0.0)
        .flat_map(|l| inset(l, -d))
        .collect()
}

fn ring_paths(outline: &[Loop], distances: &[f64], width: f64) -> Vec<Path> {
    let mut out = Vec::new();
    for d in distances {
        for l in grown(outline, *d) {
            out.push(Path {
                kind: Kind::Adhesion,
                points: l,
                closed: true,
                width_mm: width,
            });
        }
    }
    out
}

/// The loops of a skirt: `skirt_loops` loops, the first `skirt_distance_mm` from the part.
pub fn skirt_paths(outline: &[Loop], cfg: &SliceConfig, height: f64, head: &mut P) -> Vec<Path> {
    let w = cfg.line_width_mm;
    let s = cfg.spacing_for(w, height);
    let distances: Vec<f64> = (0..cfg.skirt_loops)
        .map(|k| cfg.skirt_distance_mm + w / 2.0 + f64::from(k) * s)
        .collect();
    order_paths(ring_paths(outline, &distances, w), head)
}

/// The loops of a brim: as many touching loops as fit in `brim_width_mm`, the outer ones first.
pub fn brim_paths(outline: &[Loop], cfg: &SliceConfig, height: f64, head: &mut P) -> Vec<Path> {
    let w = cfg.line_width_mm;
    let s = cfg.spacing_for(w, height);
    let count = (cfg.brim_width_mm / s).floor().max(1.0) as u32;
    let distances: Vec<f64> = (0..count)
        .rev()
        .map(|k| w / 2.0 + f64::from(k) * s)
        .collect();
    order_paths(ring_paths(outline, &distances, w), head)
}

/// The lines of layer `k` of a raft of `count` layers: straight lines over the first layer of the part and `raft_margin_mm` beyond it, turning by a right angle on each layer; the
/// lowest layer of a raft of several is spaced out, so it holds to the bed and the next ones lie across it.
pub fn raft_paths(
    outline: &[Loop],
    cfg: &SliceConfig,
    k: usize,
    count: usize,
    height: f64,
    head: &mut P,
) -> Vec<Path> {
    let w = cfg.line_width_mm;
    let s = cfg.spacing_for(w, height);
    let spacing = if k == 0 && count > 1 { 2.0 * s } else { s };
    let footprint = if cfg.raft_margin_mm > 0.0 {
        grown(outline, cfg.raft_margin_mm)
    } else {
        outline.iter().filter(|l| area(l) > 0.0).cloned().collect()
    };
    let paths: Vec<Path> = scan_lines(&footprint, k.is_multiple_of(2), spacing, (w * 0.5).max(0.2))
        .into_iter()
        .map(|(a, b)| Path {
            kind: Kind::Raft,
            points: vec![a, b],
            closed: false,
            width_mm: w,
        })
        .collect();
    order_paths(paths, head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{bounds, loop_length};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn cfg() -> SliceConfig {
        SliceConfig::default()
    }

    #[test]
    fn a_shape_grows_outwards_and_a_hole_is_left_alone() {
        let outer = square(0.0, 0.0, 20.0, 20.0);
        let mut hole = square(8.0, 8.0, 12.0, 12.0);
        hole.reverse();
        let g = grown(&[outer, hole], 2.0);
        assert_eq!(g.len(), 1);
        let (lo, hi) = bounds(&g).unwrap();
        assert!(
            (lo[0] + 2.0).abs() < 1e-9 && (hi[1] - 22.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
    }

    #[test]
    fn a_concave_shape_grows_into_one_simple_loop() {
        let l = vec![
            [0.0, 0.0],
            [20.0, 0.0],
            [20.0, 8.0],
            [8.0, 8.0],
            [8.0, 20.0],
            [0.0, 20.0],
        ];
        let g = grown(std::slice::from_ref(&l), 1.0);
        assert_eq!(g.len(), 1, "{g:?}");
        assert!(area(&g[0]) > area(&l), "{} {}", area(&g[0]), area(&l));
    }

    #[test]
    fn the_skirt_keeps_its_distance_and_has_the_loops_asked() {
        let c = cfg();
        let mut head = [0.0, 0.0];
        let p = skirt_paths(&[square(50.0, 50.0, 70.0, 70.0)], &c, 0.2, &mut head);
        assert_eq!(p.len(), c.skirt_loops as usize);
        let (lo, _) = bounds(&p.iter().map(|x| x.points.clone()).collect::<Vec<_>>()).unwrap();
        // the outer loop is the gap, half a line and the spacing of the second loop from the part
        let outer =
            c.skirt_distance_mm + c.line_width_mm / 2.0 + c.spacing_for(c.line_width_mm, 0.2);
        assert!((50.0 - lo[0] - outer).abs() < 1e-6, "{lo:?} {outer}");
        assert!(p.iter().all(|x| x.closed && x.kind == Kind::Adhesion));
    }

    #[test]
    fn the_brim_touches_the_part_and_stops_at_its_width() {
        let mut c = cfg();
        c.brim_width_mm = 4.0;
        let mut head = [0.0, 0.0];
        let p = brim_paths(&[square(50.0, 50.0, 70.0, 70.0)], &c, 0.2, &mut head);
        assert!(p.len() >= 8, "{}", p.len());
        let all: Vec<Vec<P>> = p.iter().map(|x| x.points.clone()).collect();
        let (lo, hi) = bounds(&all).unwrap();
        assert!(
            lo[0] > 50.0 - 4.0 - 1e-9 && hi[0] < 70.0 + 4.0 + 1e-9,
            "{lo:?} {hi:?}"
        );
        // the loop next to the part is half a line away from it
        let inner = p.last().unwrap();
        let (ilo, _) = bounds(std::slice::from_ref(&inner.points)).unwrap();
        assert!(
            (50.0 - ilo[0] - c.line_width_mm / 2.0).abs() < 1e-6,
            "{ilo:?}"
        );
        // outer loops are longer
        assert!(loop_length(&p[0].points) > loop_length(&inner.points));
    }

    #[test]
    fn the_raft_covers_the_part_and_its_margin_and_turns_on_each_layer() {
        let mut c = cfg();
        c.raft_margin_mm = 3.0;
        let part = [square(50.0, 50.0, 70.0, 70.0)];
        let mut head = [0.0, 0.0];
        let a = raft_paths(&part, &c, 1, 3, 0.2, &mut head);
        let b = raft_paths(&part, &c, 2, 3, 0.2, &mut head);
        let horizontal = |p: &[Path]| {
            p.iter()
                .all(|x| (x.points[0][1] - x.points[1][1]).abs() < 1e-9)
        };
        assert!(horizontal(&b) && !horizontal(&a));
        let all: Vec<Vec<P>> = a.iter().map(|x| x.points.clone()).collect();
        let (lo, hi) = bounds(&all).unwrap();
        assert!(
            lo[0] >= 47.0 - 1e-9 && hi[0] <= 73.0 + 1e-9 && hi[0] > 72.0,
            "{lo:?} {hi:?}"
        );
        // the lowest layer of several is spaced out
        let base = raft_paths(&part, &c, 0, 3, 0.2, &mut head);
        let solid = raft_paths(&part, &c, 0, 1, 0.2, &mut head);
        assert!(
            base.len() * 3 < solid.len() * 2 + 2,
            "{} {}",
            base.len(),
            solid.len()
        );
    }
}
