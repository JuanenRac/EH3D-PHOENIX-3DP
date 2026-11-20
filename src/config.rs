//! The settings file of the program: the limits of the readers, the link to the server and the language, in one file of `key = value` lines.
//! A key that no part knows is an error that names the line, so a typo is not silently ignored.

use crate::i18n::Lang;
use crate::keys::{Action, Scheme, SchemeId};
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
}

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
}
