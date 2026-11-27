//! From the plan to the moves of the head and to G-code. The moves are worked out once (`layer_moves`) and used for the time, the summaries and the file, so the three agree. The
//! file is written by the writer of the program, which refuses a temperature over the limit and a coordinate outside the bed: this module cannot write what that one refuses.

use crate::error::SliceError;
use crate::geom::{dist, P};
use crate::plan::{Kind, Layer, Path, Plan};
use crate::timing::{self, Block};
use p3dp_core::Dialect;
use p3dp_gcode::{BlockJob, JobParams, Place, Writer};

/// What the head does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// Pull the filament back by this many millimetres.
    Retract(f64),
    /// Push it forward again by this many millimetres.
    Unretract(f64),
    /// Go back along the line just laid while pulling the filament back by this many millimetres.
    Wipe { to: P, mm: f64 },
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

/// The start and end G-code of the person, added to the ones of the program and checked against the machine (see `p3dp_gcode::template`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserGcode {
    pub start: Option<String>,
    pub end: Option<String>,
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

    /// The speed of one line: `speed_mm_s`, kept so that the plastic the line asks for in a second (its speed times the section of the line) is no more than
    /// `max_volumetric_flow_mm3_s` when that is set. A line is slowed down and never thinned, and never to less than 1 mm/s.
    pub fn line_speed_mm_s(&self, layer: usize, kind: Kind, width_mm: f64, height_mm: f64) -> f64 {
        let speed = self.speed_mm_s(layer, kind);
        let q = self.config.max_volumetric_flow_mm3_s;
        if q <= 0.0 {
            return speed;
        }
        let area = self.line_area_mm2(height_mm, kind, width_mm);
        if area <= 0.0 {
            return speed;
        }
        speed.min((q / area).max(1.0))
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
            Kind::Tower => c.perimeter_speed_mm_s,
            Kind::Ironing => c.ironing_speed_mm_s,
            Kind::OverhangPerimeter if c.overhang_speed_mm_s > 0.0 => {
                c.overhang_speed_mm_s.min(c.external_perimeter_speed_mm_s)
            }
            Kind::OverhangPerimeter => c.external_perimeter_speed_mm_s,
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
        // the outer walls of the layer, for the travels that stay inside them
        let walls = if c.retract_only_crossing {
            outer_walls(layer)
        } else {
            Vec::new()
        };
        // the path that ends where the head is, which a wipe goes back along
        let mut laid: Option<&Path> = None;
        for path in &layer.paths {
            let Some(&start) = path.points.first() else {
                continue;
            };
            let d = dist(at, start);
            if d > 1e-6 {
                let long = d >= c.retract_min_travel_mm
                    && !(c.retract_only_crossing && stays_inside(at, start, &walls));
                if c.retract_mm > 0.0 && *pulled <= 0.0 && long {
                    let mm = self.retract_for_travel(d);
                    if mm > 1e-9 {
                        let wiped = match laid {
                            Some(last) if c.wipe_mm > 0.0 => wipe_moves(last, c.wipe_mm, mm),
                            _ => Vec::new(),
                        };
                        if wiped.is_empty() {
                            moves.push(Move::Retract(mm));
                        }
                        moves.extend(
                            wiped
                                .into_iter()
                                .map(|(to, part)| Move::Wipe { to, mm: part }),
                        );
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
            laid = Some(path);
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
                    Move::Wipe { to, .. } => {
                        s.travel_mm += dist(here, to);
                        blocks.push(Block::planar(here, to, c.external_perimeter_speed_mm_s));
                        here = to;
                    }
                    Move::Line {
                        to,
                        kind,
                        length_mm,
                        width_mm,
                    } => {
                        match kind {
                            Kind::ExternalPerimeter | Kind::Perimeter | Kind::OverhangPerimeter => {
                                s.perimeter_mm += length_mm
                            }
                            Kind::Infill => s.infill_mm += length_mm,
                            Kind::SolidInfill | Kind::Ironing => s.solid_mm += length_mm,
                            Kind::Bridge => s.bridge_mm += length_mm,
                            Kind::Support => s.support_mm += length_mm,
                            Kind::Adhesion | Kind::Raft | Kind::Tower => s.adhesion_mm += length_mm,
                        }
                        s.extrusion_mm3 +=
                            length_mm * self.line_area_mm2(l.height_mm, kind, width_mm);
                        blocks.push(Block::planar(
                            here,
                            to,
                            self.line_speed_mm_s(l.index, kind, width_mm, l.height_mm),
                        ));
                        here = to;
                    }
                }
            }
            // a change of tool takes the same time whatever the planner does
            let change_s: f64 = self
                .tool_changes
                .iter()
                .filter(|e| e.layer == l.index)
                .map(|e| e.time_s)
                .sum();
            s.time_s = timing::seconds(&blocks, c.accel_mm_s2, c.corner_speed_mm_s) + change_s;
            s.flat_time_s = timing::flat_seconds(&blocks) + change_s;
            s.stop_go_time_s = timing::stop_and_go_seconds(&blocks, c.accel_mm_s2) + change_s;
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
        self.gcode_with(version, &UserGcode::default())
    }

    /// The G-code of the plan with the start and end G-code of the person. A line of theirs that is not allowed refuses the whole file.
    pub fn gcode_with(&self, version: &str, user: &UserGcode) -> Result<String, SliceError> {
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
        let times: Vec<f64> = self.summaries().iter().map(|s| s.time_s).collect();
        let total_s: f64 = times.iter().sum();
        let minutes_of = |s: f64| (s / 60.0).ceil().clamp(0.0, f64::from(u32::MAX)) as u32;
        let minutes = minutes_of(total_s);
        let or_all = |first: f64, rest: f64| if first > 0.0 { first } else { rest };
        let first_nozzle = or_all(c.first_layer_nozzle_c, c.nozzle_c);
        let first_bed = or_all(c.first_layer_bed_c, c.bed_c);
        w.preamble(&JobParams {
            nozzle_c: first_nozzle,
            bed_c: first_bed,
            chamber_c: (c.chamber_c > 0.0).then_some(c.chamber_c),
            fan_pct: c.fan_percent,
            estimated_minutes: Some(minutes),
        })?;
        let job = BlockJob {
            nozzle_c: first_nozzle,
            bed_c: first_bed,
            chamber_c: c.chamber_c,
            top_z_mm: self.layers.last().map_or(0.0, |l| coord(l.z_mm)),
        };
        if let Some(start) = &user.start {
            w.user_block(start, Place::Start, &job)?;
            // whatever the block set, the print starts hot and with the fan off
            w.set_bed(first_bed, true)?;
            w.set_nozzle(first_nozzle, true)?;
            w.set_fan(0.0)?;
        }
        if c.pressure_advance > 0.0 {
            w.comment(&format!("pressure advance {}", c.pressure_advance))?;
            w.set_pressure_advance(c.pressure_advance)?;
        }
        if let Some(tool) = self.first_tool {
            w.comment(&format!("starts with tool {tool}"))?;
            // RepRapFirmware keeps a temperature for each tool, so the first one needs its own
            if m.dialect == Dialect::Rrf {
                w.change_tool(tool, first_nozzle)?;
            } else {
                w.select_tool(tool)?;
            }
        }
        let area_filament = c.filament_area_mm2();
        let mut carry = 0.0f64;
        let mut at: P = [0.0, 0.0];
        let mut pulled = 0.0f64;
        let mut kind_now: Option<Kind> = None;
        let mut last_z = 0.0;
        // the preamble switches the fan off
        let mut fan_now = 0.0f64;
        let mut done_s = 0.0f64;
        let mut percent_now: Option<u32> = None;
        if c.progress_marks {
            w.total_layers(self.layers.len() as u32);
        }
        for (number, l) in self.layers.iter().enumerate() {
            w.layer(l.index as u32, l.z_mm)?;
            if c.progress_marks && total_s > 0.0 {
                let percent = (done_s / total_s * 100.0).floor().clamp(0.0, 99.0) as u32;
                if percent_now != Some(percent) {
                    w.progress(percent, minutes_of(total_s - done_s));
                    percent_now = Some(percent);
                }
            }
            done_s += times.get(number).copied().unwrap_or(0.0);
            // the temperatures of the rest of the print, from the second layer on
            if number == 1 {
                if (first_bed - c.bed_c).abs() > 1e-9 {
                    w.set_bed(c.bed_c, false)?;
                }
                if (first_nozzle - c.nozzle_c).abs() > 1e-9 {
                    w.set_nozzle(c.nozzle_c, false)?;
                }
            }
            let pause = c.pause_at_layers.contains(&(number as u32 + 1));
            let changes = self.tool_changes.iter().any(|e| e.layer == l.index);
            // the filament is pulled back before a pause and before the head or the spool that is printing is left
            if (pause || changes) && c.retract_mm > 0.0 && pulled <= 0.0 {
                w.linear_move(
                    None,
                    None,
                    None,
                    Some(-c.retract_mm),
                    Some(c.retract_speed_mm_s * 60.0),
                )?;
                pulled = c.retract_mm;
            }
            if pause {
                w.comment(&format!("pause before layer {}", number + 1))?;
                w.pause();
            }
            for e in self.tool_changes.iter().filter(|e| e.layer == l.index) {
                w.comment(&format!(
                    "tool change to {} for {:.0} mm3 of purge",
                    e.tool, e.purge_mm3
                ))?;
                // the new tool is brought to its temperature, and waited for, before the purge
                w.change_tool(e.tool, e.nozzle_c)?;
            }
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
                    Move::Wipe { to, mm } => w.linear_move(
                        Some(coord(to[0])),
                        Some(coord(to[1])),
                        None,
                        Some(-mm),
                        Some(c.external_perimeter_speed_mm_s * 60.0),
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
                        let hanging = matches!(kind, Kind::Bridge | Kind::OverhangPerimeter);
                        let want_fan = if hanging {
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
                            Some(self.line_speed_mm_s(l.index, kind, width_mm, l.height_mm) * 60.0),
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
        if c.progress_marks {
            w.progress(100, 0);
        }
        if let Some(end) = &user.end {
            let job = BlockJob {
                nozzle_c: c.nozzle_c,
                bed_c: c.bed_c,
                ..job
            };
            w.user_block(end, Place::End, &job)?;
        }
        w.end_safe()?;
        Ok(w.finish()?)
    }
}

/// The segments of the outer walls of a layer (whole or in runs), which hold between them the inside of the part.
fn outer_walls(layer: &Layer) -> Vec<(P, P)> {
    let mut out = Vec::new();
    for p in &layer.paths {
        if !matches!(p.kind, Kind::ExternalPerimeter | Kind::OverhangPerimeter) {
            continue;
        }
        out.extend(p.points.windows(2).map(|w| (w[0], w[1])));
        if let (true, Some(first), Some(last)) = (p.closed, p.points.first(), p.points.last()) {
            out.push((*last, *first));
        }
    }
    out
}

/// Whether two segments touch or cross.
fn meet(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    let apart = |x: f64, y: f64| (x > 1e-9 && y > 1e-9) || (x < -1e-9 && y < -1e-9);
    let boxes = (0..2).all(|k| {
        a[k].min(b[k]) <= c[k].max(d[k]) + 1e-9 && c[k].min(d[k]) <= a[k].max(b[k]) + 1e-9
    });
    boxes && !apart(d1, d2) && !apart(d3, d4)
}

/// Whether a straight travel stays inside the outer walls: it touches none of them and its middle is inside an odd number of them.
fn stays_inside(from: P, to: P, walls: &[(P, P)]) -> bool {
    if walls.iter().any(|(a, b)| meet(from, to, *a, *b)) {
        return false;
    }
    let m = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
    let crossings = walls
        .iter()
        .filter(|(a, b)| {
            (a[1] <= m[1]) != (b[1] <= m[1])
                && a[0] + (m[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) > m[0]
        })
        .count();
    crossings % 2 == 1
}

/// The moves that go back along a path from its end, up to `wipe_mm` long, and the filament each one pulls back: together `retract_mm` to the third decimal, as it is written.
/// Nothing for a path too short to go back along.
fn wipe_moves(path: &Path, wipe_mm: f64, retract_mm: f64) -> Vec<(P, f64)> {
    let mut back: Vec<P> = path.points.iter().rev().copied().collect();
    if path.closed {
        if let Some(first) = path.points.first() {
            back.insert(0, *first);
        }
    }
    let mut legs: Vec<(P, f64)> = Vec::new();
    let mut length = 0.0;
    for w in back.windows(2) {
        let leg = dist(w[0], w[1]);
        if leg < 1e-6 {
            continue;
        }
        let take = leg.min(wipe_mm - length);
        let t = take / leg;
        legs.push((
            [
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
            ],
            take,
        ));
        length += take;
        if length >= wipe_mm - 1e-9 {
            break;
        }
    }
    if length < 0.1 {
        return Vec::new();
    }
    let milli = |v: f64| (v * 1000.0).round() / 1000.0;
    let mut gone = 0.0;
    let mut pulled = 0.0;
    legs.into_iter()
        .map(|(to, leg)| {
            gone += leg;
            let upto = milli(retract_mm * gone / length);
            let part = upto - pulled;
            pulled = upto;
            (to, part)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<(P, P)> {
        let q = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        (0..4).map(|i| (q[i], q[(i + 1) % 4])).collect()
    }

    #[test]
    fn a_travel_stays_inside_only_when_it_touches_no_wall_and_is_within_them() {
        let walls = square();
        assert!(stays_inside([2.0, 2.0], [8.0, 8.0], &walls));
        assert!(!stays_inside([2.0, 2.0], [12.0, 2.0], &walls));
        assert!(!stays_inside([12.0, 2.0], [14.0, 2.0], &walls));
        // from a point on the wall itself: the careful answer is no
        assert!(!stays_inside([0.0, 5.0], [8.0, 5.0], &walls));
        assert!(!stays_inside([2.0, 2.0], [8.0, 8.0], &[]));
    }

    #[test]
    fn a_wipe_goes_back_along_the_path_and_shares_the_filament_to_the_third_decimal() {
        let path = Path {
            kind: Kind::Infill,
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 1.0]],
            closed: false,
            width_mm: 0.45,
        };
        let legs = wipe_moves(&path, 3.0, 1.0);
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].0, [10.0, 0.0]);
        assert_eq!(legs[1].0, [8.0, 0.0]);
        let sum: f64 = legs.iter().map(|l| l.1).sum();
        assert!((sum - 1.0).abs() < 1e-12);
        assert!((legs[0].1 - 0.333).abs() < 1e-12);
        // a closed path ends where it began, and is followed back from there
        let loop_path = Path {
            closed: true,
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            ..path.clone()
        };
        assert_eq!(wipe_moves(&loop_path, 1.0, 0.5)[0].0[1], 1.0 / 2f64.sqrt());
        let dot = Path {
            points: vec![[0.0, 0.0], [0.05, 0.0]],
            ..path
        };
        assert!(wipe_moves(&dot, 3.0, 1.0).is_empty());
    }
}
