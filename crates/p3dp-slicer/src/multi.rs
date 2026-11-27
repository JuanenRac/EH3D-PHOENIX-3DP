//! A print with several materials, as a plan: the tools (a spool of a feeder or a head of a toolchanger), the layers at which the tool changes, the purge that cleans the nozzle after
//! each change and the tower that holds it. The change happens between two layers, so a part is divided by height (a colour or a material from a layer on); painting a colour on a
//! surface and the offsets of the heads of a toolchanger are not here.
//!
//! The purge of a change is a volume that grows with the difference of temperature between the two materials and, going from a dark colour to a light one, with the difference of
//! brightness; a pair can be given its own figure. The tower is a rectangle on the bed with a wall on every layer and, on the layers where the tool changes, as many lines inside it
//! as the purge needs. A tower that would not fit the bed or would touch the part, a purge that does not fit in the tower and a printer that cannot hold the tools are refusals: the
//! tower is never made smaller and the purge never cut down.
//!
//! The figures are set in a file of settings ([`MultiConfig`]), not in the code. Nothing here has been tried on a printer.

use crate::config::SliceConfig;
use crate::error::SliceError;
use crate::geom::P;
use crate::plan::{Kind, Path, Plan};
use p3dp_core::kv;
use p3dp_core::machine::Zone;
use p3dp_core::CoreError;
use p3dp_profiles::filament::FilamentProfile;
use p3dp_profiles::multi::MultiCaps;

const KEYS: [&str; 13] = [
    "tower_x_mm",
    "tower_y_mm",
    "tower_width_mm",
    "tower_depth_mm",
    "tower_clearance_mm",
    "purge_base_mm3",
    "purge_per_degree_mm3",
    "purge_light_after_dark_mm3",
    "purge_max_mm3",
    "tool_change_s",
    "max_changes",
    "max_temp_step_c",
    "equipment_present",
];

/// The settings of a print with several materials.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiConfig {
    /// The corner of the tower with the smallest x and y.
    pub tower_x_mm: f64,
    pub tower_y_mm: f64,
    pub tower_width_mm: f64,
    pub tower_depth_mm: f64,
    /// The least distance between the tower and anything else on the bed.
    pub tower_clearance_mm: f64,
    /// The purge of any change.
    pub purge_base_mm3: f64,
    /// The purge that each degree of difference between the two materials adds.
    pub purge_per_degree_mm3: f64,
    /// The purge that a change from a dark colour to the lightest adds (less for a smaller step).
    pub purge_light_after_dark_mm3: f64,
    /// A purge above this is refused.
    pub purge_max_mm3: f64,
    /// The time a change takes, on top of the lines of the purge.
    pub tool_change_s: f64,
    /// The most changes of one print.
    pub max_changes: u32,
    /// Two materials that follow one another may differ by no more than this temperature.
    pub max_temp_step_c: f64,
    /// A system that is only optional (a kit) is there: the owner says it.
    pub equipment_present: bool,
}

impl Default for MultiConfig {
    fn default() -> MultiConfig {
        MultiConfig {
            tower_x_mm: 10.0,
            tower_y_mm: 10.0,
            tower_width_mm: 40.0,
            tower_depth_mm: 40.0,
            tower_clearance_mm: 3.0,
            purge_base_mm3: 70.0,
            purge_per_degree_mm3: 1.5,
            purge_light_after_dark_mm3: 120.0,
            purge_max_mm3: 400.0,
            tool_change_s: 30.0,
            max_changes: 100,
            max_temp_step_c: 50.0,
            equipment_present: false,
        }
    }
}

impl MultiConfig {
    /// Sets one key; an unknown key or a value out of its range is an error that names the key.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), CoreError> {
        let f = |lo: f64, hi: f64| kv::float_in(key, value, lo, hi);
        match key {
            "tower_x_mm" => self.tower_x_mm = f(0.0, 2000.0)?,
            "tower_y_mm" => self.tower_y_mm = f(0.0, 2000.0)?,
            "tower_width_mm" => self.tower_width_mm = f(5.0, 500.0)?,
            "tower_depth_mm" => self.tower_depth_mm = f(5.0, 500.0)?,
            "tower_clearance_mm" => self.tower_clearance_mm = f(0.0, 50.0)?,
            "purge_base_mm3" => self.purge_base_mm3 = f(0.0, 2000.0)?,
            "purge_per_degree_mm3" => self.purge_per_degree_mm3 = f(0.0, 50.0)?,
            "purge_light_after_dark_mm3" => self.purge_light_after_dark_mm3 = f(0.0, 2000.0)?,
            "purge_max_mm3" => self.purge_max_mm3 = f(1.0, 5000.0)?,
            "tool_change_s" => self.tool_change_s = f(0.0, 600.0)?,
            "max_changes" => self.max_changes = kv::int_in(key, value, 1, 10_000)? as u32,
            "max_temp_step_c" => self.max_temp_step_c = f(0.0, 300.0)?,
            "equipment_present" => self.equipment_present = kv::on_off(key, value)?,
            other => {
                return Err(CoreError::Invalid(format!(
                    "{other}: not a setting of a print with several materials"
                )))
            }
        }
        Ok(())
    }

    /// The value of a key as the file writes it.
    pub fn value_of(&self, key: &str) -> Option<String> {
        Some(match key {
            "tower_x_mm" => self.tower_x_mm.to_string(),
            "tower_y_mm" => self.tower_y_mm.to_string(),
            "tower_width_mm" => self.tower_width_mm.to_string(),
            "tower_depth_mm" => self.tower_depth_mm.to_string(),
            "tower_clearance_mm" => self.tower_clearance_mm.to_string(),
            "purge_base_mm3" => self.purge_base_mm3.to_string(),
            "purge_per_degree_mm3" => self.purge_per_degree_mm3.to_string(),
            "purge_light_after_dark_mm3" => self.purge_light_after_dark_mm3.to_string(),
            "purge_max_mm3" => self.purge_max_mm3.to_string(),
            "tool_change_s" => self.tool_change_s.to_string(),
            "max_changes" => self.max_changes.to_string(),
            "max_temp_step_c" => self.max_temp_step_c.to_string(),
            "equipment_present" => if self.equipment_present { "on" } else { "off" }.to_string(),
            _ => return None,
        })
    }

    /// Every key with its value, to be read back by [`MultiConfig::from_text`].
    pub fn to_text(&self) -> String {
        KEYS.iter()
            .filter_map(|k| self.value_of(k).map(|v| format!("{k} = {v}\n")))
            .collect()
    }

    /// Reads a file of settings; the first mistake is the answer, with its line.
    pub fn from_text(text: &str) -> Result<MultiConfig, CoreError> {
        let mut c = MultiConfig::default();
        for e in kv::parse(text)? {
            c.set(&e.key, &e.value)
                .map_err(|err| CoreError::Invalid(format!("line {}: {err}", e.line)))?;
        }
        Ok(c)
    }
}

/// A spool of a feeder or a head of a toolchanger, with the material that is in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// The number the program gives the firmware (`T0`...`T15`).
    pub slot: u8,
    pub filament_id: String,
    pub nozzle_c: f64,
    pub diameter_mm: f64,
    /// The colour, `0xRRGGBB`.
    pub colour_rgb: u32,
}

impl Tool {
    /// A tool for a filament of the catalog, at the temperature it starts from.
    pub fn from_filament(slot: u8, f: &FilamentProfile, colour_rgb: u32) -> Tool {
        Tool {
            slot,
            filament_id: f.id.clone(),
            nozzle_c: f.nozzle_c.best,
            diameter_mm: f.diameter_mm,
            colour_rgb,
        }
    }

    /// How light the colour is, 0 (black) to 1 (white).
    pub fn brightness(&self) -> f64 {
        let c = |shift: u32| f64::from((self.colour_rgb >> shift) & 0xff) / 255.0;
        0.2126 * c(16) + 0.7152 * c(8) + 0.0722 * c(0)
    }
}

/// What to print and with what.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiJob {
    pub tools: Vec<Tool>,
    /// The tool the print starts with.
    pub first: u8,
    /// `(layer, tool)`: from this layer of the plan on, the tool prints. The layers rise, the first is above 0, and a change is to another tool than the one before.
    pub changes: Vec<(usize, u8)>,
    /// `(from, to, mm3)`: a purge of a pair that is not the one the formula gives.
    pub purge_overrides: Vec<(u8, u8, f64)>,
    pub config: MultiConfig,
}

impl MultiJob {
    /// The purge of a change from one tool to another, in cubic millimetres.
    pub fn purge_mm3(&self, from: &Tool, to: &Tool) -> f64 {
        if let Some((_, _, v)) = self
            .purge_overrides
            .iter()
            .find(|(a, b, _)| *a == from.slot && *b == to.slot)
        {
            return *v;
        }
        let c = &self.config;
        let step = (to.nozzle_c - from.nozzle_c).abs();
        let lighter = (to.brightness() - from.brightness()).max(0.0);
        c.purge_base_mm3 + c.purge_per_degree_mm3 * step + c.purge_light_after_dark_mm3 * lighter
    }

    fn tool(&self, slot: u8) -> Option<&Tool> {
        self.tools.iter().find(|t| t.slot == slot)
    }
}

/// A change of tool in the plan, as the writer of G-code reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolChange {
    /// The layer of the plan it happens at, before any line of the layer.
    pub layer: usize,
    pub tool: u8,
    pub nozzle_c: f64,
    /// The purge that was asked for, in cubic millimetres.
    pub purge_mm3: f64,
    /// The seconds the change takes besides the lines of the purge.
    pub time_s: f64,
}

/// What one change came to.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeReport {
    pub layer: usize,
    pub from: u8,
    pub to: u8,
    pub wanted_mm3: f64,
    /// What the lines of the tower lay down: never less than wanted, and less than one line more.
    pub laid_mm3: f64,
    pub lines: usize,
}

/// What a tool prints.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    pub slot: u8,
    pub filament_id: String,
    pub volume_mm3: f64,
    pub length_mm: f64,
}

/// What a plan with several materials comes to.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiReport {
    pub changes: Vec<ChangeReport>,
    pub tools: Vec<ToolUse>,
    /// The plastic of the lines of the purge, of all the changes.
    pub purge_mm3: f64,
    /// The plastic of the whole tower (its walls and the lines of the purge).
    pub tower_mm3: f64,
    pub change_time_s: f64,
}

fn refuse(s: String) -> SliceError {
    SliceError::Config(format!("several materials: {s}"))
}

fn check_tools(plan: &Plan, job: &MultiJob, caps: Option<&MultiCaps>) -> Result<(), SliceError> {
    let c = &job.config;
    if job.tools.is_empty() {
        return Err(refuse("there is no tool".to_string()));
    }
    let several = job.tools.len() > 1 || !job.changes.is_empty();
    if several {
        let Some(caps) = caps else {
            return Err(refuse(format!(
                "{} has no system for several materials in its profile",
                plan.machine.id
            )));
        };
        if caps.optional && !c.equipment_present {
            return Err(refuse(format!(
                "{} has only an optional system ({}): say that it is fitted (equipment_present = on)",
                plan.machine.id, caps.text
            )));
        }
        if job.tools.len() > usize::from(caps.max_tools) {
            return Err(refuse(format!(
                "{} tools, and {} takes {} ({})",
                job.tools.len(),
                plan.machine.id,
                caps.max_tools,
                caps.text
            )));
        }
    }
    let mut seen = Vec::new();
    for t in &job.tools {
        if t.slot > 15 || seen.contains(&t.slot) {
            return Err(refuse(format!(
                "tool {} is out of 0 to 15 or repeated",
                t.slot
            )));
        }
        seen.push(t.slot);
        plan.machine.limits.check(Zone::Nozzle, t.nozzle_c)?;
        if (t.diameter_mm - plan.config.filament_diameter_mm).abs() > 0.01 {
            return Err(refuse(format!(
                "tool {} takes {} mm filament and the plan is for {} mm",
                t.slot, t.diameter_mm, plan.config.filament_diameter_mm
            )));
        }
    }
    if job.tool(job.first).is_none() {
        return Err(refuse(format!(
            "the first tool {} is not one of them",
            job.first
        )));
    }
    if job.changes.len() > c.max_changes as usize {
        return Err(refuse(format!(
            "{} changes, more than max_changes {}",
            job.changes.len(),
            c.max_changes
        )));
    }
    let mut now = job.first;
    let mut last_layer = 0usize;
    for (layer, slot) in &job.changes {
        if *layer <= last_layer || *layer >= plan.layers.len() {
            return Err(refuse(format!(
                "a change at layer {layer}: the layers rise, from 1 to {}",
                plan.layers.len().saturating_sub(1)
            )));
        }
        let Some(to) = job.tool(*slot) else {
            return Err(refuse(format!(
                "a change to tool {slot}, which is not one of them"
            )));
        };
        if *slot == now {
            return Err(refuse(format!(
                "the change at layer {layer} is to the tool it has"
            )));
        }
        let from = job
            .tool(now)
            .ok_or_else(|| refuse("a tool is missing".to_string()))?;
        let step = (to.nozzle_c - from.nozzle_c).abs();
        if step > c.max_temp_step_c + 1e-9 {
            return Err(refuse(format!(
                "tool {now} to tool {slot} is {step} C apart and max_temp_step_c is {}",
                c.max_temp_step_c
            )));
        }
        let purge = job.purge_mm3(from, to);
        if !purge.is_finite() || !(0.0..=c.purge_max_mm3).contains(&purge) {
            return Err(refuse(format!(
                "the purge from tool {now} to tool {slot} is {purge:.0} mm3 and purge_max_mm3 is {}",
                c.purge_max_mm3
            )));
        }
        now = *slot;
        last_layer = *layer;
    }
    Ok(())
}

fn check_tower(plan: &Plan, c: &MultiConfig) -> Result<(), SliceError> {
    let [bx, by, _] = plan.machine.volume_mm;
    let m = plan.config.bed_margin_mm;
    let (x0, y0, x1, y1) = (
        c.tower_x_mm,
        c.tower_y_mm,
        c.tower_x_mm + c.tower_width_mm,
        c.tower_y_mm + c.tower_depth_mm,
    );
    if x0 < m - 1e-9 || y0 < m - 1e-9 || x1 > bx - m + 1e-9 || y1 > by - m + 1e-9 {
        return Err(refuse(format!(
            "the tower ({x0} to {x1}, {y0} to {y1}) is not inside the bed with the margin of {m} mm"
        )));
    }
    let k = c.tower_clearance_mm;
    for l in &plan.layers {
        for p in l.paths.iter().flat_map(|p| &p.points) {
            if p[0] > x0 - k && p[0] < x1 + k && p[1] > y0 - k && p[1] < y1 + k {
                return Err(refuse(format!(
                    "the print comes within {k} mm of the tower at layer {}, at ({:.2}, {:.2})",
                    l.index, p[0], p[1]
                )));
            }
        }
    }
    Ok(())
}

/// The lines of the wall of the tower on a layer.
fn tower_wall(c: &MultiConfig, width_mm: f64) -> Path {
    let (x0, y0) = (c.tower_x_mm, c.tower_y_mm);
    let (x1, y1) = (x0 + c.tower_width_mm, y0 + c.tower_depth_mm);
    Path {
        kind: Kind::Tower,
        points: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        closed: true,
        width_mm,
    }
}

/// The lines inside the tower that lay down a purge: back and forth across it. Returns the lines, and what they lay down.
fn purge_lines(
    c: &MultiConfig,
    cfg: &SliceConfig,
    height_mm: f64,
    wanted_mm3: f64,
    layer: usize,
) -> Result<(Vec<Path>, f64), SliceError> {
    if wanted_mm3 <= 0.0 {
        return Ok((Vec::new(), 0.0));
    }
    let w = cfg.line_width_mm;
    let area = cfg.line_area_for(w, height_mm) * cfg.extrusion_multiplier;
    let inner_w = c.tower_width_mm - 2.0 * w;
    let inner_d = c.tower_depth_mm - 2.0 * w;
    let per_line = inner_w * area;
    let spacing = cfg.spacing_for(w, height_mm);
    let room = if inner_d < 0.0 || per_line <= 0.0 {
        0
    } else {
        (inner_d / spacing).floor() as usize + 1
    };
    let need = (wanted_mm3 / per_line).ceil().max(1.0) as usize;
    if need > room {
        return Err(refuse(format!(
            "the purge of {wanted_mm3:.0} mm3 at layer {layer} needs {need} lines and the tower has room for {room}: make it larger (tower_width_mm, tower_depth_mm) or the purge smaller"
        )));
    }
    let x0 = c.tower_x_mm + w;
    let y0 = c.tower_y_mm + w;
    let step = if need > 1 {
        inner_d / (need - 1) as f64
    } else {
        0.0
    };
    let mut out = Vec::with_capacity(need);
    for i in 0..need {
        let y = if need == 1 {
            y0 + inner_d / 2.0
        } else {
            y0 + step * i as f64
        };
        let (a, b): (P, P) = if i % 2 == 0 {
            ([x0, y], [x0 + inner_w, y])
        } else {
            ([x0 + inner_w, y], [x0, y])
        };
        out.push(Path {
            kind: Kind::Tower,
            points: vec![a, b],
            closed: false,
            width_mm: w,
        });
    }
    Ok((out, need as f64 * per_line))
}

/// Puts the tools, the changes and the tower in a plan. The plan is changed only when everything is allowed; otherwise it is left as it was and the error says what is wrong.
pub fn apply(
    plan: &mut Plan,
    job: &MultiJob,
    caps: Option<&MultiCaps>,
) -> Result<MultiReport, SliceError> {
    check_tools(plan, job, caps)?;
    let first = job
        .tool(job.first)
        .ok_or_else(|| refuse("the first tool is missing".to_string()))?
        .clone();
    plan.machine.limits.check(Zone::Nozzle, first.nozzle_c)?;
    let mut next = plan.clone();
    let mut report = MultiReport {
        changes: Vec::new(),
        tools: Vec::new(),
        purge_mm3: 0.0,
        tower_mm3: 0.0,
        change_time_s: 0.0,
    };
    let mut events: Vec<ToolChange> = Vec::new();
    if !job.changes.is_empty() {
        let c = &job.config;
        check_tower(plan, c)?;
        let mut now = first.clone();
        let mut change_at = job.changes.iter().peekable();
        for l in &mut next.layers {
            let mut lead = vec![tower_wall(c, plan.config.line_width_mm)];
            if let Some(&&(layer, slot)) = change_at.peek() {
                if layer == l.index {
                    change_at.next();
                    let to = job
                        .tool(slot)
                        .ok_or_else(|| refuse("a tool is missing".to_string()))?
                        .clone();
                    let wanted = job.purge_mm3(&now, &to);
                    let (lines, laid) = purge_lines(c, &plan.config, l.height_mm, wanted, l.index)?;
                    report.changes.push(ChangeReport {
                        layer: l.index,
                        from: now.slot,
                        to: slot,
                        wanted_mm3: wanted,
                        laid_mm3: laid,
                        lines: lines.len(),
                    });
                    report.purge_mm3 += laid;
                    events.push(ToolChange {
                        layer: l.index,
                        tool: slot,
                        nozzle_c: to.nozzle_c,
                        purge_mm3: wanted,
                        time_s: c.tool_change_s,
                    });
                    report.change_time_s += c.tool_change_s;
                    lead.extend(lines);
                    now = to;
                }
            }
            lead.extend(std::mem::take(&mut l.paths));
            l.paths = lead;
        }
    }
    next.config.nozzle_c = first.nozzle_c;
    next.first_tool = Some(first.slot);
    next.tool_changes = events;
    next.layer_scales = next.compute_layer_scales();
    // what each tool prints: the lines of a layer go to the tool that is on it
    let mut volume = vec![0.0f64; job.tools.len()];
    let mut active = job
        .tools
        .iter()
        .position(|t| t.slot == first.slot)
        .unwrap_or(0);
    for l in &next.layers {
        if let Some(e) = next.tool_changes.iter().find(|e| e.layer == l.index) {
            active = job
                .tools
                .iter()
                .position(|t| t.slot == e.tool)
                .unwrap_or(active);
        }
        for p in &l.paths {
            let v = p.length_mm() * next.line_area_mm2(l.height_mm, p.kind, p.width_mm);
            volume[active] += v;
            if p.kind == Kind::Tower {
                report.tower_mm3 += v;
            }
        }
    }
    let area = next.config.filament_area_mm2();
    report.tools = job
        .tools
        .iter()
        .zip(volume)
        .map(|(t, v)| ToolUse {
            slot: t.slot,
            filament_id: t.filament_id.clone(),
            volume_mm3: v,
            length_mm: v / area,
        })
        .collect();
    *plan = next;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{plan as make_plan, Machine};
    use p3dp_core::mesh::cube;
    use p3dp_core::Dialect;
    use p3dp_core::TempLimits;
    use p3dp_profiles::{caps_of, Catalog, FilamentCatalog};

    fn machine() -> Machine {
        Machine {
            id: "bambu-lab-a1".into(),
            dialect: Dialect::Klipper,
            volume_mm: [256.0, 256.0, 256.0],
            limits: TempLimits {
                nozzle_c: 300.0,
                bed_c: 100.0,
                chamber_c: None,
            },
        }
    }

    fn base() -> Plan {
        make_plan(&cube(20.0), &machine(), &SliceConfig::default()).unwrap()
    }

    fn tools() -> Vec<Tool> {
        let f = FilamentCatalog::builtin().unwrap();
        let pla = f.find("generic-pla").unwrap();
        let petg = f.find("generic-petg").unwrap();
        vec![
            Tool::from_filament(0, pla, 0x202020),
            Tool::from_filament(1, petg, 0xf0f0f0),
        ]
    }

    fn job(changes: Vec<(usize, u8)>) -> MultiJob {
        MultiJob {
            tools: tools(),
            first: 0,
            changes,
            purge_overrides: Vec::new(),
            config: MultiConfig {
                tower_x_mm: 10.0,
                tower_y_mm: 10.0,
                ..MultiConfig::default()
            },
        }
    }

    fn a1() -> MultiCaps {
        caps_of(Catalog::builtin().unwrap().find("bambu-lab-a1").unwrap()).unwrap()
    }

    #[test]
    fn the_settings_are_read_checked_and_written_back() {
        let c = MultiConfig::default();
        assert_eq!(MultiConfig::from_text(&c.to_text()).unwrap(), c);
        assert_eq!(c.to_text().lines().count(), KEYS.len());
        let c = MultiConfig::from_text("tower_width_mm = 40\nequipment_present = on\n").unwrap();
        assert!((c.tower_width_mm - 40.0).abs() < 1e-12 && c.equipment_present);
        for (bad, want) in [
            ("tower_width_mm = 1", "tower_width_mm"),
            ("max_changes = 0", "max_changes"),
            ("purge = 3", "purge"),
            ("equipment_present = maybe", "equipment_present"),
        ] {
            let e = MultiConfig::from_text(bad).unwrap_err().to_string();
            assert!(e.contains(want) && e.contains("line 1"), "{bad}: {e}");
        }
    }

    #[test]
    fn a_two_material_print_gets_a_purge_tower() {
        let mut p = base();
        let n = p.layers.len();
        let before = p.extrusion_mm3();
        let r = apply(&mut p, &job(vec![(n / 2, 1)]), Some(&a1())).unwrap();
        assert_eq!(r.changes.len(), 1);
        let ch = &r.changes[0];
        assert_eq!((ch.layer, ch.from, ch.to), (n / 2, 0, 1));
        // the lines lay down what was asked, and less than one line more
        assert!(ch.laid_mm3 >= ch.wanted_mm3 - 1e-9, "{ch:?}");
        let one_line = ch.laid_mm3 / ch.lines as f64;
        assert!(ch.laid_mm3 - ch.wanted_mm3 < one_line + 1e-9, "{ch:?}");
        // the tower is on every layer, and the purge only on the layer of the change
        assert!(p.layers.iter().all(|l| l.paths[0].kind == Kind::Tower));
        let with_lines: Vec<usize> = p
            .layers
            .iter()
            .filter(|l| l.paths.iter().filter(|x| x.kind == Kind::Tower).count() > 1)
            .map(|l| l.index)
            .collect();
        assert_eq!(with_lines, vec![n / 2]);
        // the tools print everything there is, and the tower is part of it
        let sum: f64 = r.tools.iter().map(|t| t.volume_mm3).sum();
        assert!((sum - p.extrusion_mm3()).abs() < 1e-6);
        assert!((p.extrusion_mm3() - before - r.tower_mm3).abs() < 1e-6);
        assert!(r.tools[0].volume_mm3 > 0.0);
        assert!(r.tools[1].volume_mm3 > ch.laid_mm3);
        assert_eq!(p.first_tool, Some(0));
        assert!((p.config.nozzle_c - tools()[0].nozzle_c).abs() < 1e-9);
    }

    #[test]
    fn the_gcode_changes_the_tool_with_its_temperature_before_the_purge() {
        let mut p = base();
        let n = p.layers.len();
        apply(&mut p, &job(vec![(n / 2, 1)]), Some(&a1())).unwrap();
        let g = p.gcode("test").unwrap();
        let lines: Vec<&str> = g.lines().collect();
        let first_t = lines.iter().position(|l| *l == "T0").expect("T0");
        let change = lines.iter().position(|l| *l == "T1").expect("T1");
        assert!(first_t < change);
        // the filament of the tool that was printing is pulled back before the change, not after it
        assert!(
            lines[change - 2].starts_with("G1 E-"),
            "{}",
            lines[change - 2]
        );
        // the temperature of the new tool is set and waited for just before the tool is picked
        let petg = tools()[1].nozzle_c;
        assert!(
            lines[change - 1].starts_with(&format!("M109 S{petg}")),
            "{}",
            lines[change - 1]
        );
        assert_eq!(lines.iter().filter(|l| l.starts_with("T")).count(), 2);
        assert!(g.contains("TYPE:purge tower"));
        // the program stays within the limits of the machine
        assert!(p3dp_gcode::check_program(&g, &machine().limits).is_empty());
    }

    #[test]
    fn reprapfirmware_gives_each_tool_its_own_temperature_before_it_is_waited_for() {
        let mut p = base();
        let n = p.layers.len();
        apply(&mut p, &job(vec![(n / 2, 1)]), Some(&a1())).unwrap();
        p.machine.dialect = p3dp_core::Dialect::Rrf;
        let g = p.gcode("test").unwrap();
        let lines: Vec<&str> = g.lines().collect();
        let change = lines.iter().position(|l| *l == "T1").expect("T1");
        let petg = tools()[1].nozzle_c;
        assert_eq!(lines[change - 1], format!("G10 P1 S{petg}"));
        assert_eq!(lines[change + 1], "M116");
        assert!(!g.contains(&format!("G10 P0 S{petg}")), "{g}");
        // the program is turned off with the tool that is in use
        assert!(g.contains("G10 P1 S0"));
    }

    #[test]
    fn what_the_printer_cannot_hold_is_refused() {
        let cat = Catalog::builtin().unwrap();
        let mut p = base();
        let keep = p.clone();
        let mut j = job(vec![(3, 1)]);
        // no system at all
        let e = apply(&mut p, &j, None).unwrap_err().to_string();
        assert!(e.contains("no system for several materials"), "{e}");
        // an optional kit that the owner has not said is fitted
        let mk4 = caps_of(cat.find("prusa-mk4s").unwrap()).unwrap();
        let e = apply(&mut p, &j, Some(&mk4)).unwrap_err().to_string();
        assert!(e.contains("equipment_present"), "{e}");
        j.config.equipment_present = true;
        assert!(apply(&mut p, &j, Some(&mk4)).is_ok());
        // too many tools for a feeder of four
        let mut p = keep.clone();
        let mut j = job(vec![(3, 1)]);
        let f = FilamentCatalog::builtin().unwrap();
        for s in 2..5 {
            j.tools.push(Tool::from_filament(
                s,
                f.find("generic-pla").unwrap(),
                0x808080,
            ));
        }
        let e = apply(&mut p, &j, Some(&a1())).unwrap_err().to_string();
        assert!(e.contains("5 tools") && e.contains("takes 4"), "{e}");
        assert_eq!(p, keep, "a refusal leaves the plan as it was");
    }

    #[test]
    fn a_change_that_is_not_allowed_is_refused_and_never_changed() {
        let keep = base();
        let n = keep.layers.len();
        let a = a1();
        let cases: Vec<(MultiJob, &str)> = vec![
            (job(vec![(0, 1)]), "the layers rise"),
            (job(vec![(n, 1)]), "the layers rise"),
            (job(vec![(5, 1), (5, 0)]), "the layers rise"),
            (job(vec![(5, 0)]), "to the tool it has"),
            (job(vec![(5, 7)]), "not one of them"),
            (
                MultiJob {
                    first: 9,
                    ..job(vec![(5, 1)])
                },
                "first tool",
            ),
            (
                MultiJob {
                    purge_overrides: vec![(0, 1, 9000.0)],
                    ..job(vec![(5, 1)])
                },
                "purge_max_mm3",
            ),
        ];
        for (j, want) in cases {
            let mut p = keep.clone();
            let e = apply(&mut p, &j, Some(&a)).unwrap_err().to_string();
            assert!(e.contains(want), "{want}: {e}");
            assert_eq!(p, keep);
        }
        // two materials too far apart in temperature
        let mut hot = job(vec![(5, 1)]);
        hot.tools[1].nozzle_c = 290.0;
        hot.tools[0].nozzle_c = 200.0;
        let mut p = keep.clone();
        let e = apply(&mut p, &hot, Some(&a)).unwrap_err().to_string();
        assert!(e.contains("max_temp_step_c"), "{e}");
        // a temperature over what the machine takes
        hot.tools[1].nozzle_c = 310.0;
        hot.config.max_temp_step_c = 300.0;
        assert!(matches!(
            apply(&mut p, &hot, Some(&a)).unwrap_err(),
            SliceError::Temp(_)
        ));
        // a filament of another diameter
        let mut wide = job(vec![(5, 1)]);
        wide.tools[1].diameter_mm = 2.85;
        let e = apply(&mut p, &wide, Some(&a)).unwrap_err().to_string();
        assert!(e.contains("2.85"), "{e}");
        assert_eq!(p, keep);
    }

    #[test]
    fn the_tower_is_inside_the_bed_away_from_the_part_and_big_enough() {
        let keep = base();
        let a = a1();
        let mut j = job(vec![(5, 1)]);
        // the part is in the middle of the bed: put the tower under it
        j.config.tower_x_mm = 118.0;
        j.config.tower_y_mm = 118.0;
        let mut p = keep.clone();
        let e = apply(&mut p, &j, Some(&a)).unwrap_err().to_string();
        assert!(e.contains("comes within"), "{e}");
        // past the edge of the bed
        j.config.tower_x_mm = 240.0;
        let e = apply(&mut p, &j, Some(&a)).unwrap_err().to_string();
        assert!(e.contains("not inside the bed"), "{e}");
        // too small for the purge: refused, not made larger
        let mut j = job(vec![(5, 1)]);
        j.config.tower_depth_mm = 6.0;
        let e = apply(&mut p, &j, Some(&a)).unwrap_err().to_string();
        assert!(e.contains("room for"), "{e}");
        assert_eq!(p, keep);
    }

    #[test]
    fn the_purge_follows_the_temperature_and_the_brightness() {
        let j = job(vec![]);
        let (dark, light) = (&j.tools[0], &j.tools[1]);
        let same = j.purge_mm3(dark, dark);
        assert!((same - j.config.purge_base_mm3).abs() < 1e-9);
        // dark to light costs more than light to dark of the same materials
        let up = j.purge_mm3(dark, light);
        let mut back_light = light.clone();
        back_light.nozzle_c = dark.nozzle_c;
        let mut to_light = light.clone();
        to_light.nozzle_c = dark.nozzle_c;
        assert!(j.purge_mm3(dark, &to_light) > j.purge_mm3(&back_light, dark));
        assert!(up > same);
        // an own figure wins
        let mut o = j.clone();
        o.purge_overrides.push((0, 1, 33.0));
        assert!((o.purge_mm3(dark, light) - 33.0).abs() < 1e-12);
        assert!(dark.brightness() < 0.2 && light.brightness() > 0.9);
    }

    #[test]
    fn shapes_from_a_fixed_seed_keep_the_tower_and_the_totals() {
        use p3dp_core::rng::Rng;
        let mut rng = Rng::new(0x003d_9a11);
        let a = a1();
        for _ in 0..24 {
            let size = 10.0 + (rng.next_u64() % 30) as f64;
            let cfg = SliceConfig::default();
            let base = make_plan(&cube(size), &machine(), &cfg).unwrap();
            let n = base.layers.len();
            if n < 4 {
                continue;
            }
            let k = 1 + (rng.next_u64() as usize % 3).min(n - 3);
            let mut at: Vec<usize> = (0..k)
                .map(|_| 1 + rng.next_u64() as usize % (n - 1))
                .collect();
            at.sort_unstable();
            at.dedup();
            let changes: Vec<(usize, u8)> = at
                .iter()
                .enumerate()
                .map(|(i, l)| (*l, if i % 2 == 0 { 1 } else { 0 }))
                .collect();
            let mut p = base.clone();
            let r = apply(&mut p, &job(changes.clone()), Some(&a)).unwrap();
            // the tower never leaves its rectangle, the part never enters it
            let (x0, y0, x1, y1) = (10.0, 10.0, 50.0, 50.0);
            for l in &p.layers {
                for q in &l.paths {
                    let inside = q.points.iter().all(|s| {
                        s[0] >= x0 - 1e-9
                            && s[0] <= x1 + 1e-9
                            && s[1] >= y0 - 1e-9
                            && s[1] <= y1 + 1e-9
                    });
                    assert_eq!(q.kind == Kind::Tower, inside, "layer {}", l.index);
                }
            }
            // every change laid its purge, the plan adds up, and the time grew by the changes
            assert_eq!(r.changes.len(), changes.len());
            for c in &r.changes {
                assert!(c.laid_mm3 >= c.wanted_mm3 - 1e-9);
            }
            let sum: f64 = r.tools.iter().map(|t| t.volume_mm3).sum();
            assert!((sum - p.extrusion_mm3()).abs() < 1e-6);
            let (t0, t1) = (base.estimated_seconds(), p.estimated_seconds());
            assert!(
                t1 >= t0 + r.change_time_s - 1e-6,
                "{t0} {t1} {}",
                r.change_time_s
            );
            assert!(
                p3dp_gcode::check_program(&p.gcode("t").unwrap(), &machine().limits).is_empty()
            );
        }
    }
}
