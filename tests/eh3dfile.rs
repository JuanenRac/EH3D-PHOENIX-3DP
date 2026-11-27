//! The branch `3dp/` of an `.eh3d` project: what is saved comes back as it was, every other branch (known or not) keeps its bytes, a copy taken from another branch is told as out of date
//! when its source changes, an imported file is kept with a report, and a damaged file is an error and never a crash. Every project here is made up.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use eh3d_container::AtomicOptions;
use eh3d_phoenix_3dp::cli;
use eh3d_phoenix_3dp::eh3dfile::{self, Contents, Held, Identity, Part, BRANCH, PLATE};
use eh3d_project::{BranchUpdate, NewProject, OpenOptions, Project, SaveOptions, Writer};
use p3dp_core::mesh::cube;
use p3dp_core::{Limits, Mesh};
use p3dp_formats::{read_mesh, write_mesh, Format};
use p3dp_slicer::SliceConfig;

/// A folder of its own for one test.
fn folder(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("eh3dfile_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn stl(size: f64) -> Vec<u8> {
    write_mesh(&cube(size), Format::Stl, "").unwrap()
}

/// The mesh the program reads from the STL of a cube.
fn read_stl(size: f64) -> Mesh {
    read_mesh(&stl(size), Some("cube.stl"), &Limits::default()).unwrap()
}

fn file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn write_branch(path: &Path, branch: &str, entries: &[(&str, &[u8])]) {
    let project = Project::open(path, &OpenOptions::default()).unwrap();
    let mut update = BranchUpdate::new(branch, "1.0", "0.1.0");
    for (rel, bytes) in entries {
        update = update.with_entry(rel, bytes);
    }
    project
        .save_branch(
            &Writer::new(branch, &[branch]),
            update,
            &SaveOptions::default(),
        )
        .unwrap();
}

/// A project with a branch of another program and one this program does not know.
fn foreign(path: &Path) {
    let new = NewProject {
        id: "00000000-0000-4000-8000-000000000032".into(),
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
    write_branch(
        path,
        "cad",
        &[("box.stl", &stl(8.0)), ("part.step", b"made up")],
    );
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

fn part(name: &str, size: f64) -> Part {
    Part {
        name: name.to_string(),
        mesh: cube(size),
        pos: Some([100.5, 60.25]),
        rot_deg: [0.0, 0.0, 33.3],
        scale: [1.0, 1.5, 2.0],
    }
}

fn work() -> Contents {
    let mut settings = SliceConfig::default();
    settings.set("perimeters", "3").unwrap();
    Contents {
        parts: Some(vec![
            part("a #1.stl", 10.0),
            part("b.stl", 12.5),
            part("a #1.stl", 10.0),
        ]),
        settings: Some(settings),
        printer: Some("bambu-lab-a1".into()),
        filament: None,
        program: Some(Held {
            name: "plate.gcode".into(),
            bytes: b"G28\n".to_vec(),
        }),
        ..Contents::default()
    }
}

fn save(path: &Path, contents: &Contents) -> Option<Vec<String>> {
    eh3dfile::save_contents(path, contents, &Identity::default(), &Limits::default()).unwrap()
}

#[test]
fn what_is_saved_comes_back_and_saving_it_again_writes_nothing() {
    let dir = folder("roundtrip");
    let project = dir.join("job.eh3d");
    foreign(&project);
    let kept = others(&project);
    assert!(save(&project, &work()).is_some());
    assert_eq!(others(&project), kept);
    let held = eh3dfile::held(&project, &Limits::default()).unwrap();
    let wanted = work();
    assert_eq!(Some(held.parts.clone()), wanted.parts);
    assert_eq!(held.settings, wanted.settings);
    assert_eq!(held.printer, wanted.printer);
    assert_eq!(held.program.as_deref(), Some("programs/plate.gcode"));
    // the two parts of the same mesh share one file
    let plates = held.files.keys().filter(|n| n.ends_with(".3mf")).count();
    assert_eq!(plates, 2);
    let bytes = fs::read(&project).unwrap();
    assert!(save(&project, &wanted).is_none());
    assert_eq!(fs::read(&project).unwrap(), bytes);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_work_that_changed_loses_its_old_g_code_and_keeps_the_rest() {
    let dir = folder("stale");
    let project = dir.join("job.eh3d");
    save(&project, &work());
    let changed = Contents {
        printer: Some("bambu-lab-p1s".into()),
        ..Contents::default()
    };
    assert!(save(&project, &changed).is_some());
    let held = eh3dfile::held(&project, &Limits::default()).unwrap();
    assert!(held.program.is_none());
    assert!(!held.files.keys().any(|n| n.starts_with("programs/")));
    assert_eq!(held.parts.len(), 3);
    assert_eq!(held.printer.as_deref(), Some("bambu-lab-p1s"));
    assert!(eh3dfile::save_contents(
        &project,
        &Contents::default(),
        &Identity::default(),
        &Limits::default()
    )
    .is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_imported_model_is_kept_with_a_report_and_is_the_part_of_the_bed() {
    let dir = folder("import");
    let project = dir.join("job.eh3d");
    let source = file(&dir, "bracket.stl", &stl(15.0));
    let limits = Limits::default();
    let (report, _) =
        eh3dfile::import_file(&project, &source, &Identity::default(), &limits).unwrap();
    assert_eq!(report.source.name, "bracket.stl");
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert_eq!(held.files["origin/bracket.stl"], stl(15.0));
    assert_eq!(
        held.parts,
        vec![Part::loaded("bracket.stl", read_stl(15.0))]
    );
    // the record of the bed refers to the original and writes no second copy
    assert!(!held.files.keys().any(|n| n.ends_with(".3mf")));

    // a program replaces the import: the part stays on the bed
    let program = file(&dir, "bracket.gcode", b"G28\nG1 X10\n");
    eh3dfile::import_file(&project, &program, &Identity::default(), &limits).unwrap();
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert_eq!(held.program.as_deref(), Some("origin/bracket.gcode"));
    assert_eq!(
        held.parts,
        vec![Part::loaded("bracket.stl", read_stl(15.0))]
    );
    assert!(!held.files.contains_key("origin/bracket.stl"));

    for (name, bytes) in [("notes.txt", &b"x"[..]), ("broken.stl", &b"garbage"[..])] {
        let bad = file(&dir, name, bytes);
        let before = fs::read(&project).unwrap();
        assert!(
            eh3dfile::import_file(&project, &bad, &Identity::default(), &limits).is_err(),
            "{name}"
        );
        assert_eq!(fs::read(&project).unwrap(), before);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_model_taken_from_cad_keeps_its_reference_until_the_part_is_gone() {
    let dir = folder("take");
    let project = dir.join("job.eh3d");
    foreign(&project);
    let limits = Limits::default();
    let (target, _) = eh3dfile::take(&project, "cad", "box.stl", &limits).unwrap();
    assert_eq!(target, "taken/cad/box.stl");
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert_eq!(held.parts, vec![Part::loaded("box.stl", read_stl(8.0))]);
    // a file the program cannot read is kept and is no part
    eh3dfile::take(&project, "cad", "part.step", &limits).unwrap();
    assert_eq!(eh3dfile::held(&project, &limits).unwrap().parts.len(), 1);

    // placing the part keeps the reference to its source
    let mut placed = held.parts.clone();
    placed[0].pos = Some([20.0, 20.0]);
    save(
        &project,
        &Contents {
            parts: Some(placed),
            ..Contents::default()
        },
    );
    write_branch(
        &project,
        "cad",
        &[("box.stl", &stl(9.0)), ("part.step", b"made up")],
    );
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert!(
        held.warnings.iter().any(|w| w.starts_with("OUT OF DATE")),
        "{:?}",
        held.warnings
    );
    // with the part gone, its copy and its reference go too
    save(
        &project,
        &Contents {
            parts: Some(vec![part("other.stl", 5.0)]),
            ..Contents::default()
        },
    );
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert!(!held.files.contains_key("taken/cad/box.stl"));
    assert!(eh3dfile::take(&project, "3dp", PLATE, &limits).is_err());
    assert!(eh3dfile::take(&project, "cad", "nothing.stl", &limits).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_record_of_the_bed_that_is_not_valid_is_refused() {
    let dir = folder("badrecord");
    let project = dir.join("job.eh3d");
    save(&project, &work());
    let limits = Limits::default();
    for record in [
        "not json",
        "{}",
        r#"{"parts":[{"file":"plate/gone.3mf","name":"a","pos":null,"rot":[0,0,0],"scale":[1,1,1]}]}"#,
        r#"{"parts":[{"file":"plate/part-01.3mf","name":"a","pos":null,"rot":[0,0,0],"scale":[0,1,1]}]}"#,
        r#"{"parts":[{"file":"plate/part-01.3mf","name":"a","pos":[1e9,0],"rot":[0,0,0],"scale":[1,1,1]}]}"#,
    ] {
        // the files of the branch as they are, whether or not the record can be read
        let mut opened = Project::open(&project, &OpenOptions::default()).unwrap();
        let names: Vec<String> = opened
            .branch(BRANCH)
            .unwrap()
            .files
            .keys()
            .filter(|n| n.as_str() != "branch.json")
            .cloned()
            .collect();
        let mut files: Vec<(String, Vec<u8>)> = names
            .into_iter()
            .map(|n| {
                let bytes = opened.read_entry(BRANCH, &n).unwrap();
                (n, bytes)
            })
            .collect();
        drop(opened);
        for (name, bytes) in &mut files {
            if name == PLATE {
                *bytes = record.as_bytes().to_vec();
            }
        }
        let entries: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        write_branch(&project, BRANCH, &entries);
        assert!(eh3dfile::held(&project, &limits).is_err(), "{record}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_command_saves_lists_opens_and_extracts_and_a_damaged_file_is_an_error() {
    let dir = folder("command");
    let project = dir.join("job.eh3d");
    let model = file(&dir, "cube.stl", &stl(10.0));
    let go = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        cli::run(&args, None)
    };
    let (p, m) = (text(&project), text(&model));
    let saved = go(&[
        "project",
        "save",
        &p,
        "--model",
        &m,
        "--printer",
        "bambu-lab-a1",
    ]);
    assert_eq!(saved.code, 0, "{}", saved.err);
    assert!(saved.out.contains("was saved"));
    let again = go(&["project", "save", &p, "--printer", "bambu-lab-a1"]);
    assert!(again.out.contains("nothing has changed"), "{}", again.out);
    let list = go(&["project", "list", &p]);
    assert!(list.out.contains("3dp"), "{}", list.out);
    assert!(go(&["project", "list", &p, "--json"])
        .out
        .contains("\"branches\""));
    let opened = go(&["project", "open", &p]);
    assert!(
        opened.out.contains("part cube.stl") && opened.out.contains("printer bambu-lab-a1"),
        "{}",
        opened.out
    );
    let out = text(&dir.join("out"));
    assert_eq!(go(&["project", "extract", &p, "3dp", &out]).code, 0);
    assert_eq!(go(&["project", "extract", &p, "3dp", &out]).code, 2);

    // a settings file with anything else than the keys of the slicer, such as the token of the server, never goes into a project
    let sample = ["abc", "123"].concat();
    let own = file(
        &dir,
        "own.cfg",
        format!("server_token = {sample}\n").as_bytes(),
    );
    let before = fs::read(&project).unwrap();
    let refused = go(&["project", "save", &p, "--settings", &text(&own)]);
    assert_eq!(refused.code, 2);
    assert!(!refused.err.contains(&sample), "{}", refused.err);
    assert_eq!(fs::read(&project).unwrap(), before);

    for words in [
        vec!["project", "save", p.as_str()],
        vec!["project", "save", p.as_str(), "--printer", "ghost"],
        vec!["project", "save", p.as_str(), "--model", "no-such-file.stl"],
        vec!["project", "open", p.as_str(), "--wrong"],
        vec!["project", "take", p.as_str(), "cad"],
    ] {
        assert_eq!(go(&words).code, 2, "{words:?}");
    }
    let junk = text(&file(&dir, "junk.eh3d", b"not a project"));
    assert_eq!(go(&["project", "list", &junk]).code, 2);
    assert_eq!(go(&["project", "open", &junk]).code, 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_start_and_end_gcode_are_kept_in_the_branch_replaced_and_taken_out() {
    let dir = folder("blocks");
    let project = dir.join("job.eh3d");
    let limits = Limits::default();
    save(&project, &work());
    let blocks = Contents {
        start_gcode: Some("G92 E0\nG1 E5\n".into()),
        end_gcode: Some("G28 X Y\n".into()),
        ..Contents::default()
    };
    assert!(save(&project, &blocks).is_some());
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert_eq!(held.blocks.start.as_deref(), Some("G92 E0\nG1 E5\n"));
    assert_eq!(held.blocks.end.as_deref(), Some("G28 X Y\n"));
    assert!(
        held.program.is_none(),
        "the G-code of before was not made with these lines"
    );
    assert_eq!(held.parts.len(), 3);
    assert!(
        save(&project, &blocks).is_none(),
        "the same is not written again"
    );
    // one is taken out with an empty text and the other stays
    let gone = Contents {
        start_gcode: Some(String::new()),
        ..Contents::default()
    };
    assert!(save(&project, &gone).is_some());
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert!(held.blocks.start.is_none() && held.blocks.end.is_some());
    assert!(!held.files.contains_key("blocks/start.gcode"));
    assert!(save(&project, &gone).is_none());
    // what no slice would take does not go in
    let before = fs::read(&project).unwrap();
    for bad in [
        "G4 S1\n".repeat(201),
        "G".repeat(201),
        "G28\u{0}\n".to_string(),
    ] {
        let contents = Contents {
            start_gcode: Some(bad),
            ..Contents::default()
        };
        assert!(
            eh3dfile::save_contents(&project, &contents, &Identity::default(), &limits).is_err()
        );
    }
    assert_eq!(fs::read(&project).unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_command_checks_the_temperatures_of_the_gcode_of_a_project() {
    let dir = folder("heat");
    let project = dir.join("job.eh3d");
    let hot = file(&dir, "hot.gcode", b"G28\nM104 S999\n");
    let fine = file(&dir, "fine.gcode", b"G28\nM104 S200\n");
    let end = file(&dir, "end.gcode", b"G28 X Y\n");
    let go = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        cli::run(&args, None)
    };
    let p = text(&project);
    let saved = go(&["project", "save", &p, "--end-gcode", &text(&end)]);
    assert_eq!(saved.code, 0, "{}", saved.err);
    // no printer is known yet: it is said, and nothing is claimed
    let imported = go(&["project", "import", &p, &text(&hot)]);
    assert!(
        imported.out.contains("temperatures not checked"),
        "{} {}",
        imported.out,
        imported.err
    );
    let opened = go(&["project", "open", &p, "--printer", "bambu-lab-a1"]);
    assert_eq!(opened.code, 1, "{}", opened.err);
    assert!(
        opened.out.contains("line 2: ") && opened.out.contains("M104 S999"),
        "{}",
        opened.out
    );
    assert!(opened.err.contains("past the limits of bambu-lab-a1"));
    assert!(
        opened.out.contains("start G-code none") && opened.out.contains("end G-code 1 line(s)"),
        "{}",
        opened.out
    );
    // with the printer in the project, an import is checked against it
    assert_eq!(
        go(&["project", "save", &p, "--printer", "bambu-lab-a1"]).code,
        0
    );
    let again = go(&["project", "import", &p, &text(&hot)]);
    assert_eq!(again.code, 1, "{} {}", again.out, again.err);
    let good = go(&["project", "import", &p, &text(&fine)]);
    assert!(
        good.out
            .contains("no temperature command goes past the limits of bambu-lab-a1"),
        "{}",
        good.out
    );
    assert_eq!(go(&["project", "open", &p, "--printer", "ghost"]).code, 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_program_that_is_not_text_is_not_said_to_be_checked() {
    let catalog = p3dp_profiles::Catalog::builtin().unwrap();
    let a1 = catalog.find("bambu-lab-a1");
    let binary = eh3dfile::review("x.gcode", b"GCDE\x01\x00\x00\x00", a1);
    assert_eq!(binary.heat, eh3dfile::Heat::NotText);
    let resin = catalog.find("elegoo-mars-4");
    assert_eq!(
        eh3dfile::review("x.gcode", b"M104 S999\n", resin).heat,
        eh3dfile::Heat::NoPrinter
    );
    let r = eh3dfile::review("x.gcode", b"G28\nM104 S999\n", a1);
    assert_eq!(r.lines, 2);
    assert!(
        matches!(r.heat, eh3dfile::Heat::Checked { ref violations, .. } if violations.len() == 1)
    );
}

#[test]
fn the_printer_profile_of_a_project_is_kept_and_only_ever_lowers_the_limits() {
    let dir = folder("profile");
    let project = dir.join("job.eh3d");
    let limits = Limits::default();
    let catalog = p3dp_profiles::suite_overlay().unwrap().catalog;
    let own = catalog.find("bambu-lab-a1").unwrap().clone();
    let mut mine = own.clone();
    let l = own.limits.unwrap();
    mine.limits = Some(p3dp_core::TempLimits {
        nozzle_c: 230.0,
        bed_c: l.bed_c + 40.0,
        chamber_c: l.chamber_c,
    });
    mine.limit_source = p3dp_profiles::LimitSource::Owner;
    mine.verified = false;
    let profile = file(&dir, "mine.printer", mine.to_text().as_bytes());
    let hot = file(
        &dir,
        "hot.gcode",
        format!("G28\nM104 S250\nM140 S{}\n", l.bed_c + 30.0).as_bytes(),
    );
    let go = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        cli::run(&args, None)
    };
    let p = text(&project);
    let saved = go(&["project", "save", &p, "--printer-file", &text(&profile)]);
    assert_eq!(saved.code, 0, "{}", saved.err);
    let held = eh3dfile::held(&project, &limits).unwrap();
    assert_eq!(held.profile.as_ref(), Some(&mine));
    // the printer of the project is the one of the profile
    assert_eq!(held.printer.as_deref(), Some("bambu-lab-a1"));
    let looked = go(&["project", "import", &p, &text(&hot)]);
    assert_eq!(looked.code, 1, "{} {}", looked.out, looked.err);
    // 250 C is fine for the printer and not for the profile; the higher bed of the profile is not followed
    assert!(looked
        .out
        .contains("the profile of the project lowers: nozzle_max_c 230"));
    assert!(looked.err.contains("bed_max_c") && looked.err.contains("not followed"));
    assert!(looked.out.contains("M104 S250"), "{}", looked.out);
    assert!(looked.out.contains("M140 S"), "{}", looked.out);
    // a profile that is not one is refused and nothing is written
    let before = fs::read(&project).unwrap();
    let junk = file(&dir, "junk.printer", b"id = x\nnozzle_max_c = 999\n");
    assert_eq!(
        go(&["project", "save", &p, "--printer-file", &text(&junk)]).code,
        2
    );
    assert_eq!(fs::read(&project).unwrap(), before);
    // an empty file takes it out
    let empty = file(&dir, "empty.printer", b"");
    assert_eq!(
        go(&["project", "save", &p, "--printer-file", &text(&empty)]).code,
        0
    );
    assert!(eh3dfile::held(&project, &limits).unwrap().profile.is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_command_lists_the_layers_and_tells_what_leaves_the_bed_or_acts_on_the_machine() {
    let dir = folder("layers");
    let project = dir.join("job.eh3d");
    let program = file(
        &dir,
        "moves.gcode",
        b"G28\nM211 S0\nM83\nG1 Z0.2\nG1 X10 Y10\nG1 X20 E1\nG1 Z0.4\nG1 X999 E1\n",
    );
    let go = |words: &[&str]| {
        let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        cli::run(&args, None)
    };
    let p = text(&project);
    assert_eq!(
        go(&[
            "project",
            "save",
            &p,
            "--printer",
            "bambu-lab-a1",
            "--gcode",
            &text(&program)
        ])
        .code,
        0
    );
    let opened = go(&["project", "open", &p]);
    assert_eq!(opened.code, 1, "{}", opened.err);
    assert!(opened
        .out
        .contains("line 8: X999.000 is outside the build volume"));
    assert!(opened
        .out
        .contains("turns off or raises a protection (M211 S0)"));
    assert!(opened.err.contains("leave the build volume"));
    let layers = go(&["project", "layers", &p]);
    assert_eq!(layers.code, 1, "{}", layers.err);
    assert!(layers.out.contains("2 layer(s)"), "{}", layers.out);
    assert!(layers.out.contains("layer 2: z 0.400 mm"));
    let one = go(&["project", "layers", &p, "--layer", "1"]);
    assert!(one.out.contains("layer 1:") && !one.out.contains("layer 2:"));
    assert_eq!(go(&["project", "layers", &p, "--layer", "3"]).code, 2);
    assert_eq!(go(&["project", "layers", &p, "--layer", "x"]).code, 2);
    // the same for a loose file against a printer
    let checked = go(&["check-profile", "bambu-lab-a1", "--gcode", &text(&program)]);
    assert_eq!(checked.code, 1, "{}", checked.out);
    assert!(checked.out.contains("is outside the build volume"));
    assert!(checked.err.contains("act on the machine itself"));
    let _ = fs::remove_dir_all(&dir);
}
