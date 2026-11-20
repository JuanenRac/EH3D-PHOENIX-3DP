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
const SIDES: usize = 48;

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
    turned(&PROFILE, SIDES)
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
}
