//! The window (built with the `gui` feature): it draws the state of `app_state::AppState` and passes on what the person does. Everything that can be decided without a
//! screen is decided there, in code that is tested. This file has not been built or looked at on a screen yet.

use crate::app_state::{demo_device, AppState, Tab};
use crate::config::AppConfig;
use crate::i18n::{fill, state_text, Lang, Msg};
use crate::view;
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Pos2, Stroke};
use p3dp_link::PrinterState;
use std::sync::Arc;

const ACCENT: Color32 = Color32::from_rgb(0xff, 0x6a, 0x3d);
const DIM: Color32 = Color32::from_rgb(0x9c, 0x8d, 0x7a);
const DANGER: Color32 = Color32::from_rgb(0xff, 0x54, 0x70);
const GOOD: Color32 = Color32::from_rgb(0x9b, 0xe3, 0x4a);
const EDGE: Color32 = Color32::from_rgb(0x4d, 0xd0, 0xe1);
const PLATE: Color32 = Color32::from_rgb(0x4a, 0x55, 0x66);

struct App {
    state: AppState,
    path_text: String,
    save_text: String,
    device: Option<(String, Result<PrinterState, String>)>,
}

impl App {
    fn new(state: AppState) -> App {
        App {
            state,
            path_text: String::new(),
            save_text: "project.3mf".to_string(),
            device: None,
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("EH3D-PHOENIX-3DP")
                        .strong()
                        .size(18.0)
                        .color(ACCENT),
                );
                ui.separator();
                for tab in Tab::ALL {
                    let label = tab.msg().tr(self.state.lang);
                    if ui.selectable_label(self.state.tab == tab, label).clicked() {
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
                    ui.label(egui::RichText::new(Msg::Language.tr(self.state.lang)).color(DIM));
                });
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let text = self.state.status.text(self.state.lang);
                let color = if self.state.status.is_problem() {
                    DANGER
                } else {
                    GOOD
                };
                ui.label(egui::RichText::new(text).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(Msg::SimulatorOnly.tr(self.state.lang)).color(DIM),
                    );
                });
            });
        });
    }

    fn printer_combo(&mut self, ui: &mut egui::Ui) {
        let mut chosen = self.state.printer_id.clone();
        let shown = self.state.printer().map_or_else(String::new, |p| p.title());
        ui.horizontal(|ui| {
            ui.label(Msg::Printer.tr(self.state.lang));
            egui::ComboBox::from_id_salt("printer")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for p in &self.state.catalog.profiles {
                        ui.selectable_value(&mut chosen, p.id.clone(), p.title());
                    }
                });
        });
        if chosen != self.state.printer_id {
            self.state.select_printer(&chosen);
        }
    }

    fn prepare(&mut self, ui: &mut egui::Ui) {
        self.printer_combo(ui);
        for line in self.state.printer_report() {
            ui.label(egui::RichText::new(line).color(DIM));
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(Msg::Path.tr(self.state.lang));
            ui.add(egui::TextEdit::singleline(&mut self.path_text).desired_width(360.0));
            if ui.button(Msg::Open.tr(self.state.lang)).clicked() {
                let path = self.path_text.trim().to_string();
                self.state.load_model_file(&path);
            }
        });
        for line in self.state.model_report() {
            ui.label(line);
        }
        ui.separator();
        ui.label(
            egui::RichText::new(Msg::Safety.tr(self.state.lang))
                .color(DIM)
                .small(),
        );
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
        let rect = response.rect;
        if response.dragged() {
            let d = response.drag_delta();
            self.state.camera = self
                .state
                .camera
                .orbit(f64::from(d.x) * 0.01, f64::from(d.y) * 0.01);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.state.camera = self.state.camera.zoomed((f64::from(scroll) * 0.002).exp());
            }
        }
        let Some(mesh) = &self.state.model else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                Msg::NoModel.tr(self.state.lang),
                egui::FontId::proportional(18.0),
                DIM,
            );
            return;
        };
        let at = |p: (f32, f32)| Pos2::new(rect.min.x + p.0, rect.min.y + p.1);
        if let Some(p) = self.state.printer() {
            let plate = [p.volume_mm[0], p.volume_mm[1]];
            for s in
                view::plate_outline(mesh, plate, self.state.camera, rect.width(), rect.height())
            {
                painter.line_segment([at(s[0]), at(s[1])], Stroke::new(1.0, PLATE));
            }
        }
        for s in view::edges(mesh, self.state.camera, rect.width(), rect.height()) {
            painter.line_segment([at(s[0]), at(s[1])], Stroke::new(1.0, EDGE));
        }
    }

    fn device(&mut self, ui: &mut egui::Ui) {
        let lang = self.state.lang;
        ui.label(egui::RichText::new(Msg::SimulatorOnly.tr(lang)).color(ACCENT));
        ui.label(self.state.connection_line());
        let id = self.state.printer_id.clone();
        let stale = self.device.as_ref().is_none_or(|(for_id, _)| *for_id != id);
        if stale {
            let result = match self.state.printer() {
                Some(p) => demo_device(p).map_err(|e| e.to_string()),
                None => Err(Msg::NoModel.tr(lang).to_string()),
            };
            self.device = Some((id, result));
        }
        match self.device.as_ref().map(|(_, r)| r) {
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
                ui.label(egui::RichText::new(fill(Msg::Problem.tr(lang), &[e])).color(DANGER));
            }
            None => {}
        }
    }

    fn project(&mut self, ui: &mut egui::Ui) {
        let lang = self.state.lang;
        ui.horizontal(|ui| {
            ui.label(Msg::Path.tr(lang));
            ui.add(egui::TextEdit::singleline(&mut self.save_text).desired_width(360.0));
            if ui.button(Msg::Save.tr(lang)).clicked() {
                let path = self.save_text.trim().to_string();
                self.state.save_project(&path, false);
            }
        });
        ui.separator();
        // the token is never shown: to_text(false) writes it as ***
        ui.label(
            egui::RichText::new(self.state.config.to_text(false))
                .monospace()
                .color(DIM),
        );
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_bar(ui);
        self.status_bar(ui);
        let tab = self.state.tab;
        egui::CentralPanel::no_frame().show(ui, |ui| match tab {
            Tab::Prepare => self.prepare(ui),
            Tab::Preview => self.preview(ui),
            Tab::Device => self.device(ui),
            Tab::Project => self.project(ui),
        });
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

/// Opens the window.
pub fn run(config: AppConfig, lang: Lang) -> eframe::Result {
    let state = match AppState::new(config, lang) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return Ok(());
        }
    };
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([760.0, 480.0])
        .with_title("EH3D-PHOENIX-3DP");
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "EH3D-PHOENIX-3DP",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(state)))
        }),
    )
}
