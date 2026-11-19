//! The preview of the model: an orbit camera that turns the edges of the mesh into lines on a plane. It is plain geometry with no window, so it is tested; the 3D viewer
//! with a graphics card (wgpu) is a later step of the roadmap.

use p3dp_core::Mesh;

/// A camera that turns around the middle of the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Turn around the vertical axis, in radians.
    pub yaw: f64,
    /// Tilt, in radians, kept between -89 and +89 degrees.
    pub pitch: f64,
    /// 1 fills the view; larger zooms in (0.05 to 50).
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            yaw: 0.6,
            pitch: 0.5,
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// The camera with every number made sensible: a number that is not finite goes back to its default and the others are held in their range.
    pub fn sanitized(self) -> Camera {
        let d = Camera::default();
        let limit = 89f64.to_radians();
        Camera {
            yaw: if self.yaw.is_finite() {
                self.yaw.rem_euclid(std::f64::consts::TAU)
            } else {
                d.yaw
            },
            pitch: if self.pitch.is_finite() {
                self.pitch.clamp(-limit, limit)
            } else {
                d.pitch
            },
            zoom: if self.zoom.is_finite() {
                self.zoom.clamp(0.05, 50.0)
            } else {
                d.zoom
            },
        }
    }

    /// Turns by an amount (what a drag of the mouse gives).
    pub fn orbit(self, d_yaw: f64, d_pitch: f64) -> Camera {
        Camera {
            yaw: self.yaw + d_yaw,
            pitch: self.pitch + d_pitch,
            zoom: self.zoom,
        }
        .sanitized()
    }

    /// Zooms by a factor.
    pub fn zoomed(self, factor: f64) -> Camera {
        Camera {
            zoom: self.zoom * factor,
            ..self
        }
        .sanitized()
    }
}

/// A line on the plane of the view, in pixels from the top left corner.
pub type Segment = [(f32, f32); 2];

fn rotate(p: [f64; 3], cam: &Camera) -> [f64; 3] {
    // z is up: first turn around z, then tilt around x
    let (sy, cy) = cam.yaw.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    let x = p[0] * cy - p[1] * sy;
    let y = p[0] * sy + p[1] * cy;
    let z = p[2];
    [x, y * cp - z * sp, y * sp + z * cp]
}

/// The edges of a mesh seen by a camera, fitted in a view of `width` by `height` pixels. Each edge is given once, even when two triangles share it. An empty mesh or a view with
/// no size gives no lines.
pub fn edges(mesh: &Mesh, cam: Camera, width: f32, height: f32) -> Vec<Segment> {
    if mesh.triangles.is_empty()
        || !(width.is_finite() && height.is_finite())
        || width < 2.0
        || height < 2.0
    {
        return Vec::new();
    }
    let cam = cam.sanitized();
    let Some((lo, hi)) = mesh.bounds() else {
        return Vec::new();
    };
    let mid = [
        (lo.x.to_mm() + hi.x.to_mm()) / 2.0,
        (lo.y.to_mm() + hi.y.to_mm()) / 2.0,
        (lo.z.to_mm() + hi.z.to_mm()) / 2.0,
    ];
    let pts: Vec<[f64; 3]> = mesh
        .vertices
        .iter()
        .map(|v| {
            let m = v.mm();
            rotate([m[0] - mid[0], m[1] - mid[1], m[2] - mid[2]], &cam)
        })
        .collect();
    let mut radius = 0.0f64;
    for p in &pts {
        radius = radius.max(p[0].abs()).max(p[1].abs());
    }
    if radius <= 0.0 {
        return Vec::new();
    }
    let scale = (f64::from(width.min(height)) / 2.0) * 0.9 / radius * cam.zoom;
    let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let to_screen = |p: &[f64; 3]| ((cx + p[0] * scale) as f32, (cy - p[1] * scale) as f32);
    let mut seen: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in &mesh.triangles {
        let [a, b, c] = *t;
        for (i, j) in [(a, b), (b, c), (c, a)] {
            let key = if i < j { (i, j) } else { (j, i) };
            if i == j || !seen.insert(key) {
                continue;
            }
            if let (Some(p), Some(q)) = (pts.get(i as usize), pts.get(j as usize)) {
                out.push([to_screen(p), to_screen(q)]);
            }
        }
    }
    out
}

/// The outline of the build plate seen by the same camera, as a closed loop of four lines (`x` by `y` millimetres, centred on the model's middle in x and y).
pub fn plate_outline(
    mesh: &Mesh,
    plate_mm: [f64; 2],
    cam: Camera,
    width: f32,
    height: f32,
) -> Vec<Segment> {
    if !(plate_mm[0].is_finite() && plate_mm[1].is_finite())
        || plate_mm[0] <= 0.0
        || plate_mm[1] <= 0.0
        || width < 2.0
        || height < 2.0
    {
        return Vec::new();
    }
    let cam = cam.sanitized();
    let (hx, hy) = (plate_mm[0] / 2.0, plate_mm[1] / 2.0);
    let corners = [
        [-hx, -hy, 0.0],
        [hx, -hy, 0.0],
        [hx, hy, 0.0],
        [-hx, hy, 0.0],
    ];
    let r = rotate_all(&corners, &cam);
    let radius = r
        .iter()
        .fold(0.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs()))
        .max(mesh.size_mm().map_or(0.0, |s| s[0].max(s[1]) / 2.0));
    if radius <= 0.0 {
        return Vec::new();
    }
    let scale = (f64::from(width.min(height)) / 2.0) * 0.9 / radius * cam.zoom;
    let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let s: Vec<(f32, f32)> = r
        .iter()
        .map(|p| ((cx + p[0] * scale) as f32, (cy - p[1] * scale) as f32))
        .collect();
    (0..4).map(|i| [s[i], s[(i + 1) % 4]]).collect()
}

fn rotate_all(points: &[[f64; 3]], cam: &Camera) -> Vec<[f64; 3]> {
    points.iter().map(|p| rotate(*p, cam)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;

    #[test]
    fn a_cube_has_eighteen_edges_and_stays_in_the_view() {
        let e = edges(&cube(20.0), Camera::default(), 400.0, 300.0);
        assert_eq!(
            e.len(),
            18,
            "12 edges of the cube and 6 diagonals of its faces"
        );
        for s in &e {
            for (x, y) in s {
                assert!(
                    (0.0..=400.0).contains(x) && (0.0..=300.0).contains(y),
                    "{x},{y}"
                );
            }
        }
    }

    #[test]
    fn nothing_to_draw_gives_no_lines() {
        assert!(edges(&Mesh::new(), Camera::default(), 400.0, 300.0).is_empty());
        assert!(edges(&cube(5.0), Camera::default(), 0.0, 300.0).is_empty());
        assert!(edges(&cube(5.0), Camera::default(), f32::NAN, 300.0).is_empty());
        assert!(
            plate_outline(&Mesh::new(), [0.0, 100.0], Camera::default(), 100.0, 100.0).is_empty()
        );
    }

    #[test]
    fn a_camera_with_nonsense_is_put_right() {
        let c = Camera {
            yaw: f64::NAN,
            pitch: f64::INFINITY,
            zoom: -3.0,
        }
        .sanitized();
        assert_eq!(
            (c.yaw, c.pitch),
            (Camera::default().yaw, Camera::default().pitch)
        );
        assert!((c.zoom - 0.05).abs() < 1e-12);
        let c = Camera::default().orbit(0.0, 100.0);
        assert!(c.pitch < 89f64.to_radians() + 1e-9);
        assert!(Camera::default().zoomed(1e9).zoom <= 50.0);
        assert!(!edges(
            &cube(5.0),
            Camera {
                yaw: f64::NAN,
                pitch: 0.0,
                zoom: f64::NAN
            },
            200.0,
            200.0
        )
        .is_empty());
    }

    #[test]
    fn zooming_in_spreads_the_lines_out() {
        let spread = |zoom: f64| {
            let e = edges(
                &cube(10.0),
                Camera {
                    zoom,
                    ..Camera::default()
                },
                400.0,
                400.0,
            );
            let xs: Vec<f32> = e.iter().flatten().map(|p| p.0).collect();
            xs.iter().cloned().fold(f32::MIN, f32::max)
                - xs.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(spread(2.0) > spread(1.0) * 1.9);
    }

    #[test]
    fn turning_all_the_way_round_comes_back() {
        let c = Camera::default();
        let a = edges(&cube(10.0), c, 300.0, 300.0);
        let b = edges(
            &cube(10.0),
            c.orbit(std::f64::consts::TAU, 0.0),
            300.0,
            300.0,
        );
        assert_eq!(a.len(), b.len());
        for (s, t) in a.iter().zip(&b) {
            assert!((s[0].0 - t[0].0).abs() < 0.01 && (s[1].1 - t[1].1).abs() < 0.01);
        }
    }

    #[test]
    fn the_plate_is_four_lines_that_close() {
        let p = plate_outline(&cube(10.0), [256.0, 256.0], Camera::default(), 400.0, 300.0);
        assert_eq!(p.len(), 4);
        for (i, side) in p.iter().enumerate() {
            assert_eq!(side[1], p[(i + 1) % 4][0]);
        }
        assert!(
            plate_outline(&cube(10.0), [f64::NAN, 5.0], Camera::default(), 10.0, 10.0).is_empty()
        );
    }

    #[test]
    fn random_cameras_and_meshes_never_panic() {
        let mut r = Rng::new(31);
        for _ in 0..300 {
            let cam = Camera {
                yaw: r.range(-50.0, 50.0),
                pitch: r.range(-5.0, 5.0),
                zoom: r.range(-1.0, 100.0),
            };
            let _ = edges(
                &cube(r.range(0.1, 300.0)),
                cam,
                r.range(0.0, 2000.0) as f32,
                r.range(0.0, 2000.0) as f32,
            );
        }
    }
}
