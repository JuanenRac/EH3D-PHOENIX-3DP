//! The model the window shows when it is asked for a demonstration (`--demo`) and no model is given: a goblet turned from a few radii, one closed solid with no holes, so that it
//! can be sliced and looked at on every tab. It is built here from numbers; it is not a file taken from anywhere.

use p3dp_core::{Limits, Mesh};

/// The name of the demonstration model.
pub const DEMO_NAME: &str = "goblet.stl";

/// The profile of the goblet: the height and the radius at that height, in millimetres, from the foot to the rim.
const PROFILE: [(f64, f64); 13] = [
    (0.0, 14.0),
    (2.0, 15.0),
    (4.0, 12.0),
    (8.0, 5.0),
    (14.0, 3.0),
    (20.0, 3.2),
    (26.0, 6.0),
    (30.0, 12.0),
    (34.0, 17.0),
    (38.0, 20.0),
    (42.0, 22.0),
    (46.0, 22.5),
    (50.0, 22.0),
];

/// How many sides the turned shape has.
const SIDES: usize = 64;

/// How many rings are put between two rows of the profile, so the surface is smooth and the light does not change in steps from one ring to the next.
const RINGS_BETWEEN: usize = 8;

/// The profile with `between` rings added between each two rows, on a curve that goes through every row and never leaves the radius of its neighbours (a monotone cubic: it cannot swell
/// past a row and a radius over zero stays over zero). The heights of the new rings are evenly spaced between the rows. A profile that cannot be smoothed (fewer than two rows, a height that
/// does not rise, a number that is not finite) is given back as it is.
pub fn smooth_profile(profile: &[(f64, f64)], between: usize) -> Vec<(f64, f64)> {
    let n = profile.len();
    if n < 2
        || between == 0
        || profile
            .iter()
            .any(|r| !(r.0.is_finite() && r.1.is_finite()))
        || profile.windows(2).any(|w| w[1].0 <= w[0].0)
    {
        return profile.to_vec();
    }
    // the slope at each row: the harmonic mean of the slopes of the two stretches when they agree in sign, zero when they do not (Fritsch and Carlson)
    let h: Vec<f64> = profile.windows(2).map(|w| w[1].0 - w[0].0).collect();
    let d: Vec<f64> = profile
        .windows(2)
        .zip(&h)
        .map(|(w, h)| (w[1].1 - w[0].1) / h)
        .collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            (w1 + w2) / (w1 / d[i - 1] + w2 / d[i])
        } else {
            0.0
        };
    }
    let mut out = Vec::with_capacity((n - 1) * (between + 1) + 1);
    for i in 0..n - 1 {
        let (x0, y0) = profile[i];
        for k in 0..=between {
            let t = k as f64 / (between + 1) as f64;
            let x = x0 + t * h[i];
            // the cubic of Hermite through the two rows with the slopes above
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * h[i] * m[i]
                + (-2.0 * t3 + 3.0 * t2) * profile[i + 1].1
                + (t3 - t2) * h[i] * m[i + 1];
            out.push((x, y));
        }
    }
    out.push(profile[n - 1]);
    out
}

/// A solid turned round the vertical axis from a profile of `(height, radius)`: a closed mesh, with the base on the plane z = 0 and its middle on the axis x = y = 0. The profile
/// must have two or more rows, rising, with a radius over zero; anything else gives an empty mesh.
pub fn turned(profile: &[(f64, f64)], sides: usize) -> Mesh {
    let rows: Vec<(f64, f64)> = {
        // two equal rows in a row would be a ring of no height
        let mut v: Vec<(f64, f64)> = Vec::new();
        for r in profile {
            if v.last()
                .is_none_or(|l| (l.0 - r.0).abs() > 1e-9 || (l.1 - r.1).abs() > 1e-9)
            {
                v.push(*r);
            }
        }
        v
    };
    if rows.len() < 2
        || sides < 3
        || rows
            .iter()
            .any(|r| !(r.0.is_finite() && r.1.is_finite() && r.1 > 0.0))
        || rows.windows(2).any(|w| w[1].0 <= w[0].0)
    {
        return Mesh::new();
    }
    let at = |row: usize, k: usize| {
        let a = std::f64::consts::TAU * (k % sides) as f64 / sides as f64;
        [rows[row].1 * a.cos(), rows[row].1 * a.sin(), rows[row].0]
    };
    let mut soup: Vec<[[f64; 3]; 3]> = Vec::new();
    for row in 0..rows.len() - 1 {
        for k in 0..sides {
            let (a, b, c, d) = (
                at(row, k),
                at(row, k + 1),
                at(row + 1, k + 1),
                at(row + 1, k),
            );
            soup.push([a, b, c]);
            soup.push([a, c, d]);
        }
    }
    let (foot, rim) = ([0.0, 0.0, rows[0].0], [0.0, 0.0, rows[rows.len() - 1].0]);
    for k in 0..sides {
        soup.push([foot, at(0, k + 1), at(0, k)]);
        soup.push([rim, at(rows.len() - 1, k), at(rows.len() - 1, k + 1)]);
    }
    Mesh::from_soup(&soup, &Limits::default()).unwrap_or_default()
}

/// The goblet of the demonstration.
pub fn goblet() -> Mesh {
    turned(&smooth_profile(&PROFILE, RINGS_BETWEEN), SIDES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_goblet_is_a_closed_solid_that_stands_on_the_bed() {
        let m = goblet();
        assert!(!m.triangles.is_empty());
        assert_eq!(m.open_edges(), 0, "no hole");
        assert_eq!(m.degenerate_count(), 0);
        assert!(m.volume_mm3() > 1000.0, "outwards: {}", m.volume_mm3());
        let s = m.size_mm().unwrap();
        assert!(
            (s[2] - 50.0).abs() < 1e-3 && s[0] > 40.0 && s[0] < 46.0,
            "{s:?}"
        );
        assert!(m.validate(&Limits::default()).is_ok());
        let (lo, _) = m.bounds().unwrap();
        assert!(lo.z.to_mm().abs() < 1e-9);
    }

    #[test]
    fn a_bad_profile_gives_an_empty_mesh() {
        assert!(turned(&[], 12).triangles.is_empty());
        assert!(turned(&[(0.0, 5.0)], 12).triangles.is_empty());
        assert!(turned(&[(0.0, 5.0), (10.0, 5.0)], 2).triangles.is_empty());
        assert!(turned(&[(0.0, 5.0), (0.0, 6.0)], 12).triangles.is_empty());
        assert!(turned(&[(0.0, 5.0), (10.0, -1.0)], 12).triangles.is_empty());
        assert!(turned(&[(0.0, 5.0), (f64::NAN, 5.0)], 12)
            .triangles
            .is_empty());
        // a cylinder is a closed solid with the volume pi r^2 h
        let c = turned(&[(0.0, 10.0), (20.0, 10.0)], 96);
        assert_eq!(c.open_edges(), 0);
        let want = std::f64::consts::PI * 100.0 * 20.0;
        assert!(
            (c.volume_mm3() - want).abs() / want < 0.01,
            "{}",
            c.volume_mm3()
        );
    }

    #[test]
    fn the_goblet_can_be_sliced_on_a_printer_of_the_catalog() {
        let cat = p3dp_profiles::Catalog::builtin().unwrap();
        let p = cat.find("prusa-mk4s").unwrap();
        let mut s = crate::preview::PreviewState::default();
        let mut o = crate::objects::PlacedObject::new(1, DEMO_NAME, goblet());
        o.pos = [125.0, 105.0];
        let mesh = o.world_mesh().unwrap();
        s.config.placement = p3dp_slicer::Placement::Origin;
        assert!(s.slice(&mesh, p, None), "{:?}", s.error);
        assert!(s.layer_count() > 100);
    }

    #[test]
    fn a_smoothed_profile_goes_through_every_row_never_swells_past_its_neighbours_and_stays_above_zero(
    ) {
        let smooth = smooth_profile(&PROFILE, RINGS_BETWEEN);
        assert_eq!(smooth.len(), (PROFILE.len() - 1) * (RINGS_BETWEEN + 1) + 1);
        // the rows are all there, at their places
        for (i, row) in PROFILE.iter().enumerate() {
            assert_eq!(smooth[i * (RINGS_BETWEEN + 1)], *row);
        }
        assert!(
            smooth.windows(2).all(|w| w[1].0 > w[0].0),
            "the heights rise"
        );
        // between two rows the radius stays between theirs (monotone), and over zero
        for (i, w) in PROFILE.windows(2).enumerate() {
            let (lo, hi) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
            for r in &smooth[i * (RINGS_BETWEEN + 1)..=(i + 1) * (RINGS_BETWEEN + 1)] {
                // a row that is a turning point (a neck, a belly) is allowed to be passed by the curve only inside its own two neighbours
                assert!(
                    r.1 >= lo - 1e-9 && r.1 <= hi + 1e-9,
                    "{r:?} outside {lo} {hi}"
                );
                assert!(r.1 > 0.0);
            }
        }
        // the light changes smoothly: the step of the radius between rings is smaller than between the rows it comes from
        let biggest_row_step = PROFILE
            .windows(2)
            .map(|w| (w[1].1 - w[0].1).abs())
            .fold(0.0, f64::max);
        let biggest_ring_step = smooth
            .windows(2)
            .map(|w| (w[1].1 - w[0].1).abs())
            .fold(0.0, f64::max);
        assert!(
            biggest_ring_step < biggest_row_step / 2.0,
            "{biggest_ring_step} {biggest_row_step}"
        );
        // what cannot be smoothed is given back as it is
        assert_eq!(smooth_profile(&[], 4), Vec::<(f64, f64)>::new());
        assert_eq!(smooth_profile(&[(0.0, 5.0)], 4), vec![(0.0, 5.0)]);
        assert_eq!(smooth_profile(&PROFILE, 0), PROFILE.to_vec());
        let flat = [(0.0, 5.0), (0.0, 6.0)];
        assert_eq!(smooth_profile(&flat, 3), flat.to_vec());
        let nan = [(0.0, 5.0), (f64::NAN, 6.0)];
        assert_eq!(smooth_profile(&nan, 3).len(), 2);
        // two rows only: a straight stretch stays straight
        let line = smooth_profile(&[(0.0, 4.0), (10.0, 8.0)], 4);
        for r in &line {
            assert!((r.1 - (4.0 + 0.4 * r.0)).abs() < 1e-9, "{r:?}");
        }
    }

    #[test]
    fn the_smooth_goblet_is_still_one_closed_solid_and_its_rings_are_many_enough_for_the_light() {
        let m = goblet();
        assert_eq!(m.open_edges(), 0);
        assert_eq!(m.degenerate_count(), 0);
        let s = m.size_mm().unwrap();
        assert!((s[2] - 50.0).abs() < 1e-3);
        // a ring every 0.5 mm or so on the bowl, where it curves most
        assert!(
            m.triangles.len() > PROFILE.len() * 2 * SIDES * RINGS_BETWEEN / 2,
            "{}",
            m.triangles.len()
        );
    }
}
