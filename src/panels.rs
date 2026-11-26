//! The panels, the workspaces and the menus of the window as data, with no window in them: which panels there are and what each is called in the eight languages, the factory arrangement of
//! each workspace, how the four old tabs (Prepare, Preview, Device and Project) map onto the workspaces, the menu bar and which of its entries can be chosen at a given moment, and the
//! rule that decides when the arrangement is written to the settings file. The window (`gui`) draws these with the frame of the suite (`eh3d-dock`) and only does what they say.

use crate::app_state::Tab;
use crate::i18n::{Lang, Msg};
use crate::keys::{Action, Scheme};
use eh3d_dock::about::{AboutBrand, Phoenix};
use eh3d_dock::core::PanelDef;
use eh3d_dock::menu::{Entry, Item, Label, Menu, MenuBar, StdAction};
use eh3d_dock::model::{Layout, Node};
use eh3d_dock::sectors::Sector;
use eh3d_dock::shortcuts::Shortcut;
use eh3d_dock::stdmenus::{
    edit_menu, file_menu, help_menu, tools_menu, view_menu, window_menu, HelpOptions,
};
use eh3d_dock::texts::Text8;
use eh3d_dock::workspaces::Workspace;

/// The look of the window «About»: which program of the family this is and its own picture, drawn in a ring of its colour.
pub fn about_brand() -> AboutBrand {
    AboutBrand {
        program: Phoenix::Printer3d,
        logo_png: include_bytes!("../images/brand/icon-256.png"),
    }
}

/// The names of the panels (what the settings file stores, so they never change).
pub mod id {
    /// The 3D view of the bed.
    pub const VIEW3D: &str = "view3d";
    /// The parts on the bed and the size of the selected one.
    pub const OBJECTS: &str = "objects";
    /// What the slice does: layers, walls, infill, supports, adhesion.
    pub const PRINT_SETTINGS: &str = "print_settings";
    /// The filament: the search, the maker, the material and the nozzle.
    pub const MATERIALS: &str = "materials";
    /// The layers of the plan, drawn.
    pub const PREVIEW: &str = "preview";
    /// What the plan is made of: the kinds of line, the estimates.
    pub const GCODE: &str = "gcode";
    /// The printer and its limits.
    pub const PRINTER: &str = "printer";
    /// What the program has to say: the state, the warnings, what to look at before printing.
    pub const CONSOLE: &str = "console";
    /// The server and its machines.
    pub const DEVICE: &str = "device";
    /// The state of the machine chosen: the temperatures and the print.
    pub const STATUS: &str = "status";
    /// The queue of jobs of the server.
    pub const QUEUE: &str = "queue";
    /// The files of the project and the settings of the window.
    pub const PROJECT: &str = "project";

    /// Every panel, in the order of the View menu.
    pub const ALL: [&str; 12] = [
        VIEW3D,
        OBJECTS,
        PRINT_SETTINGS,
        MATERIALS,
        PREVIEW,
        GCODE,
        PRINTER,
        CONSOLE,
        DEVICE,
        STATUS,
        QUEUE,
        PROJECT,
    ];
}

/// The names of the workspaces that stand for the four old tabs.
pub const WS_PREPARE: &str = "Prepare";
/// The workspace of the layers of the plan.
pub const WS_REVIEW: &str = "Review";
/// The workspace of the machine and its queue.
pub const WS_RUN: &str = "Run";
/// The workspace of the files and the settings.
pub const WS_SETUP: &str = "Setup";

/// A message as the text of the frame: the eight languages in the order English, Spanish, Italian, German, French, Portuguese (of Portugal), Chinese and Japanese.
pub fn t8(m: Msg) -> Text8 {
    Text8([
        m.tr(Lang::En),
        m.tr(Lang::Es),
        m.tr(Lang::It),
        m.tr(Lang::De),
        m.tr(Lang::Fr),
        m.tr(Lang::Pt),
        m.tr(Lang::Zh),
        m.tr(Lang::Ja),
    ])
}

/// The panels: the ones the suite proposes for a 3D printing program, and the four that this program adds (the server and its machines, the state of the machine, the queue, the project).
pub fn defs() -> Vec<PanelDef> {
    let mut v = Sector::Printer3d.panels();
    v.push(
        PanelDef::new(id::DEVICE, t8(Msg::TabDevice))
            .icon("D")
            .min(220.0, 160.0),
    );
    v.push(
        PanelDef::new(id::STATUS, t8(Msg::PanelStatus))
            .icon("St")
            .min(260.0, 160.0),
    );
    v.push(
        PanelDef::new(id::QUEUE, t8(Msg::QueueTitle))
            .icon("Q")
            .min(240.0, 140.0),
    );
    v.push(
        PanelDef::new(id::PROJECT, t8(Msg::TabProject))
            .icon("J")
            .min(280.0, 200.0),
    );
    v
}

fn ws(name: &str, root: Node) -> Workspace {
    Workspace::new(name, Layout::with_root(root))
}

/// The factory workspaces: Prepare, Review and Compact of the suite for a 3D printing program, then Run (the machine: its server, its state and its queue side by side) and Setup (the project and
/// the settings, with the console below).
pub fn workspaces() -> Vec<Workspace> {
    let t = |ids: &[&str]| Node::tabs(ids);
    let mut v = Sector::Printer3d.workspaces();
    v.push(ws(
        WS_RUN,
        Node::row(
            0.24,
            t(&[id::DEVICE]),
            Node::row(0.62, t(&[id::STATUS]), t(&[id::QUEUE])),
        ),
    ));
    v.push(ws(
        WS_SETUP,
        Node::column(0.78, t(&[id::PROJECT]), t(&[id::CONSOLE])),
    ));
    v
}

/// The workspace that stands for a tab.
pub fn workspace_of(tab: Tab) -> &'static str {
    match tab {
        Tab::Prepare => WS_PREPARE,
        Tab::Preview => WS_REVIEW,
        Tab::Device => WS_RUN,
        Tab::Project => WS_SETUP,
    }
}

/// The tab a workspace stands for; a workspace the person made has none, and the tab stays as it was.
pub fn tab_of(workspace: &str) -> Option<Tab> {
    Tab::ALL.into_iter().find(|t| workspace_of(*t) == workspace)
}

/// What the program does when an entry of its menus is chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    /// An action every program has (New, Open, Save, Quit...).
    Std(StdAction),
    /// Slice the bed.
    Slice,
    /// Put the sliced job in the queue of the server.
    SendToQueue,
    /// Read the machines and the queue again.
    Refresh,
    /// Look at the print from the start.
    ResetView,
    /// Look at the whole printer.
    FitView,
    /// The stop of the server.
    EmergencyStop,
    /// Put a model on the bed from the demonstration.
    LoadDemo,
    /// Arrange the parts.
    Arrange,
    /// Lay the selected part on its largest flat side.
    AutoOrient,
    /// Copy the selected part.
    Duplicate,
    /// Take the selected part off the bed.
    Remove,
    /// One layer up in the preview.
    LayerUp,
    /// One layer down in the preview.
    LayerDown,
    /// Write the G-code of the plan to a file.
    ExportGcode,
}

impl From<StdAction> for Cmd {
    fn from(a: StdAction) -> Cmd {
        Cmd::Std(a)
    }
}

/// The action of the shortcuts the menus have an entry for (its chord is the one of the entry), or `None` for the ones the window handles itself (go to a window, next and previous window).
pub fn menu_entry_of(a: Action) -> Option<&'static str> {
    Some(match a {
        Action::Open => "file.open",
        Action::Save => "file.save",
        Action::Slice => "tools.slice",
        Action::Refresh => "tools.refresh",
        Action::ResetView => "tools.reset_view",
        Action::FitView => "tools.fit_view",
        Action::LayerUp => "tools.layer_up",
        Action::LayerDown => "tools.layer_down",
        Action::EmergencyStop => "tools.emergency_stop",
        Action::ShowShortcuts => "help.shortcuts",
        Action::NextTab
        | Action::PreviousTab
        | Action::GoPrepare
        | Action::GoPreview
        | Action::GoDevice
        | Action::GoProject => return None,
    })
}

/// The shortcut of the frame for a chord of the program (`None` when the chord names a key the frame does not know).
fn shortcut_of(scheme: &Scheme, a: Action) -> Option<Shortcut> {
    let c = scheme.chord(a)?;
    Shortcut::new(c.ctrl, c.alt, c.shift, &c.key)
}

fn own(id: &str, m: Msg, cmd: Cmd) -> Entry<Cmd> {
    Entry::app(id, Label::Own(t8(m)), cmd)
}

/// The menu bar: File, Edit, View, Tools, Window and Help. The shortcuts of the entries are the ones of the scheme of the program (so a key changed in the settings file is the key of the entry),
/// and the keys the frame had for the same entries are taken off (F1 is the window of the shortcuts, not the online help).
pub fn menu_bar(scheme: &Scheme) -> MenuBar<Cmd> {
    let mut file: Menu<Cmd> = file_menu(true);
    let mut edit: Menu<Cmd> = edit_menu();
    let view: Menu<Cmd> = view_menu();
    let mut tools: Menu<Cmd> = tools_menu();
    let window: Menu<Cmd> = window_menu();
    let help: Menu<Cmd> = help_menu(HelpOptions {
        online_help: true,
        check_updates: true,
    });
    // File: the export of the G-code goes with the other exports
    for item in &mut file.items {
        if let Item::Entry(e) = item {
            if e.id == "file.export" {
                e.label = Label::Own(t8(Msg::ExportGcode));
            }
        }
    }
    // Edit: what is done to the parts, after the standard entries
    edit = edit
        .sep()
        .entry(own("edit.duplicate", Msg::Duplicate, Cmd::Duplicate).key(Shortcut::ctrl("D")))
        .entry(own("edit.remove", Msg::Remove, Cmd::Remove).key(Shortcut::plain("Delete")))
        .entry(own("edit.arrange", Msg::Arrange, Cmd::Arrange).key(Shortcut::plain("A")))
        .entry(own("edit.orient", Msg::AutoOrient, Cmd::AutoOrient));
    // Tools: slice, queue, the view, the machine
    tools = tools
        .entry(own("tools.slice", Msg::SliceNow, Cmd::Slice))
        .entry(own("tools.queue", Msg::SendToQueue, Cmd::SendToQueue))
        .sep()
        .entry(own("tools.demo", Msg::LoadDemo, Cmd::LoadDemo))
        .sep()
        .entry(own("tools.fit_view", Msg::ViewFit, Cmd::FitView))
        .entry(own("tools.reset_view", Msg::ViewReset, Cmd::ResetView))
        .entry(own("tools.layer_up", Msg::LayerUp, Cmd::LayerUp))
        .entry(own("tools.layer_down", Msg::LayerDown, Cmd::LayerDown))
        .sep()
        .entry(own("tools.refresh", Msg::Refresh, Cmd::Refresh))
        .entry(own(
            "tools.emergency_stop",
            Msg::EmergencyStop,
            Cmd::EmergencyStop,
        ));
    let mut menus = vec![file, edit, view, tools, window, help];
    // the shortcuts: the scheme of the program is the master
    for menu in &mut menus {
        for item in &mut menu.items {
            let Item::Entry(e) = item else { continue };
            if e.id == "help.online" {
                e.shortcut = None;
            }
            if let Some(a) = Action::ALL
                .into_iter()
                .find(|a| menu_entry_of(*a) == Some(e.id.as_str()))
            {
                e.shortcut = shortcut_of(scheme, a);
            }
        }
    }
    MenuBar::new(menus)
}

/// What the window knows when it decides which entries of the menus can be chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuFacts {
    /// There is a part on the bed.
    pub has_parts: bool,
    /// One of them is selected.
    pub has_selection: bool,
    /// There are two parts or more (so there is something to arrange).
    pub several_parts: bool,
    /// A slice is running.
    pub slicing: bool,
    /// There is a plan of the bed.
    pub has_plan: bool,
    /// A server is set.
    pub has_server: bool,
}

/// The ids of the entries that cannot be chosen now. The ones for things the program does not have (undo, redo, cut, copy, paste) are always off.
pub fn disabled_entries(f: &MenuFacts) -> Vec<String> {
    let mut off: Vec<&str> = vec![
        "edit.undo",
        "edit.redo",
        "edit.cut",
        "edit.copy",
        "edit.paste",
    ];
    let mut when = |cond: bool, ids: &[&'static str]| {
        if cond {
            off.extend_from_slice(ids);
        }
    };
    when(!f.has_parts, &["file.new", "file.save", "file.save_as"]);
    when(!f.has_plan, &["file.export", "tools.queue"]);
    when(!f.has_parts || f.slicing, &["tools.slice"]);
    when(
        !f.has_selection,
        &["edit.duplicate", "edit.remove", "edit.orient"],
    );
    when(!f.several_parts, &["edit.arrange"]);
    when(
        !f.has_server,
        &["tools.queue", "tools.refresh", "tools.emergency_stop"],
    );
    when(!f.has_plan, &["tools.layer_up", "tools.layer_down"]);
    let mut v: Vec<String> = off.into_iter().map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

/// Decides when the arrangement of the panels is written to the settings file: when the lines of the frame are not the ones last written (or read). It writes nothing when nothing changed, so
/// dragging a panel writes once and a quiet window writes never.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutWatch {
    saved: String,
}

impl LayoutWatch {
    /// A watch that knows the lines of the frame as they were read.
    pub fn new(pairs: &[(String, String)]) -> LayoutWatch {
        LayoutWatch {
            saved: LayoutWatch::flat(pairs),
        }
    }

    fn flat(pairs: &[(String, String)]) -> String {
        pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
    }

    /// Whether the lines changed since the last time this answered yes (or since the start); a yes counts them as written.
    pub fn changed(&mut self, pairs: &[(String, String)]) -> bool {
        let now = LayoutWatch::flat(pairs);
        if now == self.saved {
            return false;
        }
        self.saved = now;
        true
    }
}

/// What to do so that the workspace and the tab agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sync {
    /// They agree.
    Nothing,
    /// The program changed the tab: put the workspace that stands for it.
    SwitchWorkspace(&'static str),
    /// The person chose a workspace that stands for a tab: the tab is that one.
    SetTab(Tab),
}

/// Makes the tab and the workspace agree. `last` is the tab at the end of the previous frame: when it differs from `tab` the program changed it (a key, a button, the queue after a send) and the
/// workspace follows; when it does not, a workspace of the person that stands for no tab leaves the tab as it is, and one that stands for a tab sets it.
pub fn sync(tab: Tab, last: Tab, workspace: &str) -> Sync {
    if tab != last {
        return if workspace == workspace_of(tab) {
            Sync::Nothing
        } else {
            Sync::SwitchWorkspace(workspace_of(tab))
        };
    }
    match tab_of(workspace) {
        Some(t) if t != tab => Sync::SetTab(t),
        _ => Sync::Nothing,
    }
}

/// The width of a field for a path: what is free less `reserved` (the button beside it and the gaps), held between a width that shows a short path and one that does not run across a huge
/// panel. A width that is not a number gives the middle one.
pub fn path_field_width(available: f32, reserved: f32) -> f32 {
    if !available.is_finite() || !reserved.is_finite() {
        return PATH_FIELD_DEFAULT;
    }
    (available - reserved.max(0.0) - 12.0).clamp(PATH_FIELD_MIN, PATH_FIELD_MAX)
}

/// The narrowest a field for a path is drawn.
pub const PATH_FIELD_MIN: f32 = 160.0;
/// The widest a field for a path is drawn.
pub const PATH_FIELD_MAX: f32 = 1200.0;
/// The width of a field for a path when the room is not known.
pub const PATH_FIELD_DEFAULT: f32 = 320.0;

/// Cuts a rectangle `[min x, min y, max x, max y]` in two: what is left and a strip of `width` on the right side, as tall as the rectangle. A strip wider than the rectangle is the whole
/// of it, and what is left is empty; a width that is not a number or is negative gives no strip.
pub fn split_right(rect: [f32; 4], width: f32) -> ([f32; 4], [f32; 4]) {
    let [x0, y0, x1, y1] = rect;
    let w = if width.is_finite() {
        width.max(0.0)
    } else {
        0.0
    };
    let cut = (x1 - w).max(x0);
    ([x0, y0, cut, y1], [cut, y0, x1, y1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::SchemeId;
    use eh3d_dock::core::DockCore;
    use eh3d_dock::menu::MenuContext;
    use eh3d_dock::sectors::workspace_title;
    use eh3d_view::texts::Lang as ViewLang;

    fn core() -> DockCore {
        DockCore::new(defs(), workspaces())
    }

    fn bar_ids(bar: &MenuBar<Cmd>) -> Vec<String> {
        fn walk(items: &[Item<Cmd>], out: &mut Vec<String>) {
            for i in items {
                match i {
                    Item::Entry(e) => out.push(e.id.clone()),
                    Item::Submenu(m) => walk(&m.items, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for m in &bar.menus {
            walk(&m.items, &mut out);
        }
        out
    }

    fn entry<'a>(bar: &'a MenuBar<Cmd>, id: &str) -> &'a Entry<Cmd> {
        bar.menus
            .iter()
            .flat_map(|m| m.items.iter())
            .find_map(|i| match i {
                Item::Entry(e) if e.id == id => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no entry {id}"))
    }

    #[test]
    fn the_panels_have_names_that_hold_and_titles_in_the_eight_languages() {
        let d = defs();
        assert_eq!(d.len(), 12);
        let names: Vec<&str> = d.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(names, id::ALL.to_vec());
        for p in &d {
            assert!(
                p.id.len() <= 48
                    && p.id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{}",
                p.id
            );
            for t in p.title.0 {
                assert!(!t.trim().is_empty(), "{}", p.id);
            }
        }
        // the titles of the four panels of this program are the messages of the program, in the order the frame wants
        let device = d.iter().find(|p| p.id == id::DEVICE).unwrap();
        assert_eq!(device.title.0[1], "Dispositivo");
        assert_eq!(device.title.0[5], "Dispositivo");
        assert_eq!(device.title.0[7], "デバイス");
        let status = d.iter().find(|p| p.id == id::STATUS).unwrap();
        assert_eq!(status.title.0[5], "Estado da máquina");
    }

    #[test]
    fn the_factory_arrangement_is_sound_and_every_workspace_has_a_name_in_eight_languages() {
        let c = core();
        assert_eq!(
            c.workspaces().names(),
            vec!["Prepare", "Review", "Compact", "Run", "Setup"]
        );
        assert!(c.warnings().is_empty(), "{:?}", c.warnings());
        assert!(c.layout_is_sound());
        for name in c.workspaces().names() {
            assert!(workspace_title(name).is_some(), "{name}");
        }
        // every panel is in some workspace, so the View menu can bring any of them back from a factory layout
        for p in id::ALL {
            let shown = c
                .workspaces()
                .factory()
                .iter()
                .any(|w| w.layout.is_shown(p));
            assert!(shown, "{p} is in no workspace");
        }
        // the four tabs of the old window are four of the workspaces
        for t in Tab::ALL {
            assert!(c.workspaces().names().contains(&workspace_of(t)), "{t:?}");
        }
    }

    #[test]
    fn a_tab_and_its_workspace_name_each_other_and_a_workspace_of_the_person_names_no_tab() {
        for t in Tab::ALL {
            assert_eq!(tab_of(workspace_of(t)), Some(t));
        }
        assert_eq!(tab_of("Compact"), None);
        assert_eq!(tab_of("my layout"), None);
        assert_eq!(tab_of(""), None);
        assert_eq!(workspace_of(Tab::Preview), "Review");
    }

    #[test]
    fn the_menu_bar_is_well_made_for_both_schemes_and_every_language() {
        for id in SchemeId::ALL {
            let scheme = Scheme::of(id);
            let bar = menu_bar(&scheme);
            assert!(bar.validate().is_empty(), "{:?}", bar.validate());
            let titles: Vec<&str> = bar.menus.iter().map(|m| m.id.as_str()).collect();
            assert_eq!(titles, ["file", "edit", "view", "tools", "window", "help"]);
            for l in [
                ViewLang::En,
                ViewLang::Es,
                ViewLang::De,
                ViewLang::Fr,
                ViewLang::It,
                ViewLang::Pt,
                ViewLang::Zh,
                ViewLang::Ja,
            ] {
                let ctx = MenuContext::empty(l);
                assert!(
                    bar.conflicts(&ctx).is_empty(),
                    "{:?} in {l:?}",
                    bar.conflicts(&ctx)
                );
            }
            // File and Help have what a program has
            let ids = bar_ids(&bar);
            for want in [
                "file.new",
                "file.open",
                "file.save",
                "file.save_as",
                "file.export",
                "file.quit",
                "help.shortcuts",
                "help.about",
                "help.updates",
                "help.online",
            ] {
                assert!(ids.iter().any(|x| x == want), "{want}");
            }
        }
    }

    #[test]
    fn the_chord_of_each_action_of_the_scheme_is_the_one_of_its_menu_entry_or_is_handled_by_the_window(
    ) {
        for id in SchemeId::ALL {
            let scheme = Scheme::of(id);
            let bar = menu_bar(&scheme);
            for a in Action::ALL {
                match menu_entry_of(a) {
                    Some(entry_id) => {
                        let e = entry(&bar, entry_id);
                        let c = scheme.chord(a).unwrap();
                        let s = e
                            .shortcut
                            .clone()
                            .unwrap_or_else(|| panic!("{entry_id} has no shortcut"));
                        assert_eq!(
                            (s.ctrl, s.alt, s.shift),
                            (c.ctrl, c.alt, c.shift),
                            "{entry_id}"
                        );
                        assert!(
                            s.key.eq_ignore_ascii_case(&c.key),
                            "{entry_id}: {} {}",
                            s.key,
                            c.key
                        );
                    }
                    None => assert!(
                        matches!(
                            a,
                            Action::NextTab
                                | Action::PreviousTab
                                | Action::GoPrepare
                                | Action::GoPreview
                                | Action::GoDevice
                                | Action::GoProject
                        ),
                        "{a:?}"
                    ),
                }
            }
            // no chord of the scheme is the shortcut of two things in the bar: the keys of the window and the keys of the menus are different
            let mut seen: Vec<String> = Vec::new();
            for e in bar_ids(&bar) {
                if let Some(s) = &entry(&bar, &e).shortcut {
                    let name = s.to_string();
                    assert!(!seen.contains(&name), "{name} twice");
                    seen.push(name);
                }
            }
            assert!(entry(&bar, "help.online").shortcut.is_none());
        }
    }

    #[test]
    fn a_key_changed_in_the_settings_is_the_key_of_the_entry() {
        let scheme = Scheme::with_changes(
            SchemeId::Family,
            &[("slice".to_string(), "Ctrl+Shift+B".to_string())],
        )
        .unwrap();
        let bar = menu_bar(&scheme);
        let s = entry(&bar, "tools.slice").shortcut.clone().unwrap();
        assert!(s.ctrl && s.shift && !s.alt && s.key == "B", "{s:?}");
        // a key taken off in the settings leaves the entry with none
        let off = Scheme::with_changes(
            SchemeId::Family,
            &[("slice".to_string(), "none".to_string())],
        );
        if let Ok(off) = off {
            assert!(entry(&menu_bar(&off), "tools.slice").shortcut.is_none());
        }
    }

    #[test]
    fn the_entries_that_are_off_follow_what_there_is_on_the_bed_and_on_the_server() {
        let nothing = disabled_entries(&MenuFacts::default());
        for id in [
            "edit.undo",
            "edit.paste",
            "file.save",
            "tools.slice",
            "tools.queue",
            "edit.remove",
            "edit.arrange",
            "tools.refresh",
            "tools.emergency_stop",
            "file.export",
            "file.new",
        ] {
            assert!(nothing.iter().any(|x| x == id), "{id}");
        }
        // every id that can be off is an entry of the bar
        let bar = menu_bar(&Scheme::of(SchemeId::Family));
        let ids = bar_ids(&bar);
        for off in &nothing {
            assert!(ids.contains(off), "{off} is not in the menus");
        }
        let all = disabled_entries(&MenuFacts {
            has_parts: true,
            has_selection: true,
            several_parts: true,
            slicing: false,
            has_plan: true,
            has_server: true,
        });
        assert_eq!(
            all,
            [
                "edit.copy",
                "edit.cut",
                "edit.paste",
                "edit.redo",
                "edit.undo"
            ]
        );
        // a slice that is running cannot be started again
        let slicing = disabled_entries(&MenuFacts {
            has_parts: true,
            slicing: true,
            ..MenuFacts::default()
        });
        assert!(
            slicing.contains(&"tools.slice".to_string())
                && !slicing.contains(&"file.save".to_string())
        );
        // no plan, no export and no queue even with a server
        let no_plan = disabled_entries(&MenuFacts {
            has_parts: true,
            has_server: true,
            ..MenuFacts::default()
        });
        assert!(
            no_plan.contains(&"file.export".to_string())
                && no_plan.contains(&"tools.queue".to_string())
        );
        assert!(!no_plan.contains(&"tools.refresh".to_string()));
    }

    #[test]
    fn the_arrangement_is_written_once_when_it_changes_and_never_when_it_does_not() {
        let mut c = core();
        let mut watch = LayoutWatch::new(&c.config_pairs());
        assert!(!watch.changed(&c.config_pairs()));
        assert!(!watch.changed(&c.config_pairs()));
        c.apply(&eh3d_dock::menu::DockAction::SwitchWorkspace(
            "Review".to_string(),
        ))
        .unwrap();
        assert!(
            watch.changed(&c.config_pairs()),
            "the workspace in use is part of the lines"
        );
        assert!(!watch.changed(&c.config_pairs()));
        c.apply(&eh3d_dock::menu::DockAction::HidePanel(
            id::GCODE.to_string(),
        ))
        .unwrap();
        assert!(watch.changed(&c.config_pairs()));
        assert!(!LayoutWatch::default().changed(&[]));
    }

    #[test]
    fn the_arrangement_read_from_the_settings_gives_back_what_was_saved() {
        let mut a = core();
        a.apply(&eh3d_dock::menu::DockAction::SwitchWorkspace(
            "Run".to_string(),
        ))
        .unwrap();
        a.apply(&eh3d_dock::menu::DockAction::HidePanel(
            id::QUEUE.to_string(),
        ))
        .unwrap();
        let text = a.config_text("theme = dark\n");
        assert!(text.starts_with("theme = dark\n") && text.contains("dock.current="));
        let mut b = core();
        b.load_config(&text);
        assert!(b.warnings().is_empty(), "{:?}", b.warnings());
        assert_eq!(b.workspaces().current_name(), "Run");
        assert_eq!(b.layout_text(), a.layout_text());
        // a settings file with nothing of the frame gives the factory arrangement
        let mut c = core();
        c.load_config("theme = dark\n");
        assert_eq!(c.workspaces().current_name(), "Prepare");
        assert!(c.warnings().is_empty());
    }

    #[test]
    fn the_workspace_follows_the_program_and_the_tab_follows_the_person() {
        // nothing changed: nothing to do
        for t in Tab::ALL {
            assert_eq!(sync(t, t, workspace_of(t)), Sync::Nothing);
        }
        // the program changed the tab (the queue after a send): the workspace of the new tab is put
        assert_eq!(
            sync(Tab::Device, Tab::Preview, "Review"),
            Sync::SwitchWorkspace("Run")
        );
        assert_eq!(
            sync(Tab::Device, Tab::Preview, "Compact"),
            Sync::SwitchWorkspace("Run")
        );
        // and when the workspace is already that one, nothing
        assert_eq!(sync(Tab::Device, Tab::Preview, "Run"), Sync::Nothing);
        // the person chose a workspace: the tab is the one it stands for
        assert_eq!(
            sync(Tab::Prepare, Tab::Prepare, "Setup"),
            Sync::SetTab(Tab::Project)
        );
        // a workspace of the person that stands for no tab leaves the tab as it is
        assert_eq!(sync(Tab::Device, Tab::Device, "Compact"), Sync::Nothing);
        assert_eq!(sync(Tab::Device, Tab::Device, "my layout"), Sync::Nothing);
        // the program wins when both changed in one frame
        assert_eq!(
            sync(Tab::Project, Tab::Prepare, "Run"),
            Sync::SwitchWorkspace("Setup")
        );
    }

    #[test]
    fn a_field_for_a_path_takes_the_room_there_is_between_a_least_and_a_most() {
        assert_eq!(path_field_width(500.0, 96.0), 392.0);
        assert_eq!(
            path_field_width(100.0, 96.0),
            PATH_FIELD_MIN,
            "a narrow panel still shows a short path"
        );
        assert_eq!(path_field_width(9000.0, 96.0), PATH_FIELD_MAX);
        assert_eq!(path_field_width(f32::NAN, 96.0), PATH_FIELD_DEFAULT);
        assert_eq!(path_field_width(500.0, f32::INFINITY), PATH_FIELD_DEFAULT);
        assert_eq!(path_field_width(f32::INFINITY, 0.0), PATH_FIELD_DEFAULT);
        // a reserved width below zero reserves nothing
        assert_eq!(path_field_width(500.0, -50.0), 488.0);
        // the old fields were 360 and 150 points wide: the new one is wider than both on a panel of the usual width
        assert!(path_field_width(520.0, 96.0) > 360.0);
    }

    #[test]
    fn a_rectangle_is_cut_into_what_is_left_and_a_strip_on_the_right() {
        let (left, strip) = split_right([0.0, 10.0, 300.0, 410.0], 86.0);
        assert_eq!(left, [0.0, 10.0, 214.0, 410.0]);
        assert_eq!(strip, [214.0, 10.0, 300.0, 410.0]);
        // a strip wider than the rectangle is all of it
        let (left, strip) = split_right([5.0, 0.0, 50.0, 40.0], 86.0);
        assert_eq!(left, [5.0, 0.0, 5.0, 40.0]);
        assert_eq!(strip, [5.0, 0.0, 50.0, 40.0]);
        // no width, no strip
        for w in [0.0, -3.0, f32::NAN] {
            let (left, strip) = split_right([0.0, 0.0, 100.0, 100.0], w);
            assert_eq!(left, [0.0, 0.0, 100.0, 100.0], "{w}");
            assert_eq!(strip[2] - strip[0], 0.0, "{w}");
        }
    }
}

#[cfg(test)]
mod about_tests {
    use super::*;

    #[test]
    fn the_window_about_has_the_name_the_repository_and_the_picture_of_this_program() {
        let brand = about_brand();
        assert_eq!(brand.program, Phoenix::Printer3d);
        assert_eq!(brand.program.name(), "EH3D-PHOENIX-3DP");
        assert_eq!(
            brand.program.repository(),
            "https://github.com/JuanenRac/EH3D-PHOENIX-3DP"
        );
        let png = brand.logo_png;
        assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
        // the width and the height in the header of the picture
        assert_eq!(png.get(16..24), Some(&[0, 0, 1, 0, 0, 0, 1, 0][..]));
        // eight bits a channel with alpha, which is what the icon of the title bar is read from
        assert_eq!(png.get(24..26), Some(&[8, 6][..]));
    }
}
