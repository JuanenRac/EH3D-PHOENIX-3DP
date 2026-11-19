//! The triangle mesh: vertices in nanometres, triangles as three indices, and the checks that say whether it can be printed.

use crate::error::CoreError;
use crate::limits::Limits;
use crate::units::Nm;
use std::collections::HashMap;
use std::fmt;

/// A point, in nanometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point3 {
    pub x: Nm,
    pub y: Nm,
    pub z: Nm,
}

impl Point3 {
    pub fn new(x: Nm, y: Nm, z: Nm) -> Point3 {
        Point3 { x, y, z }
    }

    /// A point from three numbers in millimetres; a number that is not finite is an error.
    pub fn from_mm(x: f64, y: f64, z: f64) -> Result<Point3, CoreError> {
        Ok(Point3 {
            x: Nm::from_mm(x)?,
            y: Nm::from_mm(y)?,
            z: Nm::from_mm(z)?,
        })
    }

    /// The three coordinates in millimetres.
    pub fn mm(&self) -> [f64; 3] {
        [self.x.to_mm(), self.y.to_mm(), self.z.to_mm()]
    }
}

/// A triangle mesh.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Point3>,
    pub triangles: Vec<[u32; 3]>,
}

/// One thing a check found.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    Empty,
    BadIndex {
        triangle: usize,
        index: u32,
    },
    TooManyTriangles {
        count: usize,
        max: usize,
    },
    TooManyVertices {
        count: usize,
        max: usize,
    },
    TooLarge {
        axis: char,
        mm: f64,
        max: f64,
    },
    /// Triangles with no area.
    Degenerate(usize),
    /// Edges that are not shared by exactly two triangles: the mesh is not watertight.
    OpenEdges(usize),
    /// The volume is negative: the triangles face inwards.
    Inverted,
}

impl Issue {
    /// An error stops the file; the others are warnings.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Issue::Empty
                | Issue::BadIndex { .. }
                | Issue::TooManyTriangles { .. }
                | Issue::TooManyVertices { .. }
                | Issue::TooLarge { .. }
        )
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::Empty => f.write_str("the mesh has no triangles"),
            Issue::BadIndex { triangle, index } => {
                write!(
                    f,
                    "triangle {triangle} uses vertex {index}, which does not exist"
                )
            }
            Issue::TooManyTriangles { count, max } => {
                write!(f, "{count} triangles, more than the limit of {max}")
            }
            Issue::TooManyVertices { count, max } => {
                write!(f, "{count} vertices, more than the limit of {max}")
            }
            Issue::TooLarge { axis, mm, max } => {
                write!(
                    f,
                    "the mesh is {mm:.1} mm along {axis}, more than the limit of {max} mm"
                )
            }
            Issue::Degenerate(n) => write!(f, "{n} triangles have no area"),
            Issue::OpenEdges(n) => write!(
                f,
                "{n} edges are not shared by two triangles (the mesh is not watertight)"
            ),
            Issue::Inverted => f.write_str("the volume is negative (the triangles face inwards)"),
        }
    }
}

/// The result of `Mesh::validate`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    /// No error (warnings do not count).
    pub fn is_ok(&self) -> bool {
        !self.issues.iter().any(Issue::is_error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.is_error())
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| !i.is_error())
    }
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh::default()
    }

    /// A mesh from triangles given as three corners each, in millimetres: equal corners are welded into one vertex. A corner that is not finite, or more than the limits allow, is an error.
    pub fn from_soup(soup: &[[[f64; 3]; 3]], limits: &Limits) -> Result<Mesh, CoreError> {
        if soup.len() > limits.max_triangles {
            return Err(CoreError::TooMany {
                what: "triangles",
                count: soup.len(),
                max: limits.max_triangles,
            });
        }
        let mut mesh = Mesh::new();
        let mut seen: HashMap<Point3, u32> = HashMap::new();
        for tri in soup {
            let mut idx = [0u32; 3];
            for (k, c) in tri.iter().enumerate() {
                let p = Point3::from_mm(c[0], c[1], c[2])?;
                idx[k] = match seen.get(&p) {
                    Some(&i) => i,
                    None => {
                        if mesh.vertices.len() >= limits.max_vertices {
                            return Err(CoreError::TooMany {
                                what: "vertices",
                                count: mesh.vertices.len() + 1,
                                max: limits.max_vertices,
                            });
                        }
                        let i = mesh.vertices.len() as u32;
                        mesh.vertices.push(p);
                        seen.insert(p, i);
                        i
                    }
                };
            }
            mesh.triangles.push(idx);
        }
        Ok(mesh)
    }

    /// The corners of the box that holds every vertex used (`None` when there are none).
    pub fn bounds(&self) -> Option<(Point3, Point3)> {
        let first = self.vertices.first()?;
        let (mut lo, mut hi) = (*first, *first);
        for v in &self.vertices[1..] {
            lo = Point3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
            hi = Point3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
        }
        Some((lo, hi))
    }

    /// The size of the box along x, y and z, in millimetres.
    pub fn size_mm(&self) -> Option<[f64; 3]> {
        let (lo, hi) = self.bounds()?;
        Some([
            (hi.x - lo.x).to_mm(),
            (hi.y - lo.y).to_mm(),
            (hi.z - lo.z).to_mm(),
        ])
    }

    fn corners(&self, t: &[u32; 3]) -> Option<[[f64; 3]; 3]> {
        Some([
            self.vertices.get(t[0] as usize)?.mm(),
            self.vertices.get(t[1] as usize)?.mm(),
            self.vertices.get(t[2] as usize)?.mm(),
        ])
    }

    /// The volume in cubic millimetres, from the signed volumes of the tetrahedra to the origin (negative when the triangles face inwards).
    pub fn volume_mm3(&self) -> f64 {
        let mut v = 0.0;
        for t in &self.triangles {
            if let Some([a, b, c]) = self.corners(t) {
                v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0;
            }
        }
        v
    }

    /// The area of the surface in square millimetres.
    pub fn surface_area_mm2(&self) -> f64 {
        let mut s = 0.0;
        for t in &self.triangles {
            if let Some(tri) = self.corners(t) {
                s += triangle_area(&tri);
            }
        }
        s
    }

    /// How many triangles have no area (two equal corners, or three in a line).
    pub fn degenerate_count(&self) -> usize {
        self.triangles
            .iter()
            .filter(|t| {
                t[0] == t[1]
                    || t[1] == t[2]
                    || t[0] == t[2]
                    || self.corners(t).is_some_and(|c| triangle_area(&c) < 1e-12)
            })
            .count()
    }

    /// How many edges are not shared by exactly two triangles.
    pub fn open_edges(&self) -> usize {
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &self.triangles {
            let [p, q, r] = *t;
            for (a, b) in [(p, q), (q, r), (r, p)] {
                if a == b {
                    continue;
                }
                let key = if a < b { (a, b) } else { (b, a) };
                *count.entry(key).or_insert(0) += 1;
            }
        }
        count.values().filter(|&&n| n != 2).count()
    }

    /// A copy moved so that the lowest corner of its box is at the origin.
    pub fn at_origin(&self) -> Mesh {
        let Some((lo, _)) = self.bounds() else {
            return self.clone();
        };
        let mut m = self.clone();
        for v in &mut m.vertices {
            v.x = v.x - lo.x;
            v.y = v.y - lo.y;
            v.z = v.z - lo.z;
        }
        m
    }

    /// Checks the mesh against the limits. Errors stop the file; warnings say what a person should look at.
    pub fn validate(&self, limits: &Limits) -> Report {
        let mut issues = Vec::new();
        if self.triangles.is_empty() {
            issues.push(Issue::Empty);
            return Report { issues };
        }
        if self.triangles.len() > limits.max_triangles {
            issues.push(Issue::TooManyTriangles {
                count: self.triangles.len(),
                max: limits.max_triangles,
            });
        }
        if self.vertices.len() > limits.max_vertices {
            issues.push(Issue::TooManyVertices {
                count: self.vertices.len(),
                max: limits.max_vertices,
            });
        }
        for (i, t) in self.triangles.iter().enumerate() {
            if let Some(&bad) = t.iter().find(|&&k| k as usize >= self.vertices.len()) {
                issues.push(Issue::BadIndex {
                    triangle: i,
                    index: bad,
                });
                return Report { issues };
            }
        }
        if let Some(size) = self.size_mm() {
            for (axis, mm) in ['x', 'y', 'z'].into_iter().zip(size) {
                if mm > limits.max_extent_mm {
                    issues.push(Issue::TooLarge {
                        axis,
                        mm,
                        max: limits.max_extent_mm,
                    });
                }
            }
        }
        let deg = self.degenerate_count();
        if deg > 0 {
            issues.push(Issue::Degenerate(deg));
        }
        let open = self.open_edges();
        if open > 0 {
            issues.push(Issue::OpenEdges(open));
        }
        if open == 0 && self.volume_mm3() < 0.0 {
            issues.push(Issue::Inverted);
        }
        Report { issues }
    }

    /// Whether the box of the mesh fits in a build volume (x, y, z in millimetres).
    pub fn fits_in(&self, volume_mm: [f64; 3]) -> bool {
        match self.size_mm() {
            Some(s) => s.iter().zip(volume_mm).all(|(a, b)| *a <= b),
            None => true,
        }
    }
}

fn triangle_area(c: &[[f64; 3]; 3]) -> f64 {
    let u = [c[1][0] - c[0][0], c[1][1] - c[0][1], c[1][2] - c[0][2]];
    let v = [c[2][0] - c[0][0], c[2][1] - c[0][1], c[2][2] - c[0][2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
}

/// A cube of a side in millimetres with its corner at the origin: the model that the tests and the examples use.
pub fn cube(side_mm: f64) -> Mesh {
    let s = side_mm;
    let p = |x: f64, y: f64, z: f64| [x * s, y * s, z * s];
    let quad = |a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]| [[a, b, c], [a, c, d]];
    let faces = [
        quad(p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.), p(1., 0., 0.)),
        quad(p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)),
        quad(p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)),
        quad(p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.), p(1., 0., 1.)),
        quad(p(1., 1., 0.), p(0., 1., 0.), p(0., 1., 1.), p(1., 1., 1.)),
        quad(p(0., 1., 0.), p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.)),
    ];
    let soup: Vec<[[f64; 3]; 3]> = faces.iter().flatten().copied().collect();
    Mesh::from_soup(&soup, &Limits::default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn a_cube_is_a_good_mesh() {
        let c = cube(20.0);
        assert_eq!(c.vertices.len(), 8);
        assert_eq!(c.triangles.len(), 12);
        assert_eq!(c.open_edges(), 0);
        assert!((c.volume_mm3() - 8000.0).abs() < 1e-6);
        assert!((c.surface_area_mm2() - 2400.0).abs() < 1e-6);
        assert!(c.validate(&Limits::default()).issues.is_empty());
        assert_eq!(c.size_mm(), Some([20.0, 20.0, 20.0]));
    }

    #[test]
    fn a_missing_face_is_an_open_mesh() {
        let mut c = cube(10.0);
        c.triangles.pop();
        let r = c.validate(&Limits::default());
        assert!(r.is_ok());
        assert!(r.warnings().any(|i| matches!(i, Issue::OpenEdges(_))));
    }

    #[test]
    fn inverted_faces_are_noticed() {
        let mut c = cube(10.0);
        for t in &mut c.triangles {
            t.swap(1, 2);
        }
        assert!(c.volume_mm3() < 0.0);
        assert!(c
            .validate(&Limits::default())
            .issues
            .contains(&Issue::Inverted));
    }

    #[test]
    fn a_bad_index_is_an_error_and_never_a_panic() {
        let mut c = cube(10.0);
        c.triangles[3] = [0, 1, 99];
        let r = c.validate(&Limits::default());
        assert!(!r.is_ok());
        assert!(r.errors().next().unwrap().to_string().contains("99"));
        assert!(c.volume_mm3().is_finite());
    }

    #[test]
    fn the_limits_are_obeyed() {
        let l = Limits {
            max_triangles: 5,
            ..Limits::default()
        };
        assert!(!cube(5.0).validate(&l).is_ok());
        let l = Limits {
            max_extent_mm: 10.0,
            ..Limits::default()
        };
        assert!(!cube(50.0).validate(&l).is_ok());
        assert!(Mesh::from_soup(
            &[[[0.0; 3]; 3]; 3],
            &Limits {
                max_triangles: 2,
                ..Limits::default()
            }
        )
        .is_err());
        assert!(!Mesh::new().validate(&Limits::default()).is_ok());
    }

    #[test]
    fn degenerate_triangles_are_counted() {
        let soup = [
            [[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]],
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        ];
        let m = Mesh::from_soup(&soup, &Limits::default()).unwrap();
        assert_eq!(m.degenerate_count(), 1);
    }

    #[test]
    fn welding_joins_equal_corners_and_refuses_nan() {
        let soup = [
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            [[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        ];
        let m = Mesh::from_soup(&soup, &Limits::default()).unwrap();
        assert_eq!(m.vertices.len(), 4);
        let bad = [[[f64::NAN, 0., 0.], [1., 0., 0.], [0., 1., 0.]]];
        assert!(Mesh::from_soup(&bad, &Limits::default()).is_err());
    }

    #[test]
    fn at_origin_moves_the_box_and_fits_in_checks_the_volume() {
        let mut c = cube(10.0);
        for v in &mut c.vertices {
            v.x = v.x + Nm(5_000_000);
            v.z = v.z - Nm(3_000_000);
        }
        let o = c.at_origin();
        assert_eq!(o.bounds().unwrap().0, Point3::default());
        assert!(o.fits_in([10.0, 10.0, 10.0]));
        assert!(!o.fits_in([9.0, 10.0, 10.0]));
        assert!(Mesh::new().fits_in([1.0, 1.0, 1.0]));
    }

    #[test]
    fn random_garbage_never_panics_the_checks() {
        let mut r = Rng::new(2024);
        for _ in 0..200 {
            let n = r.below(30) as usize;
            let mut m = Mesh::new();
            for _ in 0..r.below(12) {
                m.vertices.push(Point3::new(
                    Nm(r.next_u64() as i64 >> 20),
                    Nm(r.next_u64() as i64 >> 20),
                    Nm(r.next_u64() as i64 >> 20),
                ));
            }
            for _ in 0..n {
                m.triangles
                    .push([r.below(16) as u32, r.below(16) as u32, r.below(16) as u32]);
            }
            let rep = m.validate(&Limits::default());
            let _ = rep.is_ok();
            let _ = m.volume_mm3();
            let _ = m.open_edges();
            let _ = m.at_origin();
        }
    }
}
