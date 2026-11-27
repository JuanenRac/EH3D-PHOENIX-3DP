//! The picture mode of the window (`phoenix-3dp gui --shot FILE.png`): what is asked for, read and checked without a screen, and the file that is written. The window opens at the
//! size asked, loads the model given with `--open`, waits a number of frames so that the drawing is complete, saves what it drew as a PNG file and closes by itself. In this mode the
//! window keeps no settings and reaches for no network, so a run leaves nothing of the person's changed.

use crate::app_state::Tab;
use crate::i18n::Lang;
use std::path::{Path, PathBuf};

/// The printer a demonstration starts with when none is given: the first of the two machines of the server made in memory, so the Prepare and the Device windows show the same printer.
pub const DEMO_PRINTER: &str = "bambu-lab-p1s";

/// The printer to start with: the one asked for with `--printer`, else, in a demonstration, the one of the demonstration server.
pub fn start_printer(args: &WindowArgs) -> Option<String> {
    match (&args.printer, args.demo) {
        (Some(id), _) => Some(id.clone()),
        (None, true) => Some(DEMO_PRINTER.to_string()),
        (None, false) => None,
    }
}

/// The smallest side of the window of a picture, in pixels.
pub const SIZE_MIN: u32 = 320;
/// The largest side of the window of a picture, in pixels.
pub const SIZE_MAX: u32 = 8192;
/// The size of the window of a picture when `--size` is not given.
pub const DEFAULT_SIZE: (u32, u32) = (1280, 800);
/// The frames drawn before the picture is taken when `--wait` is not given.
pub const DEFAULT_WAIT: u32 = 6;
/// The most frames `--wait` may ask for.
pub const WAIT_MAX: u32 = 3600;
/// The frames after the one asked for in which the picture may still come back; after them the run fails.
pub const PATIENCE_FRAMES: u32 = 600;

/// What the picture was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotRequest {
    pub output: PathBuf,
    /// True when a file that exists may be replaced; otherwise it is an error.
    pub replace: bool,
    pub size: (u32, u32),
    pub wait: u32,
}

/// What the window was asked for on the command line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowArgs {
    /// The settings file (a bare argument).
    pub config: Option<PathBuf>,
    /// The model to load at once (`--open`).
    pub model: Option<PathBuf>,
    pub lang: Option<Lang>,
    pub shot: Option<ShotRequest>,
    /// The tab the picture shows (`--window`).
    pub window: Option<Tab>,
    /// The printer to start with (`--printer`).
    pub printer: Option<String>,
    /// Slice the model before the picture, so the Preview tab has a plan (`--slice`).
    pub slice: bool,
    /// Use a server made in memory with two printers instead of a network (`--demo`): the Device tab has something to show without a server.
    pub demo: bool,
    /// Show the inside of the plan, with the top and bottom skins hidden, in the Preview tab (`--peek`).
    pub peek: bool,
    /// A panel to put in front for the picture (`--panel project`), when it is a tab behind another one.
    pub panel: Option<String>,
}

/// Reads a size written `WIDTHxHEIGHT` (`1280x720`, also with a capital X), each side between [`SIZE_MIN`] and [`SIZE_MAX`].
pub fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.trim().split_once(['x', 'X'])?;
    let width: u32 = width.trim().parse().ok()?;
    let height: u32 = height.trim().parse().ok()?;
    let ok = |side: u32| (SIZE_MIN..=SIZE_MAX).contains(&side);
    (ok(width) && ok(height)).then_some((width, height))
}

/// Whether the arguments ask for the picture mode.
pub fn wants_picture(args: &[String]) -> bool {
    args.iter().any(|a| a == "--shot")
}

/// Reads the arguments of the window (after the word `gui`). The answer for a mistake is the reason.
pub fn parse_window_args(args: &[String]) -> Result<WindowArgs, String> {
    let mut parsed = WindowArgs::default();
    let mut output: Option<PathBuf> = None;
    let (mut size, mut wait, mut replace) = (None, None, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--shot" => {
                let path = iter.next().ok_or("--shot needs a file to write")?;
                if !path.to_ascii_lowercase().ends_with(".png") {
                    return Err("--shot writes a PNG file: the name must end in .png".to_owned());
                }
                output = Some(PathBuf::from(path));
            }
            "--size" => {
                let text = iter.next().ok_or("--size needs WIDTHxHEIGHT")?;
                size = Some(parse_size(text).ok_or(
                    "--size is WIDTHxHEIGHT in pixels, each side from 320 to 8192, as 1280x720",
                )?);
            }
            "--wait" => {
                let text = iter.next().ok_or("--wait needs a number of frames")?;
                wait = Some(
                    text.parse::<u32>()
                        .ok()
                        .filter(|n| *n <= WAIT_MAX)
                        .ok_or("--wait is a number of frames from 0 to 3600")?,
                );
            }
            "--lang" => {
                let tag = iter.next().ok_or("--lang needs a language code")?;
                parsed.lang = Some(Lang::from_code(tag).ok_or("unknown language code")?);
            }
            "--window" => {
                let text = iter.next().ok_or("--window needs the name of a tab")?;
                parsed.window = Some(
                    Tab::from_key(text).ok_or("--window is prepare, preview, device or project (or the name of the tab in a language of the program, as preparar, vista previa or dispositivo)")?,
                );
            }
            "--open" => {
                let path = iter.next().ok_or("--open needs a model")?;
                if parsed.model.replace(PathBuf::from(path)).is_some() {
                    return Err("only one model can be opened".to_owned());
                }
            }
            "--printer" => {
                let id = iter.next().ok_or("--printer needs the id of a profile")?;
                parsed.printer = Some(id.to_ascii_lowercase());
            }
            "--panel" => {
                let id = iter.next().ok_or("--panel needs the name of a panel")?;
                parsed.panel = Some(id.to_ascii_lowercase());
            }
            "--replace" => replace = true,
            "--slice" => parsed.slice = true,
            "--demo" => parsed.demo = true,
            "--peek" => parsed.peek = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => {
                if parsed.config.replace(PathBuf::from(other)).is_some() {
                    return Err("only one settings file can be given".to_owned());
                }
            }
        }
    }
    parsed.shot = match output {
        Some(output) => Some(ShotRequest {
            output,
            replace,
            size: size.unwrap_or(DEFAULT_SIZE),
            wait: wait.unwrap_or(DEFAULT_WAIT),
        }),
        None => {
            if size.is_some() || wait.is_some() || replace {
                return Err("--size, --wait and --replace need --shot".to_owned());
            }
            None
        }
    };
    if parsed.shot.is_none()
        && (parsed.window.is_some() || parsed.slice || parsed.demo || parsed.peek)
    {
        return Err("--window, --slice, --demo and --peek need --shot".to_owned());
    }
    if parsed.shot.is_none() && parsed.panel.is_some() {
        return Err("--panel needs --shot".to_owned());
    }
    if parsed.slice && parsed.model.is_none() && !parsed.demo {
        return Err("--slice needs a model: give --open or --demo".to_owned());
    }
    Ok(parsed)
}

/// Checks what has to be true before the window opens for a picture: the file must not exist unless `--replace`, its folder must, and the model must be a file.
pub fn check_request(args: &WindowArgs) -> Result<(), String> {
    let Some(r) = &args.shot else {
        return Ok(());
    };
    if !r.replace && r.output.exists() {
        return Err(format!(
            "{} exists; give --replace to write over it",
            r.output.display()
        ));
    }
    let folder = r
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    if folder.is_some_and(|dir| !dir.is_dir()) {
        return Err(format!(
            "the folder of {} does not exist",
            r.output.display()
        ));
    }
    if let Some(m) = &args.model {
        if !m.is_file() {
            return Err(format!("{} is not a file", m.display()));
        }
    }
    Ok(())
}

/// Writes the picture as a PNG file, a new file unless `replace` is true. `rgba` is the pixels row after row.
pub fn write_png(
    path: &Path,
    replace: bool,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    use std::io::Write;
    let bytes =
        p3dp_formats::png_write::encode_rgba(width, height, rgba).map_err(|e| e.to_string())?;
    let mut open = std::fs::OpenOptions::new();
    open.write(true);
    if replace {
        open.create(true).truncate(true);
    } else {
        open.create_new(true);
    }
    let mut file = open
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    fn temp(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("p3dp_shot_{}_{name}", std::process::id()));
        p
    }

    #[test]
    fn sizes_are_width_by_height_inside_the_limits() {
        assert_eq!(parse_size("1280x720"), Some((1280, 720)));
        assert_eq!(parse_size(" 320 X 8192 "), Some((320, 8192)));
        for bad in [
            "",
            "1280",
            "1280x",
            "x720",
            "319x720",
            "1280x8193",
            "axb",
            "-1x5",
            "1280x720x3",
        ] {
            assert_eq!(parse_size(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_plain_window_has_no_picture_and_the_picture_has_its_defaults() {
        let w = parse_window_args(&[]).unwrap();
        assert_eq!(w, WindowArgs::default());
        let w = parse_window_args(&args("my.cfg")).unwrap();
        assert_eq!(w.config, Some(PathBuf::from("my.cfg")));
        let w = parse_window_args(&args("--shot out.png")).unwrap();
        let s = w.shot.unwrap();
        assert_eq!(
            (s.size, s.wait, s.replace),
            (DEFAULT_SIZE, DEFAULT_WAIT, false)
        );
        assert!(wants_picture(&args("gui --shot x.png")) && !wants_picture(&args("gui my.cfg")));
    }

    #[test]
    fn every_option_is_read() {
        let w = parse_window_args(&args(
            "my.cfg --shot out.PNG --size 1000x600 --wait 12 --replace --window preview --open cube.stl --printer PRUSA-MK4S --slice --demo --peek --panel Project --lang pt",
        ))
        .unwrap();
        let s = w.shot.clone().unwrap();
        assert_eq!(s.output, PathBuf::from("out.PNG"));
        assert_eq!((s.size, s.wait, s.replace), ((1000, 600), 12, true));
        assert_eq!(w.window, Some(Tab::Preview));
        assert_eq!(w.model, Some(PathBuf::from("cube.stl")));
        assert_eq!(w.printer.as_deref(), Some("prusa-mk4s"));
        assert!(w.slice && w.demo && w.peek);
        assert_eq!(w.panel.as_deref(), Some("project"));
        assert_eq!(w.lang, Some(Lang::Pt));
        assert_eq!(w.config, Some(PathBuf::from("my.cfg")));
    }

    #[test]
    fn a_demonstration_is_a_model_to_slice_and_the_name_of_a_tab_in_any_language_is_read() {
        let a =
            parse_window_args(&args("--shot a.png --slice --demo --window vista-previa")).unwrap();
        assert!(a.slice && a.demo);
        assert_eq!(a.window, Some(Tab::Preview));
        let b = parse_window_args(&args("--shot b.png --window dispositivo --demo")).unwrap();
        assert_eq!(b.window, Some(Tab::Device));
        let c = parse_window_args(&args("--shot c.png --window preparar --demo")).unwrap();
        assert_eq!(c.window, Some(Tab::Prepare));
        assert!(parse_window_args(&args("--shot a.png --slice")).is_err());
    }

    #[test]
    fn mistakes_are_told_with_the_reason() {
        for (text, want) in [
            ("--shot", "needs a file"),
            ("--shot out.jpg", "must end in .png"),
            ("--shot a.png --size 10x10", "--size is WIDTHxHEIGHT"),
            ("--shot a.png --wait 9999", "0 to 3600"),
            ("--shot a.png --wait x", "0 to 3600"),
            (
                "--shot a.png --window cnc",
                "prepare, preview, device or project",
            ),
            ("--shot a.png --lang xx", "unknown language"),
            ("--shot a.png --bogus", "unknown option --bogus"),
            ("--size 800x600", "need --shot"),
            ("--wait 3", "need --shot"),
            ("--replace", "need --shot"),
            ("--window device", "need --shot"),
            ("--demo", "need --shot"),
            ("--peek", "need --shot"),
            ("--shot a.png --slice", "give --open"),
            ("--shot a.png --open a.stl --open b.stl", "only one model"),
            ("one.cfg two.cfg", "only one settings file"),
            ("--open", "needs a model"),
            ("--printer", "needs the id"),
            ("--panel", "needs the name of a panel"),
            ("--panel project", "--panel needs --shot"),
        ] {
            let e = parse_window_args(&args(text)).unwrap_err();
            assert!(e.contains(want), "{text:?} -> {e}");
        }
    }

    #[test]
    fn the_request_is_checked_before_the_window_opens() {
        let dir = temp("dir");
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("a.png");
        let mk = |replace: bool, model: Option<PathBuf>| WindowArgs {
            shot: Some(ShotRequest {
                output: out.clone(),
                replace,
                size: DEFAULT_SIZE,
                wait: 1,
            }),
            model,
            ..WindowArgs::default()
        };
        assert!(check_request(&WindowArgs::default()).is_ok());
        assert!(check_request(&mk(false, None)).is_ok());
        std::fs::write(&out, b"x").unwrap();
        assert!(check_request(&mk(false, None))
            .unwrap_err()
            .contains("--replace"));
        assert!(check_request(&mk(true, None)).is_ok());
        assert!(check_request(&mk(true, Some(dir.join("none.stl"))))
            .unwrap_err()
            .contains("is not a file"));
        let missing_dir = WindowArgs {
            shot: Some(ShotRequest {
                output: dir.join("no_such_folder").join("a.png"),
                replace: false,
                size: DEFAULT_SIZE,
                wait: 1,
            }),
            ..WindowArgs::default()
        };
        assert!(check_request(&missing_dir).unwrap_err().contains("folder"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_picture_is_written_once_and_only_replaced_when_asked() {
        let path = temp("pic.png");
        let _ = std::fs::remove_file(&path);
        let px = vec![200u8; 4 * 3 * 2];
        write_png(&path, false, 3, 2, &px).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert!(write_png(&path, false, 3, 2, &px).is_err());
        write_png(&path, true, 2, 3, &px).unwrap();
        assert_ne!(std::fs::read(&path).unwrap(), bytes);
        // the size must agree with the pixels
        assert!(write_png(&path, true, 5, 5, &px).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_demonstration_starts_with_the_printer_of_its_server_unless_another_is_asked_for() {
        let demo = parse_window_args(&args("--shot a.png --demo")).unwrap();
        assert_eq!(start_printer(&demo).as_deref(), Some(DEMO_PRINTER));
        let other = parse_window_args(&args("--shot a.png --demo --printer prusa-mk4s")).unwrap();
        assert_eq!(start_printer(&other).as_deref(), Some("prusa-mk4s"));
        let plain = parse_window_args(&args("--shot a.png")).unwrap();
        assert_eq!(start_printer(&plain), None);
        // the printer of the demonstration is one of the catalog, and the one the server made in memory lists first
        let cat = p3dp_profiles::Catalog::builtin().unwrap();
        assert!(cat.find(DEMO_PRINTER).is_some());
    }
}
