//! The cut of a mesh by a horizontal plane: the loops in which the plane meets the surface, joined from the segments of the triangles it crosses.
//!
//! A vertex on the plane counts as above it, which is the same decision for every triangle that shares the vertex, so the segments of neighbouring triangles meet exactly and a
//! closed mesh gives closed loops. The point where an edge meets the plane is worked out from the edge in one fixed order (lower index first), so the two triangles of an edge
//! find the same point to the last bit.

use crate::error::SliceError;
use crate::geom::{area, Loop, P};
use p3dp_core::Mesh;
use std::collections::HashMap;

/// The mesh in millimetres with a shift, ready to be cut many times.
pub struct Cutter {
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    /// For each triangle, its lowest and highest z.
    z_range: Vec<(f64, f64)>,
}

/// A point of the cut by the name of the edge it lies on (lower vertex, higher vertex): the same on both sides of the edge.
type EdgeKey = (u32, u32);

impl Cutter {
    /// A cutter for a mesh moved by `shift` millimetres.
    pub fn new(mesh: &Mesh, shift: [f64; 3]) -> Cutter {
        let vertices: Vec<[f64; 3]> = mesh
            .vertices
            .iter()
            .map(|v| {
                let m = v.mm();
                [m[0] + shift[0], m[1] + shift[1], m[2] + shift[2]]
            })
            .collect();
        let z_range = mesh
            .triangles
            .iter()
            .map(|t| {
                let z = t.map(|i| vertices.get(i as usize).map_or(0.0, |v| v[2]));
                (z[0].min(z[1]).min(z[2]), z[0].max(z[1]).max(z[2]))
            })
            .collect();
        Cutter {
            vertices,
            triangles: mesh.triangles.clone(),
            z_range,
        }
    }

    fn point_on(&self, a: u32, b: u32, z: f64) -> (EdgeKey, P) {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let (u, v) = (self.vertices[lo as usize], self.vertices[hi as usize]);
        let t = (z - u[2]) / (v[2] - u[2]);
        (
            (lo, hi),
            [u[0] + t * (v[0] - u[0]), u[1] + t * (v[1] - u[1])],
        )
    }

    /// The loops of the cut at height `z`. Outer loops turn counter-clockwise and holes clockwise when the mesh faces outwards. `layer` only names the layer in an error.
    pub fn loops_at(
        &self,
        z: f64,
        layer: usize,
        max_points: usize,
    ) -> Result<Vec<Loop>, SliceError> {
        // the segments, each from one point to the next: (start edge, start point, end edge, end point)
        let mut segments: Vec<(EdgeKey, P, EdgeKey, P)> = Vec::new();
        for (t, &(zlo, zhi)) in self.triangles.iter().zip(&self.z_range) {
            if zlo > z || zhi < z {
                continue;
            }
            let above = t.map(|i| self.vertices[i as usize][2] >= z);
            let count = above.iter().filter(|a| **a).count();
            if count == 0 || count == 3 {
                continue;
            }
            // the vertex that is alone on its side, and the other two in the order of the triangle
            let lone_above = count == 1;
            let k = (0..3).find(|&k| above[k] == lone_above).unwrap_or(0);
            let (l, p, q) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            let (kx, x) = self.point_on(l, p, z);
            let (ky, y) = self.point_on(q, l, z);
            if kx == ky {
                continue;
            }
            if lone_above {
                segments.push((kx, x, ky, y));
            } else {
                segments.push((ky, y, kx, x));
            }
        }
        // join: each segment starts where another ends
        let mut starts: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
        for (i, s) in segments.iter().enumerate() {
            starts.entry(s.0).or_default().push(i);
        }
        let mut used = vec![false; segments.len()];
        let mut loops: Vec<Loop> = Vec::new();
        let mut points = 0usize;
        for first in 0..segments.len() {
            if used[first] {
                continue;
            }
            used[first] = true;
            let origin = segments[first].0;
            let mut pts: Loop = vec![segments[first].1];
            let mut at = segments[first].2;
            let mut end_point = segments[first].3;
            loop {
                if at == origin {
                    break;
                }
                pts.push(end_point);
                let next = starts
                    .get(&at)
                    .and_then(|c| c.iter().copied().find(|&j| !used[j]));
                let Some(j) = next else {
                    return Err(SliceError::OpenLoop { layer });
                };
                used[j] = true;
                at = segments[j].2;
                end_point = segments[j].3;
                if pts.len() > max_points {
                    return Err(SliceError::TooManyPoints {
                        layer,
                        points: pts.len(),
                        max: max_points,
                    });
                }
            }
            points += pts.len();
            if points > max_points {
                return Err(SliceError::TooManyPoints {
                    layer,
                    points,
                    max: max_points,
                });
            }
            if pts.len() >= 3 && area(&pts).abs() > 1e-9 {
                loops.push(pts);
            }
        }
        Ok(loops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::Limits;

    #[test]
    fn a_cube_is_cut_in_one_counter_clockwise_square_of_the_right_area() {
        let c = Cutter::new(&cube(10.0), [0.0; 3]);
        let loops = c.loops_at(5.0, 0, 1000).unwrap();
        assert_eq!(loops.len(), 1);
        assert!(
            (area(&loops[0]) - 100.0).abs() < 1e-9,
            "{}",
            area(&loops[0])
        );
        // the diagonal of each wall adds a point in the middle of it
        assert_eq!(loops[0].len(), 8);
    }

    #[test]
    fn a_plane_through_vertices_or_outside_is_the_same_decision_everywhere() {
        let c = Cutter::new(&cube(10.0), [0.0; 3]);
        // exactly at the bottom face: the vertices count as above, so the cut is the bottom face and it closes
        let at_bottom = c.loops_at(0.0, 0, 1000).unwrap();
        assert!(at_bottom.iter().all(|l| area(l).abs() < 1e-9) || at_bottom.is_empty());
        // exactly at the top face: the vertices are above, nothing is below
        assert!(c.loops_at(10.0, 0, 1000).unwrap().is_empty() || c.loops_at(10.0, 0, 1000).is_ok());
        assert!(c.loops_at(-1.0, 0, 1000).unwrap().is_empty());
        assert!(c.loops_at(11.0, 0, 1000).unwrap().is_empty());
        // a plane just inside the top gives the full square
        let near = c.loops_at(9.999, 0, 1000).unwrap();
        assert!((area(&near[0]) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_mesh_with_a_hole_gives_a_clockwise_loop_for_it() {
        // a square tube: an outer box of 10 and an inner box of 4 with the triangles facing the other way
        let mut soup: Vec<[[f64; 3]; 3]> = Vec::new();
        let add_box = |soup: &mut Vec<[[f64; 3]; 3]>, lo: f64, hi: f64, flip: bool| {
            let m = cube(hi - lo);
            for t in &m.triangles {
                let p = |i: u32| {
                    let v = m.vertices[i as usize].mm();
                    [v[0] + lo, v[1] + lo, v[2]]
                };
                if flip {
                    soup.push([p(t[0]), p(t[2]), p(t[1])]);
                } else {
                    soup.push([p(t[0]), p(t[1]), p(t[2])]);
                }
            }
        };
        add_box(&mut soup, 0.0, 10.0, false);
        add_box(&mut soup, 3.0, 7.0, true);
        let mesh = Mesh::from_soup(&soup, &Limits::default()).unwrap();
        let loops = Cutter::new(&mesh, [0.0; 3]).loops_at(2.0, 0, 1000).unwrap();
        assert_eq!(loops.len(), 2);
        let mut areas: Vec<f64> = loops.iter().map(|l| area(l)).collect();
        areas.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            (areas[0] + 16.0).abs() < 1e-9 && (areas[1] - 100.0).abs() < 1e-9,
            "{areas:?}"
        );
    }

    #[test]
    fn a_shift_moves_the_cut() {
        let c = Cutter::new(&cube(10.0), [5.0, -5.0, 2.0]);
        let loops = c.loops_at(7.0, 0, 1000).unwrap();
        let xs: Vec<f64> = loops[0].iter().map(|p| p[0]).collect();
        assert!(xs.iter().all(|x| [5.0, 10.0, 15.0].contains(x)), "{xs:?}");
        assert!(xs.contains(&5.0) && xs.contains(&15.0));
        assert!(
            c.loops_at(1.0, 0, 1000).unwrap().is_empty(),
            "below the shifted cube"
        );
    }

    #[test]
    fn a_mesh_that_is_not_closed_gives_an_error_and_a_huge_cut_is_stopped() {
        let mut walls = cube(10.0);
        // remove one triangle of a side wall: the cut at mid height cannot close
        let wall = walls
            .triangles
            .iter()
            .position(|t| t.iter().all(|&i| walls.vertices[i as usize].mm()[1] == 0.0))
            .unwrap();
        walls.triangles.remove(wall);
        assert!(matches!(
            Cutter::new(&walls, [0.0; 3]).loops_at(5.0, 3, 1000),
            Err(SliceError::OpenLoop { layer: 3 })
        ));
        assert!(matches!(
            Cutter::new(&cube(10.0), [0.0; 3]).loops_at(5.0, 0, 3),
            Err(SliceError::TooManyPoints { .. })
        ));
    }
}
