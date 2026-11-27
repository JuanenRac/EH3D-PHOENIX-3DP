//! The entries of the menu File for a project file of the suite (`.eh3d`): open it, save it and save it under another name without losing a branch, import a model or a program keeping the
//! original, take a file of another branch and extract a branch.
//!
//! The work is done by the functions of the command `project` (`eh3dfile`); here it is decided what the window holds, what goes into the branch and what the person has to agree to
//! first. Only the branch `3dp` is written. A project that was not opened here, a branch that changed since it was read and a signature that a write removes are asked about before
//! anything is written. The G-code of a project is kept in it and not loaded into the window, and nothing here sends anything to a printer.

use super::*;
use crate::eh3dfile::{self, Contents, Held, Identity, Opened, Part, BRANCH};
use crate::suitecore::{self, name_of, ProjectView};
use crate::suitetext::{Said, S};

/// Something that writes a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    Save(PathBuf),
    Import(PathBuf),
    Take(String, String),
}

/// A write that waits for the person to agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    action: Action,
    pub warnings: Vec<Said>,
}

/// How an entry ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Done,
    /// The window has no project yet: a name has to be asked for.
    NeedsName,
    /// Nothing was written yet: the person has to agree.
    Asked,
    /// It was refused; the window says why.
    Refused,
}

/// The path that is being typed for an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    Open,
    SaveAs,
    Import,
    Extract,
}

/// The list of the project that is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SuiteList {
    Branches,
    Take,
    Extract,
}

/// What the window keeps about its project file of the suite.
#[derive(Default)]
pub(super) struct SuiteState {
    /// The project the window works on.
    pub file: Option<PathBuf>,
    view: Option<ProjectView>,
    notes: Vec<Said>,
    /// Lines in the words of the program, shown with the notes.
    own_notes: Vec<String>,
    extra: Vec<String>,
    pub pending: Option<Pending>,
    /// The content hash of the branch of the program when it was read or written.
    seen: Option<String>,
    /// What the window held when the branch was read or written, to know what changed since.
    loaded: Option<Contents>,
    pub ask: Option<Ask>,
    pub ask_text: String,
    pub extract_branch: String,
    list: Option<SuiteList>,
    was_focused: bool,
}

/// Leaves out what did not change since the branch was read or written, so a part taken from another branch stays with its reference and the G-code of the project stays with the work.
fn drop_untouched(contents: &mut Contents, loaded: &Contents) {
    if contents.parts == loaded.parts {
        contents.parts = None;
    }
    if contents.settings == loaded.settings {
        contents.settings = None;
    }
    if contents.printer == loaded.printer {
        contents.printer = None;
    }
    if contents.filament == loaded.filament {
        contents.filament = None;
    }
}

impl App {
    fn suite_code(&self) -> &'static str {
        self.lang().code()
    }

    /// Says a message, in the language of the window.
    fn suite_tell(&mut self, said: &Said) {
        self.note = Some(said.text(self.suite_code()));
    }

    fn suite_fail(&mut self, why: String) -> Step {
        self.state.status = Status::Problem(why);
        Step::Refused
    }

    fn suite_refuse(&mut self, word: S) -> Step {
        let line = Said::plain(word).text(self.suite_code());
        self.suite_fail(line)
    }

    /// A refusal of the shared module; the one of a file that another program saved is said in the language of the window.
    fn suite_refusal(&mut self, why: String, target: &Path) -> Step {
        if suitecore::is_changed_refusal(&why) {
            let said = Said(S::ChangedOnDisk, vec![name_of(target)]);
            let line = said.text(self.suite_code());
            return self.suite_fail(line);
        }
        self.suite_fail(why)
    }

    /// Whether the project of the window still holds what was read; `Err` with the step of the refusal when another program saved it. Asked right before a write, because the person
    /// may have taken long to answer a question.
    fn suite_unchanged(&mut self, target: &Path) -> Result<(), Step> {
        let own = self
            .suite
            .file
            .as_deref()
            .is_some_and(|c| suitecore::same_file(c, target));
        match self.suite.seen.clone().filter(|_| own) {
            Some(seen) => {
                suitecore::check_unchanged(target, &seen).map_err(|e| self.suite_refusal(e, target))
            }
            None => Ok(()),
        }
    }

    /// Reads the project again to tell what is out of date now. What the window holds does not change.
    pub(super) fn suite_refresh(&mut self) {
        if let Some(file) = &self.suite.file {
            self.suite.view = suitecore::view(file, BRANCH).ok();
        }
    }

    /// The window forgets its project (a new bed).
    pub(super) fn suite_forget(&mut self) {
        self.suite = SuiteState::default();
    }

    /// The lines that stay in sight while the window works on a project; the flag marks the ones that ask for attention.
    pub(super) fn suite_banner(&self) -> Vec<(String, bool)> {
        let Some(file) = &self.suite.file else {
            return Vec::new();
        };
        let code = self.suite_code();
        let mut lines = suitecore::banner(file, self.suite.view.as_ref(), BRANCH, code);
        lines.extend(self.suite.notes.iter().map(|n| (n.text(code), false)));
        lines.extend(self.suite.own_notes.iter().map(|n| (n.clone(), false)));
        lines.extend(self.suite.extra.iter().map(|n| (n.clone(), true)));
        lines
    }

    /// The parts of the bed as the branch keeps them.
    fn suite_parts(&self) -> Vec<Part> {
        self.studio
            .plate
            .objects
            .iter()
            .map(|o| Part {
                name: o.name.clone(),
                mesh: o.mesh.clone(),
                pos: Some(o.pos),
                rot_deg: o.rot_deg,
                scale: o.scale,
            })
            .collect()
    }

    /// The work of the window without its G-code.
    fn suite_work(&self) -> Contents {
        Contents {
            parts: Some(self.suite_parts()),
            settings: Some(self.preview.config.clone()),
            printer: (!self.state.printer_id.is_empty()).then(|| self.state.printer_id.clone()),
            filament: self.filament_id.clone(),
            program: None,
        }
    }

    /// What the window would put into the branch, and what is to be said about what it leaves out. The G-code goes only when it is the one of the bed as it is now.
    pub(super) fn suite_contents(&self, target: &Path) -> (Contents, Vec<Said>) {
        let mut contents = self.suite_work();
        let mut notes = Vec::new();
        if let Some(gcode) = &self.preview.gcode {
            if self.studio.stale || self.studio.is_slicing() || self.preview.plan.is_none() {
                notes.push(Said::plain(S::ProgramNotCurrent));
            } else {
                let stem = target
                    .file_stem()
                    .map_or_else(|| "print".into(), |s| s.to_string_lossy().into_owned());
                contents.program = Some(Held {
                    name: format!("{stem}.gcode"),
                    bytes: gcode.clone().into_bytes(),
                });
            }
        }
        (contents, notes)
    }

    /// Whether the bed or its settings have something the project does not.
    fn suite_unsaved(&self) -> bool {
        let mut work = self.suite_work();
        match &self.suite.loaded {
            Some(loaded) => {
                drop_untouched(&mut work, loaded);
                !work.is_empty()
            }
            None => !self.studio.plate.is_empty(),
        }
    }

    /// Opens a project: its branch `3dp` becomes the bed of the window; a project without one leaves the bed empty, so that saving never puts the work of before into it by
    /// accident. A project that cannot be read changes nothing.
    pub(super) fn suite_open(&mut self, path: &Path) -> bool {
        if self.studio.is_slicing() {
            self.suite_refuse(S::Busy);
            return false;
        }
        let found = match suitecore::view(path, BRANCH) {
            Ok(found) => found,
            Err(e) => {
                self.suite_fail(e);
                return false;
            }
        };
        let has_branch = found.branch(BRANCH).is_some();
        let held = if has_branch {
            match eh3dfile::held(path, &self.state.config.limits) {
                Ok(held) => held,
                Err(e) => {
                    self.suite_fail(e);
                    return false;
                }
            }
        } else {
            Opened::default()
        };
        self.suite.notes = held
            .report
            .as_ref()
            .map(suitecore::report_lines)
            .unwrap_or_default();
        self.suite.own_notes = Vec::new();
        self.suite_load(&held);
        self.suite.extra = held
            .warnings
            .iter()
            .filter(|w| !w.starts_with("OUT OF DATE"))
            .cloned()
            .collect();
        self.suite.pending = None;
        self.suite.seen = found.own_hash.clone();
        self.suite.view = Some(found);
        self.suite.file = Some(path.to_path_buf());
        self.suite.loaded = has_branch.then(|| self.suite_work());
        self.recent_added.push(path.display().to_string());
        self.state.tab = Tab::Prepare;
        self.state.status = Status::Loaded(name_of(path));
        let said = if has_branch {
            Said(S::Opened, vec![name_of(path)])
        } else {
            Said(S::NoWork, vec![name_of(path), BRANCH.to_owned()])
        };
        self.suite_tell(&said);
        true
    }

    /// Puts what the branch holds on the bed. The limits of the person (the most layers, points and lines) stay.
    fn suite_load(&mut self, held: &Opened) {
        let lang = self.lang();
        self.studio.clear_all();
        self.preview.forget();
        self.saved_path = None;
        self.exported_path = None;
        self.sliced_with = None;
        if let Some(id) = &held.printer {
            if self.state.catalog.find(id).is_some() {
                self.state.select_printer(id);
            } else {
                self.suite
                    .own_notes
                    .push(fill(Msg::SuitePrinterUnknown.tr(lang), &[id]));
            }
        }
        if let Some(id) = &held.filament {
            self.filament_id = self.filaments.find(id).map(|f| f.id.clone());
            if self.filament_id.is_none() {
                self.suite
                    .own_notes
                    .push(fill(Msg::SuiteFilamentUnknown.tr(lang), &[id]));
            }
        }
        if let Some(settings) = &held.settings {
            let own = self.preview.config.clone();
            self.preview.config = settings.clone();
            self.preview.config.max_layers = own.max_layers;
            self.preview.config.max_points_per_layer = own.max_points_per_layer;
            self.preview.config.max_gcode_lines = own.max_gcode_lines;
            self.preview.config.max_support_samples = own.max_support_samples;
            self.process = None;
            self.suite.notes.push(Said::plain(S::LimitsKept));
        }
        let volume = self.volume();
        for part in &held.parts {
            let bed = [volume[0], volume[1]];
            let Some(id) = self.studio.plate.add(&part.name, part.mesh.clone(), bed) else {
                break;
            };
            if let Some(placed) = self.studio.plate.get_mut(id) {
                if let Some(pos) = part.pos {
                    placed.pos = pos;
                }
                placed.rot_deg = part.rot_deg;
                placed.scale = part.scale;
            }
        }
        // parts that were never placed would lie one over another
        if held.parts.len() > 1 && held.parts.iter().all(|p| p.pos.is_none()) {
            self.studio.arrange(volume, &self.state.config);
        }
        self.studio.mark_edited();
        self.studio.fit_next = true;
        self.studio.sync(&mut self.state);
        if held.program.is_some() {
            self.suite
                .own_notes
                .push(Msg::SuiteSliceAgain.tr(lang).to_string());
        }
    }

    /// The name offered for a project that is saved for the first time or under another name.
    pub(super) fn suite_suggested_name(&self) -> String {
        let stem = self
            .suite
            .file
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .or_else(|| {
                let first = self.studio.plate.objects.first()?;
                Path::new(&first.name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "job".to_owned());
        format!("{stem}.eh3d")
    }

    /// The name of the folder offered to extract a branch into.
    pub(super) fn suite_extract_name(&self) -> String {
        let stem = self
            .suite
            .file
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "job".to_owned());
        format!("{stem}-{}", self.suite.extract_branch.replace('/', "-"))
    }

    /// Saves into the project of the window, or says that it has none.
    pub(super) fn suite_save(&mut self) -> Step {
        match self.suite.file.clone() {
            Some(file) => self.suite_run(Action::Save(file), false),
            None => Step::NeedsName,
        }
    }

    /// Saves under another name. A new file gets every branch of the project that is open; a file that is there keeps its own branches and only `3dp` is written, once the person
    /// agreed.
    pub(super) fn suite_save_as(&mut self, target: &Path) -> Step {
        self.suite_run(Action::Save(suitecore::with_extension(target)), false)
    }

    /// Keeps the original of a model or a program of another program in the project.
    pub(super) fn suite_import(&mut self, source: &Path) -> Step {
        self.suite_run(Action::Import(source.to_path_buf()), false)
    }

    /// Takes a file of another branch into the project, with the reference that tells later when it is out of date.
    pub(super) fn suite_take(&mut self, branch: &str, rel: &str) -> Step {
        self.suite_run(Action::Take(branch.to_owned(), rel.to_owned()), false)
    }

    /// The person agreed to the write that was waiting.
    pub(super) fn suite_confirm(&mut self) -> Step {
        match self.suite.pending.take() {
            Some(pending) => self.suite_run(pending.action, true),
            None => Step::Refused,
        }
    }

    /// The person did not agree: nothing was written and nothing is.
    pub(super) fn suite_cancel(&mut self) {
        self.suite.pending = None;
    }

    /// Writes the files of a branch into a folder; a file that is there is never written over.
    pub(super) fn suite_extract(&mut self, branch: &str, dir: &Path) -> bool {
        let Some(file) = self.suite.file.clone() else {
            self.suite_refuse(S::NeedProject);
            return false;
        };
        match eh3dfile::extract(&file, branch, dir) {
            Ok(count) => {
                let said = Said(
                    S::Extracted,
                    vec![count.to_string(), dir.display().to_string()],
                );
                self.suite_tell(&said);
                true
            }
            Err(e) => {
                self.suite_fail(e);
                false
            }
        }
    }

    /// Asks for the agreement of the person when there is something to agree to.
    fn suite_ask(&mut self, target: &Path, action: &Action) -> Option<Step> {
        let found = suitecore::warnings(
            target,
            self.suite.file.as_deref(),
            self.suite.seen.as_deref(),
            BRANCH,
        );
        match found {
            Ok(warnings) if warnings.is_empty() => None,
            Ok(warnings) => {
                self.suite.pending = Some(Pending {
                    action: action.clone(),
                    warnings,
                });
                Some(Step::Asked)
            }
            Err(e) => Some(self.suite_refusal(e, target)),
        }
    }

    fn suite_run(&mut self, action: Action, agreed: bool) -> Step {
        self.suite.pending = None;
        match &action {
            Action::Save(target) => self.suite_write(target, &action, agreed),
            Action::Import(_) | Action::Take(_, _) => self.suite_bring(&action, agreed),
        }
    }

    /// An import and a take: no slice running, a project, nothing on the bed that the project does not have (the bed is read again afterwards) and the agreement of the person when
    /// there is something to agree to.
    fn suite_bring(&mut self, action: &Action, agreed: bool) -> Step {
        if self.studio.is_slicing() {
            return self.suite_refuse(S::Busy);
        }
        let Some(file) = self.suite.file.clone() else {
            return self.suite_refuse(S::NeedProject);
        };
        if self.suite_unsaved() {
            let line = Msg::SuiteSaveFirst.tr(self.lang()).to_string();
            return self.suite_fail(line);
        }
        if !agreed {
            if let Some(step) = self.suite_ask(&file, action) {
                return step;
            }
        }
        if let Err(step) = self.suite_unchanged(&file) {
            return step;
        }
        let limits = self.state.config.limits.clone();
        let done = match action {
            Action::Import(source) => {
                eh3dfile::import_file(&file, source, &Identity::default(), &limits)
                    .map(|(report, said)| (said, Said(S::Imported, vec![report.source.name])))
            }
            Action::Take(branch, name) => eh3dfile::take(&file, branch, name, &limits)
                .map(|(_, said)| (said, Said(S::Taken, vec![branch.clone(), name.clone()]))),
            Action::Save(_) => return Step::Refused,
        };
        match done {
            Ok((said, told)) => {
                if !self.suite_open(&file) {
                    return Step::Refused;
                }
                let (words, other) = suitecore::notices(&said);
                self.suite.notes.extend(words);
                self.suite.extra.extend(other);
                self.suite_tell(&told);
                Step::Done
            }
            Err(e) => self.suite_fail(e),
        }
    }

    fn suite_write(&mut self, target: &Path, action: &Action, agreed: bool) -> Step {
        if !suitecore::is_project(target) {
            return self.suite_refuse(S::NeedExtension);
        }
        let (mut contents, notes) = self.suite_contents(target);
        let current = self.suite.file.clone();
        let same = current
            .as_deref()
            .is_some_and(|c| suitecore::same_file(c, target));
        let copy_from = current.filter(|c| !target.exists() && c.is_file());
        // the file that will be written, or the one that is copied to it, is the project of the window: only what changed since goes
        let untouched = match (&self.suite.file, &self.suite.seen) {
            (Some(file), Some(seen)) => suitecore::check_unchanged(file, seen).is_ok(),
            _ => true,
        };
        if (same || copy_from.is_some()) && untouched {
            if let Some(loaded) = &self.suite.loaded {
                drop_untouched(&mut contents, loaded);
            }
        }
        // what the branch has already, in the file that will be written or in the one that is copied to it
        let base = if target.exists() {
            Some(target.to_path_buf())
        } else {
            copy_from.clone()
        };
        if let Some(base) = &base {
            let found = match suitecore::view(base, BRANCH) {
                Ok(found) => found,
                Err(e) => return self.suite_fail(e),
            };
            if found.branch(BRANCH).is_some() {
                match eh3dfile::held(base, &self.state.config.limits) {
                    Ok(held) => contents = eh3dfile::changes(contents, &held),
                    Err(e) => return self.suite_fail(e),
                }
            }
        }
        if contents.is_empty() && copy_from.is_none() {
            if !same {
                return self.suite_refuse(S::NothingToSave);
            }
            self.suite.notes = notes;
            self.suite_tell(&Said::plain(S::NothingNew));
            return Step::Done;
        }
        if !contents.is_empty() && !agreed {
            if let Some(step) = self.suite_ask(target, action) {
                return step;
            }
        }
        if !contents.is_empty() {
            if let Err(step) = self.suite_unchanged(target) {
                return step;
            }
        }
        if let Some(from) = &copy_from {
            if let Err(e) = suitecore::copy_project(from, target) {
                return self.suite_fail(e);
            }
        }
        let said = if contents.is_empty() {
            Vec::new()
        } else {
            let limits = self.state.config.limits.clone();
            match eh3dfile::save_contents(target, &contents, &Identity::default(), &limits) {
                Ok(said) => said.unwrap_or_default(),
                Err(e) => {
                    // a copy that could not be written to is not left behind
                    if copy_from.is_some() {
                        let _ = std::fs::remove_file(target);
                    }
                    return self.suite_fail(e);
                }
            }
        };
        let (words, other) = suitecore::notices(&said);
        self.suite.file = Some(target.to_path_buf());
        self.suite.view = suitecore::view(target, BRANCH).ok();
        self.suite.seen = self.suite.view.as_ref().and_then(|v| v.own_hash.clone());
        self.suite.notes = notes;
        self.suite.notes.extend(words);
        self.suite.own_notes = Vec::new();
        self.suite.extra = other;
        self.suite.loaded = Some(self.suite_work());
        self.recent_added.push(target.display().to_string());
        self.state.status = Status::Saved(name_of(target));
        self.suite_tell(&Said(S::Saved, vec![BRANCH.to_owned(), name_of(target)]));
        Step::Done
    }

    /// Asks for the path of an entry.
    pub(super) fn suite_prompt(&mut self, ask: Ask) {
        self.suite.ask_text = match ask {
            Ask::SaveAs => self.suite_suggested_name(),
            Ask::Extract => self.suite_extract_name(),
            Ask::Open | Ask::Import => String::new(),
        };
        self.suite.ask = Some(ask);
    }

    /// Does what the typed path was asked for.
    pub(super) fn suite_answer(&mut self) {
        let Some(ask) = self.suite.ask else {
            return;
        };
        let text = self.suite.ask_text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.suite.ask = None;
        let path = PathBuf::from(text);
        match ask {
            Ask::Open => {
                self.suite_open(&path);
            }
            Ask::SaveAs => {
                self.suite_save_as(&path);
            }
            Ask::Import => {
                self.suite_import(&path);
            }
            Ask::Extract => {
                let branch = self.suite.extract_branch.clone();
                self.suite_extract(&branch, &path);
            }
        }
    }

    /// The entry Save: into the project of the window, or a name is asked for.
    pub(super) fn suite_save_entry(&mut self) {
        if self.suite_save() == Step::NeedsName {
            self.suite_prompt(Ask::SaveAs);
        }
    }

    /// Shows a list of the project, read again.
    pub(super) fn suite_show_list(&mut self, list: SuiteList) {
        if self.suite.file.is_none() {
            self.suite_refuse(S::NeedProject);
            return;
        }
        self.suite_refresh();
        self.suite.list = Some(list);
    }

    /// The project stays in sight under the top bar: its name, what is out of date, what a write would unsign and what was last done. It is read again when the window gets the
    /// keyboard back, so what another program changed is told.
    pub(super) fn suite_bar(&mut self, ui: &mut egui::Ui) {
        let focused = ui.ctx().input(|i| i.focused);
        if focused && !self.suite.was_focused {
            self.suite_refresh();
        }
        self.suite.was_focused = focused;
        let lines = self.suite_banner();
        if lines.is_empty() {
            return;
        }
        let (danger, dim) = (col(self.look.danger), col(self.look.dim));
        egui::Panel::top("suite").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (text, attention) in lines {
                    let text = egui::RichText::new(text);
                    ui.label(if attention {
                        text.strong().color(danger)
                    } else {
                        text.color(dim)
                    });
                    ui.separator();
                }
            });
        });
    }

    /// The question for a path, the lists of the project and the question before a write.
    pub(super) fn suite_windows(&mut self, ctx: &egui::Context) {
        self.suite_path_window(ctx);
        self.suite_list_window(ctx);
        self.suite_confirm_window(ctx);
    }

    fn suite_path_window(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.suite.ask else {
            return;
        };
        let lang = self.lang();
        let code = self.suite_code();
        let (title, label, button) = match ask {
            Ask::Open => (S::MenuOpen, Msg::SuiteFilePath, Msg::Open.tr(lang)),
            Ask::SaveAs => (S::MenuSaveAs, Msg::SuiteFilePath, Msg::Save.tr(lang)),
            Ask::Import => (S::MenuImport, Msg::SuiteFilePath, Msg::Open.tr(lang)),
            Ask::Extract => (S::MenuExtract, Msg::SuiteFolderPath, S::Extract.tr(code)),
        };
        let mut answer = None;
        egui::Window::new(title.tr(code))
            .id(egui::Id::unique("suite-path"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(label.tr(lang));
                ui.add(egui::TextEdit::singleline(&mut self.suite.ask_text).desired_width(420.0));
                ui.horizontal(|ui| {
                    if ui.button(button).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(Msg::CancelAction.tr(lang)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            answer = Some(true);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            answer = Some(false);
        }
        match answer {
            Some(true) => self.suite_answer(),
            Some(false) => self.suite.ask = None,
            None => {}
        }
    }

    fn suite_list_window(&mut self, ctx: &egui::Context) {
        let Some(list) = self.suite.list else {
            return;
        };
        let Some(view) = self.suite.view.clone() else {
            self.suite.list = None;
            return;
        };
        let code = self.suite_code();
        let (accent, danger) = (col(self.look.accent), col(self.look.danger));
        let title = match list {
            SuiteList::Branches => S::MenuBranches,
            SuiteList::Take => S::MenuTake,
            SuiteList::Extract => S::MenuExtract,
        };
        let mut take = None;
        let mut extract = None;
        let mut again = false;
        let mut close = false;
        egui::Window::new(title.tr(code))
            .id(egui::Id::unique("suite-list"))
            .collapsible(false)
            .default_size([620.0, 360.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| match list {
                        SuiteList::Branches => {
                            egui::Grid::new("suite-branches")
                                .num_columns(5)
                                .striped(true)
                                .show(ui, |ui| {
                                    for word in [
                                        S::ColBranch,
                                        S::ColApp,
                                        S::ColFormat,
                                        S::ColFiles,
                                        S::ColState,
                                    ] {
                                        ui.label(egui::RichText::new(word.tr(code)).color(accent));
                                    }
                                    ui.end_row();
                                    for b in &view.branches {
                                        ui.label(b.id.as_str());
                                        ui.label(b.app.as_str());
                                        ui.label(b.format.as_str());
                                        ui.label(b.files.to_string());
                                        let state =
                                            egui::RichText::new(suitecore::state_text(b, code));
                                        ui.label(if b.stale.is_empty() {
                                            state
                                        } else {
                                            state.color(danger)
                                        });
                                        ui.end_row();
                                    }
                                });
                        }
                        SuiteList::Take => {
                            if view.takeable.is_empty() {
                                ui.label(S::NothingToTake.tr(code));
                            } else {
                                ui.label(S::ChooseFile.tr(code));
                            }
                            for (branch, name) in &view.takeable {
                                if ui
                                    .selectable_label(false, format!("{branch}/{name}"))
                                    .clicked()
                                {
                                    take = Some((branch.clone(), name.clone()));
                                }
                            }
                        }
                        SuiteList::Extract => {
                            ui.label(S::ChooseBranch.tr(code));
                            for b in &view.branches {
                                if ui.selectable_label(false, b.id.as_str()).clicked() {
                                    extract = Some(b.id.clone());
                                }
                            }
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(S::ReadAgain.tr(code)).clicked() {
                        again = true;
                    }
                    if ui.button(S::Close.tr(code)).clicked() {
                        close = true;
                    }
                });
            });
        if again {
            self.suite_refresh();
        }
        if close {
            self.suite.list = None;
        }
        if let Some((branch, name)) = take {
            self.suite.list = None;
            self.suite_take(&branch, &name);
        }
        if let Some(branch) = extract {
            self.suite.list = None;
            self.suite.extract_branch = branch;
            self.suite_prompt(Ask::Extract);
        }
    }

    fn suite_confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.suite.pending else {
            return;
        };
        let code = self.suite_code();
        let lines: Vec<String> = pending.warnings.iter().map(|w| w.text(code)).collect();
        let cancel = Msg::CancelAction.tr(self.lang());
        let danger = col(self.look.danger);
        let mut answer = None;
        egui::Window::new(S::ConfirmTitle.tr(code))
            .id(egui::Id::unique("suite-confirm"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                for line in &lines {
                    ui.label(egui::RichText::new(line.as_str()).color(danger));
                }
                ui.horizontal(|ui| {
                    if ui.button(S::Write.tr(code)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(cancel).clicked() {
                        answer = Some(false);
                    }
                });
            });
        // Escape says no; no button has the keyboard, so Enter does not write
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            answer = Some(false);
        }
        match answer {
            Some(true) => {
                self.suite_confirm();
            }
            Some(false) => self.suite_cancel(),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_container::AtomicOptions;
    use eh3d_project::{
        BranchUpdate, Ed25519Signer, NewProject, OpenOptions, Project, SaveOptions, Writer,
    };
    use p3dp_core::mesh::cube;
    use p3dp_formats::{write_mesh, Format};
    use std::collections::BTreeMap;
    use std::fs;

    fn folder(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p3dp_suitewin_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn model(dir: &Path, name: &str, size: f64) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, write_mesh(&cube(size), Format::Stl, "").unwrap()).unwrap();
        path
    }

    fn write_branch(path: &Path, branch: &str, entries: &[(&str, &[u8])]) {
        let project = Project::open(path, &OpenOptions::default()).unwrap();
        let mut update = BranchUpdate::new(branch, "1.0", "0.1.0");
        for (rel, bytes) in entries {
            update = update.with_entry(rel, bytes);
        }
        let writer = Writer::new(branch, &[branch]);
        project
            .save_branch(&writer, update, &SaveOptions::default())
            .unwrap();
    }

    /// A made-up project with the branches of other programs and none of this one.
    fn foreign(path: &Path) {
        let new = NewProject {
            id: "00000000-0000-4000-8000-000000000033".into(),
            name: "Made up".into(),
            units: "millimetres".into(),
            created: None,
        };
        Project::create(
            path,
            new,
            &OpenOptions::default(),
            &AtomicOptions::default(),
        )
        .unwrap();
        let boxed = write_mesh(&cube(8.0), Format::Stl, "").unwrap();
        write_branch(path, "cad", &[("box.stl", &boxed)]);
        write_branch(path, "x-vendor", &[("blob", b"anything")]);
    }

    /// Every file of every branch but `3dp`.
    fn others(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut project = Project::open(path, &OpenOptions::default()).unwrap();
        let names: Vec<(String, String)> = project
            .branches()
            .filter(|b| b.id != BRANCH)
            .flat_map(|b| b.files.keys().map(|f| (b.id.clone(), f.clone())))
            .collect();
        names
            .into_iter()
            .map(|(b, f)| (format!("{b}/{f}"), project.read_entry(&b, &f).unwrap()))
            .collect()
    }

    fn window() -> App {
        let state = AppState::new(AppConfig::default(), Lang::En).unwrap();
        App::new(state, &WindowArgs::default(), None)
    }

    fn with_part(dir: &Path, name: &str) -> App {
        let mut a = window();
        a.open_path(&model(dir, name, 10.0).display().to_string());
        assert_eq!(a.studio.plate.len(), 1, "{:?}", a.state.status);
        a
    }

    fn problem(a: &App) -> String {
        match &a.state.status {
            Status::Problem(p) => p.clone(),
            other => panic!("no problem is told: {other:?}"),
        }
    }

    fn warned(a: &App) -> Vec<S> {
        let pending = a.suite.pending.as_ref().expect("a write waits");
        pending.warnings.iter().map(|n| n.0).collect()
    }

    #[test]
    fn the_bed_is_saved_into_a_new_project_and_opened_again() {
        let d = folder("new");
        let mut a = with_part(&d, "part.stl");
        a.studio.plate.selected_mut().unwrap().rot_deg = [0.0, 0.0, 30.0];
        assert_eq!(a.suite_save(), Step::NeedsName);
        assert_eq!(a.suite_suggested_name(), "part.eh3d");
        assert!(a.suite_banner().is_empty());
        assert_eq!(a.suite_save_as(&d.join("job.zip")), Step::Refused);
        assert_eq!(a.suite_save_as(&d.join("job")), Step::Done);
        let project = d.join("job.eh3d");
        assert_eq!(a.suite.file.as_deref(), Some(project.as_path()));
        // nothing changed: nothing is written
        let before = fs::read(&project).unwrap();
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(fs::read(&project).unwrap(), before);

        let mut b = window();
        b.open_path(&project.display().to_string());
        assert_eq!(b.suite_parts(), a.suite_parts());
        assert_eq!(b.preview.config, a.preview.config);
        assert_eq!(b.state.printer_id, a.state.printer_id);
        assert!(b.suite_banner()[0].0.contains("job.eh3d"));
        assert_eq!(b.suite_save(), Step::Done);
        assert_eq!(fs::read(&project).unwrap(), before);
        // a new bed forgets the project, so that it is not written over by accident
        b.run_cmd(Cmd::Std(StdAction::New), &egui::Context::default());
        assert!(b.suite.file.is_none());
        assert_eq!(b.suite_save(), Step::NeedsName);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_g_code_goes_only_when_it_is_the_one_of_the_bed() {
        let d = folder("gcode");
        let mut a = with_part(&d, "part.stl");
        assert!(a.state.select_printer("bambu-lab-p1s"));
        a.slice_blocking();
        assert!(a.preview.gcode.is_some(), "{:?}", a.preview.error);
        a.studio.stale = false;
        let project = d.join("job.eh3d");
        assert_eq!(a.suite_save_as(&project), Step::Done);
        let limits = a.state.config.limits.clone();
        let held = eh3dfile::held(&project, &limits).unwrap();
        assert_eq!(held.program.as_deref(), Some("programs/job.gcode"));
        // a part that moved makes the G-code of before not the one of the bed: it leaves the project
        a.studio.plate.selected_mut().unwrap().pos = [50.0, 50.0];
        a.studio.mark_edited();
        assert_eq!(a.suite_save(), Step::Done);
        assert!(eh3dfile::held(&project, &limits).unwrap().program.is_none());
        assert!(a
            .suite_banner()
            .iter()
            .any(|(l, _)| l.contains("not put in the project")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn saving_under_another_name_keeps_every_branch_and_another_project_is_asked_about() {
        let d = folder("saveas");
        let first = d.join("first.eh3d");
        foreign(&first);
        let mut a = with_part(&d, "part.stl");
        assert!(a.suite_open(&first));
        // what was on the bed before does not go into the project that is opened
        assert!(a.studio.plate.is_empty());
        a.open_path(&d.join("part.stl").display().to_string());
        let kept = others(&first);
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(others(&first), kept);
        let second = d.join("second.eh3d");
        assert_eq!(a.suite_save_as(&second), Step::Done);
        assert_eq!(fs::read(&second).unwrap(), fs::read(&first).unwrap());

        let theirs = d.join("theirs.eh3d");
        foreign(&theirs);
        let bytes = fs::read(&theirs).unwrap();
        assert_eq!(a.suite_save_as(&theirs), Step::Asked);
        assert_eq!(warned(&a), [S::WarnForeign]);
        assert_eq!(fs::read(&theirs).unwrap(), bytes);
        a.suite_cancel();
        assert_eq!(a.suite_confirm(), Step::Refused);
        assert_eq!(fs::read(&theirs).unwrap(), bytes);
        let kept = others(&theirs);
        assert_eq!(a.suite_save_as(&theirs), Step::Asked);
        assert_eq!(a.suite_confirm(), Step::Done);
        assert_eq!(others(&theirs), kept);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_signed_branch_is_asked_about_and_escape_says_no() {
        let d = folder("signed");
        let project = d.join("job.eh3d");
        let mut a = with_part(&d, "part.stl");
        assert_eq!(a.suite_save_as(&project), Step::Done);
        let key = Ed25519Signer::from_seed(&[5; 32]);
        Project::open(&project, &OpenOptions::default())
            .unwrap()
            .sign_branch(BRANCH, &key, &SaveOptions::default())
            .unwrap();
        a.suite_refresh();
        assert!(a
            .suite_banner()
            .iter()
            .any(|(line, attention)| *attention && line.contains("is signed")));
        a.studio.plate.selected_mut().unwrap().pos = [40.0, 40.0];
        let bytes = fs::read(&project).unwrap();
        assert_eq!(a.suite_save(), Step::Asked);
        assert!(warned(&a).contains(&S::SignedBranch));

        let ctx = egui::Context::default();
        let press = |a: &mut App, key: Option<egui::Key>| {
            let mut input = egui::RawInput::default();
            if let Some(key) = key {
                input.events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut out = ctx.run_ui(input, |ui| a.suite_windows(ui.ctx()));
            out.textures_delta.clear();
        };
        press(&mut a, None);
        press(&mut a, Some(egui::Key::Enter));
        assert!(a.suite.pending.is_some(), "Enter does not write");
        // nothing else of the menus is done behind the question
        a.run_cmd(Cmd::Std(StdAction::New), &ctx);
        assert!(a.suite.pending.is_some() && a.suite.file.is_some());
        press(&mut a, Some(egui::Key::Escape));
        assert!(a.suite.pending.is_none());
        assert_eq!(fs::read(&project).unwrap(), bytes);
        assert_eq!(a.suite_save(), Step::Asked);
        assert_eq!(a.suite_confirm(), Step::Done);
        assert_ne!(fs::read(&project).unwrap(), bytes);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_project_saved_by_another_program_is_not_written_over() {
        let d = folder("changed");
        let project = d.join("job.eh3d");
        let mut a = with_part(&d, "part.stl");
        assert_eq!(a.suite_save_as(&project), Step::Done);
        // another program saves the file: the bytes arrive without this program writing them
        let elsewhere = d.join("elsewhere.eh3d");
        foreign(&elsewhere);
        fs::copy(&elsewhere, &project).unwrap();
        let theirs = fs::read(&project).unwrap();
        a.studio.plate.selected_mut().unwrap().pos = [30.0, 30.0];
        assert_eq!(a.suite_save(), Step::Refused);
        assert!(
            problem(&a).contains("another program saved it"),
            "{}",
            problem(&a)
        );
        assert_eq!(fs::read(&project).unwrap(), theirs);
        // saved after the question was shown: the answer writes nothing either
        a.suite.pending = Some(Pending {
            action: Action::Save(project.clone()),
            warnings: Vec::new(),
        });
        assert_eq!(a.suite_confirm(), Step::Refused);
        assert_eq!(fs::read(&project).unwrap(), theirs);
        // another name takes the work of the window
        assert_eq!(a.suite_save_as(&d.join("mine.eh3d")), Step::Done);
        let held = eh3dfile::held(&d.join("mine.eh3d"), &a.state.config.limits).unwrap();
        assert_eq!(held.parts, a.suite_parts());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_part_taken_from_cad_goes_on_the_bed_and_unsaved_work_is_not_thrown_away() {
        let d = folder("take");
        let project = d.join("job.eh3d");
        foreign(&project);
        let mut a = with_part(&d, "part.stl");
        assert_eq!(a.suite_take("cad", "box.stl"), Step::Refused);
        assert!(a.suite_open(&project));
        a.open_path(&d.join("part.stl").display().to_string());
        let bytes = fs::read(&project).unwrap();
        assert_eq!(a.suite_take("cad", "box.stl"), Step::Refused);
        assert!(problem(&a).contains("save"), "{}", problem(&a));
        assert_eq!(fs::read(&project).unwrap(), bytes);
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(
            a.suite_take("cad", "box.stl"),
            Step::Done,
            "{:?}",
            a.state.status
        );
        assert_eq!(a.studio.plate.len(), 2);
        let other = write_mesh(&cube(9.0), Format::Stl, "").unwrap();
        write_branch(&project, "cad", &[("box.stl", &other)]);
        a.suite_refresh();
        assert!(a.suite_banner().iter().any(|(line, attention)| *attention
            && line.starts_with("OUT OF DATE: 3dp was derived from cad")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_import_keeps_the_original_and_a_branch_is_extracted_without_writing_over() {
        let d = folder("import");
        let source = model(&d, "other.stl", 7.0);
        let mut a = window();
        assert_eq!(a.suite_import(&source), Step::Refused);
        let project = d.join("job.eh3d");
        a.open_path(&model(&d, "part.stl", 10.0).display().to_string());
        assert_eq!(a.suite_save_as(&project), Step::Done);
        assert_eq!(a.suite_import(&source), Step::Done, "{:?}", a.state.status);
        assert_eq!(a.studio.plate.len(), 1);
        assert_eq!(a.studio.plate.objects[0].name, "other.stl");
        assert!(a
            .suite_banner()
            .iter()
            .any(|(l, _)| l.contains("other.stl")));
        let held = eh3dfile::held(&project, &a.state.config.limits).unwrap();
        assert_eq!(held.files["origin/other.stl"], fs::read(&source).unwrap());

        a.suite.extract_branch = BRANCH.to_owned();
        assert_eq!(a.suite_extract_name(), "job-3dp");
        let out = d.join("job-3dp");
        assert!(a.suite_extract(BRANCH, &out));
        assert_eq!(
            fs::read(out.join("origin/other.stl")).unwrap(),
            fs::read(&source).unwrap()
        );
        assert!(!a.suite_extract(BRANCH, &out));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_panel_of_the_attachments_that_is_asked_for_comes_to_the_front() {
        let mut shell = Shell::new(window(), false);
        let _ = shell
            .dock
            .apply(DockAction::SwitchWorkspace("Setup".to_string()));
        let front = |shell: &Shell| {
            shell
                .dock
                .core()
                .layout()
                .active_of(pid::ATTACHMENTS)
                .map(str::to_string)
        };
        assert_eq!(front(&shell).as_deref(), Some(pid::PROJECT));
        let show = DockAction::ShowPanel(pid::ATTACHMENTS.to_string());
        shell.app.borrow_mut().pending_dock.push(show);
        shell.after(None, &egui::Context::default());
        assert_eq!(front(&shell).as_deref(), Some(pid::ATTACHMENTS));
    }

    #[test]
    fn a_damaged_project_changes_nothing_and_the_windows_are_drawn() {
        let d = folder("damaged");
        let junk = d.join("junk.eh3d");
        fs::write(&junk, "not a project").unwrap();
        let mut a = with_part(&d, "part.stl");
        assert!(!a.suite_open(&junk));
        assert_eq!(a.studio.plate.len(), 1);
        assert!(a.suite.file.is_none());
        assert_eq!(a.suite_save_as(&junk), Step::Refused);
        assert_eq!(fs::read(&junk).unwrap(), b"not a project");
        a.suite_show_list(SuiteList::Branches);
        assert!(a.suite.list.is_none());

        let project = d.join("job.eh3d");
        foreign(&project);
        assert!(a.suite_open(&project));
        let ctx = egui::Context::default();
        let draw = |a: &mut App| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                a.suite_bar(ui);
                a.suite_windows(ui.ctx());
            });
            out.textures_delta.clear();
        };
        for list in [SuiteList::Branches, SuiteList::Take, SuiteList::Extract] {
            a.suite_show_list(list);
            draw(&mut a);
        }
        for ask in [Ask::Open, Ask::SaveAs, Ask::Import, Ask::Extract] {
            a.suite_prompt(ask);
            draw(&mut a);
        }
        a.suite.pending = Some(Pending {
            action: Action::Save(project.clone()),
            warnings: vec![Said::plain(S::SignedProject)],
        });
        draw(&mut a);
        assert!(a.suite.pending.is_some());
        let _ = fs::remove_dir_all(&d);
    }
}
