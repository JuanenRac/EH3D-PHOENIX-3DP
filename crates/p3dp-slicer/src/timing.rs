//! The time a run of moves takes when the head has to speed up and slow down. Each move is a block with a length and a top speed; the speed at the joint of two blocks is limited by
//! the angle between them (a straight joint keeps the speed, a joint that turns all the way back goes to the corner speed) and by how much the head can speed up or slow down over
//! the length of the block, found by a pass back from the end and a pass forward from the start. Each block is then a trapezoid of speed. The result is longer than the plain
//! length over speed, and it is still an estimate: the firmware of the printer has its own planner, its own limits and its own jerk.

/// One move: a length at a top speed, and its direction when it is a move in the plane (a move of z or of the filament has none, so the head stops on both sides of it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    pub length_mm: f64,
    pub top_speed_mm_s: f64,
    pub direction: Option<[f64; 2]>,
}

impl Block {
    /// A move that stops the head at both ends (z, retraction).
    pub fn stop(length_mm: f64, top_speed_mm_s: f64) -> Block {
        Block {
            length_mm,
            top_speed_mm_s,
            direction: None,
        }
    }

    /// A move in the plane from one point to another.
    pub fn planar(from: [f64; 2], to: [f64; 2], top_speed_mm_s: f64) -> Block {
        let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
        let length_mm = (dx * dx + dy * dy).sqrt();
        let direction = (length_mm > 1e-12).then(|| [dx / length_mm, dy / length_mm]);
        Block {
            length_mm,
            top_speed_mm_s,
            direction,
        }
    }
}

/// The time in seconds of a run of blocks that starts and ends at rest.
pub fn seconds(blocks: &[Block], accel_mm_s2: f64, corner_speed_mm_s: f64) -> f64 {
    let a = accel_mm_s2;
    if !(a.is_finite() && a > 0.0) {
        return flat_seconds(blocks);
    }
    let b: Vec<&Block> = blocks
        .iter()
        .filter(|b| b.length_mm > 1e-12 && b.top_speed_mm_s > 0.0 && b.length_mm.is_finite())
        .collect();
    let n = b.len();
    if n == 0 {
        return 0.0;
    }
    // e[i] is the speed at which block i starts; e[n] is the speed at the end of the last one (at rest)
    let mut e = vec![0.0f64; n + 1];
    for i in 1..n {
        let (p, q) = (b[i - 1], b[i]);
        let top = p.top_speed_mm_s.min(q.top_speed_mm_s);
        e[i] = match (p.direction, q.direction) {
            (Some(d1), Some(d2)) => {
                let cos = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0);
                (top * ((1.0 + cos) / 2.0)).max(corner_speed_mm_s.min(top))
            }
            _ => 0.0,
        };
    }
    for i in (0..n).rev() {
        let reach = (e[i + 1] * e[i + 1] + 2.0 * a * b[i].length_mm).sqrt();
        e[i] = e[i].min(reach).min(b[i].top_speed_mm_s);
    }
    for i in 0..n {
        let reach = (e[i] * e[i] + 2.0 * a * b[i].length_mm).sqrt();
        e[i + 1] = e[i + 1].min(reach);
    }
    let mut total = 0.0;
    for i in 0..n {
        let (v0, v1) = (
            e[i].min(b[i].top_speed_mm_s),
            e[i + 1].min(b[i].top_speed_mm_s),
        );
        let (len, top) = (b[i].length_mm, b[i].top_speed_mm_s);
        let peak = ((2.0 * a * len + v0 * v0 + v1 * v1) / 2.0).sqrt();
        total += if peak >= top {
            let up = (top * top - v0 * v0) / (2.0 * a);
            let down = (top * top - v1 * v1) / (2.0 * a);
            (top - v0) / a + (top - v1) / a + (len - up - down).max(0.0) / top
        } else {
            (peak - v0) / a + (peak - v1) / a
        };
    }
    total
}

/// The time if the head stopped at every joint: each block a trapezoid (or a triangle) from rest to rest. It is the most the estimate with the planner can be, as the plain time is the
/// least.
pub fn stop_and_go_seconds(blocks: &[Block], accel_mm_s2: f64) -> f64 {
    blocks
        .iter()
        .map(|b| seconds(std::slice::from_ref(b), accel_mm_s2, 0.0))
        .sum()
}

/// The time with no acceleration at all: every block at its top speed.
pub fn flat_seconds(blocks: &[Block]) -> f64 {
    blocks
        .iter()
        .filter(|b| b.top_speed_mm_s > 0.0 && b.length_mm.is_finite())
        .map(|b| b.length_mm / b.top_speed_mm_s)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f64, x1: f64, v: f64) -> Block {
        Block::planar([x0, 0.0], [x1, 0.0], v)
    }

    #[test]
    fn the_estimate_lies_between_the_plain_time_and_the_stop_and_go_time() {
        let mut rng = p3dp_core::rng::Rng::new(0x71_3E);
        for case in 0..200 {
            let mut at = [0.0f64, 0.0];
            let mut blocks = Vec::new();
            for _ in 0..(1 + rng.below(30)) {
                let to = [rng.range(0.0, 200.0), rng.range(0.0, 200.0)];
                blocks.push(Block::planar(at, to, rng.range(5.0, 200.0)));
                at = to;
            }
            let a = rng.range(100.0, 20_000.0);
            let corner = rng.range(0.5, 20.0);
            let (flat, est, stop) = (
                flat_seconds(&blocks),
                seconds(&blocks, a, corner),
                stop_and_go_seconds(&blocks, a),
            );
            assert!(
                flat <= est + 1e-9 && est <= stop + 1e-9,
                "case {case}: {flat} {est} {stop}"
            );
        }
    }

    #[test]
    fn a_triangle_and_a_trapezoid_are_told_apart_by_the_top_speed() {
        // 10 mm at 1000 mm/s2: the head could reach 100 mm/s at the middle, so a top speed of 200 changes nothing and one of 50 does
        let slow = stop_and_go_seconds(&[line(0.0, 10.0, 50.0)], 1000.0);
        let fast = stop_and_go_seconds(&[line(0.0, 10.0, 200.0)], 1000.0);
        assert!(
            (fast - 2.0 * (10.0f64 / 1000.0).sqrt()).abs() < 1e-9,
            "{fast}"
        );
        // 50 mm/s: 0.05 s up (1.25 mm), 0.05 s down, 7.5 mm at 50 mm/s = 0.15 s
        assert!((slow - 0.25).abs() < 1e-9, "{slow}");
    }

    #[test]
    fn one_move_from_rest_to_rest_is_the_trapezoid_of_the_books() {
        // 100 mm at 100 mm/s with 1000 mm/s2: 0.1 s up (5 mm), 0.1 s down (5 mm), 90 mm at 100 mm/s
        let t = seconds(&[line(0.0, 100.0, 100.0)], 1000.0, 5.0);
        assert!((t - 1.1).abs() < 1e-9, "{t}");
        // too short to reach the top speed: a triangle, 2 * sqrt(L / a)
        let t = seconds(&[line(0.0, 2.0, 100.0)], 1000.0, 5.0);
        assert!((t - 2.0 * (2.0f64 / 1000.0).sqrt()).abs() < 1e-9, "{t}");
    }

    #[test]
    fn a_stronger_acceleration_is_faster_and_a_huge_one_is_the_plain_estimate() {
        let blocks = vec![
            line(0.0, 50.0, 60.0),
            line(50.0, 80.0, 60.0),
            line(80.0, 90.0, 30.0),
        ];
        let slow = seconds(&blocks, 500.0, 5.0);
        let quick = seconds(&blocks, 5000.0, 5.0);
        let flat = flat_seconds(&blocks);
        assert!(slow > quick && quick > flat, "{slow} {quick} {flat}");
        let huge = seconds(&blocks, 1.0e9, 5.0);
        assert!((huge - flat).abs() / flat < 1e-3, "{huge} {flat}");
    }

    #[test]
    fn a_turn_costs_time_and_a_straight_joint_costs_none() {
        let straight = seconds(
            &[line(0.0, 20.0, 80.0), line(20.0, 40.0, 80.0)],
            2000.0,
            5.0,
        );
        let whole = seconds(&[line(0.0, 40.0, 80.0)], 2000.0, 5.0);
        assert!((straight - whole).abs() < 1e-9, "{straight} {whole}");
        let right_angle = seconds(
            &[
                Block::planar([0.0, 0.0], [20.0, 0.0], 80.0),
                Block::planar([20.0, 0.0], [20.0, 20.0], 80.0),
            ],
            2000.0,
            5.0,
        );
        let back = seconds(&[line(0.0, 20.0, 80.0), line(20.0, 0.0, 80.0)], 2000.0, 5.0);
        assert!(
            right_angle > whole && back > right_angle,
            "{whole} {right_angle} {back}"
        );
        // a move that stops the head is a stop on both sides
        let stopped = seconds(
            &[
                line(0.0, 20.0, 80.0),
                Block::stop(1.0, 10.0),
                line(20.0, 40.0, 80.0),
            ],
            2000.0,
            5.0,
        );
        assert!(stopped > straight + 1.0 / 10.0);
    }

    #[test]
    fn nothing_odd_gives_a_time_that_is_not_a_number() {
        assert_eq!(seconds(&[], 1000.0, 5.0), 0.0);
        assert_eq!(seconds(&[Block::stop(0.0, 10.0)], 1000.0, 5.0), 0.0);
        let b = [line(0.0, 10.0, 50.0)];
        assert!(
            (seconds(&b, 0.0, 5.0) - 0.2).abs() < 1e-12,
            "no acceleration is the plain time"
        );
        assert!((seconds(&b, f64::NAN, 5.0) - 0.2).abs() < 1e-12);
        let t = seconds(&b, 3000.0, 1.0e6);
        assert!(t.is_finite() && t > 0.2);
    }
}
