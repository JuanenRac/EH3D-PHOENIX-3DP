//! Files of resin (mSLA) printers, read from their headers only, and the time a job of layers takes to expose.
//!
//! Resin printers take layers as images, and most makers keep their file format to themselves. This module reads only what is open: a package in the form of a ZIP that holds a plain
//! text `config.ini` and one PNG per layer (the form used by the `.sl1` files) gives its layer height, its exposure times, its material and the number of layers, without decoding a
//! single image. Every other kind of resin file is refused with the reason, by its extension: nothing is guessed about a closed format and nothing is written. The estimate of the
//! time is a sum over the layers (the waiting, the exposure, the lift and the return), and it is an estimate: the firmware of the printer has its own moves.

use crate::error::FormatError;
use crate::zip::Archive;
use p3dp_core::Limits;

/// The largest `config.ini` that is read, in bytes.
pub const MAX_CONFIG_BYTES: u64 = 1 << 20;
/// The most lines read from it, and the longest text kept.
pub const MAX_CONFIG_LINES: usize = 2_000;
const MAX_TEXT: usize = 200;
/// The most layers a job may have in this module.
pub const MAX_LAYERS: usize = 200_000;

/// The extensions of the resin formats that this program does not read, because they are not open: the reason is given to the person.
pub const CLOSED_EXTENSIONS: [&str; 14] = [
    "ctb", "cbddlp", "photon", "photons", "pwmx", "pwms", "pws", "pw0", "pwmo", "phz", "fdg",
    "goo", "lgs", "jxs",
];

/// The extensions of the packages that are read.
pub const OPEN_EXTENSIONS: [&str; 2] = ["sl1", "sl1s"];

/// What a file of layers says about the job.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinJob {
    pub layers: usize,
    pub layer_height_mm: f64,
    pub exposure_s: f64,
    pub bottom_exposure_s: f64,
    /// The layers over which the exposure goes from the bottom one to the normal one.
    pub transition_layers: u32,
    pub material: String,
    /// The time the file says the print takes, in seconds, when it says.
    pub stated_seconds: Option<f64>,
    /// The resin the file says is used, in millilitres, when it says.
    pub used_ml: Option<f64>,
    /// Every `key = value` of the file, cut to a short length, in the order of the file.
    pub config: Vec<(String, String)>,
}

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("resin", reason)
}

/// The extension of a name in lower case.
fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Reads the header of a resin file given its name (for the extension) and its bytes.
pub fn read_job(name: &str, bytes: &[u8], limits: &Limits) -> Result<ResinJob, FormatError> {
    let ext = extension(name);
    if CLOSED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(FormatError::unsupported(
            "resin",
            format!(
                "the .{ext} format is not open: this program does not read it, and does not guess it. Save the job as a package of layers (.sl1) from the program that prepared it"
            ),
        ));
    }
    if !OPEN_EXTENSIONS.contains(&ext.as_str()) {
        return Err(FormatError::unsupported(
            "resin",
            format!("the extension .{ext} is not one of a resin file this program knows"),
        ));
    }
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(FormatError::TooLarge {
            bytes: bytes.len() as u64,
            max: limits.max_file_bytes,
        });
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(bad(
            "a package of layers is a ZIP file, and this is not one",
        ));
    }
    let zip = Archive::open(bytes, limits)?;
    let layers = zip
        .entries
        .iter()
        .filter(|e| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(".png") && !n.starts_with("thumbnail")
        })
        .count();
    if layers == 0 {
        return Err(bad("the package has no layer images"));
    }
    if layers > MAX_LAYERS {
        return Err(bad(format!(
            "{layers} layers, more than the limit of {MAX_LAYERS}"
        )));
    }
    let entry = zip
        .find("config.ini")
        .ok_or_else(|| bad("the package has no config.ini"))?;
    if entry.size() > MAX_CONFIG_BYTES {
        return Err(bad(format!(
            "config.ini is {} bytes, more than the limit of {MAX_CONFIG_BYTES}",
            entry.size()
        )));
    }
    let text = String::from_utf8_lossy(&zip.read(entry)?).into_owned();
    let mut config: Vec<(String, String)> = Vec::new();
    for line in text.lines().take(MAX_CONFIG_LINES) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            config.push((
                k.trim().chars().take(MAX_TEXT).collect(),
                v.trim().chars().take(MAX_TEXT).collect(),
            ));
        }
    }
    let get = |key: &str| {
        config
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let number = |key: &str| -> Result<Option<f64>, FormatError> {
        match get(key) {
            None => Ok(None),
            Some(v) => {
                let x: f64 = v
                    .parse()
                    .map_err(|_| bad(format!("{key} is {v:?}, not a number")))?;
                if x.is_finite() && x >= 0.0 {
                    Ok(Some(x))
                } else {
                    Err(bad(format!("{key} is {v:?}, not a usable number")))
                }
            }
        }
    };
    let layer_height_mm =
        number("layerHeight")?.ok_or_else(|| bad("config.ini has no layerHeight"))?;
    let exposure_s = number("expTime")?.ok_or_else(|| bad("config.ini has no expTime"))?;
    let bottom_exposure_s =
        number("expTimeFirst")?.ok_or_else(|| bad("config.ini has no expTimeFirst"))?;
    let transition = number("numFade")?.unwrap_or(0.0);
    let job = ResinJob {
        layers,
        layer_height_mm,
        exposure_s,
        bottom_exposure_s,
        transition_layers: transition.min(f64::from(u32::MAX)) as u32,
        material: get("materialName").unwrap_or("").to_string(),
        stated_seconds: number("printTime")?,
        used_ml: number("usedMaterial")?,
        config,
    };
    Ok(job)
}

/// What the estimate of a job needs besides the job: the moves of the printer. The defaults are conservative guesses of a common machine, not the figures of any one model.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinSettings {
    pub bottom_layers: u32,
    pub lift_mm: f64,
    pub lift_speed_mm_s: f64,
    pub retract_speed_mm_s: f64,
    pub wait_before_s: f64,
    pub wait_after_s: f64,
}

impl Default for ResinSettings {
    fn default() -> ResinSettings {
        ResinSettings {
            bottom_layers: 5,
            lift_mm: 6.0,
            lift_speed_mm_s: 2.0,
            retract_speed_mm_s: 3.0,
            wait_before_s: 1.0,
            wait_after_s: 0.5,
        }
    }
}

/// What the estimate comes to.
#[derive(Debug, Clone, PartialEq)]
pub struct ResinEstimate {
    pub layers: usize,
    /// The seconds the light is on.
    pub exposure_s: f64,
    /// The seconds of moving and waiting.
    pub movement_s: f64,
    pub total_s: f64,
    /// The exposure of each layer, in order.
    pub per_layer_exposure_s: Vec<f64>,
}

/// The exposure of layer `index` (from 0): the bottom exposure for the bottom layers, then a straight line down to the normal exposure over the transition layers.
pub fn layer_exposure(job: &ResinJob, bottom_layers: u32, index: usize) -> f64 {
    let bottom = bottom_layers as usize;
    if index < bottom {
        return job.bottom_exposure_s;
    }
    let step = index - bottom + 1;
    let fade = job.transition_layers as usize;
    if fade == 0 || step > fade {
        return job.exposure_s;
    }
    let t = step as f64 / (fade + 1) as f64;
    job.bottom_exposure_s + (job.exposure_s - job.bottom_exposure_s) * t
}

/// The time of the job. A setting out of its range is an error that names it.
pub fn estimate(job: &ResinJob, s: &ResinSettings) -> Result<ResinEstimate, FormatError> {
    let check = |name: &str, v: f64, lo: f64, hi: f64| {
        if v.is_finite() && (lo..=hi).contains(&v) {
            Ok(())
        } else {
            Err(bad(format!("{name} {v} is outside {lo} to {hi}")))
        }
    };
    check("lift_mm", s.lift_mm, 0.0, 100.0)?;
    check("lift_speed_mm_s", s.lift_speed_mm_s, 0.1, 100.0)?;
    check("retract_speed_mm_s", s.retract_speed_mm_s, 0.1, 100.0)?;
    check("wait_before_s", s.wait_before_s, 0.0, 600.0)?;
    check("wait_after_s", s.wait_after_s, 0.0, 600.0)?;
    check("exposure_s", job.exposure_s, 0.05, 600.0)?;
    check("bottom_exposure_s", job.bottom_exposure_s, 0.05, 3600.0)?;
    check("layer_height_mm", job.layer_height_mm, 0.005, 1.0)?;
    if job.layers == 0 || job.layers > MAX_LAYERS {
        return Err(bad(format!(
            "{} layers is outside 1 to {MAX_LAYERS}",
            job.layers
        )));
    }
    let mut per_layer = Vec::with_capacity(job.layers);
    let mut exposure = 0.0;
    for i in 0..job.layers {
        let e = layer_exposure(job, s.bottom_layers, i);
        exposure += e;
        per_layer.push(e);
    }
    let per_move = s.wait_before_s
        + s.wait_after_s
        + s.lift_mm / s.lift_speed_mm_s
        + s.lift_mm / s.retract_speed_mm_s;
    let movement = per_move * job.layers as f64;
    Ok(ResinEstimate {
        layers: job.layers,
        exposure_s: exposure,
        movement_s: movement,
        total_s: exposure + movement,
        per_layer_exposure_s: per_layer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::Writer;

    fn package(config: &str, layers: usize, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = Writer::new();
        w.add("config.ini", config.as_bytes()).unwrap();
        w.add("thumbnail/thumbnail400x400.png", b"x").unwrap();
        for i in 0..layers {
            w.add(&format!("part{i:05}.png"), b"png").unwrap();
        }
        for (n, d) in extra {
            w.add(n, d).unwrap();
        }
        w.finish()
    }

    const CONFIG: &str = "action = print\nexpTime = 2.5\nexpTimeFirst = 35\nlayerHeight = 0.05\nmaterialName = Generic Tough\nnumFade = 10\nprintTime = 3600.5\nusedMaterial = 12.3\n";

    #[test]
    fn a_package_of_layers_gives_what_its_config_says_without_decoding_an_image() {
        let bytes = package(CONFIG, 120, &[]);
        let job = read_job("part.SL1", &bytes, &Limits::default()).unwrap();
        assert_eq!(job.layers, 120);
        assert_eq!(
            (job.layer_height_mm, job.exposure_s, job.bottom_exposure_s),
            (0.05, 2.5, 35.0)
        );
        assert_eq!(
            (job.transition_layers, job.material.as_str()),
            (10, "Generic Tough")
        );
        assert_eq!(
            (job.stated_seconds, job.used_ml),
            (Some(3600.5), Some(12.3))
        );
        assert!(job
            .config
            .iter()
            .any(|(k, v)| k == "action" && v == "print"));
        assert!(read_job("part.sl1s", &bytes, &Limits::default()).is_ok());
    }

    #[test]
    fn closed_formats_and_strangers_are_refused_with_the_reason() {
        let bytes = package(CONFIG, 3, &[]);
        for name in ["a.ctb", "a.CBDDLP", "a.photon", "a.pwmx", "a.goo", "a.jxs"] {
            let e = read_job(name, &bytes, &Limits::default()).unwrap_err();
            assert!(
                matches!(&e, FormatError::Unsupported { what, .. } if what.contains("not open")),
                "{name}: {e:?}"
            );
        }
        for name in ["a.zip", "a", "a.gcode"] {
            assert!(matches!(
                read_job(name, &bytes, &Limits::default()),
                Err(FormatError::Unsupported { .. })
            ));
        }
        // a name that says it is open and content that is not
        assert!(read_job("a.sl1", b"GCDE....", &Limits::default()).is_err());
        assert!(read_job("a.sl1", b"", &Limits::default()).is_err());
        // a package without layers, without a config, or with a config that is wrong
        assert!(read_job("a.sl1", &package(CONFIG, 0, &[]), &Limits::default()).is_err());
        let mut w = Writer::new();
        w.add("part00000.png", b"png").unwrap();
        assert!(read_job("a.sl1", &w.finish(), &Limits::default()).is_err());
        for broken in [
            "expTime = 2.5\nexpTimeFirst = 35\n",
            "expTime = fast\nexpTimeFirst = 35\nlayerHeight = 0.05\n",
            "expTime = -2\nexpTimeFirst = 35\nlayerHeight = 0.05\n",
            "expTime = nan\nexpTimeFirst = 35\nlayerHeight = 0.05\n",
        ] {
            assert!(
                read_job("a.sl1", &package(broken, 2, &[]), &Limits::default()).is_err(),
                "{broken}"
            );
        }
        // too large for the limit of the program
        let tiny = Limits {
            max_file_bytes: 10,
            ..Limits::default()
        };
        assert!(matches!(
            read_job("a.sl1", &bytes, &tiny),
            Err(FormatError::TooLarge { .. })
        ));
        // cut at every length: an error, never a panic
        for n in (0..bytes.len()).step_by(7) {
            let _ = read_job("a.sl1", &bytes[..n], &Limits::default());
        }
    }

    #[test]
    fn the_exposure_is_the_bottom_one_then_a_straight_line_down_to_the_normal_one() {
        let job = read_job("a.sl1", &package(CONFIG, 40, &[]), &Limits::default()).unwrap();
        let e = |i| layer_exposure(&job, 5, i);
        assert_eq!((e(0), e(4)), (35.0, 35.0));
        assert!(e(5) < 35.0 && e(5) > e(6) && e(6) > e(14));
        assert!((e(15) - 2.5).abs() < 1e-12 && e(39) == 2.5);
        assert!(e(14) > 2.5);
        // no fade: straight to the normal exposure
        let sharp = ResinJob {
            transition_layers: 0,
            ..job
        };
        assert_eq!(layer_exposure(&sharp, 5, 5), 2.5);
    }

    #[test]
    fn the_estimate_adds_the_light_the_waiting_and_the_moves_of_every_layer() {
        let job = ResinJob {
            layers: 100,
            layer_height_mm: 0.05,
            exposure_s: 2.0,
            bottom_exposure_s: 20.0,
            transition_layers: 0,
            material: String::new(),
            stated_seconds: None,
            used_ml: None,
            config: Vec::new(),
        };
        let s = ResinSettings::default();
        let est = estimate(&job, &s).unwrap();
        assert!((est.exposure_s - (5.0 * 20.0 + 95.0 * 2.0)).abs() < 1e-9);
        // 1 + 0.5 + 6 / 2 + 6 / 3 = 6.5 s of moving per layer
        assert!((est.movement_s - 650.0).abs() < 1e-9, "{}", est.movement_s);
        assert!((est.total_s - (est.exposure_s + est.movement_s)).abs() < 1e-9);
        assert_eq!(est.per_layer_exposure_s.len(), 100);
        // a longer lift is longer
        let slow = ResinSettings {
            lift_mm: 10.0,
            ..s.clone()
        };
        assert!(estimate(&job, &slow).unwrap().total_s > est.total_s);
        // what is out of range is an error that names it
        for (bad, name) in [
            (
                ResinSettings {
                    lift_speed_mm_s: 0.0,
                    ..s.clone()
                },
                "lift_speed_mm_s",
            ),
            (
                ResinSettings {
                    lift_mm: f64::NAN,
                    ..s.clone()
                },
                "lift_mm",
            ),
            (
                ResinSettings {
                    wait_after_s: 1000.0,
                    ..s.clone()
                },
                "wait_after_s",
            ),
        ] {
            let e = estimate(&job, &bad).unwrap_err().to_string();
            assert!(e.contains(name), "{e}");
        }
        let none = ResinJob {
            layers: 0,
            ..job.clone()
        };
        assert!(estimate(&none, &s).is_err());
        let dark = ResinJob {
            exposure_s: 0.0,
            ..job
        };
        assert!(estimate(&dark, &s).is_err());
    }
}
