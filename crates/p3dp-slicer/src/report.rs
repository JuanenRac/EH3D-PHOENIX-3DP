//! What a plan comes to, in the terms a person plans a print by: the material it takes (length, volume, weight and cost, and how much of that each kind of line takes), the layers that go
//! by so fast the plastic has no time to cool and the slowest layers, and where the print stands after each layer (time gone, time left, share done, filament used). All of it is
//! computed from the plan and from the figures it is given; nothing is looked up, and a figure that makes no sense (a density of zero, a negative price) is refused, never assumed.
//!
//! The time is the estimate of the plan, with the acceleration of the settings and not the planner of the printer; the weight is the volume the plan lays down at the density it is given.

use crate::emit::LayerSummary;
use crate::error::SliceError;
use crate::plan::{Kind, Plan};

/// The kinds of line, in the order they are reported.
pub const KINDS: [Kind; 11] = [
    Kind::ExternalPerimeter,
    Kind::Perimeter,
    Kind::Infill,
    Kind::SolidInfill,
    Kind::Bridge,
    Kind::Support,
    Kind::Adhesion,
    Kind::Raft,
    Kind::Tower,
    Kind::Ironing,
    Kind::OverhangPerimeter,
];

/// What one kind of line takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindUse {
    pub kind: Kind,
    pub volume_mm3: f64,
    pub mass_g: f64,
}

/// The material a plan takes.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialEstimate {
    /// The length of filament through the extruder, in millimetres.
    pub length_mm: f64,
    pub volume_mm3: f64,
    pub mass_g: f64,
    /// The cost of `mass_g` at the price per kilogram given (0 when the price was 0).
    pub cost: f64,
    /// What each kind of line takes, in the order of [`KINDS`]; a kind the plan does not use is not listed.
    pub by_kind: Vec<KindUse>,
}

fn config(reason: impl Into<String>) -> SliceError {
    SliceError::Config(reason.into())
}

/// The material a plan takes at a density (g/cm3) and a price per kilogram.
pub fn material(
    plan: &Plan,
    density_g_cm3: f64,
    price_per_kg: f64,
) -> Result<MaterialEstimate, SliceError> {
    if !(density_g_cm3.is_finite() && density_g_cm3 > 0.0 && density_g_cm3 <= 20.0) {
        return Err(config(format!(
            "the density {density_g_cm3} g/cm3 is not above 0 and up to 20"
        )));
    }
    if !(price_per_kg.is_finite() && price_per_kg >= 0.0) {
        return Err(config(format!(
            "the price {price_per_kg} is not a number of zero or more"
        )));
    }
    let mut volumes = [0.0f64; KINDS.len()];
    for layer in &plan.layers {
        for path in &layer.paths {
            let i = KINDS.iter().position(|k| *k == path.kind).unwrap_or(0);
            volumes[i] +=
                path.length_mm() * plan.line_area_mm2(layer.height_mm, path.kind, path.width_mm);
        }
    }
    let per_mm3 = density_g_cm3 / 1000.0;
    let by_kind: Vec<KindUse> = KINDS
        .iter()
        .zip(volumes)
        .filter(|(_, v)| *v > 0.0)
        .map(|(k, v)| KindUse {
            kind: *k,
            volume_mm3: v,
            mass_g: v * per_mm3,
        })
        .collect();
    let volume_mm3: f64 = volumes.iter().sum();
    let mass_g = volume_mm3 * per_mm3;
    Ok(MaterialEstimate {
        length_mm: volume_mm3 / plan.config.filament_area_mm2(),
        volume_mm3,
        mass_g,
        cost: mass_g / 1000.0 * price_per_kg,
        by_kind,
    })
}

/// The time of the layers.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeReport {
    pub total_s: f64,
    /// The layers that take less than the time asked for (their numbers), in the order of the print.
    pub short_layers: Vec<usize>,
    /// The slowest layers as `(layer, seconds)`, slowest first.
    pub slowest: Vec<(usize, f64)>,
    /// The length the head travels without extruding, in millimetres.
    pub travel_mm: f64,
}

/// The time report of the summaries of a plan: the layers under `short_under_s` seconds (0 lists none) and the `slowest_n` slowest.
pub fn time_report(summaries: &[LayerSummary], short_under_s: f64, slowest_n: usize) -> TimeReport {
    let mut slowest: Vec<(usize, f64)> = summaries.iter().map(|s| (s.index, s.time_s)).collect();
    // slowest first; two equal layers keep the order of the print
    slowest.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    slowest.truncate(slowest_n);
    TimeReport {
        total_s: summaries.iter().map(|s| s.time_s).sum(),
        short_layers: summaries
            .iter()
            .filter(|s| short_under_s > 0.0 && s.time_s < short_under_s)
            .map(|s| s.index)
            .collect(),
        slowest,
        travel_mm: summaries.iter().map(|s| s.travel_mm).sum(),
    }
}

/// Where the print stands after a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub layer: usize,
    pub z_mm: f64,
    /// The time from the start to the end of this layer.
    pub elapsed_s: f64,
    /// The time from the end of this layer to the end of the print.
    pub remaining_s: f64,
    /// The share of the time that is gone, 0 to 100.
    pub percent: f64,
    /// The filament through the extruder up to the end of this layer, in millimetres.
    pub filament_mm: f64,
}

/// The groups of lines that the time is told by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Perimeters,
    Infill,
    Solid,
    Bridges,
    Supports,
    /// The skirt, the brim, the raft and the purge tower.
    Adhesion,
    /// What is not a line of plastic: the travel, the retractions, the changes of height and of tool, and the time the head loses speeding up and slowing down.
    Other,
}

impl Group {
    fn of(kind: Kind) -> Group {
        match kind {
            Kind::ExternalPerimeter | Kind::Perimeter | Kind::OverhangPerimeter => {
                Group::Perimeters
            }
            Kind::Infill => Group::Infill,
            Kind::SolidInfill | Kind::Ironing => Group::Solid,
            Kind::Bridge => Group::Bridges,
            Kind::Support => Group::Supports,
            Kind::Adhesion | Kind::Raft | Kind::Tower => Group::Adhesion,
        }
    }
}

/// How much of the time of a print goes to one group of lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeShare {
    pub group: Group,
    pub seconds: f64,
}

/// Where the time of the print goes. The lines of plastic of each group are counted at their own speed (the one that is written, with the limit of the flow and the slower short
/// layers), and what is left of the estimate of the whole print (the travel, the retractions, the changes and the acceleration) is `Group::Other`, so the shares add up to the estimate
/// of the plan. A group the plan has no line for is not listed; `Other` comes last.
pub fn time_by_group(plan: &Plan, summaries: &[LayerSummary]) -> Vec<TimeShare> {
    let total: f64 = summaries.iter().map(|s| s.time_s).sum();
    let order = [
        Group::Perimeters,
        Group::Infill,
        Group::Solid,
        Group::Bridges,
        Group::Supports,
        Group::Adhesion,
    ];
    let mut seconds = [0.0f64; 6];
    for l in &plan.layers {
        for p in &l.paths {
            let g = Group::of(p.kind);
            let i = order.iter().position(|x| *x == g).unwrap_or(0);
            let v = plan.line_speed_mm_s(l.index, p.kind, p.width_mm, l.height_mm);
            if v > 0.0 {
                seconds[i] += p.length_mm() / v;
            }
        }
    }
    let lines: f64 = seconds.iter().sum();
    // the lines at their own speed cannot take longer than the print: the estimate has the acceleration in it
    let scale = if lines > total && lines > 0.0 {
        total / lines
    } else {
        1.0
    };
    let mut out: Vec<TimeShare> = order
        .iter()
        .zip(seconds)
        .filter(|(_, s)| *s > 0.0)
        .map(|(g, s)| TimeShare {
            group: *g,
            seconds: s * scale,
        })
        .collect();
    let rest = total - out.iter().map(|t| t.seconds).sum::<f64>();
    if rest > 1e-9 {
        out.push(TimeShare {
            group: Group::Other,
            seconds: rest,
        });
    }
    out
}

/// The progress after each layer, from the summaries of a plan and the section of its filament (mm2).
pub fn progress(summaries: &[LayerSummary], filament_area_mm2: f64) -> Vec<Progress> {
    let total: f64 = summaries.iter().map(|s| s.time_s).sum();
    let mut elapsed = 0.0;
    let mut volume = 0.0;
    summaries
        .iter()
        .map(|s| {
            elapsed += s.time_s;
            volume += s.extrusion_mm3;
            Progress {
                layer: s.index,
                z_mm: s.z_mm,
                elapsed_s: elapsed,
                remaining_s: (total - elapsed).max(0.0),
                percent: if total > 0.0 {
                    elapsed / total * 100.0
                } else {
                    100.0
                },
                filament_mm: if filament_area_mm2 > 0.0 {
                    volume / filament_area_mm2
                } else {
                    0.0
                },
            }
        })
        .collect()
}

/// The layer where a share of the print (0 to 100 percent of the time) has gone by, `None` when there are no layers or the share is out of range.
pub fn layer_at_percent(progress: &[Progress], percent: f64) -> Option<usize> {
    if !(0.0..=100.0).contains(&percent) {
        return None;
    }
    progress
        .iter()
        .find(|p| p.percent >= percent)
        .map(|p| p.layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SliceConfig;
    use crate::plan::{plan, Machine};
    use p3dp_core::mesh::cube;
    use p3dp_core::{Dialect, TempLimits};

    fn machine() -> Machine {
        Machine {
            id: "report-test".to_string(),
            dialect: Dialect::Marlin,
            volume_mm: [220.0, 220.0, 250.0],
            limits: TempLimits {
                nozzle_c: 300.0,
                bed_c: 110.0,
                chamber_c: None,
            },
        }
    }

    fn sliced(infill: f64) -> Plan {
        let c = SliceConfig {
            infill_percent: infill,
            ..SliceConfig::default()
        };
        plan(&cube(20.0), &machine(), &c).unwrap()
    }

    fn summary(index: usize, time_s: f64, travel: f64, mm3: f64) -> LayerSummary {
        LayerSummary {
            index,
            z_mm: 0.2 * (index as f64 + 1.0),
            height_mm: 0.2,
            paths: 1,
            perimeter_mm: 0.0,
            infill_mm: 0.0,
            solid_mm: 0.0,
            bridge_mm: 0.0,
            support_mm: 0.0,
            adhesion_mm: 0.0,
            travel_mm: travel,
            extrusion_mm3: mm3,
            time_s,
            flat_time_s: time_s,
            stop_go_time_s: time_s,
        }
    }

    #[test]
    fn the_material_adds_up_to_the_plan_and_the_weight_follows_the_density() {
        let p = sliced(100.0);
        let m = material(&p, 1.24, 20.0).unwrap();
        assert!((m.volume_mm3 - p.extrusion_mm3()).abs() < 1e-6);
        assert!((m.length_mm - p.filament_mm()).abs() < 1e-6);
        assert!((m.mass_g - m.volume_mm3 * 1.24 / 1000.0).abs() < 1e-9);
        assert!((m.cost - m.mass_g / 1000.0 * 20.0).abs() < 1e-9);
        let kinds: f64 = m.by_kind.iter().map(|k| k.volume_mm3).sum();
        assert!((kinds - m.volume_mm3).abs() < 1e-6);
        let grams: f64 = m.by_kind.iter().map(|k| k.mass_g).sum();
        assert!((grams - m.mass_g).abs() < 1e-9);
        // a solid cube of 20 mm is 8 cm3: the plastic laid down is close to it
        assert!(
            (m.volume_mm3 / 8000.0 - 1.0).abs() < 0.1,
            "{}",
            m.volume_mm3
        );
        // a denser plastic weighs more, a price of zero costs nothing
        let heavy = material(&p, 2.48, 0.0).unwrap();
        assert!((heavy.mass_g / m.mass_g - 2.0).abs() < 1e-9);
        assert_eq!(heavy.cost, 0.0);
        assert!(m.by_kind.iter().any(|k| k.kind == Kind::ExternalPerimeter));
        assert!(m.by_kind.iter().all(|k| k.volume_mm3 > 0.0));
    }

    #[test]
    fn less_infill_takes_less_material_and_a_density_or_price_that_makes_no_sense_is_refused() {
        let full = material(&sliced(100.0), 1.24, 0.0).unwrap();
        let light = material(&sliced(10.0), 1.24, 0.0).unwrap();
        assert!(light.mass_g < full.mass_g);
        let p = sliced(10.0);
        for (d, price) in [
            (0.0, 1.0),
            (-1.0, 1.0),
            (f64::NAN, 1.0),
            (21.0, 1.0),
            (1.2, -0.01),
            (1.2, f64::NAN),
        ] {
            let e = material(&p, d, price).unwrap_err().to_string();
            assert!(
                e.contains("density") || e.contains("price"),
                "{d} {price}: {e}"
            );
        }
    }

    #[test]
    fn the_time_report_names_the_short_layers_and_the_slowest_ones() {
        let s = vec![
            summary(0, 30.0, 10.0, 5.0),
            summary(1, 4.0, 2.0, 1.0),
            summary(2, 30.0, 8.0, 5.0),
            summary(3, 9.0, 1.0, 2.0),
            summary(4, 55.0, 4.0, 9.0),
        ];
        let r = time_report(&s, 10.0, 3);
        assert_eq!(r.total_s, 128.0);
        assert_eq!(r.short_layers, vec![1, 3]);
        assert_eq!(
            r.slowest,
            vec![(4, 55.0), (0, 30.0), (2, 30.0)],
            "equal layers keep the order of the print"
        );
        assert_eq!(r.travel_mm, 25.0);
        // 0 lists no short layers; asking for more than there are gives all of them
        let r = time_report(&s, 0.0, 99);
        assert!(r.short_layers.is_empty());
        assert_eq!(r.slowest.len(), 5);
        assert!(time_report(&[], 10.0, 3).slowest.is_empty());
        assert_eq!(time_report(&[], 10.0, 3).total_s, 0.0);
    }

    #[test]
    fn the_progress_after_each_layer_ends_at_the_whole_print() {
        let s = vec![
            summary(0, 10.0, 0.0, 240.5),
            summary(1, 30.0, 0.0, 240.5),
            summary(2, 60.0, 0.0, 240.5),
        ];
        let area = std::f64::consts::PI * 0.875 * 0.875;
        let p = progress(&s, area);
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].elapsed_s, p[0].remaining_s), (10.0, 90.0));
        assert!((p[0].percent - 10.0).abs() < 1e-9);
        assert_eq!((p[2].elapsed_s, p[2].remaining_s), (100.0, 0.0));
        assert!((p[2].percent - 100.0).abs() < 1e-9);
        assert!((p[2].filament_mm - 721.5 / area).abs() < 1e-9);
        assert!(p
            .windows(2)
            .all(|w| w[0].elapsed_s <= w[1].elapsed_s && w[0].filament_mm <= w[1].filament_mm));
        assert_eq!(layer_at_percent(&p, 0.0), Some(0));
        assert_eq!(layer_at_percent(&p, 35.0), Some(1));
        assert_eq!(layer_at_percent(&p, 40.5), Some(2));
        assert_eq!(layer_at_percent(&p, 100.0), Some(2));
        assert_eq!(layer_at_percent(&p, 101.0), None);
        assert_eq!(layer_at_percent(&[], 10.0), None);
        // a print with no time is all done, and a section of zero does not divide
        let z = progress(&[summary(0, 0.0, 0.0, 1.0)], 0.0);
        assert_eq!((z[0].percent, z[0].filament_mm), (100.0, 0.0));
    }

    #[test]
    fn the_progress_of_a_real_plan_is_monotonic_and_ends_at_its_filament_and_time() {
        let p = sliced(20.0);
        let s = p.summaries();
        let pr = progress(&s, p.config.filament_area_mm2());
        assert_eq!(pr.len(), p.layer_count());
        let last = pr.last().unwrap();
        assert!((last.filament_mm - p.filament_mm()).abs() < 1e-6);
        assert!((last.elapsed_s - p.estimated_seconds()).abs() < 1e-6);
        assert!(pr.windows(2).all(|w| w[0].z_mm < w[1].z_mm));
        let r = time_report(&s, 1.0e9, 5);
        assert_eq!(
            r.short_layers.len(),
            p.layer_count(),
            "every layer is under a huge time"
        );
        assert_eq!(r.slowest.len(), 5);
        assert!(r.slowest.windows(2).all(|w| w[0].1 >= w[1].1));
    }
}
