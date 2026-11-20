//! The work table of the Prepare tab, without a window: the parts on the bed, the camera of the 3D view, the slice that runs in the background and the estimates for the bar at the
//! bottom. The window draws what is here and passes on what the person does; the rules (what goes past the edge, what the bar says, when a slice may start) are here and are tested.

use crate::app_state::AppState;
use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::objects::Plate;
use crate::preview::{format_duration, PreviewState};
use crate::shade::StdView;
use crate::slice_job::{Poll, SliceJob, FIRST_GUESS_S};
use p3dp_core::Mesh;
use p3dp_profiles::filament::{FilamentProfile, Nozzle};
use p3dp_profiles::PrinterProfile;
use p3dp_slicer::report::material;
use p3dp_viewer::{OrbitCamera, Viewport};

/// A thing the bed has to say about its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// This many parts go past the edge of the bed or its height.
    OutOfBed(usize),
    /// This many pairs of parts lie one over the other.
    Overlap(usize),
    /// This many parts did not fit when the bed was arranged.
    LeftOut(usize),
}

impl Warning {
    pub fn text(self, lang: Lang) -> String {
        let (msg, n) = match self {
            Warning::OutOfBed(n) => (Msg::WarnOutOfBed, n),
            Warning::Overlap(n) => (Msg::WarnOverlap, n),
            Warning::LeftOut(n) => (Msg::WarnLeftOut, n),
        };
        fill(msg.tr(lang), &[&n.to_string()])
    }
}

/// How the slice that was running ended, as far as the window is told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliceEvent {
    Idle,
    Running { fraction: f32, elapsed_s: f64 },
    Finished { ok: bool, seconds: f64 },
    Lost,
}

/// What the bar at the bottom shows after a slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimates {
    pub seconds: f64,
    pub layers: usize,
    pub length_m: f64,
    pub mass_g: f64,
    pub cost: f64,
    pub height_mm: f64,
}

impl Estimates {
    /// The figures of a plan. The cost is 0 when the price in the settings is 0.
    pub fn of(preview: &PreviewState, config: &AppConfig) -> Option<Estimates> {
        let plan = preview.plan.as_ref()?;
        let m = material(plan, preview.density_g_cm3, config.price_filament_per_kg).ok()?;
        Some(Estimates {
            seconds: plan.estimated_seconds(),
            layers: plan.layer_count(),
            length_m: m.length_mm / 1000.0,
            mass_g: m.mass_g,
            cost: m.cost,
            height_mm: plan.height_mm(),
        })
    }

    /// The label and the value of each figure, in a language; the cost is left out when there is no price.
    pub fn cells(&self, config: &AppConfig, lang: Lang) -> Vec<(&'static str, String)> {
        let mut out = vec![
            (Msg::EstTime.tr(lang), format_duration(self.seconds)),
            (
                Msg::EstMaterial.tr(lang),
                format!("{:.2} m - {:.1} g", self.length_m, self.mass_g),
            ),
        ];
        if config.price_filament_per_kg > 0.0 {
            out.push((Msg::EstCost.tr(lang), format!("{:.2}", self.cost)));
        }
        out.push((
            Msg::EstLayers.tr(lang),
            format!("{} - {:.1} mm", self.layers, self.height_mm),
        ));
        out
    }
}

/// What the world mesh of a part depends on, to know when the cache is stale.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    id: u32,
    pos: [f64; 2],
    rot: [f64; 3],
    scale: [f64; 3],
}

/// The work table.
pub struct Studio {
    pub plate: Plate,
    pub camera: OrbitCamera,
    /// Put the camera to look at the whole printer on the next frame.
    pub fit_next: bool,
    job: Option<SliceJob>,
    /// How long the last slice took, for the bar of the next one.
    pub last_slice_s: f64,
    /// The share of the bar while a slice runs.
    pub progress: f32,
    /// Parts that did not fit at the last arrangement.
    pub left_out: usize,
    /// The bed changed after the last slice, so the plan on view is not of what is on the bed.
    pub stale: bool,
    world: Vec<(u32, Mesh)>,
    keys: Vec<Key>,
    changed: bool,
}

impl Default for Studio {
    fn default() -> Studio {
        Studio {
            plate: Plate::new(),
            camera: OrbitCamera::default(),
            fit_next: true,
            job: None,
            last_slice_s: FIRST_GUESS_S,
            progress: 0.0,
            left_out: 0,
            stale: false,
            world: Vec::new(),
            keys: Vec::new(),
            changed: false,
        }
    }
}

impl Studio {
    pub fn new() -> Studio {
        Studio::default()
    }

    pub fn is_slicing(&self) -> bool {
        self.job.is_some()
    }

    /// Adds a part, and arranges the bed when the settings ask for it. Returns the id, or `None` when the bed holds all it can.
    pub fn add_model(
        &mut self,
        name: &str,
        mesh: Mesh,
        volume_mm: [f64; 3],
        config: &AppConfig,
    ) -> Option<u32> {
        let id = self.plate.add(name, mesh, [volume_mm[0], volume_mm[1]])?;
        if config.arrange_on_add && self.plate.len() > 1 {
            self.arrange(volume_mm, config);
        }
        self.touched();
        Some(id)
    }

    fn touched(&mut self) {
        self.changed = true;
        self.stale = true;
    }

    /// Whether the bed changed since the last call (the window then refreshes what depends on it).
    pub fn take_changed(&mut self) -> bool {
        let c = self.changed;
        self.changed = false;
        c
    }

    /// Arranges the bed with the gap of the settings.
    pub fn arrange(&mut self, volume_mm: [f64; 3], config: &AppConfig) {
        let r = self.plate.arrange(
            [volume_mm[0], volume_mm[1]],
            config.arrange_gap_mm,
            self.margin(),
        );
        self.left_out = r.left_out.len();
        self.touched();
    }

    /// The margin kept from the edge of the bed when parts are arranged.
    pub fn margin(&self) -> f64 {
        2.0
    }

    /// Stands the selected part the way that needs the fewest supports.
    pub fn auto_orient_selected(&mut self) -> bool {
        let Some(sel) = self.plate.selected_object() else {
            return false;
        };
        let turn = crate::objects::auto_orient(sel);
        if let Some(o) = self.plate.selected_mut() {
            o.rot_deg = turn;
            self.touched();
            return true;
        }
        false
    }

    pub fn remove_selected(&mut self) -> bool {
        let Some(id) = self.plate.selected else {
            return false;
        };
        let gone = self.plate.remove(id);
        if gone {
            self.touched();
        }
        gone
    }

    /// Makes a copy of the selected part and selects it; the bed is arranged when the settings ask for it. `false` when nothing is selected or the bed holds all it can.
    pub fn duplicate_selected(&mut self, volume_mm: [f64; 3], config: &AppConfig) -> bool {
        let Some(id) = self.plate.selected else {
            return false;
        };
        let copies = self.plate.duplicate(id, 1);
        if copies.is_empty() {
            return false;
        }
        if config.arrange_on_add {
            self.arrange(volume_mm, config);
        }
        self.touched();
        true
    }

    /// Notes that a part was changed by the window (a number typed in the right panel).
    pub fn mark_edited(&mut self) {
        self.touched();
    }

    /// What the bed has to say about its parts for a printer of this volume.
    pub fn warnings(&self, volume_mm: [f64; 3]) -> Vec<Warning> {
        let mut out = Vec::new();
        let outside = self.plate.out_of_bounds(volume_mm, 0.0).len();
        if outside > 0 {
            out.push(Warning::OutOfBed(outside));
        }
        let pairs = self.plate.overlaps().len();
        if pairs > 0 {
            out.push(Warning::Overlap(pairs));
        }
        if self.left_out > 0 {
            out.push(Warning::LeftOut(self.left_out));
        }
        out
    }

    /// The parts on the bed as meshes, made again only when a part was moved, turned or scaled.
    pub fn world_parts(&mut self) -> &[(u32, Mesh)] {
        let keys: Vec<Key> = self
            .plate
            .objects
            .iter()
            .map(|o| Key {
                id: o.id,
                pos: o.pos,
                rot: o.rot_deg,
                scale: o.scale,
            })
            .collect();
        if keys != self.keys {
            self.world = self
                .plate
                .objects
                .iter()
                .filter_map(|o| o.world_mesh().ok().map(|m| (o.id, m)))
                .collect();
            self.keys = keys;
            self.changed = true;
        }
        &self.world
    }

    /// The boxes of the parts, for a click to find the one under the pointer.
    pub fn boxes(&self) -> Vec<(u32, [f64; 3], [f64; 3])> {
        self.plate
            .objects
            .iter()
            .filter_map(|o| o.world_bounds().map(|(lo, hi)| (o.id, lo, hi)))
            .collect()
    }

    /// Puts what is on the bed in the state of the program (the model the lists, the report and the project use).
    pub fn sync(&mut self, state: &mut AppState) {
        match self.plate.merged() {
            Ok(m) => {
                state.model = Some(m);
                state.model_name = self
                    .plate
                    .objects
                    .iter()
                    .map(|o| o.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" + ");
            }
            Err(_) => {
                state.model = None;
                state.model_name.clear();
            }
        }
    }

    pub fn look_from(&mut self, view: StdView, volume_mm: [f64; 3], vp: Viewport) {
        view.apply(&mut self.camera, volume_mm, vp);
        self.fit_next = false;
    }

    /// Fits the camera when it was asked for (a new printer, a reset), looking from the corner.
    pub fn fit_if_asked(&mut self, volume_mm: [f64; 3], vp: Viewport) {
        if self.fit_next {
            self.look_from(StdView::Corner, volume_mm, vp);
        }
    }

    /// Starts slicing what is on the bed. Refused (and `false`) when a slice is already running, the bed is empty or the bed cannot be made into one mesh.
    pub fn start_slice(
        &mut self,
        preview: &PreviewState,
        printer: &PrinterProfile,
        filament: Option<(FilamentProfile, Nozzle)>,
    ) -> bool {
        if self.job.is_some() {
            return false;
        }
        let Ok(mesh) = self.plate.merged() else {
            return false;
        };
        self.progress = 0.0;
        self.job = Some(SliceJob::spawn(
            preview,
            mesh,
            printer.clone(),
            filament,
            self.last_slice_s,
        ));
        true
    }

    /// Slices what is on the bed and waits for the end (for a program with no window to keep alive, as the picture mode). Returns whether it worked.
    pub fn slice_blocking(
        &mut self,
        preview: &mut PreviewState,
        printer: &PrinterProfile,
        filament: Option<(FilamentProfile, Nozzle)>,
    ) -> bool {
        if !self.start_slice(preview, printer, filament) {
            return false;
        }
        loop {
            match self.poll_slice(preview) {
                SliceEvent::Running { .. } => {
                    std::thread::sleep(std::time::Duration::from_millis(2))
                }
                SliceEvent::Finished { ok, .. } => return ok,
                SliceEvent::Idle | SliceEvent::Lost => return false,
            }
        }
    }

    /// Looks at the slice in the background. When it ends, its result becomes the preview that is given.
    pub fn poll_slice(&mut self, preview: &mut PreviewState) -> SliceEvent {
        let Some(job) = self.job.as_ref() else {
            return SliceEvent::Idle;
        };
        match job.poll() {
            Poll::Running {
                fraction,
                elapsed_s,
            } => {
                self.progress = fraction;
                SliceEvent::Running {
                    fraction,
                    elapsed_s,
                }
            }
            Poll::Finished(done) => {
                self.job = None;
                self.progress = 1.0;
                let (ok, seconds) = (done.ok, done.seconds);
                if ok {
                    self.last_slice_s = seconds.max(0.05);
                    self.stale = false;
                }
                *preview = done.preview;
                SliceEvent::Finished { ok, seconds }
            }
            Poll::Lost => {
                self.job = None;
                SliceEvent::Lost
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_profiles::Catalog;

    const VOL: [f64; 3] = [250.0, 210.0, 220.0];

    fn config() -> AppConfig {
        AppConfig::default()
    }

    fn printer() -> PrinterProfile {
        Catalog::builtin()
            .unwrap()
            .find("prusa-mk4s")
            .unwrap()
            .clone()
    }

    fn wait_for(studio: &mut Studio, preview: &mut PreviewState) -> SliceEvent {
        for _ in 0..4000 {
            match studio.poll_slice(preview) {
                SliceEvent::Running { .. } => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                other => return other,
            }
        }
        panic!("the slice did not end");
    }

    #[test]
    fn adding_parts_arranges_them_when_the_settings_say_so_and_leaves_them_when_not() {
        let mut s = Studio::new();
        let mut c = config();
        for i in 0..4 {
            s.add_model(&format!("c{i}"), cube(40.0), VOL, &c).unwrap();
        }
        assert!(s.plate.overlaps().is_empty());
        assert!(s.warnings(VOL).is_empty());
        c.arrange_on_add = false;
        s.add_model("on top", cube(40.0), VOL, &c).unwrap();
        assert!(!s.plate.overlaps().is_empty());
        assert_eq!(
            s.warnings(VOL),
            vec![Warning::Overlap(s.plate.overlaps().len())]
        );
        s.arrange(VOL, &c);
        assert!(s.warnings(VOL).is_empty());
    }

    #[test]
    fn the_warnings_name_what_is_wrong_in_every_language() {
        let mut s = Studio::new();
        let c = config();
        let a = s.add_model("a", cube(40.0), VOL, &c).unwrap();
        s.plate.get_mut(a).unwrap().pos = [5.0, 5.0];
        assert_eq!(s.warnings(VOL), vec![Warning::OutOfBed(1)]);
        for l in Lang::ALL {
            let t = Warning::OutOfBed(3).text(l);
            assert!(t.contains('3') && !t.contains("{}"), "{}: {t}", l.code());
            assert!(Warning::Overlap(2).text(l).contains('2'));
            assert!(Warning::LeftOut(4).text(l).contains('4'));
        }
        // a bed too small for every part says how many were left
        let mut big = Studio::new();
        for i in 0..6 {
            big.add_model(&format!("b{i}"), cube(100.0), [220.0, 220.0, 250.0], &c);
        }
        assert!(big.left_out > 0);
        assert!(big
            .warnings([220.0, 220.0, 250.0])
            .contains(&Warning::LeftOut(big.left_out)));
    }

    #[test]
    fn the_world_meshes_are_made_again_only_when_a_part_changes() {
        let mut s = Studio::new();
        let c = config();
        let id = s.add_model("a", cube(30.0), VOL, &c).unwrap();
        assert_eq!(s.world_parts().len(), 1);
        assert!(s.take_changed());
        let first = s.world_parts()[0].1.clone();
        assert!(!s.take_changed(), "nothing moved");
        s.plate.get_mut(id).unwrap().pos = [50.0, 60.0];
        let moved = s.world_parts()[0].1.clone();
        assert_ne!(first, moved);
        assert!(s.take_changed());
        assert_eq!(s.boxes().len(), 1);
        let (_, lo, hi) = s.boxes()[0];
        assert!(((lo[0] + hi[0]) / 2.0 - 50.0).abs() < 1e-6);
    }

    #[test]
    fn what_is_on_the_bed_becomes_the_model_of_the_program() {
        let mut s = Studio::new();
        let mut state = AppState::new(config(), Lang::En).unwrap();
        s.sync(&mut state);
        assert!(state.model.is_none());
        let c = config();
        s.add_model("a.stl", cube(20.0), VOL, &c).unwrap();
        s.add_model("b.stl", cube(10.0), VOL, &c).unwrap();
        s.sync(&mut state);
        assert_eq!(state.model_name, "a.stl + b.stl");
        assert_eq!(state.model.as_ref().unwrap().triangles.len(), 24);
        assert!(s.remove_selected());
        s.sync(&mut state);
        assert_eq!(state.model.as_ref().unwrap().triangles.len(), 12);
        assert!(s.remove_selected());
        assert!(!s.remove_selected());
        s.sync(&mut state);
        assert!(state.model.is_none() && state.model_name.is_empty());
    }

    #[test]
    fn a_slice_in_the_background_ends_with_the_plan_the_estimates_and_a_fresh_bed() {
        let mut s = Studio::new();
        let mut preview = PreviewState::default();
        let c = config();
        assert!(!s.start_slice(&preview, &printer(), None), "an empty bed");
        s.add_model("c", cube(20.0), VOL, &c).unwrap();
        assert!(s.stale);
        assert!(s.start_slice(&preview, &printer(), None));
        assert!(s.is_slicing());
        assert!(!s.start_slice(&preview, &printer(), None), "one at a time");
        let ev = wait_for(&mut s, &mut preview);
        assert!(
            matches!(ev, SliceEvent::Finished { ok: true, .. }),
            "{ev:?}"
        );
        assert!(!s.is_slicing() && !s.stale);
        assert_eq!(s.progress, 1.0);
        assert_eq!(s.poll_slice(&mut preview), SliceEvent::Idle);
        let e = Estimates::of(&preview, &c).unwrap();
        assert!(e.seconds > 10.0 && e.layers > 20 && e.mass_g > 0.5 && e.cost > 0.0);
        assert!((e.height_mm - 20.0).abs() < 0.5, "{}", e.height_mm);
        // the bed changes after the slice: the plan on view is stale
        s.mark_edited();
        assert!(s.stale);
    }

    #[test]
    fn a_failed_slice_keeps_the_reason_and_the_old_plan_is_not_taken_for_fresh() {
        let mut s = Studio::new();
        let mut preview = PreviewState::default();
        let c = config();
        let id = s.add_model("big", cube(20.0), VOL, &c).unwrap();
        s.plate.get_mut(id).unwrap().set_scale_percent(5000.0);
        // 20 mm at 100x is 2000 mm: past the edge of the bed
        s.plate.get_mut(id).unwrap().set_size_mm(0, 2000.0, false);
        assert!(s.start_slice(&preview, &printer(), None));
        let ev = wait_for(&mut s, &mut preview);
        assert!(
            matches!(ev, SliceEvent::Finished { ok: false, .. }),
            "{ev:?}"
        );
        assert!(preview.error.is_some());
        assert!(s.stale, "nothing was sliced");
    }

    #[test]
    fn the_cells_of_the_bar_tell_time_filament_cost_and_layers_and_leave_out_a_cost_of_nothing() {
        let e = Estimates {
            seconds: 3900.0,
            layers: 250,
            length_m: 4.25,
            mass_g: 12.6,
            cost: 0.25,
            height_mm: 50.0,
        };
        let mut c = config();
        let cells = e.cells(&c, Lang::En);
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], ("Time", "1 h 05 min".to_string()));
        assert_eq!(cells[1].1, "4.25 m - 12.6 g");
        assert_eq!(cells[3].1, "250 - 50.0 mm");
        c.price_filament_per_kg = 0.0;
        assert_eq!(e.cells(&c, Lang::Pt).len(), 3);
        assert_eq!(e.cells(&c, Lang::Pt)[0].0, "Tempo");
        assert!(Estimates::of(&PreviewState::default(), &c).is_none());
    }

    #[test]
    fn the_camera_looks_at_the_printer_when_asked_and_stays_where_the_person_put_it() {
        let mut s = Studio::new();
        let vp = Viewport::new(900.0, 600.0).unwrap();
        s.fit_if_asked(VOL, vp);
        assert!(!s.fit_next);
        let corner = s.camera;
        s.camera.orbit(80.0, 20.0, 0.01);
        let turned = s.camera;
        s.fit_if_asked(VOL, vp);
        assert_eq!(s.camera, turned, "no fit was asked for");
        s.look_from(StdView::Top, VOL, vp);
        assert_ne!(s.camera, corner);
        assert!(s.camera.pitch > 1.5);
        s.fit_next = true;
        s.fit_if_asked(VOL, vp);
        assert_eq!(s.camera.yaw, corner.yaw);
    }

    #[test]
    fn a_copy_of_the_selected_part_is_made_and_selected_and_the_bed_arranged() {
        let mut s = Studio::new();
        let c = config();
        assert!(!s.duplicate_selected(VOL, &c), "nothing is selected");
        s.add_model("a", cube(30.0), VOL, &c).unwrap();
        assert!(s.duplicate_selected(VOL, &c));
        assert_eq!(s.plate.len(), 2);
        assert!(s.plate.overlaps().is_empty());
        assert_eq!(s.plate.selected, Some(s.plate.objects[1].id));
        assert!(s.stale);
    }

    #[test]
    fn a_blocking_slice_gives_the_plan_or_says_no() {
        let mut s = Studio::new();
        let mut preview = PreviewState::default();
        assert!(
            !s.slice_blocking(&mut preview, &printer(), None),
            "empty bed"
        );
        s.add_model("c", cube(15.0), VOL, &config()).unwrap();
        assert!(s.slice_blocking(&mut preview, &printer(), None));
        assert!(preview.layer_count() > 10 && !s.stale && !s.is_slicing());
    }

    #[test]
    fn auto_orient_stands_a_plate_down_and_does_nothing_without_a_selection() {
        let mut s = Studio::new();
        assert!(!s.auto_orient_selected());
        let c = config();
        let id = s.add_model("p", cube(1.0), VOL, &c).unwrap();
        {
            let o = s.plate.get_mut(id).unwrap();
            o.scale = [40.0, 40.0, 2.0];
            o.rot_deg = [90.0, 0.0, 0.0];
        }
        assert!((s.plate.get(id).unwrap().world_size().unwrap()[2] - 40.0).abs() < 1e-3);
        assert!(s.auto_orient_selected());
        assert!((s.plate.get(id).unwrap().world_size().unwrap()[2] - 2.0).abs() < 1e-3);
    }

    #[test]
    fn the_goblet_of_the_demonstration_gives_sensible_estimates_and_a_bed_without_warnings() {
        let mut s = Studio::new();
        let mut preview = PreviewState::default();
        let c = config();
        let id = s
            .add_model(crate::demo::DEMO_NAME, crate::demo::goblet(), VOL, &c)
            .unwrap();
        assert_eq!(s.plate.get(id).unwrap().name, "goblet.stl");
        assert!(s.warnings(VOL).is_empty());
        assert!(s.slice_blocking(&mut preview, &printer(), None));
        let e = Estimates::of(&preview, &c).unwrap();
        assert!((e.height_mm - 50.0).abs() < 0.5, "{}", e.height_mm);
        assert!(e.layers >= 200 && e.layers <= 300, "{}", e.layers);
        assert!(
            e.seconds > 600.0 && e.seconds < 6.0 * 3600.0,
            "{}",
            e.seconds
        );
        assert!(e.mass_g > 5.0 && e.mass_g < 120.0, "{}", e.mass_g);
        // the same model sliced twice gives the same plan
        let mut again = PreviewState::default();
        assert!(s.slice_blocking(&mut again, &printer(), None));
        assert_eq!(
            Estimates::of(&again, &c).unwrap().layers,
            e.layers,
            "slicing is deterministic"
        );
    }
}
