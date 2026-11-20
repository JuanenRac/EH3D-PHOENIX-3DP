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
    /// Pull the filament back.
    Retract,
    /// Push it forward again.
    Unretract,
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
    pub travel_mm: f64,
    pub extrusion_mm3: f64,
    /// The time of the layer with the head speeding up and slowing down.
    pub time_s: f64,
    /// The time of the layer with every move at its top speed.
    pub flat_time_s: f64,
}

impl Plan {
    /// The speed of a line of this kind on this layer, in millimetres per second.
    pub fn speed_mm_s(&self, layer: usize, kind: Kind) -> f64 {
        let c = &self.config;
        if layer == 0 {
            return c.first_layer_speed_mm_s;
        }
        match kind {
            Kind::ExternalPerimeter => c.external_perimeter_speed_mm_s,
            Kind::Perimeter => c.perimeter_speed_mm_s,
            Kind::Infill => c.infill_speed_mm_s,
            Kind::SolidInfill => c.solid_infill_speed_mm_s,
            Kind::Bridge => c.bridge_speed_mm_s,
            Kind::Support => c.support_speed_mm_s,
        }
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

    /// The moves of a layer from where the head is. `retracted` says whether the filament is pulled back, and is left as the layer ends. Returns the moves and where the head is.
    pub fn layer_moves(&self, layer: &Layer, from: P, retracted: &mut bool) -> (Vec<Move>, P) {
        let c = &self.config;
        let mut moves = Vec::new();
        let mut at = from;
        // at the very start the head is parked and the filament is where the start of the program left it
        if layer.index > 0 && c.retract_mm > 0.0 && !*retracted {
            moves.push(Move::Retract);
            *retracted = true;
        }
        moves.push(Move::Z(layer.z_mm));
        for path in &layer.paths {
            let Some(&start) = path.points.first() else {
                continue;
            };
            let d = dist(at, start);
            if d > 1e-6 {
                let long = d >= c.retract_min_travel_mm;
                if c.retract_mm > 0.0 && !*retracted && long {
                    moves.push(Move::Retract);
                    *retracted = true;
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
            if *retracted {
                moves.push(Move::Unretract);
                *retracted = false;
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
        let mut retracted = false;
        let mut z_now = 0.0f64;
        for l in &self.layers {
            let (moves, end) = self.layer_moves(l, at, &mut retracted);
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
                travel_mm: 0.0,
                extrusion_mm3: 0.0,
                time_s: 0.0,
                flat_time_s: 0.0,
            };
            let mut blocks: Vec<Block> = Vec::with_capacity(moves.len());
            for m in moves {
                match m {
                    Move::Retract | Move::Unretract => {
                        blocks.push(Block::stop(c.retract_mm, c.retract_speed_mm_s))
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
            out.push(s);
        }
        out
    }

    /// The time the print takes, in seconds, with the head speeding up and slowing down at the acceleration of the settings. It is an estimate: the planner of the printer is its own.
    pub fn estimated_seconds(&self) -> f64 {
        self.summaries().iter().map(|s| s.time_s).sum()
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
        let mut retracted = false;
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
            let (moves, end) = self.layer_moves(l, at, &mut retracted);
            at = end;
            for mv in moves {
                match mv {
                    Move::Retract => w.linear_move(
                        None,
                        None,
                        None,
                        Some(-c.retract_mm),
                        Some(c.retract_speed_mm_s * 60.0),
                    )?,
                    Move::Unretract => w.linear_move(
                        None,
                        None,
                        None,
                        Some(c.retract_mm),
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
