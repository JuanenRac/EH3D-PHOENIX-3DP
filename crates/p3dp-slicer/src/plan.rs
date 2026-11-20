//! The plan of a print: for every layer, the paths of the head in the order they are printed (perimeters from the inside out, then the infill), in millimetres on the bed. The
//! plan is the preview by layers (it is data, with no window) and the source of the G-code, so what is shown is what is written.

use crate::config::{Placement, SliceConfig, ThinRegions};
use crate::cut::Cutter;
use crate::error::SliceError;
use crate::geom::{self, inset, inside, scan_lines, Loop, P};
use p3dp_core::machine::Zone;
use p3dp_core::{Dialect, Mesh, TempLimits};
use p3dp_profiles::profile::{PrinterProfile, Technology};

/// The printer the slice is for.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub id: String,
    pub dialect: Dialect,
    pub volume_mm: [f64; 3],
    pub limits: TempLimits,
}

impl Machine {
    /// The machine of a profile. A resin printer takes layers of an image and not G-code, and a profile with no temperature limits gives nothing to keep to: both are refused.
    pub fn from_profile(p: &PrinterProfile) -> Result<Machine, SliceError> {
        if p.technology != Technology::Fdm || p.dialect == Dialect::Msla {
            return Err(SliceError::Machine(format!(
                "{} is not a filament printer: this slicer writes G-code only",
                p.title()
            )));
        }
        let limits = p.limits.ok_or_else(|| {
            SliceError::Machine(format!("{} has no temperature limits in its profile", p.id))
        })?;
        Ok(Machine {
            id: p.id.clone(),
            dialect: p.dialect,
            volume_mm: p.volume_mm,
            limits,
        })
    }
}

/// What a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    ExternalPerimeter,
    Perimeter,
    Infill,
    SolidInfill,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::ExternalPerimeter => "external perimeter",
            Kind::Perimeter => "perimeter",
            Kind::Infill => "infill",
            Kind::SolidInfill => "solid infill",
        }
    }
}

/// A path of the head while it extrudes.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub kind: Kind,
    pub points: Vec<P>,
    /// A closed path comes back to its first point.
    pub closed: bool,
}

impl Path {
    pub fn length_mm(&self) -> f64 {
        if self.closed {
            geom::loop_length(&self.points)
        } else {
            self.points.windows(2).map(|w| geom::dist(w[0], w[1])).sum()
        }
    }
}

/// A layer of the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub index: usize,
    /// The height of its top, from the bed.
    pub z_mm: f64,
    pub height_mm: f64,
    pub paths: Vec<Path>,
}

/// Something the plan says that a person should know.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// A part of the layer is thinner than the perimeters asked for and has fewer of them, or none.
    ThinRegion { layer: usize },
}

/// The plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub machine: Machine,
    pub config: SliceConfig,
    pub layers: Vec<Layer>,
    pub warnings: Vec<Warning>,
    /// How far the model was moved to put it on the bed: x, y, z in millimetres.
    pub shift_mm: [f64; 3],
    /// The volume of the model, for what is printed to be compared with.
    pub model_volume_mm3: f64,
}

/// The heights of the layers: the first, then the height asked for, the last one whatever is left.
fn layer_heights(total: f64, first: f64, each: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut done = 0.0;
    let mut next = first.min(total);
    while total - done > 1e-3 {
        out.push(next);
        done += next;
        next = each.min(total - done);
    }
    if out.is_empty() {
        out.push(total.max(1e-3));
    }
    out
}

/// Makes the plan of a slice. Everything is checked first: the settings, the temperatures against the machine, the mesh (closed, facing outwards) and the fit on the bed. Nothing is
/// changed to make it pass; the first thing that is wrong is the answer.
pub fn plan(mesh: &Mesh, machine: &Machine, config: &SliceConfig) -> Result<Plan, SliceError> {
    config.check()?;
    if machine.dialect == Dialect::Msla {
        return Err(SliceError::Machine(
            "a resin printer takes layers of an image, not G-code".to_string(),
        ));
    }
    machine.limits.check(Zone::Nozzle, config.nozzle_c)?;
    machine.limits.check(Zone::Bed, config.bed_c)?;
    machine.limits.check(Zone::Chamber, config.chamber_c)?;
    if mesh.triangles.is_empty() {
        return Err(SliceError::EmptyMesh);
    }
    let open = mesh.open_edges();
    if open > 0 {
        return Err(SliceError::OpenMesh { open_edges: open });
    }
    let volume = mesh.volume_mm3();
    if volume < 0.0 {
        return Err(SliceError::InvertedMesh);
    }
    if volume < 1e-9 {
        return Err(SliceError::EmptyMesh);
    }
    let (lo, hi) = mesh.bounds().ok_or(SliceError::EmptyMesh)?;
    let (lo, hi) = (lo.mm(), hi.mm());
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let [bx, by, bz] = machine.volume_mm;
    let margin = config.bed_margin_mm;
    if size[2] > bz + 1e-9 {
        return Err(SliceError::DoesNotFit {
            what: format!("it is {:.3} mm high and the printer takes {bz} mm", size[2]),
        });
    }
    let shift = match config.placement {
        Placement::Center => [
            bx / 2.0 - (lo[0] + hi[0]) / 2.0,
            by / 2.0 - (lo[1] + hi[1]) / 2.0,
            -lo[2],
        ],
        Placement::Origin => [0.0, 0.0, -lo[2]],
    };
    for (axis, name, max) in [(0usize, "x", bx), (1, "y", by)] {
        let (a, b) = (lo[axis] + shift[axis], hi[axis] + shift[axis]);
        if a < margin - 1e-9 || b > max - margin + 1e-9 {
            return Err(SliceError::DoesNotFit {
                what: format!(
                    "along {name} it goes from {a:.3} to {b:.3} mm and the bed allows {margin} to {:.3} mm",
                    max - margin
                ),
            });
        }
    }
    let heights = layer_heights(
        size[2],
        config.first_layer_height_mm,
        config.layer_height_mm,
    );
    if heights.len() > config.max_layers as usize {
        return Err(SliceError::TooManyLayers {
            layers: heights.len(),
            max: config.max_layers as usize,
        });
    }
    // the cuts, at the middle of each layer
    let cutter = Cutter::new(mesh, shift);
    let mut outlines: Vec<Vec<Loop>> = Vec::with_capacity(heights.len());
    let mut bottom = 0.0;
    for (i, h) in heights.iter().enumerate() {
        outlines.push(cutter.loops_at(
            bottom + h / 2.0,
            i,
            config.max_points_per_layer as usize,
        )?);
        bottom += h;
    }
    // the paths
    let mut layers = Vec::with_capacity(heights.len());
    let mut warnings = Vec::new();
    let mut top = 0.0;
    let mut head: P = [0.0, 0.0];
    for (i, h) in heights.iter().enumerate() {
        top += h;
        let paths = layer_paths(i, *h, config, &outlines, &mut warnings, &mut head)?;
        layers.push(Layer {
            index: i,
            z_mm: top,
            height_mm: *h,
            paths,
        });
    }
    // nothing may leave the bed (the model fits, and the paths are inside the model; this is the check that says so)
    for l in &layers {
        for p in l.paths.iter().flat_map(|p| &p.points) {
            let ok = p[0] >= margin - 1e-6
                && p[0] <= bx - margin + 1e-6
                && p[1] >= margin - 1e-6
                && p[1] <= by - margin + 1e-6;
            if !ok {
                return Err(SliceError::DoesNotFit {
                    what: format!(
                        "a path of layer {} leaves the bed at ({:.3}, {:.3})",
                        l.index, p[0], p[1]
                    ),
                });
            }
        }
    }
    Ok(Plan {
        machine: machine.clone(),
        config: config.clone(),
        layers,
        warnings,
        shift_mm: shift,
        model_volume_mm3: volume,
    })
}

/// The point of a closed loop nearest to a place, as the index to start from.
fn nearest_vertex(l: &[P], at: P) -> usize {
    let mut best = (0usize, f64::MAX);
    for (i, p) in l.iter().enumerate() {
        let d = geom::dist(*p, at);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

fn layer_paths(
    index: usize,
    h: f64,
    cfg: &SliceConfig,
    outlines: &[Vec<Loop>],
    warnings: &mut Vec<Warning>,
    head: &mut P,
) -> Result<Vec<Path>, SliceError> {
    let w = cfg.line_width_mm;
    let s = cfg.spacing_mm(h);
    let outline = &outlines[index];
    let mut paths: Vec<Path> = Vec::new();
    let mut thin = false;
    // the perimeters, from the inside out
    let mut rings: Vec<Vec<Loop>> = Vec::new();
    for k in 0..cfg.perimeters {
        let d = w / 2.0 + f64::from(k) * s;
        let mut ring: Vec<Loop> = Vec::new();
        for l in outline {
            let moved = inset(l, d);
            if moved.is_empty() {
                thin = true;
            }
            ring.extend(moved);
        }
        rings.push(ring);
    }
    if thin {
        if cfg.thin_regions == ThinRegions::Refuse {
            return Err(SliceError::ThinRegion { layer: index });
        }
        warnings.push(Warning::ThinRegion { layer: index });
    }
    for (k, ring) in rings.iter().enumerate().rev() {
        let kind = if k == 0 {
            Kind::ExternalPerimeter
        } else {
            Kind::Perimeter
        };
        for l in ring {
            let start = nearest_vertex(l, *head);
            let mut pts: Vec<P> = l[start..].to_vec();
            pts.extend_from_slice(&l[..start]);
            *head = pts[0];
            paths.push(Path {
                kind,
                points: pts,
                closed: true,
            });
        }
    }
    // the infill, inside the innermost perimeter
    let d_region = w / 2.0 + f64::from(cfg.perimeters.saturating_sub(1)) * s + s / 2.0;
    let region: Vec<Loop> = outline.iter().flat_map(|l| inset(l, d_region)).collect();
    if !region.is_empty() {
        paths.extend(infill_paths(index, h, cfg, outlines, &region, head));
    }
    Ok(paths)
}

/// Whether every one of `count` layers on one side of layer `index` covers the point.
fn covered(p: P, index: usize, count: u32, up: bool, outlines: &[Vec<Loop>]) -> bool {
    (1..=count as usize).all(|j| {
        let k = if up {
            index.checked_add(j)
        } else {
            index.checked_sub(j)
        };
        k.and_then(|k| outlines.get(k))
            .is_some_and(|o| inside(p, o))
    })
}

fn infill_paths(
    index: usize,
    h: f64,
    cfg: &SliceConfig,
    outlines: &[Vec<Loop>],
    region: &[Loop],
    head: &mut P,
) -> Vec<Path> {
    let s = cfg.spacing_mm(h);
    let density = cfg.infill_percent / 100.0;
    let horizontal = index.is_multiple_of(2);
    let min_len = (cfg.line_width_mm * 0.5).max(0.2);
    let all_solid = density >= 0.999;
    let needs_skin = |p: P| {
        (cfg.top_layers > 0 && !covered(p, index, cfg.top_layers, true, outlines))
            || (cfg.bottom_layers > 0 && !covered(p, index, cfg.bottom_layers, false, outlines))
    };
    // the lines are cut in short pieces and each piece is judged by its middle
    let piece = (4.0 * s).max(1.0);
    let mut segments: Vec<(Kind, P, P)> = Vec::new();
    let mut keep = |lines: Vec<(P, P)>, kind: Kind, want_skin: bool| {
        for (a, b) in lines {
            let len = geom::dist(a, b);
            let n = ((len / piece).ceil() as usize).max(1);
            let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let mut run: Option<P> = None;
            for c in 0..n {
                let (t0, t1) = (c as f64 / n as f64, (c + 1) as f64 / n as f64);
                let mid = at((t0 + t1) / 2.0);
                let wanted = all_solid || needs_skin(mid) == want_skin;
                match (wanted, run) {
                    (true, None) => run = Some(at(t0)),
                    (false, Some(from)) => {
                        segments.push((kind, from, at(t0)));
                        run = None;
                    }
                    _ => {}
                }
            }
            if let Some(from) = run {
                segments.push((kind, from, b));
            }
        }
    };
    keep(
        scan_lines(region, horizontal, s, min_len),
        Kind::SolidInfill,
        true,
    );
    if !all_solid && density > 0.0 {
        keep(
            scan_lines(region, horizontal, s / density, min_len),
            Kind::Infill,
            false,
        );
    }
    // the order: always to the nearest end that is left, so the head does not cross the layer for every line
    let mut paths = Vec::with_capacity(segments.len());
    let mut left = segments;
    while !left.is_empty() {
        let mut best = (0usize, false, f64::MAX);
        for (i, (_, a, b)) in left.iter().enumerate() {
            let (da, db) = (geom::dist(*head, *a), geom::dist(*head, *b));
            if da < best.2 {
                best = (i, false, da);
            }
            if db < best.2 {
                best = (i, true, db);
            }
        }
        let (kind, a, b) = left.swap_remove(best.0);
        let (from, to) = if best.1 { (b, a) } else { (a, b) };
        *head = to;
        paths.push(Path {
            kind,
            points: vec![from, to],
            closed: false,
        });
    }
    paths
}

impl Plan {
    /// The plastic the plan lays down, in cubic millimetres (the length of every path times the section of its line).
    pub fn extrusion_mm3(&self) -> f64 {
        self.layers
            .iter()
            .map(|l| {
                let a = self.config.line_area_mm2(l.height_mm) * self.config.extrusion_multiplier;
                l.paths.iter().map(|p| p.length_mm() * a).sum::<f64>()
            })
            .sum()
    }

    /// The length of filament that goes through the extruder, in millimetres.
    pub fn filament_mm(&self) -> f64 {
        self.extrusion_mm3() / self.config.filament_area_mm2()
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// The height of the print.
    pub fn height_mm(&self) -> f64 {
        self.layers.last().map_or(0.0, |l| l.z_mm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    pub(crate) fn machine() -> Machine {
        Machine {
            id: "test-printer".to_string(),
            dialect: Dialect::Marlin,
            volume_mm: [220.0, 220.0, 250.0],
            limits: TempLimits {
                nozzle_c: 300.0,
                bed_c: 110.0,
                chamber_c: None,
            },
        }
    }

    fn solid() -> SliceConfig {
        SliceConfig {
            infill_percent: 100.0,
            ..SliceConfig::default()
        }
    }

    #[test]
    fn the_heights_add_up_to_the_model() {
        let h = layer_heights(20.0, 0.2, 0.2);
        assert_eq!(h.len(), 100);
        assert!((h.iter().sum::<f64>() - 20.0).abs() < 1e-9);
        let h = layer_heights(10.1, 0.3, 0.2);
        assert!((h.iter().sum::<f64>() - 10.1).abs() < 1e-9);
        assert_eq!(h[0], 0.3);
        assert!(h.iter().all(|x| *x > 0.0 && *x <= 0.3 + 1e-12));
        // a model lower than the first layer is one layer, as high as the model
        assert_eq!(layer_heights(0.1, 0.2, 0.2), vec![0.1]);
    }

    #[test]
    fn a_cube_gets_the_layers_the_perimeters_and_the_infill_it_should() {
        let p = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        assert_eq!(p.layer_count(), 100);
        assert!((p.height_mm() - 20.0).abs() < 1e-9);
        assert!(p.warnings.is_empty());
        // the middle layer: two perimeters (the outer one is the external) and sparse infill
        let mid = &p.layers[50];
        let kinds = |k: Kind| mid.paths.iter().filter(|x| x.kind == k).count();
        assert_eq!(
            (kinds(Kind::ExternalPerimeter), kinds(Kind::Perimeter)),
            (1, 1)
        );
        assert!(kinds(Kind::Infill) > 0 && kinds(Kind::SolidInfill) == 0);
        // the first layers are solid, and so are the last ones
        for i in [0usize, 3, 96, 99] {
            let l = &p.layers[i];
            assert!(
                l.paths.iter().any(|x| x.kind == Kind::SolidInfill),
                "layer {i}"
            );
            assert!(!l.paths.iter().any(|x| x.kind == Kind::Infill), "layer {i}");
        }
        assert!(p.layers[4].paths.iter().any(|x| x.kind == Kind::Infill));
        // the cube is centred on the bed
        let xs: Vec<f64> = mid
            .paths
            .iter()
            .flat_map(|x| x.points.iter().map(|q| q[0]))
            .collect();
        let (lo, hi) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        assert!(
            (lo - 100.0 - 0.225).abs() < 1e-6 && (hi - 120.0 + 0.225).abs() < 1e-6,
            "{lo} {hi}"
        );
        assert_eq!(p.shift_mm, [100.0, 100.0, 0.0]);
    }

    #[test]
    fn a_solid_cube_lays_down_the_plastic_of_its_volume() {
        let p = plan(&cube(20.0), &machine(), &solid()).unwrap();
        let ratio = p.extrusion_mm3() / p.model_volume_mm3;
        assert!((0.97..=1.03).contains(&ratio), "{ratio}");
        // and less infill is less plastic
        let sparse = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        assert!(sparse.extrusion_mm3() < p.extrusion_mm3() * 0.6);
        let hollow = plan(
            &cube(20.0),
            &machine(),
            &SliceConfig {
                infill_percent: 0.0,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        assert!(hollow.extrusion_mm3() < sparse.extrusion_mm3());
        assert!(hollow.layers[50].paths.iter().all(|x| x.closed));
    }

    #[test]
    fn the_orientation_of_the_infill_changes_with_the_layer() {
        let p = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        let dir = |l: &Layer| {
            let seg = l.paths.iter().find(|x| x.kind == Kind::Infill).unwrap();
            (
                seg.points[0][0] == seg.points[1][0],
                seg.points[0][1] == seg.points[1][1],
            )
        };
        assert_eq!(dir(&p.layers[50]), (false, true));
        assert_eq!(dir(&p.layers[51]), (true, false));
    }

    #[test]
    fn what_cannot_be_printed_is_refused_and_never_changed() {
        let m = machine();
        let c = SliceConfig::default();
        // a temperature over the limit of the machine
        let hot = SliceConfig {
            nozzle_c: 301.0,
            ..c.clone()
        };
        assert!(matches!(
            plan(&cube(10.0), &m, &hot),
            Err(SliceError::Temp(_))
        ));
        let bed = SliceConfig {
            bed_c: 111.0,
            ..c.clone()
        };
        assert!(matches!(
            plan(&cube(10.0), &m, &bed),
            Err(SliceError::Temp(_))
        ));
        let chamber = SliceConfig {
            chamber_c: 40.0,
            ..c.clone()
        };
        assert!(
            matches!(plan(&cube(10.0), &m, &chamber), Err(SliceError::Temp(_))),
            "a machine with no heated chamber"
        );
        // bigger than the bed, higher than the printer
        assert!(matches!(
            plan(&cube(230.0), &m, &c),
            Err(SliceError::DoesNotFit { .. })
        ));
        let tall = Machine {
            volume_mm: [220.0, 220.0, 15.0],
            ..m.clone()
        };
        assert!(matches!(
            plan(&cube(20.0), &tall, &c),
            Err(SliceError::DoesNotFit { .. })
        ));
        // with a margin that the model does not leave
        let margin = SliceConfig {
            bed_margin_mm: 6.0,
            ..c.clone()
        };
        assert!(plan(&cube(210.0), &m, &margin).is_err());
        // a mesh that is not closed, empty or inside out
        let mut open = cube(10.0);
        open.triangles.pop();
        assert!(matches!(
            plan(&open, &m, &c),
            Err(SliceError::OpenMesh { .. })
        ));
        assert!(matches!(
            plan(&Mesh::new(), &m, &c),
            Err(SliceError::EmptyMesh)
        ));
        let mut inverted = cube(10.0);
        for t in &mut inverted.triangles {
            t.swap(1, 2);
        }
        assert!(matches!(
            plan(&inverted, &m, &c),
            Err(SliceError::InvertedMesh)
        ));
        // a resin printer
        let resin = Machine {
            dialect: Dialect::Msla,
            ..m.clone()
        };
        assert!(matches!(
            plan(&cube(10.0), &resin, &c),
            Err(SliceError::Machine(_))
        ));
        // too many layers
        let few = SliceConfig {
            max_layers: 10,
            ..c.clone()
        };
        assert!(matches!(
            plan(&cube(20.0), &m, &few),
            Err(SliceError::TooManyLayers {
                layers: 100,
                max: 10
            })
        ));
        // settings that do not go together
        let wide = SliceConfig {
            layer_height_mm: 0.5,
            ..c
        };
        assert!(matches!(
            plan(&cube(10.0), &m, &wide),
            Err(SliceError::Config(_))
        ));
    }

    #[test]
    fn a_part_thinner_than_the_perimeters_is_skipped_with_a_warning_or_refused() {
        // a plate 0.6 mm wide cannot hold a perimeter of 0.45 mm and the next one
        let mut soup = Vec::new();
        let w = 0.6;
        let b = cube(1.0);
        for t in &b.triangles {
            let p = |i: u32| {
                let v = b.vertices[i as usize].mm();
                [v[0] * 10.0, v[1] * w, v[2] * 5.0]
            };
            soup.push([p(t[0]), p(t[1]), p(t[2])]);
        }
        let mesh = Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap();
        let skip = plan(&mesh, &machine(), &SliceConfig::default()).unwrap();
        assert!(skip.warnings.contains(&Warning::ThinRegion { layer: 0 }));
        assert!(skip.layers[0]
            .paths
            .iter()
            .all(|p| p.kind == Kind::ExternalPerimeter));
        assert!(
            !skip.layers[0].paths.is_empty(),
            "the outer perimeter still fits"
        );
        let refuse = SliceConfig {
            thin_regions: ThinRegions::Refuse,
            ..SliceConfig::default()
        };
        assert!(matches!(
            plan(&mesh, &machine(), &refuse),
            Err(SliceError::ThinRegion { layer: 0 })
        ));
    }

    #[test]
    fn the_origin_placement_keeps_the_coordinates_of_the_model() {
        let c = SliceConfig {
            placement: Placement::Origin,
            ..SliceConfig::default()
        };
        let p = plan(&cube(10.0), &machine(), &c).unwrap();
        assert_eq!(p.shift_mm, [0.0, 0.0, 0.0]);
        // a model whose corner is below zero does not fit when it is not moved
        let mut m = cube(10.0);
        for v in &mut m.vertices {
            v.x = v.x - p3dp_core::Nm::from_mm(5.0).unwrap();
        }
        assert!(matches!(
            plan(&m, &machine(), &c),
            Err(SliceError::DoesNotFit { .. })
        ));
        assert!(plan(&m, &machine(), &SliceConfig::default()).is_ok());
    }
}
