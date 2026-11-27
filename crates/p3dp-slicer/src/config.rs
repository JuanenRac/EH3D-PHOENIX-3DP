//! The settings of a slice. Every value has a range and a conservative default, is read from a `key = value` file like the other settings of the program, and a value outside its
//! range is an error that names the key: nothing is lowered or raised to fit. Whatever was tuned lives here and in the file, not in the code.

use crate::error::SliceError;
use p3dp_core::kv;
use p3dp_core::CoreError;
use p3dp_profiles::filament::{Compatibility, FilamentProfile, Nozzle};
use p3dp_profiles::PrinterProfile;

/// What to do with a part of a layer that is thinner than the perimeters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinRegions {
    /// Leave it out and say so in the warnings of the plan (the default: the points of a cone are never printed with perimeters).
    Skip,
    /// Refuse the slice.
    Refuse,
}

/// Where the model is put on the bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Its box centred on the bed, its lowest point on the bed (the default).
    Center,
    /// Where the model is: its x and y are the coordinates of the bed, and only its lowest point is moved to the bed.
    Origin,
}

/// The pattern of the sparse infill. The solid skins are always rectilinear lines that turn by a right angle on each layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfillPattern {
    /// Parallel lines, turned by a right angle on each layer (the default).
    Rectilinear,
    /// Two sets of parallel lines on every layer, at a right angle to each other.
    Grid,
    /// The wavy lines of a gyroid, a surface that never repeats the same cut on two layers.
    Gyroid,
}

impl InfillPattern {
    pub fn name(self) -> &'static str {
        match self {
            InfillPattern::Rectilinear => "rectilinear",
            InfillPattern::Grid => "grid",
            InfillPattern::Gyroid => "gyroid",
        }
    }
}

/// Where each closed perimeter starts and ends, which is where the seam shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seam {
    /// The corner nearest to where the head is (the default: the shortest travel).
    Nearest,
    /// The corner nearest to the seam of the layer below, so the seams make one line up the part.
    Aligned,
    /// The corner farthest back (the highest y, then the lowest x): the seam is at the back of the part.
    Rear,
    /// The sharpest corner that turns into the part, else the sharpest one: a corner hides the seam better than a flat wall.
    Corner,
}

impl Seam {
    pub fn name(self) -> &'static str {
        match self {
            Seam::Nearest => "nearest",
            Seam::Aligned => "aligned",
            Seam::Rear => "rear",
            Seam::Corner => "corner",
        }
    }
}

/// Whether the overhangs get supports, and where those may stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supports {
    /// No supports (the default): an overhang is printed as it is, and a bridge that is too long is refused.
    Off,
    /// Supports that stand on the bed only; an overhang over the part itself is refused.
    BuildPlate,
    /// Supports that may also stand on the part.
    Everywhere,
}

impl Supports {
    pub fn name(self) -> &'static str {
        match self {
            Supports::Off => "off",
            Supports::BuildPlate => "buildplate",
            Supports::Everywhere => "everywhere",
        }
    }
}

/// What helps the first layer to hold to the bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adhesion {
    /// Nothing (the default).
    None,
    /// Loops round the part, apart from it: they prime the nozzle and show whether the first layer is level.
    Skirt,
    /// Loops that touch the part and widen its first layer.
    Brim,
    /// Layers printed under the part, which stands on them.
    Raft,
}

impl Adhesion {
    pub fn name(self) -> &'static str {
        match self {
            Adhesion::None => "none",
            Adhesion::Skirt => "skirt",
            Adhesion::Brim => "brim",
            Adhesion::Raft => "raft",
        }
    }
}

/// The settings.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceConfig {
    pub layer_height_mm: f64,
    pub first_layer_height_mm: f64,
    pub nozzle_diameter_mm: f64,
    pub line_width_mm: f64,
    pub perimeters: u32,
    pub top_layers: u32,
    pub bottom_layers: u32,
    /// 0 to 100.
    pub infill_percent: f64,
    pub filament_diameter_mm: f64,
    pub extrusion_multiplier: f64,
    pub nozzle_c: f64,
    pub bed_c: f64,
    /// 0 is off.
    pub chamber_c: f64,
    pub fan_percent: f64,
    /// The layer (counted from 0) the part fan starts at.
    pub fan_from_layer: u32,
    pub first_layer_speed_mm_s: f64,
    pub perimeter_speed_mm_s: f64,
    pub external_perimeter_speed_mm_s: f64,
    pub infill_speed_mm_s: f64,
    pub travel_speed_mm_s: f64,
    /// How fast the bed or the head moves in z.
    pub z_speed_mm_s: f64,
    /// No speed of the slice may be above this one.
    pub max_speed_mm_s: f64,
    pub retract_mm: f64,
    pub retract_speed_mm_s: f64,
    /// A travel shorter than this is made without retracting.
    pub retract_min_travel_mm: f64,
    /// How far the head is lifted above the print when it is done (never above the printer).
    pub end_lift_mm: f64,
    pub placement: Placement,
    /// The clearance kept from every edge of the bed.
    pub bed_margin_mm: f64,
    pub thin_regions: ThinRegions,
    pub max_layers: u32,
    /// The most points in the cut of one layer.
    pub max_points_per_layer: u32,
    pub max_gcode_lines: u32,
    pub infill_pattern: InfillPattern,
    pub seam: Seam,
    /// How much narrower than `line_width_mm` the perimeters of a layer may be made, in per cent, when the part is too thin for them at full width (0 is off).
    pub width_variation_percent: f64,
    pub solid_infill_speed_mm_s: f64,
    pub bridge_speed_mm_s: f64,
    /// The plastic of a bridge as a part of a normal line (a bridge is stretched, so a little less).
    pub bridge_flow_ratio: f64,
    /// The part fan on a bridge, 0 to 100.
    pub bridge_fan_percent: f64,
    /// A bridge longer than this is refused when there are no supports (0 is no limit).
    pub max_bridge_span_mm: f64,
    pub supports: Supports,
    /// The distance between the lines of a support, and between the samples that decide where it stands.
    pub support_spacing_mm: f64,
    /// An overhang is left alone while it leans no more than this from the vertical.
    pub support_overhang_angle_deg: f64,
    /// The clearance between a support and the part, sideways.
    pub support_xy_gap_mm: f64,
    /// The layers left empty between the top of a support and the part.
    pub support_z_gap_layers: u32,
    pub support_speed_mm_s: f64,
    /// The most samples (columns times layers) the decision on the supports may take.
    pub max_support_samples: u32,
    /// How far the head is lifted on a travel that retracts (0 is off).
    pub z_hop_mm: f64,
    /// The layers over which the part fan climbs from nothing to `fan_percent` after `fan_from_layer` (0 is at once).
    pub fan_ramp_layers: u32,
    /// The acceleration the time is estimated with.
    pub accel_mm_s2: f64,
    /// The speed at a corner that turns all the way back, for the estimate of the time.
    pub corner_speed_mm_s: f64,
    pub adhesion: Adhesion,
    /// The loops of a skirt.
    pub skirt_loops: u32,
    /// The gap between the part and the first loop of a skirt.
    pub skirt_distance_mm: f64,
    /// How far a brim reaches from the part.
    pub brim_width_mm: f64,
    /// The layers of a raft.
    pub raft_layers: u32,
    /// The extra height between the top of a raft and the part, so the part can be pulled off.
    pub raft_air_gap_mm: f64,
    /// How far a raft reaches beyond the first layer of the part.
    pub raft_margin_mm: f64,
    /// A layer that would take less than this many seconds is slowed down (0 is off).
    pub min_layer_time_s: f64,
    /// The slowing down of a short layer never takes a line below this speed (unless it was slower already).
    pub min_speed_mm_s: f64,
    /// A short travel pulls the filament back less than a long one.
    pub retract_adaptive: bool,
    /// The travel from which the whole `retract_mm` is used.
    pub retract_full_travel_mm: f64,
    /// The part of `retract_mm` that a travel of the shortest kind that retracts uses, in per cent.
    pub retract_min_percent: f64,
    /// One wall, no infill and no top: a vase. The bottom layers stay solid.
    pub vase_mode: bool,
    /// The pressure advance of the extruder, 0 to 2 (0 is off): written at the start of the program in the command of the firmware.
    pub pressure_advance: f64,
    /// The top of the print gets one more pass over its solid skin with very little plastic, to smooth it.
    pub ironing: bool,
    /// The plastic of an ironing pass against a normal line, in per cent.
    pub ironing_flow_percent: f64,
    /// The speed of an ironing pass.
    pub ironing_speed_mm_s: f64,
    /// Layers whose height follows the shape (thin where a wall leans, thick where it is straight up): see `adaptive`.
    pub adaptive_layers: bool,
    /// The thinnest layer of an adaptive stack.
    pub adaptive_min_layer_mm: f64,
    /// The thickest layer of an adaptive stack.
    pub adaptive_max_layer_mm: f64,
    /// The step that a leaning wall may show between two layers (the cusp), in millimetres.
    pub adaptive_cusp_mm: f64,
    /// How much the first layer is drawn in from the outline, against the flared foot of a print, in millimetres.
    pub elephant_foot_mm: f64,
    /// The most plastic the nozzle melts, in cubic millimetres a second (0 is no limit): a line that would ask for more is slowed down.
    pub max_volumetric_flow_mm3_s: f64,
    /// The layers at the top of a support that are laid close together, so the overhang rests on an even floor (0 is off).
    pub support_interface_layers: u32,
    /// The distance between the lines of those layers.
    pub support_interface_spacing_mm: f64,
    /// The speed of the parts of an outer wall that hang past the layer below, with the fan of a bridge (0 is off).
    pub overhang_speed_mm_s: f64,
    /// The nozzle temperature of the first layer (0 is `nozzle_c`).
    pub first_layer_nozzle_c: f64,
    /// The bed temperature of the first layer (0 is `bed_c`).
    pub first_layer_bed_c: f64,
    /// How far the infill and the skins reach into the innermost perimeter, in per cent of the line width, so they hold to it.
    pub infill_overlap_percent: f64,
    /// The outer wall is laid before the inner ones (truer dimensions; the overhangs have less to rest on).
    pub external_perimeters_first: bool,
    /// A travel that stays inside the outline of its layer is made without pulling the filament back.
    pub retract_only_crossing: bool,
    /// How far the head goes back along the line it has just laid while it pulls the filament back (0 is off).
    pub wipe_mm: f64,
    /// The layers, counted from 1, before which the printer pauses for a change of colour or an insert.
    pub pause_at_layers: Vec<u32>,
    /// The G-code says how far the print is and how long is left (`M73`).
    pub progress_marks: bool,
    /// The skirt gets more loops until it is at least this long, to bring the nozzle to a steady flow (0 is off).
    pub skirt_min_length_mm: f64,
}

/// The most layers a print may pause at.
pub const MAX_PAUSES: usize = 20;

/// The layers of `pause_at_layers`: numbers from 2, rising, separated by commas; `none` (or nothing) is no pause.
fn pauses(key: &str, value: &str) -> Result<Vec<u32>, CoreError> {
    let v = value.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut out: Vec<u32> = Vec::new();
    for part in v.split(',') {
        let layer = kv::int_in(key, part.trim(), 2, 100_000)? as u32;
        if out.last().is_some_and(|last| *last >= layer) {
            return Err(invalid(format!("{key}: the layers rise, each once")));
        }
        out.push(layer);
        if out.len() > MAX_PAUSES {
            return Err(invalid(format!("{key}: at most {MAX_PAUSES} pauses")));
        }
    }
    Ok(out)
}

impl Default for SliceConfig {
    fn default() -> SliceConfig {
        SliceConfig {
            layer_height_mm: 0.2,
            first_layer_height_mm: 0.2,
            nozzle_diameter_mm: 0.4,
            line_width_mm: 0.45,
            perimeters: 2,
            top_layers: 4,
            bottom_layers: 4,
            infill_percent: 20.0,
            filament_diameter_mm: 1.75,
            extrusion_multiplier: 1.0,
            nozzle_c: 210.0,
            bed_c: 60.0,
            chamber_c: 0.0,
            fan_percent: 100.0,
            fan_from_layer: 3,
            first_layer_speed_mm_s: 20.0,
            perimeter_speed_mm_s: 40.0,
            external_perimeter_speed_mm_s: 30.0,
            infill_speed_mm_s: 60.0,
            travel_speed_mm_s: 150.0,
            z_speed_mm_s: 10.0,
            max_speed_mm_s: 200.0,
            retract_mm: 1.0,
            retract_speed_mm_s: 30.0,
            retract_min_travel_mm: 2.0,
            end_lift_mm: 10.0,
            placement: Placement::Center,
            bed_margin_mm: 0.0,
            thin_regions: ThinRegions::Skip,
            max_layers: 5000,
            max_points_per_layer: 200_000,
            max_gcode_lines: 5_000_000,
            infill_pattern: InfillPattern::Rectilinear,
            seam: Seam::Nearest,
            width_variation_percent: 0.0,
            solid_infill_speed_mm_s: 50.0,
            bridge_speed_mm_s: 25.0,
            bridge_flow_ratio: 0.95,
            bridge_fan_percent: 100.0,
            max_bridge_span_mm: 100.0,
            supports: Supports::Off,
            support_spacing_mm: 3.0,
            support_overhang_angle_deg: 50.0,
            support_xy_gap_mm: 0.4,
            support_z_gap_layers: 1,
            support_speed_mm_s: 50.0,
            max_support_samples: 5_000_000,
            z_hop_mm: 0.0,
            fan_ramp_layers: 0,
            accel_mm_s2: 3000.0,
            corner_speed_mm_s: 8.0,
            adhesion: Adhesion::None,
            skirt_loops: 2,
            skirt_distance_mm: 3.0,
            brim_width_mm: 5.0,
            raft_layers: 2,
            raft_air_gap_mm: 0.1,
            raft_margin_mm: 3.0,
            min_layer_time_s: 0.0,
            min_speed_mm_s: 10.0,
            retract_adaptive: false,
            retract_full_travel_mm: 20.0,
            retract_min_percent: 30.0,
            vase_mode: false,
            pressure_advance: 0.0,
            ironing: false,
            ironing_flow_percent: 10.0,
            ironing_speed_mm_s: 20.0,
            adaptive_layers: false,
            adaptive_min_layer_mm: 0.08,
            adaptive_max_layer_mm: 0.28,
            adaptive_cusp_mm: 0.05,
            elephant_foot_mm: 0.0,
            max_volumetric_flow_mm3_s: 0.0,
            support_interface_layers: 0,
            support_interface_spacing_mm: 0.8,
            overhang_speed_mm_s: 0.0,
            first_layer_nozzle_c: 0.0,
            first_layer_bed_c: 0.0,
            infill_overlap_percent: 0.0,
            external_perimeters_first: false,
            retract_only_crossing: false,
            wipe_mm: 0.0,
            pause_at_layers: Vec::new(),
            progress_marks: true,
            skirt_min_length_mm: 0.0,
        }
    }
}

/// The keys, in the order of the file.
pub const KEYS: [&str; 86] = [
    "layer_height_mm",
    "first_layer_height_mm",
    "nozzle_diameter_mm",
    "line_width_mm",
    "perimeters",
    "top_layers",
    "bottom_layers",
    "infill_percent",
    "filament_diameter_mm",
    "extrusion_multiplier",
    "nozzle_c",
    "bed_c",
    "chamber_c",
    "fan_percent",
    "fan_from_layer",
    "first_layer_speed_mm_s",
    "perimeter_speed_mm_s",
    "external_perimeter_speed_mm_s",
    "infill_speed_mm_s",
    "travel_speed_mm_s",
    "z_speed_mm_s",
    "max_speed_mm_s",
    "retract_mm",
    "retract_speed_mm_s",
    "retract_min_travel_mm",
    "end_lift_mm",
    "placement",
    "bed_margin_mm",
    "thin_regions",
    "max_layers",
    "max_points_per_layer",
    "max_gcode_lines",
    "infill_pattern",
    "seam",
    "width_variation_percent",
    "solid_infill_speed_mm_s",
    "bridge_speed_mm_s",
    "bridge_flow_ratio",
    "bridge_fan_percent",
    "max_bridge_span_mm",
    "supports",
    "support_spacing_mm",
    "support_overhang_angle_deg",
    "support_xy_gap_mm",
    "support_z_gap_layers",
    "support_speed_mm_s",
    "max_support_samples",
    "z_hop_mm",
    "fan_ramp_layers",
    "accel_mm_s2",
    "corner_speed_mm_s",
    "adhesion",
    "skirt_loops",
    "skirt_distance_mm",
    "brim_width_mm",
    "raft_layers",
    "raft_air_gap_mm",
    "raft_margin_mm",
    "min_layer_time_s",
    "min_speed_mm_s",
    "retract_adaptive",
    "retract_full_travel_mm",
    "retract_min_percent",
    "vase_mode",
    "pressure_advance",
    "ironing",
    "ironing_flow_percent",
    "ironing_speed_mm_s",
    "adaptive_layers",
    "adaptive_min_layer_mm",
    "adaptive_max_layer_mm",
    "adaptive_cusp_mm",
    "elephant_foot_mm",
    "max_volumetric_flow_mm3_s",
    "support_interface_layers",
    "support_interface_spacing_mm",
    "overhang_speed_mm_s",
    "first_layer_nozzle_c",
    "first_layer_bed_c",
    "infill_overlap_percent",
    "external_perimeters_first",
    "retract_only_crossing",
    "wipe_mm",
    "pause_at_layers",
    "progress_marks",
    "skirt_min_length_mm",
];

fn invalid(msg: String) -> CoreError {
    CoreError::Invalid(msg)
}

fn on_off(v: bool) -> String {
    if v { "on" } else { "off" }.to_string()
}

impl SliceConfig {
    /// Sets one key. An unknown key is an error (all the keys of the file are the slicer's); a value out of its range is an error that names the key and changes nothing.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), CoreError> {
        let f = |lo: f64, hi: f64| kv::float_in(key, value, lo, hi);
        let n = |lo: i64, hi: i64| kv::int_in(key, value, lo, hi);
        match key {
            "layer_height_mm" => self.layer_height_mm = f(0.05, 1.0)?,
            "first_layer_height_mm" => self.first_layer_height_mm = f(0.05, 1.0)?,
            "nozzle_diameter_mm" => self.nozzle_diameter_mm = f(0.1, 2.0)?,
            "line_width_mm" => self.line_width_mm = f(0.1, 2.5)?,
            "perimeters" => self.perimeters = n(1, 10)? as u32,
            "top_layers" => self.top_layers = n(0, 50)? as u32,
            "bottom_layers" => self.bottom_layers = n(0, 50)? as u32,
            "infill_percent" => self.infill_percent = f(0.0, 100.0)?,
            "filament_diameter_mm" => self.filament_diameter_mm = f(1.0, 3.5)?,
            "extrusion_multiplier" => self.extrusion_multiplier = f(0.5, 1.5)?,
            "nozzle_c" => self.nozzle_c = f(0.0, 500.0)?,
            "bed_c" => self.bed_c = f(0.0, 200.0)?,
            "chamber_c" => self.chamber_c = f(0.0, 120.0)?,
            "fan_percent" => self.fan_percent = f(0.0, 100.0)?,
            "fan_from_layer" => self.fan_from_layer = n(0, 100_000)? as u32,
            "first_layer_speed_mm_s" => self.first_layer_speed_mm_s = f(1.0, 1000.0)?,
            "perimeter_speed_mm_s" => self.perimeter_speed_mm_s = f(1.0, 1000.0)?,
            "external_perimeter_speed_mm_s" => self.external_perimeter_speed_mm_s = f(1.0, 1000.0)?,
            "infill_speed_mm_s" => self.infill_speed_mm_s = f(1.0, 1000.0)?,
            "travel_speed_mm_s" => self.travel_speed_mm_s = f(1.0, 1000.0)?,
            "z_speed_mm_s" => self.z_speed_mm_s = f(0.1, 1000.0)?,
            "max_speed_mm_s" => self.max_speed_mm_s = f(1.0, 1000.0)?,
            "retract_mm" => self.retract_mm = f(0.0, 20.0)?,
            "retract_speed_mm_s" => self.retract_speed_mm_s = f(1.0, 200.0)?,
            "retract_min_travel_mm" => self.retract_min_travel_mm = f(0.0, 100.0)?,
            "end_lift_mm" => self.end_lift_mm = f(0.0, 100.0)?,
            "placement" => {
                self.placement = match value.trim().to_ascii_lowercase().as_str() {
                    "center" | "centre" => Placement::Center,
                    "origin" => Placement::Origin,
                    _ => return Err(invalid(format!("{key}: expected center or origin"))),
                }
            }
            "bed_margin_mm" => self.bed_margin_mm = f(0.0, 100.0)?,
            "thin_regions" => {
                self.thin_regions = match value.trim().to_ascii_lowercase().as_str() {
                    "skip" => ThinRegions::Skip,
                    "refuse" => ThinRegions::Refuse,
                    _ => return Err(invalid(format!("{key}: expected skip or refuse"))),
                }
            }
            "max_layers" => self.max_layers = n(1, 100_000)? as u32,
            "max_points_per_layer" => self.max_points_per_layer = n(100, 10_000_000)? as u32,
            "max_gcode_lines" => self.max_gcode_lines = n(1_000, 100_000_000)? as u32,
            "infill_pattern" => {
                self.infill_pattern = match value.trim().to_ascii_lowercase().as_str() {
                    "rectilinear" => InfillPattern::Rectilinear,
                    "grid" => InfillPattern::Grid,
                    "gyroid" => InfillPattern::Gyroid,
                    _ => {
                        return Err(invalid(format!(
                            "{key}: expected rectilinear, grid or gyroid"
                        )))
                    }
                }
            }
            "seam" => {
                self.seam = match value.trim().to_ascii_lowercase().as_str() {
                    "nearest" => Seam::Nearest,
                    "aligned" => Seam::Aligned,
                    "rear" => Seam::Rear,
                    "corner" => Seam::Corner,
                    _ => return Err(invalid(format!("{key}: expected nearest, aligned or rear"))),
                }
            }
            "width_variation_percent" => self.width_variation_percent = f(0.0, 50.0)?,
            "solid_infill_speed_mm_s" => self.solid_infill_speed_mm_s = f(1.0, 1000.0)?,
            "bridge_speed_mm_s" => self.bridge_speed_mm_s = f(1.0, 1000.0)?,
            "bridge_flow_ratio" => self.bridge_flow_ratio = f(0.5, 1.5)?,
            "bridge_fan_percent" => self.bridge_fan_percent = f(0.0, 100.0)?,
            "max_bridge_span_mm" => self.max_bridge_span_mm = f(0.0, 1000.0)?,
            "supports" => {
                self.supports = match value.trim().to_ascii_lowercase().as_str() {
                    "off" => Supports::Off,
                    "buildplate" => Supports::BuildPlate,
                    "everywhere" => Supports::Everywhere,
                    _ => {
                        return Err(invalid(format!(
                            "{key}: expected off, buildplate or everywhere"
                        )))
                    }
                }
            }
            "support_spacing_mm" => self.support_spacing_mm = f(1.0, 20.0)?,
            "support_overhang_angle_deg" => self.support_overhang_angle_deg = f(1.0, 89.0)?,
            "support_xy_gap_mm" => self.support_xy_gap_mm = f(0.0, 5.0)?,
            "support_z_gap_layers" => self.support_z_gap_layers = n(0, 10)? as u32,
            "support_speed_mm_s" => self.support_speed_mm_s = f(1.0, 1000.0)?,
            "max_support_samples" => self.max_support_samples = n(1_000, 2_000_000_000)? as u32,
            "z_hop_mm" => self.z_hop_mm = f(0.0, 10.0)?,
            "fan_ramp_layers" => self.fan_ramp_layers = n(0, 1_000)? as u32,
            "accel_mm_s2" => self.accel_mm_s2 = f(50.0, 100_000.0)?,
            "corner_speed_mm_s" => self.corner_speed_mm_s = f(0.5, 100.0)?,
            "adhesion" => {
                self.adhesion = match value.trim().to_ascii_lowercase().as_str() {
                    "none" => Adhesion::None,
                    "skirt" => Adhesion::Skirt,
                    "brim" => Adhesion::Brim,
                    "raft" => Adhesion::Raft,
                    _ => {
                        return Err(invalid(format!(
                            "{key}: expected none, skirt, brim or raft"
                        )))
                    }
                }
            }
            "skirt_loops" => self.skirt_loops = n(1, 20)? as u32,
            "skirt_distance_mm" => self.skirt_distance_mm = f(0.0, 30.0)?,
            "brim_width_mm" => self.brim_width_mm = f(0.0, 30.0)?,
            "raft_layers" => self.raft_layers = n(1, 10)? as u32,
            "raft_air_gap_mm" => self.raft_air_gap_mm = f(0.0, 1.0)?,
            "raft_margin_mm" => self.raft_margin_mm = f(0.0, 20.0)?,
            "min_layer_time_s" => self.min_layer_time_s = f(0.0, 600.0)?,
            "min_speed_mm_s" => self.min_speed_mm_s = f(1.0, 1000.0)?,
            "retract_adaptive" => self.retract_adaptive = kv::on_off(key, value)?,
            "retract_full_travel_mm" => self.retract_full_travel_mm = f(1.0, 500.0)?,
            "retract_min_percent" => self.retract_min_percent = f(0.0, 100.0)?,
            "vase_mode" => self.vase_mode = kv::on_off(key, value)?,
            "pressure_advance" => self.pressure_advance = f(0.0, 2.0)?,
            "ironing" => self.ironing = kv::on_off(key, value)?,
            "ironing_flow_percent" => self.ironing_flow_percent = f(1.0, 30.0)?,
            "ironing_speed_mm_s" => self.ironing_speed_mm_s = f(1.0, 1000.0)?,
            "adaptive_layers" => self.adaptive_layers = kv::on_off(key, value)?,
            "adaptive_min_layer_mm" => self.adaptive_min_layer_mm = f(0.04, 1.0)?,
            "adaptive_max_layer_mm" => self.adaptive_max_layer_mm = f(0.04, 1.0)?,
            "adaptive_cusp_mm" => self.adaptive_cusp_mm = f(0.005, 0.5)?,
            "elephant_foot_mm" => self.elephant_foot_mm = f(0.0, 2.0)?,
            "max_volumetric_flow_mm3_s" => self.max_volumetric_flow_mm3_s = f(0.0, 100.0)?,
            "support_interface_layers" => self.support_interface_layers = n(0, 10)? as u32,
            "support_interface_spacing_mm" => self.support_interface_spacing_mm = f(0.2, 5.0)?,
            "overhang_speed_mm_s" => self.overhang_speed_mm_s = f(0.0, 1000.0)?,
            "first_layer_nozzle_c" => self.first_layer_nozzle_c = f(0.0, 500.0)?,
            "first_layer_bed_c" => self.first_layer_bed_c = f(0.0, 200.0)?,
            "infill_overlap_percent" => self.infill_overlap_percent = f(0.0, 50.0)?,
            "external_perimeters_first" => self.external_perimeters_first = kv::on_off(key, value)?,
            "retract_only_crossing" => self.retract_only_crossing = kv::on_off(key, value)?,
            "wipe_mm" => self.wipe_mm = f(0.0, 10.0)?,
            "pause_at_layers" => self.pause_at_layers = pauses(key, value)?,
            "progress_marks" => self.progress_marks = kv::on_off(key, value)?,
            "skirt_min_length_mm" => self.skirt_min_length_mm = f(0.0, 10_000.0)?,
            other => return Err(invalid(format!("{other}: not a setting of the slicer"))),
        }
        Ok(())
    }

    /// Reads a file of `key = value` lines; the error names the line.
    pub fn from_text(text: &str) -> Result<SliceConfig, CoreError> {
        let mut c = SliceConfig::default();
        for e in kv::parse(text)? {
            c.set(&e.key, &e.value)
                .map_err(|err| invalid(format!("line {}: {err}", e.line)))?;
        }
        c.check().map_err(|e| invalid(e.to_string()))?;
        Ok(c)
    }

    /// The value of a key as it is written in the file; `None` for a key that is not one.
    pub fn value_of(&self, key: &str) -> Option<String> {
        let v = match key {
            "layer_height_mm" => self.layer_height_mm.to_string(),
            "first_layer_height_mm" => self.first_layer_height_mm.to_string(),
            "nozzle_diameter_mm" => self.nozzle_diameter_mm.to_string(),
            "line_width_mm" => self.line_width_mm.to_string(),
            "perimeters" => self.perimeters.to_string(),
            "top_layers" => self.top_layers.to_string(),
            "bottom_layers" => self.bottom_layers.to_string(),
            "infill_percent" => self.infill_percent.to_string(),
            "filament_diameter_mm" => self.filament_diameter_mm.to_string(),
            "extrusion_multiplier" => self.extrusion_multiplier.to_string(),
            "nozzle_c" => self.nozzle_c.to_string(),
            "bed_c" => self.bed_c.to_string(),
            "chamber_c" => self.chamber_c.to_string(),
            "fan_percent" => self.fan_percent.to_string(),
            "fan_from_layer" => self.fan_from_layer.to_string(),
            "first_layer_speed_mm_s" => self.first_layer_speed_mm_s.to_string(),
            "perimeter_speed_mm_s" => self.perimeter_speed_mm_s.to_string(),
            "external_perimeter_speed_mm_s" => self.external_perimeter_speed_mm_s.to_string(),
            "infill_speed_mm_s" => self.infill_speed_mm_s.to_string(),
            "travel_speed_mm_s" => self.travel_speed_mm_s.to_string(),
            "z_speed_mm_s" => self.z_speed_mm_s.to_string(),
            "max_speed_mm_s" => self.max_speed_mm_s.to_string(),
            "retract_mm" => self.retract_mm.to_string(),
            "retract_speed_mm_s" => self.retract_speed_mm_s.to_string(),
            "retract_min_travel_mm" => self.retract_min_travel_mm.to_string(),
            "end_lift_mm" => self.end_lift_mm.to_string(),
            "bed_margin_mm" => self.bed_margin_mm.to_string(),
            "max_layers" => self.max_layers.to_string(),
            "max_points_per_layer" => self.max_points_per_layer.to_string(),
            "max_gcode_lines" => self.max_gcode_lines.to_string(),
            "width_variation_percent" => self.width_variation_percent.to_string(),
            "solid_infill_speed_mm_s" => self.solid_infill_speed_mm_s.to_string(),
            "bridge_speed_mm_s" => self.bridge_speed_mm_s.to_string(),
            "bridge_flow_ratio" => self.bridge_flow_ratio.to_string(),
            "bridge_fan_percent" => self.bridge_fan_percent.to_string(),
            "max_bridge_span_mm" => self.max_bridge_span_mm.to_string(),
            "support_spacing_mm" => self.support_spacing_mm.to_string(),
            "support_overhang_angle_deg" => self.support_overhang_angle_deg.to_string(),
            "support_xy_gap_mm" => self.support_xy_gap_mm.to_string(),
            "support_z_gap_layers" => self.support_z_gap_layers.to_string(),
            "support_speed_mm_s" => self.support_speed_mm_s.to_string(),
            "max_support_samples" => self.max_support_samples.to_string(),
            "z_hop_mm" => self.z_hop_mm.to_string(),
            "fan_ramp_layers" => self.fan_ramp_layers.to_string(),
            "accel_mm_s2" => self.accel_mm_s2.to_string(),
            "corner_speed_mm_s" => self.corner_speed_mm_s.to_string(),
            "skirt_loops" => self.skirt_loops.to_string(),
            "skirt_distance_mm" => self.skirt_distance_mm.to_string(),
            "brim_width_mm" => self.brim_width_mm.to_string(),
            "raft_layers" => self.raft_layers.to_string(),
            "raft_air_gap_mm" => self.raft_air_gap_mm.to_string(),
            "raft_margin_mm" => self.raft_margin_mm.to_string(),
            "min_layer_time_s" => self.min_layer_time_s.to_string(),
            "min_speed_mm_s" => self.min_speed_mm_s.to_string(),
            "retract_full_travel_mm" => self.retract_full_travel_mm.to_string(),
            "retract_min_percent" => self.retract_min_percent.to_string(),
            "retract_adaptive" => on_off(self.retract_adaptive),
            "vase_mode" => on_off(self.vase_mode),
            "pressure_advance" => self.pressure_advance.to_string(),
            "ironing" => on_off(self.ironing),
            "ironing_flow_percent" => self.ironing_flow_percent.to_string(),
            "ironing_speed_mm_s" => self.ironing_speed_mm_s.to_string(),
            "adaptive_layers" => on_off(self.adaptive_layers),
            "adaptive_min_layer_mm" => self.adaptive_min_layer_mm.to_string(),
            "adaptive_max_layer_mm" => self.adaptive_max_layer_mm.to_string(),
            "adaptive_cusp_mm" => self.adaptive_cusp_mm.to_string(),
            "elephant_foot_mm" => self.elephant_foot_mm.to_string(),
            "max_volumetric_flow_mm3_s" => self.max_volumetric_flow_mm3_s.to_string(),
            "support_interface_layers" => self.support_interface_layers.to_string(),
            "support_interface_spacing_mm" => self.support_interface_spacing_mm.to_string(),
            "overhang_speed_mm_s" => self.overhang_speed_mm_s.to_string(),
            "first_layer_nozzle_c" => self.first_layer_nozzle_c.to_string(),
            "first_layer_bed_c" => self.first_layer_bed_c.to_string(),
            "infill_overlap_percent" => self.infill_overlap_percent.to_string(),
            "external_perimeters_first" => on_off(self.external_perimeters_first),
            "retract_only_crossing" => on_off(self.retract_only_crossing),
            "wipe_mm" => self.wipe_mm.to_string(),
            "pause_at_layers" if self.pause_at_layers.is_empty() => "none".to_string(),
            "pause_at_layers" => self
                .pause_at_layers
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
            "progress_marks" => on_off(self.progress_marks),
            "skirt_min_length_mm" => self.skirt_min_length_mm.to_string(),
            "adhesion" => self.adhesion.name().to_string(),
            "placement" => match self.placement {
                Placement::Center => "center",
                Placement::Origin => "origin",
            }
            .to_string(),
            "thin_regions" => match self.thin_regions {
                ThinRegions::Skip => "skip",
                ThinRegions::Refuse => "refuse",
            }
            .to_string(),
            "infill_pattern" => self.infill_pattern.name().to_string(),
            "seam" => self.seam.name().to_string(),
            "supports" => self.supports.name().to_string(),
            _ => return None,
        };
        Some(v)
    }

    /// The settings as text, every key with its value (`from_text` reads it back).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for k in KEYS {
            if let Some(v) = self.value_of(k) {
                out.push_str(k);
                out.push_str(" = ");
                out.push_str(&v);
                out.push('\n');
            }
        }
        out
    }

    /// Checks the settings against each other. Nothing is changed.
    pub fn check(&self) -> Result<(), SliceError> {
        let bad = |s: String| Err(SliceError::Config(s));
        let max_h = self.nozzle_diameter_mm * 0.8;
        for (name, h) in [
            ("layer_height_mm", self.layer_height_mm),
            ("first_layer_height_mm", self.first_layer_height_mm),
        ] {
            if h > max_h + 1e-9 {
                return bad(format!(
                    "{name} {h} is above 80% of the nozzle ({max_h:.3}): the plastic would not be pressed into the layer below"
                ));
            }
        }
        if self.line_width_mm < self.nozzle_diameter_mm * 0.8 - 1e-9
            || self.line_width_mm > self.nozzle_diameter_mm * 2.0 + 1e-9
        {
            return bad(format!(
                "line_width_mm {} must be from 80% to 200% of the nozzle ({})",
                self.line_width_mm, self.nozzle_diameter_mm
            ));
        }
        if self.line_width_mm < self.layer_height_mm
            || self.line_width_mm < self.first_layer_height_mm
        {
            return bad("line_width_mm must not be smaller than a layer height".to_string());
        }
        for (name, v) in [
            ("first_layer_speed_mm_s", self.first_layer_speed_mm_s),
            ("perimeter_speed_mm_s", self.perimeter_speed_mm_s),
            (
                "external_perimeter_speed_mm_s",
                self.external_perimeter_speed_mm_s,
            ),
            ("infill_speed_mm_s", self.infill_speed_mm_s),
            ("travel_speed_mm_s", self.travel_speed_mm_s),
            ("z_speed_mm_s", self.z_speed_mm_s),
            ("retract_speed_mm_s", self.retract_speed_mm_s),
            ("solid_infill_speed_mm_s", self.solid_infill_speed_mm_s),
            ("bridge_speed_mm_s", self.bridge_speed_mm_s),
            ("support_speed_mm_s", self.support_speed_mm_s),
        ] {
            if v > self.max_speed_mm_s {
                return bad(format!(
                    "{name} {v} is above max_speed_mm_s {}",
                    self.max_speed_mm_s
                ));
            }
        }
        if self.overhang_speed_mm_s > self.max_speed_mm_s {
            return bad(format!(
                "overhang_speed_mm_s {} is above max_speed_mm_s {}",
                self.overhang_speed_mm_s, self.max_speed_mm_s
            ));
        }
        if self.overhang_speed_mm_s > 0.0 && self.overhang_speed_mm_s < 1.0 {
            return bad("overhang_speed_mm_s is 0 (off) or at least 1".to_string());
        }
        if self.support_interface_layers > 0
            && self.support_interface_spacing_mm < self.line_width_mm - 1e-9
        {
            return bad(format!(
                "support_interface_spacing_mm {} is narrower than a line ({})",
                self.support_interface_spacing_mm, self.line_width_mm
            ));
        }
        let narrowest = self.line_width_mm * (1.0 - self.width_variation_percent / 100.0);
        if self.width_variation_percent > 0.0
            && (narrowest < self.nozzle_diameter_mm * 0.6 - 1e-9
                || narrowest < self.layer_height_mm
                || narrowest < self.first_layer_height_mm)
        {
            return bad(format!(
                "width_variation_percent {} would make a line {narrowest:.3} mm wide, narrower than 60% of the nozzle or than a layer",
                self.width_variation_percent
            ));
        }
        if self.supports != Supports::Off && self.support_spacing_mm < self.line_width_mm {
            return bad(format!(
                "support_spacing_mm {} is narrower than a line ({})",
                self.support_spacing_mm, self.line_width_mm
            ));
        }
        if self.z_hop_mm > 0.0 && self.z_hop_mm < self.layer_height_mm * 0.5 {
            return bad("z_hop_mm is under half a layer: it would not clear the print".to_string());
        }
        if self.adhesion == Adhesion::Brim && self.brim_width_mm < self.line_width_mm {
            return bad(format!(
                "brim_width_mm {} is narrower than a line ({}): a brim of that width would be nothing",
                self.brim_width_mm, self.line_width_mm
            ));
        }
        if self.min_layer_time_s > 0.0 && self.min_speed_mm_s > self.max_speed_mm_s {
            return bad(format!(
                "min_speed_mm_s {} is above max_speed_mm_s {}",
                self.min_speed_mm_s, self.max_speed_mm_s
            ));
        }
        if self.vase_mode && self.supports != Supports::Off {
            return bad(
                "a vase is one wall with nothing inside: it cannot have supports (supports = off)"
                    .to_string(),
            );
        }
        if self.vase_mode && self.adhesion == Adhesion::Raft {
            return bad("a vase cannot stand on a raft: use a skirt or a brim".to_string());
        }
        if self.ironing {
            if self.vase_mode {
                return bad("a vase has no solid top to iron: ironing = off".to_string());
            }
            if self.top_layers == 0 {
                return bad(
                    "ironing smooths the solid skin of the top: top_layers is 0".to_string()
                );
            }
            if self.ironing_speed_mm_s > self.max_speed_mm_s {
                return bad(format!(
                    "ironing_speed_mm_s {} is above max_speed_mm_s {}",
                    self.ironing_speed_mm_s, self.max_speed_mm_s
                ));
            }
        }
        if self.adaptive_layers {
            if self.adaptive_min_layer_mm > self.adaptive_max_layer_mm + 1e-9 {
                return bad(format!(
                    "adaptive_min_layer_mm {} is above adaptive_max_layer_mm {}",
                    self.adaptive_min_layer_mm, self.adaptive_max_layer_mm
                ));
            }
            if self.adaptive_max_layer_mm > max_h + 1e-9 {
                return bad(format!(
                    "adaptive_max_layer_mm {} is above 80% of the nozzle ({max_h:.3}): the plastic would not be pressed into the layer below",
                    self.adaptive_max_layer_mm
                ));
            }
            if self.vase_mode {
                return bad(
                    "a vase is one wall of layers of the same height: adaptive_layers = off"
                        .to_string(),
                );
            }
        }
        if self.max_volumetric_flow_mm3_s > 0.0 && self.max_volumetric_flow_mm3_s < 1.0 {
            return bad(format!(
                "max_volumetric_flow_mm3_s {} is under 1: 0 is no limit, and a hot end melts more than that",
                self.max_volumetric_flow_mm3_s
            ));
        }
        if self.elephant_foot_mm > 0.0
            && self.elephant_foot_mm >= self.line_width_mm * self.perimeters as f64
        {
            return bad(format!(
                "elephant_foot_mm {} would eat the whole wall ({} lines of {} mm)",
                self.elephant_foot_mm, self.perimeters, self.line_width_mm
            ));
        }
        Ok(())
    }

    /// Sets the temperatures, the fan, the diameter of the filament and the nozzle from a filament profile, for a printer and a nozzle. A pair that does not fit (a temperature the
    /// printer cannot reach, a nozzle it does not have, a nozzle too small or too soft) is an error that lists the reasons and changes nothing, and so are settings that no longer
    /// go together once the nozzle is changed (the layer height and the line width are the person's to change). Returns the advice that does not stop the slice.
    pub fn apply_filament(
        &mut self,
        filament: &FilamentProfile,
        printer: &PrinterProfile,
        nozzle: Nozzle,
    ) -> Result<Compatibility, SliceError> {
        let compat = filament.check(printer, nozzle);
        if !compat.ok() {
            let reasons: Vec<String> = compat.issues.iter().map(|i| i.text()).collect();
            return Err(SliceError::Config(format!(
                "{} does not go with {}: {}",
                filament.name,
                printer.title(),
                reasons.join("; ")
            )));
        }
        let (Some(nozzle_c), Some(bed_c)) = (compat.nozzle_c, compat.bed_c) else {
            return Err(SliceError::Config(
                "the filament gave no temperatures".to_string(),
            ));
        };
        let mut next = self.clone();
        next.nozzle_c = nozzle_c;
        next.bed_c = bed_c;
        next.fan_percent = compat.fan_percent;
        next.filament_diameter_mm = filament.diameter_mm;
        next.nozzle_diameter_mm = nozzle.diameter_mm;
        next.check()?;
        *self = next;
        Ok(compat)
    }

    /// The cross-section of a line of a layer of this height, in square millimetres: a rectangle with rounded ends, as the plastic lies when it is pressed.
    pub fn line_area_mm2(&self, layer_height: f64) -> f64 {
        self.line_area_for(self.line_width_mm, layer_height)
    }

    /// The same for a line of another width.
    pub fn line_area_for(&self, width: f64, layer_height: f64) -> f64 {
        let h = layer_height;
        h * (width - h) + std::f64::consts::PI * h * h / 4.0
    }

    /// The distance between the centres of two lines of another width that touch.
    pub fn spacing_for(&self, width: f64, layer_height: f64) -> f64 {
        self.line_area_for(width, layer_height) / layer_height
    }

    /// The distance between the centres of two lines that touch, for a layer of this height.
    pub fn spacing_mm(&self, layer_height: f64) -> f64 {
        self.line_area_mm2(layer_height) / layer_height
    }

    /// The area of the section of the filament, in square millimetres.
    pub fn filament_area_mm2(&self) -> f64 {
        std::f64::consts::PI * (self.filament_diameter_mm / 2.0).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_the_text_reads_back() {
        let c = SliceConfig::default();
        assert!(c.check().is_ok());
        let back = SliceConfig::from_text(&c.to_text()).unwrap();
        assert_eq!(back, c);
        let again = SliceConfig::from_text(&back.to_text()).unwrap();
        assert_eq!(again, c);
    }

    #[test]
    fn every_key_of_the_list_is_a_key_and_the_file_writes_all_of_them() {
        let mut c = SliceConfig::default();
        let text = c.to_text();
        let written: Vec<&str> = text
            .lines()
            .map(|l| l.split(" = ").next().unwrap())
            .collect();
        for k in KEYS {
            assert!(written.contains(&k), "{k} is not written");
        }
        assert_eq!(written.len(), KEYS.len());
        assert!(c.set("nope", "1").is_err());
    }

    #[test]
    fn a_value_out_of_its_range_is_an_error_that_changes_nothing() {
        let mut c = SliceConfig::default();
        for (k, v) in [
            ("layer_height_mm", "0.01"),
            ("layer_height_mm", "abc"),
            ("layer_height_mm", "nan"),
            ("perimeters", "0"),
            ("perimeters", "1.5"),
            ("infill_percent", "101"),
            ("infill_percent", "-1"),
            ("nozzle_c", "501"),
            ("fan_percent", "100.1"),
            ("placement", "left"),
            ("thin_regions", "maybe"),
            ("max_layers", "0"),
        ] {
            assert!(c.set(k, v).is_err(), "{k}={v}");
        }
        assert_eq!(c, SliceConfig::default());
    }

    #[test]
    fn settings_that_do_not_go_together_are_refused_not_adjusted() {
        let mut c = SliceConfig::default();
        c.set("layer_height_mm", "0.4").unwrap();
        assert!(
            c.check().is_err(),
            "0.4 mm layers are too tall for a 0.4 mm nozzle"
        );
        let mut c = SliceConfig::default();
        c.set("line_width_mm", "0.2").unwrap();
        assert!(c.check().is_err());
        let mut c = SliceConfig::default();
        c.set("infill_speed_mm_s", "300").unwrap();
        assert!(c.check().is_err(), "above max_speed_mm_s");
        c.set("max_speed_mm_s", "300").unwrap();
        assert!(c.check().is_ok());
    }

    #[test]
    fn the_new_settings_read_back_and_refuse_what_does_not_go_together() {
        let mut c = SliceConfig::default();
        for (k, v) in [
            ("infill_pattern", "gyroid"),
            ("seam", "rear"),
            ("supports", "buildplate"),
            ("width_variation_percent", "20"),
            ("z_hop_mm", "0.4"),
            ("fan_ramp_layers", "5"),
        ] {
            c.set(k, v).unwrap();
        }
        assert!(c.check().is_ok());
        let back = SliceConfig::from_text(&c.to_text()).unwrap();
        assert_eq!(back, c);
        assert_eq!(c.value_of("infill_pattern").as_deref(), Some("gyroid"));
        assert_eq!(c.value_of("nope"), None);
        for (k, v) in [
            ("infill_pattern", "honeycomb"),
            ("seam", "random"),
            ("supports", "tree"),
            ("support_overhang_angle_deg", "90"),
            ("z_hop_mm", "-1"),
            ("bridge_flow_ratio", "2"),
        ] {
            assert!(c.set(k, v).is_err(), "{k}={v}");
        }
        // lines that would be too thin, a support narrower than a line, a hop too small to clear anything
        let mut d = SliceConfig::default();
        d.set("width_variation_percent", "50").unwrap();
        assert!(d.check().is_err());
        let mut d = SliceConfig::default();
        d.set("supports", "everywhere").unwrap();
        d.set("support_spacing_mm", "0.2").unwrap_err();
        d.set("nozzle_diameter_mm", "2").unwrap();
        d.set("line_width_mm", "2").unwrap();
        d.set("support_spacing_mm", "1").unwrap();
        assert!(d.check().is_err());
        let mut d = SliceConfig::default();
        d.set("z_hop_mm", "0.05").unwrap();
        assert!(d.check().is_err());
        let mut d = SliceConfig::default();
        d.set("bridge_speed_mm_s", "250").unwrap();
        assert!(d.check().is_err(), "above max_speed_mm_s");
    }

    #[test]
    fn a_filament_sets_the_temperatures_and_a_pair_that_does_not_fit_changes_nothing() {
        use p3dp_profiles::{Catalog, FilamentCatalog};
        let cat = Catalog::builtin().unwrap();
        let fil = FilamentCatalog::builtin().unwrap();
        let a1 = cat.find("bambu-lab-a1").unwrap();
        let n = |d| Nozzle {
            diameter_mm: d,
            hardened: false,
        };
        let mut c = SliceConfig::default();
        let petg = fil.find("generic-petg").unwrap();
        let advice = c.apply_filament(petg, a1, n(0.4)).unwrap();
        assert!(advice.advice.is_empty());
        assert_eq!((c.nozzle_c, c.bed_c, c.fan_percent), (235.0, 80.0, 40.0));
        // the temperatures it sets are within the limits of the printer
        let l = a1.limits.unwrap();
        assert!(c.nozzle_c <= l.nozzle_c && c.bed_c <= l.bed_c);
        // a bed that stops at 80 does not print ABS: the error says why and the settings stay as they were
        let mini = cat.find("bambu-lab-a1-mini").unwrap();
        let before = c.clone();
        let abs = fil.find("generic-abs").unwrap();
        let e = c.apply_filament(abs, mini, n(0.4)).unwrap_err().to_string();
        assert!(e.contains("Generic ABS") && e.contains("bed"), "{e}");
        assert_eq!(c, before);
        // a nozzle the printer lists but whose size does not go with the layers of the settings is refused too
        let e = c.apply_filament(petg, a1, n(0.2)).unwrap_err().to_string();
        assert!(e.contains("layer_height_mm"), "{e}");
        assert_eq!(c, before);
        // a resin printer takes no filament
        let resin = cat.find("elegoo-mars-4").unwrap();
        assert!(c.apply_filament(petg, resin, n(0.4)).is_err());
    }

    #[test]
    fn a_file_names_the_line_that_is_wrong() {
        let e = SliceConfig::from_text("perimeters = 3\ninfill_percent = 500\n").unwrap_err();
        assert!(e.to_string().contains("line 2"), "{e}");
        let e = SliceConfig::from_text("colour = red\n").unwrap_err();
        assert!(e.to_string().contains("colour"));
        let ok =
            SliceConfig::from_text("# a comment\nperimeters = 3 # three\nplacement = origin\n")
                .unwrap();
        assert_eq!((ok.perimeters, ok.placement), (3, Placement::Origin));
    }

    #[test]
    fn the_keys_of_adhesion_speed_retraction_and_vase_are_read_checked_and_written_back() {
        let c = SliceConfig::from_text(
            "adhesion = brim
brim_width_mm = 6
skirt_loops = 3
raft_layers = 4
min_layer_time_s = 8
min_speed_mm_s = 15
retract_adaptive = on
retract_min_percent = 40
vase_mode = off
",
        )
        .unwrap();
        assert_eq!(
            (c.adhesion, c.skirt_loops, c.raft_layers),
            (Adhesion::Brim, 3, 4)
        );
        assert!(c.retract_adaptive && !c.vase_mode);
        assert_eq!(SliceConfig::from_text(&c.to_text()).unwrap(), c);
        assert_eq!(KEYS.len(), 74);
        for k in KEYS {
            assert!(c.value_of(k).is_some(), "{k}");
        }
        for (text, want) in [
            (
                "adhesion = glue
",
                "adhesion",
            ),
            (
                "skirt_loops = 0
",
                "skirt_loops",
            ),
            (
                "raft_layers = 11
",
                "raft_layers",
            ),
            (
                "retract_min_percent = 101
",
                "retract_min_percent",
            ),
            (
                "vase_mode = maybe
",
                "vase_mode",
            ),
            (
                "adhesion = brim
brim_width_mm = 0.2
",
                "brim_width_mm",
            ),
            (
                "vase_mode = on
supports = buildplate
",
                "vase",
            ),
            (
                "vase_mode = on
adhesion = raft
",
                "raft",
            ),
            (
                "min_layer_time_s = 5
min_speed_mm_s = 300
max_speed_mm_s = 200
",
                "min_speed_mm_s",
            ),
        ] {
            let e = SliceConfig::from_text(text).unwrap_err().to_string();
            assert!(e.contains(want), "{text:?} -> {e}");
        }
    }

    #[test]
    fn the_area_of_a_line_and_its_spacing_follow_the_shape_of_the_plastic() {
        let c = SliceConfig::default();
        let a = c.line_area_mm2(0.2);
        assert!((a - (0.2 * 0.25 + std::f64::consts::PI * 0.01)).abs() < 1e-12);
        let s = c.spacing_mm(0.2);
        assert!(s < c.line_width_mm && s > c.line_width_mm - 0.2);
        assert!((c.filament_area_mm2() - 2.405_28).abs() < 1e-4);
    }
}
