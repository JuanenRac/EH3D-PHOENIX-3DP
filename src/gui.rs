//! The window (built with the `gui` feature): four tabs (Prepare, Preview, Device, Project) that draw what the modules without a window decide. The lists and their filters are
//! `lists`, the plan and the slider of layers are `preview`, the printers of the server and the queue are `device`, the settings of the server and the login are `server_ui` (the
//! panel and the badge of `eh3d-view`), the shortcuts are `keys`, the look is `look` and the picture mode is `shot`. This file only draws and passes on what the person does.
//! It has not been built or looked at on a screen yet: the part that can be checked without one is checked in those modules.

use crate::app_state::{demo_device, AppState, Tab};
use crate::config::AppConfig;
use crate::device::{DevicePanel, Done};
use crate::i18n::{fill, state_text, Lang, Msg};
use crate::keys::{Action, Chord, Scheme, SchemeId};
use crate::lists::{FilamentFilter, PrinterFilter};
use crate::look::{self, Layout, Palette};
use crate::preview::PreviewState;
use crate::server_ui::{self, ServerUi};
use crate::shot::{self, ShotRequest, WindowArgs};
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Pos2, Stroke};
use eh3d_view::server_gui;
use p3dp_link::PrinterState;
use p3dp_profiles::filament::{FilamentCatalog, Material, Nozzle};
use p3dp_profiles::Technology;
use p3dp_slicer::{Adhesion, InfillPattern, Seam, Supports};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn col(c: look::Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn col_f(c: [f32; 3]) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
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
    device: DevicePanel,
    device_read: bool,
    server: ServerUi,
    scheme: Scheme,
    show_shortcuts: bool,
    path_text: String,
    save_text: String,
    project_text: String,
    shot: Option<ShotState>,
    visuals_set: bool,
}

impl App {
    fn new(state: AppState, args: &WindowArgs, shot: Option<ShotState>) -> App {
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
        App {
            look: Palette::phoenix(),
            config_path: args.config.clone(),
            filaments: FilamentCatalog::builtin().unwrap_or(FilamentCatalog {
                profiles: Vec::new(),
            }),
            printer_filter: PrinterFilter::default(),
            filament_filter: FilamentFilter::default(),
            filament_id: None,
            nozzle_mm: 0.4,
            preview: PreviewState::default(),
            device: DevicePanel::with_policy(state.config.queue_policy()),
            device_read: false,
            server,
            scheme,
            show_shortcuts: false,
            path_text: String::new(),
            save_text: "project.3mf".to_string(),
            project_text: String::new(),
            shot,
            visuals_set: false,
            state,
        }
    }

    fn lang(&self) -> Lang {
        self.state.lang
    }

    // ---- what the person does

    fn open_model(&mut self) {
        let path = self.path_text.trim().to_string();
        if self.state.load_model_file(&path) {
            self.preview.forget();
        }
    }

    fn slice(&mut self) {
        let Some(mesh) = self.state.model.clone() else {
            return;
        };
        let Some(printer) = self.state.printer().cloned() else {
            return;
        };
        let nozzle = Nozzle {
            diameter_mm: self.nozzle_mm,
            hardened: false,
        };
        let filament = self
            .filament_id
            .as_deref()
            .and_then(|id| self.filaments.find(id))
            .cloned();
        let ok = self
            .preview
            .slice(&mesh, &printer, filament.as_ref().map(|f| (f, nozzle)));
        if ok {
            self.state.tab = Tab::Preview;
        }
    }

    fn refresh_device(&mut self) {
        self.device.refresh(self.server.device_client());
        self.device_read = true;
    }

    fn emergency_stop(&mut self) {
        self.device.emergency_stop(self.server.device_client());
    }

    /// Keeps lines in the settings file the person gave, if one was given.
    fn keep_lines(&mut self, lines: &[(&str, String)]) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let merged = server_ui::merge_lines(&text, lines);
        if let Err(e) = std::fs::write(&path, merged) {
            self.state.status =
                crate::app_state::Status::Problem(format!("{}: {e}", path.display()));
        }
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
            Action::ResetView | Action::FitView => self.preview.reset_view(),
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

    // ---- the bars

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
                    self.server.refresh();
                    let badge = self.server.panel.badge();
                    server_gui::link_badge(ui, badge, server_ui::view_lang(lang));
                    if self.server.has_server() {
                        let stop = egui::Button::new(
                            egui::RichText::new(Msg::EmergencyStop.tr(lang))
                                .strong()
                                .color(Color32::WHITE),
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

    // ---- the Prepare tab

    fn printer_list(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.horizontal(|ui| {
            ui.label(Msg::FilterSearch.tr(lang));
            ui.add(
                egui::TextEdit::singleline(&mut self.printer_filter.search).desired_width(150.0),
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, title) in &rows {
                let on = *id == self.state.printer_id;
                if ui.selectable_label(on, title).clicked() {
                    chosen = Some(id.clone());
                }
            }
        });
        if let Some(id) = chosen {
            self.state.select_printer(&id);
        }
    }

    fn filament_section(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.label(egui::RichText::new(Msg::PanelFilament.tr(lang)).strong());
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
        });
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
    }

    fn slice_options(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        let c = &mut self.preview.config;
        ui.horizontal(|ui| {
            ui.label(Msg::PanelInfill.tr(lang));
            ui.add(
                egui::DragValue::new(&mut c.infill_percent)
                    .range(0.0..=100.0)
                    .suffix(" %"),
            );
            let names = [
                (InfillPattern::Rectilinear, Msg::PatternRectilinear),
                (InfillPattern::Grid, Msg::PatternGrid),
                (InfillPattern::Gyroid, Msg::PatternGyroid),
            ];
            let shown = names
                .iter()
                .find(|(p, _)| *p == c.infill_pattern)
                .map_or("", |(_, m)| m.tr(lang));
            egui::ComboBox::from_id_salt("pattern")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (p, m) in names {
                        ui.selectable_value(&mut c.infill_pattern, p, m.tr(lang));
                    }
                });
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
    }

    fn prepare(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        for line in self.state.printer_report() {
            ui.label(egui::RichText::new(line).color(col(self.look.dim)));
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(Msg::Path.tr(lang));
            ui.add(egui::TextEdit::singleline(&mut self.path_text).desired_width(360.0));
            if ui.button(Msg::Open.tr(lang)).clicked() {
                self.open_model();
            }
        });
        for line in self.state.model_report() {
            ui.label(line);
        }
        ui.separator();
        self.filament_section(ui);
        ui.separator();
        self.slice_options(ui);
        ui.horizontal(|ui| {
            let can = self.state.model.is_some();
            if ui
                .add_enabled(can, egui::Button::new(Msg::SliceNow.tr(lang)))
                .clicked()
            {
                self.slice();
            }
            if let Some(e) = &self.preview.error {
                ui.label(
                    egui::RichText::new(fill(Msg::Problem.tr(lang), &[e]))
                        .color(col(self.look.danger)),
                );
            }
        });
        for a in &self.preview.advice {
            ui.label(egui::RichText::new(a).color(col(self.look.dim)).small());
        }
        ui.separator();
        ui.label(
            egui::RichText::new(Msg::Safety.tr(lang))
                .color(col(self.look.dim))
                .small(),
        );
    }

    // ---- the Preview tab

    fn preview_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.horizontal(|ui| {
            let can = self.state.model.is_some();
            if ui
                .add_enabled(can, egui::Button::new(Msg::SliceNow.tr(lang)))
                .clicked()
            {
                self.slice();
            }
            if let Some(s) = self.preview.summary(lang) {
                ui.label(s);
            }
            if self.preview.plan.is_some() && self.server.has_server() {
                let queue = format!("+ {}", Msg::QueueTitle.tr(lang));
                if ui.button(queue).clicked() {
                    self.send_to_queue();
                }
            }
            if ui.button(Msg::ViewReset.tr(lang)).clicked() {
                self.preview.reset_view();
            }
        });
        if let Some(e) = &self.preview.error {
            ui.label(
                egui::RichText::new(fill(Msg::Problem.tr(lang), &[e])).color(col(self.look.danger)),
            );
        }
        let count = self.preview.layer_count();
        if count > 0 {
            ui.horizontal(|ui| {
                let mut layer = self.preview.layer;
                let top = count - 1;
                if ui.add(egui::Slider::new(&mut layer, 0..=top)).changed() {
                    self.preview.set_layer(layer);
                }
                ui.label(self.preview.layer_label(lang));
                ui.checkbox(&mut self.preview.only_one, Msg::LayerOnlyOne.tr(lang));
            });
            ui.horizontal_wrapped(|ui| {
                let f = &mut self.preview.filter;
                ui.checkbox(&mut f.perimeters, Msg::KindPerimeters.tr(lang));
                ui.checkbox(&mut f.infill, Msg::KindInfill.tr(lang));
                ui.checkbox(&mut f.solid, Msg::KindSolid.tr(lang));
                ui.checkbox(&mut f.bridges, Msg::PanelBridges.tr(lang));
                ui.checkbox(&mut f.supports, Msg::PanelSupports.tr(lang));
                ui.checkbox(&mut f.adhesion, Msg::KindAdhesion.tr(lang));
                ui.checkbox(&mut self.preview.show_travel, Msg::ViewTravel.tr(lang));
            });
            if let Some(s) = self.preview.progress_label(lang) {
                ui.label(s);
            }
            ui.collapsing(Msg::PanelEstimate.tr(lang), |ui| {
                for l in self.preview.report_lines(&self.state.config, lang) {
                    ui.label(l);
                }
            });
        }
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
        let rect = response.rect;
        if response.dragged() {
            let d = response.drag_delta();
            self.preview.orbit(f64::from(d.x), f64::from(d.y));
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
                Msg::NoModel.tr(lang),
                egui::FontId::proportional(18.0),
                col(self.look.dim),
            );
            return;
        }
        let frame = self
            .preview
            .frame(f64::from(rect.width()), f64::from(rect.height()));
        for s in &frame.segments {
            let a = Pos2::new(rect.min.x + s.a[0], rect.min.y + s.a[1]);
            let b = Pos2::new(rect.min.x + s.b[0], rect.min.y + s.b[1]);
            painter.line_segment([a, b], Stroke::new(1.0, col_f(s.colour)));
        }
        let mut y = rect.min.y + 6.0;
        for (kind, mm) in self.preview.lengths() {
            let c = p3dp_viewer::layers::colour_of(kind);
            painter.text(
                Pos2::new(rect.min.x + 8.0, y),
                egui::Align2::LEFT_TOP,
                format!("{}  {:.0} mm", kind.name(), mm),
                egui::FontId::proportional(12.0),
                col_f(c),
            );
            y += 15.0;
        }
        if frame.left_out > 0 {
            painter.text(
                Pos2::new(rect.min.x + 8.0, y),
                egui::Align2::LEFT_TOP,
                format!("+{}", frame.left_out),
                egui::FontId::proportional(12.0),
                col(self.look.dim),
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

    fn device_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.label(egui::RichText::new(Msg::SimulatorOnly.tr(lang)).color(col(self.look.accent)));
        ui.label(self.state.connection_line());
        egui::CollapsingHeader::new(Msg::ServerPanelTitle.tr(lang)).show(ui, |ui| {
            let action = server_gui::show(ui, &mut self.server.panel, server_ui::view_lang(lang));
            if let Some(a) = action {
                if let Some(applied) = self.server.handle(a, &mut self.state.config.server) {
                    self.keep_lines(&applied.lines);
                    self.device.clear();
                    self.device_read = false;
                }
            }
        });
        ui.separator();
        if !self.server.has_server() {
            ui.label(Msg::NoServerSet.tr(lang));
            self.demo_printer(ui);
            return;
        }
        if !self.device_read {
            self.refresh_device();
        }
        ui.horizontal(|ui| {
            if ui.button(Msg::Refresh.tr(lang)).clicked() {
                self.refresh_device();
            }
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
        });
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
        for line in self.device.lines(lang) {
            ui.label(line);
        }
        ui.separator();
        self.queue_section(ui);
    }

    fn queue_section(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang();
        ui.label(egui::RichText::new(Msg::QueueTitle.tr(lang)).strong());
        if self.device.queue.is_empty() {
            ui.label(Msg::QueueEmpty.tr(lang));
        }
        // a job never starts by itself: the question below is the only way
        if let Some(q) = self.device.question(lang) {
            ui.horizontal(|ui| {
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
                ui.horizontal(|ui| {
                    ui.label(format!("{} - {} - {} %", j.name, j.state, j.percent));
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
                ui.label(fill(Msg::State.tr(lang), &[state_text(st.state, lang)]));
                for t in &st.temperatures {
                    ui.label(format!(
                        "{}: {:.0} / {:.0} C",
                        t.zone.name(),
                        t.actual,
                        t.target
                    ));
                }
                if let Some(j) = &st.job {
                    ui.label(j.file.as_str());
                    ui.label(fill(
                        Msg::Progress.tr(lang),
                        &[&format!("{:.0}", j.progress_pct)],
                    ));
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
        ui.horizontal(|ui| {
            ui.label(Msg::Path.tr(lang));
            ui.add(egui::TextEdit::singleline(&mut self.save_text).desired_width(360.0));
            if ui.button(Msg::Save.tr(lang)).clicked() {
                let path = self.save_text.trim().to_string();
                self.state.save_project(&path, false);
            }
        });
        ui.horizontal(|ui| {
            ui.label(Msg::Path.tr(lang));
            ui.add(egui::TextEdit::singleline(&mut self.project_text).desired_width(360.0));
            if ui.button(Msg::Open.tr(lang)).clicked() {
                let path = self.project_text.trim().to_string();
                self.state.load_project_info(&path);
            }
        });
        for line in self.state.project_info_lines() {
            ui.label(line);
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
        for (action, chord) in self.scheme.rows() {
            ui.label(format!("{}: {}", action.msg().tr(lang), chord));
        }
        ui.separator();
        // the token is never shown: to_text(false) writes it as ***
        ui.label(
            egui::RichText::new(self.state.config.to_text(false))
                .monospace()
                .color(col(self.look.dim)),
        );
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
                for (action, chord) in self.scheme.rows() {
                    ui.label(format!("{}: {}", action.msg().tr(lang), chord));
                }
            });
        self.show_shortcuts = open;
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
        if self.visuals_set {
            return;
        }
        self.visuals_set = true;
        let p = self.look;
        let mut v = egui::Visuals::dark();
        v.panel_fill = col(p.background);
        v.window_fill = col(p.panel);
        v.override_text_color = Some(col(p.text));
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
        let rect = ui.max_rect();
        let layout = Layout::for_size(rect.width(), rect.height());
        self.top_bar(ui);
        self.status_bar(ui);
        if let Some(side) = layout.side {
            if self.state.tab == Tab::Prepare {
                egui::Panel::left("side")
                    .default_size(side.w)
                    .resizable(true)
                    .show(ui, |ui| self.printer_list(ui));
            }
        }
        let tab = self.state.tab;
        egui::CentralPanel::no_frame().show(ui, |ui| match tab {
            Tab::Prepare => {
                if layout.side.is_none() {
                    egui::CollapsingHeader::new(Msg::Printer.tr(self.state.lang))
                        .show(ui, |ui| self.printer_list(ui));
                }
                egui::ScrollArea::vertical()
                    .id_salt("prepare")
                    .show(ui, |ui| self.prepare(ui));
            }
            Tab::Preview => self.preview_tab(ui),
            Tab::Device => {
                egui::ScrollArea::vertical()
                    .id_salt("device")
                    .show(ui, |ui| self.device_tab(ui));
            }
            Tab::Project => {
                egui::ScrollArea::vertical()
                    .id_salt("project")
                    .show(ui, |ui| self.project_tab(ui));
            }
        });
        self.shortcuts_window(&ctx);
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
    let mut state = AppState::new(config, lang).map_err(|e| e.to_string())?;
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
    let size = args.shot.as_ref().map_or((1100, 720), |r| r.size);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([size.0 as f32, size.1 as f32])
        .with_min_inner_size([760.0, 480.0])
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
                app.slice();
                if let Some(tab) = start.window {
                    app.state.tab = tab;
                }
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
