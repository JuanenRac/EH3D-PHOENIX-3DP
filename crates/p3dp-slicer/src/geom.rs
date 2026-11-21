//! The plane geometry of a layer, in millimetres: areas, the inside of a set of loops, the offset of a loop inwards (with the loops that the offset crosses itself in cut apart), and
//! the lines of a rectilinear infill clipped to a region. A loop is a closed list of points; the material of a layer lies on the LEFT of its loops (outer loops turn
//! counter-clockwise, holes clockwise).

use std::collections::HashMap;

/// A point: x and y in millimetres.
pub type P = [f64; 2];

/// A closed loop.
pub type Loop = Vec<P>;

/// The signed area of a loop: positive for a counter-clockwise loop.
pub fn area(pts: &[P]) -> f64 {
    let n = pts.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s / 2.0
}

/// The length of a closed loop.
pub fn loop_length(pts: &[P]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| dist(pts[i], pts[(i + 1) % n])).sum()
}

pub fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The box of a set of points: the lowest corner and the highest.
pub fn bounds(loops: &[Loop]) -> Option<(P, P)> {
    let mut it = loops.iter().flatten();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    Some((lo, hi))
}

/// Whether a point is inside the material of a set of loops (a point inside an odd number of loops).
pub fn inside(p: P, loops: &[Loop]) -> bool {
    let mut crossings = 0usize;
    for l in loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            if (a[1] <= p[1]) != (b[1] <= p[1]) {
                let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if x > p[0] {
                    crossings += 1;
                }
            }
        }
    }
    crossings % 2 == 1
}

/// Where two segments cross, as the fraction along each: only a proper crossing (not a touch at an end).
fn cross(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let q = [c[0] - a[0], c[1] - a[1]];
    let t = (q[0] * s[1] - q[1] * s[0]) / den;
    let u = (q[0] * r[1] - q[1] * r[0]) / den;
    let eps = 1e-9;
    (t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps).then_some((t, u))
}

/// The pairs of segments of a closed loop whose boxes share a cell of a grid: the candidates for a crossing, found without comparing every pair.
fn candidate_pairs(pts: &[P]) -> Vec<(usize, usize)> {
    let n = pts.len();
    let Some((lo, hi)) = bounds(&[pts.to_vec()]) else {
        return Vec::new();
    };
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-6);
    let cells = ((n as f64).sqrt().ceil() as usize).clamp(1, 512);
    let size = span / cells as f64;
    let at = |v: f64, o: f64| (((v - o) / size).floor().max(0.0) as usize).min(cells - 1);
    let mut grid: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (x0, x1) = (at(a[0].min(b[0]), lo[0]), at(a[0].max(b[0]), lo[0]));
        let (y0, y1) = (at(a[1].min(b[1]), lo[1]), at(a[1].max(b[1]), lo[1]));
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                grid.entry((cx, cy)).or_default().push(i);
            }
        }
    }
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for cell in grid.values() {
        for (k, &i) in cell.iter().enumerate() {
            for &j in &cell[k + 1..] {
                let (i, j) = if i < j { (i, j) } else { (j, i) };
                // neighbours share an end and cannot cross
                if j != i + 1 && !(i == 0 && j == n - 1) {
                    pairs.push((i, j));
                }
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// Whether a closed loop crosses itself.
pub fn self_crosses(pts: &[P]) -> bool {
    let n = pts.len();
    candidate_pairs(pts)
        .into_iter()
        .any(|(i, j)| cross(pts[i], pts[(i + 1) % n], pts[j], pts[(j + 1) % n]).is_some())
}

/// The loop that lies between the first visit of a crossing (at `from` in the stack) and now: it is taken out, and the crossing stays where the walk goes on from.
fn close_at(
    from: usize,
    stack: &mut Vec<(P, Option<usize>)>,
    seen_at: &mut HashMap<usize, usize>,
) -> Loop {
    let mut piece: Loop = vec![stack[from].0];
    for (p, id) in stack.drain(from + 1..) {
        if let Some(id) = id {
            seen_at.remove(&id);
        }
        piece.push(p);
    }
    piece
}

/// Cuts a loop that crosses itself into the simple loops it is made of, and gives back those that turn the way `want_positive` says (counter-clockwise for the outer loops of
/// material, clockwise for holes). The loops that turn the other way are what an offset leaves behind where it folded over itself: they are not material.
pub fn simple_loops(raw: &[P], want_positive: bool) -> Vec<Loop> {
    let n = raw.len();
    if n < 3 {
        return Vec::new();
    }
    // every crossing becomes a point of both segments, named by one number
    let mut on_segment: Vec<Vec<(f64, usize)>> = vec![Vec::new(); n];
    let mut crossing_points: Vec<P> = Vec::new();
    for (i, j) in candidate_pairs(raw) {
        let (a, b) = (raw[i], raw[(i + 1) % n]);
        let (c, d) = (raw[j], raw[(j + 1) % n]);
        if let Some((t, u)) = cross(a, b, c, d) {
            let id = crossing_points.len();
            crossing_points.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            on_segment[i].push((t, id));
            on_segment[j].push((u, id));
        }
    }
    // the walk along the loop, with the crossings in their places: a crossing met for the second time closes a loop of its own
    let mut stack: Vec<(P, Option<usize>)> = Vec::new();
    let mut seen_at: HashMap<usize, usize> = HashMap::new();
    let mut out: Vec<Loop> = Vec::new();
    for i in 0..n {
        stack.push((raw[i], None));
        let mut here = on_segment[i].clone();
        here.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, id) in here {
            let p = crossing_points[id];
            match seen_at.remove(&id) {
                Some(from) => out.push(close_at(from, &mut stack, &mut seen_at)),
                None => {
                    seen_at.insert(id, stack.len());
                    stack.push((p, Some(id)));
                }
            }
        }
    }
    let rest: Loop = stack.into_iter().map(|(p, _)| p).collect();
    out.push(rest);
    out.into_iter()
        .filter(|l| l.len() >= 3)
        .filter(|l| {
            let a = area(l);
            a.abs() > 1e-9 && (a > 0.0) == want_positive
        })
        .collect()
}

/// The loop without the points that lie on the straight line between their neighbours (the cut of a face that a diagonal splits leaves one) and without repeated points: they
/// change nothing of the shape, and next to a corner they would leave a whisker when the loop is moved in.
pub fn without_collinear(pts: &[P]) -> Loop {
    let mut v: Loop = Vec::with_capacity(pts.len());
    for p in pts {
        if v.last().is_none_or(|q| dist(*q, *p) > 1e-12) {
            v.push(*p);
        }
    }
    while v.len() > 1 && dist(v[0], v[v.len() - 1]) <= 1e-12 {
        v.pop();
    }
    loop {
        let n = v.len();
        if n < 4 {
            return v;
        }
        let drop = (0..n).find(|&i| {
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            let cross = e1[0] * e2[1] - e1[1] * e2[0];
            let dot = e1[0] * e2[0] + e1[1] * e2[1];
            cross.abs() <= 1e-9 * (dist(a, b) * dist(b, c)).max(1e-18) && dot > 0.0
        });
        match drop {
            Some(i) => {
                v.remove(i);
            }
            None => return v,
        }
    }
}

/// A loop moved to its left by `d` (inwards for material), with a miter at every corner, or a bevel where the corner is so sharp that the miter would run away. The loops it
/// folds over itself in are cut apart and the folds dropped, so the answer is zero, one or several simple loops. A loop that is thinner than `2 d` is gone.
pub fn inset(pts: &[P], d: f64) -> Vec<Loop> {
    let cleaned = without_collinear(pts);
    let pts = cleaned.as_slice();
    let n = pts.len();
    if n < 3 || !d.is_finite() {
        return Vec::new();
    }
    let a0 = area(pts);
    if a0.abs() < 1e-12 {
        return Vec::new();
    }
    let unit = |a: P, b: P| {
        let l = dist(a, b);
        if l < 1e-12 {
            [0.0, 0.0]
        } else {
            [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
        }
    };
    let mut raw: Vec<P> = Vec::with_capacity(n + 4);
    for i in 0..n {
        let (p0, p1, p2) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (e1, e2) = (unit(p0, p1), unit(p1, p2));
        if e1 == [0.0, 0.0] || e2 == [0.0, 0.0] {
            continue; // a repeated point
        }
        let (n1, n2) = ([-e1[1], e1[0]], [-e2[1], e2[0]]);
        let dot = n1[0] * n2[0] + n1[1] * n2[1];
        if 1.0 + dot < 0.05 {
            raw.push([p1[0] + n1[0] * d, p1[1] + n1[1] * d]);
            raw.push([p1[0] + n2[0] * d, p1[1] + n2[1] * d]);
        } else {
            let k = d / (1.0 + dot);
            raw.push([p1[0] + (n1[0] + n2[0]) * k, p1[1] + (n1[1] + n2[1]) * k]);
        }
    }
    // an offset past the middle of a shape turns it inside out: the corners cross over and the loop comes out the same way round, a shape that is not there (the corners of a
    // square inset by more than half its side go to the other side of it). A true offset has a point that is at least `d` from the edges it came from and on the side it
    // was offset to (inside the loop for an outer loop inset, outside it for a hole); a loop with no such point is dropped.
    let least = d.abs() * (1.0 - 1e-6) - 1e-9;
    let want_inside = (a0 > 0.0) == (d > 0.0);
    let own = std::slice::from_ref(&cleaned);
    simple_loops(&raw, a0 > 0.0)
        .into_iter()
        .filter(|l| {
            d.abs() < 1e-12
                || l.iter().any(|p| {
                    crate::support::distance_to_loops(*p, own) >= least
                        && inside(*p, own) == want_inside
                })
        })
        .collect()
}

/// The segments in which the lines `y = k * spacing` (or `x = ...` when not `horizontal`) cross the material of a set of loops, longer than `min_len`. The lines lie on a grid
/// anchored at zero, so those of two layers line up.
pub fn scan_lines(loops: &[Loop], horizontal: bool, spacing: f64, min_len: f64) -> Vec<(P, P)> {
    let mut out = Vec::new();
    let Some((lo, hi)) = bounds(loops) else {
        return out;
    };
    if !(spacing.is_finite() && spacing > 1e-6) {
        return out;
    }
    let (c0, c1) = if horizontal {
        (lo[1], hi[1])
    } else {
        (lo[0], hi[0])
    };
    let first = (c0 / spacing).ceil() as i64;
    let last = (c1 / spacing).floor() as i64;
    for k in first..=last {
        let c = k as f64 * spacing;
        let mut xs: Vec<f64> = Vec::new();
        for l in loops {
            let m = l.len();
            for i in 0..m {
                let (a, b) = (l[i], l[(i + 1) % m]);
                // along the line (u) and across it (v)
                let (au, av, bu, bv) = if horizontal {
                    (a[0], a[1], b[0], b[1])
                } else {
                    (a[1], a[0], b[1], b[0])
                };
                if (av <= c) != (bv <= c) {
                    xs.push(au + (c - av) / (bv - av) * (bu - au));
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.as_chunks::<2>().0 {
            if pair[1] - pair[0] >= min_len {
                let (p, q) = if horizontal {
                    ([pair[0], c], [pair[1], c])
                } else {
                    ([c, pair[0]], [c, pair[1]])
                };
                out.push((p, q));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn reversed(l: &Loop) -> Loop {
        l.iter().rev().copied().collect()
    }

    #[test]
    fn area_has_a_sign_and_the_inside_is_even_odd() {
        let s = square(0.0, 0.0, 10.0, 10.0);
        assert!((area(&s) - 100.0).abs() < 1e-12);
        assert!((area(&reversed(&s)) + 100.0).abs() < 1e-12);
        assert!((loop_length(&s) - 40.0).abs() < 1e-12);
        let hole = reversed(&square(4.0, 4.0, 6.0, 6.0));
        let both = vec![s.clone(), hole];
        assert!(
            inside([1.0, 1.0], &both) && !inside([5.0, 5.0], &both) && !inside([11.0, 5.0], &both)
        );
        assert!(!inside([5.0, 5.0], &[]));
        assert_eq!(bounds(&both), Some(([0.0, 0.0], [10.0, 10.0])));
        assert_eq!(bounds(&[]), None);
    }

    #[test]
    fn a_square_moves_in_by_the_distance_and_a_hole_grows() {
        let s = square(0.0, 0.0, 10.0, 10.0);
        let r = inset(&s, 1.0);
        assert_eq!(r.len(), 1);
        assert!((area(&r[0]) - 64.0).abs() < 1e-9, "{}", area(&r[0]));
        let hole = reversed(&square(4.0, 4.0, 6.0, 6.0));
        let h = inset(&hole, 0.5);
        assert_eq!(h.len(), 1);
        assert!((area(&h[0]) + 9.0).abs() < 1e-9, "{}", area(&h[0]));
    }

    #[test]
    fn points_on_a_straight_stretch_do_not_leave_a_whisker_at_the_corner() {
        // a square with a point 0.1 mm from a corner on one side, and a repeated point
        let l: Loop = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [10.0, 10.0],
            [0.1, 10.0],
            [0.0, 10.0],
        ];
        let c = without_collinear(&l);
        assert_eq!(c.len(), 4, "{c:?}");
        let r = inset(&l, 0.225);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].len(), 4, "{:?}", r[0]);
        assert!((area(&r[0]) - 9.55 * 9.55).abs() < 1e-9);
        assert!(without_collinear(&[[0.0, 0.0], [1.0, 0.0]]).len() == 2);
        // a line that goes back on itself is not straight
        let back = without_collinear(&[[0.0, 0.0], [2.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);
        assert_eq!(back.len(), 4);
    }

    #[test]
    fn what_is_thinner_than_twice_the_distance_is_gone() {
        assert!(inset(&square(0.0, 0.0, 10.0, 1.0), 0.6).is_empty());
        assert_eq!(inset(&square(0.0, 0.0, 10.0, 1.0), 0.4).len(), 1);
        assert!(inset(&square(0.0, 0.0, 2.0, 2.0), 1.0).is_empty());
        assert!(inset(&[[0.0, 0.0], [1.0, 1.0]], 0.1).is_empty());
        assert!(inset(&square(0.0, 0.0, 5.0, 5.0), f64::NAN).is_empty());
    }

    #[test]
    fn a_neck_narrower_than_the_offset_cuts_the_shape_in_two() {
        // two 10 x 10 blocks joined by a neck 1 mm wide
        let dumbbell: Loop = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.4, 4.5],
            [13.6, 4.5],
            [14.0, 0.0],
            [24.0, 0.0],
            [24.0, 10.0],
            [14.0, 10.0],
            [13.6, 5.5],
            [10.4, 5.5],
            [10.0, 10.0],
            [0.0, 10.0],
        ];
        assert!(area(&dumbbell) > 0.0);
        assert_eq!(
            inset(&dumbbell, 0.3).len(),
            1,
            "the neck survives a small offset"
        );
        let cut = inset(&dumbbell, 1.0);
        assert_eq!(cut.len(), 2, "{cut:?}");
        for l in &cut {
            assert!(area(l) > 0.0 && !self_crosses(l));
            assert!(area(l) > 55.0 && area(l) < 70.0, "{}", area(l));
        }
    }

    #[test]
    fn a_concave_corner_keeps_its_miter_and_a_loop_that_crosses_itself_is_found() {
        let l_shape: Loop = vec![
            [0.0, 0.0],
            [20.0, 0.0],
            [20.0, 10.0],
            [10.0, 10.0],
            [10.0, 20.0],
            [0.0, 20.0],
        ];
        let r = inset(&l_shape, 1.0);
        assert_eq!(r.len(), 1);
        assert!(r[0].contains(&[9.0, 9.0]), "{:?}", r[0]);
        let bow = vec![[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]];
        assert!(self_crosses(&bow));
        assert!(!self_crosses(&square(0.0, 0.0, 1.0, 1.0)));
        // the two halves of the bow turn opposite ways: one of each
        assert_eq!(simple_loops(&bow, true).len(), 1);
        assert_eq!(simple_loops(&bow, false).len(), 1);
    }

    #[test]
    fn a_long_loop_with_many_points_is_cut_without_comparing_every_pair() {
        // a circle of 20000 points: no crossing, and it must not take a second
        let n = 20_000;
        let circle: Loop = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                [50.0 * a.cos(), 50.0 * a.sin()]
            })
            .collect();
        assert!(!self_crosses(&circle));
        let r = inset(&circle, 1.0);
        assert_eq!(r.len(), 1);
        let expect = std::f64::consts::PI * 49.0 * 49.0;
        assert!((area(&r[0]) - expect).abs() / expect < 1e-3);
    }

    #[test]
    fn scan_lines_stop_at_the_material_and_line_up_between_layers() {
        let s = square(0.0, 0.0, 10.0, 10.0);
        let hole = reversed(&square(4.0, 4.0, 6.0, 6.0));
        let lines = scan_lines(&[s.clone(), hole.clone()], true, 1.0, 0.1);
        // the lines at y = 4 .. 6 are cut in two by the hole
        let at5: Vec<_> = lines.iter().filter(|l| l.0[1] == 5.0).collect();
        assert_eq!(at5.len(), 2);
        assert_eq!(
            (at5[0].0[0], at5[0].1[0], at5[1].0[0], at5[1].1[0]),
            (0.0, 4.0, 6.0, 10.0)
        );
        let inner = square(1.0, 1.0, 9.0, 9.0);
        let vertical = scan_lines(&[inner], false, 2.5, 0.1);
        assert_eq!(vertical.len(), 3);
        assert!(vertical
            .iter()
            .all(|l| l.0[0] == l.1[0] && l.0[1] == 1.0 && l.1[1] == 9.0));
        // anchored at zero, not at the box
        let shifted = scan_lines(&[square(0.3, 0.3, 9.7, 9.7)], true, 1.0, 0.1);
        assert!(shifted
            .iter()
            .all(|l| (l.0[1] - l.0[1].round()).abs() < 1e-12));
        assert!(scan_lines(std::slice::from_ref(&s), true, 0.0, 0.1).is_empty());
        assert!(scan_lines(std::slice::from_ref(&s), true, f64::NAN, 0.1).is_empty());
        assert!(scan_lines(&[], true, 1.0, 0.1).is_empty());
        assert!(
            scan_lines(&[s], true, 1.0, 20.0).is_empty(),
            "short pieces are dropped"
        );
    }
}
