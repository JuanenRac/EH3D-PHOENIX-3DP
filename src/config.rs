//! The settings file of the program: the limits of the readers, the link to the server and the language, in one file of `key = value` lines.
//! A key that no part knows is an error that names the line, so a typo is not silently ignored.

use crate::i18n::Lang;
use crate::keys::{Action, Scheme, SchemeId};
use crate::look::Theme;
use p3dp_core::kv;
use p3dp_core::Limits;
use p3dp_formats::gcode_meta::ScanLimits;
use p3dp_server_bridge::{QueuePolicy, Settings};

/// The settings.
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub limits: Limits,
    pub server: Settings,
    pub lang: Option<Lang>,
    /// The id of the printer profile to start with (empty: none).
    pub default_printer: String,
    /// The scheme of keyboard shortcuts the window starts with.
    pub scheme: SchemeId,
    /// The changes to the scheme, as the file has them: the name of an action after `key_` and a chord or `none`.
    pub key_changes: Vec<(String, String)>,
    /// What a kilogram of filament costs, in the currency of the person (0: no cost is shown).
    pub price_filament_per_kg: f64,
    /// What a litre of resin costs.
    pub price_resin_per_litre: f64,
    /// How much more resin than the part has, in percent, for supports, raft and drips.
    pub resin_waste_percent: f64,
    /// How many of the slowest layers a report lists.
    pub report_slowest_layers: usize,
    /// A layer that takes less than this many seconds is listed as a short one in a report (0: none).
    pub report_short_layer_s: f64,
    /// Refuse a job whose name repeats one that still waits on the same machine.
    pub queue_refuse_duplicates: bool,
    /// The most jobs that may wait on one machine.
    pub queue_max_waiting: usize,
    /// How much of the start of a G-code file is searched for thumbnails, in KiB.
    pub limit_gcode_head_kib: usize,
    /// How much of the end of a G-code file is read for the figures of the print, in KiB.
    pub limit_gcode_tail_kib: usize,
    /// The most thumbnails taken from one G-code file.
    pub limit_gcode_thumbnails: usize,
    /// The look of the window: `dark` or `light`.
    pub theme: Theme,
    /// The distance between the lines of the grid of the bed in the 3D view, in millimetres.
    pub grid_mm: f64,
    /// The gap that "arrange" leaves between the parts on the bed, in millimetres.
    pub arrange_gap_mm: f64,
    /// Arrange the parts by themselves each time one is added.
    pub arrange_on_add: bool,
    /// Show the short hints that explain each panel of the window.
    pub ui_hints: bool,
    /// The temperature limits a person set for a printer of the catalog; they replace the figures of the catalog and make the limits the person's.
    pub limit_overrides: Vec<LimitOverride>,
}

/// The limits of one printer set by the person.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitOverride {
    pub printer_id: String,
    pub nozzle_c: f64,
    pub bed_c: f64,
    pub chamber_c: Option<f64>,
}

impl LimitOverride {
    /// Reads `NOZZLE,BED,CHAMBER` where the chamber is a number or `none`.
    pub fn parse(printer_id: &str, value: &str) -> Result<LimitOverride, String> {
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(
                "three values are needed: nozzle,bed,chamber (the chamber may be none)".into(),
            );
        }
        let num = |t: &str, what: &str, max: f64| -> Result<f64, String> {
            let v: f64 = t
                .parse()
                .map_err(|_| format!("{what}: {t:?} is not a number"))?;
            if v.is_finite() && v > 0.0 && v <= max {
                Ok(v)
            } else {
                Err(format!("{what}: from above 0 to {max} C"))
            }
        };
        let chamber = if parts[2].eq_ignore_ascii_case("none") {
            None
        } else {
            Some(num(parts[2], "chamber", 200.0)?)
        };
        Ok(LimitOverride {
            printer_id: printer_id.to_string(),
            nozzle_c: num(parts[0], "nozzle", 600.0)?,
            bed_c: num(parts[1], "bed", 200.0)?,
            chamber_c: chamber,
        })
    }

    pub fn value_text(&self) -> String {
        let chamber = self.chamber_c.map_or("none".to_string(), |c| c.to_string());
        format!("{},{},{chamber}", self.nozzle_c, self.bed_c)
    }
}

/// The prefix of the keys that hold the limits of a printer.
pub const LIMITS_KEY_PREFIX: &str = "printer_limits_";

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            limits: Limits::default(),
            server: Settings::default(),
            lang: None,
            default_printer: String::new(),
            scheme: SchemeId::default(),
            key_changes: Vec::new(),
            price_filament_per_kg: 20.0,
            price_resin_per_litre: 40.0,
            resin_waste_percent: 20.0,
            report_slowest_layers: 5,
            report_short_layer_s: 5.0,
            queue_refuse_duplicates: true,
            queue_max_waiting: 20,
            limit_gcode_head_kib: 1024,
            limit_gcode_tail_kib: 64,
            limit_gcode_thumbnails: 8,
            theme: Theme::default(),
            grid_mm: 10.0,
            arrange_gap_mm: 5.0,
            arrange_on_add: true,
            ui_hints: true,
            limit_overrides: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Reads the text of a settings file.
    pub fn from_text(text: &str) -> Result<AppConfig, String> {
        let mut c = AppConfig::default();
        for e in kv::parse(text).map_err(|e| e.to_string())? {
            let at = |m: String| format!("line {}: {m}", e.line);
            if c.limits
                .set(&e.key, &e.value)
                .map_err(|x| at(x.to_string()))?
            {
                continue;
            }
            if c.server
                .set(&e.key, &e.value)
                .map_err(|x| at(x.to_string()))?
            {
                continue;
            }
            match e.key.as_str() {
                "lang" => {
                    c.lang = Some(Lang::from_code(&e.value).ok_or_else(|| {
                        at(format!(
                            "lang: {:?} is not one of en, es, de, fr, it, ja, zh, pt",
                            e.value
                        ))
                    })?)
                }
                "default_printer" => {
                    if !e
                        .value
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    {
                        return Err(at(
                            "default_printer: lower case letters, digits and hyphens".to_string(),
                        ));
                    }
                    c.default_printer = e.value.clone();
                }
                "shortcut_scheme" => {
                    c.scheme = SchemeId::from_name(&e.value)
                        .ok_or_else(|| at("shortcut_scheme: family or own".to_string()))?
                }
                "price_filament_per_kg" => {
                    c.price_filament_per_kg = kv::float_in(&e.key, &e.value, 0.0, 100_000.0)
                        .map_err(|x| at(x.to_string()))?
                }
                "price_resin_per_litre" => {
                    c.price_resin_per_litre = kv::float_in(&e.key, &e.value, 0.0, 100_000.0)
                        .map_err(|x| at(x.to_string()))?
                }
                "resin_waste_percent" => {
                    c.resin_waste_percent =
                        kv::float_in(&e.key, &e.value, 0.0, 500.0).map_err(|x| at(x.to_string()))?
                }
                "report_slowest_layers" => {
                    c.report_slowest_layers =
                        kv::int_in(&e.key, &e.value, 0, 50).map_err(|x| at(x.to_string()))? as usize
                }
                "report_short_layer_s" => {
                    c.report_short_layer_s =
                        kv::float_in(&e.key, &e.value, 0.0, 600.0).map_err(|x| at(x.to_string()))?
                }
                "queue_refuse_duplicates" => {
                    c.queue_refuse_duplicates =
                        kv::on_off(&e.key, &e.value).map_err(|x| at(x.to_string()))?
                }
                "queue_max_waiting" => {
                    c.queue_max_waiting = kv::int_in(&e.key, &e.value, 1, 1000)
                        .map_err(|x| at(x.to_string()))?
                        as usize
                }
                "limit_gcode_head_kib" => {
                    c.limit_gcode_head_kib = kv::int_in(&e.key, &e.value, 1, 65_536)
                        .map_err(|x| at(x.to_string()))?
                        as usize
                }
                "limit_gcode_tail_kib" => {
                    c.limit_gcode_tail_kib = kv::int_in(&e.key, &e.value, 1, 4096)
                        .map_err(|x| at(x.to_string()))?
                        as usize
                }
                "limit_gcode_thumbnails" => {
                    c.limit_gcode_thumbnails =
                        kv::int_in(&e.key, &e.value, 0, 64).map_err(|x| at(x.to_string()))? as usize
                }
                "theme" => {
                    c.theme = Theme::from_name(&e.value)
                        .ok_or_else(|| at("theme: dark or light".to_string()))?
                }
                "grid_mm" => {
                    c.grid_mm =
                        kv::float_in(&e.key, &e.value, 1.0, 100.0).map_err(|x| at(x.to_string()))?
                }
                "arrange_gap_mm" => {
                    c.arrange_gap_mm =
                        kv::float_in(&e.key, &e.value, 0.0, 100.0).map_err(|x| at(x.to_string()))?
                }
                "arrange_on_add" => {
                    c.arrange_on_add =
                        kv::on_off(&e.key, &e.value).map_err(|x| at(x.to_string()))?
                }
                "ui_hints" => {
                    c.ui_hints = kv::on_off(&e.key, &e.value).map_err(|x| at(x.to_string()))?
                }
                other if other.starts_with(LIMITS_KEY_PREFIX) => {
                    let id = &other[LIMITS_KEY_PREFIX.len()..];
                    if id.is_empty()
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    {
                        return Err(at(format!(
                            "{other}: the printer id is lower case letters, digits and hyphens"
                        )));
                    }
                    let o = LimitOverride::parse(id, &e.value)
                        .map_err(|m| at(format!("{other}: {m}")))?;
                    c.limit_overrides.retain(|x| x.printer_id != id);
                    c.limit_overrides.push(o);
                }
                other if other.starts_with("key_") => {
                    let name = &other[4..];
                    if Action::from_name(name).is_none() {
                        return Err(at(format!("{other}: no such action")));
                    }
                    c.key_changes.retain(|(n, _)| n != name);
                    c.key_changes.push((name.to_string(), e.value.clone()));
                }
                other => return Err(at(format!("the key {other} is not known"))),
            }
        }
        c.shortcuts()?;
        Ok(c)
    }

    /// The shortcuts in force: the scheme with the changes of the file. Two actions on one chord, or a chord that cannot be read, is an error.
    pub fn shortcuts(&self) -> Result<Scheme, String> {
        Scheme::with_changes(self.scheme, &self.key_changes)
    }

    /// The rules for putting a job in the queue of the server.
    pub fn queue_policy(&self) -> QueuePolicy {
        QueuePolicy {
            refuse_duplicates: self.queue_refuse_duplicates,
            max_waiting: self.queue_max_waiting,
        }
    }

    /// How far a search of a G-code file for thumbnails and figures goes.
    pub fn scan_limits(&self) -> ScanLimits {
        ScanLimits {
            head_bytes: self.limit_gcode_head_kib.saturating_mul(1024),
            tail_bytes: self.limit_gcode_tail_kib.saturating_mul(1024),
            max_thumbnails: self.limit_gcode_thumbnails,
            ..ScanLimits::default()
        }
    }

    /// The settings as text (the token is `***` unless `reveal`).
    pub fn to_text(&self, reveal: bool) -> String {
        let mut s = String::new();
        if let Some(l) = self.lang {
            s.push_str(&format!("lang = {}\n", l.code()));
        }
        s.push_str(&format!("default_printer = {}\n", self.default_printer));
        s.push_str(&format!("shortcut_scheme = {}\n", self.scheme.name()));
        for (name, value) in &self.key_changes {
            s.push_str(&format!("key_{name} = {value}\n"));
        }
        s.push_str(&format!(
            "price_filament_per_kg = {}\nprice_resin_per_litre = {}\nresin_waste_percent = {}\nreport_slowest_layers = {}\nreport_short_layer_s = {}\nqueue_refuse_duplicates = {}\nqueue_max_waiting = {}\nlimit_gcode_head_kib = {}\nlimit_gcode_tail_kib = {}\nlimit_gcode_thumbnails = {}\n",
            self.price_filament_per_kg,
            self.price_resin_per_litre,
            self.resin_waste_percent,
            self.report_slowest_layers,
            self.report_short_layer_s,
            if self.queue_refuse_duplicates { "on" } else { "off" },
            self.queue_max_waiting,
            self.limit_gcode_head_kib,
            self.limit_gcode_tail_kib,
            self.limit_gcode_thumbnails
        ));
        s.push_str(&format!(
            "theme = {}\ngrid_mm = {}\narrange_gap_mm = {}\narrange_on_add = {}\nui_hints = {}\n",
            self.theme.name(),
            self.grid_mm,
            self.arrange_gap_mm,
            if self.arrange_on_add { "on" } else { "off" },
            if self.ui_hints { "on" } else { "off" },
        ));
        for o in &self.limit_overrides {
            s.push_str(&format!(
                "{LIMITS_KEY_PREFIX}{} = {}\n",
                o.printer_id,
                o.value_text()
            ));
        }
        s.push_str(&self.limits.to_text());
        s.push_str(&self.server.to_text(reveal));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_round_trips() {
        let c = AppConfig::default();
        assert_eq!(AppConfig::from_text(&c.to_text(true)).unwrap(), c);
    }

    #[test]
    fn keys_of_every_part_are_read() {
        let c = AppConfig::from_text("lang = pt-PT\ndefault_printer = bambu-lab-a1\nlimit_max_triangles = 1000\nserver_address = 10.0.0.2:18200\nserver_retries = 1\n").unwrap();
        assert_eq!(c.lang, Some(Lang::Pt));
        assert_eq!(c.default_printer, "bambu-lab-a1");
        assert_eq!(c.limits.max_triangles, 1000);
        assert_eq!(c.server.server_retries, 1);
    }

    #[test]
    fn the_scheme_of_shortcuts_and_the_changes_are_read_checked_and_written_back() {
        let c = AppConfig::from_text(
            "shortcut_scheme = own\nkey_open = Ctrl+Alt+O\nkey_refresh = none\nkey_open = Ctrl+Alt+P\n",
        )
        .unwrap();
        assert_eq!(c.scheme, SchemeId::Own);
        assert_eq!(
            c.key_changes.len(),
            2,
            "a second line of the same action wins"
        );
        let s = c.shortcuts().unwrap();
        assert_eq!(s.chord(Action::Open).unwrap().to_string(), "Ctrl+Alt+P");
        assert!(s.chord(Action::Refresh).is_none());
        assert_eq!(AppConfig::from_text(&c.to_text(true)).unwrap(), c);
        for (text, want) in [
            ("shortcut_scheme = qwerty\n", "family or own"),
            ("key_fly = Ctrl+F\n", "no such action"),
            ("key_open = Ctrl+\n", "key_open"),
            ("key_open = Ctrl+S\n", "both"),
        ] {
            let e = AppConfig::from_text(text).unwrap_err();
            assert!(e.contains(want), "{text:?} -> {e}");
        }
    }

    #[test]
    fn the_report_the_queue_and_the_scan_keys_have_conservative_defaults_are_read_and_written_back()
    {
        let d = AppConfig::default();
        assert_eq!(
            (
                d.price_filament_per_kg,
                d.price_resin_per_litre,
                d.resin_waste_percent
            ),
            (20.0, 40.0, 20.0)
        );
        assert_eq!((d.report_slowest_layers, d.report_short_layer_s), (5, 5.0));
        assert!(
            d.queue_refuse_duplicates,
            "a repeated job is refused unless the person turns it off"
        );
        assert_eq!(
            d.queue_policy(),
            QueuePolicy {
                refuse_duplicates: true,
                max_waiting: 20
            }
        );
        assert_eq!(d.scan_limits(), ScanLimits::default());
        let c = AppConfig::from_text("price_filament_per_kg = 27.5\nprice_resin_per_litre = 55\nresin_waste_percent = 35\nreport_slowest_layers = 9\nreport_short_layer_s = 0\nqueue_refuse_duplicates = off\nqueue_max_waiting = 3\nlimit_gcode_head_kib = 512\nlimit_gcode_tail_kib = 8\nlimit_gcode_thumbnails = 2\n").unwrap();
        assert_eq!(c.price_filament_per_kg, 27.5);
        assert_eq!(
            c.queue_policy(),
            QueuePolicy {
                refuse_duplicates: false,
                max_waiting: 3
            }
        );
        let l = c.scan_limits();
        assert_eq!(
            (l.head_bytes, l.tail_bytes, l.max_thumbnails),
            (512 * 1024, 8 * 1024, 2)
        );
        assert_eq!(AppConfig::from_text(&c.to_text(true)).unwrap(), c);
        for (text, want) in [
            ("price_filament_per_kg = -1\n", "price_filament_per_kg"),
            ("price_resin_per_litre = cheap\n", "price_resin_per_litre"),
            ("resin_waste_percent = 900\n", "resin_waste_percent"),
            ("report_slowest_layers = 51\n", "report_slowest_layers"),
            ("report_short_layer_s = 601\n", "report_short_layer_s"),
            (
                "queue_refuse_duplicates = maybe\n",
                "queue_refuse_duplicates",
            ),
            ("queue_max_waiting = 0\n", "queue_max_waiting"),
            ("limit_gcode_head_kib = 0\n", "limit_gcode_head_kib"),
            ("limit_gcode_tail_kib = 5000\n", "limit_gcode_tail_kib"),
            ("limit_gcode_thumbnails = 65\n", "limit_gcode_thumbnails"),
        ] {
            let e = AppConfig::from_text(text).unwrap_err();
            assert!(e.contains(want) && e.contains("line 1"), "{text:?} -> {e}");
        }
    }

    #[test]
    fn the_look_the_grid_the_arrangement_and_the_limits_of_a_printer_are_read_checked_and_written_back(
    ) {
        let d = AppConfig::default();
        assert_eq!(
            (
                d.theme,
                d.grid_mm,
                d.arrange_gap_mm,
                d.arrange_on_add,
                d.ui_hints
            ),
            (Theme::Dark, 10.0, 5.0, true, true)
        );
        assert!(d.limit_overrides.is_empty());
        let c = AppConfig::from_text(
            "theme = light\ngrid_mm = 20\narrange_gap_mm = 0\narrange_on_add = off\nui_hints = off\nprinter_limits_prusa-mk4s = 290, 110, none\nprinter_limits_qidi-plus4 = 300,120,65\nprinter_limits_prusa-mk4s = 280,100,none\n",
        )
        .unwrap();
        assert_eq!(c.theme, Theme::Light);
        assert_eq!((c.grid_mm, c.arrange_gap_mm), (20.0, 0.0));
        assert!(!c.arrange_on_add && !c.ui_hints);
        assert_eq!(
            c.limit_overrides.len(),
            2,
            "a second line of the same printer wins"
        );
        let mk4 = c
            .limit_overrides
            .iter()
            .find(|o| o.printer_id == "prusa-mk4s")
            .unwrap();
        assert_eq!(
            (mk4.nozzle_c, mk4.bed_c, mk4.chamber_c),
            (280.0, 100.0, None)
        );
        assert_eq!(AppConfig::from_text(&c.to_text(true)).unwrap(), c);
        for (text, want) in [
            ("theme = sepia\n", "dark or light"),
            ("grid_mm = 0\n", "grid_mm"),
            ("grid_mm = 101\n", "grid_mm"),
            ("arrange_gap_mm = -1\n", "arrange_gap_mm"),
            ("arrange_on_add = perhaps\n", "arrange_on_add"),
            ("ui_hints = 2\n", "ui_hints"),
            ("printer_limits_ = 1,2,none\n", "printer id"),
            ("printer_limits_BadId = 1,2,none\n", "printer id"),
            ("printer_limits_x = 1,2\n", "three values"),
            ("printer_limits_x = hot,2,none\n", "nozzle"),
            ("printer_limits_x = 300,0,none\n", "bed"),
            ("printer_limits_x = 300,100,250\n", "chamber"),
            ("printer_limits_x = 700,100,none\n", "nozzle"),
        ] {
            let e = AppConfig::from_text(text).unwrap_err();
            assert!(e.contains(want) && e.contains("line 1"), "{text:?} -> {e}");
        }
    }

    #[test]
    fn mistakes_name_the_line() {
        for (text, want) in [
            ("lang = xx\n", "line 1"),
            ("\nlimit_max_triangles = 0\n", "line 2"),
            ("server_timeout_s = 999\n", "line 1"),
            ("surprise = 1\n", "surprise"),
            ("default_printer = Bad Id\n", "default_printer"),
            ("no equals sign\n", "line 1"),
        ] {
            let e = AppConfig::from_text(text).unwrap_err();
            assert!(e.contains(want), "{text:?} -> {e}");
        }
    }

    #[test]
    fn the_token_is_hidden_when_written_for_a_screen() {
        let c = AppConfig::from_text("server_token = topsecret\nserver_address = a:1\n").unwrap();
        assert!(!c.to_text(false).contains("topsecret"));
        assert!(c.to_text(true).contains("topsecret"));
        assert!(!format!("{c:?}").contains("topsecret"));
    }

    #[test]
    fn garbage_in_the_new_keys_never_panics_and_always_names_the_line_or_is_read() {
        let mut rng = p3dp_core::rng::Rng::new(77);
        let keys = [
            "theme",
            "grid_mm",
            "arrange_gap_mm",
            "arrange_on_add",
            "ui_hints",
            "printer_limits_prusa-mk4s",
            "printer_limits_x",
        ];
        for _ in 0..500 {
            let key = keys[rng.below(keys.len() as u64) as usize];
            let junk: String = (0..rng.below(14))
                .map(|_| {
                    let pool = "0123456789.,-+ eEnNoOfFdDlLtT_xX";
                    pool.chars()
                        .nth(rng.below(pool.chars().count() as u64) as usize)
                        .unwrap()
                })
                .collect();
            let text = format!(
                "{key} = {junk}
"
            );
            match AppConfig::from_text(&text) {
                Ok(c) => assert_eq!(
                    AppConfig::from_text(&c.to_text(true)).unwrap(),
                    c,
                    "{text:?}"
                ),
                Err(e) => assert!(e.contains("line 1") || e.contains("key"), "{text:?} -> {e}"),
            }
        }
    }
}
