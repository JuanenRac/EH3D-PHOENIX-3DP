//! The window (built with the `gui` feature), laid out like a slicer: a bar on top with the tabs Prepare, Preview, Device and Project, a panel at the left (printer, filament and
//! process), a 3D view in the middle (the bed with its grid, the parts as shaded solids, the standard views), a panel at the right (the parts and their numbers), and a bar at the bottom
//! with the slice, its progress and the estimates. Everything that is decided is in the modules without a window and tested there: the parts and the bed are `objects` and `studio`, the
//! solid look is `shade`, the plan and the layers are `preview`, the machines of the server and the queue are `device`, the settings of the server and the login are `server_ui`
//! (the panel and the badge of `eh3d-view`), the limits of a printer are `limits_ui`, the shortcuts are `keys`, the colours and the layout are `look` and the picture mode is `shot`.
//! This file only draws and passes on what the person does. It has not been built or looked at on a screen yet: the part that can be checked without one is checked in those modules.

use crate::app_state::{demo_device, AppState, Status, Tab};
use crate::config::AppConfig;
use crate::demo;
use crate::device::{self, DevicePanel, Done};
use crate::i18n::{fill, state_text, Lang, Msg};
use crate::keys::{Action, Chord, Scheme, SchemeId};
use crate::limits_ui::{self, LimitsDraft};
use crate::lists::{FilamentFilter, PrinterFilter};
use crate::look::{self, Palette, Theme};
use crate::preview::{self, PreviewState};
use crate::server_ui::{self, ServerUi};
use crate::shade::{self, LineKind, ShadedTriangle, StdView};
use crate::shot::{self, ShotRequest, WindowArgs};
use crate::studio::{Estimates, SliceEvent, Studio};
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Pos2, Stroke};
use eh3d_view::server_gui;
use p3dp_core::Mesh;
use p3dp_link::PrinterState;
use p3dp_profiles::filament::{FilamentCatalog, FilamentProfile, Material, Nozzle};
use p3dp_profiles::Technology;
use p3dp_slicer::{Adhesion, InfillPattern, Kind, Seam, Supports};
use p3dp_viewer::Viewport;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn col(c: look::Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn col_f(c: [f32; 3]) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// The key of egui for the name of a key in a chord.
fn egui_key(name: &str) -> Option<egui::Key> {
    use egui::Key as K;
    Some(match name {
        "A" => K::A,
        "B" => K::B,
        "C" => K::C,
        "D" => K::D,
        "E" => K::E,
        "F" => K::F,
        "G" => K::G,
        "H" => K::H,
        "I" => K::I,
        "J" => K::J,
        "K" => K::K,
        "L" => K::L,
        "M" => K::M,
        "N" => K::N,
        "O" => K::O,
        "P" => K::P,
        "Q" => K::Q,
        "R" => K::R,
        "S" => K::S,
        "T" => K::T,
        "U" => K::U,
        "V" => K::V,
        "W" => K::W,
        "X" => K::X,
        "Y" => K::Y,
        "Z" => K::Z,
        "0" => K::Num0,
        "1" => K::Num1,
        "2" => K::Num2,
        "3" => K::Num3,
        "4" => K::Num4,
        "5" => K::Num5,
        "6" => K::Num6,
        "7" => K::Num7,
        "8" => K::Num8,
        "9" => K::Num9,
        "F1" => K::F1,
        "F2" => K::F2,
        "F3" => K::F3,
        "F4" => K::F4,
        "F5" => K::F5,
        "F6" => K::F6,
        "F7" => K::F7,
        "F8" => K::F8,
        "F9" => K::F9,
        "F10" => K::F10,
        "F11" => K::F11,
        "F12" => K::F12,
        "Up" => K::ArrowUp,
        "Down" => K::ArrowDown,
        "Left" => K::ArrowLeft,
        "Right" => K::ArrowRight,
        "PageUp" => K::PageUp,
        "PageDown" => K::PageDown,
        "Home" => K::Home,
        "End" => K::End,
        "Tab" => K::Tab,
        "Escape" => K::Escape,
        "Enter" => K::Enter,
        "Space" => K::Space,
        "Delete" => K::Delete,
        _ => return None,
    })
}

/// Whether the chord was pressed this frame; the key is taken, so no widget sees it too. A chord with no Ctrl and no Alt is left alone while a text box has the keyboard.
fn pressed(ctx: &egui::Context, chord: &Chord) -> bool {
    let Some(key) = egui_key(&chord.key) else {
        return false;
    };
    if !chord.ctrl && !chord.alt && ctx.egui_wants_keyboard_input() {
        return false;
    }
    let mods = egui::Modifiers {
        alt: chord.alt,
        ctrl: false,
        shift: chord.shift,
        mac_cmd: false,
        command: chord.ctrl,
    };
    ctx.input_mut(|i| i.consume_key(mods, key))
}

/// The name of a standard view in the language.
fn view_msg(v: StdView) -> Msg {
    match v {
        StdView::Corner => Msg::ViewCorner,
        StdView::Top => Msg::ViewTop,
        StdView::Bottom => Msg::ViewBottom,
        StdView::Front => Msg::ViewFront,
        StdView::Back => Msg::ViewBack,
        StdView::Left => Msg::ViewLeft,
        StdView::Right => Msg::ViewRight,
    }
}

/// The stroke of a line of the bed.
fn bed_stroke(kind: LineKind, pal: &Palette) -> Stroke {
    match kind {
        LineKind::Grid => Stroke::new(1.0, col(pal.grid)),
        LineKind::GridMajor => Stroke::new(1.0, col(look::scaled(pal.grid, 1.4))),
        LineKind::Edge => Stroke::new(2.0, col(pal.line)),
        LineKind::Volume => Stroke::new(1.0, alpha(col(pal.dim), 90)),
        LineKind::AxisX => Stroke::new(2.0, Color32::from_rgb(230, 70, 70)),
        LineKind::AxisY => Stroke::new(2.0, Color32::from_rgb(80, 200, 90)),
        LineKind::AxisZ => Stroke::new(2.0, Color32::from_rgb(90, 140, 255)),
    }
}

/// Paints the shaded polygons as meshes (cut in pieces, so none is too large for the graphics card), each one in the colour of its part lit by its light.
fn paint_triangles(
    painter: &egui::Painter,
    origin: Pos2,
    tris: &[ShadedTriangle],
    colour_of: &dyn Fn(u32) -> look::Rgb,
) {
    let mut mesh = egui::epaint::Mesh::default();
    for t in tris {
        let base = colour_of(t.object);
        let first = mesh.vertices.len() as u32;
        // each corner has its own light, so a curved part is filled with a blend and not with flat facets
        for (p, light) in t.p.iter().zip(t.corners) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(origin.x + p[0], origin.y + p[1]),
                uv: egui::epaint::WHITE_UV,
                color: col(look::scaled(base, light)),
            });
        }
        mesh.add_triangle(first, first + 1, first + 2);
        if mesh.vertices.len() > 60_000 {
            painter.add(egui::Shape::mesh(std::mem::take(&mut mesh)));
        }
    }
    if !mesh.vertices.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// The picture being taken.
struct ShotState {
    request: ShotRequest,
    frame: u32,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}

struct App {
    state: AppState,
    look: Palette,
    config_path: Option<PathBuf>,
    filaments: FilamentCatalog,
    printer_filter: PrinterFilter,
    filament_filter: FilamentFilter,
    filament_id: Option<String>,
    nozzle_mm: f64,
    preview: PreviewState,
    studio: Studio,
    device: DevicePanel,
    device_read: bool,
    /// When the machines of the server were last read, for the line that says how old the figures are.
    device_read_at: Option<std::time::Instant>,
    server: ServerUi,
    scheme: Scheme,
    show_shortcuts: bool,
    show_help: bool,
    path_text: String,
    save_text: String,
    project_text: String,
    /// Keep the proportions when one size of a part is typed.
    keep_ratio: bool,
    /// The limits being typed for the printer with this id.
    limits: Option<(String, LimitsDraft)>,
    /// What the last edit of the limits has to say.
    limits_note: Vec<String>,
    /// A standard view asked for, to be applied when the size of the view is known.
    pending_view: Option<StdView>,
    /// The last thing the slice said.
    note: Option<String>,
    shot: Option<ShotState>,
    visuals_theme: Option<Theme>,
}

impl App {
    fn new(mut state: AppState, args: &WindowArgs, shot: Option<ShotState>) -> App {
        let scheme = state
            .config
            .shortcuts()
            .unwrap_or_else(|_| Scheme::of(SchemeId::Family));
        // a picture reaches for no network: the server is the one made in memory, or none
        let server = if shot.is_some() {
            if args.demo {
                demo_server()
            } else {
                ServerUi::new(&p3dp_server_bridge::Settings::default())
            }
        } else {
            ServerUi::new(&state.config.server)
        };
        let volume = state
            .printer()
            .map_or([220.0, 220.0, 250.0], |p| p.volume_mm);
        let mut studio = Studio::new();
        if let Some(mesh) = state.model.take() {
            let name = state.model_name.clone();
            studio.add_model(&name, mesh, volume, &state.config);
        } else if args.demo && shot.is_some() {
            studio.add_model(demo::DEMO_NAME, demo::goblet(), volume, &state.config);
        }
        studio.sync(&mut state);
        App {
            look: state.config.theme.palette(),
            config_path: args.config.clone(),
            filaments: FilamentCatalog::builtin().unwrap_or(FilamentCatalog {
                profiles: Vec::new(),
            }),
            printer_filter: PrinterFilter::default(),
            filament_filter: FilamentFilter::default(),
            filament_id: None,
            nozzle_mm: 0.4,
            preview: PreviewState::looking_inside(args.peek),
            studio,
            device: DevicePanel::with_policy(state.config.queue_policy()),
            device_read: false,
            device_read_at: None,
            server,
            scheme,
            show_shortcuts: false,
            show_help: false,
            path_text: String::new(),
            save_text: "project.3mf".to_string(),
            project_text: String::new(),
            keep_ratio: true,
            limits: None,
            limits_note: Vec::new(),
            pending_view: None,
            note: None,
            shot,
            visuals_theme: None,
            state,
        }
    }

    fn lang(&self) -> Lang {
        self.state.lang
    }

    /// The build volume of the printer that is chosen.
    fn volume(&self) -> [f64; 3] {
        self.state
            .printer()
            .map_or([220.0, 220.0, 250.0], |p| p.volume_mm)
    }

    // ---- what the person does

    fn add_part(&mut self, name: &str, mesh: Mesh) {
        let volume = self.volume();
        if self
            .studio
            .add_model(name, mesh, volume, &self.state.config)
            .is_some()
        {
            self.state.status = Status::Loaded(name.to_string());
        } else {
            self.state.status = Status::Problem(format!("{name}: the bed holds all it can"));
        }
        self.studio.sync(&mut self.state);
    }

    fn open_model(&mut self) {
        let path = self.path_text.trim().to_string();
        match self.state.read_model_file(&path) {
            Ok((name, mesh)) => self.add_part(&name, mesh),
            Err(e) => self.state.status = Status::Problem(e),
        }
    }

    fn load_demo(&mut self) {
        self.add_part(demo::DEMO_NAME, demo::goblet());
    }

    /// The filament that is chosen, with the nozzle.
    fn chosen_filament(&self) -> Option<(FilamentProfile, Nozzle)> {
        let nozzle = Nozzle {
            diameter_mm: self.nozzle_mm,
            hardened: false,
        };
        self.filament_id
            .as_deref()
            .and_then(|id| self.filaments.find(id))
            .cloned()
            .map(|f| (f, nozzle))
    }

    fn slice(&mut self) {
        let Some(printer) = self.state.printer().cloned() else {
            return;
        };
        let filament = self.chosen_filament();
        if self.studio.is_slicing() {
            return;
        }
        if self.studio.start_slice(&self.preview, &printer, filament) {
            self.note = None;
        } else {
            self.state.status = Status::Problem(Msg::NoModel.tr(self.lang()).to_string());
        }
    }

    /// Slices and waits for the end (the picture mode has no time to wait for a frame).
    fn slice_blocking(&mut self) {
        let Some(printer) = self.state.printer().cloned() else {
            return;
        };
        let filament = self.chosen_filament();
        self.studio
            .slice_blocking(&mut self.preview, &printer, filament);
    }

    fn poll_slice(&mut self, ctx: &egui::Context) {
        match self.studio.poll_slice(&mut self.preview) {
            SliceEvent::Idle => {}
            SliceEvent::Running { .. } => ctx.request_repaint(),
            SliceEvent::Finished { ok, seconds } => {
                if ok {
                    self.note = Some(fill(
                        Msg::SliceFinished.tr(self.lang()),
                        &[&format!("{seconds:.1}")],
                    ));
                    self.state.tab = Tab::Preview;
                }
            }
            SliceEvent::Lost => {
                self.preview.error = Some("the slice stopped without an answer".to_string());
            }
        }
    }

    fn refresh_device(&mut self) {
        self.device.refresh(self.server.device_client());
        self.device_read = true;
        self.device_read_at = Some(std::time::Instant::now());
    }

    fn emergency_stop(&mut self) {
        self.device.emergency_stop(self.server.device_client());
    }

    /// Keeps lines in the settings file the person gave, if one was given (a picture keeps nothing).
    fn keep_lines(&mut self, lines: &[(&str, String)]) {
        if self.shot.is_some() {
            return;
        }
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let merged = server_ui::merge_lines(&text, lines);
        if let Err(e) = std::fs::write(&path, merged) {
            self.state.status = Status::Problem(format!("{}: {e}", path.display()));
        }
    }

    /// Takes lines out of the settings file the person gave.
    fn forget_lines(&mut self, keys: &[&str]) {
        if self.shot.is_some() {
            return;
        }
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, server_ui::drop_keys(&text, keys)) {
            self.state.status = Status::Problem(format!("{}: {e}", path.display()));
        }
    }

    /// Keeps the settings of the window itself.
    fn keep_window_settings(&mut self) {
        let c = &self.state.config;
        let onoff = |b: bool| if b { "on" } else { "off" }.to_string();
        let lines = [
            ("theme", c.theme.name().to_string()),
            ("grid_mm", c.grid_mm.to_string()),
            ("arrange_gap_mm", c.arrange_gap_mm.to_string()),
            ("arrange_on_add", onoff(c.arrange_on_add)),
            ("ui_hints", onoff(c.ui_hints)),
        ];
        self.keep_lines(&lines);
    }

    fn set_theme(&mut self, theme: Theme) {
        self.state.config.theme = theme;
        self.look = theme.palette();
        self.keep_window_settings();
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::Open => {
                self.state.tab = Tab::Prepare;
                self.open_model();
            }
            Action::Save => {
                self.state.tab = Tab::Project;
                let path = self.save_text.trim().to_string();
                self.state.save_project(&path, false);
            }
            Action::Slice => self.slice(),
            Action::NextTab => self.state.tab = self.state.tab.next(),
            Action::PreviousTab => self.state.tab = self.state.tab.previous(),
            Action::GoPrepare => self.state.tab = Tab::Prepare,
            Action::GoPreview => self.state.tab = Tab::Preview,
            Action::GoDevice => self.state.tab = Tab::Device,
            Action::GoProject => self.state.tab = Tab::Project,
            Action::Refresh => self.refresh_device(),
            Action::ResetView => {
                self.preview.reset_view();
                self.studio.fit_next = true;
            }
            Action::FitView => {
                self.preview.reset_view();
                self.studio.fit_next = true;
                self.studio.fit_bed = true;
            }
            Action::LayerUp => self.preview.step_layer(1),
            Action::LayerDown => self.preview.step_layer(-1),
            Action::EmergencyStop => self.emergency_stop(),
            Action::ShowShortcuts => self.show_shortcuts = !self.show_shortcuts,
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let found: Vec<Action> = self
            .scheme
            .rows()
            .into_iter()
            .filter_map(|(a, _)| self.scheme.chord(a).map(|c| (a, c.clone())))
            .filter(|(_, c)| pressed(ctx, c))
            .map(|(a, _)| a)
            .collect();
        for a in found {
            self.run_action(a);
        }
    }

    // ---- small pieces

    fn draw_logo(&self, ui: &mut egui::Ui, size: f32) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let at = |p: [f32; 2]| Pos2::new(rect.min.x + p[0] * size, rect.min.y + p[1] * size);
        for line in look::logo() {
            for w in line.windows(2) {
                painter.line_segment(
                    [at(w[0]), at(w[1])],
                    Stroke::new(1.6, col(self.look.accent)),
                );
            }
        }
    }

    fn draw_glyph(&self, ui: &mut egui::Ui, tab: Tab, active: bool) {
        let size = 16.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let at = |p: [f32; 2]| Pos2::new(rect.min.x + p[0] * size, rect.min.y + p[1] * size);
        let c = if active {
            self.look.accent
        } else {
            self.look.dim
        };
        for line in look::glyph(tab) {
            for w in line.windows(2) {
                painter.line_segment([at(w[0]), at(w[1])], Stroke::new(1.2, col(c)));
            }
        }
    }

    /// A band of text on the accent colour, wrapped to the width there is.
    fn banner(&self, ui: &mut egui::Ui, text: &str) {
        let width = ui.available_width();
        let lines = look::wrap_lines(text, ((width - 24.0) / 7.5).max(20.0) as usize);
        let height = 12.0 + 16.0 * lines.len() as f32;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, col(self.look.accent));
        let ink = col(look::readable_on(
            self.look.accent,
            self.look.text,
            self.look.background,
        ));
        for (i, line) in lines.iter().enumerate() {
            painter.text(
                Pos2::new(rect.min.x + 10.0, rect.min.y + 6.0 + 16.0 * i as f32),
                egui::Align2::LEFT_TOP,
                line,
                egui::FontId::proportional(13.0),
                ink,
            );
        }
    }

    /// A short explanation that folds away; it is there only when the settings ask for hints.
    fn hint(&self, ui: &mut egui::Ui, id: &str, text: &str) {
        if !self.state.config.ui_hints {
            return;
        }
        egui::CollapsingHeader::new(Msg::HelpTitle.tr(self.lang()))
            .id_salt(id)
            .default_open(false)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).small().color(col(self.look.dim)));
            });
    }

    // ---- the bars

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                self.draw_logo(ui, 26.0);
                ui.label(
                    egui::RichText::new("EH3D-PHOENIX-3DP")
                        .strong()
                        .size(17.0)
                        .color(col(self.look.accent)),
                );
                ui.separator();
                for tab in Tab::ALL {
                    let active = self.state.tab == tab;
                    self.draw_glyph(ui, tab, active);
                    if ui.selectable_label(active, tab.msg().tr(lang)).clicked() {
                        self.state.tab = tab;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(self.state.lang.native_name())
                        .show_ui(ui, |ui| {
                            for l in Lang::ALL {
                                ui.selectable_value(&mut self.state.lang, l, l.native_name());
                            }
                        });
                    let theme = self.state.config.theme;
                    let theme_name = match theme {
                        Theme::Dark => Msg::ThemeDark,
                        Theme::Light => Msg::ThemeLight,
                    };
                    let label = format!("{}: {}", Msg::ThemeName.tr(lang), theme_name.tr(lang));
                    if ui.button(label).clicked() {
                        self.set_theme(theme.toggled());
                    }
                    if ui.button("?").clicked() {
                        self.show_help = !self.show_help;
                    }
                    self.server.refresh();
                    let badge = self.server.panel.badge();
                    server_gui::link_badge(ui, badge, server_ui::view_lang(lang));
                    if self.server.has_server() {
                        let ink = col(look::readable_on(
                            self.look.danger,
                            [255, 255, 255],
                            self.look.background,
                        ));
                        let stop = egui::Button::new(
                            egui::RichText::new(Msg::EmergencyStop.tr(lang))
                                .strong()
                                .color(ink),
                        )
                        .fill(col(self.look.danger));
                        // there is no question in front of it
                        if ui.add(stop).clicked() {
                            self.emergency_stop();
                        }
                    }
                });
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let text = self.state.status.text(lang);
                let color = if self.state.status.is_problem() {
                    self.look.danger
                } else {
                    self.look.good
                };
                ui.label(egui::RichText::new(text).color(col(color)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(Msg::SimulatorOnly.tr(lang)).color(col(self.look.dim)),
                    );
                });
            });
        });
    }

    /// The bar of the slice: the button, the progress while it runs and the estimates after.
    fn slice_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let volume = self.volume();
        let ink = col(look::readable_on(
            self.look.accent,
            self.look.text,
            self.look.background,
        ));
        egui::Panel::bottom("slice_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let slicing = self.studio.is_slicing();
                let can = !self.studio.plate.is_empty() && !slicing;
                let label = egui::RichText::new(Msg::SliceNow.tr(lang))
                    .strong()
                    .color(ink);
                let button = egui::Button::new(label)
                    .fill(col(self.look.accent))
                    .min_size(egui::vec2(150.0, 30.0));
                if ui.add_enabled(can, button).clicked() {
                    self.slice();
                }
                if slicing {
                    ui.add(
                        egui::ProgressBar::new(self.studio.progress)
                            .desired_width(240.0)
                            .animate(true)
                            .text(Msg::Slicing.tr(lang)),
                    );
                } else if Estimates::pending(
                    !self.studio.plate.is_empty(),
                    slicing,
                    self.preview.plan.is_some(),
                ) {
                    ui.label(
                        egui::RichText::new(Msg::EstimatesPending.tr(lang))
                            .color(col(self.look.dim)),
                    );
                } else if let Some(e) = Estimates::of(&self.preview, &self.state.config) {
                    for (label, value) in e.cells(&self.state.config, lang) {
                        ui.label(egui::RichText::new(label).small().color(col(self.look.dim)));
                        ui.label(egui::RichText::new(value).strong());
                        ui.separator();
                    }
                    if self.studio.stale {
                        ui.label(
                            egui::RichText::new(Msg::StalePlan.tr(lang))
                                .color(col(self.look.accent)),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for w in self.studio.warnings(volume) {
                        ui.label(egui::RichText::new(w.text(lang)).color(col(self.look.danger)));
                    }
                    if let Some(n) = &self.note {
                        ui.label(egui::RichText::new(n).color(col(self.look.good)));
                    }
                });
            });
        });
    }

    // ---- the Prepare tab: the left panel

    fn printer_list(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.horizontal(|ui| {
            ui.label(Msg::FilterSearch.tr(lang));
            ui.add(
                egui::TextEdit::singleline(&mut self.printer_filter.search)
                    .hint_text(Msg::SearchHint.tr(lang))
                    .desired_width(140.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label(Msg::FilterTechnology.tr(lang));
            let shown = match self.printer_filter.technology {
                None => Msg::FilterAll.tr(lang),
                Some(Technology::Fdm) => Msg::TechFdm.tr(lang),
                Some(Technology::Msla) => Msg::TechResin.tr(lang),
            };
            egui::ComboBox::from_id_salt("technology")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.printer_filter.technology,
                        None,
                        Msg::FilterAll.tr(lang),
                    );
                    ui.selectable_value(
                        &mut self.printer_filter.technology,
                        Some(Technology::Fdm),
                        Msg::TechFdm.tr(lang),
                    );
                    ui.selectable_value(
                        &mut self.printer_filter.technology,
                        Some(Technology::Msla),
                        Msg::TechResin.tr(lang),
                    );
                });
        });
        // the model on the bed filters the list when it is asked
        let mut fits = self.printer_filter.fits_size_mm.is_some();
        let size = self.state.model.as_ref().and_then(|m| m.size_mm());
        if ui
            .add_enabled(
                size.is_some(),
                egui::Checkbox::new(&mut fits, Msg::FitsYes.tr(lang)),
            )
            .changed()
        {
            self.printer_filter.fits_size_mm = if fits { size } else { None };
        }
        let rows: Vec<(String, String)> = self
            .printer_filter
            .apply(&self.state.catalog)
            .iter()
            .map(|p| (p.id.clone(), p.title()))
            .collect();
        ui.label(
            egui::RichText::new(fill(
                Msg::PrintersShown.tr(lang),
                &[
                    &rows.len().to_string(),
                    &self.state.catalog.len().to_string(),
                ],
            ))
            .color(col(self.look.dim)),
        );
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("printers")
            .max_height(170.0)
            .show(ui, |ui| {
                for (id, title) in &rows {
                    let on = *id == self.state.printer_id;
                    if ui.selectable_label(on, title).clicked() {
                        chosen = Some(id.clone());
                    }
                }
            });
        if let Some(id) = chosen {
            if self.state.select_printer(&id) {
                self.studio.fit_next = true;
            }
        }
    }

    /// The limits of the printer that is chosen, to type and keep.
    fn limits_editor(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let Some(printer) = self.state.printer().cloned() else {
            return;
        };
        let Some(current) = LimitsDraft::of(&printer) else {
            return;
        };
        let mut draft = match self.limits.take() {
            Some((id, d)) if id == printer.id => d,
            _ => {
                self.limits_note.clear();
                current
            }
        };
        let mut chamber = draft.chamber_c.unwrap_or(0.0);
        egui::Grid::new("limits_grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(Msg::LimitNozzle.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut draft.nozzle_c)
                        .range(1.0..=600.0)
                        .speed(1.0),
                );
                ui.end_row();
                ui.label(Msg::LimitBed.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut draft.bed_c)
                        .range(1.0..=200.0)
                        .speed(1.0),
                );
                ui.end_row();
                ui.label(Msg::LimitChamber.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut chamber)
                        .range(0.0..=200.0)
                        .speed(1.0),
                );
                ui.end_row();
            });
        draft.chamber_c = if chamber > 0.0 { Some(chamber) } else { None };
        if self.state.limits_are_mine() {
            ui.label(
                egui::RichText::new(Msg::LimitsMine.tr(lang))
                    .small()
                    .color(col(self.look.accent)),
            );
        }
        ui.horizontal(|ui| {
            if ui.button(Msg::ApplyLimits.tr(lang)).clicked() {
                match self.state.set_printer_limits(&draft) {
                    Ok(warnings) => {
                        self.limits_note = warnings.iter().map(|w| w.text(lang)).collect();
                        self.limits_note.push(Msg::LimitsSaved.tr(lang).to_string());
                        let lines: Vec<(String, String)> =
                            limits_ui::override_lines(&self.state.config);
                        let refs: Vec<(&str, String)> =
                            lines.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                        self.keep_lines(&refs);
                    }
                    Err(e) => self.limits_note = vec![e],
                }
            }
            if ui.button(Msg::BackToCatalog.tr(lang)).clicked() && self.state.reset_printer_limits()
            {
                let key = format!("{}{}", crate::config::LIMITS_KEY_PREFIX, printer.id);
                self.forget_lines(&[key.as_str()]);
                self.limits_note.clear();
                draft = self
                    .state
                    .printer()
                    .and_then(LimitsDraft::of)
                    .unwrap_or(current);
            }
        });
        for n in &self.limits_note {
            ui.label(egui::RichText::new(n).small().color(col(self.look.dim)));
        }
        self.limits = Some((printer.id.clone(), draft));
    }

    fn filament_section(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.horizontal(|ui| {
            ui.label(Msg::FilamentMaterial.tr(lang));
            let shown = self
                .filament_filter
                .material
                .map_or(Msg::FilterAll.tr(lang).to_string(), |m| {
                    m.name().to_uppercase()
                });
            egui::ComboBox::from_id_salt("material")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.filament_filter.material,
                        None,
                        Msg::FilterAll.tr(lang),
                    );
                    for m in Material::ALL {
                        ui.selectable_value(
                            &mut self.filament_filter.material,
                            Some(m),
                            m.name().to_uppercase(),
                        );
                    }
                });
        });
        let nozzle = Nozzle {
            diameter_mm: self.nozzle_mm,
            hardened: false,
        };
        let mut only = self.filament_filter.compatible_with.is_some();
        if ui
            .checkbox(&mut only, Msg::FilamentCompatible.tr(lang))
            .changed()
        {
            self.filament_filter.compatible_with = if only { Some(nozzle) } else { None };
        }
        if let Some(p) = self.state.printer() {
            let nozzles = p.nozzles_mm.clone();
            if !nozzles.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(Msg::Nozzle.tr(lang));
                    egui::ComboBox::from_id_salt("nozzle")
                        .selected_text(format!("{} mm", self.nozzle_mm))
                        .show_ui(ui, |ui| {
                            for n in &nozzles {
                                ui.selectable_value(&mut self.nozzle_mm, *n, format!("{n} mm"));
                            }
                        });
                });
            }
        }
        let printer = self.state.printer().cloned();
        let rows: Vec<(String, String)> = self
            .filament_filter
            .apply(&self.filaments, printer.as_ref())
            .iter()
            .map(|r| (r.profile.id.clone(), r.profile.name.clone()))
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("filaments")
            .max_height(140.0)
            .show(ui, |ui| {
                for (id, name) in &rows {
                    let on = self.filament_id.as_deref() == Some(id.as_str());
                    if ui.selectable_label(on, name).clicked() {
                        self.filament_id = if on { None } else { Some(id.clone()) };
                    }
                }
            });
        if self.filament_id.is_none() {
            ui.label(
                egui::RichText::new(Msg::NoFilamentChosen.tr(lang))
                    .small()
                    .color(col(self.look.dim)),
            );
        }
    }

    fn slice_options(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let c = &mut self.preview.config;
        egui::Grid::new("process_grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(Msg::PanelLayers.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut c.layer_height_mm)
                        .range(0.05..=1.0)
                        .speed(0.01)
                        .suffix(" mm"),
                );
                ui.end_row();
                ui.label(Msg::KindPerimeters.tr(lang));
                ui.add(egui::DragValue::new(&mut c.perimeters).range(1..=10));
                ui.end_row();
                ui.label(Msg::PanelInfill.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut c.infill_percent)
                        .range(0.0..=100.0)
                        .suffix(" %"),
                );
                ui.end_row();
            });
        let patterns = [
            (InfillPattern::Rectilinear, Msg::PatternRectilinear),
            (InfillPattern::Grid, Msg::PatternGrid),
            (InfillPattern::Gyroid, Msg::PatternGyroid),
        ];
        let shown = patterns
            .iter()
            .find(|(p, _)| *p == c.infill_pattern)
            .map_or("", |(_, m)| m.tr(lang));
        egui::ComboBox::from_id_salt("pattern")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (p, m) in patterns {
                    ui.selectable_value(&mut c.infill_pattern, p, m.tr(lang));
                }
            });
        ui.horizontal(|ui| {
            ui.label(Msg::PanelSupports.tr(lang));
            let names = [
                (Supports::Off, Msg::SupportsOff),
                (Supports::BuildPlate, Msg::SupportsBed),
                (Supports::Everywhere, Msg::SupportsEverywhere),
            ];
            let shown = names
                .iter()
                .find(|(p, _)| *p == c.supports)
                .map_or("", |(_, m)| m.tr(lang));
            egui::ComboBox::from_id_salt("supports")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (p, m) in names {
                        ui.selectable_value(&mut c.supports, p, m.tr(lang));
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(Msg::PanelSeam.tr(lang));
            let seams = [
                (Seam::Nearest, Msg::SeamNearest),
                (Seam::Aligned, Msg::SeamAligned),
                (Seam::Rear, Msg::SeamRear),
            ];
            let shown = seams
                .iter()
                .find(|(p, _)| *p == c.seam)
                .map_or("", |(_, m)| m.tr(lang));
            egui::ComboBox::from_id_salt("seam")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (p, m) in seams {
                        ui.selectable_value(&mut c.seam, p, m.tr(lang));
                    }
                });
        });
        let mut with_adhesion = c.adhesion != Adhesion::None;
        if ui
            .checkbox(&mut with_adhesion, Msg::KindAdhesion.tr(lang))
            .changed()
        {
            c.adhesion = if with_adhesion {
                Adhesion::Brim
            } else {
                Adhesion::None
            };
        }
        for a in &self.preview.advice {
            ui.label(egui::RichText::new(a).small().color(col(self.look.dim)));
        }
        if let Some(e) = &self.preview.error {
            ui.label(
                egui::RichText::new(fill(Msg::Problem.tr(lang), &[e])).color(col(self.look.danger)),
            );
        }
    }

    fn prepare_left(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        self.hint(ui, "hint_prepare", Msg::HintPrepare.tr(lang));
        egui::CollapsingHeader::new(egui::RichText::new(Msg::Printer.tr(lang)).strong())
            .id_salt("sec_printer")
            .default_open(true)
            .show(ui, |ui| {
                self.printer_list(ui);
                ui.separator();
                for line in self.state.printer_report() {
                    ui.label(egui::RichText::new(line).color(col(self.look.dim)));
                }
                let v = self.volume();
                ui.label(fill(
                    Msg::BedSize.tr(lang),
                    &[
                        &format!("{:.0}", v[0]),
                        &format!("{:.0}", v[1]),
                        &format!("{:.0}", v[2]),
                    ],
                ));
                egui::CollapsingHeader::new(Msg::PanelLimits.tr(lang))
                    .id_salt("sec_limits")
                    .default_open(false)
                    .show(ui, |ui| self.limits_editor(ui));
            });
        egui::CollapsingHeader::new(egui::RichText::new(Msg::PanelFilament.tr(lang)).strong())
            .id_salt("sec_filament")
            .default_open(true)
            .show(ui, |ui| self.filament_section(ui));
        egui::CollapsingHeader::new(egui::RichText::new(Msg::PanelProcess.tr(lang)).strong())
            .id_salt("sec_process")
            .default_open(true)
            .show(ui, |ui| self.slice_options(ui));
        ui.separator();
        ui.label(
            egui::RichText::new(Msg::Safety.tr(lang))
                .color(col(self.look.dim))
                .small(),
        );
    }

    // ---- the Prepare tab: the right panel

    fn objects_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let volume = self.volume();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(Msg::PanelObjects.tr(lang)).strong());
            ui.label(
                egui::RichText::new(fill(
                    Msg::PartsCount.tr(lang),
                    &[&self.studio.plate.len().to_string()],
                ))
                .color(col(self.look.dim)),
            );
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.path_text)
                    .hint_text(Msg::OpenModelHint.tr(lang))
                    .desired_width(150.0),
            );
            if ui.button(Msg::Open.tr(lang)).clicked() {
                self.open_model();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(Msg::LoadDemo.tr(lang)).clicked() {
                self.load_demo();
            }
            if ui
                .add_enabled(
                    self.studio.plate.len() > 1,
                    egui::Button::new(Msg::Arrange.tr(lang)),
                )
                .clicked()
            {
                self.studio.arrange(volume, &self.state.config);
            }
        });
        let rows: Vec<(u32, String)> = self
            .studio
            .plate
            .objects
            .iter()
            .map(|o| (o.id, o.name.clone()))
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("object_list")
            .max_height(130.0)
            .show(ui, |ui| {
                for (id, name) in &rows {
                    let on = self.studio.plate.selected == Some(*id);
                    if ui.selectable_label(on, name).clicked() {
                        self.studio.plate.select(*id);
                    }
                }
            });
        ui.separator();
        if self.studio.plate.selected_object().is_none() {
            ui.label(egui::RichText::new(Msg::NothingSelected.tr(lang)).color(col(self.look.dim)));
            return;
        }
        ui.checkbox(&mut self.keep_ratio, Msg::KeepProportions.tr(lang));
        let keep = self.keep_ratio;
        let axes = ["X", "Y", "Z"];
        let mut edited = false;
        if let Some(o) = self.studio.plate.selected_mut() {
            ui.label(egui::RichText::new(Msg::ObjectPlacement.tr(lang)).strong());
            ui.horizontal(|ui| {
                for (k, name) in axes.iter().take(2).enumerate() {
                    ui.label(*name);
                    edited |= ui
                        .add(
                            egui::DragValue::new(&mut o.pos[k])
                                .speed(0.5)
                                .range(-1000.0..=1000.0)
                                .suffix(" mm"),
                        )
                        .changed();
                }
            });
            ui.label(egui::RichText::new(Msg::ObjectRotation.tr(lang)).strong());
            ui.horizontal(|ui| {
                for (k, name) in axes.iter().enumerate() {
                    ui.label(*name);
                    edited |= ui
                        .add(
                            egui::DragValue::new(&mut o.rot_deg[k])
                                .speed(1.0)
                                .range(-360.0..=360.0)
                                .suffix(" deg"),
                        )
                        .changed();
                }
            });
            ui.label(egui::RichText::new(Msg::SizeMm.tr(lang)).strong());
            let size = o.size_mm().unwrap_or([0.0; 3]);
            ui.horizontal(|ui| {
                for (k, name) in axes.iter().enumerate() {
                    ui.label(*name);
                    let mut v = size[k];
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.5)
                                .range(0.1..=5000.0)
                                .suffix(" mm"),
                        )
                        .changed()
                        && o.set_size_mm(k, v, keep)
                    {
                        edited = true;
                    }
                }
            });
            ui.label(egui::RichText::new(Msg::ObjectScale.tr(lang)).strong());
            let mut percent = o.scale[0] * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut percent)
                        .speed(1.0)
                        .range(1.0..=10_000.0)
                        .suffix(" %"),
                )
                .changed()
                && o.set_scale_percent(percent)
            {
                edited = true;
            }
        }
        if edited {
            self.studio.mark_edited();
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button(Msg::AutoOrient.tr(lang)).clicked() {
                self.studio.auto_orient_selected();
            }
            if ui.button(Msg::Duplicate.tr(lang)).clicked() {
                self.studio.duplicate_selected(volume, &self.state.config);
            }
            if ui.button(Msg::Remove.tr(lang)).clicked() {
                self.studio.remove_selected();
            }
            if ui.button(Msg::ResetObject.tr(lang)).clicked() {
                if let Some(o) = self.studio.plate.selected_mut() {
                    o.reset();
                }
                self.studio.mark_edited();
            }
        });
    }

    // ---- the 3D view of the bed

    fn scene_view(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let volume = self.volume();
        let pal = self.look;
        ui.horizontal_wrapped(|ui| {
            for v in StdView::ALL {
                if ui.button(view_msg(v).tr(lang)).clicked() {
                    self.pending_view = Some(v);
                }
            }
        });
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, col(pal.panel));
        let Some(vp) = Viewport::new(f64::from(rect.width()), f64::from(rect.height())) else {
            return;
        };
        if let Some(v) = self.pending_view.take() {
            self.studio.look_from(v, volume, vp);
        }
        self.studio.fit_if_asked(volume, vp);
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            self.studio
                .camera
                .orbit(f64::from(d.x), f64::from(d.y), 0.01);
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            self.studio.camera.pan(f64::from(d.x), f64::from(d.y), vp);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.studio.camera.zoom((f64::from(-scroll) * 0.002).exp());
            }
        }
        let cam = self.studio.camera;
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let px = [f64::from(pos.x - rect.min.x), f64::from(pos.y - rect.min.y)];
                match shade::pick(&self.studio.boxes(), &cam, vp, px) {
                    Some(id) => {
                        self.studio.plate.select(id);
                    }
                    None => self.studio.plate.selected = None,
                }
            }
        }
        // the bed
        let drawing = shade::bed(volume, self.state.config.grid_mm, &cam, vp);
        let at = |p: [f32; 2]| Pos2::new(rect.min.x + p[0], rect.min.y + p[1]);
        if let Some(q) = drawing.quad {
            painter.add(egui::Shape::convex_polygon(
                q.iter().map(|p| at(*p)).collect(),
                col(pal.bed),
                Stroke::NONE,
            ));
        }
        for l in &drawing.lines {
            painter.line_segment([at(l.a), at(l.b)], bed_stroke(l.kind, &pal));
        }
        // the parts
        let tris = {
            let parts: Vec<(u32, &Mesh)> = self
                .studio
                .world_parts()
                .iter()
                .map(|(id, m)| (*id, m))
                .collect();
            shade::shade(&parts, &cam, vp)
        };
        let selected = self.studio.plate.selected;
        paint_triangles(&painter, rect.min, &tris, &|id| {
            if Some(id) == selected {
                pal.selected
            } else {
                pal.model
            }
        });
        // the outline of the part that is chosen, and of the ones that go past the edge
        let outside = self.studio.plate.out_of_bounds(volume, 0.0);
        for o in &self.studio.plate.objects {
            let is_selected = selected == Some(o.id);
            let is_outside = outside.contains(&o.id);
            if !(is_selected || is_outside) {
                continue;
            }
            if let Some((lo, hi)) = o.world_bounds() {
                let colour = if is_outside { pal.danger } else { pal.selected };
                for (a, b) in shade::box_lines(lo, hi, &cam, vp) {
                    painter.line_segment([at(a), at(b)], Stroke::new(1.0, col(colour)));
                }
            }
        }
        if self.studio.plate.is_empty() {
            let max_chars = ((rect.width() - 40.0) / 8.5).max(20.0) as usize;
            let lines = look::wrap_lines(Msg::EmptyBedHint.tr(lang), max_chars);
            let top = rect.center().y - 9.0 * lines.len() as f32;
            for (i, line) in lines.iter().enumerate() {
                painter.text(
                    Pos2::new(rect.center().x, top + 18.0 * i as f32),
                    egui::Align2::CENTER_TOP,
                    line,
                    egui::FontId::proportional(15.0),
                    col(pal.dim),
                );
            }
        }
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.max.y - 8.0),
            egui::Align2::LEFT_BOTTOM,
            fill(
                Msg::BedSize.tr(lang),
                &[
                    &format!("{:.0}", volume[0]),
                    &format!("{:.0}", volume[1]),
                    &format!("{:.0}", volume[2]),
                ],
            ),
            egui::FontId::proportional(12.0),
            col(pal.dim),
        );
    }

    fn prepare_panels(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("prepare_left")
            .default_size(310.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("prepare_left_scroll")
                    .show(ui, |ui| self.prepare_left(ui));
            });
        egui::Panel::right("prepare_right")
            .default_size(290.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("prepare_right_scroll")
                    .show(ui, |ui| self.objects_panel(ui));
            });
    }

    // ---- the Preview tab

    /// The vertical slider of the layers: the top of the track is the last layer.
    fn layer_slider(&mut self, ui: &mut egui::Ui) {
        let count = self.preview.layer_count();
        if count == 0 {
            return;
        }
        let pal = self.look;
        ui.horizontal(|ui| {
            if ui.button("+").clicked() {
                self.preview.step_layer(1);
            }
            if ui.button("-").clicked() {
                self.preview.step_layer(-1);
            }
        });
        let height = (ui.available_height() - 36.0).max(80.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(40.0, height), egui::Sense::click_and_drag());
        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let f = f64::from((pos.y - rect.min.y) / rect.height());
                self.preview.set_layer(preview::layer_at(f, count));
            }
        }
        let painter = ui.painter_at(rect);
        let x = rect.center().x;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(3.0, col(pal.grid)),
        );
        let y = rect.min.y
            + rect.height() * preview::fraction_of_layer(self.preview.layer, count) as f32;
        painter.line_segment(
            [Pos2::new(x, y), Pos2::new(x, rect.max.y)],
            Stroke::new(3.0, col(pal.accent)),
        );
        painter.circle_filled(Pos2::new(x, y), 8.0, col(pal.accent));
        ui.label(format!("{}/{}", self.preview.layer + 1, count));
    }

    fn thumbnail(&mut self, ui: &mut egui::Ui) {
        let pal = self.look;
        let side = 120.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, col(pal.panel));
        let tris = {
            let parts: Vec<(u32, &Mesh)> = self
                .studio
                .world_parts()
                .iter()
                .map(|(id, m)| (*id, m))
                .collect();
            shade::thumbnail(&parts, f64::from(side))
        };
        paint_triangles(&painter, rect.min, &tris, &|_| pal.model);
    }

    fn preview_left(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        self.hint(ui, "hint_preview", Msg::HintPreview.tr(lang));
        self.thumbnail(ui);
        ui.separator();
        let lengths = self.preview.lengths();
        let mm = |kinds: &[Kind]| -> f64 {
            lengths
                .iter()
                .filter(|(k, _)| kinds.contains(k))
                .map(|(_, v)| *v)
                .sum()
        };
        let pal = self.look;
        let colour = p3dp_viewer::layers::colour_of;
        let f = &mut self.preview.filter;
        let rows: [(&mut bool, Msg, [f32; 3], f64); 7] = [
            (
                &mut f.perimeters,
                Msg::KindPerimeters,
                colour(Kind::ExternalPerimeter),
                mm(&[Kind::ExternalPerimeter, Kind::Perimeter]),
            ),
            (
                &mut f.infill,
                Msg::KindInfill,
                colour(Kind::Infill),
                mm(&[Kind::Infill]),
            ),
            (
                &mut f.solid,
                Msg::KindSolid,
                colour(Kind::SolidInfill),
                mm(&[Kind::SolidInfill]),
            ),
            (
                &mut f.bridges,
                Msg::PanelBridges,
                colour(Kind::Bridge),
                mm(&[Kind::Bridge]),
            ),
            (
                &mut f.supports,
                Msg::PanelSupports,
                colour(Kind::Support),
                mm(&[Kind::Support]),
            ),
            (
                &mut f.adhesion,
                Msg::KindAdhesion,
                colour(Kind::Adhesion),
                mm(&[Kind::Adhesion, Kind::Raft]),
            ),
            (
                &mut self.preview.show_travel,
                Msg::ViewTravel,
                p3dp_viewer::layers::TRAVEL_COLOUR,
                0.0,
            ),
        ];
        for (flag, msg, c, length) in rows {
            ui.horizontal(|ui| {
                let (swatch, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 2.0, col_f(c));
                ui.checkbox(flag, msg.tr(lang));
                if length > 0.0 {
                    ui.label(
                        egui::RichText::new(format!("{length:.0} mm"))
                            .small()
                            .color(col(pal.dim)),
                    );
                }
            });
        }
        ui.checkbox(&mut self.preview.only_one, Msg::LayerOnlyOne.tr(lang));
        ui.checkbox(&mut self.preview.peek_inside, Msg::ViewPeekInside.tr(lang));
        if ui.button(Msg::ViewReset.tr(lang)).clicked() {
            self.preview.reset_view();
        }
        if let Some(s) = self.preview.progress_label(lang) {
            ui.label(egui::RichText::new(s).small().color(col(pal.dim)));
        }
        egui::CollapsingHeader::new(Msg::PanelEstimate.tr(lang))
            .id_salt("sec_estimate")
            .default_open(false)
            .show(ui, |ui| {
                for l in self.preview.report_lines(&self.state.config, lang) {
                    ui.label(l);
                }
            });
    }

    fn preview_panels(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("preview_left")
            .default_size(260.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("preview_left_scroll")
                    .show(ui, |ui| self.preview_left(ui));
            });
        egui::Panel::right("preview_right")
            .exact_size(86.0)
            .show(ui, |ui| self.layer_slider(ui));
    }

    fn preview_centre(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let pal = self.look;
        ui.horizontal(|ui| {
            if let Some(s) = self.preview.summary(lang) {
                ui.label(egui::RichText::new(s).strong());
            }
            if self.preview.plan.is_some() && self.server.has_server() {
                let queue = format!("+ {}", Msg::QueueTitle.tr(lang));
                if ui.button(queue).clicked() {
                    self.send_to_queue();
                }
            }
            if self.studio.stale && self.preview.plan.is_some() {
                ui.label(egui::RichText::new(Msg::StalePlan.tr(lang)).color(col(pal.accent)));
            }
        });
        if let Some(e) = &self.preview.error {
            ui.label(egui::RichText::new(fill(Msg::Problem.tr(lang), &[e])).color(col(pal.danger)));
        }
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, col(pal.panel));
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            self.preview.orbit(f64::from(d.x), f64::from(d.y));
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            self.preview.pan(f64::from(d.x), f64::from(d.y), w, h);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.preview.zoom((f64::from(-scroll) * 0.002).exp());
            }
        }
        if self.preview.plan.is_none() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                Msg::NoPlanYet.tr(lang),
                egui::FontId::proportional(16.0),
                col(pal.dim),
            );
            return;
        }
        self.preview.grid_mm = self.state.config.grid_mm;
        let frame = self.preview.frame(w, h);
        let at = |p: [f32; 2]| Pos2::new(rect.min.x + p[0], rect.min.y + p[1]);
        if let Some(q) = frame.bed.quad {
            painter.add(egui::Shape::convex_polygon(
                q.iter().map(|p| at(*p)).collect(),
                col(pal.bed),
                Stroke::NONE,
            ));
        }
        for l in &frame.bed.lines {
            painter.line_segment([at(l.a), at(l.b)], bed_stroke(l.kind, &pal));
        }
        let shapes: Vec<egui::Shape> = frame
            .segments
            .iter()
            .map(|s| {
                egui::Shape::line_segment(
                    [
                        Pos2::new(rect.min.x + s.a[0], rect.min.y + s.a[1]),
                        Pos2::new(rect.min.x + s.b[0], rect.min.y + s.b[1]),
                    ],
                    Stroke::new(1.0, col_f(s.colour)),
                )
            })
            .collect();
        painter.extend(shapes);
        if frame.left_out > 0 {
            painter.text(
                Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
                egui::Align2::LEFT_TOP,
                format!("+{}", frame.left_out),
                egui::FontId::proportional(12.0),
                col(pal.dim),
            );
        }
    }

    fn send_to_queue(&mut self) {
        let Some(plan) = &self.preview.plan else {
            return;
        };
        let Some(printer) = self.state.printer() else {
            return;
        };
        let (Some(limits), Ok(gcode)) = (printer.limits, plan.gcode(env!("CARGO_PKG_VERSION")))
        else {
            return;
        };
        let name = if self.state.model_name.is_empty() {
            "job".to_string()
        } else {
            self.state.model_name.clone()
        };
        if !self.device_read {
            self.refresh_device();
        }
        if self
            .device
            .enqueue(self.server.device_client(), &name, &gcode, &limits)
            .is_some()
        {
            self.state.tab = Tab::Device;
        }
    }

    // ---- the Device tab

    fn device_left(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        self.hint(ui, "hint_device", Msg::HintDevice.tr(lang));
        ui.label(self.state.connection_line());
        egui::CollapsingHeader::new(Msg::ServerPanelTitle.tr(lang))
            .id_salt("sec_server")
            .show(ui, |ui| {
                let action =
                    server_gui::show(ui, &mut self.server.panel, server_ui::view_lang(lang));
                if let Some(a) = action {
                    if let Some(applied) = self.server.handle(a, &mut self.state.config.server) {
                        self.keep_lines(&applied.lines);
                        self.device.clear();
                        self.device_read = false;
                    }
                }
            });
        ui.separator();
        let policy = self.state.config.queue_policy();
        let rule = if policy.refuse_duplicates {
            Msg::RuleRefused
        } else {
            Msg::RuleAllowed
        };
        ui.label(
            egui::RichText::new(fill(
                Msg::QueueRules.tr(lang),
                &[rule.tr(lang), &policy.max_waiting.to_string()],
            ))
            .small()
            .color(col(self.look.dim)),
        );
        if !self.server.has_server() {
            ui.label(Msg::NoServerSet.tr(lang));
            return;
        }
        if !self.device_read {
            self.refresh_device();
        }
        ui.horizontal(|ui| {
            if ui.button(Msg::Refresh.tr(lang)).clicked() {
                self.refresh_device();
            }
        });
        if let Some(t) = self.device.error_text(lang) {
            ui.label(
                egui::RichText::new(fill(Msg::Problem.tr(lang), &[&t]))
                    .color(col(self.look.danger)),
            );
        } else if let Some(d) = &self.device.done {
            let text = match d {
                Done::Started(n)
                | Done::Paused(n)
                | Done::Resumed(n)
                | Done::Cancelled(n)
                | Done::Queued(n) => n.clone(),
                Done::Stopped(m, reached) => {
                    format!("{m}: {}", if *reached { "OK" } else { "?" })
                }
            };
            ui.label(egui::RichText::new(text).color(col(self.look.good)));
        }
        ui.label(egui::RichText::new(Msg::MachinesTitle.tr(lang)).strong());
        let machines: Vec<(String, String)> = self
            .device
            .machines
            .iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();
        for (id, name) in &machines {
            let on = self.device.selected.as_deref() == Some(id.as_str());
            if ui.selectable_label(on, name).clicked() && self.device.select(id) {
                self.refresh_device();
            }
        }
        if let Some(m) = self.device.machine() {
            let (msg, colour) = if m.auto_start_queue {
                (Msg::AutoStartOn, self.look.danger)
            } else {
                (Msg::AutoStartOff, self.look.good)
            };
            ui.label(egui::RichText::new(msg.tr(lang)).small().color(col(colour)));
        }
    }

    /// A gauge: what it measures and its figures on one line, the bar under it at the width there is (with the target of a heater marked on it) and a small line of detail.
    fn draw_bar(&self, ui: &mut egui::Ui, bar: &device::Bar) {
        let pal = self.look;
        let width = device::bar_width(ui.available_width());
        ui.scope(|ui| {
            ui.set_max_width(width);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&bar.label).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&bar.figures);
                });
            });
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, device::BAR_H), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, col(pal.grid));
            let filled = device::bar_fill_width(width, bar.fraction);
            if filled > 0.0 {
                let fill_rect =
                    egui::Rect::from_min_size(rect.min, egui::vec2(filled, device::BAR_H));
                painter.rect_filled(fill_rect, 4.0, col(pal.accent));
            }
            if let Some(m) = bar.marker {
                let x = rect.min.x + width * m.clamp(0.0, 1.0);
                painter.line_segment(
                    [
                        Pos2::new(x, rect.min.y - 2.0),
                        Pos2::new(x, rect.max.y + 2.0),
                    ],
                    Stroke::new(2.0, col(pal.text)),
                );
            }
            if let Some(d) = &bar.detail {
                ui.label(egui::RichText::new(d).small().color(col(pal.dim)));
            }
        });
        ui.add_space(8.0);
    }

    fn section_title(&self, ui: &mut egui::Ui, msg: Msg) {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(msg.tr(self.lang()))
                .strong()
                .size(15.0)
                .color(col(self.look.accent)),
        );
        ui.add_space(4.0);
    }

    fn device_centre(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let pal = self.look;
        // always: the server's printer plugin is a simulator
        self.banner(ui, Msg::SimBanner.tr(lang));
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("device_centre_scroll")
            .show(ui, |ui| {
                if !self.server.has_server() {
                    self.demo_printer(ui);
                    return;
                }
                if let Some(h) = self.device.headline() {
                    ui.label(egui::RichText::new(h).strong().size(17.0));
                }
                // the heaters are the first bars and the layer, when there is one, the last
                let bars = self.device.bars(lang);
                let (heaters, print) = bars.split_at(self.device.heaters.len().min(bars.len()));
                self.section_title(ui, Msg::DeviceTemperatures);
                if heaters.is_empty() {
                    ui.label(
                        egui::RichText::new(Msg::DeviceNoHeaters.tr(lang)).color(col(pal.dim)),
                    );
                }
                for b in heaters {
                    self.draw_bar(ui, b);
                }
                self.section_title(ui, Msg::DevicePrintTitle);
                if print.is_empty() {
                    ui.label(egui::RichText::new(Msg::DeviceNoPrint.tr(lang)).color(col(pal.dim)));
                }
                for b in print {
                    self.draw_bar(ui, b);
                }
                ui.add_space(10.0);
                if let Some(at) = self.device_read_at {
                    let text = device::read_age_text(at.elapsed().as_secs(), lang);
                    ui.label(egui::RichText::new(text).small().color(col(pal.dim)));
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs(1));
                }
                ui.label(
                    egui::RichText::new(Msg::Safety.tr(lang))
                        .small()
                        .color(col(pal.dim)),
                );
            });
    }

    fn device_panels(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("device_left")
            .default_size(310.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("device_left_scroll")
                    .show(ui, |ui| self.device_left(ui));
            });
        egui::Panel::right("device_right")
            .default_size(330.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("device_right_scroll")
                    .show(ui, |ui| self.queue_section(ui));
            });
    }

    fn queue_section(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.label(egui::RichText::new(Msg::QueueTitle.tr(lang)).strong());
        if self.device.queue.is_empty() {
            ui.label(Msg::QueueEmpty.tr(lang));
        }
        // a job never starts by itself: the question below is the only way
        if let Some(q) = self.device.question(lang) {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(q).color(col(self.look.accent)));
                if ui.button(Msg::Confirm.tr(lang)).clicked() {
                    self.device.confirm_start(self.server.device_client());
                }
                if ui.button(Msg::CancelAction.tr(lang)).clicked() {
                    self.device.cancel_ask();
                }
            });
        }
        let jobs = self.device.queue.clone();
        for (i, j) in jobs.iter().enumerate() {
            ui.push_id(i, |ui| {
                self.draw_bar(ui, &device::job_bar(j));
                ui.horizontal(|ui| {
                    if j.waiting() && ui.button(Msg::JobStart.tr(lang)).clicked() {
                        self.device.ask_start(&j.id);
                    }
                    if j.state == "running" && ui.button(Msg::JobPause.tr(lang)).clicked() {
                        self.device.pause(self.server.device_client(), &j.id);
                    }
                    if j.state == "paused" && ui.button(Msg::JobResume.tr(lang)).clicked() {
                        self.device.resume(self.server.device_client(), &j.id);
                    }
                    if !j.finished() && ui.button(Msg::JobCancel.tr(lang)).clicked() {
                        self.device.cancel(self.server.device_client(), &j.id);
                    }
                });
                ui.separator();
            });
        }
    }

    /// The printer that lives in memory, so that the tab has something to show with no server.
    fn demo_printer(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let result: Option<Result<PrinterState, String>> = self
            .state
            .printer()
            .map(|p| demo_device(p).map_err(|e| e.to_string()));
        match result {
            Some(Ok(st)) => {
                ui.label(
                    egui::RichText::new(fill(Msg::State.tr(lang), &[state_text(st.state, lang)]))
                        .strong()
                        .size(17.0),
                );
                self.section_title(ui, Msg::DeviceTemperatures);
                for b in device::state_bars(&st, lang) {
                    self.draw_bar(ui, &b);
                }
            }
            Some(Err(e)) => {
                ui.label(
                    egui::RichText::new(fill(Msg::Problem.tr(lang), &[&e]))
                        .color(col(self.look.danger)),
                );
            }
            None => {}
        }
    }

    // ---- the Project tab

    fn project_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        egui::Grid::new("project_files")
            .num_columns(3)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(Msg::SaveToLabel.tr(lang));
                ui.add(
                    egui::TextEdit::singleline(&mut self.save_text)
                        .hint_text(Msg::SavePathHint.tr(lang))
                        .desired_width(360.0),
                );
                if ui.button(Msg::Save.tr(lang)).clicked() {
                    let path = self.save_text.trim().to_string();
                    self.state.save_project(&path, false);
                }
                ui.end_row();
                ui.label(Msg::OpenFromLabel.tr(lang));
                ui.add(
                    egui::TextEdit::singleline(&mut self.project_text)
                        .hint_text(Msg::OpenPathHint.tr(lang))
                        .desired_width(360.0),
                );
                if ui.button(Msg::Open.tr(lang)).clicked() {
                    let path = self.project_text.trim().to_string();
                    self.state.load_project_info(&path);
                }
                ui.end_row();
            });
        for line in self.state.project_info_lines() {
            ui.label(line);
        }
        ui.separator();
        ui.label(egui::RichText::new(Msg::SettingsTitle.tr(lang)).strong());
        let before = (
            self.state.config.grid_mm,
            self.state.config.arrange_gap_mm,
            self.state.config.arrange_on_add,
            self.state.config.ui_hints,
        );
        egui::Grid::new("window_settings")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(Msg::ThemeName.tr(lang));
                ui.horizontal(|ui| {
                    let mut theme = self.state.config.theme;
                    ui.selectable_value(&mut theme, Theme::Dark, Msg::ThemeDark.tr(lang));
                    ui.selectable_value(&mut theme, Theme::Light, Msg::ThemeLight.tr(lang));
                    if theme != self.state.config.theme {
                        self.set_theme(theme);
                    }
                });
                ui.end_row();
                ui.label(Msg::GridSpacing.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut self.state.config.grid_mm)
                        .range(1.0..=100.0)
                        .speed(0.5),
                );
                ui.end_row();
                ui.label(Msg::ArrangeGap.tr(lang));
                ui.add(
                    egui::DragValue::new(&mut self.state.config.arrange_gap_mm)
                        .range(0.0..=100.0)
                        .speed(0.5),
                );
                ui.end_row();
            });
        ui.checkbox(
            &mut self.state.config.arrange_on_add,
            Msg::ArrangeOnAdd.tr(lang),
        );
        ui.checkbox(&mut self.state.config.ui_hints, Msg::ShowHints.tr(lang));
        let after = (
            self.state.config.grid_mm,
            self.state.config.arrange_gap_mm,
            self.state.config.arrange_on_add,
            self.state.config.ui_hints,
        );
        if before != after {
            self.keep_window_settings();
        }
        ui.separator();
        ui.label(egui::RichText::new(Msg::ShortcutsTitle.tr(lang)).strong());
        let mut chosen = self.scheme.base;
        ui.horizontal(|ui| {
            for id in SchemeId::ALL {
                ui.selectable_value(&mut chosen, id, id.msg().tr(lang));
            }
        });
        if chosen != self.scheme.base {
            self.state.config.scheme = chosen;
            self.state.config.key_changes.clear();
            self.scheme = Scheme::of(chosen);
            self.keep_lines(&[("shortcut_scheme", chosen.name().to_string())]);
        }
        for line in self.scheme.rows_text(lang) {
            ui.label(line);
        }
        ui.separator();
        // the token is never shown: to_text(false) writes it as ***; the file is read only here and scrolls inside a box of its own
        let box_height = look::scroll_box_height(ui.ctx().content_rect().height());
        egui::CollapsingHeader::new(egui::RichText::new(Msg::SettingsFileTitle.tr(lang)).strong())
            .id_salt("sec_settings_file")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings_file_scroll")
                    .max_height(box_height)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(self.state.config.to_text(false))
                                .monospace()
                                .color(col(self.look.dim)),
                        );
                    });
            });
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let lang = self.lang();
        let mut open = true;
        egui::Window::new(Msg::ShortcutsTitle.tr(lang))
            .open(&mut open)
            .show(ctx, |ui| {
                for line in self.scheme.rows_text(lang) {
                    ui.label(line);
                }
            });
        self.show_shortcuts = open;
    }

    /// The help: what each tab is for, and the shortcuts.
    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let lang = self.lang();
        let mut open = true;
        egui::Window::new(Msg::HelpTitle.tr(lang))
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                for (tab, text) in [
                    (Tab::Prepare, Msg::HintPrepare),
                    (Tab::Preview, Msg::HintPreview),
                    (Tab::Device, Msg::HintDevice),
                ] {
                    ui.label(egui::RichText::new(tab.msg().tr(lang)).strong());
                    ui.label(text.tr(lang));
                    ui.add_space(4.0);
                }
                ui.label(egui::RichText::new(Msg::ShortcutsTitle.tr(lang)).strong());
                for line in self.scheme.rows_text(lang) {
                    ui.label(line);
                }
            });
        self.show_help = open;
    }

    // ---- the picture mode

    fn shot_tick(&mut self, ctx: &egui::Context) {
        let Some(s) = self.shot.as_mut() else {
            return;
        };
        s.frame += 1;
        ctx.request_repaint();
        let due = 3 + s.request.wait;
        let finished = match s.outcome.lock() {
            Ok(slot) => slot.is_some(),
            Err(_) => true,
        };
        if finished {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if s.frame == due {
            let (output, replace) = (s.request.output.clone(), s.request.replace);
            let (outcome, repaint) = (s.outcome.clone(), ctx.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |image| {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    let result = shot::write_png(
                        &output,
                        replace,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        &pixels,
                    );
                    if let Ok(mut slot) = outcome.lock() {
                        *slot = Some(result);
                    }
                    repaint.request_repaint();
                }),
            ));
        } else if s.frame > due + shot::PATIENCE_FRAMES {
            if let Ok(mut slot) = s.outcome.lock() {
                *slot = Some(Err(
                    "the picture did not come back from the window".to_owned()
                ));
            }
        }
    }

    fn apply_visuals(&mut self, ctx: &egui::Context) {
        let theme = self.state.config.theme;
        if self.visuals_theme == Some(theme) {
            return;
        }
        self.visuals_theme = Some(theme);
        let p = self.look;
        let mut v = match theme {
            Theme::Dark => egui::Visuals::dark(),
            Theme::Light => egui::Visuals::light(),
        };
        v.panel_fill = col(p.background);
        v.window_fill = col(p.panel);
        v.override_text_color = Some(col(p.text));
        v.selection.bg_fill = alpha(col(p.accent), 90);
        v.selection.stroke = Stroke::new(1.5, col(p.accent));
        ctx.set_visuals(v);
    }
}

/// A server made in memory with two printers, for the Device tab of a picture.
fn demo_server() -> ServerUi {
    use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};
    let mut s = p3dp_server_bridge::Settings::default();
    let _ = s.set("server_address", "127.0.0.1:18200");
    let _ = s.set("server_token", TOKEN);
    match p3dp_server_bridge::ServerClient::new(s, Box::new(SimPrinterServer::new())) {
        Ok(client) => ServerUi::with_bridge(client),
        Err(_) => ServerUi::new(&p3dp_server_bridge::Settings::default()),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_visuals(&ctx);
        self.handle_keys(&ctx);
        self.shot_tick(&ctx);
        self.poll_slice(&ctx);
        // the parts as meshes are made again when one moved; what is on the bed is the model of the program
        let _ = self.studio.world_parts();
        if self.studio.take_changed() {
            self.studio.sync(&mut self.state);
        }
        self.top_bar(ui);
        self.status_bar(ui);
        let tab = self.state.tab;
        if matches!(tab, Tab::Prepare | Tab::Preview) {
            self.slice_bar(ui);
        }
        match tab {
            Tab::Prepare => self.prepare_panels(ui),
            Tab::Preview => self.preview_panels(ui),
            Tab::Device => self.device_panels(ui),
            Tab::Project => {}
        }
        // the pages of the Device and Project tabs have the ground of the 3D views and a margin, so the four tabs look like one program
        let ground = match tab {
            Tab::Prepare | Tab::Preview => egui::Frame::NONE,
            Tab::Device | Tab::Project => egui::Frame::NONE
                .fill(col(self.look.panel))
                .inner_margin(egui::Margin::same(12)),
        };
        egui::CentralPanel::default()
            .frame(ground)
            .show(ui, |ui| match tab {
                Tab::Prepare => self.scene_view(ui),
                Tab::Preview => self.preview_centre(ui),
                Tab::Device => self.device_centre(ui),
                Tab::Project => {
                    egui::ScrollArea::vertical()
                        .id_salt("project")
                        .show(ui, |ui| self.project_tab(ui));
                }
            });
        self.shortcuts_window(&ctx);
        self.help_window(&ctx);
    }
}

/// The fonts of the system that draw Japanese and Chinese are added to the ones of egui, which have no such letters.
fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/meiryo.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), Arc::new(FontData::from_owned(bytes)));
            for family in [FontFamily::Monospace, FontFamily::Proportional] {
                fonts.families.entry(family).or_default().push("cjk".into());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

/// Opens the window with the arguments of the command line. With `--shot` it takes the picture and closes by itself; the answer is the reason when the picture could not be written.
pub fn run(config: AppConfig, lang: Lang, args: &WindowArgs) -> Result<(), String> {
    shot::check_request(args)?;
    let mut state = AppState::new(config, lang)
        .map_err(|e| e.to_string())?
        .with_limit_overrides();
    if let Some(id) = &args.printer {
        if !state.select_printer(id) {
            return Err(format!("{id}: no such printer"));
        }
    }
    if let Some(model) = &args.model {
        if !state.load_model_file(&model.to_string_lossy()) {
            return Err(state.status.text(lang));
        }
    }
    if let Some(tab) = args.window {
        state.tab = tab;
    }
    let outcome: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
    let shot_state = args.shot.clone().map(|request| ShotState {
        request,
        frame: 0,
        outcome: outcome.clone(),
    });
    let size = args.shot.as_ref().map_or((1360, 860), |r| r.size);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([size.0 as f32, size.1 as f32])
        .with_min_inner_size([900.0, 560.0])
        .with_title("EH3D-PHOENIX-3DP");
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let start = args.clone();
    eframe::run_native(
        "EH3D-PHOENIX-3DP",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            let mut app = App::new(state, &start, shot_state);
            if start.slice {
                app.slice_blocking();
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())?;
    if args.shot.is_some() {
        let taken = outcome.lock().map_err(|e| e.to_string())?.take();
        return match taken {
            Some(result) => result,
            None => Err("the window closed before the picture was taken".to_owned()),
        };
    }
    Ok(())
}
