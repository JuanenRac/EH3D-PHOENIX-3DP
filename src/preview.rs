//! The Preview tab without the window: slices the model that is loaded, keeps the plan, the slider of layers, the kinds of line that are shown and the camera, and gives the lines of
//! the plan already projected on the screen. The slicer and the viewer model do the work; this puts them in the order a person uses them.

use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::report;
use p3dp_core::Mesh;
use p3dp_profiles::filament::{FilamentProfile, Nozzle};
use p3dp_profiles::PrinterProfile;
use p3dp_slicer::report::{progress, Progress};
use p3dp_slicer::{Kind, LayerSummary, Machine, Plan, SliceConfig};
use p3dp_viewer::layers::{
    lines_of, plate_lines, travel_lines, KindFilter, LayerRange, FLOATS_PER_VERTEX,
};
use p3dp_viewer::{OrbitCamera, Viewport};

/// The most lines the window is asked to draw in one frame.
pub const MAX_SEGMENTS: usize = 400_000;

/// A line on the screen: two points in pixels from the top left of the view, and a colour from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub colour: [f32; 3],
}

/// The lines for one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub segments: Vec<Segment>,
    /// Lines that were left out because there were more than [`MAX_SEGMENTS`].
    pub left_out: usize,
}

/// The time in words that need no language: `1 h 05 min`, `42 min` or `30 s`.
pub fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-".to_string();
    }
    let total = seconds.round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else if m > 0 {
        format!("{m} min")
    } else {
        format!("{s} s")
    }
}

/// The state of the tab.
#[derive(Debug, Clone)]
pub struct PreviewState {
    /// The settings of the slice, which a person changes in the tab.
    pub config: SliceConfig,
    pub plan: Option<Plan>,
    pub filter: KindFilter,
    pub camera: OrbitCamera,
    /// The slider: the layer it is on, counted from 0.
    pub layer: usize,
    /// Show only the layer the slider is on, and not all the layers up to it.
    pub only_one: bool,
    /// Why the last slice did not happen.
    pub error: Option<String>,
    /// Advice from the check of the filament against the printer, that did not stop the slice.
    pub advice: Vec<String>,
    /// Draw the moves of the head between the paths too (pale blue lines).
    pub show_travel: bool,
    /// The density of the plastic of the last slice, for the weight in the report.
    pub density_g_cm3: f64,
    /// What every layer of the plan comes to, worked out once when the plan is made.
    summaries: Vec<LayerSummary>,
    /// Where the print stands after each layer, worked out with the summaries.
    progress: Vec<Progress>,
    /// The camera is put to fit the plate on the next frame.
    fit_next: bool,
}

impl Default for PreviewState {
    fn default() -> PreviewState {
        PreviewState {
            config: SliceConfig::default(),
            plan: None,
            filter: KindFilter::default(),
            camera: OrbitCamera::default(),
            layer: 0,
            only_one: false,
            error: None,
            advice: Vec::new(),
            show_travel: false,
            density_g_cm3: report::DEFAULT_DENSITY_G_CM3,
            summaries: Vec::new(),
            progress: Vec::new(),
            fit_next: true,
        }
    }
}

impl PreviewState {
    /// Slices a model for a printer, with a filament when one is chosen. A plan that was there stays when the new slice fails, and the reason is kept. Returns whether it worked.
    pub fn slice(
        &mut self,
        mesh: &Mesh,
        printer: &PrinterProfile,
        filament: Option<(&FilamentProfile, Nozzle)>,
    ) -> bool {
        self.error = None;
        self.advice.clear();
        let machine = match Machine::from_profile(printer) {
            Ok(m) => m,
            Err(e) => {
                self.error = Some(e.to_string());
                return false;
            }
        };
        let mut config = self.config.clone();
        let mut density = report::DEFAULT_DENSITY_G_CM3;
        if let Some((f, nozzle)) = filament {
            density = f.density_g_cm3;
            match config.apply_filament(f, printer, nozzle) {
                Ok(c) => self.advice = c.advice.iter().map(|a| a.text().to_string()).collect(),
                Err(e) => {
                    self.error = Some(e.to_string());
                    return false;
                }
            }
        }
        match p3dp_slicer::plan(mesh, &machine, &config) {
            Ok(plan) => {
                let top = plan.layer_count().saturating_sub(1);
                self.summaries = plan.summaries();
                self.progress = progress(&self.summaries, plan.config.filament_area_mm2());
                self.density_g_cm3 = density;
                self.plan = Some(plan);
                self.layer = top;
                self.fit_next = true;
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// Forgets the plan (a new model was loaded).
    pub fn forget(&mut self) {
        self.plan = None;
        self.summaries.clear();
        self.progress.clear();
        self.error = None;
        self.advice.clear();
        self.layer = 0;
    }

    pub fn layer_count(&self) -> usize {
        self.plan.as_ref().map_or(0, Plan::layer_count)
    }

    /// Moves the slider; it stays between the first and the last layer.
    pub fn set_layer(&mut self, layer: usize) {
        self.layer = layer.min(self.layer_count().saturating_sub(1));
    }

    /// Moves the slider by a number of layers (up when positive).
    pub fn step_layer(&mut self, steps: i64) {
        let top = self.layer_count().saturating_sub(1) as i64;
        self.layer = (self.layer as i64 + steps).clamp(0, top) as usize;
    }

    /// The range of layers the slider and the switch ask for: all the layers up to the slider, or only the one it is on.
    pub fn range(&self) -> Option<LayerRange> {
        let mut r = LayerRange::new(self.layer_count())?;
        if self.only_one {
            r.only(self.layer);
        } else {
            r.set_last(self.layer);
        }
        Some(r)
    }

    /// The words for the slider: layers `first` to `last` of `count`.
    pub fn layer_label(&self, lang: Lang) -> String {
        match self.range() {
            Some(r) => fill(
                Msg::ViewLayerRange.tr(lang),
                &[
                    &(r.first() + 1).to_string(),
                    &(r.last() + 1).to_string(),
                    &r.count().to_string(),
                ],
            ),
            None => Msg::NoModel.tr(lang).to_string(),
        }
    }

    /// The words for the result of the slice: the layers and the estimate of the time.
    pub fn summary(&self, lang: Lang) -> Option<String> {
        let p = self.plan.as_ref()?;
        Some(fill(
            Msg::SliceSummary.tr(lang),
            &[
                &p.layer_count().to_string(),
                &format_duration(p.estimated_seconds()),
            ],
        ))
    }

    /// The lines of the report of the plan (material, cost, time, short and slowest layers), in a language; empty when there is no plan.
    pub fn report_lines(&self, config: &AppConfig, lang: Lang) -> Vec<String> {
        let Some(plan) = self.plan.as_ref() else {
            return Vec::new();
        };
        report::lines_from(plan, &self.summaries, self.density_g_cm3, config, lang)
            .unwrap_or_else(|e| vec![fill(Msg::Problem.tr(lang), &[&e])])
    }

    /// Where the print stands after the layer the slider is on: the time gone, the time left and the share done.
    pub fn progress_label(&self, lang: Lang) -> Option<String> {
        let p = self.progress.get(self.layer)?;
        Some(fill(
            Msg::ProgressAtLayer.tr(lang),
            &[
                &format_duration(p.elapsed_s),
                &format_duration(p.remaining_s),
                &format!("{:.0}", p.percent),
            ],
        ))
    }

    /// The length of line of each kind in the layers that are shown, in millimetres, for the key of the colours.
    pub fn lengths(&self) -> Vec<(Kind, f64)> {
        let (Some(p), Some(r)) = (self.plan.as_ref(), self.range()) else {
            return Vec::new();
        };
        let mut out: Vec<(Kind, f64)> = Vec::new();
        for path in p
            .layers
            .iter()
            .filter(|l| r.shows(l.index))
            .flat_map(|l| &l.paths)
        {
            match out.iter_mut().find(|(k, _)| *k == path.kind) {
                Some((_, mm)) => *mm += path.length_mm(),
                None => out.push((path.kind, path.length_mm())),
            }
        }
        out
    }

    /// Turns the camera by the pixels of a drag.
    pub fn orbit(&mut self, dx_px: f64, dy_px: f64) {
        self.camera.orbit(dx_px, dy_px, 0.01);
    }

    /// Slides the view across by the pixels of a drag, in a view of `width` by `height`.
    pub fn pan(&mut self, dx_px: f64, dy_px: f64, width: f64, height: f64) {
        if let Some(vp) = Viewport::new(width, height) {
            self.camera.pan(dx_px, dy_px, vp);
        }
    }

    /// Moves the camera nearer or farther (a wheel of 1 notch is a factor of about 0.9 or 1.1).
    pub fn zoom(&mut self, factor: f64) {
        self.camera.zoom(factor);
    }

    /// Puts the camera back to look at the plate.
    pub fn reset_view(&mut self) {
        self.fit_next = true;
    }

    /// The lines of this frame, in pixels, for a view of `width` by `height`. The camera is put to fit the plate when a new plan is there or the view was reset.
    pub fn frame(&mut self, width: f64, height: f64) -> Frame {
        let (Some(vp), Some(plan)) = (Viewport::new(width, height), self.plan.as_ref()) else {
            return Frame::default();
        };
        let [bx, by, _] = plan.machine.volume_mm;
        if self.fit_next {
            self.camera.reset_for_plate(bx, by, vp);
            self.fit_next = false;
        }
        let mut batches = vec![plate_lines(bx, by)];
        if let Some(r) = self.range() {
            batches.push(lines_of(plan, &r, &self.filter));
            if self.show_travel {
                batches.push(travel_lines(plan, &r));
            }
        }
        let mut frame = Frame::default();
        for batch in &batches {
            for v in batch.vertices.as_chunks::<{ FLOATS_PER_VERTEX * 2 }>().0 {
                if frame.segments.len() >= MAX_SEGMENTS {
                    frame.left_out += 1;
                    continue;
                }
                let at = |i: usize| {
                    let o = i * FLOATS_PER_VERTEX;
                    self.camera.project(
                        [f64::from(v[o]), f64::from(v[o + 1]), f64::from(v[o + 2])],
                        vp,
                    )
                };
                if let (Some(a), Some(b)) = (at(0), at(1)) {
                    frame.segments.push(Segment {
                        a: [a[0] as f32, a[1] as f32],
                        b: [b[0] as f32, b[1] as f32],
                        colour: [v[3], v[4], v[5]],
                    });
                }
            }
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_profiles::filament::FilamentCatalog;
    use p3dp_profiles::Catalog;

    fn catalog() -> Catalog {
        Catalog::builtin().unwrap()
    }

    fn sliced() -> (PreviewState, Catalog) {
        let c = catalog();
        let mut s = PreviewState::default();
        let p = c.find("prusa-mk4s").unwrap();
        assert!(s.slice(&cube(20.0), p, None), "{:?}", s.error);
        (s, c)
    }

    #[test]
    fn durations_are_told_in_hours_minutes_and_seconds() {
        for (s, want) in [
            (0.0, "0 s"),
            (29.6, "30 s"),
            (60.0, "1 min"),
            (3599.0, "59 min"),
            (3600.0, "1 h 00 min"),
            (3725.0, "1 h 02 min"),
            (86_400.0, "24 h 00 min"),
            (f64::NAN, "-"),
            (-5.0, "-"),
        ] {
            assert_eq!(format_duration(s), want, "{s}");
        }
    }

    #[test]
    fn slicing_gives_a_plan_the_slider_on_the_top_and_the_summary_in_every_language() {
        let (s, _) = sliced();
        let n = s.layer_count();
        assert_eq!(n, 100);
        assert_eq!(s.layer, n - 1);
        assert_eq!(s.layer_label(Lang::En), "Layers 1 to 100 of 100");
        let r = s.range().unwrap();
        assert_eq!((r.first(), r.last()), (0, 99));
        for l in Lang::ALL {
            let t = s.summary(l).unwrap();
            assert!(t.contains("100") && !t.contains("{}"), "{}: {t}", l.code());
        }
        assert!(s.summary(Lang::Pt).unwrap().contains("camadas"));
    }

    #[test]
    fn the_slider_shows_the_layers_up_to_it_or_only_the_one_it_is_on() {
        let (mut s, _) = sliced();
        s.set_layer(9);
        assert_eq!(s.layer_label(Lang::En), "Layers 1 to 10 of 100");
        s.only_one = true;
        assert_eq!(s.layer_label(Lang::En), "Layers 10 to 10 of 100");
        let r = s.range().unwrap();
        assert!(r.shows(9) && !r.shows(8) && !r.shows(10));
        s.step_layer(5);
        assert_eq!(s.layer, 14);
        s.step_layer(-1000);
        assert_eq!(s.layer, 0);
        s.step_layer(1000);
        assert_eq!(s.layer, 99);
        s.set_layer(5000);
        assert_eq!(s.layer, 99);
        // the lines follow the slider
        let few = {
            s.only_one = false;
            s.set_layer(2);
            s.frame(800.0, 600.0).segments.len()
        };
        s.set_layer(50);
        let more = s.frame(800.0, 600.0).segments.len();
        assert!(more > few, "{more} {few}");
    }

    #[test]
    fn the_travel_moves_are_drawn_only_when_asked_and_in_their_own_colour() {
        let (mut s, _) = sliced();
        let without = s.frame(800.0, 600.0);
        assert!(!s.show_travel);
        s.show_travel = true;
        let with = s.frame(800.0, 600.0);
        assert!(with.segments.len() > without.segments.len());
        let pale = with
            .segments
            .iter()
            .filter(|x| x.colour == p3dp_viewer::layers::TRAVEL_COLOUR)
            .count();
        assert_eq!(pale, with.segments.len() - without.segments.len());
        assert!(without
            .segments
            .iter()
            .all(|x| x.colour != p3dp_viewer::layers::TRAVEL_COLOUR));
    }

    #[test]
    fn the_report_and_the_progress_follow_the_plan_the_filament_and_the_slider() {
        let (mut s, _) = sliced();
        let cfg = AppConfig::default();
        let lines = s.report_lines(&cfg, Lang::En);
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert!(lines[0].starts_with("Material: "));
        // at the top of the slider the print is all done, at the bottom it has just begun
        assert!(
            s.progress_label(Lang::En).unwrap().contains("(100 %)"),
            "{:?}",
            s.progress_label(Lang::En)
        );
        s.set_layer(0);
        let first = s.progress_label(Lang::En).unwrap();
        assert!(
            first.starts_with("After this layer: ") && !first.contains("(100 %)"),
            "{first}"
        );
        assert!(s
            .progress_label(Lang::Pt)
            .unwrap()
            .starts_with("Depois desta camada"));
        // the plastic of the filament chosen sets the density that weighs the report
        let c = catalog();
        let petg = FilamentCatalog::builtin()
            .unwrap()
            .find("generic-petg")
            .unwrap()
            .clone();
        let p = c.find("prusa-mk4s").unwrap();
        let nozzle = Nozzle {
            diameter_mm: 0.4,
            hardened: false,
        };
        assert!(
            s.slice(&cube(20.0), p, Some((&petg, nozzle))),
            "{:?}",
            s.error
        );
        assert_eq!(s.density_g_cm3, petg.density_g_cm3);
        // forgetting the plan forgets the report too
        s.forget();
        assert!(s.report_lines(&cfg, Lang::En).is_empty() && s.progress_label(Lang::En).is_none());
    }

    #[test]
    fn the_kinds_can_be_switched_off_and_the_lengths_follow_the_layers_shown() {
        let (mut s, _) = sliced();
        let all = s.frame(800.0, 600.0).segments.len();
        s.filter.infill = false;
        s.filter.solid = false;
        let walls = s.frame(800.0, 600.0).segments.len();
        assert!(walls < all && walls > 4, "{walls} {all}");
        let l = s.lengths();
        assert!(l
            .iter()
            .any(|(k, mm)| *k == Kind::ExternalPerimeter && *mm > 0.0));
        s.set_layer(0);
        let first = s.lengths();
        let perimeter = |v: &[(Kind, f64)]| {
            v.iter()
                .find(|(k, _)| *k == Kind::ExternalPerimeter)
                .unwrap()
                .1
        };
        assert!(perimeter(&first) < perimeter(&l));
    }

    #[test]
    fn the_camera_turns_zooms_and_comes_back_and_the_lines_follow_it() {
        let (mut s, _) = sliced();
        let a = s.frame(800.0, 600.0);
        assert!(!a.segments.is_empty() && a.left_out == 0);
        // every point is on the screen after the fit
        let inside = |f: &Frame| {
            f.segments.iter().all(|g| {
                [g.a, g.b]
                    .iter()
                    .all(|p| (0.0..=800.0).contains(&p[0]) && (0.0..=600.0).contains(&p[1]))
            })
        };
        assert!(inside(&a));
        s.orbit(120.0, 40.0);
        let b = s.frame(800.0, 600.0);
        assert_ne!(a.segments, b.segments);
        s.zoom(0.5);
        let c = s.frame(800.0, 600.0);
        assert_ne!(b.segments, c.segments);
        s.reset_view();
        let d = s.frame(800.0, 600.0);
        assert_eq!(a.segments, d.segments);
        // a view with no size draws nothing, and a number that is not a number changes nothing
        assert!(s.frame(0.0, 600.0).segments.is_empty());
        s.orbit(f64::NAN, 1.0);
        s.zoom(f64::NAN);
        assert_eq!(s.frame(800.0, 600.0).segments, d.segments);
    }

    #[test]
    fn a_slice_that_fails_keeps_the_plan_it_had_and_says_why() {
        let (mut s, c) = sliced();
        let before = s.plan.clone();
        // a resin printer takes no G-code
        let resin = c.find("elegoo-mars-4").unwrap();
        assert!(!s.slice(&cube(20.0), resin, None));
        assert!(s.error.as_ref().unwrap().contains("not a filament printer"));
        assert_eq!(s.plan, before);
        // a model that does not fit
        let small = c.find("bambu-lab-a1-mini").unwrap();
        assert!(!s.slice(&cube(300.0), small, None));
        assert!(s.error.as_ref().unwrap().contains("does not fit"));
        // and a good one clears the reason
        assert!(s.slice(&cube(10.0), small, None));
        assert!(s.error.is_none());
        s.forget();
        assert!(s.plan.is_none() && s.layer_count() == 0 && s.range().is_none());
        assert!(s.frame(800.0, 600.0).segments.is_empty());
        assert_eq!(s.layer_label(Lang::En), "No model loaded");
        assert!(s.summary(Lang::En).is_none() && s.lengths().is_empty());
    }

    #[test]
    fn a_filament_sets_the_temperatures_and_a_pair_that_does_not_go_is_refused() {
        let c = catalog();
        let fil = FilamentCatalog::builtin().unwrap();
        let nozzle = Nozzle {
            diameter_mm: 0.4,
            hardened: false,
        };
        let a1 = c.find("bambu-lab-a1").unwrap();
        let petg = fil.profiles.iter().find(|f| f.id.contains("petg")).unwrap();
        let mut s = PreviewState::default();
        assert!(
            s.slice(&cube(15.0), a1, Some((petg, nozzle))),
            "{:?}",
            s.error
        );
        let plan = s.plan.as_ref().unwrap();
        assert!(plan.config.nozzle_c <= a1.limits.unwrap().nozzle_c);
        assert!((plan.config.nozzle_c - petg.nozzle_c.best).abs() < 1e-9);
        // nylon in a printer that cannot get hot enough, or a nozzle too small for it
        let pa = fil
            .profiles
            .iter()
            .find(|f| f.material == p3dp_profiles::Material::Pa)
            .unwrap();
        let tiny = Nozzle {
            diameter_mm: 0.2,
            hardened: false,
        };
        let mut t = PreviewState::default();
        let ok = t.slice(&cube(15.0), a1, Some((pa, tiny)));
        if !ok {
            assert!(t.error.is_some() && t.plan.is_none());
        }
    }

    #[test]
    fn sliding_the_view_moves_the_target_and_nonsense_does_nothing() {
        let mut s = PreviewState::default();
        let before = s.camera.target;
        s.pan(40.0, -20.0, 800.0, 600.0);
        assert_ne!(s.camera.target, before);
        let moved = s.camera;
        s.pan(f64::NAN, 1.0, 800.0, 600.0);
        s.pan(10.0, 10.0, 0.0, 600.0);
        assert_eq!(s.camera, moved);
        s.pan(-40.0, 20.0, 800.0, 600.0);
        for k in 0..3 {
            assert!((s.camera.target[k] - before[k]).abs() < 1e-6);
        }
    }
}
