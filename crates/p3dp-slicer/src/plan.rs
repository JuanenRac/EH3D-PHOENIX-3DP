//! The plan of a print: for every layer, the paths of the head in the order they are printed (supports, perimeters from the inside out, then the infill), in millimetres on the bed.
//! The plan is the preview by layers (it is data, with no window) and the source of the G-code, so what is shown is what is written.

use crate::adhesion;
use crate::config::{Adhesion, Placement, Seam, SliceConfig, ThinRegions};
use crate::cut::Cutter;
use crate::error::SliceError;
use crate::geom::{self, inset, Loop, P};
use crate::infill::{fill_paths, order_paths, Fill};
use crate::support::support_paths;
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
    /// A solid skin over nothing: it is printed slower, with the fan on and a little less plastic.
    Bridge,
    Support,
    /// A loop of a skirt or of a brim.
    Adhesion,
    /// A line of a raft.
    Raft,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::ExternalPerimeter => "external perimeter",
            Kind::Perimeter => "perimeter",
            Kind::Infill => "infill",
            Kind::SolidInfill => "solid infill",
            Kind::Bridge => "bridge",
            Kind::Support => "support",
            Kind::Adhesion => "adhesion",
            Kind::Raft => "raft",
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
    /// The width of the line, in millimetres (the perimeters of a thin part may be narrower than the others).
    pub width_mm: f64,
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
    /// The layers of the raft under the model (0 when there is none): they are the first layers of the plan.
    pub raft_layers: usize,
    /// The slowing down of each layer that would take less than `min_layer_time_s`: 1 is no change.
    pub layer_scales: Vec<f64>,
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
    let raft = if config.adhesion == Adhesion::Raft {
        config.raft_layers as usize
    } else {
        0
    };
    let raft_heights: Vec<f64> = (0..raft)
        .map(|k| {
            if k == 0 {
                config.first_layer_height_mm
            } else {
                config.layer_height_mm
            }
        })
        .collect();
    let raft_total = if raft > 0 {
        raft_heights.iter().sum::<f64>() + config.raft_air_gap_mm
    } else {
        0.0
    };
    if size[2] + raft_total > bz + 1e-9 {
        return Err(SliceError::DoesNotFit {
            what: format!(
                "with the raft it is {:.3} mm high and the printer takes {bz} mm",
                size[2] + raft_total
            ),
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
    if config.z_hop_mm > 0.0 && size[2] + config.z_hop_mm > bz + 1e-9 {
        return Err(SliceError::LiftPastTop {
            needed_mm: size[2] + config.z_hop_mm,
            height_mm: bz,
        });
    }
    let heights = layer_heights(
        size[2],
        config.first_layer_height_mm,
        config.layer_height_mm,
    );
    if heights.len() + raft > config.max_layers as usize {
        return Err(SliceError::TooManyLayers {
            layers: heights.len() + raft,
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
    // the supports, decided on the whole stack of cuts
    let supports = support_paths(&outlines, &heights, config)?;
    // the paths
    let mut layers = Vec::with_capacity(heights.len() + raft);
    let mut warnings = Vec::new();
    let mut top = 0.0;
    let mut state = State {
        head: [0.0, 0.0],
        seams: Vec::new(),
    };
    // the raft first: the part stands on it
    let mut raft_top = 0.0;
    for (k, h) in raft_heights.iter().enumerate() {
        raft_top += h;
        layers.push(Layer {
            index: k,
            z_mm: raft_top,
            height_mm: *h,
            paths: adhesion::raft_paths(&outlines[0], config, k, raft, *h, &mut state.head),
        });
    }
    for (i, h) in heights.iter().enumerate() {
        top += h;
        if config.vase_mode && i >= config.bottom_layers as usize && outlines[i].len() != 1 {
            return Err(SliceError::VaseNeedsOneOutline {
                layer: i,
                outlines: outlines[i].len(),
            });
        }
        let mut lead = if i == 0 {
            match config.adhesion {
                Adhesion::Skirt => adhesion::skirt_paths(&outlines[0], config, *h, &mut state.head),
                Adhesion::Brim => adhesion::brim_paths(&outlines[0], config, *h, &mut state.head),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let paths = layer_paths(
            LayerInput {
                index: i,
                height: *h,
                top,
                supports: &supports[i],
            },
            config,
            &outlines,
            &mut warnings,
            &mut state,
        )?;
        lead.extend(paths);
        layers.push(Layer {
            index: raft + i,
            z_mm: top + raft_total,
            height_mm: *h,
            paths: lead,
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
    let mut plan = Plan {
        machine: machine.clone(),
        config: config.clone(),
        layers,
        warnings,
        shift_mm: shift,
        model_volume_mm3: volume,
        raft_layers: raft,
        layer_scales: Vec::new(),
    };
    plan.layer_scales = plan.compute_layer_scales();
    Ok(plan)
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

/// The vertex farthest back: the highest y, and the lowest x among those.
fn rear_vertex(l: &[P]) -> usize {
    let mut best = 0usize;
    for (i, p) in l.iter().enumerate() {
        let b = l[best];
        if p[1] > b[1] + 1e-9 || ((p[1] - b[1]).abs() <= 1e-9 && p[0] < b[0]) {
            best = i;
        }
    }
    best
}

/// Where a perimeter that closes starts, by the rule of the seam. `before` are the seams of the layer below.
fn seam_start(l: &[P], seam: Seam, before: &[P], head: P) -> usize {
    match seam {
        Seam::Nearest => nearest_vertex(l, head),
        Seam::Rear => rear_vertex(l),
        Seam::Aligned => {
            if before.is_empty() {
                return rear_vertex(l);
            }
            let mut best = (0usize, f64::MAX);
            for (i, p) in l.iter().enumerate() {
                let d = before
                    .iter()
                    .map(|q| geom::dist(*p, *q))
                    .fold(f64::MAX, f64::min);
                if d < best.1 {
                    best = (i, d);
                }
            }
            best.0
        }
    }
}

/// What is carried from one layer to the next.
struct State {
    head: P,
    /// The seams of the layer that was just made.
    seams: Vec<P>,
}

struct LayerInput<'a> {
    index: usize,
    height: f64,
    /// The height of the top of the layer.
    top: f64,
    supports: &'a [Path],
}

/// The perimeters of a layer at one width, from the outside in, and whether the part was too thin for them.
fn rings_at(outline: &[Loop], width: f64, cfg: &SliceConfig, h: f64) -> (Vec<Vec<Loop>>, bool) {
    let s = cfg.spacing_for(width, h);
    let mut thin = false;
    let mut rings: Vec<Vec<Loop>> = Vec::new();
    for k in 0..cfg.perimeters {
        let d = width / 2.0 + f64::from(k) * s;
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
    (rings, thin)
}

fn layer_paths(
    input: LayerInput,
    cfg: &SliceConfig,
    outlines: &[Vec<Loop>],
    warnings: &mut Vec<Warning>,
    st: &mut State,
) -> Result<Vec<Path>, SliceError> {
    let (index, h) = (input.index, input.height);
    let outline = &outlines[index];
    // a vase: one wall, a solid base, nothing inside and no top
    let vase_cfg;
    let open_vase = cfg.vase_mode && index >= cfg.bottom_layers as usize;
    let cfg = if cfg.vase_mode {
        let mut c = cfg.clone();
        c.perimeters = 1;
        c.top_layers = 0;
        c.infill_percent = 100.0;
        vase_cfg = c;
        &vase_cfg
    } else {
        cfg
    };
    let mut paths: Vec<Path> = Vec::new();
    // the supports first, so the part is laid on them
    paths.extend(order_paths(input.supports.to_vec(), &mut st.head));
    // the perimeters, from the inside out; a part too thin for them at full width gets narrower lines when the settings allow it
    let mut width = cfg.line_width_mm;
    let (mut rings, mut thin) = rings_at(outline, width, cfg, h);
    if thin && cfg.width_variation_percent > 0.0 {
        let narrowest = cfg.line_width_mm * (1.0 - cfg.width_variation_percent / 100.0);
        let steps = 5;
        for i in 1..=steps {
            let w = cfg.line_width_mm
                - (cfg.line_width_mm - narrowest) * f64::from(i) / f64::from(steps);
            let (r, t) = rings_at(outline, w, cfg, h);
            if !t {
                rings = r;
                thin = false;
                width = w;
                break;
            }
        }
    }
    if thin {
        if cfg.thin_regions == ThinRegions::Refuse {
            return Err(SliceError::ThinRegion { layer: index });
        }
        warnings.push(Warning::ThinRegion { layer: index });
    }
    let mut seams: Vec<P> = Vec::new();
    for (k, ring) in rings.iter().enumerate().rev() {
        let kind = if k == 0 {
            Kind::ExternalPerimeter
        } else {
            Kind::Perimeter
        };
        for l in ring {
            let start = if k == 0 {
                seam_start(l, cfg.seam, &st.seams, st.head)
            } else {
                nearest_vertex(l, st.head)
            };
            let mut pts: Vec<P> = l[start..].to_vec();
            pts.extend_from_slice(&l[..start]);
            if k == 0 {
                seams.push(pts[0]);
            }
            st.head = pts[0];
            paths.push(Path {
                kind,
                points: pts,
                closed: true,
                width_mm: width,
            });
        }
    }
    if !seams.is_empty() {
        st.seams = seams;
    }
    // the infill, inside the innermost perimeter
    let s = cfg.spacing_for(width, h);
    let d_region = width / 2.0 + f64::from(cfg.perimeters.saturating_sub(1)) * s + s / 2.0;
    let region: Vec<Loop> = outline.iter().flat_map(|l| inset(l, d_region)).collect();
    if !region.is_empty() && !open_vase {
        let fill = Fill {
            index,
            height: h,
            z: input.top,
            cfg,
            outlines,
            region: &region,
            bridges: cfg.supports == crate::config::Supports::Off,
        };
        paths.extend(fill_paths(&fill, &mut st.head)?);
    }
    Ok(paths)
}

impl Plan {
    /// The plastic the plan lays down, in cubic millimetres (the length of every path times the section of its line).
    pub fn extrusion_mm3(&self) -> f64 {
        self.layers
            .iter()
            .map(|l| {
                l.paths
                    .iter()
                    .map(|p| p.length_mm() * self.line_area_mm2(l.height_mm, p.kind, p.width_mm))
                    .sum::<f64>()
            })
            .sum()
    }

    /// The section of the plastic of a line of this kind and width on a layer of this height, in square millimetres: the shape of the line, the extrusion multiplier and, for a
    /// bridge, its own flow.
    pub fn line_area_mm2(&self, layer_height: f64, kind: Kind, width: f64) -> f64 {
        let c = &self.config;
        let flow = if kind == Kind::Bridge {
            c.bridge_flow_ratio
        } else {
            1.0
        };
        c.line_area_for(width, layer_height) * c.extrusion_multiplier * flow
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

    /// A closed mesh made of boxes that may touch on a face (the shared faces cancel): each is `(low corner, high corner)` in millimetres.
    pub(crate) fn boxes(parts: &[([f64; 3], [f64; 3])]) -> Mesh {
        let mut soup = Vec::new();
        let c = cube(1.0);
        for (lo, hi) in parts {
            for t in &c.triangles {
                soup.push(t.map(|i| {
                    let v = c.vertices[i as usize].mm();
                    [
                        lo[0] + v[0] * (hi[0] - lo[0]),
                        lo[1] + v[1] * (hi[1] - lo[1]),
                        lo[2] + v[2] * (hi[2] - lo[2]),
                    ]
                }));
            }
        }
        Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap()
    }

    /// A table: two pillars 5 mm wide and a plate over them, 30 x 10 x 6 mm, made from its profile in x and z. The plate hangs over 20 mm of nothing.
    fn table_mesh() -> Mesh {
        let poly: [[f64; 2]; 8] = [
            [0.0, 0.0],
            [5.0, 0.0],
            [5.0, 5.0],
            [25.0, 5.0],
            [25.0, 0.0],
            [30.0, 0.0],
            [30.0, 6.0],
            [0.0, 6.0],
        ];
        let caps: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 7],
            [2, 3, 6],
            [2, 6, 7],
            [4, 5, 6],
            [4, 6, 3],
        ];
        let at = |i: usize, y: f64| [poly[i][0], y, poly[i][1]];
        let mut soup: Vec<[[f64; 3]; 3]> = Vec::new();
        for t in caps {
            soup.push([at(t[0], 0.0), at(t[1], 0.0), at(t[2], 0.0)]);
            soup.push([at(t[0], 10.0), at(t[2], 10.0), at(t[1], 10.0)]);
        }
        for i in 0..poly.len() {
            let j = (i + 1) % poly.len();
            soup.push([at(i, 0.0), at(j, 10.0), at(j, 0.0)]);
            soup.push([at(i, 0.0), at(i, 10.0), at(j, 10.0)]);
        }
        Mesh::from_soup(&soup, &p3dp_core::Limits::default()).unwrap()
    }

    fn table() -> Mesh {
        table_mesh()
    }

    fn total_of(p: &Plan, kind: Kind) -> f64 {
        p.layers
            .iter()
            .flat_map(|l| &l.paths)
            .filter(|x| x.kind == kind)
            .map(|x| x.length_mm())
            .sum()
    }

    #[test]
    fn the_grid_has_lines_both_ways_on_every_layer_and_as_much_plastic_as_the_lines() {
        let rect = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        let grid_cfg = SliceConfig {
            infill_pattern: crate::config::InfillPattern::Grid,
            ..SliceConfig::default()
        };
        let grid = plan(&cube(20.0), &machine(), &grid_cfg).unwrap();
        for i in [30usize, 31] {
            let lines: Vec<&Path> = grid.layers[i]
                .paths
                .iter()
                .filter(|x| x.kind == Kind::Infill)
                .collect();
            assert!(lines.iter().any(|x| x.points[0][0] == x.points[1][0]));
            assert!(lines.iter().any(|x| x.points[0][1] == x.points[1][1]));
        }
        let ratio = grid.extrusion_mm3() / rect.extrusion_mm3();
        assert!((0.9..=1.1).contains(&ratio), "{ratio}");
    }

    #[test]
    fn the_gyroid_waves_stay_inside_and_more_infill_is_more_plastic() {
        let mk = |pct: f64| SliceConfig {
            infill_pattern: crate::config::InfillPattern::Gyroid,
            infill_percent: pct,
            ..SliceConfig::default()
        };
        let a = plan(&cube(20.0), &machine(), &mk(15.0)).unwrap();
        let b = plan(&cube(20.0), &machine(), &mk(35.0)).unwrap();
        let rect = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        assert!(b.extrusion_mm3() > a.extrusion_mm3());
        // 20 % of rectilinear and 15 % to 35 % of gyroid bracket each other
        assert!(a.extrusion_mm3() < rect.extrusion_mm3() * 1.1);
        assert!(b.extrusion_mm3() > rect.extrusion_mm3() * 0.9);
        // a wave is a path with many points, and its cut changes from one layer to the next
        let waves = |l: &Layer| -> Vec<Vec<P>> {
            l.paths
                .iter()
                .filter(|x| x.kind == Kind::Infill)
                .map(|x| x.points.clone())
                .collect()
        };
        assert!(waves(&a.layers[50]).iter().any(|w| w.len() > 5));
        assert_ne!(waves(&a.layers[50]), waves(&a.layers[51]));
        // inside the cube: 100 +- 10 on both axes
        for p in waves(&a.layers[50]).iter().flatten() {
            assert!(
                p[0] > 100.0 && p[0] < 120.0 && p[1] > 100.0 && p[1] < 120.0,
                "{p:?}"
            );
        }
        // the same plan twice, the same answer
        assert_eq!(a, plan(&cube(20.0), &machine(), &mk(15.0)).unwrap());
    }

    #[test]
    fn the_seam_goes_where_the_setting_says() {
        let first_external = |p: &Plan, layer: usize| -> P {
            p.layers[layer]
                .paths
                .iter()
                .find(|x| x.kind == Kind::ExternalPerimeter)
                .unwrap()
                .points[0]
        };
        let rear = plan(
            &cube(20.0),
            &machine(),
            &SliceConfig {
                seam: Seam::Rear,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        for i in [0usize, 10, 60] {
            let s = first_external(&rear, i);
            // the back left corner of the outer perimeter
            assert!(
                (s[1] - (120.0 - 0.225)).abs() < 1e-6 && (s[0] - (100.0 + 0.225)).abs() < 1e-6,
                "{s:?}"
            );
        }
        let aligned = plan(
            &cube(20.0),
            &machine(),
            &SliceConfig {
                seam: Seam::Aligned,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        let (a, b) = (first_external(&aligned, 20), first_external(&aligned, 21));
        assert_eq!(a, b, "the seam climbs in a line");
        let nearest = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        assert!(nearest.layers[50]
            .paths
            .iter()
            .any(|x| x.kind == Kind::ExternalPerimeter));
    }

    #[test]
    fn a_wall_too_thin_for_the_perimeters_gets_narrower_lines_when_the_settings_allow() {
        // a plate 0.9 mm thick: two lines of 0.45 mm do not fit, two of 0.32 do
        let plate = boxes(&[([0.0, 0.0, 0.0], [20.0, 0.9, 1.0])]);
        let fixed = plan(&plate, &machine(), &SliceConfig::default()).unwrap();
        assert!(fixed.warnings.contains(&Warning::ThinRegion { layer: 0 }));
        let wide = SliceConfig {
            width_variation_percent: 30.0,
            ..SliceConfig::default()
        };
        let var = plan(&plate, &machine(), &wide).unwrap();
        assert!(var.warnings.is_empty(), "{:?}", var.warnings);
        let widths: Vec<f64> = var.layers[0].paths.iter().map(|x| x.width_mm).collect();
        assert!(
            widths.iter().all(|w| *w < 0.45 && *w >= 0.45 * 0.70 - 1e-9),
            "{widths:?}"
        );
        // and a narrower line is less plastic per millimetre
        let long = |p: &Plan| {
            p.extrusion_mm3() / p.layers[0].paths.iter().map(|x| x.length_mm()).sum::<f64>()
        };
        assert!(long(&var) < long(&fixed));
        // a part that is wide enough keeps the full width even with the setting on
        let cube_plan = plan(&cube(20.0), &machine(), &wide).unwrap();
        assert!(cube_plan.layers[5].paths.iter().all(|x| x.width_mm == 0.45));
    }

    #[test]
    fn a_skin_over_nothing_is_a_bridge_and_a_bridge_that_is_too_long_is_refused() {
        let t = table();
        let p = plan(&t, &machine(), &SliceConfig::default()).unwrap();
        // the plate is the layers from z = 5: layer 25 is the first one over the gap
        let bridge = total_of(&p, Kind::Bridge);
        assert!(bridge > 100.0, "{bridge}");
        assert!(p.layers[24].paths.iter().all(|x| x.kind != Kind::Bridge));
        assert!(p.layers[25].paths.iter().any(|x| x.kind == Kind::Bridge));
        // laid across the gap from pillar to pillar, along x, and not along the 10 mm of the other way that no pillar holds
        let across = p.layers[25]
            .paths
            .iter()
            .find(|x| x.kind == Kind::Bridge)
            .unwrap();
        assert_eq!(across.points[0][1], across.points[1][1]);
        assert!(across.length_mm() > 15.0, "{}", across.length_mm());
        // a bridge is slower and has a little less plastic than a solid line of the same length
        assert!(p.speed_mm_s(25, Kind::Bridge) < p.speed_mm_s(25, Kind::SolidInfill));
        let line = p.line_area_mm2(0.2, Kind::SolidInfill, 0.45);
        assert!((p.line_area_mm2(0.2, Kind::Bridge, 0.45) / line - 0.95).abs() < 1e-12);
        // 20 mm of nothing is too much for a limit of 10 mm
        let strict = SliceConfig {
            max_bridge_span_mm: 10.0,
            ..SliceConfig::default()
        };
        let refused = plan(&t, &machine(), &strict);
        assert!(
            matches!(refused, Err(SliceError::BridgeTooLong { layer: 25, .. })),
            "{refused:?}"
        );
        // and 0 is no limit
        let none = SliceConfig {
            max_bridge_span_mm: 0.0,
            ..strict
        };
        assert!(plan(&t, &machine(), &none).is_ok());
    }

    #[test]
    fn supports_hold_the_plate_and_there_is_no_bridge_over_them() {
        let t = table();
        let cfg = SliceConfig {
            supports: crate::config::Supports::BuildPlate,
            max_bridge_span_mm: 10.0,
            ..SliceConfig::default()
        };
        let p = plan(&t, &machine(), &cfg).unwrap();
        assert!(total_of(&p, Kind::Support) > 100.0);
        assert_eq!(total_of(&p, Kind::Bridge), 0.0);
        // nothing is under the plate where the pillars are, and the supports end one layer short of the plate
        for l in &p.layers {
            for path in l.paths.iter().filter(|x| x.kind == Kind::Support) {
                for q in &path.points {
                    let x = q[0] - 95.0;
                    assert!(x > 5.0 && x < 25.0, "x {x}");
                }
                assert!(l.index < 24, "layer {}", l.index);
            }
        }
        // the gcode names them and the plastic of the file is the plastic of the plan
        let g = p.gcode("test").unwrap();
        assert!(g.contains("TYPE:support"));
    }

    #[test]
    fn the_head_lifts_on_a_travel_that_retracts_and_never_past_the_printer() {
        let cfg = SliceConfig {
            z_hop_mm: 0.4,
            ..SliceConfig::default()
        };
        let p = plan(&cube(20.0), &machine(), &cfg).unwrap();
        let g = p.gcode("test").unwrap();
        let mut hops = 0;
        let mut up = false;
        let mut layer_z = 0.0;
        for line in g.lines() {
            let code = line.split(';').next().unwrap_or("");
            if let Some(z) = code
                .split_ascii_whitespace()
                .find_map(|w| w.strip_prefix('Z'))
                .and_then(|v| v.parse::<f64>().ok())
            {
                if up {
                    assert!(
                        (z - layer_z).abs() < 1e-9,
                        "comes back to {layer_z}, not {z}"
                    );
                    up = false;
                } else if z > layer_z + 0.39 && z < layer_z + 0.41 {
                    hops += 1;
                    up = true;
                } else if z >= layer_z {
                    layer_z = z;
                }
            }
        }
        assert!(hops > 0 && !up);
        let low = Machine {
            volume_mm: [220.0, 220.0, 20.2],
            ..machine()
        };
        assert!(matches!(
            plan(&cube(20.0), &low, &cfg),
            Err(SliceError::LiftPastTop { .. })
        ));
        // with no hop the same printer takes the cube
        assert!(plan(&cube(20.0), &low, &SliceConfig::default()).is_ok());
    }

    #[test]
    fn the_fan_climbs_over_the_layers_asked_and_a_bridge_has_its_own() {
        let cfg = SliceConfig {
            fan_percent: 80.0,
            fan_from_layer: 2,
            fan_ramp_layers: 4,
            ..SliceConfig::default()
        };
        let p = plan(&cube(5.0), &machine(), &cfg).unwrap();
        let fans: Vec<f64> = (0..8).map(|i| p.fan_for_layer(i)).collect();
        assert_eq!(fans[0], 0.0);
        assert_eq!(fans[1], 0.0);
        assert!((fans[2] - 20.0).abs() < 1e-9 && (fans[3] - 40.0).abs() < 1e-9);
        assert!((fans[5] - 80.0).abs() < 1e-9 && fans[7] == 80.0);
        let g = p.gcode("test").unwrap();
        let set: Vec<&str> = g.lines().filter(|l| l.starts_with("M106")).collect();
        assert_eq!(set, vec!["M106 S51", "M106 S102", "M106 S153", "M106 S204"]);
        // a bridge puts the fan to its own level and the layer goes back to its level after it
        let t = plan(
            &table(),
            &machine(),
            &SliceConfig {
                fan_percent: 50.0,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        let g = t.gcode("test").unwrap();
        let lines: Vec<&str> = g.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.contains("TYPE:bridge"))
            .unwrap();
        assert!(
            lines[at..at + 3].iter().any(|l| l.starts_with("M106 S255")),
            "{:?}",
            &lines[at..at + 3]
        );
        let after = lines[at..]
            .iter()
            .position(|l| l.contains("TYPE:") && !l.contains("bridge"))
            .unwrap();
        assert!(lines[at + after..]
            .iter()
            .take(3)
            .any(|l| l.starts_with("M106")));
    }

    #[test]
    fn the_estimate_with_acceleration_is_longer_than_the_plain_one_and_follows_the_acceleration() {
        let base = plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap();
        assert!(base.estimated_seconds() > base.flat_estimated_seconds());
        let quick = plan(
            &cube(20.0),
            &machine(),
            &SliceConfig {
                accel_mm_s2: 20_000.0,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        let slow = plan(
            &cube(20.0),
            &machine(),
            &SliceConfig {
                accel_mm_s2: 500.0,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        assert!(slow.estimated_seconds() > base.estimated_seconds());
        assert!(base.estimated_seconds() > quick.estimated_seconds());
        assert!(quick.estimated_seconds() > quick.flat_estimated_seconds());
        let sums = base.summaries();
        assert!(
            (sums.iter().map(|s| s.time_s).sum::<f64>() - base.estimated_seconds()).abs() < 1e-9
        );
        assert!(sums.iter().all(|s| s.flat_time_s <= s.time_s));
    }
}
