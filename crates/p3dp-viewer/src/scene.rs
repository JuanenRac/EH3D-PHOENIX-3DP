//! The scene on the plate: the bed, the objects on it as boxes, which of them are selected, the click that picks the nearest one under the pointer and the box dragged over the screen
//! that picks all those inside it or touched by it. It works on the boxes of the objects (the lowest and the highest corner), which is what a click needs; the triangles are not
//! looked at.

use crate::camera::{OrbitCamera, Ray, Viewport};
use crate::math::Vec3;

/// The bed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plate {
    pub size_x_mm: f64,
    pub size_y_mm: f64,
}

/// An object on the plate, as its box.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject {
    pub id: u32,
    pub name: String,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// A rectangle dragged on the screen, in pixels; its corners may be given in any order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl ScreenRect {
    /// The rectangle with its corners in order; `None` for a corner that is not a number.
    pub fn normalized(self) -> Option<ScreenRect> {
        if ![self.x0, self.y0, self.x1, self.y1]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }
        Some(ScreenRect {
            x0: self.x0.min(self.x1),
            y0: self.y0.min(self.y1),
            x1: self.x0.max(self.x1),
            y1: self.y0.max(self.y1),
        })
    }

    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.x0 && p[0] <= self.x1 && p[1] >= self.y0 && p[1] <= self.y1
    }

    fn overlaps(&self, other: &ScreenRect) -> bool {
        self.x0 <= other.x1 && self.x1 >= other.x0 && self.y0 <= other.y1 && self.y1 >= other.y0
    }
}

/// What the dragged box must do to an object to pick it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxMode {
    /// The whole object is inside the box.
    Inside,
    /// Any part of the object is in the box.
    Touching,
}

/// Why an object was not added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    /// The id is used already.
    Duplicate(u32),
    /// The box is not made of numbers or its low corner is above its high one.
    BadBox,
    /// The object is not on the plate.
    OffPlate,
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneError::Duplicate(id) => write!(f, "the object {id} is already on the plate"),
            SceneError::BadBox => f.write_str("the box of the object is not a box"),
            SceneError::OffPlate => f.write_str("the object is not on the plate"),
        }
    }
}

impl std::error::Error for SceneError {}

/// The plate with its objects and the selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub plate: Plate,
    objects: Vec<SceneObject>,
    selected: Vec<u32>,
}

/// The nearest point of a ray in a box: the distance along the ray, or `None` when the ray misses it (the slab method).
pub fn ray_hits_box(ray: &Ray, lo: Vec3, hi: Vec3) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, f64::INFINITY);
    for axis in 0..3 {
        let (o, d) = (ray.origin[axis], ray.direction[axis]);
        if d.abs() < 1e-12 {
            if o < lo[axis] || o > hi[axis] {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((lo[axis] - o) / d, (hi[axis] - o) / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        near = near.max(t0);
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    Some(near)
}

impl Scene {
    pub fn new(size_x_mm: f64, size_y_mm: f64) -> Option<Scene> {
        (size_x_mm.is_finite() && size_y_mm.is_finite() && size_x_mm > 0.0 && size_y_mm > 0.0).then(
            || Scene {
                plate: Plate {
                    size_x_mm,
                    size_y_mm,
                },
                objects: Vec::new(),
                selected: Vec::new(),
            },
        )
    }

    pub fn objects(&self) -> &[SceneObject] {
        &self.objects
    }

    /// Puts an object on the plate: its box must be a box and lie over the plate.
    pub fn add(&mut self, o: SceneObject) -> Result<(), SceneError> {
        if self.objects.iter().any(|x| x.id == o.id) {
            return Err(SceneError::Duplicate(o.id));
        }
        let finite = o.lo.iter().chain(o.hi.iter()).all(|v| v.is_finite());
        if !finite || (0..3).any(|a| o.lo[a] > o.hi[a]) {
            return Err(SceneError::BadBox);
        }
        if o.lo[0] < 0.0
            || o.lo[1] < 0.0
            || o.hi[0] > self.plate.size_x_mm
            || o.hi[1] > self.plate.size_y_mm
        {
            return Err(SceneError::OffPlate);
        }
        self.objects.push(o);
        Ok(())
    }

    /// Takes an object off, and off the selection too.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.objects.len();
        self.objects.retain(|o| o.id != id);
        self.selected.retain(|s| *s != id);
        self.objects.len() != before
    }

    /// The box that holds all the objects, or the plate when there are none.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        if self.objects.is_empty() {
            return (
                [0.0, 0.0, 0.0],
                [self.plate.size_x_mm, self.plate.size_y_mm, 0.0],
            );
        }
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for o in &self.objects {
            for a in 0..3 {
                lo[a] = lo[a].min(o.lo[a]);
                hi[a] = hi[a].max(o.hi[a]);
            }
        }
        (lo, hi)
    }

    /// The ids that are selected, in the order they were selected.
    pub fn selected(&self) -> &[u32] {
        &self.selected
    }

    pub fn is_selected(&self, id: u32) -> bool {
        self.selected.contains(&id)
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Selects an object (only one that is on the plate); `add` keeps the rest of the selection, otherwise it is replaced.
    pub fn select(&mut self, id: u32, add: bool) -> bool {
        if !self.objects.iter().any(|o| o.id == id) {
            return false;
        }
        if !add {
            self.selected.clear();
        }
        if !self.selected.contains(&id) {
            self.selected.push(id);
        }
        true
    }

    /// Flips an object in or out of the selection.
    pub fn toggle(&mut self, id: u32) -> bool {
        if !self.objects.iter().any(|o| o.id == id) {
            return false;
        }
        match self.selected.iter().position(|s| *s == id) {
            Some(i) => {
                self.selected.remove(i);
            }
            None => self.selected.push(id),
        }
        true
    }

    /// The object nearest to the camera under a pixel.
    pub fn pick(&self, camera: &OrbitCamera, viewport: Viewport, px: [f64; 2]) -> Option<u32> {
        let ray = camera.ray_through(px, viewport)?;
        self.objects
            .iter()
            .filter_map(|o| ray_hits_box(&ray, o.lo, o.hi).map(|t| (t, o.id)))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, id)| id)
    }

    /// The rectangle on the screen that the box of an object covers; `None` when any corner is behind the camera.
    pub fn screen_rect(
        o: &SceneObject,
        camera: &OrbitCamera,
        viewport: Viewport,
    ) -> Option<ScreenRect> {
        let mut r = ScreenRect {
            x0: f64::INFINITY,
            y0: f64::INFINITY,
            x1: f64::NEG_INFINITY,
            y1: f64::NEG_INFINITY,
        };
        for x in [o.lo[0], o.hi[0]] {
            for y in [o.lo[1], o.hi[1]] {
                for z in [o.lo[2], o.hi[2]] {
                    let p = camera.project([x, y, z], viewport)?;
                    r.x0 = r.x0.min(p[0]);
                    r.y0 = r.y0.min(p[1]);
                    r.x1 = r.x1.max(p[0]);
                    r.y1 = r.y1.max(p[1]);
                }
            }
        }
        Some(r)
    }

    /// The ids of the objects that a rectangle dragged on the screen picks, in the order of the plate.
    pub fn in_box(
        &self,
        camera: &OrbitCamera,
        viewport: Viewport,
        rect: ScreenRect,
        mode: BoxMode,
    ) -> Vec<u32> {
        let Some(rect) = rect.normalized() else {
            return Vec::new();
        };
        self.objects
            .iter()
            .filter(|o| {
                let Some(r) = Scene::screen_rect(o, camera, viewport) else {
                    return false;
                };
                match mode {
                    BoxMode::Inside => rect.contains([r.x0, r.y0]) && rect.contains([r.x1, r.y1]),
                    BoxMode::Touching => rect.overlaps(&r),
                }
            })
            .map(|o| o.id)
            .collect()
    }

    /// Selects what a dragged rectangle picks; `add` keeps the selection that was there. Returns how many it picked.
    pub fn select_in_box(
        &mut self,
        camera: &OrbitCamera,
        viewport: Viewport,
        rect: ScreenRect,
        mode: BoxMode,
        add: bool,
    ) -> usize {
        let ids = self.in_box(camera, viewport, rect, mode);
        if !add {
            self.selected.clear();
        }
        for id in &ids {
            if !self.selected.contains(id) {
                self.selected.push(*id);
            }
        }
        ids.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> Viewport {
        Viewport::new(1000.0, 700.0).unwrap()
    }

    fn obj(id: u32, x: f64, y: f64, size: f64) -> SceneObject {
        SceneObject {
            id,
            name: format!("part {id}"),
            lo: [x, y, 0.0],
            hi: [x + size, y + size, size],
        }
    }

    fn scene() -> Scene {
        let mut s = Scene::new(256.0, 256.0).unwrap();
        s.add(obj(1, 20.0, 20.0, 30.0)).unwrap();
        s.add(obj(2, 120.0, 20.0, 30.0)).unwrap();
        s.add(obj(3, 120.0, 140.0, 50.0)).unwrap();
        s
    }

    fn cam(s: &Scene) -> OrbitCamera {
        let mut c = OrbitCamera::default();
        c.reset_for_plate(s.plate.size_x_mm, s.plate.size_y_mm, vp());
        c
    }

    #[test]
    fn the_plate_wants_a_size_and_an_object_wants_to_be_on_it() {
        assert!(Scene::new(0.0, 100.0).is_none());
        assert!(Scene::new(100.0, f64::NAN).is_none());
        let mut s = Scene::new(100.0, 100.0).unwrap();
        s.add(obj(1, 10.0, 10.0, 20.0)).unwrap();
        assert_eq!(
            s.add(obj(1, 50.0, 50.0, 10.0)),
            Err(SceneError::Duplicate(1))
        );
        assert_eq!(s.add(obj(2, 90.0, 10.0, 20.0)), Err(SceneError::OffPlate));
        assert_eq!(s.add(obj(3, -1.0, 10.0, 5.0)), Err(SceneError::OffPlate));
        let mut inverted = obj(4, 10.0, 10.0, 5.0);
        inverted.hi[2] = -1.0;
        assert_eq!(s.add(inverted), Err(SceneError::BadBox));
        let mut nan = obj(5, 10.0, 10.0, 5.0);
        nan.lo[0] = f64::NAN;
        assert_eq!(s.add(nan), Err(SceneError::BadBox));
        assert_eq!(s.objects().len(), 1);
        assert!(SceneError::OffPlate.to_string().contains("plate"));
    }

    #[test]
    fn a_ray_hits_the_nearest_face_of_a_box_or_misses_it() {
        let ray = Ray {
            origin: [0.0, -10.0, 5.0],
            direction: [0.0, 1.0, 0.0],
        };
        assert_eq!(
            ray_hits_box(&ray, [-5.0, 0.0, 0.0], [5.0, 10.0, 10.0]),
            Some(10.0)
        );
        assert_eq!(
            ray_hits_box(&ray, [20.0, 0.0, 0.0], [30.0, 10.0, 10.0]),
            None
        );
        // a ray that starts inside a box hits it at once; a box behind the ray is not hit
        let inside = Ray {
            origin: [0.0, 5.0, 5.0],
            direction: [0.0, 1.0, 0.0],
        };
        assert_eq!(
            ray_hits_box(&inside, [-5.0, 0.0, 0.0], [5.0, 10.0, 10.0]),
            Some(0.0)
        );
        let away = Ray {
            origin: [0.0, 50.0, 5.0],
            direction: [0.0, 1.0, 0.0],
        };
        assert_eq!(
            ray_hits_box(&away, [-5.0, 0.0, 0.0], [5.0, 10.0, 10.0]),
            None
        );
    }

    #[test]
    fn a_click_picks_the_object_under_the_pointer_and_the_nearest_of_two() {
        let s = scene();
        let c = cam(&s);
        for o in s.objects() {
            let centre = [
                (o.lo[0] + o.hi[0]) / 2.0,
                (o.lo[1] + o.hi[1]) / 2.0,
                (o.lo[2] + o.hi[2]) / 2.0,
            ];
            let px = c.project(centre, vp()).unwrap();
            assert_eq!(s.pick(&c, vp(), px), Some(o.id));
        }
        // a click on the empty corner of the plate picks nothing
        let corner = c.project([250.0, 250.0, 0.0], vp()).unwrap();
        assert_eq!(s.pick(&c, vp(), corner), None);
        assert_eq!(s.pick(&c, vp(), [f64::NAN, 3.0]), None);
        // two boxes one behind the other along the line of sight: the nearer one wins
        let mut two = Scene::new(256.0, 256.0).unwrap();
        two.add(obj(10, 100.0, 100.0, 20.0)).unwrap();
        two.add(obj(11, 100.0, 140.0, 20.0)).unwrap();
        let mut front = OrbitCamera {
            target: [110.0, 130.0, 10.0],
            yaw: -std::f64::consts::FRAC_PI_2,
            pitch: 0.0,
            ..OrbitCamera::default()
        };
        front.distance = 400.0;
        let centre = front.project([110.0, 130.0, 10.0], vp()).unwrap();
        assert_eq!(
            two.pick(&front, vp(), centre),
            Some(10),
            "the camera is at the low y side"
        );
        let mut behind = front;
        behind.yaw = std::f64::consts::FRAC_PI_2;
        assert_eq!(two.pick(&behind, vp(), centre), Some(11));
    }

    #[test]
    fn the_selection_is_replaced_added_to_and_toggled_and_follows_what_is_removed() {
        let mut s = scene();
        assert!(s.select(1, false) && s.select(2, true) && s.select(2, true));
        assert_eq!(s.selected(), &[1, 2]);
        assert!(s.select(3, false));
        assert_eq!(s.selected(), &[3]);
        assert!(s.toggle(1) && s.toggle(3));
        assert_eq!(s.selected(), &[1]);
        assert!(!s.select(99, true) && !s.toggle(99));
        assert!(s.is_selected(1) && !s.is_selected(3));
        assert!(s.remove(1));
        assert!(s.selected().is_empty() && !s.remove(1));
        s.select(2, false);
        s.clear_selection();
        assert!(s.selected().is_empty());
    }

    #[test]
    fn a_box_dragged_over_the_screen_picks_what_is_inside_or_touched() {
        let s = scene();
        let c = cam(&s);
        let r = |id: usize| Scene::screen_rect(&s.objects()[id - 1], &c, vp()).unwrap();
        // a box around the first object only
        let (a, b) = (r(1), r(2));
        let around_one = ScreenRect {
            x0: a.x1 + 3.0,
            y0: a.y1 + 3.0,
            x1: a.x0 - 3.0,
            y1: a.y0 - 3.0,
        };
        assert_eq!(s.in_box(&c, vp(), around_one, BoxMode::Inside), vec![1]);
        // a box that cuts the second object picks it only when touching counts
        let cut = ScreenRect {
            x0: (b.x0 + b.x1) / 2.0,
            y0: b.y0 - 5.0,
            x1: b.x1 + 40.0,
            y1: b.y1 + 5.0,
        };
        assert!(s.in_box(&c, vp(), cut, BoxMode::Inside).is_empty());
        assert_eq!(s.in_box(&c, vp(), cut, BoxMode::Touching), vec![2]);
        // the whole screen picks all; a box of nothing picks nothing; a corner that is no number picks nothing
        let all = ScreenRect {
            x0: 0.0,
            y0: 0.0,
            x1: 1000.0,
            y1: 700.0,
        };
        assert_eq!(s.in_box(&c, vp(), all, BoxMode::Inside), vec![1, 2, 3]);
        let none = ScreenRect {
            x0: 0.0,
            y0: 0.0,
            x1: 2.0,
            y1: 2.0,
        };
        assert!(s.in_box(&c, vp(), none, BoxMode::Touching).is_empty());
        let bad = ScreenRect {
            x0: f64::NAN,
            ..all
        };
        assert!(s.in_box(&c, vp(), bad, BoxMode::Touching).is_empty());
        // selecting by box replaces or adds
        let mut t = s.clone();
        t.select(3, false);
        assert_eq!(
            t.select_in_box(&c, vp(), around_one, BoxMode::Inside, true),
            1
        );
        assert_eq!(t.selected(), &[3, 1]);
        assert_eq!(
            t.select_in_box(&c, vp(), around_one, BoxMode::Inside, false),
            1
        );
        assert_eq!(t.selected(), &[1]);
    }

    #[test]
    fn the_bounds_of_the_scene_are_those_of_its_objects_or_the_plate() {
        let s = scene();
        assert_eq!(s.bounds(), ([20.0, 20.0, 0.0], [170.0, 190.0, 50.0]));
        let empty = Scene::new(200.0, 100.0).unwrap();
        assert_eq!(empty.bounds(), ([0.0, 0.0, 0.0], [200.0, 100.0, 0.0]));
    }
}
