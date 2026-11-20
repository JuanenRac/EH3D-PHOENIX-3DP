//! The control of a resin (mSLA) print layer by layer, with limits that refuse. Three things are worked out here and none of them touches a file:
//!
//! * the exposure a resin needs, from the working curve of a photopolymer (the depth it cures to grows with the logarithm of the dose: `depth = penetration * ln(dose / critical)`),
//!   so a layer height gives a dose, and a dose and the power of the light give seconds;
//! * the moves of every layer: the exposure (the bottom layers, the fade, the normal layers), and a lift that goes slower and waits longer under a large section, which pulls on
//!   the film more;
//! * the limits of the printer and of the resin: an exposure, a lift, a speed or a dose outside them is an error that names the layer and the setting. Nothing is cut down to
//!   make a layer pass.
//!
//! The figures of a resin are the person's to measure with a test print; the defaults are round figures of the usual order of size, not the figures of any one resin.

use crate::error::FormatError;
use crate::resin::{layer_exposure, ResinJob, ResinSettings};

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("resin control", reason)
}

/// What the working curve of a resin needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinMaterial {
    pub name: String,
    /// How far the light goes into the resin before it is a tenth as strong, in the form of the working curve (the depth grows by this per natural logarithm of the dose), in mm.
    pub penetration_mm: f64,
    /// The dose under which the resin does not set at all, in mJ/cm2.
    pub critical_dose_mj_cm2: f64,
}

impl ResinMaterial {
    /// A resin of the usual order of size. It is a place to start from, to be replaced by the figures of a test print.
    pub fn generic() -> ResinMaterial {
        ResinMaterial {
            name: "generic".to_string(),
            penetration_mm: 0.12,
            critical_dose_mj_cm2: 6.0,
        }
    }
}

/// The limits of the printer and the resin. They are the person's to set for the machine; the defaults are conservative.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinLimits {
    pub min_exposure_s: f64,
    pub max_exposure_s: f64,
    pub max_bottom_exposure_s: f64,
    pub max_lift_mm: f64,
    pub min_lift_speed_mm_s: f64,
    pub max_lift_speed_mm_s: f64,
    pub max_wait_s: f64,
    /// The most light on one layer, in mJ/cm2.
    pub max_layer_dose_mj_cm2: f64,
}

impl Default for ResinLimits {
    fn default() -> ResinLimits {
        ResinLimits {
            min_exposure_s: 0.5,
            max_exposure_s: 60.0,
            max_bottom_exposure_s: 180.0,
            max_lift_mm: 20.0,
            min_lift_speed_mm_s: 0.2,
            max_lift_speed_mm_s: 10.0,
            max_wait_s: 30.0,
            max_layer_dose_mj_cm2: 1500.0,
        }
    }
}

/// How the lift reacts to the size of the section being pulled off the film.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionRule {
    /// The area (mm2) up to which the lift is the plain one.
    pub reference_mm2: f64,
    /// The slowest the lift gets as a part of the plain speed, 0.05 to 1.
    pub floor_ratio: f64,
    /// The seconds of waiting added for every time the area is the reference.
    pub extra_wait_per_reference_s: f64,
}

impl Default for SectionRule {
    fn default() -> SectionRule {
        SectionRule {
            reference_mm2: 1000.0,
            floor_ratio: 0.4,
            extra_wait_per_reference_s: 0.5,
        }
    }
}

/// The exposure for a layer height, and for the bottom layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    pub normal_s: f64,
    pub bottom_s: f64,
}

/// The exposure a resin needs for a layer height. The depth to cure is `cure_factor` layers (a layer has to set into the one below it, so more than 1), the dose comes from the
/// working curve, and the seconds from the power of the light. The bottom layers get `bottom_factor` times as much. A result outside the limits is an error.
pub fn suggest_exposure(
    material: &ResinMaterial,
    layer_height_mm: f64,
    power_mw_cm2: f64,
    cure_factor: f64,
    bottom_factor: f64,
    limits: &ResinLimits,
) -> Result<Exposure, FormatError> {
    let ok = |name: &str, v: f64, lo: f64, hi: f64| {
        if v.is_finite() && (lo..=hi).contains(&v) {
            Ok(())
        } else {
            Err(bad(format!("{name} {v} is outside {lo} to {hi}")))
        }
    };
    ok("layer_height_mm", layer_height_mm, 0.005, 1.0)?;
    ok("power_mw_cm2", power_mw_cm2, 0.1, 1000.0)?;
    ok("cure_factor", cure_factor, 1.0, 5.0)?;
    ok("bottom_factor", bottom_factor, 1.0, 30.0)?;
    ok("penetration_mm", material.penetration_mm, 0.005, 2.0)?;
    ok(
        "critical_dose_mj_cm2",
        material.critical_dose_mj_cm2,
        0.01,
        1000.0,
    )?;
    let depth = layer_height_mm * cure_factor;
    let dose = material.critical_dose_mj_cm2 * (depth / material.penetration_mm).exp();
    let normal = dose / power_mw_cm2;
    let bottom = normal * bottom_factor;
    ok(
        "exposure_s",
        normal,
        limits.min_exposure_s,
        limits.max_exposure_s,
    )?;
    ok(
        "bottom_exposure_s",
        bottom,
        limits.min_exposure_s,
        limits.max_bottom_exposure_s,
    )?;
    Ok(Exposure {
        normal_s: normal,
        bottom_s: bottom,
    })
}

/// The depth a dose cures to, from the working curve (0 when the dose is under the critical one).
pub fn cure_depth_mm(material: &ResinMaterial, dose_mj_cm2: f64) -> f64 {
    if dose_mj_cm2 <= material.critical_dose_mj_cm2 || !dose_mj_cm2.is_finite() {
        0.0
    } else {
        material.penetration_mm * (dose_mj_cm2 / material.critical_dose_mj_cm2).ln()
    }
}

/// What the printer does for one layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerControl {
    pub index: usize,
    pub exposure_s: f64,
    pub dose_mj_cm2: f64,
    pub lift_mm: f64,
    pub lift_speed_mm_s: f64,
    pub retract_speed_mm_s: f64,
    pub wait_before_s: f64,
    pub wait_after_s: f64,
}

impl LayerControl {
    /// The seconds the layer takes: waiting, light, lift and return.
    pub fn seconds(&self) -> f64 {
        self.wait_before_s
            + self.wait_after_s
            + self.exposure_s
            + self.lift_mm / self.lift_speed_mm_s
            + self.lift_mm / self.retract_speed_mm_s
    }
}

/// What the control comes to.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlReport {
    pub layers: Vec<LayerControl>,
    pub light_on_s: f64,
    pub movement_s: f64,
    pub total_s: f64,
    pub total_dose_mj_cm2: f64,
    /// The layers whose lift was slowed down by the size of their section.
    pub slowed_layers: usize,
}

/// The moves of every layer of a job. `areas_mm2` are the sections of the layers, in order, when they are known (reading the images of the layers is not done here); without them
/// every layer gets the plain lift. Every value is checked against `limits`; the first layer that does not pass is the error.
pub fn control_layers(
    job: &ResinJob,
    moves: &ResinSettings,
    power_mw_cm2: f64,
    rule: &SectionRule,
    limits: &ResinLimits,
    areas_mm2: Option<&[f64]>,
) -> Result<ControlReport, FormatError> {
    if !(power_mw_cm2.is_finite() && (0.1..=1000.0).contains(&power_mw_cm2)) {
        return Err(bad(format!(
            "power_mw_cm2 {power_mw_cm2} is outside 0.1 to 1000"
        )));
    }
    if !(rule.reference_mm2.is_finite()
        && rule.reference_mm2 > 0.0
        && (0.05..=1.0).contains(&rule.floor_ratio)
        && rule.extra_wait_per_reference_s.is_finite()
        && rule.extra_wait_per_reference_s >= 0.0)
    {
        return Err(bad("the rule of the section is out of its range"));
    }
    if job.layers == 0 || job.layers > crate::resin::MAX_LAYERS {
        return Err(bad(format!(
            "{} layers is outside 1 to {}",
            job.layers,
            crate::resin::MAX_LAYERS
        )));
    }
    if let Some(a) = areas_mm2 {
        if a.len() != job.layers {
            return Err(bad(format!("{} areas for {} layers", a.len(), job.layers)));
        }
    }
    let mut layers = Vec::with_capacity(job.layers);
    let (mut light, mut movement, mut dose_sum, mut slowed) = (0.0, 0.0, 0.0, 0usize);
    for i in 0..job.layers {
        let exposure = layer_exposure(job, moves.bottom_layers, i);
        let bottom = i < moves.bottom_layers as usize;
        let area = areas_mm2.map_or(0.0, |a| a[i]);
        if !(area.is_finite() && area >= 0.0) {
            return Err(bad(format!("layer {i}: the area {area} is not a size")));
        }
        let ratio = (rule.reference_mm2 / area.max(1e-9)).clamp(rule.floor_ratio, 1.0);
        let big = (area / rule.reference_mm2 - 1.0).max(0.0);
        let c = LayerControl {
            index: i,
            exposure_s: exposure,
            dose_mj_cm2: exposure * power_mw_cm2,
            lift_mm: moves.lift_mm,
            lift_speed_mm_s: moves.lift_speed_mm_s * ratio,
            retract_speed_mm_s: moves.retract_speed_mm_s,
            wait_before_s: moves.wait_before_s + big * rule.extra_wait_per_reference_s,
            wait_after_s: moves.wait_after_s,
        };
        let top = if bottom {
            limits.max_bottom_exposure_s
        } else {
            limits.max_exposure_s
        };
        let fail = |name: &str, v: f64, lo: f64, hi: f64| {
            Err(bad(format!(
                "layer {i}: {name} {v:.3} is outside {lo} to {hi}"
            )))
        };
        if !(limits.min_exposure_s..=top).contains(&c.exposure_s) {
            return fail("exposure_s", c.exposure_s, limits.min_exposure_s, top);
        }
        if c.lift_mm > limits.max_lift_mm {
            return fail("lift_mm", c.lift_mm, 0.0, limits.max_lift_mm);
        }
        for (name, v) in [
            ("lift_speed_mm_s", c.lift_speed_mm_s),
            ("retract_speed_mm_s", c.retract_speed_mm_s),
        ] {
            if !(limits.min_lift_speed_mm_s..=limits.max_lift_speed_mm_s).contains(&v) {
                return fail(
                    name,
                    v,
                    limits.min_lift_speed_mm_s,
                    limits.max_lift_speed_mm_s,
                );
            }
        }
        for (name, v) in [
            ("wait_before_s", c.wait_before_s),
            ("wait_after_s", c.wait_after_s),
        ] {
            if v > limits.max_wait_s {
                return fail(name, v, 0.0, limits.max_wait_s);
            }
        }
        if c.dose_mj_cm2 > limits.max_layer_dose_mj_cm2 {
            return fail(
                "dose_mj_cm2",
                c.dose_mj_cm2,
                0.0,
                limits.max_layer_dose_mj_cm2,
            );
        }
        if ratio < 1.0 {
            slowed += 1;
        }
        light += c.exposure_s;
        movement += c.seconds() - c.exposure_s;
        dose_sum += c.dose_mj_cm2;
        layers.push(c);
    }
    Ok(ControlReport {
        layers,
        light_on_s: light,
        movement_s: movement,
        total_s: light + movement,
        total_dose_mj_cm2: dose_sum,
        slowed_layers: slowed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::rng::Rng;

    fn job(layers: usize, height: f64) -> ResinJob {
        ResinJob {
            layers,
            layer_height_mm: height,
            exposure_s: 2.5,
            bottom_exposure_s: 25.0,
            transition_layers: 3,
            material: String::new(),
            stated_seconds: None,
            used_ml: None,
            config: Vec::new(),
        }
    }

    #[test]
    fn the_working_curve_gives_back_the_depth_it_was_asked_for() {
        let m = ResinMaterial::generic();
        let limits = ResinLimits::default();
        for h in [0.025, 0.05, 0.1] {
            let e = suggest_exposure(&m, h, 3.0, 1.5, 8.0, &limits).unwrap();
            let dose = e.normal_s * 3.0;
            assert!((cure_depth_mm(&m, dose) - h * 1.5).abs() < 1e-9, "{h}");
            assert!((e.bottom_s - e.normal_s * 8.0).abs() < 1e-9);
        }
        // a thicker layer needs more light, a stronger lamp less
        let at = |h: f64, p: f64| {
            suggest_exposure(&m, h, p, 1.5, 8.0, &limits)
                .unwrap()
                .normal_s
        };
        assert!(at(0.1, 3.0) > at(0.05, 3.0) && at(0.05, 6.0) < at(0.05, 3.0));
        // under the critical dose nothing sets
        assert_eq!(cure_depth_mm(&m, 5.0), 0.0);
        assert_eq!(cure_depth_mm(&m, f64::NAN), 0.0);
    }

    #[test]
    fn an_exposure_outside_the_limits_or_a_setting_out_of_range_is_an_error_that_names_it() {
        let m = ResinMaterial::generic();
        let limits = ResinLimits::default();
        // 0.5 mm layers on a weak lamp need minutes
        let e = suggest_exposure(&m, 0.5, 0.2, 1.5, 8.0, &limits)
            .unwrap_err()
            .to_string();
        assert!(e.contains("exposure_s"), "{e}");
        let strict = ResinLimits {
            max_bottom_exposure_s: 20.0,
            ..ResinLimits::default()
        };
        let e = suggest_exposure(&m, 0.05, 3.0, 1.5, 12.0, &strict)
            .unwrap_err()
            .to_string();
        assert!(e.contains("bottom_exposure_s"), "{e}");
        for (h, p, c, b, name) in [
            (0.0, 3.0, 1.5, 8.0, "layer_height_mm"),
            (0.05, 0.0, 1.5, 8.0, "power_mw_cm2"),
            (0.05, 3.0, 0.5, 8.0, "cure_factor"),
            (0.05, 3.0, 1.5, 0.5, "bottom_factor"),
            (f64::NAN, 3.0, 1.5, 8.0, "layer_height_mm"),
        ] {
            let e = suggest_exposure(&m, h, p, c, b, &limits)
                .unwrap_err()
                .to_string();
            assert!(e.contains(name), "{e}");
        }
        let weird = ResinMaterial {
            penetration_mm: 0.0,
            ..m
        };
        assert!(suggest_exposure(&weird, 0.05, 3.0, 1.5, 8.0, &limits).is_err());
    }

    #[test]
    fn every_layer_has_its_exposure_and_the_plain_lift_without_sections() {
        let j = job(40, 0.05);
        let s = ResinSettings::default();
        let r = control_layers(
            &j,
            &s,
            3.0,
            &SectionRule::default(),
            &ResinLimits::default(),
            None,
        )
        .unwrap();
        assert_eq!(r.layers.len(), 40);
        assert_eq!(r.slowed_layers, 0);
        // the bottom layers, the fade and the normal layers
        assert!((r.layers[0].exposure_s - 25.0).abs() < 1e-9);
        assert!(r.layers[5].exposure_s < 25.0 && r.layers[5].exposure_s > 2.5);
        assert!((r.layers[39].exposure_s - 2.5).abs() < 1e-9);
        assert!(r
            .layers
            .windows(2)
            .all(|w| w[1].exposure_s <= w[0].exposure_s + 1e-12));
        assert!((r.layers[10].dose_mj_cm2 - 7.5).abs() < 1e-9);
        // the total is the estimate of the module that reads the files, when the lift is plain
        let est = crate::resin::estimate(&j, &s).unwrap();
        assert!(
            (r.total_s - est.total_s).abs() < 1e-6,
            "{} {}",
            r.total_s,
            est.total_s
        );
        assert!((r.light_on_s - est.exposure_s).abs() < 1e-6);
        let sum: f64 = r.layers.iter().map(LayerControl::seconds).sum();
        assert!((sum - r.total_s).abs() < 1e-6);
    }

    #[test]
    fn a_large_section_lifts_slower_and_waits_longer_and_never_past_the_floor() {
        let j = job(30, 0.05);
        let s = ResinSettings::default();
        let rule = SectionRule::default();
        let areas: Vec<f64> = (0..30).map(|i| 200.0 + 400.0 * i as f64).collect();
        let r = control_layers(&j, &s, 3.0, &rule, &ResinLimits::default(), Some(&areas)).unwrap();
        assert!(r.slowed_layers > 0);
        for w in r.layers.windows(2) {
            assert!(
                w[1].lift_speed_mm_s <= w[0].lift_speed_mm_s + 1e-12,
                "the lift never speeds up as the section grows"
            );
            assert!(w[1].wait_before_s >= w[0].wait_before_s - 1e-12);
        }
        assert!((r.layers[0].lift_speed_mm_s - s.lift_speed_mm_s).abs() < 1e-12);
        assert!((r.layers[29].lift_speed_mm_s - s.lift_speed_mm_s * rule.floor_ratio).abs() < 1e-9);
        // a section of 3000 mm2 waits 2 references longer than the plain wait
        let at3000 = control_layers(
            &job(1, 0.05),
            &s,
            3.0,
            &rule,
            &ResinLimits::default(),
            Some(&[3000.0]),
        )
        .unwrap();
        assert!(
            (at3000.layers[0].wait_before_s
                - (s.wait_before_s + 2.0 * rule.extra_wait_per_reference_s))
                .abs()
                < 1e-9
        );
        // it takes longer than the plain lift
        let plain = control_layers(&j, &s, 3.0, &rule, &ResinLimits::default(), None).unwrap();
        assert!(r.total_s > plain.total_s);
    }

    #[test]
    fn what_goes_past_a_limit_is_refused_with_the_layer_and_the_setting() {
        let j = job(10, 0.05);
        let s = ResinSettings::default();
        let rule = SectionRule::default();
        let l = ResinLimits::default();
        let e = |j: &ResinJob, s: &ResinSettings, p: f64, l: &ResinLimits, a: Option<&[f64]>| {
            control_layers(j, s, p, &rule, l, a)
                .unwrap_err()
                .to_string()
        };
        // the dose of the bottom layers is the first to go past
        let msg = e(
            &j,
            &s,
            3.0,
            &ResinLimits {
                max_layer_dose_mj_cm2: 50.0,
                ..l.clone()
            },
            None,
        );
        assert!(
            msg.contains("layer 0") && msg.contains("dose_mj_cm2"),
            "{msg}"
        );
        let msg = e(
            &j,
            &ResinSettings {
                lift_mm: 30.0,
                ..s.clone()
            },
            3.0,
            &l,
            None,
        );
        assert!(msg.contains("lift_mm"), "{msg}");
        let msg = e(
            &j,
            &ResinSettings {
                lift_speed_mm_s: 0.3,
                ..s.clone()
            },
            3.0,
            &l,
            Some(&[5000.0; 10]),
        );
        assert!(msg.contains("lift_speed_mm_s"), "{msg}");
        let msg = e(
            &j,
            &ResinSettings {
                wait_before_s: 29.9,
                ..s.clone()
            },
            3.0,
            &l,
            Some(&[5000.0; 10]),
        );
        assert!(msg.contains("wait_before_s"), "{msg}");
        let msg = e(
            &ResinJob {
                exposure_s: 0.2,
                ..j.clone()
            },
            &s,
            3.0,
            &l,
            None,
        );
        assert!(msg.contains("exposure_s"), "{msg}");
        // the wrong number of areas, a size that is not one, a power out of range
        assert!(e(&j, &s, 3.0, &l, Some(&[1.0; 3])).contains("3 areas for 10 layers"));
        assert!(e(&j, &s, 3.0, &l, Some(&[f64::NAN; 10])).contains("not a size"));
        assert!(e(&j, &s, 0.0, &l, None).contains("power_mw_cm2"));
        assert!(control_layers(&job(0, 0.05), &s, 3.0, &rule, &l, None).is_err());
        let rule_bad = SectionRule {
            floor_ratio: 0.0,
            ..SectionRule::default()
        };
        assert!(control_layers(&j, &s, 3.0, &rule_bad, &l, None).is_err());
    }

    #[test]
    fn jobs_from_a_fixed_seed_keep_to_the_limits_or_are_refused_and_never_cut() {
        let mut rng = Rng::new(0x5124_5EED);
        let limits = ResinLimits::default();
        let rule = SectionRule::default();
        for case in 0..200 {
            let layers = 1 + rng.below(300) as usize;
            let j = ResinJob {
                exposure_s: rng.range(0.4, 70.0),
                bottom_exposure_s: rng.range(0.4, 200.0),
                transition_layers: rng.below(10) as u32,
                ..job(layers, 0.05)
            };
            let s = ResinSettings {
                bottom_layers: rng.below(12) as u32,
                lift_mm: rng.range(0.0, 25.0),
                lift_speed_mm_s: rng.range(0.1, 12.0),
                retract_speed_mm_s: rng.range(0.1, 12.0),
                wait_before_s: rng.range(0.0, 20.0),
                wait_after_s: rng.range(0.0, 20.0),
            };
            let areas: Vec<f64> = (0..layers).map(|_| rng.range(0.0, 6000.0)).collect();
            let power = rng.range(0.5, 20.0);
            match control_layers(&j, &s, power, &rule, &limits, Some(&areas)) {
                Ok(r) => {
                    for c in &r.layers {
                        assert!(
                            (limits.min_exposure_s..=limits.max_bottom_exposure_s)
                                .contains(&c.exposure_s),
                            "case {case}"
                        );
                        assert!(
                            c.lift_mm <= limits.max_lift_mm
                                && c.dose_mj_cm2 <= limits.max_layer_dose_mj_cm2,
                            "case {case}"
                        );
                        assert!(
                            (limits.min_lift_speed_mm_s..=limits.max_lift_speed_mm_s)
                                .contains(&c.lift_speed_mm_s),
                            "case {case}"
                        );
                        assert!(
                            c.wait_before_s <= limits.max_wait_s
                                && c.wait_after_s <= limits.max_wait_s
                        );
                        // no value was lowered to fit: they are the settings, or the section's slowing of them
                        assert!(
                            c.lift_mm == s.lift_mm && c.retract_speed_mm_s == s.retract_speed_mm_s
                        );
                        assert!(c.lift_speed_mm_s <= s.lift_speed_mm_s + 1e-12);
                    }
                    assert!(r.total_s.is_finite() && r.total_s >= r.light_on_s);
                }
                Err(e) => assert!(
                    e.to_string().contains("outside") || e.to_string().contains("layer"),
                    "{e}"
                ),
            }
        }
    }
}
