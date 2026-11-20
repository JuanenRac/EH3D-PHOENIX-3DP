//! The solid look of the Prepare tab, without a window: the triangles of the parts on the bed turned into flat shaded polygons on the screen (nearest last, so a painter can lay them
//! one over the other), the bed with its grid, its edge, the box of the printer and the three axes as lines, and the standard views (top, front, side, corner). The window only
//! fills the polygons and strokes the lines it is given. The look is flat shading from one light that follows the camera a little and the sun above; that is all, and it is enough to read
//! a shape.

use p3dp_core::Mesh;
use p3dp_viewer::{OrbitCamera, Viewport};

/// The most polygons one frame is given; a part with more triangles is thinned out after the faces that look away are left out.
pub const MAX_TRIANGLES: usize = 60_000;
/// The most grid lines along one side of the bed.
pub const MAX_GRID_LINES: usize = 400;

/// A polygon on the screen: three points in pixels from the top left of the view, how lit it is from 0 to 1, and the part it belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadedTriangle {
    pub p: [[f32; 2]; 3],
    pub light: f32,
    pub object: u32,
}

/// What a line of the bed is, so the window can give it a colour and a width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Grid,
    GridMajor,
    Edge,
    Volume,
    AxisX,
    AxisY,
    AxisZ,
}

/// A line on the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenLine {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub kind: LineKind,
}

/// The bed seen by the camera.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BedDrawing {
    /// The four corners of the bed, in order round it, when all of them are in front of the camera.
    pub quad: Option<[[f32; 2]; 4]>,
    pub lines: Vec<ScreenLine>,
}

/// The standard views of the 3D view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdView {
    Corner,
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
}

impl StdView {
    pub const ALL: [StdView; 7] = [
        StdView::Corner,
        StdView::Top,
        StdView::Bottom,
        StdView::Front,
        StdView::Back,
        StdView::Left,
        StdView::Right,
    ];

    /// The turn about the vertical axis and the height of the camera, in degrees, for the camera of the viewer (the camera is at `yaw` 0 on the side of +x).
    pub fn angles_deg(self) -> (f64, f64) {
        match self {
            StdView::Corner => (-135.0, 35.0),
            StdView::Top => (-90.0, 89.0),
            StdView::Bottom => (-90.0, -89.0),
            StdView::Front => (-90.0, 0.0),
            StdView::Back => (90.0, 0.0),
            StdView::Left => (180.0, 0.0),
            StdView::Right => (0.0, 0.0),
        }
    }

    /// The name of the view in the arguments of the command line and the keys of the window.
    pub fn key(self) -> &'static str {
        match self {
            StdView::Corner => "corner",
            StdView::Top => "top",
            StdView::Bottom => "bottom",
            StdView::Front => "front",
            StdView::Back => "back",
            StdView::Left => "left",
            StdView::Right => "right",
        }
    }

    /// Puts the camera where this view looks at the whole volume of the printer.
    pub fn apply(self, cam: &mut OrbitCamera, volume_mm: [f64; 3], vp: Viewport) {
        let (yaw, pitch) = self.angles_deg();
        cam.yaw = yaw.to_radians();
        cam.pitch = pitch.to_radians();
        *cam = cam.sanitized();
        cam.fit_to_box(
            [0.0, 0.0, 0.0],
            [volume_mm[0], volume_mm[1], volume_mm[2]],
            vp,
        );
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(a, a).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// The brightness of the darkest face and of the brightest.
pub const LIGHT_MIN: f32 = 0.22;
pub const LIGHT_MAX: f32 = 1.0;

/// The triangles of the parts as polygons on the screen, farthest first. Each part is `(id, mesh)` with its points already on the bed. A triangle behind the camera or entirely
/// outside the view is left out; a triangle seen from behind is drawn (a mesh that is turned inside out is still seen), lit from the side it shows.
pub fn shade(parts: &[(u32, &Mesh)], cam: &OrbitCamera, vp: Viewport) -> Vec<ShadedTriangle> {
    let eye = cam.eye();
    let (forward, _, _) = cam.axes();
    // the light: from the camera, leaning to the sun above
    let to_eye = unit([-forward[0], -forward[1], -forward[2]]).unwrap_or([0.0, -1.0, 0.0]);
    let light_dir =
        unit([0.6 * to_eye[0], 0.6 * to_eye[1], 0.6 * to_eye[2] + 0.4]).unwrap_or([0.0, 0.0, 1.0]);
    let total: usize = parts.iter().map(|(_, m)| m.triangles.len()).sum();
    let thin = total > MAX_TRIANGLES;
    let mut out: Vec<(f64, ShadedTriangle)> = Vec::with_capacity(total.min(MAX_TRIANGLES));
    let mut seen = 0usize;
    for (id, mesh) in parts {
        let pts: Vec<[f64; 3]> = mesh.vertices.iter().map(|v| v.mm()).collect();
        for t in &mesh.triangles {
            let get = |i: u32| pts.get(i as usize).copied();
            let (Some(a), Some(b), Some(c)) = (get(t[0]), get(t[1]), get(t[2])) else {
                continue;
            };
            let Some(mut n) = unit(cross(sub(b, a), sub(c, a))) else {
                continue;
            };
            let centre = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            let facing = dot(n, sub(eye, centre));
            if thin {
                // too many to draw: the faces that look away are the first to go, then every other one
                if facing <= 0.0 {
                    continue;
                }
                seen += 1;
                let keep_every = (total / MAX_TRIANGLES).max(1);
                if seen % keep_every != 0 {
                    continue;
                }
            }
            if facing < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            let (Some(pa), Some(pb), Some(pc)) =
                (cam.project(a, vp), cam.project(b, vp), cam.project(c, vp))
            else {
                continue;
            };
            let inside = |p: &[f64; 2]| {
                p[0] > -50.0 && p[0] < vp.width + 50.0 && p[1] > -50.0 && p[1] < vp.height + 50.0
            };
            if !(inside(&pa) || inside(&pb) || inside(&pc)) {
                continue;
            }
            let lit = dot(n, light_dir).max(0.0) as f32;
            let light = LIGHT_MIN + (LIGHT_MAX - LIGHT_MIN) * lit.clamp(0.0, 1.0);
            let d = sub(centre, eye);
            out.push((
                dot(d, d),
                ShadedTriangle {
                    p: [
                        [pa[0] as f32, pa[1] as f32],
                        [pb[0] as f32, pb[1] as f32],
                        [pc[0] as f32, pc[1] as f32],
                    ],
                    light,
                    object: *id,
                },
            ));
        }
    }
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|(_, t)| t).collect()
}

/// The bed seen by the camera: the grid every `grid_mm`, the edge, the box of the printer up to its height, and the axes from the corner where x and y are zero.
pub fn bed(volume_mm: [f64; 3], grid_mm: f64, cam: &OrbitCamera, vp: Viewport) -> BedDrawing {
    let [bx, by, bz] = volume_mm;
    let mut drawing = BedDrawing::default();
    if !(bx.is_finite() && by.is_finite() && bz.is_finite()) || bx <= 0.0 || by <= 0.0 {
        return drawing;
    }
    let step = if grid_mm.is_finite() {
        grid_mm.clamp(1.0, 500.0)
    } else {
        10.0
    };
    let mut put = |p: [f64; 3], q: [f64; 3], kind: LineKind| {
        if let (Some(a), Some(b)) = (cam.project(p, vp), cam.project(q, vp)) {
            drawing.lines.push(ScreenLine {
                a: [a[0] as f32, a[1] as f32],
                b: [b[0] as f32, b[1] as f32],
                kind,
            });
        }
    };
    let across = |size: f64| ((size / step).floor() as usize).min(MAX_GRID_LINES);
    for i in 1..=across(bx) {
        let x = i as f64 * step;
        if x < bx - 1e-9 {
            let kind = if i % 5 == 0 {
                LineKind::GridMajor
            } else {
                LineKind::Grid
            };
            put([x, 0.0, 0.0], [x, by, 0.0], kind);
        }
    }
    for i in 1..=across(by) {
        let y = i as f64 * step;
        if y < by - 1e-9 {
            let kind = if i % 5 == 0 {
                LineKind::GridMajor
            } else {
                LineKind::Grid
            };
            put([0.0, y, 0.0], [bx, y, 0.0], kind);
        }
    }
    let corners = [[0.0, 0.0], [bx, 0.0], [bx, by], [0.0, by]];
    for i in 0..4 {
        let (p, q) = (corners[i], corners[(i + 1) % 4]);
        put([p[0], p[1], 0.0], [q[0], q[1], 0.0], LineKind::Edge);
        // the box of the printer: the top and the four posts
        put([p[0], p[1], bz], [q[0], q[1], bz], LineKind::Volume);
        put([p[0], p[1], 0.0], [p[0], p[1], bz], LineKind::Volume);
    }
    let reach = (bx.min(by) / 6.0).clamp(10.0, 60.0);
    put([0.0, 0.0, 0.0], [reach, 0.0, 0.0], LineKind::AxisX);
    put([0.0, 0.0, 0.0], [0.0, reach, 0.0], LineKind::AxisY);
    put([0.0, 0.0, 0.0], [0.0, 0.0, reach], LineKind::AxisZ);
    let screen: Vec<Option<[f64; 2]>> = corners
        .iter()
        .map(|c| cam.project([c[0], c[1], 0.0], vp))
        .collect();
    if screen.iter().all(Option::is_some) {
        let at = |i: usize| screen[i].map_or([0.0, 0.0], |p| [p[0] as f32, p[1] as f32]);
        drawing.quad = Some([at(0), at(1), at(2), at(3)]);
    }
    drawing
}

/// The parts as a small picture of `size` by `size` pixels, from the corner, filling the picture: the polygons to paint. Nothing when there is nothing to show.
pub fn thumbnail(parts: &[(u32, &Mesh)], size: f64) -> Vec<ShadedTriangle> {
    let Some(vp) = Viewport::new(size, size) else {
        return Vec::new();
    };
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for (_, m) in parts {
        for v in &m.vertices {
            let p = v.mm();
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    if lo.iter().chain(hi.iter()).any(|v| !v.is_finite()) {
        return Vec::new();
    }
    let mut cam = OrbitCamera::default();
    let (yaw, pitch) = StdView::Corner.angles_deg();
    cam.yaw = yaw.to_radians();
    cam.pitch = pitch.to_radians();
    cam.fit_to_box(lo, hi, vp);
    shade(parts, &cam, vp)
}

/// Where the corner of the origin is told by a small mark in the window: the point of the bed where x and y are zero, on the screen.
pub fn origin_on_screen(cam: &OrbitCamera, vp: Viewport) -> Option<[f32; 2]> {
    cam.project([0.0, 0.0, 0.0], vp)
        .map(|p| [p[0] as f32, p[1] as f32])
}

/// The part under a pixel (the nearest one the ray from the eye meets), by the boxes of the parts on the bed. `None` when the pixel is over the bed or the empty space.
pub fn pick(
    parts: &[(u32, [f64; 3], [f64; 3])],
    cam: &OrbitCamera,
    vp: Viewport,
    px: [f64; 2],
) -> Option<u32> {
    let ray = cam.ray_through(px, vp)?;
    let mut best: Option<(f64, u32)> = None;
    for (id, lo, hi) in parts {
        if let Some(t) = p3dp_viewer::scene::ray_hits_box(&ray, *lo, *hi) {
            if best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, *id));
            }
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::PlacedObject;
    use p3dp_core::mesh::cube;

    fn vp() -> Viewport {
        Viewport::new(800.0, 600.0).unwrap()
    }

    fn cam_for(volume: [f64; 3], view: StdView) -> OrbitCamera {
        let mut c = OrbitCamera::default();
        view.apply(&mut c, volume, vp());
        c
    }

    fn placed(side: f64, at: [f64; 2]) -> Mesh {
        let mut o = PlacedObject::new(1, "c", cube(side));
        o.pos = at;
        o.world_mesh().unwrap()
    }

    #[test]
    fn a_cube_is_twelve_polygons_inside_the_view_and_lit_between_the_limits() {
        let m = placed(40.0, [110.0, 110.0]);
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        let tris = shade(&[(7, &m)], &cam, vp());
        assert_eq!(tris.len(), 12);
        for t in &tris {
            assert_eq!(t.object, 7);
            assert!((LIGHT_MIN..=LIGHT_MAX).contains(&t.light), "{}", t.light);
            for p in t.p {
                assert!(
                    p[0] > 0.0 && p[0] < 800.0 && p[1] > 0.0 && p[1] < 600.0,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn the_polygons_come_farthest_first_and_the_top_is_brighter_from_above() {
        let near = placed(20.0, [30.0, 30.0]);
        let far = placed(20.0, [190.0, 190.0]);
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        let tris = shade(&[(1, &near), (2, &far)], &cam, vp());
        assert_eq!(tris.len(), 24);
        // the corner view looks from -x -y: the part at 30,30 is the nearer one, so its polygons are last
        let last_part = tris.last().unwrap().object;
        let first_part = tris.first().unwrap().object;
        assert_eq!((first_part, last_part), (2, 1));
        // from above, the faces that look up are lit more than from the side
        let top = cam_for([220.0, 220.0, 250.0], StdView::Top);
        let from_top = shade(&[(1, &near)], &top, vp());
        let best = from_top.iter().map(|t| t.light).fold(0.0f32, f32::max);
        assert!(best > 0.85, "{best}");
    }

    #[test]
    fn a_mesh_turned_inside_out_is_still_drawn_and_a_huge_one_is_thinned() {
        let mut m = placed(40.0, [110.0, 110.0]);
        for t in &mut m.triangles {
            t.swap(1, 2);
        }
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        assert_eq!(shade(&[(1, &m)], &cam, vp()).len(), 12);
        // many copies of the triangles of a cube: more than a frame is given
        let mut big = placed(40.0, [110.0, 110.0]);
        let base = big.triangles.clone();
        while big.triangles.len() <= MAX_TRIANGLES * 2 {
            big.triangles.extend(base.iter().copied());
        }
        let n = shade(&[(1, &big)], &cam, vp()).len();
        assert!(n <= MAX_TRIANGLES + 12, "{n}");
        assert!(n > 0);
        assert!(shade(&[], &cam, vp()).is_empty());
        let empty = Mesh::new();
        assert!(shade(&[(1, &empty)], &cam, vp()).is_empty());
    }

    #[test]
    fn a_part_behind_the_camera_or_off_the_view_gives_nothing() {
        let m = placed(10.0, [110.0, 110.0]);
        let mut cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        cam.target = [100_000.0, 100_000.0, 0.0];
        assert!(shade(&[(1, &m)], &cam, vp()).is_empty());
    }

    #[test]
    fn the_bed_has_its_grid_its_edge_its_box_and_three_axes() {
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        let d = bed([220.0, 220.0, 250.0], 10.0, &cam, vp());
        let count = |k: LineKind| d.lines.iter().filter(|l| l.kind == k).count();
        assert_eq!(count(LineKind::Edge), 4);
        assert_eq!(count(LineKind::Volume), 8);
        assert_eq!(
            count(LineKind::AxisX) + count(LineKind::AxisY) + count(LineKind::AxisZ),
            3
        );
        // 21 lines each way inside the edge (10 to 210), every fifth one a major line
        assert_eq!(count(LineKind::Grid) + count(LineKind::GridMajor), 42);
        assert_eq!(count(LineKind::GridMajor), 8);
        assert!(d.quad.is_some());
    }

    #[test]
    fn a_grid_that_is_too_fine_or_not_a_number_is_held_and_a_bad_bed_gives_nothing() {
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        let fine = bed([220.0, 220.0, 250.0], 0.0001, &cam, vp());
        assert!(fine.lines.len() < 2 * MAX_GRID_LINES + 20);
        let nan = bed([220.0, 220.0, 250.0], f64::NAN, &cam, vp());
        assert!(!nan.lines.is_empty());
        for bad in [
            [0.0, 10.0, 10.0],
            [f64::NAN, 10.0, 10.0],
            [10.0, -1.0, 10.0],
        ] {
            let d = bed(bad, 10.0, &cam, vp());
            assert!(d.lines.is_empty() && d.quad.is_none(), "{bad:?}");
        }
    }

    #[test]
    fn every_standard_view_looks_at_the_whole_volume() {
        let volume = [220.0, 220.0, 250.0];
        for v in StdView::ALL {
            let cam = cam_for(volume, v);
            let d = bed(volume, 10.0, &cam, vp());
            assert!(!d.lines.is_empty(), "{}", v.key());
            for l in d.lines.iter().filter(|l| l.kind == LineKind::Edge) {
                for p in [l.a, l.b] {
                    assert!(
                        p[0] > -5.0 && p[0] < 805.0 && p[1] > -5.0 && p[1] < 605.0,
                        "{} {p:?}",
                        v.key()
                    );
                }
            }
        }
        let mut keys: Vec<&str> = StdView::ALL.iter().map(|v| v.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 7);
    }

    #[test]
    fn the_standard_views_look_from_the_side_they_are_named_for() {
        let volume = [220.0, 220.0, 250.0];
        let eye = |v: StdView| cam_for(volume, v).eye();
        let target = cam_for(volume, StdView::Front).target;
        assert!(eye(StdView::Front)[1] < target[1] - 100.0);
        assert!(eye(StdView::Back)[1] > target[1] + 100.0);
        assert!(eye(StdView::Right)[0] > target[0] + 100.0);
        assert!(eye(StdView::Left)[0] < target[0] - 100.0);
        assert!(eye(StdView::Top)[2] > target[2] + 100.0);
        assert!(eye(StdView::Bottom)[2] < target[2] - 100.0);
    }

    #[test]
    fn a_click_picks_the_nearest_part_and_nothing_over_the_empty_bed() {
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Top);
        let parts = vec![
            (1u32, [10.0, 10.0, 0.0], [50.0, 50.0, 20.0]),
            (2u32, [150.0, 150.0, 0.0], [200.0, 200.0, 20.0]),
        ];
        let at = |x: f64, y: f64| {
            let p = cam.project([x, y, 20.0], vp()).unwrap();
            pick(&parts, &cam, vp(), p)
        };
        assert_eq!(at(30.0, 30.0), Some(1));
        assert_eq!(at(175.0, 175.0), Some(2));
        assert_eq!(at(100.0, 100.0), None);
        assert_eq!(pick(&parts, &cam, vp(), [f64::NAN, 1.0]), None);
    }

    #[test]
    fn a_thumbnail_fills_its_square_and_an_empty_one_is_nothing() {
        let a = placed(30.0, [60.0, 60.0]);
        let b = placed(10.0, [150.0, 90.0]);
        let tris = thumbnail(&[(1, &a), (2, &b)], 96.0);
        assert_eq!(tris.len(), 24);
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for t in &tris {
            for p in t.p {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        assert!(
            lo[0] >= 0.0 && lo[1] >= 0.0 && hi[0] <= 96.0 && hi[1] <= 96.0,
            "{lo:?} {hi:?}"
        );
        assert!(
            hi[0] - lo[0] > 40.0 || hi[1] - lo[1] > 40.0,
            "it fills the square"
        );
        assert!(thumbnail(&[], 96.0).is_empty());
        assert!(thumbnail(&[(1, &Mesh::new())], 96.0).is_empty());
        assert!(thumbnail(&[(1, &a)], 0.0).is_empty());
        assert!(thumbnail(&[(1, &a)], f64::NAN).is_empty());
    }

    #[test]
    fn random_cameras_and_parts_never_panic() {
        let mut rng = p3dp_core::rng::Rng::new(11);
        let m = placed(30.0, [110.0, 110.0]);
        for _ in 0..300 {
            let mut c = OrbitCamera::default();
            c.orbit(rng.range(-900.0, 900.0), rng.range(-900.0, 900.0), 0.01);
            c.zoom(rng.range(0.05, 20.0));
            let _ = shade(&[(1, &m)], &c, vp());
            let _ = bed([220.0, 220.0, 250.0], rng.range(0.5, 50.0), &c, vp());
        }
    }
}
