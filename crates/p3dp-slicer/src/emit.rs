//! From the plan to the moves of the head and to G-code. The moves are worked out once (`layer_moves`) and used for the time, the summaries and the file, so the three agree. The
//! file is written by the writer of the program, which refuses a temperature over the limit and a coordinate outside the bed: this module cannot write what that one refuses.

use crate::error::SliceError;
use crate::geom::{dist, P};
use crate::plan::{Kind, Layer, Plan};
use crate::timing::{self, Block};
use p3dp_gcode::{JobParams, Writer};

/// What the head does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// Pull the filament back by this many millimetres.
    Retract(f64),
    /// Push it forward again by this many millimetres.
    Unretract(f64),
    /// Go to this height.
    Z(f64),
    /// Go to this point without extruding.
    Travel(P),
    /// Go to this point extruding.
    Line {
        to: P,
        kind: Kind,
        length_mm: f64,
        width_mm: f64,
    },
}

/// What a layer comes to.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSummary {
    pub index: usize,
    pub z_mm: f64,
    pub height_mm: f64,
    pub paths: usize,
    pub perimeter_mm: f64,
    pub infill_mm: f64,
    pub solid_mm: f64,
    pub bridge_mm: f64,
    pub support_mm: f64,
    /// The skirt, the brim and the raft.
    pub adhesion_mm: f64,
    pub travel_mm: f64,
    pub extrusion_mm3: f64,
    /// The time of the layer with the head speeding up and slowing down.
    pub time_s: f64,
    /// The time of the layer with every move at its top speed.
    pub flat_time_s: f64,
    /// The time of the layer if the head stopped at every joint.
    pub stop_go_time_s: f64,
}

/// The three times of a print and whether they agree: the plain time is the least, the stop-and-go time is the most, and the estimate lies between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeCheck {
    pub flat_s: f64,
    pub estimate_s: f64,
    pub stop_go_s: f64,
}

impl TimeCheck {
    /// Whether the estimate lies between the least and the most (to a millionth, for the rounding of the sums).
    pub fn is_consistent(&self) -> bool {
        let slack = 1e-6 * self.stop_go_s.max(1.0);
        self.flat_s.is_finite()
            && self.estimate_s.is_finite()
            && self.stop_go_s.is_finite()
            && self.flat_s <= self.estimate_s + slack
            && self.estimate_s <= self.stop_go_s + slack
    }
}

impl Plan {
    /// The speed of a line of this kind on this layer, in millimetres per second: the speed of its kind, slowed down on a layer that would take less than `min_layer_time_s`
    /// (never below `min_speed_mm_s`, unless the kind was slower than that already).
    pub fn speed_mm_s(&self, layer: usize, kind: Kind) -> f64 {
        let base = self.base_speed_mm_s(layer, kind);
        let scale = self.layer_scales.get(layer).copied().unwrap_or(1.0);
        if scale < 1.0 {
            (base * scale).max(self.config.min_speed_mm_s.min(base))
        } else {
            base
        }
    }

    /// The speed of a line of this kind on this layer before a short layer is slowed down.
    pub fn base_speed_mm_s(&self, layer: usize, kind: Kind) -> f64 {
        let c = &self.config;
        if layer == 0 || (self.raft_layers > 0 && layer == self.raft_layers) {
            return c.first_layer_speed_mm_s;
        }
        match kind {
            Kind::ExternalPerimeter => c.external_perimeter_speed_mm_s,
            Kind::Perimeter => c.perimeter_speed_mm_s,
            Kind::Infill => c.infill_speed_mm_s,
            Kind::SolidInfill => c.solid_infill_speed_mm_s,
            Kind::Bridge => c.bridge_speed_mm_s,
            Kind::Support => c.support_speed_mm_s,
            Kind::Adhesion => c.perimeter_speed_mm_s,
            Kind::Raft => c.solid_infill_speed_mm_s,
        }
    }

    /// How much each layer is slowed down so that no layer takes less than `min_layer_time_s` (1 is not at all): the time of the lines of the layer at their own speeds against
    /// the time asked for. The first layer and the first layer on a raft are slow already and are left alone.
    pub(crate) fn compute_layer_scales(&self) -> Vec<f64> {
        let c = &self.config;
        self.layers
            .iter()
            .map(|l| {
                let first = l.index == 0 || (self.raft_layers > 0 && l.index == self.raft_layers);
                if c.min_layer_time_s <= 0.0 || first {
                    return 1.0;
                }
                let t: f64 = l
                    .paths
                    .iter()
                    .map(|p| p.length_mm() / self.base_speed_mm_s(l.index, p.kind))
                    .sum();
                if t > 0.0 && t < c.min_layer_time_s {
                    t / c.min_layer_time_s
                } else {
                    1.0
                }
            })
            .collect()
    }

    /// The filament pulled back for a travel of this length: all of `retract_mm`, or less for a short travel when `retract_adaptive` is on.
    pub fn retract_for_travel(&self, travel_mm: f64) -> f64 {
        let c = &self.config;
        if !c.retract_adaptive {
            return c.retract_mm;
        }
        let least = c.retract_min_percent / 100.0;
        let share = (travel_mm / c.retract_full_travel_mm).clamp(0.0, 1.0);
        c.retract_mm * (least + (1.0 - least) * share)
    }

    /// The part fan on a layer, in per cent: nothing before `fan_from_layer`, then `fan_percent`, climbing over `fan_ramp_layers` when that is set.
    pub fn fan_for_layer(&self, layer: usize) -> f64 {
        let c = &self.config;
        let from = c.fan_from_layer as usize;
        if c.fan_percent <= 0.0 || layer < from {
            return 0.0;
        }
        if c.fan_ramp_layers == 0 {
            return c.fan_percent;
        }
        let done = (layer - from + 1) as f64 / f64::from(c.fan_ramp_layers);
        c.fan_percent * done.min(1.0)
    }

    /// The moves of a layer from where the head is. `pulled` is how many millimetres of filament are pulled back (0 when none), and is left as the layer ends. Returns the moves
    /// and where the head is.
    pub fn layer_moves(&self, layer: &Layer, from: P, pulled: &mut f64) -> (Vec<Move>, P) {
        let c = &self.config;
        let mut moves = Vec::new();
        let mut at = from;
        // at the very start the head is parked and the filament is where the start of the program left it
        if layer.index > 0 && c.retract_mm > 0.0 && *pulled <= 0.0 {
            moves.push(Move::Retract(c.retract_mm));
            *pulled = c.retract_mm;
        }
        moves.push(Move::Z(layer.z_mm));
        for path in &layer.paths {
            let Some(&start) = path.points.first() else {
                continue;
            };
            let d = dist(at, start);
            if d > 1e-6 {
                let long = d >= c.retract_min_travel_mm;
                if c.retract_mm > 0.0 && *pulled <= 0.0 && long {
                    let mm = self.retract_for_travel(d);
                    if mm > 1e-9 {
                        moves.push(Move::Retract(mm));
                        *pulled = mm;
                    }
                }
                let hop = c.z_hop_mm > 0.0 && long;
                if hop {
                    moves.push(Move::Z(layer.z_mm + c.z_hop_mm));
                }
                moves.push(Move::Travel(start));
                at = start;
                if hop {
                    moves.push(Move::Z(layer.z_mm));
                }
            }
            if *pulled > 0.0 {
                moves.push(Move::Unretract(*pulled));
                *pulled = 0.0;
            }
            let rest = path.points.iter().skip(1).copied();
            let closing = path.closed.then_some(start);
            for to in rest.chain(closing) {
                moves.push(Move::Line {
                    to,
                    kind: path.kind,
                    length_mm: dist(at, to),
                    width_mm: path.width_mm,
                });
                at = to;
            }
        }
        (moves, at)
    }

    /// What every layer comes to.
    pub fn summaries(&self) -> Vec<LayerSummary> {
        let c = &self.config;
        let mut out = Vec::with_capacity(self.layers.len());
        let mut at: P = [0.0, 0.0];
        let mut pulled = 0.0f64;
        let mut z_now = 0.0f64;
        for l in &self.layers {
            let (moves, end) = self.layer_moves(l, at, &mut pulled);
            let mut here = at;
            at = end;
            let mut s = LayerSummary {
                index: l.index,
                z_mm: l.z_mm,
                height_mm: l.height_mm,
                paths: l.paths.len(),
                perimeter_mm: 0.0,
                infill_mm: 0.0,
                solid_mm: 0.0,
                bridge_mm: 0.0,
                support_mm: 0.0,
                adhesion_mm: 0.0,
                travel_mm: 0.0,
                extrusion_mm3: 0.0,
                time_s: 0.0,
                flat_time_s: 0.0,
                stop_go_time_s: 0.0,
            };
            let mut blocks: Vec<Block> = Vec::with_capacity(moves.len());
            for m in moves {
                match m {
                    Move::Retract(mm) | Move::Unretract(mm) => {
                        blocks.push(Block::stop(mm, c.retract_speed_mm_s))
                    }
                    Move::Z(z) => {
                        blocks.push(Block::stop((z - z_now).abs(), c.z_speed_mm_s));
                        z_now = z;
                    }
                    Move::Travel(to) => {
                        s.travel_mm += dist(here, to);
                        blocks.push(Block::planar(here, to, c.travel_speed_mm_s));
                        here = to;
                    }
                    Move::Line {
                        to,
                        kind,
                        length_mm,
                        width_mm,
                    } => {
                        match kind {
                            Kind::ExternalPerimeter | Kind::Perimeter => {
                                s.perimeter_mm += length_mm
                            }
                            Kind::Infill => s.infill_mm += length_mm,
                            Kind::SolidInfill => s.solid_mm += length_mm,
                            Kind::Bridge => s.bridge_mm += length_mm,
                            Kind::Support => s.support_mm += length_mm,
                            Kind::Adhesion | Kind::Raft => s.adhesion_mm += length_mm,
                        }
                        s.extrusion_mm3 +=
                            length_mm * self.line_area_mm2(l.height_mm, kind, width_mm);
                        blocks.push(Block::planar(here, to, self.speed_mm_s(l.index, kind)));
                        here = to;
                    }
                }
            }
            s.time_s = timing::seconds(&blocks, c.accel_mm_s2, c.corner_speed_mm_s);
            s.flat_time_s = timing::flat_seconds(&blocks);
            s.stop_go_time_s = timing::stop_and_go_seconds(&blocks, c.accel_mm_s2);
            out.push(s);
        }
        out
    }

    /// The time the print takes, in seconds, with the head speeding up and slowing down at the acceleration of the settings. It is an estimate: the planner of the printer is its own.
    pub fn estimated_seconds(&self) -> f64 {
        self.summaries().iter().map(|s| s.time_s).sum()
    }

    /// The least, the estimate and the most of the time of the print, to be compared with one another.
    pub fn time_check(&self) -> TimeCheck {
        let s = self.summaries();
        TimeCheck {
            flat_s: s.iter().map(|x| x.flat_time_s).sum(),
            estimate_s: s.iter().map(|x| x.time_s).sum(),
            stop_go_s: s.iter().map(|x| x.stop_go_time_s).sum(),
        }
    }

    /// The time with every move at its top speed and no acceleration: the least the print can take.
    pub fn flat_estimated_seconds(&self) -> f64 {
        self.summaries().iter().map(|s| s.flat_time_s).sum()
    }

    /// The G-code of the plan, in the dialect of the machine. `version` is the version of the program, for the header.
    pub fn gcode(&self, version: &str) -> Result<String, SliceError> {
        let c = &self.config;
        let m = &self.machine;
        let mut w = Writer::new(m.dialect, m.limits, m.volume_mm)?;
        w.header("PHOENIX-3DP slicer", version, &m.id);
        for line in [
            format!(
                "layers: {} of {} mm (the first {} mm)",
                self.layers.len(),
                c.layer_height_mm,
                c.first_layer_height_mm
            ),
            format!(
                "line width {} mm, nozzle {} mm, {} perimeters, {} % {} infill",
                c.line_width_mm,
                c.nozzle_diameter_mm,
                c.perimeters,
                c.infill_percent,
                c.infill_pattern.name()
            ),
            format!(
                "filament {} mm, extrusion multiplier {}",
                c.filament_diameter_mm, c.extrusion_multiplier
            ),
        ] {
            w.comment(&line)?;
        }
        let minutes = (self.estimated_seconds() / 60.0)
            .ceil()
            .min(f64::from(u32::MAX)) as u32;
        w.preamble(&JobParams {
            nozzle_c: c.nozzle_c,
            bed_c: c.bed_c,
            chamber_c: (c.chamber_c > 0.0).then_some(c.chamber_c),
            fan_pct: c.fan_percent,
            estimated_minutes: Some(minutes),
        })?;
        let area_filament = c.filament_area_mm2();
        let mut carry = 0.0f64;
        let mut at: P = [0.0, 0.0];
        let mut pulled = 0.0f64;
        let mut kind_now: Option<Kind> = None;
        let mut last_z = 0.0;
        // the preamble switches the fan off
        let mut fan_now = 0.0f64;
        for l in &self.layers {
            w.layer(l.index as u32, l.z_mm)?;
            let layer_fan = self.fan_for_layer(l.index);
            if (layer_fan - fan_now).abs() > 1e-9 {
                w.set_fan(layer_fan)?;
                fan_now = layer_fan;
            }
            let (moves, end) = self.layer_moves(l, at, &mut pulled);
            at = end;
            for mv in moves {
                match mv {
                    Move::Retract(mm) => w.linear_move(
                        None,
                        None,
                        None,
                        Some(-mm),
                        Some(c.retract_speed_mm_s * 60.0),
                    )?,
                    Move::Unretract(mm) => w.linear_move(
                        None,
                        None,
                        None,
                        Some(mm),
                        Some(c.retract_speed_mm_s * 60.0),
                    )?,
                    Move::Z(z) => {
                        w.linear_move(
                            None,
                            None,
                            Some(coord(z)),
                            None,
                            Some(c.z_speed_mm_s * 60.0),
                        )?;
                        last_z = z;
                    }
                    Move::Travel(p) => w.linear_move(
                        Some(coord(p[0])),
                        Some(coord(p[1])),
                        None,
                        None,
                        Some(c.travel_speed_mm_s * 60.0),
                    )?,
                    Move::Line {
                        to,
                        kind,
                        length_mm,
                        width_mm,
                    } => {
                        if kind_now != Some(kind) {
                            w.comment(&format!("TYPE:{}", kind.name()))?;
                            kind_now = Some(kind);
                        }
                        let want_fan = if kind == Kind::Bridge {
                            c.bridge_fan_percent
                        } else {
                            layer_fan
                        };
                        if (want_fan - fan_now).abs() > 1e-9 {
                            w.set_fan(want_fan)?;
                            fan_now = want_fan;
                        }
                        let e_per_mm =
                            self.line_area_mm2(l.height_mm, kind, width_mm) / area_filament;
                        // the amounts are written with three decimals: what the rounding takes off one line is put on the next, so the total is the true one
                        let exact = length_mm * e_per_mm + carry;
                        let written = (exact * 1000.0).round() / 1000.0;
                        carry = exact - written;
                        w.linear_move(
                            Some(coord(to[0])),
                            Some(coord(to[1])),
                            None,
                            Some(written),
                            Some(self.speed_mm_s(l.index, kind) * 60.0),
                        )?;
                    }
                }
            }
            if w.lines().len() > c.max_gcode_lines as usize {
                return Err(SliceError::TooManyLines {
                    lines: w.lines().len(),
                    max: c.max_gcode_lines as usize,
                });
            }
        }
        if c.end_lift_mm > 0.0 {
            let z = (last_z + c.end_lift_mm).min(m.volume_mm[2]);
            if z > last_z {
                w.linear_move(
                    None,
                    None,
                    Some(coord(z)),
                    None,
                    Some(c.z_speed_mm_s * 60.0),
                )?;
            }
        }
        w.end_safe()?;
        Ok(w.finish()?)
    }
}

/// A coordinate to three decimals. The paths are inside the bed to a millionth of a millimetre (the plan checks it); the rounding takes off the float noise at the edge.
fn coord(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}
