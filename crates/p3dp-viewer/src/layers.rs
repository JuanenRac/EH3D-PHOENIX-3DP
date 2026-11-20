//! The layers of a plan in the viewer: the range of layers that is shown (the two handles of a slider), and the lines of those layers as plain arrays of numbers that a graphics card
//! takes as they are, each kind of path in its own colour. The colours are the program's own.

use p3dp_slicer::{Kind, Plan};

/// The layers that are shown: from `first` to `last`, both counted from 0 and both shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerRange {
    count: usize,
    first: usize,
    last: usize,
}

impl LayerRange {
    /// A range over `count` layers that shows all of them; `None` for no layers.
    pub fn new(count: usize) -> Option<LayerRange> {
        (count > 0).then(|| LayerRange {
            count,
            first: 0,
            last: count - 1,
        })
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn first(&self) -> usize {
        self.first
    }

    pub fn last(&self) -> usize {
        self.last
    }

    /// Moves the low handle; it never goes above the high one or past the last layer.
    pub fn set_first(&mut self, layer: usize) {
        self.first = layer.min(self.last);
    }

    /// Moves the high handle; it never goes below the low one or past the last layer.
    pub fn set_last(&mut self, layer: usize) {
        self.last = layer.clamp(self.first, self.count - 1);
    }

    /// Moves both handles by `steps` layers (up when positive, down when negative), keeping the width of the range and stopping at the ends.
    pub fn shift(&mut self, steps: i64) {
        let width = self.last - self.first;
        let max_first = (self.count - 1 - width) as i64;
        let first = (self.first as i64 + steps).clamp(0, max_first);
        self.first = first as usize;
        self.last = self.first + width;
    }

    /// Shows one layer only (the one a slider is on).
    pub fn only(&mut self, layer: usize) {
        let l = layer.min(self.count - 1);
        self.first = l;
        self.last = l;
    }

    /// Shows all of them again.
    pub fn all(&mut self) {
        self.first = 0;
        self.last = self.count - 1;
    }

    /// Whether a layer is shown.
    pub fn shows(&self, layer: usize) -> bool {
        layer >= self.first && layer <= self.last
    }

    /// How many layers are shown.
    pub fn shown(&self) -> usize {
        self.last - self.first + 1
    }

    /// The same range over another number of layers (a new slice): the handles are kept where they can be.
    pub fn resized(&self, count: usize) -> Option<LayerRange> {
        let mut r = LayerRange::new(count)?;
        r.set_last(self.last.min(count - 1));
        r.set_first(self.first.min(count - 1));
        Some(r)
    }
}

/// The colour of each kind of path, red green blue from 0 to 1.
pub fn colour_of(kind: Kind) -> [f32; 3] {
    match kind {
        Kind::ExternalPerimeter => [0.96, 0.62, 0.12],
        Kind::Perimeter => [0.82, 0.45, 0.10],
        Kind::Infill => [0.80, 0.25, 0.25],
        Kind::SolidInfill => [0.62, 0.20, 0.62],
        Kind::Bridge => [0.15, 0.62, 0.78],
        Kind::Support => [0.30, 0.72, 0.38],
        Kind::Adhesion => [0.90, 0.85, 0.30],
        Kind::Raft => [0.55, 0.55, 0.60],
    }
}

/// Lines ready for a graphics card: for each vertex three numbers of position (x, y, z in millimetres) and three of colour; every two vertices make a line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LineBatch {
    pub vertices: Vec<f32>,
}

/// The numbers of one vertex in a batch.
pub const FLOATS_PER_VERTEX: usize = 6;

impl LineBatch {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / FLOATS_PER_VERTEX
    }

    pub fn line_count(&self) -> usize {
        self.vertex_count() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// The batch as bytes in the order of the machine of the graphics card (little endian), for writing into a buffer.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.vertices.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn push(&mut self, a: [f64; 3], b: [f64; 3], colour: [f32; 3]) {
        for p in [a, b] {
            self.vertices
                .extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
            self.vertices.extend_from_slice(&colour);
        }
    }
}

/// Which kinds of path are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindFilter {
    pub perimeters: bool,
    pub infill: bool,
    pub solid: bool,
    pub bridges: bool,
    pub supports: bool,
    /// The skirt, the brim and the raft.
    pub adhesion: bool,
}

impl Default for KindFilter {
    fn default() -> KindFilter {
        KindFilter {
            perimeters: true,
            infill: true,
            solid: true,
            bridges: true,
            supports: true,
            adhesion: true,
        }
    }
}

impl KindFilter {
    pub fn allows(&self, kind: Kind) -> bool {
        match kind {
            Kind::ExternalPerimeter | Kind::Perimeter => self.perimeters,
            Kind::Infill => self.infill,
            Kind::SolidInfill => self.solid,
            Kind::Bridge => self.bridges,
            Kind::Support => self.supports,
            Kind::Adhesion | Kind::Raft => self.adhesion,
        }
    }
}

/// The lines of the layers in a range, from the paths of a plan: each path is a run of lines at the height of its layer (a closed path comes back to its first point).
pub fn lines_of(plan: &Plan, range: &LayerRange, filter: &KindFilter) -> LineBatch {
    let mut batch = LineBatch::default();
    for layer in plan.layers.iter().filter(|l| range.shows(l.index)) {
        for path in layer.paths.iter().filter(|p| filter.allows(p.kind)) {
            let colour = colour_of(path.kind);
            let z = layer.z_mm;
            for w in path.points.windows(2) {
                batch.push([w[0][0], w[0][1], z], [w[1][0], w[1][1], z], colour);
            }
            if path.closed && path.points.len() > 2 {
                let (a, b) = (path.points[path.points.len() - 1], path.points[0]);
                batch.push([a[0], a[1], z], [b[0], b[1], z], colour);
            }
        }
    }
    batch
}

/// The colour of the travel moves: a pale blue, apart from every kind of path.
pub const TRAVEL_COLOUR: [f32; 3] = [0.35, 0.65, 0.95];

/// The moves of the head between one path and the next, in the layers of a range: a line from where a path ends to where the next one starts, at the height of the layer. A closed
/// path ends where it began. The first path of a layer has no line before it (the head comes up from the layer below), and a move of no length is not drawn. The kinds filter does not
/// apply: the head travels whether or not the path it comes from is shown.
pub fn travel_lines(plan: &Plan, range: &LayerRange) -> LineBatch {
    let mut batch = LineBatch::default();
    for layer in plan.layers.iter().filter(|l| range.shows(l.index)) {
        let z = layer.z_mm;
        let end_of = |p: &p3dp_slicer::Path| {
            if p.closed {
                p.points.first().copied()
            } else {
                p.points.last().copied()
            }
        };
        for pair in layer.paths.windows(2) {
            let (Some(from), Some(to)) = (end_of(&pair[0]), pair[1].points.first().copied()) else {
                continue;
            };
            if (from[0] - to[0]).abs() > 1e-9 || (from[1] - to[1]).abs() > 1e-9 {
                batch.push([from[0], from[1], z], [to[0], to[1], z], TRAVEL_COLOUR);
            }
        }
    }
    batch
}

/// The lines of the outline of the plate on the bed, in the colour of a guide.
pub fn plate_lines(size_x_mm: f64, size_y_mm: f64) -> LineBatch {
    let mut batch = LineBatch::default();
    let colour = [0.55, 0.55, 0.60];
    let c = [
        [0.0, 0.0, 0.0],
        [size_x_mm, 0.0, 0.0],
        [size_x_mm, size_y_mm, 0.0],
        [0.0, size_y_mm, 0.0],
    ];
    for i in 0..4 {
        batch.push(c[i], c[(i + 1) % 4], colour);
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::{Dialect, TempLimits};
    use p3dp_slicer::{plan, Machine, SliceConfig, Supports};

    fn machine() -> Machine {
        Machine {
            id: "viewer-test".to_string(),
            dialect: Dialect::Marlin,
            volume_mm: [220.0, 220.0, 250.0],
            limits: TempLimits {
                nozzle_c: 300.0,
                bed_c: 110.0,
                chamber_c: None,
            },
        }
    }

    #[test]
    fn the_handles_of_the_range_keep_their_order_and_stay_on_the_layers() {
        assert!(LayerRange::new(0).is_none());
        let mut r = LayerRange::new(100).unwrap();
        assert_eq!(
            (r.first(), r.last(), r.shown(), r.count()),
            (0, 99, 100, 100)
        );
        r.set_first(30);
        r.set_last(60);
        assert_eq!((r.first(), r.last(), r.shown()), (30, 60, 31));
        r.set_first(80);
        assert_eq!(
            (r.first(), r.last()),
            (60, 60),
            "the low handle stops at the high one"
        );
        r.set_last(10);
        assert_eq!(
            (r.first(), r.last()),
            (60, 60),
            "the high handle stops at the low one"
        );
        r.set_last(5000);
        assert_eq!(r.last(), 99);
        r.all();
        r.set_first(20);
        r.set_last(40);
        r.shift(10);
        assert_eq!((r.first(), r.last()), (30, 50));
        r.shift(-1000);
        assert_eq!((r.first(), r.last()), (0, 20));
        r.shift(1000);
        assert_eq!((r.first(), r.last()), (79, 99));
        r.only(500);
        assert_eq!((r.first(), r.last()), (99, 99));
        assert!(r.shows(99) && !r.shows(98));
        r.all();
        assert_eq!(r.shown(), 100);
    }

    #[test]
    fn a_range_over_a_new_slice_keeps_the_handles_where_it_can() {
        let mut r = LayerRange::new(100).unwrap();
        r.set_first(10);
        r.set_last(90);
        let s = r.resized(50).unwrap();
        assert_eq!((s.first(), s.last(), s.count()), (10, 49, 50));
        let t = r.resized(200).unwrap();
        assert_eq!((t.first(), t.last()), (10, 90));
        assert!(r.resized(0).is_none());
    }

    #[test]
    fn the_lines_of_a_plan_are_those_of_the_layers_in_the_range_and_the_kinds_asked() {
        let p = plan(&cube(10.0), &machine(), &SliceConfig::default()).unwrap();
        let all = LayerRange::new(p.layer_count()).unwrap();
        let every = lines_of(&p, &all, &KindFilter::default());
        // the same number of lines as the paths hold
        let expect: usize = p
            .layers
            .iter()
            .flat_map(|l| &l.paths)
            .map(|x| x.points.len().saturating_sub(1) + usize::from(x.closed && x.points.len() > 2))
            .sum();
        assert!(expect > 100);
        assert_eq!(every.line_count(), expect);
        assert_eq!(
            every.vertices.len(),
            every.vertex_count() * FLOATS_PER_VERTEX
        );
        // a range of one layer has only the lines of that layer, at its height
        let mut one = LayerRange::new(p.layer_count()).unwrap();
        one.only(7);
        let b = lines_of(&p, &one, &KindFilter::default());
        assert!(b.line_count() > 0 && b.line_count() < every.line_count());
        let z = p.layers[7].z_mm as f32;
        assert!(b
            .vertices
            .chunks(FLOATS_PER_VERTEX)
            .all(|v| (v[2] - z).abs() < 1e-6));
        // without the perimeters only the fill is left, in its colours
        let fill_only = lines_of(
            &p,
            &all,
            &KindFilter {
                perimeters: false,
                ..KindFilter::default()
            },
        );
        assert!(fill_only.line_count() < every.line_count() && fill_only.line_count() > 0);
        let allowed = [colour_of(Kind::Infill), colour_of(Kind::SolidInfill)];
        assert!(fill_only
            .vertices
            .chunks(FLOATS_PER_VERTEX)
            .all(|v| allowed.iter().any(|c| v[3..6] == c[..])));
        let nothing = KindFilter {
            perimeters: false,
            infill: false,
            solid: false,
            bridges: false,
            supports: false,
            adhesion: false,
        };
        assert!(lines_of(&p, &all, &nothing).is_empty());
    }

    #[test]
    fn every_line_is_inside_the_bed_and_the_kinds_have_their_own_colours() {
        let cfg = SliceConfig {
            supports: Supports::Off,
            ..SliceConfig::default()
        };
        let p = plan(&cube(20.0), &machine(), &cfg).unwrap();
        let b = lines_of(
            &p,
            &LayerRange::new(p.layer_count()).unwrap(),
            &KindFilter::default(),
        );
        for v in b.vertices.chunks(FLOATS_PER_VERTEX) {
            assert!(
                v[0] >= 0.0 && v[0] <= 220.0 && v[1] >= 0.0 && v[1] <= 220.0,
                "{v:?}"
            );
            assert!(v.iter().all(|x| x.is_finite()));
            assert!(v[3..6].iter().all(|c| (0.0..=1.0).contains(c)));
        }
        let kinds = [
            Kind::ExternalPerimeter,
            Kind::Perimeter,
            Kind::Infill,
            Kind::SolidInfill,
            Kind::Bridge,
            Kind::Support,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for c in &kinds[i + 1..] {
                assert_ne!(colour_of(*a), colour_of(*c), "{a:?} and {c:?}");
            }
        }
    }

    #[test]
    fn the_travel_lines_join_the_end_of_each_path_to_the_start_of_the_next_inside_the_range() {
        let p = plan(&cube(10.0), &machine(), &SliceConfig::default()).unwrap();
        let all = LayerRange::new(p.layer_count()).unwrap();
        let t = travel_lines(&p, &all);
        assert!(t.line_count() > 10, "{}", t.line_count());
        // never more lines than gaps between paths, and every line has the colour of a travel
        let gaps: usize = p
            .layers
            .iter()
            .map(|l| l.paths.len().saturating_sub(1))
            .sum();
        assert!(t.line_count() <= gaps);
        for v in t.vertices.chunks(FLOATS_PER_VERTEX) {
            assert_eq!(&v[3..], &TRAVEL_COLOUR);
        }
        // one layer: its lines are at its height and no other
        let mut one = all;
        one.only(2);
        let z = p.layers[2].z_mm as f32;
        let t1 = travel_lines(&p, &one);
        assert!(t1.line_count() > 0 && t1.line_count() < t.line_count());
        assert!(t1
            .vertices
            .chunks(FLOATS_PER_VERTEX)
            .all(|v| (v[2] - z).abs() < 1e-4));
        // the first line of the layer starts where the first path ends
        let first = &p.layers[2].paths[0];
        let end = if first.closed {
            first.points[0]
        } else {
            *first.points.last().unwrap()
        };
        let second = p.layers[2].paths[1].points[0];
        if (end[0] - second[0]).abs() > 1e-9 || (end[1] - second[1]).abs() > 1e-9 {
            assert_eq!(
                (t1.vertices[0], t1.vertices[1]),
                (end[0] as f32, end[1] as f32)
            );
        }
    }

    #[test]
    fn a_plan_with_one_path_per_layer_has_no_travel_and_the_travel_ignores_the_kinds_filter() {
        let mut p = plan(&cube(10.0), &machine(), &SliceConfig::default()).unwrap();
        let all = LayerRange::new(p.layer_count()).unwrap();
        let with = travel_lines(&p, &all).line_count();
        for l in &mut p.layers {
            l.paths.truncate(1);
        }
        assert!(travel_lines(&p, &all).is_empty());
        assert!(with > 0);
        // a path with no points is skipped, not a panic
        p.layers[0].paths.push(p3dp_slicer::Path {
            kind: Kind::Infill,
            points: vec![],
            closed: false,
            width_mm: 0.4,
        });
        assert!(travel_lines(&p, &all).is_empty());
    }

    #[test]
    fn the_bytes_of_a_batch_are_its_numbers_and_the_plate_is_a_rectangle() {
        let plate = plate_lines(200.0, 100.0);
        assert_eq!(plate.line_count(), 4);
        let bytes = plate.to_bytes();
        assert_eq!(bytes.len(), plate.vertices.len() * 4);
        assert_eq!(
            f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            plate.vertices[0]
        );
        let xs: Vec<f32> = plate
            .vertices
            .chunks(FLOATS_PER_VERTEX)
            .map(|v| v[0])
            .collect();
        assert!(xs.contains(&200.0) && xs.contains(&0.0));
        assert!(LineBatch::default().is_empty() && LineBatch::default().to_bytes().is_empty());
    }
}
