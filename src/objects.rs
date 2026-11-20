//! The objects on the bed, without a window: each one is a mesh with a position, a turn and a scale that a person types or drags, and the bed keeps them apart. Everything the
//! right panel of the Prepare tab does goes through here: place, turn, scale to a size in millimetres, stand the part the way that needs the fewest supports, arrange several parts
//! on the bed with a gap between them, say which ones go past the edge, and join the parts into the one mesh the slicer takes. All of it is plain arithmetic and is tested.

use p3dp_core::mesh::Point3;
use p3dp_core::units::Nm;
use p3dp_core::Mesh;

/// The most objects on one bed.
pub const MAX_OBJECTS: usize = 64;
/// The smallest and the largest scale factor of an axis.
pub const SCALE_MIN: f64 = 0.01;
pub const SCALE_MAX: f64 = 100.0;

/// The corners of a box, in millimetres.
pub type Bounds = ([f64; 3], [f64; 3]);

/// One part on the bed. The mesh is kept as it was loaded; what the person changes is the placement. The scale and the turn are about the middle of the part, and the part always
/// rests on the bed: its lowest point is at height 0 and the middle of its footprint is at `pos`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub id: u32,
    pub name: String,
    pub mesh: Mesh,
    /// Where the middle of the box of the part is on the bed, in millimetres from the corner of the bed.
    pub pos: [f64; 2],
    /// Turns about x, then y, then z, in degrees.
    pub rot_deg: [f64; 3],
    /// A factor for each axis of the part before it is turned (1 is the size it was loaded with).
    pub scale: [f64; 3],
}

fn rotate(p: [f64; 3], rot_deg: [f64; 3]) -> [f64; 3] {
    let (sx, cx) = rot_deg[0].to_radians().sin_cos();
    let (sy, cy) = rot_deg[1].to_radians().sin_cos();
    let (sz, cz) = rot_deg[2].to_radians().sin_cos();
    let (x, y, z) = (p[0], p[1] * cx - p[2] * sx, p[1] * sx + p[2] * cx);
    let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
    [x * cz - y * sz, x * sz + y * cz, z]
}

/// The corners of a box that holds all the points, or `None` when there are none.
fn bounds_of(points: &[[f64; 3]]) -> Option<Bounds> {
    let first = *points.first()?;
    let (mut lo, mut hi) = (first, first);
    for p in points {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    Some((lo, hi))
}

impl PlacedObject {
    pub fn new(id: u32, name: &str, mesh: Mesh) -> PlacedObject {
        PlacedObject {
            id,
            name: name.to_string(),
            mesh,
            pos: [0.0, 0.0],
            rot_deg: [0.0; 3],
            scale: [1.0; 3],
        }
    }

    /// The points of the part on the bed, in millimetres.
    pub fn world_points(&self) -> Vec<[f64; 3]> {
        let raw: Vec<[f64; 3]> = self.mesh.vertices.iter().map(Point3::mm).collect();
        let Some((lo, hi)) = bounds_of(&raw) else {
            return Vec::new();
        };
        let mid = [
            (lo[0] + hi[0]) / 2.0,
            (lo[1] + hi[1]) / 2.0,
            (lo[2] + hi[2]) / 2.0,
        ];
        let turned: Vec<[f64; 3]> = raw
            .iter()
            .map(|p| {
                rotate(
                    [
                        (p[0] - mid[0]) * self.scale[0],
                        (p[1] - mid[1]) * self.scale[1],
                        (p[2] - mid[2]) * self.scale[2],
                    ],
                    self.rot_deg,
                )
            })
            .collect();
        let Some((tlo, thi)) = bounds_of(&turned) else {
            return Vec::new();
        };
        let shift = [
            self.pos[0] - (tlo[0] + thi[0]) / 2.0,
            self.pos[1] - (tlo[1] + thi[1]) / 2.0,
            -tlo[2],
        ];
        turned
            .iter()
            .map(|p| [p[0] + shift[0], p[1] + shift[1], p[2] + shift[2]])
            .collect()
    }

    /// The box of the part on the bed.
    pub fn world_bounds(&self) -> Option<Bounds> {
        bounds_of(&self.world_points())
    }

    /// The size of the box of the part on the bed.
    pub fn world_size(&self) -> Option<[f64; 3]> {
        self.world_bounds()
            .map(|(lo, hi)| [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])
    }

    /// The part as a mesh on the bed. A point that falls outside what the units can hold is an error.
    pub fn world_mesh(&self) -> Result<Mesh, String> {
        let mut vertices = Vec::with_capacity(self.mesh.vertices.len());
        for p in self.world_points() {
            let c = |v: f64| Nm::from_mm(v).map_err(|e| e.to_string());
            vertices.push(Point3::new(c(p[0])?, c(p[1])?, c(p[2])?));
        }
        // a mirror (a negative scale is not allowed, so this never flips the winding)
        Ok(Mesh {
            vertices,
            triangles: self.mesh.triangles.clone(),
        })
    }

    /// Sets the size of one axis of the box of the part before it is turned, in millimetres; with `uniform` the other two axes follow in proportion.
    pub fn set_size_mm(&mut self, axis: usize, mm: f64, uniform: bool) -> bool {
        let Some((lo, hi)) = self.mesh_bounds() else {
            return false;
        };
        if axis > 2 || !mm.is_finite() || mm <= 0.0 {
            return false;
        }
        let base = hi[axis] - lo[axis];
        if base <= 1e-9 {
            return false;
        }
        let factor = mm / base;
        if !(SCALE_MIN..=SCALE_MAX).contains(&factor) {
            return false;
        }
        if uniform {
            let ratio = factor / self.scale[axis];
            let next = self.scale.map(|s| s * ratio);
            if next.iter().any(|s| !(SCALE_MIN..=SCALE_MAX).contains(s)) {
                return false;
            }
            self.scale = next;
        } else {
            self.scale[axis] = factor;
        }
        true
    }

    /// The size of the part before it is turned, in millimetres, with its scale.
    pub fn size_mm(&self) -> Option<[f64; 3]> {
        let (lo, hi) = self.mesh_bounds()?;
        Some([
            (hi[0] - lo[0]) * self.scale[0],
            (hi[1] - lo[1]) * self.scale[1],
            (hi[2] - lo[2]) * self.scale[2],
        ])
    }

    fn mesh_bounds(&self) -> Option<Bounds> {
        let raw: Vec<[f64; 3]> = self.mesh.vertices.iter().map(Point3::mm).collect();
        bounds_of(&raw)
    }

    /// Sets the scale in percent on all the axes (100 is the size it was loaded with).
    pub fn set_scale_percent(&mut self, percent: f64) -> bool {
        let f = percent / 100.0;
        if !f.is_finite() || !(SCALE_MIN..=SCALE_MAX).contains(&f) {
            return false;
        }
        self.scale = [f; 3];
        true
    }

    /// Sets a turn, in degrees, held between -360 and 360.
    pub fn set_rotation(&mut self, axis: usize, deg: f64) -> bool {
        if axis > 2 || !deg.is_finite() {
            return false;
        }
        self.rot_deg[axis] = deg.clamp(-360.0, 360.0);
        true
    }

    /// Puts the turn and the scale back to what they were when the part was loaded.
    pub fn reset(&mut self) {
        self.rot_deg = [0.0; 3];
        self.scale = [1.0; 3];
    }
}

/// How well a turn of a part suits a print: the area that hangs in the air at more than 45 degrees (it needs supports), the area that lies on the bed and the height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stance {
    pub overhang_mm2: f64,
    pub contact_mm2: f64,
    pub height_mm: f64,
}

impl Stance {
    /// A number to compare stances with: less is better.
    pub fn cost(&self) -> f64 {
        self.overhang_mm2 - 0.25 * self.contact_mm2 + 0.05 * self.height_mm
    }
}

/// The stance of a part with the turn it has now.
pub fn stance_of(obj: &PlacedObject) -> Option<Stance> {
    let pts = obj.world_points();
    let (lo, hi) = bounds_of(&pts)?;
    let mut overhang = 0.0;
    let mut contact = 0.0;
    for t in &obj.mesh.triangles {
        let get = |i: u32| pts.get(i as usize).copied();
        let (Some(a), Some(b), Some(c)) = (get(t[0]), get(t[1]), get(t[2])) else {
            continue;
        };
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-12 {
            continue;
        }
        let (area, nz) = (len / 2.0, n[2] / len);
        if nz < -std::f64::consts::FRAC_1_SQRT_2 {
            let lowest = a[2].min(b[2]).min(c[2]);
            if lowest - lo[2] < 0.05 {
                contact += area;
            } else {
                overhang += area;
            }
        }
    }
    Some(Stance {
        overhang_mm2: overhang,
        contact_mm2: contact,
        height_mm: hi[2] - lo[2],
    })
}

/// The six ways of standing a part on one of its faces, as turns about x and y in degrees.
pub const STANDS: [[f64; 3]; 6] = [
    [0.0, 0.0, 0.0],
    [90.0, 0.0, 0.0],
    [-90.0, 0.0, 0.0],
    [180.0, 0.0, 0.0],
    [0.0, 90.0, 0.0],
    [0.0, -90.0, 0.0],
];

/// The turn that needs the fewest supports among the six ways of standing the part on a face; a tie goes to the first (the way it was loaded). The turn about z is not touched.
pub fn auto_orient(obj: &PlacedObject) -> [f64; 3] {
    let mut best = (f64::INFINITY, obj.rot_deg);
    for stand in STANDS {
        let mut probe = obj.clone();
        probe.rot_deg = [stand[0], stand[1], obj.rot_deg[2]];
        if let Some(s) = stance_of(&probe) {
            let c = s.cost();
            if c < best.0 - 1e-9 {
                best = (c, probe.rot_deg);
            }
        }
    }
    best.1
}

/// What an arrangement did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Arranged {
    /// The parts that were placed on the bed.
    pub placed: usize,
    /// The parts that did not fit in what was left (they are put in the middle, and the person is told).
    pub left_out: Vec<u32>,
}

/// The bed with its objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plate {
    pub objects: Vec<PlacedObject>,
    pub selected: Option<u32>,
    next_id: u32,
}

impl Plate {
    pub fn new() -> Plate {
        Plate::default()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Adds a part in the middle of a bed and selects it. `None` when the bed holds the most parts it can.
    pub fn add(&mut self, name: &str, mesh: Mesh, bed_mm: [f64; 2]) -> Option<u32> {
        if self.objects.len() >= MAX_OBJECTS {
            return None;
        }
        self.next_id += 1;
        let mut o = PlacedObject::new(self.next_id, name, mesh);
        o.pos = [bed_mm[0] / 2.0, bed_mm[1] / 2.0];
        let id = o.id;
        self.objects.push(o);
        self.selected = Some(id);
        Some(id)
    }

    pub fn get(&self, id: u32) -> Option<&PlacedObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut PlacedObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn selected_object(&self) -> Option<&PlacedObject> {
        self.selected.and_then(|id| self.get(id))
    }

    pub fn selected_mut(&mut self) -> Option<&mut PlacedObject> {
        let id = self.selected?;
        self.get_mut(id)
    }

    /// Selects a part; an id that is not on the bed is refused.
    pub fn select(&mut self, id: u32) -> bool {
        if self.get(id).is_some() {
            self.selected = Some(id);
            true
        } else {
            false
        }
    }

    /// Takes a part off the bed. The selection goes to the last part that is left.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.objects.len();
        self.objects.retain(|o| o.id != id);
        if self.selected == Some(id) || self.get(self.selected.unwrap_or(0)).is_none() {
            self.selected = self.objects.last().map(|o| o.id);
        }
        self.objects.len() != before
    }

    /// Makes copies of a part, side by side (the bed is arranged afterwards by the caller). Returns the ids of the copies.
    pub fn duplicate(&mut self, id: u32, copies: usize) -> Vec<u32> {
        let Some(src) = self.get(id).cloned() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for _ in 0..copies {
            if self.objects.len() >= MAX_OBJECTS {
                break;
            }
            self.next_id += 1;
            let mut c = src.clone();
            c.id = self.next_id;
            out.push(c.id);
            self.objects.push(c);
        }
        if let Some(last) = out.last() {
            self.selected = Some(*last);
        }
        out
    }

    /// The box of all the parts together.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut all = Vec::new();
        for o in &self.objects {
            if let Some((lo, hi)) = o.world_bounds() {
                all.push(lo);
                all.push(hi);
            }
        }
        bounds_of(&all)
    }

    /// The parts that go past the edge of the bed (less a margin) or past its height.
    pub fn out_of_bounds(&self, volume_mm: [f64; 3], margin_mm: f64) -> Vec<u32> {
        self.objects
            .iter()
            .filter(|o| {
                o.world_bounds().is_some_and(|(lo, hi)| {
                    lo[0] < margin_mm - 1e-6
                        || lo[1] < margin_mm - 1e-6
                        || hi[0] > volume_mm[0] - margin_mm + 1e-6
                        || hi[1] > volume_mm[1] - margin_mm + 1e-6
                        || hi[2] > volume_mm[2] + 1e-6
                })
            })
            .map(|o| o.id)
            .collect()
    }

    /// The pairs of parts whose footprints lie one over the other.
    pub fn overlaps(&self) -> Vec<(u32, u32)> {
        let boxes: Vec<(u32, Bounds)> = self
            .objects
            .iter()
            .filter_map(|o| o.world_bounds().map(|b| (o.id, b)))
            .collect();
        let mut out = Vec::new();
        for (i, (ia, (alo, ahi))) in boxes.iter().enumerate() {
            for (ib, (blo, bhi)) in &boxes[i + 1..] {
                if alo[0] < bhi[0] - 1e-6
                    && blo[0] < ahi[0] - 1e-6
                    && alo[1] < bhi[1] - 1e-6
                    && blo[1] < ahi[1] - 1e-6
                {
                    out.push((*ia, *ib));
                }
            }
        }
        out
    }

    /// Puts the parts in rows on the bed with `gap_mm` between them and `margin_mm` from the edge, the deepest first, and then centres the group. A part that is left over
    /// does not fit in what remains of the bed: it goes to the middle and is named in the answer.
    pub fn arrange(&mut self, bed_mm: [f64; 2], gap_mm: f64, margin_mm: f64) -> Arranged {
        let gap = if gap_mm.is_finite() {
            gap_mm.max(0.0)
        } else {
            0.0
        };
        let margin = if margin_mm.is_finite() {
            margin_mm.max(0.0)
        } else {
            0.0
        };
        let mut sizes: Vec<(u32, f64, f64)> = self
            .objects
            .iter()
            .filter_map(|o| o.world_size().map(|s| (o.id, s[0], s[1])))
            .collect();
        sizes.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        let width = bed_mm[0] - 2.0 * margin;
        let depth = bed_mm[1] - 2.0 * margin;
        let mut result = Arranged::default();
        let mut spots: Vec<(u32, f64, f64)> = Vec::new();
        let (mut x, mut y, mut row_h) = (0.0f64, 0.0f64, 0.0f64);
        for (id, w, d) in &sizes {
            if x > 0.0 && x + w > width + 1e-9 {
                x = 0.0;
                y += row_h + gap;
                row_h = 0.0;
            }
            if *w > width + 1e-9 || y + d > depth + 1e-9 {
                result.left_out.push(*id);
                continue;
            }
            spots.push((*id, x + w / 2.0, y + d / 2.0));
            x += w + gap;
            row_h = row_h.max(*d);
        }
        // the group is centred on the bed
        let used_w = spots
            .iter()
            .map(|(id, cx, _)| cx + sizes.iter().find(|s| s.0 == *id).map_or(0.0, |s| s.1) / 2.0)
            .fold(0.0, f64::max);
        let used_d = spots
            .iter()
            .map(|(id, _, cy)| cy + sizes.iter().find(|s| s.0 == *id).map_or(0.0, |s| s.2) / 2.0)
            .fold(0.0, f64::max);
        let (ox, oy) = (
            margin + (width - used_w) / 2.0,
            margin + (depth - used_d) / 2.0,
        );
        for (id, cx, cy) in &spots {
            if let Some(o) = self.get_mut(*id) {
                o.pos = [ox + cx, oy + cy];
            }
        }
        for id in &result.left_out {
            if let Some(o) = self.get_mut(*id) {
                o.pos = [bed_mm[0] / 2.0, bed_mm[1] / 2.0];
            }
        }
        result.placed = spots.len();
        result
    }

    /// All the parts joined into one mesh, on the bed, for the slicer. An empty bed is an error.
    pub fn merged(&self) -> Result<Mesh, String> {
        if self.objects.is_empty() {
            return Err("there is nothing on the bed".to_string());
        }
        let mut out = Mesh::new();
        for o in &self.objects {
            let m = o.world_mesh()?;
            let base = out.vertices.len() as u32;
            out.vertices.extend(m.vertices);
            out.triangles.extend(
                m.triangles
                    .iter()
                    .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    const BED: [f64; 2] = [220.0, 220.0];

    fn bar(x: f64, y: f64, z: f64) -> Mesh {
        let mut o = PlacedObject::new(1, "bar", cube(1.0));
        o.scale = [x, y, z];
        o.world_mesh().unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn a_part_rests_on_the_bed_with_its_middle_at_the_position() {
        let mut o = PlacedObject::new(1, "c", cube(20.0));
        o.pos = [100.0, 50.0];
        let (lo, hi) = o.world_bounds().unwrap();
        assert!(close(lo[2], 0.0) && close(hi[2], 20.0));
        assert!(close((lo[0] + hi[0]) / 2.0, 100.0) && close((lo[1] + hi[1]) / 2.0, 50.0));
        o.rot_deg = [45.0, 0.0, 0.0];
        let (lo, _) = o.world_bounds().unwrap();
        assert!(close(lo[2], 0.0), "turned, it still rests on the bed");
    }

    #[test]
    fn turning_by_ninety_degrees_swaps_the_axes() {
        let mut o = PlacedObject::new(1, "bar", bar(10.0, 20.0, 30.0));
        let s = o.world_size().unwrap();
        assert!(close(s[0], 10.0) && close(s[1], 20.0) && close(s[2], 30.0));
        assert!(o.set_rotation(2, 90.0));
        let s = o.world_size().unwrap();
        assert!(
            close(s[0], 20.0) && close(s[1], 10.0) && close(s[2], 30.0),
            "{s:?}"
        );
        assert!(o.set_rotation(0, 90.0));
        let s = o.world_size().unwrap();
        assert!(
            close(s[0], 30.0) && close(s[1], 10.0) && close(s[2], 20.0),
            "{s:?}"
        );
        assert!(!o.set_rotation(3, 1.0) && !o.set_rotation(0, f64::NAN));
        assert!(o.set_rotation(1, 9000.0) && o.rot_deg[1] == 360.0);
    }

    #[test]
    fn a_size_in_millimetres_sets_the_scale_alone_or_in_proportion() {
        let mut o = PlacedObject::new(1, "c", cube(10.0));
        assert!(o.set_size_mm(0, 20.0, false));
        assert_eq!(o.scale, [2.0, 1.0, 1.0]);
        assert!(o.set_size_mm(2, 30.0, true));
        let s = o.size_mm().unwrap();
        assert!(
            close(s[0], 60.0) && close(s[1], 30.0) && close(s[2], 30.0),
            "{s:?}"
        );
        let before = o.scale;
        for bad in [0.0, -3.0, f64::NAN, 1e9, 1e-9] {
            assert!(!o.set_size_mm(0, bad, false), "{bad}");
        }
        assert!(!o.set_size_mm(5, 10.0, false));
        assert_eq!(o.scale, before, "a refused size changes nothing");
        assert!(o.set_scale_percent(50.0) && o.scale == [0.5; 3]);
        assert!(!o.set_scale_percent(0.0) && !o.set_scale_percent(1e7));
        o.reset();
        assert_eq!(o.scale, [1.0; 3]);
    }

    #[test]
    fn a_flat_plate_standing_on_its_edge_is_laid_down() {
        let mut o = PlacedObject::new(1, "plate", bar(40.0, 40.0, 2.0));
        o.rot_deg = [90.0, 0.0, 0.0];
        assert!(close(o.world_size().unwrap()[2], 40.0));
        let turn = auto_orient(&o);
        o.rot_deg = turn;
        assert!(close(o.world_size().unwrap()[2], 2.0), "{turn:?}");
    }

    #[test]
    fn a_part_that_is_already_well_placed_keeps_its_turn_and_the_turn_about_z() {
        let mut o = PlacedObject::new(1, "c", cube(20.0));
        o.rot_deg = [0.0, 0.0, 33.0];
        assert_eq!(auto_orient(&o), [0.0, 0.0, 33.0]);
        let s = stance_of(&o).unwrap();
        assert!(
            close(s.overhang_mm2, 0.0) && close(s.contact_mm2, 400.0) && close(s.height_mm, 20.0)
        );
    }

    #[test]
    fn a_mushroom_is_stood_on_its_wide_face() {
        // a wide flat cap with a thin stem: upside down the cap hangs in the air
        let cap = PlacedObject::new(1, "cap", bar(40.0, 40.0, 4.0));
        let stem = PlacedObject::new(2, "stem", bar(6.0, 6.0, 20.0));
        let mut plate = Plate::new();
        let a = plate.add("cap", cap.world_mesh().unwrap(), BED).unwrap();
        let b = plate.add("stem", stem.world_mesh().unwrap(), BED).unwrap();
        plate.get_mut(a).unwrap().pos = [100.0, 100.0];
        plate.get_mut(b).unwrap().pos = [100.0, 100.0];
        let mut m = plate.merged().unwrap();
        // lift the stem under the cap: the cap is the first 4 mm; shift the stem's z up by 4 mm
        let n = cap.mesh.vertices.len();
        for v in m.vertices.iter_mut().skip(n) {
            v.z = Nm::from_mm(v.z.to_mm() + 4.0).unwrap();
        }
        let mut up = PlacedObject::new(9, "m", m);
        // turned upside down the cap would be at the top: the best stance puts the cap on the bed
        up.rot_deg = [180.0, 0.0, 0.0];
        let flipped = stance_of(&up).unwrap();
        up.rot_deg = auto_orient(&up);
        let best = stance_of(&up).unwrap();
        assert!(best.cost() < flipped.cost(), "{best:?} {flipped:?}");
        assert!(best.contact_mm2 >= flipped.contact_mm2);
    }

    #[test]
    fn parts_are_arranged_in_rows_inside_the_bed_with_the_gap() {
        let mut p = Plate::new();
        for i in 0..6 {
            p.add(&format!("c{i}"), cube(40.0), BED).unwrap();
        }
        let r = p.arrange(BED, 5.0, 2.0);
        assert_eq!((r.placed, r.left_out.len()), (6, 0));
        assert!(p.out_of_bounds([220.0, 220.0, 250.0], 2.0).is_empty());
        assert!(p.overlaps().is_empty());
        // the gap between neighbours is kept
        let boxes: Vec<Bounds> = p
            .objects
            .iter()
            .map(|o| o.world_bounds().unwrap())
            .collect();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let dx = (a.0[0] - b.1[0]).max(b.0[0] - a.1[0]);
                let dy = (a.0[1] - b.1[1]).max(b.0[1] - a.1[1]);
                assert!(dx >= 5.0 - 1e-6 || dy >= 5.0 - 1e-6, "{a:?} {b:?}");
            }
        }
        // the group is centred
        let (lo, hi) = p.bounds().unwrap();
        assert!(close((lo[0] + hi[0]) / 2.0, 110.0), "{lo:?} {hi:?}");
    }

    #[test]
    fn what_does_not_fit_is_named_and_put_in_the_middle() {
        let mut p = Plate::new();
        for i in 0..6 {
            p.add(&format!("c{i}"), cube(100.0), BED).unwrap();
        }
        let r = p.arrange(BED, 5.0, 2.0);
        assert_eq!(r.placed, 4);
        assert_eq!(r.left_out.len(), 2);
        let wide = p.add("wide", cube(300.0), BED).unwrap();
        let r = p.arrange(BED, 5.0, 2.0);
        assert!(r.left_out.contains(&wide));
        assert!(!p.out_of_bounds([220.0, 220.0, 250.0], 2.0).is_empty());
        // a gap that is not a number is the same as none
        let mut q = Plate::new();
        q.add("a", cube(10.0), BED).unwrap();
        q.add("b", cube(10.0), BED).unwrap();
        let r = q.arrange(BED, f64::NAN, f64::NAN);
        assert_eq!(r.placed, 2);
    }

    #[test]
    fn the_bed_adds_removes_selects_and_copies() {
        let mut p = Plate::new();
        assert!(p.merged().is_err());
        let a = p.add("a", cube(10.0), BED).unwrap();
        let b = p.add("b", cube(10.0), BED).unwrap();
        assert_eq!(p.selected, Some(b));
        assert!(p.select(a) && !p.select(99));
        let copies = p.duplicate(a, 3);
        assert_eq!((copies.len(), p.len()), (3, 5));
        assert_eq!(p.selected, copies.last().copied());
        assert!(p.remove(a));
        assert!(!p.remove(a));
        assert!(p.selected_object().is_some());
        for id in p.objects.iter().map(|o| o.id).collect::<Vec<_>>() {
            p.remove(id);
        }
        assert!(p.is_empty() && p.selected.is_none());
        // the number of parts is held
        for i in 0..MAX_OBJECTS {
            assert!(p.add(&i.to_string(), cube(1.0), BED).is_some());
        }
        assert!(p.add("one more", cube(1.0), BED).is_none());
        assert!(p.duplicate(1, 5).is_empty());
    }

    #[test]
    fn overlapping_parts_and_parts_over_the_edge_are_found() {
        let mut p = Plate::new();
        let a = p.add("a", cube(20.0), BED).unwrap();
        let b = p.add("b", cube(20.0), BED).unwrap();
        assert_eq!(p.overlaps(), vec![(a, b)]);
        p.get_mut(b).unwrap().pos = [10.0, 110.0];
        assert!(p.overlaps().is_empty());
        assert!(p.out_of_bounds([220.0, 220.0, 250.0], 0.0).is_empty());
        assert_eq!(p.out_of_bounds([220.0, 220.0, 250.0], 5.0), vec![b]);
        assert_eq!(p.out_of_bounds([220.0, 220.0, 10.0], 0.0), vec![a, b]);
    }

    #[test]
    fn the_parts_are_joined_into_one_mesh_that_keeps_every_triangle() {
        let mut p = Plate::new();
        p.add("a", cube(20.0), BED).unwrap();
        let b = p.add("b", cube(10.0), BED).unwrap();
        p.get_mut(b).unwrap().pos = [30.0, 30.0];
        let m = p.merged().unwrap();
        assert_eq!(m.triangles.len(), 24);
        assert!(m
            .triangles
            .iter()
            .all(|t| t.iter().all(|i| (*i as usize) < m.vertices.len())));
        assert!(close(m.volume_mm3(), 8000.0 + 1000.0));
    }

    #[test]
    fn random_placements_never_panic_and_stay_on_the_bed_floor() {
        let mut rng = p3dp_core::rng::Rng::new(7);
        for _ in 0..200 {
            let mut o = PlacedObject::new(1, "c", cube(5.0 + rng.unit() * 40.0));
            o.rot_deg = [
                rng.unit() * 720.0 - 360.0,
                rng.unit() * 720.0 - 360.0,
                rng.unit() * 720.0 - 360.0,
            ];
            o.scale = [0.1 + rng.unit() * 3.0, 0.1 + rng.unit() * 3.0, 0.5];
            o.pos = [rng.unit() * 220.0, rng.unit() * 220.0];
            let (lo, _) = o.world_bounds().unwrap();
            assert!(close(lo[2], 0.0));
            let _ = auto_orient(&o);
        }
    }
}
