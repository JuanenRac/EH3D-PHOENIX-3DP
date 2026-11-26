//! The Preview tab without the window: slices the model that is loaded, keeps the plan, the slider of layers, the kinds of line that are shown and the camera, and gives the lines of
//! the plan already projected on the screen. The slicer and the viewer model do the work; this puts them in the order a person uses them.

use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::report;
use crate::shade::{self, BedDrawing, LineKind};
use p3dp_core::Mesh;
use p3dp_profiles::filament::{FilamentProfile, Nozzle};
use p3dp_profiles::PrinterProfile;
use p3dp_slicer::report::{progress, Progress};
use p3dp_slicer::{Kind, LayerSummary, Machine, Plan, SliceConfig};
use p3dp_viewer::layers::{lines_of, travel_lines, KindFilter, LayerRange, FLOATS_PER_VERTEX};
use p3dp_viewer::{OrbitCamera, Viewport};

/// The most lines the window is asked to draw in one frame.
pub const MAX_SEGMENTS: usize = 400_000;

/// A line on the screen: two points in pixels from the top left of the view, and a colour from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub colour: [f32; 3],
    /// How opaque it is, from 0 to 1: the walls are faint when the person looks inside, so the infill behind them shows.
    pub alpha: f32,
}

/// The lines for one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub segments: Vec<Segment>,
    /// Lines that were left out because there were more than [`MAX_SEGMENTS`].
    pub left_out: usize,
    /// The bed under the plan, with its grid, its edge and the axes (the box of the printer is left out), drawn the way the Prepare tab draws it.
    pub bed: BedDrawing,
}

/// The box that holds the plan: where the lines of its layers are, and its height, a little wider all round so the print does not touch the edge of the view. `None` for a plan with no
/// line at all.
pub fn plan_box(plan: &Plan) -> Option<([f64; 3], [f64; 3])> {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in plan
        .layers
        .iter()
        .flat_map(|l| &l.paths)
        .flat_map(|p| &p.points)
    {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if !lo.iter().chain(hi.iter()).all(|v| v.is_finite()) {
        return None;
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(plan.height_mm());
    let pad = (span * 0.08).max(3.0);
    Some((
        [lo[0] - pad, lo[1] - pad, 0.0],
        [hi[0] + pad, hi[1] + pad, plan.height_mm() + pad],
    ))
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

/// The layer a vertical slider is on when the pointer is at `fraction` of its track counted from the top (0 is the top, 1 the bottom): the top of the track is the last layer. A
/// fraction that is not a number or is outside the track is held to its ends, and a plan with no layers gives layer 0.
pub fn layer_at(fraction_from_top: f64, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let f = if fraction_from_top.is_finite() {
        fraction_from_top.clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((1.0 - f) * (count - 1) as f64).round() as usize
}

/// Where the handle of the slider is on its track, from the top, for a layer: the opposite of [`layer_at`].
pub fn fraction_of_layer(layer: usize, count: usize) -> f64 {
    if count <= 1 {
        return 0.0;
    }
    1.0 - layer.min(count - 1) as f64 / (count - 1) as f64
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
    /// See inside: the top and bottom skins are left out, so the infill and the walls under them show.
    pub peek_inside: bool,
    /// The spacing of the grid of the bed under the plan, in millimetres.
    pub grid_mm: f64,
    /// How opaque the walls are when the person looks inside (from the settings file); the rest of the lines stay solid.
    pub peek_wall_alpha: f32,
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
            peek_inside: false,
            grid_mm: 10.0,
            peek_wall_alpha: crate::config::PEEK_WALL_ALPHA_DEFAULT,
            density_g_cm3: report::DEFAULT_DENSITY_G_CM3,
            summaries: Vec::new(),
            progress: Vec::new(),
            fit_next: true,
        }
    }
}

impl PreviewState {
    /// A state with nothing sliced, that looks inside the plan or not.
    pub fn looking_inside(peek_inside: bool) -> PreviewState {
        PreviewState {
            peek_inside,
            ..PreviewState::default()
        }
    }

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
                Ok(c) => {
                    self.advice = c.advice.iter().map(|a| a.text().to_string()).collect();
                    // the flow the maker gives for the filament holds every line of the slice
                    if let Some(r) = crate::preflight::record(&f.id) {
                        crate::preflight::hold_flow(r, &mut config);
                    }
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    return false;
                }
            }
        }
        let held = p3dp_profiles::motion_limits(&printer.id)
            .map_err(|e| e.to_string())
            .and_then(|l| {
                p3dp_slicer::machine_check::apply(&mut config, &l).map_err(|e| e.to_string())
            });
        if let Err(e) = held {
            self.error = Some(e);
            return false;
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

    /// The kinds of line that are drawn: the ones that are switched on, and without the skins when the person looks inside.
    pub fn shown_filter(&self) -> KindFilter {
        KindFilter {
            solid: self.filter.solid && !self.peek_inside,
            ..self.filter
        }
    }

    /// The lines of this frame, in pixels, for a view of `width` by `height`. The camera is put to fit the print (the box of the plan, or the plate when it has no line) when a new
    /// plan is there or the view was reset.
    pub fn frame(&mut self, width: f64, height: f64) -> Frame {
        let (Some(vp), Some(plan)) = (Viewport::new(width, height), self.plan.as_ref()) else {
            return Frame::default();
        };
        let [bx, by, bz] = plan.machine.volume_mm;
        if self.fit_next {
            self.camera = OrbitCamera::default();
            match plan_box(plan) {
                Some((lo, hi)) => self.camera.fit_to_box(lo, hi, vp),
                None => self.camera.reset_for_plate(bx, by, vp),
            }
            self.fit_next = false;
        }
        let mut batches = Vec::new();
        if let Some(r) = self.range() {
            let shown = self.shown_filter();
            if self.peek_inside && shown.perimeters {
                // looking inside: the walls are drawn first and faint, and everything else over them, so the infill is seen through the walls and not only at the mouth of the part
                let walls = KindFilter {
                    perimeters: true,
                    infill: false,
                    solid: false,
                    bridges: false,
                    supports: false,
                    adhesion: false,
                };
                batches.push((lines_of(plan, &r, &walls), self.peek_wall_alpha));
                let rest = KindFilter {
                    perimeters: false,
                    ..shown
                };
                batches.push((lines_of(plan, &r, &rest), 1.0));
            } else {
                batches.push((lines_of(plan, &r, &shown), 1.0));
            }
            if self.show_travel {
                batches.push((travel_lines(plan, &r), 1.0));
            }
        }
        let mut frame = Frame {
            bed: shade::bed([bx, by, bz], self.grid_mm, &self.camera, vp),
            ..Frame::default()
        };
        frame.bed.lines.retain(|l| l.kind != LineKind::Volume);
        for (batch, alpha) in &batches {
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
                        alpha: *alpha,
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
        assert_eq!(lines.len(), 7, "{lines:?}");
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
        for (now, then) in s.camera.target.iter().zip(before.iter()) {
            assert!((now - then).abs() < 1e-6);
        }
    }

    #[test]
    fn the_view_is_fitted_to_the_print_and_not_to_the_whole_plate() {
        let (mut s, _) = sliced();
        let plan = s.plan.as_ref().unwrap();
        let (lo, hi) = plan_box(plan).unwrap();
        // the box holds the print (a cube of 20 mm) with a margin, stands on the bed and is as high as the plan
        assert!(
            hi[0] - lo[0] > 20.0 && hi[0] - lo[0] < 40.0,
            "{lo:?} {hi:?}"
        );
        assert_eq!(lo[2], 0.0);
        assert!(hi[2] > plan.height_mm());
        // a plan that holds a cube of 20 mm on a bed of 250 fills the view: it is far bigger than it would be with the plate fitted
        let f = s.frame(800.0, 600.0);
        let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
        for g in &f.segments {
            for p in [g.a, g.b] {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
        }
        assert!(
            (max[0] - min[0]) > 200.0 || (max[1] - min[1]) > 150.0,
            "{min:?} {max:?}"
        );
        // a plan with no line falls back to the plate
        let mut empty = s.plan.clone().unwrap();
        for l in &mut empty.layers {
            l.paths.clear();
        }
        assert!(plan_box(&empty).is_none());
    }

    #[test]
    fn the_bed_under_the_plan_has_its_grid_its_edge_and_its_axes_and_no_box() {
        let (mut s, _) = sliced();
        s.reset_view();
        let f = s.frame(800.0, 600.0);
        let count = |k: LineKind| f.bed.lines.iter().filter(|l| l.kind == k).count();
        // the camera is near the print, so an edge of the plate may be cut by the plane in front of it, but the near ones are there
        assert!(count(LineKind::Edge) >= 2);
        assert!(count(LineKind::Grid) + count(LineKind::GridMajor) > 20);
        assert_eq!(count(LineKind::Volume), 0);
        // the axes are at the corner of the origin, which a near view may leave out of sight, but they are never drawn twice
        assert!(
            count(LineKind::AxisX) <= 1
                && count(LineKind::AxisY) <= 1
                && count(LineKind::AxisZ) <= 1
        );
        assert!(f.bed.quad.is_some());
        // the spacing of the grid is the setting
        s.grid_mm = 50.0;
        let wide = s.frame(800.0, 600.0);
        let grid = |f: &Frame| {
            f.bed
                .lines
                .iter()
                .filter(|l| matches!(l.kind, LineKind::Grid | LineKind::GridMajor))
                .count()
        };
        assert!(grid(&wide) < grid(&f));
        // nothing is drawn with no plan
        s.forget();
        assert_eq!(s.frame(800.0, 600.0), Frame::default());
    }

    #[test]
    fn looking_inside_leaves_out_the_skins_and_keeps_the_rest() {
        let (mut s, _) = sliced();
        let all = s.frame(800.0, 600.0).segments.len();
        assert!(s.shown_filter().solid);
        s.peek_inside = true;
        let f = s.shown_filter();
        assert!(!f.solid && f.perimeters && f.infill);
        let inside = s.frame(800.0, 600.0);
        assert!(
            inside.segments.len() < all,
            "{} {all}",
            inside.segments.len()
        );
        let solid = p3dp_viewer::layers::colour_of(Kind::SolidInfill);
        assert!(inside.segments.iter().all(|g| g.colour != solid));
        // a kind that is off stays off whatever the person looks at
        s.filter.solid = false;
        s.peek_inside = false;
        assert!(!s.shown_filter().solid);
    }

    #[test]
    fn the_vertical_slider_maps_the_track_to_the_layers_and_back() {
        assert_eq!(
            layer_at(0.0, 100),
            99,
            "the top of the track is the last layer"
        );
        assert_eq!(layer_at(1.0, 100), 0);
        assert_eq!(layer_at(0.5, 101), 50);
        assert_eq!(layer_at(-3.0, 10), 9);
        assert_eq!(layer_at(7.0, 10), 0);
        assert_eq!(layer_at(f64::NAN, 10), 9);
        assert_eq!(layer_at(0.3, 0), 0);
        assert_eq!(layer_at(0.3, 1), 0);
        assert_eq!(fraction_of_layer(0, 1), 0.0);
        assert_eq!(fraction_of_layer(99, 100), 0.0);
        assert_eq!(fraction_of_layer(0, 100), 1.0);
        assert_eq!(fraction_of_layer(500, 100), 0.0, "past the end is the end");
        for count in [2usize, 3, 17, 250, 1000] {
            for layer in 0..count {
                assert_eq!(
                    layer_at(fraction_of_layer(layer, count), count),
                    layer,
                    "{layer}/{count}"
                );
            }
        }
    }

    #[test]
    fn looking_inside_draws_the_walls_faint_so_the_infill_shows_through_and_looking_from_outside_is_solid(
    ) {
        let (mut s, _) = sliced();
        let outside = s.frame(800.0, 600.0);
        assert!(!outside.segments.is_empty());
        assert!(outside.segments.iter().all(|g| g.alpha == 1.0));
        s.peek_inside = true;
        s.peek_wall_alpha = 0.3;
        let inside = s.frame(800.0, 600.0);
        let wall = p3dp_viewer::layers::colour_of(Kind::ExternalPerimeter);
        let infill = p3dp_viewer::layers::colour_of(Kind::Infill);
        let faint: Vec<&Segment> = inside.segments.iter().filter(|g| g.alpha < 1.0).collect();
        assert!(!faint.is_empty());
        assert!(faint.iter().all(|g| g.alpha == 0.3));
        let wall_colours = [wall, p3dp_viewer::layers::colour_of(Kind::Perimeter)];
        assert!(faint.iter().all(|g| wall_colours.contains(&g.colour)));
        // the infill is there and solid, and it comes after the walls so it is drawn over them
        let first_solid = inside.segments.iter().position(|g| g.alpha == 1.0).unwrap();
        let last_faint = inside.segments.iter().rposition(|g| g.alpha < 1.0).unwrap();
        assert!(last_faint < first_solid);
        assert!(inside
            .segments
            .iter()
            .any(|g| g.colour == infill && g.alpha == 1.0));
        // with the walls switched off there is nothing faint
        s.filter.perimeters = false;
        assert!(s
            .frame(800.0, 600.0)
            .segments
            .iter()
            .all(|g| g.alpha == 1.0));
    }
}
