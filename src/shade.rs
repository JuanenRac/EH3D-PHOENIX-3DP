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
    /// How lit the polygon is as a whole (the mean of its corners).
    pub light: f32,
    /// How lit each corner is: on a smooth surface the corners differ, and the polygon is filled blending them, so the facets of a curved part do not show.
    pub corners: [f32; 3],
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
    /// The corners of the bed, in order round it (four, or up to five when the camera is so near that the bed is cut at the plane in front of it); `None` when the bed is all behind
    /// the camera.
    pub quad: Option<Vec<[f32; 2]>>,
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
        self.apply_to_box(cam, [0.0, 0.0, 0.0], volume_mm, vp);
    }

    /// Puts the camera where this view looks at a box `(low, high)` of the world (the whole printer, or the part and what is round it).
    pub fn apply_to_box(self, cam: &mut OrbitCamera, lo: [f64; 3], hi: [f64; 3], vp: Viewport) {
        let (yaw, pitch) = self.angles_deg();
        cam.yaw = yaw.to_radians();
        cam.pitch = pitch.to_radians();
        *cam = cam.sanitized();
        cam.fit_to_box(lo, hi, vp);
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

/// Two faces that meet at a corner are one smooth surface when the angle between them is under this (cosine of 50 degrees); over it the corner is an edge and each face keeps its
/// own direction, so a cube stays sharp and a goblet is round.
pub const CREASE_COS: f64 = 0.6428;
/// The most triangles that meet at one point and still count as a surface; a real mesh has a dozen or so.
pub const MAX_AROUND: usize = 64;

/// The direction each corner of each triangle is lit from: the mean of the directions of the faces that meet at the corner and bend away from the face by less than the crease angle
/// (weighted by their area), and the face's own direction when the corner is an edge. A triangle with no area gets the zero vector, and a corner with an index that is not in `pts`
/// too. The answer has one entry for each triangle, in the order of `tris`.
pub fn smooth_normals(pts: &[[f64; 3]], tris: &[[u32; 3]]) -> Vec<[[f64; 3]; 3]> {
    // the face directions, not unit: the length is twice the area, which is the weight
    let faces: Vec<[f64; 3]> = tris
        .iter()
        .map(|t| {
            let get = |i: u32| pts.get(i as usize).copied();
            match (get(t[0]), get(t[1]), get(t[2])) {
                (Some(a), Some(b), Some(c)) => cross(sub(b, a), sub(c, a)),
                _ => [0.0; 3],
            }
        })
        .collect();
    // the triangles that touch each point, as one list cut in pieces (no list for each point)
    let mut start = vec![0usize; pts.len() + 1];
    for t in tris {
        for i in t {
            if let Some(s) = start.get_mut(*i as usize + 1) {
                *s += 1;
            }
        }
    }
    let mut running = 0usize;
    for s in start.iter_mut() {
        running += *s;
        *s = running;
    }
    let mut next = start.clone();
    let mut touching = vec![0u32; start[pts.len()]];
    for (ti, t) in tris.iter().enumerate() {
        for i in t {
            if let Some(slot) = next.get_mut(*i as usize) {
                touching[*slot] = ti as u32;
                *slot += 1;
            }
        }
    }
    tris.iter()
        .enumerate()
        .map(|(ti, t)| {
            let own = unit(faces[ti]);
            let mut out = [[0.0; 3]; 3];
            for (k, corner) in t.iter().enumerate() {
                let Some(own) = own else { continue };
                let i = *corner as usize;
                if i >= pts.len() {
                    continue;
                }
                let around = &touching[start[i]..start[i + 1]];
                // a point shared by a crowd of triangles is not a surface (a mesh made of copies): it keeps the direction of its face, and the work stays small
                if around.len() > MAX_AROUND {
                    out[k] = own;
                    continue;
                }
                let mut sum = faces[ti];
                for other in around {
                    let o = *other as usize;
                    if o == ti {
                        continue;
                    }
                    if let Some(u) = unit(faces[o]) {
                        if dot(u, own) >= CREASE_COS {
                            sum = [
                                sum[0] + faces[o][0],
                                sum[1] + faces[o][1],
                                sum[2] + faces[o][2],
                            ];
                        }
                    }
                }
                out[k] = unit(sum).unwrap_or(own);
            }
            out
        })
        .collect()
}

/// The box the 3D view is fitted to when the parts on the bed are known: the whole printer when there is no part or the part is as big as the bed, and otherwise a box around the part
/// (about two and a half times its size, never under a third of the bed), so a small part is not a speck on a large bed. `parts` is the box that holds every part, `(low, high)`.
/// A box that is not made of real numbers is no part. The answer is `(low, high)`.
pub fn focus_box(volume_mm: [f64; 3], parts: Option<([f64; 3], [f64; 3])>) -> ([f64; 3], [f64; 3]) {
    let whole = ([0.0; 3], volume_mm);
    let Some((lo, hi)) = parts else {
        return whole;
    };
    if !lo.iter().chain(hi.iter()).all(|v| v.is_finite())
        || !volume_mm.iter().all(|v| v.is_finite() && *v > 0.0)
    {
        return whole;
    }
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    if size.iter().any(|s| *s < 0.0) {
        return whole;
    }
    let span = size[0].max(size[1]).max(size[2]);
    let bed = volume_mm[0].max(volume_mm[1]);
    if span >= bed * 0.45 {
        return whole;
    }
    let side = (span * 2.5).clamp(bed * 0.33, bed);
    let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let top = hi[2].max(side * 0.5);
    (
        [cx - side / 2.0, cy - side / 2.0, 0.0],
        [cx + side / 2.0, cy + side / 2.0, top],
    )
}

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
    let light_of = |n: [f64; 3]| {
        let lit = dot(n, light_dir).max(0.0) as f32;
        LIGHT_MIN + (LIGHT_MAX - LIGHT_MIN) * lit.clamp(0.0, 1.0)
    };
    let thin = total > MAX_TRIANGLES;
    let mut out: Vec<(f64, ShadedTriangle)> = Vec::with_capacity(total.min(MAX_TRIANGLES));
    let mut seen = 0usize;
    for (id, mesh) in parts {
        let pts: Vec<[f64; 3]> = mesh.vertices.iter().map(|v| v.mm()).collect();
        let corner_normals = smooth_normals(&pts, &mesh.triangles);
        for (ti, t) in mesh.triangles.iter().enumerate() {
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
                if !seen.is_multiple_of(keep_every) {
                    continue;
                }
            }
            let mut corner_n = corner_normals[ti];
            if facing < 0.0 {
                n = [-n[0], -n[1], -n[2]];
                for c in &mut corner_n {
                    *c = [-c[0], -c[1], -c[2]];
                }
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
            // a corner with no direction of its own (a bad index) is lit like the face
            let corners = corner_n.map(|c| light_of(if dot(c, c) > 0.5 { c } else { n }));
            let light = corners.iter().sum::<f32>() / 3.0;
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
                    corners,
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
    let near = near_distance(cam);
    let mut put = |p: [f64; 3], q: [f64; 3], kind: LineKind| {
        // a line of the bed that runs behind the camera is drawn as far as the plane in front of it, so a view that is near the bed still has its grid
        let Some((p, q)) = clip_segment_near(cam, near, p, q) else {
            return;
        };
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
    let flat: Vec<[f64; 3]> = corners.iter().map(|c| [c[0], c[1], 0.0]).collect();
    let screen: Vec<[f32; 2]> = clip_polygon_near(cam, near, &flat)
        .iter()
        .filter_map(|p| cam.project(*p, vp))
        .map(|p| [p[0] as f32, p[1] as f32])
        .collect();
    if screen.len() >= 3 {
        drawing.quad = Some(screen);
    }
    drawing
}

/// How far in front of the camera the plane is where what is drawn is cut: a twentieth of the distance to the target, and never under a millimetre.
fn near_distance(cam: &OrbitCamera) -> f64 {
    (cam.distance * 0.05).max(1.0)
}

/// How far in front of the camera a point is, along the direction it looks.
fn depth_of(cam: &OrbitCamera, p: [f64; 3]) -> f64 {
    let (forward, _, _) = cam.axes();
    dot(sub(p, cam.eye()), forward)
}

/// A segment of the world cut at the plane `near` in front of the camera: whole when both ends are in front of it, shortened to the plane when one is behind, and `None` when both
/// are.
pub fn clip_segment_near(
    cam: &OrbitCamera,
    near: f64,
    a: [f64; 3],
    b: [f64; 3],
) -> Option<([f64; 3], [f64; 3])> {
    let (da, db) = (depth_of(cam, a), depth_of(cam, b));
    if !(da.is_finite() && db.is_finite()) {
        return None;
    }
    if da >= near && db >= near {
        return Some((a, b));
    }
    if da < near && db < near {
        return None;
    }
    let t = (near - da) / (db - da);
    let cut = [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ];
    Some(if da < near { (cut, b) } else { (a, cut) })
}

/// A polygon of the world cut at the plane `near` in front of the camera (the part that is in front of it, with the points where it crosses the plane).
pub fn clip_polygon_near(cam: &OrbitCamera, near: f64, points: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity(points.len() + 1);
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        let (da, db) = (depth_of(cam, *a), depth_of(cam, b));
        if da >= near {
            out.push(*a);
        }
        if (da >= near) != (db >= near) && da.is_finite() && db.is_finite() {
            let t = (near - da) / (db - da);
            out.push([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]);
        }
    }
    out
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

/// The twelve edges of a box seen by the camera, as pairs of points in pixels (an edge with an end behind the camera is left out). It outlines the part that is selected.
pub fn box_lines(
    lo: [f64; 3],
    hi: [f64; 3],
    cam: &OrbitCamera,
    vp: Viewport,
) -> Vec<([f32; 2], [f32; 2])> {
    let corner = |i: usize| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    let mut out = Vec::new();
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                let j = i | bit;
                if let (Some(a), Some(b)) = (cam.project(corner(i), vp), cam.project(corner(j), vp))
                {
                    out.push(([a[0] as f32, a[1] as f32], [b[0] as f32, b[1] as f32]));
                }
            }
        }
    }
    out
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
    fn a_view_near_the_bed_keeps_its_grid_its_edge_and_its_surface() {
        // the camera is put round a part of 45 mm in the middle of a bed of 256: the far corners of the bed are behind it
        let volume = [256.0, 256.0, 256.0];
        let (lo, hi) = focus_box(volume, Some(([105.5, 105.5, 0.0], [150.5, 150.5, 50.0])));
        let mut cam = OrbitCamera::default();
        StdView::Corner.apply_to_box(&mut cam, lo, hi, vp());
        let d = bed(volume, 10.0, &cam, vp());
        let count = |k: LineKind| d.lines.iter().filter(|l| l.kind == k).count();
        assert!(
            count(LineKind::Grid) + count(LineKind::GridMajor) >= 30,
            "grid"
        );
        assert!(count(LineKind::Edge) >= 2, "edge");
        let q = d.quad.expect("the surface of the bed");
        assert!((3..=5).contains(&q.len()), "{}", q.len());
        // and when the whole bed is in view the surface is its four corners
        let whole = cam_for(volume, StdView::Corner);
        assert_eq!(bed(volume, 10.0, &whole, vp()).quad.unwrap().len(), 4);
    }

    #[test]
    fn a_segment_and_a_polygon_are_cut_at_the_plane_in_front_of_the_camera() {
        let cam = OrbitCamera {
            target: [0.0, 0.0, 0.0],
            distance: 100.0,
            ..OrbitCamera::default()
        };
        let near = near_distance(&cam);
        assert_eq!(near, 5.0);
        let eye = cam.eye();
        let (forward, _, _) = cam.axes();
        let at = |t: f64| {
            [
                eye[0] + forward[0] * t,
                eye[1] + forward[1] * t,
                eye[2] + forward[2] * t,
            ]
        };
        // in front: untouched; behind: nothing; across: cut at the plane
        assert_eq!(
            clip_segment_near(&cam, near, at(20.0), at(60.0)),
            Some((at(20.0), at(60.0)))
        );
        assert_eq!(clip_segment_near(&cam, near, at(-60.0), at(-20.0)), None);
        let (a, b) = clip_segment_near(&cam, near, at(-30.0), at(70.0)).unwrap();
        assert!((depth_of(&cam, a) - near).abs() < 1e-9 && b == at(70.0));
        let (a, b) = clip_segment_near(&cam, near, at(70.0), at(-30.0)).unwrap();
        assert!(a == at(70.0) && (depth_of(&cam, b) - near).abs() < 1e-9);
        assert_eq!(
            clip_segment_near(&cam, near, [f64::NAN, 0.0, 0.0], at(20.0)),
            None
        );
        // a triangle with one corner behind becomes a quadrilateral all in front, and one that is all behind is nothing
        let tri = [at(30.0), at(40.0), at(-30.0)];
        let cut = clip_polygon_near(&cam, near, &tri);
        assert_eq!(cut.len(), 4);
        assert!(cut.iter().all(|p| depth_of(&cam, *p) >= near - 1e-9));
        assert!(clip_polygon_near(&cam, near, &[at(-1.0), at(-2.0), at(-3.0)]).is_empty());
        assert_eq!(
            clip_polygon_near(&cam, near, &[at(10.0), at(20.0), at(30.0)]).len(),
            3
        );
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
    fn a_box_has_twelve_edges_and_none_when_it_is_behind_the_camera() {
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        let lines = box_lines([10.0, 10.0, 0.0], [50.0, 50.0, 20.0], &cam, vp());
        assert_eq!(lines.len(), 12);
        for (a, b) in &lines {
            assert!(a != b);
        }
        let mut behind = cam;
        behind.target = [100_000.0, 100_000.0, 0.0];
        behind.distance = 5.0;
        assert!(box_lines([10.0, 10.0, 0.0], [50.0, 50.0, 20.0], &behind, vp()).len() < 12);
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
    fn a_cube_keeps_its_edges_sharp_and_a_round_part_is_lit_smoothly() {
        // the corners of a cube are edges: every corner of a face is lit like the face
        let cube_part = placed(40.0, [110.0, 110.0]);
        let cam = cam_for([220.0, 220.0, 250.0], StdView::Corner);
        for t in shade(&[(1, &cube_part)], &cam, vp()) {
            for c in t.corners {
                assert!((c - t.light).abs() < 1e-6, "{c} {}", t.light);
            }
        }
        // a turned part is smooth: the corners of one polygon are lit differently, and the lights stay in the limits
        let round = crate::demo::goblet();
        let mut o = PlacedObject::new(1, "g", round);
        o.pos = [110.0, 110.0];
        let m = o.world_mesh().unwrap();
        let tris = shade(&[(1, &m)], &cam, vp());
        assert!(!tris.is_empty());
        let mut differ = 0usize;
        for t in &tris {
            for c in t.corners {
                assert!((LIGHT_MIN..=LIGHT_MAX).contains(&c), "{c}");
            }
            if t.corners.iter().any(|c| (c - t.corners[0]).abs() > 0.01) {
                differ += 1;
            }
        }
        assert!(differ * 4 > tris.len(), "{differ} of {}", tris.len());
        // neighbours on a smooth surface are lit alike at the point they share, so there is no step between facets
        let pts: Vec<[f64; 3]> = m.vertices.iter().map(|v| v.mm()).collect();
        let normals = smooth_normals(&pts, &m.triangles);
        assert_eq!(normals.len(), m.triangles.len());
        let mut at_point: std::collections::HashMap<u32, Vec<[f64; 3]>> =
            std::collections::HashMap::new();
        for (t, n) in m.triangles.iter().zip(&normals) {
            for k in 0..3 {
                at_point.entry(t[k]).or_default().push(n[k]);
            }
        }
        let smooth = at_point
            .values()
            .filter(|v| v.len() > 1 && v.iter().all(|n| dot(*n, v[0]) > 0.9))
            .count();
        assert!(
            smooth > at_point.len() / 4,
            "{smooth} of {}",
            at_point.len()
        );
    }

    #[test]
    fn the_directions_of_the_corners_are_unit_vectors_and_bad_input_gives_zero() {
        let pts = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 10.0, 0.0],
        ];
        // two triangles of one flat square: every corner looks up
        let flat = smooth_normals(&pts, &[[0, 1, 2], [1, 3, 2]]);
        for tri in &flat {
            for n in tri {
                assert!((n[2] - 1.0).abs() < 1e-9 && n[0].abs() < 1e-9, "{n:?}");
            }
        }
        // a triangle with no area, one with an index that is not there, and nothing at all
        let bad = smooth_normals(&pts, &[[0, 0, 1], [0, 1, 9]]);
        assert_eq!(bad, vec![[[0.0; 3]; 3]; 2]);
        assert!(smooth_normals(&[], &[]).is_empty());
        // a fold of 90 degrees is an edge: each face keeps its own direction
        let fold_pts = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
        ];
        let fold = smooth_normals(&fold_pts, &[[0, 1, 2], [0, 3, 1]]);
        assert!((fold[0][0][2] - 1.0).abs() < 1e-9);
        assert!((fold[1][0][1] - 1.0).abs() < 1e-9, "{:?}", fold[1][0]);
    }

    #[test]
    fn a_small_part_gets_a_view_around_it_and_a_big_one_or_none_gets_the_whole_bed() {
        let volume = [256.0, 256.0, 256.0];
        let whole = ([0.0; 3], volume);
        assert_eq!(focus_box(volume, None), whole);
        // a part as big as half the bed keeps the whole bed in view
        assert_eq!(
            focus_box(volume, Some(([60.0, 60.0, 0.0], [200.0, 200.0, 50.0]))),
            whole
        );
        // the goblet of the demonstration: 45 by 45 by 50 on a bed of 256
        let (lo, hi) = focus_box(volume, Some(([105.5, 105.5, 0.0], [150.5, 150.5, 50.0])));
        let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
        assert!(
            (w - d).abs() < 1e-9 && (256.0 * 0.33..160.0).contains(&w),
            "{w}"
        );
        // the part is in the middle of the box and the box starts on the bed
        assert!(((lo[0] + hi[0]) / 2.0 - 128.0).abs() < 1e-9 && lo[2] == 0.0);
        assert!(hi[2] >= 50.0);
        // a speck of a part is held to a third of the bed, and the box never reaches past what the bed is
        let (l2, h2) = focus_box(volume, Some(([100.0, 100.0, 0.0], [102.0, 102.0, 2.0])));
        assert!((h2[0] - l2[0] - 256.0 * 0.33).abs() < 1e-9);
        // numbers that are not numbers give the whole bed
        for bad in [
            ([f64::NAN, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([0.0, 0.0, 0.0], [f64::INFINITY, 1.0, 1.0]),
            ([5.0, 5.0, 5.0], [1.0, 1.0, 1.0]),
        ] {
            assert_eq!(focus_box(volume, Some(bad)), whole, "{bad:?}");
        }
        assert_eq!(
            focus_box([0.0, 10.0, 10.0], Some(([0.0; 3], [1.0; 3]))),
            ([0.0; 3], [0.0, 10.0, 10.0])
        );
    }

    #[test]
    fn the_part_is_bigger_on_the_screen_with_the_focus_than_with_the_whole_bed() {
        let volume = [256.0, 256.0, 256.0];
        let lo = [105.5, 105.5, 0.0];
        let hi = [150.5, 150.5, 50.0];
        let mut whole = OrbitCamera::default();
        StdView::Corner.apply(&mut whole, volume, vp());
        let mut near = OrbitCamera::default();
        let (flo, fhi) = focus_box(volume, Some((lo, hi)));
        StdView::Corner.apply_to_box(&mut near, flo, fhi, vp());
        let height = |cam: &OrbitCamera| {
            let a = cam.project([128.0, 128.0, 0.0], vp()).unwrap();
            let b = cam.project([128.0, 128.0, 50.0], vp()).unwrap();
            (a[1] - b[1]).abs()
        };
        assert!(
            height(&near) > height(&whole) * 1.8,
            "{} {}",
            height(&near),
            height(&whole)
        );
        // and the part is still inside the view
        for p in [lo, hi, [lo[0], hi[1], hi[2]], [hi[0], lo[1], 0.0]] {
            let s = near.project(p, vp()).unwrap();
            assert!(
                s[0] > 0.0 && s[0] < 800.0 && s[1] > 0.0 && s[1] < 600.0,
                "{s:?}"
            );
        }
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
