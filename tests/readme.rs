//! What the READMEs say about the program is true: the number of tests is the real one, and the commands they show are commands of the program.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn count_tests_in(dir: &Path, total: &mut usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name != "target" && name != ".git" && name != "fixtures" {
                count_tests_in(&p, total);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            *total += text.lines().filter(|l| l.trim() == "#[test]").count();
        }
    }
}

/// The number of tests written in the repository.
fn tests_in_repo() -> usize {
    let mut total = 0;
    for d in ["src", "tests"] {
        count_tests_in(&root().join(d), &mut total);
    }
    count_tests_in(&root().join("crates"), &mut total);
    total
}

const NAMES: [&str; 8] = [
    "README.md",
    "README_spa.md",
    "README_deu.md",
    "README_fra.md",
    "README_ita.md",
    "README_jpn.md",
    "README_por.md",
    "README_zho.md",
];

/// The number of tests that the table "At a glance" of a README states: the third value row.
fn stated(readme: &str) -> usize {
    let start = readme.find("## 📋").expect("the At a glance section");
    let end = readme.find("## 🚦").expect("the Project status section");
    let rows: Vec<&str> = readme[start..end]
        .lines()
        .filter(|l| l.starts_with('|'))
        .collect();
    let cells: Vec<&str> = rows[4].split('|').map(str::trim).collect();
    cells[2]
        .parse()
        .unwrap_or_else(|_| panic!("the third row is not a number: {}", rows[4]))
}

#[test]
fn every_readme_states_the_real_number_of_tests() {
    let real = tests_in_repo();
    assert!(real >= 200, "{real}");
    for name in NAMES {
        let text =
            std::fs::read_to_string(root().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            stated(&text),
            real,
            "{name} states another number of tests than the {real} that exist"
        );
    }
}

#[test]
fn the_readmes_show_only_commands_the_program_has() {
    let known = [
        "printers",
        "machines",
        "machine",
        "import",
        "info",
        "check-profile",
        "convert",
        "slice",
        "filaments",
        "resins",
        "project",
        "version",
        "gui",
    ];
    for name in NAMES {
        let text = std::fs::read_to_string(root().join(name)).unwrap();
        for line in text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("phoenix-3dp "))
        {
            let word = line.split_whitespace().next().unwrap_or("");
            assert!(known.contains(&word), "{name}: {word} is not a command");
        }
    }
}

#[test]
fn the_scripts_exist_and_the_version_agrees() {
    for f in [
        "build.sh",
        "build.bat",
        "test.sh",
        "test.bat",
        "run.sh",
        "run.bat",
        "rust-toolchain.toml",
        "LICENSE",
        "CHANGELOG.md",
    ] {
        assert!(root().join(f).is_file(), "{f}");
    }
    let manifest = std::fs::read_to_string(root().join("eh3d.project.json")).unwrap();
    assert!(manifest.contains(&format!("\"version\": \"{}\"", eh3d_phoenix_3dp::VERSION)));
    let changelog = std::fs::read_to_string(root().join("CHANGELOG.md")).unwrap();
    assert!(changelog.contains(&format!("## [{}]", eh3d_phoenix_3dp::VERSION)));
}
