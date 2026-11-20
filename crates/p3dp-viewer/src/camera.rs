//! The orbit camera of the viewer: it turns around a point on the plate, moves nearer or farther, slides across the view, projects a point of the world to the screen and
//! turns a point of the screen into a ray. Every number that comes in is checked; a number that is not a number changes nothing. The z axis of the world is up, as on a printer.

use crate::math::{self, Mat4, Vec3};

/// The size of the area the picture is drawn in, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    pub fn new(width: f64, height: f64) -> Option<Viewport> {
        (width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0)
            .then_some(Viewport { width, height })
    }

    pub fn aspect(&self) -> f64 {
        self.width / self.height
    }
}

/// A ray in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// The camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    /// The point the camera looks at and turns around, in millimetres.
    pub target: Vec3,
    /// How far the camera is from the target, in millimetres.
    pub distance: f64,
    /// The turn around the vertical axis, in radians.
    pub yaw: f64,
    /// The height of the camera over the horizontal plane through the target, in radians, kept between -89 and +89 degrees.
    pub pitch: f64,
    /// The opening of the view from top to bottom, in radians.
    pub fov_y: f64,
}

/// The nearest and the farthest the camera may be from its target, in millimetres.
pub const MIN_DISTANCE: f64 = 5.0;
pub const MAX_DISTANCE: f64 = 20_000.0;
const MAX_PITCH_DEG: f64 = 89.0;

impl Default for OrbitCamera {
    fn default() -> OrbitCamera {
        OrbitCamera {
            target: [0.0, 0.0, 0.0],
            distance: 600.0,
            yaw: -std::f64::consts::FRAC_PI_4 * 3.0,
            pitch: 35f64.to_radians(),
            fov_y: 45f64.to_radians(),
        }
    }
}

impl OrbitCamera {
    /// The camera with every number held in its range.
    pub fn sanitized(mut self) -> OrbitCamera {
        let d = OrbitCamera::default();
        if !self.yaw.is_finite() {
            self.yaw = d.yaw;
        }
        self.yaw = self.yaw.rem_euclid(std::f64::consts::TAU);
        let limit = MAX_PITCH_DEG.to_radians();
        self.pitch = if self.pitch.is_finite() {
            self.pitch.clamp(-limit, limit)
        } else {
            d.pitch
        };
        self.distance = if self.distance.is_finite() {
            self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE)
        } else {
            d.distance
        };
        if !self.target.iter().all(|v| v.is_finite()) {
            self.target = d.target;
        }
        if !(self.fov_y.is_finite() && self.fov_y > 0.1 && self.fov_y < 2.0) {
            self.fov_y = d.fov_y;
        }
        self
    }

    /// Where the camera is.
    pub fn eye(&self) -> Vec3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        math::add(
            self.target,
            math::scale(
                [cp * self.yaw.cos(), cp * self.yaw.sin(), sp],
                self.distance,
            ),
        )
    }

    /// The directions of the camera: forward, right and up.
    pub fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let forward =
            math::normalized(math::sub(self.target, self.eye())).unwrap_or([0.0, 1.0, 0.0]);
        let right =
            math::normalized(math::cross(forward, [0.0, 0.0, 1.0])).unwrap_or([1.0, 0.0, 0.0]);
        let up = math::cross(right, forward);
        (forward, right, up)
    }

    /// Turns the camera by what a drag of the mouse gives: pixels across and pixels down, `radians_per_pixel` for each pixel.
    pub fn orbit(&mut self, dx_px: f64, dy_px: f64, radians_per_pixel: f64) {
        if !(dx_px.is_finite() && dy_px.is_finite() && radians_per_pixel.is_finite()) {
            return;
        }
        self.yaw -= dx_px * radians_per_pixel;
        self.pitch += dy_px * radians_per_pixel;
        *self = self.sanitized();
    }

    /// Moves the camera nearer (`factor` under 1) or farther (over 1), by the same proportion each time; a wheel of the mouse gives `0.9` or `1.1`.
    pub fn zoom(&mut self, factor: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        self.distance *= factor;
        *self = self.sanitized();
    }

    /// Slides the view across: a drag of `dx_px` to the right and `dy_px` down moves the target so the model follows the hand.
    pub fn pan(&mut self, dx_px: f64, dy_px: f64, viewport: Viewport) {
        if !(dx_px.is_finite() && dy_px.is_finite()) {
            return;
        }
        let (_, right, up) = self.axes();
        let per_px = 2.0 * self.distance * (self.fov_y / 2.0).tan() / viewport.height;
        let shift = math::add(
            math::scale(right, -dx_px * per_px),
            math::scale(up, dy_px * per_px),
        );
        self.target = math::add(self.target, shift);
        *self = self.sanitized();
    }

    /// The matrix from the world to the camera.
    pub fn view(&self) -> Option<Mat4> {
        math::look_at(self.eye(), self.target, [0.0, 0.0, 1.0])
    }

    /// The depth range that holds the target and what is around it.
    fn planes(&self) -> (f64, f64) {
        (
            (self.distance * 0.02).max(0.5),
            self.distance * 4.0 + 2_000.0,
        )
    }

    /// The matrix from the world to the clip space.
    pub fn view_projection(&self, viewport: Viewport) -> Option<Mat4> {
        let (near, far) = self.planes();
        let p = math::perspective(self.fov_y, viewport.aspect(), near, far)?;
        Some(math::mul(&p, &self.view()?))
    }

    /// The same matrix as the 16 numbers a graphics card takes, column by column.
    pub fn view_projection_f32(&self, viewport: Viewport) -> Option<[f32; 16]> {
        let m = self.view_projection(viewport)?;
        let mut out = [0f32; 16];
        for c in 0..4 {
            for r in 0..4 {
                out[c * 4 + r] = m[r][c] as f32;
            }
        }
        Some(out)
    }

    /// Where a point of the world falls on the screen, in pixels from the top left; `None` for a point behind the camera.
    pub fn project(&self, p: Vec3, viewport: Viewport) -> Option<[f64; 2]> {
        let m = self.view_projection(viewport)?;
        let c = math::transform(&m, p);
        if c[3].is_nan() || c[3] <= 1e-9 {
            return None;
        }
        let (x, y) = (c[0] / c[3], c[1] / c[3]);
        Some([
            (x * 0.5 + 0.5) * viewport.width,
            (1.0 - (y * 0.5 + 0.5)) * viewport.height,
        ])
    }

    /// The ray that goes from the eye through a pixel.
    pub fn ray_through(&self, px: [f64; 2], viewport: Viewport) -> Option<Ray> {
        if !(px[0].is_finite() && px[1].is_finite()) {
            return None;
        }
        let nx = (px[0] / viewport.width) * 2.0 - 1.0;
        let ny = 1.0 - (px[1] / viewport.height) * 2.0;
        let (forward, right, up) = self.axes();
        let t = (self.fov_y / 2.0).tan();
        let dir = math::normalized(math::add(
            forward,
            math::add(
                math::scale(right, nx * t * viewport.aspect()),
                math::scale(up, ny * t),
            ),
        ))?;
        Some(Ray {
            origin: self.eye(),
            direction: dir,
        })
    }

    /// Puts the camera where the whole box is in view, from the direction it is looking now, and turns around the middle of the box.
    pub fn fit_to_box(&mut self, lo: Vec3, hi: Vec3, viewport: Viewport) {
        if !(lo.iter().chain(hi.iter()).all(|v| v.is_finite())) {
            return;
        }
        let centre = math::scale(math::add(lo, hi), 0.5);
        let radius = math::length(math::sub(hi, lo)) / 2.0;
        // the sphere around the box fits when its distance is radius / sin(half the narrower opening)
        let half_v = self.fov_y / 2.0;
        let half_h = (half_v.tan() * viewport.aspect()).atan();
        let half = half_v.min(half_h);
        self.target = centre;
        self.distance = radius.max(1.0) / half.sin() * 1.05;
        *self = self.sanitized();
    }

    /// The camera back where it starts, looking at a plate of this size.
    pub fn reset_for_plate(&mut self, plate_x: f64, plate_y: f64, viewport: Viewport) {
        *self = OrbitCamera::default();
        self.fit_to_box([0.0, 0.0, 0.0], [plate_x, plate_y, 0.0], viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> Viewport {
        Viewport::new(800.0, 600.0).unwrap()
    }

    #[test]
    fn the_viewport_wants_a_real_size() {
        assert!(Viewport::new(0.0, 10.0).is_none());
        assert!(Viewport::new(10.0, f64::NAN).is_none());
        assert!(Viewport::new(f64::INFINITY, 10.0).is_none());
        assert!((vp().aspect() - 4.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn dragging_turns_the_camera_and_the_tilt_stops_short_of_the_poles() {
        let mut c = OrbitCamera::default();
        let (y0, p0) = (c.yaw, c.pitch);
        c.orbit(100.0, 0.0, 0.01);
        assert!((c.yaw - (y0 - 1.0).rem_euclid(std::f64::consts::TAU)).abs() < 1e-9);
        assert_eq!(c.pitch, p0);
        c.orbit(0.0, 10_000.0, 0.01);
        assert!((c.pitch - 89f64.to_radians()).abs() < 1e-12, "{}", c.pitch);
        c.orbit(0.0, -100_000.0, 0.01);
        assert!((c.pitch + 89f64.to_radians()).abs() < 1e-12);
        // the camera is always at the same distance from the target whatever the turn
        for (dx, dy) in [(10.0, 20.0), (-300.0, 40.0), (1e6, -1e6)] {
            c.orbit(dx, dy, 0.003);
            assert!((math::length(math::sub(c.eye(), c.target)) - c.distance).abs() < 1e-6);
        }
        // numbers that are not numbers change nothing
        let before = c;
        c.orbit(f64::NAN, 1.0, 0.01);
        c.orbit(1.0, f64::INFINITY, 0.01);
        c.orbit(1.0, 1.0, f64::NAN);
        assert_eq!(c, before);
    }

    #[test]
    fn the_wheel_moves_by_proportion_and_stops_at_both_ends() {
        let mut c = OrbitCamera::default();
        let d = c.distance;
        c.zoom(0.9);
        c.zoom(0.9);
        assert!((c.distance - d * 0.81).abs() < 1e-9);
        c.zoom(1.0 / 0.9);
        c.zoom(1.0 / 0.9);
        assert!((c.distance - d).abs() < 1e-9);
        for _ in 0..200 {
            c.zoom(0.5);
        }
        assert_eq!(c.distance, MIN_DISTANCE);
        for _ in 0..200 {
            c.zoom(2.0);
        }
        assert_eq!(c.distance, MAX_DISTANCE);
        let before = c;
        for bad in [f64::NAN, 0.0, -1.0, f64::INFINITY] {
            c.zoom(bad);
        }
        assert_eq!(c, before);
    }

    #[test]
    fn panning_keeps_the_picture_under_the_hand() {
        let mut c = OrbitCamera {
            target: [100.0, 100.0, 0.0],
            ..OrbitCamera::default()
        };
        // a point at the depth of the target moves exactly as far as the hand (one nearer or farther moves a little more or less)
        let probe = c.target;
        let before = c.project(probe, vp()).unwrap();
        c.pan(30.0, -12.0, vp());
        let after = c.project(probe, vp()).unwrap();
        assert!(
            (after[0] - before[0] - 30.0).abs() < 1e-6,
            "{before:?} {after:?}"
        );
        assert!(
            (after[1] - before[1] + 12.0).abs() < 1e-6,
            "{before:?} {after:?}"
        );
        // the camera moved with the target: same distance, same angles
        assert!((math::length(math::sub(c.eye(), c.target)) - c.distance).abs() < 1e-9);
        let t = c.target;
        c.pan(f64::NAN, 1.0, vp());
        assert_eq!(c.target, t);
    }

    #[test]
    fn the_target_is_in_the_middle_of_the_screen_and_what_is_behind_the_camera_is_not_shown() {
        let c = OrbitCamera {
            target: [110.0, 110.0, 0.0],
            ..OrbitCamera::default()
        };
        let m = c.project(c.target, vp()).unwrap();
        assert!(
            (m[0] - 400.0).abs() < 1e-6 && (m[1] - 300.0).abs() < 1e-6,
            "{m:?}"
        );
        let (fwd, _, _) = c.axes();
        let behind = math::sub(c.eye(), math::scale(fwd, 50.0));
        assert!(c.project(behind, vp()).is_none());
        // up on the screen is up in the world
        let higher = c.project([110.0, 110.0, 30.0], vp()).unwrap();
        assert!(higher[1] < m[1]);
    }

    #[test]
    fn a_pixel_gives_a_ray_through_the_point_that_projects_to_it() {
        let mut c = OrbitCamera {
            target: [90.0, 120.0, 20.0],
            ..OrbitCamera::default()
        };
        c.orbit(37.0, -12.0, 0.01);
        for p in [
            [80.0, 100.0, 0.0],
            [120.0, 150.0, 45.0],
            [90.0, 120.0, 20.0],
        ] {
            let px = c.project(p, vp()).unwrap();
            let ray = c.ray_through(px, vp()).unwrap();
            // the distance from the point to the ray is nothing
            let to_p = math::sub(p, ray.origin);
            let along = math::dot(to_p, ray.direction);
            let off = math::length(math::sub(to_p, math::scale(ray.direction, along)));
            assert!(off < 1e-6, "{p:?}: {off}");
            assert!(along > 0.0);
        }
        assert!(c.ray_through([f64::NAN, 3.0], vp()).is_none());
    }

    #[test]
    fn fitting_a_box_puts_all_its_corners_in_view() {
        let mut c = OrbitCamera::default();
        let (lo, hi) = ([10.0, 20.0, 0.0], [60.0, 90.0, 40.0]);
        for aspect in [(800.0, 600.0), (300.0, 900.0), (1600.0, 400.0)] {
            let v = Viewport::new(aspect.0, aspect.1).unwrap();
            c.fit_to_box(lo, hi, v);
            for x in [lo[0], hi[0]] {
                for y in [lo[1], hi[1]] {
                    for z in [lo[2], hi[2]] {
                        let p = c.project([x, y, z], v).unwrap();
                        assert!(
                            p[0] >= 0.0 && p[0] <= v.width && p[1] >= 0.0 && p[1] <= v.height,
                            "{aspect:?}: {p:?}"
                        );
                    }
                }
            }
            assert_eq!(c.target, [35.0, 55.0, 20.0]);
        }
        let t = c.target;
        c.fit_to_box([f64::NAN, 0.0, 0.0], hi, vp());
        assert_eq!(c.target, t);
        c.reset_for_plate(256.0, 256.0, vp());
        assert_eq!(c.target, [128.0, 128.0, 0.0]);
        assert_eq!(
            c.yaw,
            OrbitCamera::default().yaw.rem_euclid(std::f64::consts::TAU)
        );
    }

    #[test]
    fn the_matrix_for_a_graphics_card_goes_column_by_column() {
        let c = OrbitCamera::default();
        let m = c.view_projection(vp()).unwrap();
        let g = c.view_projection_f32(vp()).unwrap();
        for r in 0..4 {
            for col in 0..4 {
                assert!((g[col * 4 + r] as f64 - m[r][col]).abs() < 1e-3 * (1.0 + m[r][col].abs()));
            }
        }
    }

    #[test]
    fn a_camera_with_nonsense_in_it_is_put_right() {
        let c = OrbitCamera {
            target: [f64::NAN, 0.0, 0.0],
            distance: f64::INFINITY,
            yaw: f64::NAN,
            pitch: 7.0,
            fov_y: 0.0,
        }
        .sanitized();
        assert!(c.target.iter().all(|v| v.is_finite()));
        assert!(c.distance.is_finite() && c.yaw.is_finite() && c.fov_y > 0.0);
        assert!((c.pitch - 89f64.to_radians()).abs() < 1e-12);
        assert!(c.eye().iter().all(|v| v.is_finite()));
    }
}
