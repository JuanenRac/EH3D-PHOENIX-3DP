//! Whether a job fits the machine and keeps to what its maker states: the size of the part with what is laid round it, the speeds, the acceleration, the flow, the nozzle and
//! the filament. A speed over the limit refuses the slice; it is not cut down. The acceleration and the flow are held to the figure of the maker, and that is told: the first
//! only changes the estimate of the time, and the second slows a line down without thinning it.

use crate::config::{Adhesion, SliceConfig};
use crate::error::SliceError;
use p3dp_profiles::MotionLimits;

/// One speed of the settings that is over the highest speed of the machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TooFast {
    pub key: &'static str,
    pub set_mm_s: f64,
    pub max_mm_s: f64,
}

/// A figure of the settings that was held to the one of the machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Held {
    /// The acceleration the time is estimated with.
    Acceleration(f64),
    /// The most plastic a line asks for in a second.
    Flow(f64),
}

/// How the part and what is laid round it sit in the build volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// What the job takes along x, y and z, in mm: the part, the clearance of the bed and what is laid round and under it.
    pub needs_mm: [f64; 3],
    /// How much too large it is along x, y and z, in mm (0 where it fits).
    pub over_mm: [f64; 3],
    /// It does not fit as it is, and would if it were turned by a right angle about the vertical.
    pub fits_turned: bool,
    /// The height used by default by the maker, when the job is higher than it and still inside the volume.
    pub above_default_height_mm: Option<f64>,
}

impl Fit {
    /// Whether it fits as it is.
    pub fn fits(&self) -> bool {
        self.over_mm.iter().all(|o| *o <= 0.0)
    }
}

/// The loops a skirt round a box of this size has: `skirt_loops`, or more until it is `skirt_min_length_mm` long.
fn skirt_loops_of(c: &SliceConfig, size_mm: [f64; 3]) -> f64 {
    let round = 2.0 * (size_mm[0] + size_mm[1] + 4.0 * c.skirt_distance_mm);
    let wanted = if c.skirt_min_length_mm > 0.0 && round > 0.0 {
        (c.skirt_min_length_mm / round)
            .ceil()
            .min(f64::from(crate::adhesion::MAX_SKIRT_LOOPS))
    } else {
        0.0
    };
    wanted.max(f64::from(c.skirt_loops))
}

/// How far what is laid round the part reaches beyond it, on each side.
fn reach_mm(c: &SliceConfig, size_mm: [f64; 3]) -> f64 {
    match c.adhesion {
        Adhesion::None => 0.0,
        Adhesion::Skirt => c.skirt_distance_mm + skirt_loops_of(c, size_mm) * c.line_width_mm,
        Adhesion::Brim => c.brim_width_mm,
        Adhesion::Raft => c.raft_margin_mm,
    }
}

/// How much a raft raises the part.
fn raft_height_mm(c: &SliceConfig) -> f64 {
    if c.adhesion != Adhesion::Raft {
        return 0.0;
    }
    c.first_layer_height_mm
        + f64::from(c.raft_layers.saturating_sub(1)) * c.layer_height_mm
        + c.raft_air_gap_mm
}

/// How a part of this size (x, y, z in mm) sits in the volume with these settings.
pub fn fit(
    size_mm: [f64; 3],
    volume_mm: [f64; 3],
    config: &SliceConfig,
    default_height_mm: Option<f64>,
) -> Fit {
    let round = 2.0 * (config.bed_margin_mm + reach_mm(config, size_mm));
    let needs_mm = [
        size_mm[0] + round,
        size_mm[1] + round,
        size_mm[2] + raft_height_mm(config),
    ];
    let over = |need: f64, have: f64| {
        let o = need - have;
        if o > 1e-9 {
            o
        } else {
            0.0
        }
    };
    let over_mm = [
        over(needs_mm[0], volume_mm[0]),
        over(needs_mm[1], volume_mm[1]),
        over(needs_mm[2], volume_mm[2]),
    ];
    let flat = over_mm[0] > 0.0 || over_mm[1] > 0.0;
    let fits_turned = flat
        && over_mm[2] <= 0.0
        && over(needs_mm[1], volume_mm[0]) <= 0.0
        && over(needs_mm[0], volume_mm[1]) <= 0.0;
    let above_default_height_mm =
        default_height_mm.filter(|h| needs_mm[2] > h + 1e-9 && over_mm[2] <= 0.0);
    Fit {
        needs_mm,
        over_mm,
        fits_turned,
        above_default_height_mm,
    }
}

/// The speeds of the settings that are over the highest speed of the machine; none when the machine states none.
pub fn too_fast(config: &SliceConfig, limits: &MotionLimits) -> Vec<TooFast> {
    let Some(max_mm_s) = limits.max_speed_mm_s else {
        return Vec::new();
    };
    let mut speeds = vec![
        ("first_layer_speed_mm_s", config.first_layer_speed_mm_s),
        ("perimeter_speed_mm_s", config.perimeter_speed_mm_s),
        (
            "external_perimeter_speed_mm_s",
            config.external_perimeter_speed_mm_s,
        ),
        ("infill_speed_mm_s", config.infill_speed_mm_s),
        ("solid_infill_speed_mm_s", config.solid_infill_speed_mm_s),
        ("bridge_speed_mm_s", config.bridge_speed_mm_s),
        ("travel_speed_mm_s", config.travel_speed_mm_s),
    ];
    if config.supports != crate::config::Supports::Off {
        speeds.push(("support_speed_mm_s", config.support_speed_mm_s));
    }
    if config.ironing {
        speeds.push(("ironing_speed_mm_s", config.ironing_speed_mm_s));
    }
    if config.overhang_speed_mm_s > 0.0 {
        speeds.push(("overhang_speed_mm_s", config.overhang_speed_mm_s));
    }
    speeds
        .into_iter()
        .filter(|(_, v)| *v > max_mm_s + 1e-9)
        .map(|(key, set_mm_s)| TooFast {
            key,
            set_mm_s,
            max_mm_s,
        })
        .collect()
}

/// Holds the acceleration of the estimate and the flow to the figures of the machine, and returns what was changed.
pub fn hold(config: &mut SliceConfig, limits: &MotionLimits) -> Vec<Held> {
    let mut out = Vec::new();
    if let Some(a) = limits.max_acceleration_mm_s2 {
        if config.accel_mm_s2 > a {
            config.accel_mm_s2 = a;
            out.push(Held::Acceleration(a));
        }
    }
    if let Some(q) = limits.max_flow_mm3_s {
        let q = q.min(100.0);
        if config.max_volumetric_flow_mm3_s <= 0.0 || config.max_volumetric_flow_mm3_s > q {
            config.max_volumetric_flow_mm3_s = q;
            out.push(Held::Flow(q));
        }
    }
    out
}

/// The limits of the machine on the settings, before a slice: refuses when a speed is over, naming every one, and holds the rest.
pub fn apply(config: &mut SliceConfig, limits: &MotionLimits) -> Result<Vec<Held>, SliceError> {
    let over = too_fast(config, limits);
    if !over.is_empty() {
        let names: Vec<String> = over
            .iter()
            .map(|t| format!("{} = {} (at most {})", t.key, t.set_mm_s, t.max_mm_s))
            .collect();
        return Err(SliceError::Config(format!(
            "faster than the maker states for this printer: {}",
            names.join(", ")
        )));
    }
    Ok(hold(config, limits))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: [f64; 3] = [256.0, 256.0, 256.0];

    fn limits() -> MotionLimits {
        MotionLimits {
            max_speed_mm_s: Some(150.0),
            max_acceleration_mm_s2: Some(2500.0),
            max_flow_mm3_s: Some(28.0),
            default_height_mm: Some(250.0),
        }
    }

    #[test]
    fn a_part_is_measured_with_the_clearance_and_what_is_laid_round_it() {
        let mut c = SliceConfig::default();
        let f = fit([250.0, 100.0, 10.0], V, &c, None);
        assert!(f.fits());
        c.adhesion = Adhesion::Brim;
        c.brim_width_mm = 5.0;
        let f = fit([250.0, 100.0, 10.0], V, &c, None);
        assert!(!f.fits());
        assert_eq!(f.over_mm, [4.0, 0.0, 0.0]);
        assert!(!f.fits_turned, "turned it is still too long");
        c.adhesion = Adhesion::None;
        c.bed_margin_mm = 4.0;
        assert_eq!(fit([250.0, 100.0, 10.0], V, &c, None).over_mm[0], 2.0);
    }

    #[test]
    fn a_part_that_fits_turned_and_one_over_the_height_used_by_default_are_told() {
        let c = SliceConfig::default();
        let f = fit([240.0, 100.0, 10.0], [200.0, 250.0, 200.0], &c, None);
        assert!(!f.fits() && f.fits_turned);
        let tall = fit([10.0, 10.0, 253.0], V, &c, Some(250.0));
        assert!(tall.fits());
        assert_eq!(tall.above_default_height_mm, Some(250.0));
        assert_eq!(
            fit([10.0, 10.0, 250.0], V, &c, Some(250.0)).above_default_height_mm,
            None
        );
        let too_tall = fit([10.0, 10.0, 300.0], V, &c, Some(250.0));
        assert!(!too_tall.fits() && !too_tall.fits_turned);
        assert_eq!(too_tall.above_default_height_mm, None);
        let raft = SliceConfig {
            adhesion: Adhesion::Raft,
            ..SliceConfig::default()
        };
        assert!(fit([10.0, 10.0, 256.0], V, &raft, None).over_mm[2] > 0.0);
    }

    #[test]
    fn a_speed_over_the_machine_refuses_and_names_every_one() {
        let mut c = SliceConfig {
            travel_speed_mm_s: 200.0,
            infill_speed_mm_s: 160.0,
            ironing_speed_mm_s: 500.0,
            ..SliceConfig::default()
        };
        let over = too_fast(&c, &limits());
        let keys: Vec<&str> = over.iter().map(|t| t.key).collect();
        assert_eq!(keys, ["infill_speed_mm_s", "travel_speed_mm_s"]);
        let e = apply(&mut c, &limits()).unwrap_err().to_string();
        assert!(e.contains("travel_speed_mm_s = 200") && e.contains("at most 150"));
        assert_eq!(c.travel_speed_mm_s, 200.0, "nothing is cut down");
        c.ironing = true;
        assert_eq!(too_fast(&c, &limits()).len(), 3);
        assert!(too_fast(&c, &MotionLimits::default()).is_empty());
    }

    #[test]
    fn the_acceleration_and_the_flow_are_held_and_told_once() {
        let mut c = SliceConfig::default();
        assert_eq!(
            apply(&mut c, &limits()).unwrap(),
            [Held::Acceleration(2500.0), Held::Flow(28.0)]
        );
        assert_eq!(c.accel_mm_s2, 2500.0);
        assert_eq!(c.max_volumetric_flow_mm3_s, 28.0);
        assert!(apply(&mut c, &limits()).unwrap().is_empty());
        c.max_volumetric_flow_mm3_s = 12.0;
        assert!(hold(&mut c, &limits()).is_empty(), "a lower limit is kept");
        let mut free = SliceConfig::default();
        assert!(hold(&mut free, &MotionLimits::default()).is_empty());
        assert_eq!(free, SliceConfig::default());
    }
}
