//! What the window shows and changes, with no window: the tab, the printer chosen, the model loaded, the lines of the report and the simulated device. Everything the
//! window does goes through here, so everything it does is tested.

use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::project::Project;
use crate::view::Camera;
use p3dp_core::mesh::Issue;
use p3dp_core::Mesh;
use p3dp_core::Secret;
use p3dp_formats::project3mf::{read_project, ProjectInfo};
use p3dp_formats::read_mesh;
use p3dp_link::moonraker::MoonrakerDriver;
use p3dp_link::sim::{SimCore, SimMoonraker};
use p3dp_link::{Command, LinkError, PrinterDriver, PrinterState};
use p3dp_profiles::{Catalog, PrinterProfile, Technology};
use p3dp_server_bridge::Connection;

/// The tabs of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Prepare,
    Preview,
    Device,
    Project,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Prepare, Tab::Preview, Tab::Device, Tab::Project];

    pub fn msg(self) -> Msg {
        match self {
            Tab::Prepare => Msg::TabPrepare,
            Tab::Preview => Msg::TabPreview,
            Tab::Device => Msg::TabDevice,
            Tab::Project => Msg::TabProject,
        }
    }

    /// The name of the tab in the arguments of the command line.
    pub fn key(self) -> &'static str {
        match self {
            Tab::Prepare => "prepare",
            Tab::Preview => "preview",
            Tab::Device => "device",
            Tab::Project => "project",
        }
    }

    /// The tab for a name: the key (`prepare`), or the name of the tab in any of the eight languages (`preparar`, `vista previa`, `dispositivo`...), in capitals or not and with a
    /// hyphen or an underscore for a space.
    pub fn from_key(text: &str) -> Option<Tab> {
        let norm = |s: &str| {
            s.trim()
                .to_lowercase()
                .replace(['-', '_'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let t = norm(text);
        if t.is_empty() {
            return None;
        }
        Tab::ALL
            .into_iter()
            .find(|tab| tab.key() == t || Lang::ALL.iter().any(|l| norm(tab.msg().tr(*l)) == t))
    }

    /// The next tab, going round.
    pub fn next(self) -> Tab {
        let i = Tab::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Tab::ALL[(i + 1) % Tab::ALL.len()]
    }

    pub fn previous(self) -> Tab {
        let i = Tab::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Tab::ALL[(i + Tab::ALL.len() - 1) % Tab::ALL.len()]
    }
}

/// The last thing that happened, for the bar at the bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    None,
    Loaded(String),
    Saved(String),
    /// The G-code was written to this file.
    Exported(String),
    Problem(String),
}

impl Status {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Status::None => String::new(),
            Status::Loaded(n) => fill(Msg::Loaded.tr(lang), &[n]),
            Status::Saved(n) => fill(Msg::Saved.tr(lang), &[n]),
            Status::Exported(n) => fill(Msg::Exported.tr(lang), &[n]),
            Status::Problem(p) => fill(Msg::Problem.tr(lang), &[p]),
        }
    }

    pub fn is_problem(&self) -> bool {
        matches!(self, Status::Problem(_))
    }
}

/// The state of the program.
pub struct AppState {
    pub tab: Tab,
    pub lang: Lang,
    pub config: AppConfig,
    pub catalog: Catalog,
    pub printer_id: String,
    pub model: Option<Mesh>,
    pub model_name: String,
    pub status: Status,
    pub camera: Camera,
    /// What a project of another program says of itself (read only), with the name of the file.
    pub project_info: Option<(String, ProjectInfo)>,
}

impl AppState {
    /// The state to start with: the catalog of the program, the printer of the settings if it exists, the first of the catalog if not.
    pub fn new(config: AppConfig, lang: Lang) -> Result<AppState, String> {
        let catalog = Catalog::builtin().map_err(|e| e.to_string())?;
        let printer_id = if catalog.find(&config.default_printer).is_some() {
            config.default_printer.clone()
        } else {
            catalog
                .profiles
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default()
        };
        Ok(AppState {
            tab: Tab::Prepare,
            lang,
            config,
            catalog,
            printer_id,
            model: None,
            model_name: String::new(),
            status: Status::None,
            camera: Camera::default(),
            project_info: None,
        })
    }

    pub fn printer(&self) -> Option<&PrinterProfile> {
        self.catalog.find(&self.printer_id)
    }

    /// Chooses a printer by its id; an id that is not in the catalog is refused and changes nothing.
    pub fn select_printer(&mut self, id: &str) -> bool {
        if self.catalog.find(id).is_some() {
            self.printer_id = id.to_ascii_lowercase();
            true
        } else {
            self.status = Status::Problem(format!("{id}: no such printer"));
            false
        }
    }

    /// Reads a model from bytes and checks it, and changes nothing. The answer for a model that cannot be used is the reason.
    pub fn read_model(&self, bytes: &[u8], name: &str) -> Result<Mesh, String> {
        let mesh = read_mesh(bytes, Some(name), &self.config.limits).map_err(|e| e.to_string())?;
        let issues = mesh.validate(&self.config.limits);
        if let Some(e) = issues.errors().next() {
            return Err(e.to_string());
        }
        Ok(mesh)
    }

    /// Reads a model from a file (the size is checked before the file is read) and returns it with the name of the file; nothing is changed.
    pub fn read_model_file(&self, path: &str) -> Result<(String, Mesh), String> {
        let limit = self.config.limits.max_file_bytes;
        let size = std::fs::metadata(path)
            .map_err(|e| format!("{path}: {e}"))?
            .len();
        if size > limit {
            return Err(format!(
                "{path}: the file is {size} bytes, more than the limit of {limit}"
            ));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        let mesh = self.read_model(&bytes, &name)?;
        Ok((name, mesh))
    }

    /// Loads a model from bytes. A model that cannot be read leaves the one that was loaded where it was and says why.
    pub fn load_model(&mut self, bytes: &[u8], name: &str) -> bool {
        match self.read_model(bytes, name) {
            Ok(mesh) => {
                self.model = Some(mesh);
                self.model_name = name.to_string();
                self.status = Status::Loaded(name.to_string());
                true
            }
            Err(e) => {
                self.status = Status::Problem(e);
                false
            }
        }
    }

    /// Loads a model from a file. The size is checked before the file is read.
    pub fn load_model_file(&mut self, path: &str) -> bool {
        let limit = self.config.limits.max_file_bytes;
        let size = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(e) => {
                self.status = Status::Problem(format!("{path}: {e}"));
                return false;
            }
        };
        if size > limit {
            self.status = Status::Problem(format!(
                "{path}: the file is {size} bytes, more than the limit of {limit}"
            ));
            return false;
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
                self.load_model(&bytes, &name)
            }
            Err(e) => {
                self.status = Status::Problem(format!("{path}: {e}"));
                false
            }
        }
    }

    /// Reads the objects, plates and a few settings of a 3MF project saved by another program. The size is checked before the file is read, nothing is written, and a file that
    /// cannot be read leaves what was read before where it was and says why.
    pub fn load_project_info(&mut self, path: &str) -> bool {
        let limit = self.config.limits.max_file_bytes;
        let size = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(e) => {
                self.status = Status::Problem(format!("{path}: {e}"));
                return false;
            }
        };
        if size > limit {
            self.status = Status::Problem(format!(
                "{path}: the file is {size} bytes, more than the limit of {limit}"
            ));
            return false;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = Status::Problem(format!("{path}: {e}"));
                return false;
            }
        };
        match read_project(&bytes, &self.config.limits) {
            Ok(info) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
                self.status = Status::Loaded(name.clone());
                self.project_info = Some((name, info));
                true
            }
            Err(e) => {
                self.status = Status::Problem(format!("{path}: {e}"));
                false
            }
        }
    }

    /// The lines about the project that was read: who made it, how many objects and plates, each plate with the time and the weight the other program predicted, and the settings.
    pub fn project_info_lines(&self) -> Vec<String> {
        let Some((name, info)) = &self.project_info else {
            return Vec::new();
        };
        let lang = self.lang;
        let made = if info.application.is_empty() {
            info.producer.name().to_string()
        } else {
            info.application.clone()
        };
        let mut out = vec![
            name.clone(),
            fill(Msg::ProjectMadeBy.tr(lang), &[&made]),
            fill(
                Msg::ProjectObjects.tr(lang),
                &[&info.objects.len().to_string()],
            ),
            fill(
                Msg::ProjectPlates.tr(lang),
                &[&info.plates.len().to_string()],
            ),
        ];
        for p in &info.plates {
            let mut line = format!(
                "{}: {}",
                p.id,
                if p.name.is_empty() { "-" } else { &p.name }
            );
            if let Some(s) = p.predicted_seconds {
                line.push_str(&format!(" - {}", crate::preview::format_duration(s as f64)));
            }
            if let Some(g) = p.weight_g {
                line.push_str(&format!(" - {g:.1} g"));
            }
            line.push_str(&format!(" - {}", p.object_ids.len()));
            out.push(line);
        }
        for (k, v) in &info.settings {
            out.push(format!("{k} = {v}"));
        }
        out
    }

    /// The lines that describe the model and say whether it can go on the printer.
    pub fn model_report(&self) -> Vec<String> {
        let lang = self.lang;
        let Some(m) = &self.model else {
            return vec![Msg::NoModel.tr(lang).to_string()];
        };
        let mut out = vec![fill(
            Msg::Triangles.tr(lang),
            &[&m.triangles.len().to_string()],
        )];
        out.push(fill(
            Msg::Volume.tr(lang),
            &[&format!("{:.2}", m.volume_mm3().abs() / 1000.0)],
        ));
        if let Some(s) = m.size_mm() {
            out.push(fill(
                Msg::Size.tr(lang),
                &[&format!("{:.1} x {:.1} x {:.1}", s[0], s[1], s[2])],
            ));
        }
        if let Some(p) = self.printer() {
            let msg = if p.fits(m) { Msg::FitsYes } else { Msg::FitsNo };
            out.push(msg.tr(lang).to_string());
        }
        for i in m.validate(&self.config.limits).issues {
            match i {
                Issue::OpenEdges(n) => {
                    out.push(fill(Msg::NotWatertight.tr(lang), &[&n.to_string()]))
                }
                Issue::Degenerate(n) => out.push(fill(Msg::Degenerate.tr(lang), &[&n.to_string()])),
                Issue::Inverted => out.push(Msg::Inverted.tr(lang).to_string()),
                _ => {}
            }
        }
        out
    }

    /// The lines about the printer: its name, whether it has been tried, its limits, and the notes it needs.
    pub fn printer_report(&self) -> Vec<String> {
        self.printer()
            .map(|p| profile_report(p, self.lang))
            .unwrap_or_default()
    }
}

/// The lines about a profile: its name, whether it has been tried, its limits, and the notes it needs.
pub fn profile_report(p: &PrinterProfile, lang: Lang) -> Vec<String> {
    let mut out = vec![p.title()];
    let tried = if p.verified {
        Msg::VerifiedYes
    } else {
        Msg::VerifiedNo
    };
    out.push(tried.tr(lang).to_string());
    match p.limits {
        Some(l) => {
            out.push(Msg::Limits.tr(lang).to_string());
            out.push(
                match p.limit_source {
                    p3dp_profiles::LimitSource::Maker => Msg::LimitsFromMaker,
                    p3dp_profiles::LimitSource::Conservative => Msg::LimitsConservative,
                    p3dp_profiles::LimitSource::Owner => Msg::LimitsOwner,
                }
                .tr(lang)
                .to_string(),
            );
            out.push(format!("{}: {} C", Msg::Nozzle.tr(lang), l.nozzle_c));
            out.push(format!("{}: {} C", Msg::Bed.tr(lang), l.bed_c));
            out.push(match l.chamber_c {
                Some(c) => format!("{}: {c} C", Msg::Chamber.tr(lang)),
                None => format!("{}: {}", Msg::Chamber.tr(lang), Msg::NoChamber.tr(lang)),
            });
        }
        None => out.push(format!("{} (mSLA)", p.dialect)),
    }
    if p.has(p3dp_profiles::Connectivity::BambuLan) {
        out.push(Msg::BambuExperimental.tr(lang).to_string());
    }
    out
}

impl AppState {
    /// How the program would reach the printers, in words.
    pub fn connection_line(&self) -> String {
        match self.config.server.connection() {
            Connection::ViaServer(a) => fill(Msg::ModeServer.tr(self.lang), &[&a]),
            Connection::Direct => Msg::ModeStandalone.tr(self.lang).to_string(),
            Connection::Discover | Connection::NotConfigured => {
                Msg::ModeNotConfigured.tr(self.lang).to_string()
            }
        }
    }

    /// The project of what is loaded, if a model is.
    pub fn project(&self) -> Option<Project> {
        self.model
            .as_ref()
            .map(|m| Project::new(&self.model_name, &self.printer_id, m.clone()))
    }

    /// Saves the project as a 3MF package. An existing file is not replaced unless `replace` is true.
    pub fn save_project(&mut self, path: &str, replace: bool) -> bool {
        let Some(p) = self.project() else {
            self.status = Status::Problem(Msg::NoModel.tr(self.lang).to_string());
            return false;
        };
        if !replace && std::path::Path::new(path).exists() {
            self.status = Status::Problem(format!("{path}: the file exists"));
            return false;
        }
        match p
            .to_3mf()
            .map_err(|e| e.to_string())
            .and_then(|b| std::fs::write(path, b).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                self.status = Status::Saved(path.to_string());
                true
            }
            Err(e) => {
                self.status = Status::Problem(e);
                false
            }
        }
    }
}

/// A printer that exists only in memory, running a job, so that the Device tab has something to show. It is a simulator: the temperatures and the progress are made up
/// by the simulator of Moonraker, and nothing is connected. A resin printer has no driver yet.
pub fn demo_device(profile: &PrinterProfile) -> Result<PrinterState, LinkError> {
    if profile.technology != Technology::Fdm {
        return Err(LinkError::Capability(
            "be driven (resin printers have no driver yet)",
        ));
    }
    let limits = profile.limits.ok_or(LinkError::Capability(
        "be driven without temperature limits",
    ))?;
    let core = SimCore::shared();
    core.borrow_mut().total_s = 3600.0;
    let mut d = MoonrakerDriver::new(
        SimMoonraker::new(core.clone()),
        Secret::default(),
        limits,
        false,
    );
    d.upload("demo.gcode", b"G28\n", true)?;
    d.command(&Command::SetTemp {
        zone: p3dp_core::machine::Zone::Nozzle,
        target_c: limits.nozzle_c.min(215.0),
    })?;
    d.command(&Command::SetTemp {
        zone: p3dp_core::machine::Zone::Bed,
        target_c: limits.bed_c.min(60.0),
    })?;
    core.borrow_mut().advance(900.0);
    d.status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::machine::Zone;
    use p3dp_core::mesh::cube;
    use p3dp_formats::{write_mesh, Format};

    fn state() -> AppState {
        AppState::new(AppConfig::default(), Lang::En).unwrap()
    }

    fn temp_path(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("p3dp_test_{}_{name}", std::process::id()));
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn tabs_go_round() {
        let mut t = Tab::Prepare;
        for _ in 0..4 {
            t = t.next();
        }
        assert_eq!(t, Tab::Prepare);
        assert_eq!(Tab::Prepare.previous(), Tab::Project);
        assert_eq!(Tab::Device.msg(), Msg::TabDevice);
    }

    #[test]
    fn a_tab_is_found_by_its_key_or_by_its_name_in_any_language() {
        assert_eq!(Tab::from_key("prepare"), Some(Tab::Prepare));
        assert_eq!(Tab::from_key(" Preparar "), Some(Tab::Prepare));
        assert_eq!(Tab::from_key("vista previa"), Some(Tab::Preview));
        assert_eq!(Tab::from_key("Vista-Previa"), Some(Tab::Preview));
        assert_eq!(Tab::from_key("pré-visualização"), Some(Tab::Preview));
        assert_eq!(Tab::from_key("dispositivo"), Some(Tab::Device));
        assert_eq!(Tab::from_key("GERÄT"), Some(Tab::Device));
        assert_eq!(Tab::from_key("プロジェクト"), Some(Tab::Project));
        assert_eq!(Tab::from_key(""), None);
        assert_eq!(Tab::from_key("   "), None);
        assert_eq!(Tab::from_key("settings"), None);
        // every name of every tab in every language finds its own tab
        for tab in Tab::ALL {
            for l in Lang::ALL {
                assert_eq!(Tab::from_key(tab.msg().tr(l)), Some(tab), "{}", l.code());
            }
        }
    }

    #[test]
    fn it_starts_with_a_printer_of_the_catalog() {
        let s = state();
        assert!(s.printer().is_some());
        let cfg = AppConfig::from_text("default_printer = prusa-mk4s\n").unwrap();
        assert_eq!(
            AppState::new(cfg, Lang::Es).unwrap().printer_id,
            "prusa-mk4s"
        );
        let cfg = AppConfig::from_text("default_printer = not-a-printer\n").unwrap();
        assert!(AppState::new(cfg, Lang::En).unwrap().printer().is_some());
    }

    #[test]
    fn choosing_a_printer() {
        let mut s = state();
        assert!(s.select_printer("BAMBU-LAB-P1S"));
        assert_eq!(s.printer_id, "bambu-lab-p1s");
        assert!(!s.select_printer("ghost"));
        assert_eq!(s.printer_id, "bambu-lab-p1s");
        assert!(s.status.is_problem());
    }

    #[test]
    fn a_model_is_loaded_reported_and_kept_when_the_next_one_fails() {
        let mut s = state();
        assert_eq!(
            s.model_report(),
            vec![Msg::NoModel.tr(Lang::En).to_string()]
        );
        let bytes = write_mesh(&cube(30.0), Format::Stl, "").unwrap();
        assert!(s.load_model(&bytes, "cube.stl"));
        let r = s.model_report().join("\n");
        assert!(
            r.contains("Triangles: 12")
                && r.contains("Volume: 27.00")
                && r.contains("30.0 x 30.0 x 30.0")
                && r.contains("fits"),
            "{r}"
        );
        assert!(!s.load_model(b"garbage", "bad.stl"));
        assert_eq!(s.model_name, "cube.stl");
        assert!(s.status.is_problem());
        assert!(s.status.text(Lang::En).starts_with("Problem:"));
    }

    #[test]
    fn a_model_that_is_too_big_for_the_printer_says_so() {
        let mut s = state();
        s.select_printer("bambu-lab-a1-mini");
        let bytes = write_mesh(&cube(200.0), Format::Stl, "").unwrap();
        assert!(s.load_model(&bytes, "big.stl"));
        assert!(s.model_report().join("\n").contains("does NOT fit"));
        s.lang = Lang::Es;
        assert!(s.model_report().join("\n").contains("NO cabe"));
    }

    #[test]
    fn warnings_of_the_mesh_are_listed() {
        let mut s = state();
        let mut c = cube(10.0);
        c.triangles.pop();
        let bytes = write_mesh(&c, Format::Obj, "").unwrap();
        assert!(s.load_model(&bytes, "open.obj"));
        assert!(s.model_report().join("\n").contains("not watertight"));
    }

    #[test]
    fn a_model_can_be_read_without_being_loaded() {
        let s = state();
        let bytes = write_mesh(&cube(8.0), Format::Stl, "").unwrap();
        let m = s.read_model(&bytes, "c.stl").unwrap();
        assert_eq!(m.triangles.len(), 12);
        assert!(s.model.is_none(), "nothing was loaded");
        assert!(s.read_model(b"garbage", "bad.stl").is_err());
        let path = temp_path("read.stl");
        std::fs::write(&path, &bytes).unwrap();
        let (name, mesh) = s.read_model_file(&path).unwrap();
        assert!(name.starts_with("p3dp_test_") && mesh.triangles.len() == 12);
        assert!(s.read_model_file(&temp_path("nowhere.stl")).is_err());
        let mut small = state();
        small.config.limits.max_file_bytes = 10;
        assert!(small
            .read_model_file(&path)
            .unwrap_err()
            .contains("more than the limit"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn files_are_checked_before_they_are_read() {
        let mut s = state();
        assert!(!s.load_model_file(&temp_path("does_not_exist.stl")));
        let path = temp_path("small.stl");
        std::fs::write(&path, write_mesh(&cube(5.0), Format::Stl, "").unwrap()).unwrap();
        assert!(s.load_model_file(&path));
        assert!(s.model_name.starts_with("p3dp_test_"));
        s.config.limits.max_file_bytes = 100;
        assert!(!s.load_model_file(&path));
        assert!(s.status.text(Lang::En).contains("more than the limit"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_project_is_saved_and_read_back_and_nothing_is_replaced_by_accident() {
        let mut s = state();
        assert!(
            !s.save_project(&temp_path("none.3mf"), false),
            "nothing loaded"
        );
        let bytes = write_mesh(&cube(12.0), Format::ThreeMf, "x").unwrap();
        assert!(s.load_model(&bytes, "twelve.3mf"));
        s.select_printer("prusa-mk4s");
        let path = temp_path("proj.3mf");
        let _ = std::fs::remove_file(&path);
        assert!(s.save_project(&path, false));
        assert!(!s.save_project(&path, false), "it exists");
        assert!(s.save_project(&path, true));
        let back = Project::from_3mf(&std::fs::read(&path).unwrap(), &s.config.limits).unwrap();
        assert_eq!(
            (back.name.as_str(), back.printer_id.as_str()),
            ("twelve.3mf", "prusa-mk4s")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_project_of_another_program_is_read_for_what_it_says_of_itself_and_nothing_is_written() {
        let mut s = state();
        assert!(s.project_info_lines().is_empty());
        let bytes = write_mesh(&cube(12.0), Format::ThreeMf, "mine").unwrap();
        let path = temp_path("info.3mf");
        std::fs::write(&path, &bytes).unwrap();
        assert!(s.load_project_info(&path));
        let lines = s.project_info_lines();
        assert!(lines[0].starts_with("p3dp_test_"), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Made by:")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Objects:")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Plates:")), "{lines:?}");
        s.lang = Lang::Pt;
        assert!(s
            .project_info_lines()
            .iter()
            .any(|l| l.starts_with("Criado com:")));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "the file is not touched"
        );
        // what cannot be read leaves what was read and says why
        let before = s.project_info.clone();
        std::fs::write(&path, b"not a package").unwrap();
        assert!(!s.load_project_info(&path));
        assert!(s.status.is_problem());
        assert_eq!(s.project_info, before);
        assert!(!s.load_project_info(&temp_path("none.3mf")));
        s.config.limits.max_file_bytes = 3;
        assert!(!s.load_project_info(&path));
        assert!(s.status.text(Lang::En).contains("more than the limit"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_report_of_a_printer_has_its_limits_and_its_notes() {
        let mut s = state();
        s.select_printer("bambu-lab-a1");
        let r = s.printer_report().join("\n");
        assert!(
            r.contains("Nozzle: 300 C")
                && r.contains("Bed: 100 C")
                && r.contains("no heated chamber")
                && r.contains("experimental")
                && r.contains("not been tried"),
            "{r}"
        );
        s.select_printer("qidi-plus4");
        assert!(s.printer_report().join("\n").contains("Chamber: 65 C"));
        s.select_printer("elegoo-mars-4");
        assert!(s.printer_report().join("\n").contains("mSLA"));
    }

    #[test]
    fn the_connection_is_told_in_words() {
        let mut s = state();
        assert!(s.connection_line().contains("nothing is sent"));
        s.config
            .server
            .set("server_address", "10.0.0.5:18200")
            .unwrap();
        assert!(s.connection_line().contains("10.0.0.5:18200"));
        s.config.server.set("server_address", "").unwrap();
        s.config.server.set("direct_mode", "on").unwrap();
        assert!(s.connection_line().contains("Stand-alone"));
    }

    #[test]
    fn the_demo_device_is_a_simulator_that_keeps_to_the_limits() {
        let s = state();
        for p in s.catalog.technology(Technology::Fdm) {
            let st = demo_device(p).unwrap();
            assert!(st.state.is_active(), "{}", p.id);
            let n = st.temp(Zone::Nozzle).unwrap();
            assert!(
                n.target <= p.limits.unwrap().nozzle_c && n.actual <= n.target + 1e-9,
                "{}",
                p.id
            );
            assert!(st.job.as_ref().unwrap().progress_pct > 20.0);
        }
        let resin = s.catalog.find("elegoo-mars-4").unwrap();
        assert!(demo_device(resin).is_err());
    }

    #[test]
    fn what_the_window_says_of_a_file_it_wrote_names_the_file_in_every_language() {
        for l in Lang::ALL {
            for status in [
                Status::Loaded("a.stl".to_string()),
                Status::Saved("a.3mf".to_string()),
                Status::Exported("a.gcode".to_string()),
            ] {
                let text = status.text(l);
                let name = match &status {
                    Status::Loaded(n) | Status::Saved(n) | Status::Exported(n) => n,
                    _ => unreachable!(),
                };
                assert!(
                    text.contains(name.as_str()) && !text.contains("{}"),
                    "{text} in {}",
                    l.code()
                );
                assert!(!status.is_problem());
            }
        }
        assert!(Status::Problem("x".to_string()).is_problem());
        assert_eq!(Status::None.text(Lang::En), "");
    }
}
