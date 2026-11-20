//! The settings of the EH3D server, the state of the link and the login in the window, without the window. The panel itself is the one of `eh3d-view` of EH3D-COMMON, the same
//! that the other programs of the suite show, with its words in the languages of the suite; this ties it to the settings of this program (`server_address`, `server_token`,
//! `server_machine`, `server_timeout_s`, `server_retries`, `server_mdns`, `server_max_answer_mib`) and to the one client that the Device tab uses, so that a login made in the
//! panel is the login of the queue. The lines that are kept in a settings file are those keys but the token: a token is never written by the window. A login is made with the
//! name and password of a person, the password is wiped as soon as the call is made, and the login lives in the client and in no file.

use crate::config::AppConfig;
use crate::i18n::Lang;
use eh3d_serverclient as sc;
use eh3d_view::server_panel::{Action, ServerPanel};
use eh3d_view::texts::Lang as ViewLang;
use p3dp_server_bridge::{ServerClient, Settings, APP};

/// The language of the panel for a language of the window.
pub fn view_lang(lang: Lang) -> ViewLang {
    match lang {
        Lang::En => ViewLang::En,
        Lang::Es => ViewLang::Es,
        Lang::De => ViewLang::De,
        Lang::Fr => ViewLang::Fr,
        Lang::It => ViewLang::It,
        Lang::Ja => ViewLang::Ja,
        Lang::Zh => ViewLang::Zh,
        Lang::Pt => ViewLang::Pt,
    }
}

/// The language of the table of errors of the client for a language of the window.
pub fn client_lang(lang: Lang) -> sc::Lang {
    sc::Lang::from_tag(lang.code()).unwrap_or(sc::Lang::En)
}

/// The settings of the client for the settings of the program. A value the client does not accept is told; an empty address is allowed (it is the panel, empty, for the person to
/// fill in).
pub fn settings_of(settings: &Settings) -> Result<sc::Settings, String> {
    let mut s = sc::Settings::default();
    let mut put = |key: &str, value: &str| s.set(key, value).map(|_| ()).map_err(|e| e.to_string());
    put("server_address", &settings.server_address)?;
    put("server_token", settings.server_token.expose())?;
    put("server_machine", &settings.server_machine)?;
    put("server_timeout_s", &settings.server_timeout_s.to_string())?;
    put("server_retries", &settings.server_retries.to_string())?;
    put(
        "server_mdns",
        if settings.server_mdns { "on" } else { "off" },
    )?;
    put(
        "server_max_answer_mib",
        &settings.server_max_answer_mib.to_string(),
    )?;
    Ok(s)
}

/// Puts the settings of the client in the settings of the program. The stand-alone switch is not the client's and is left as it was.
pub fn apply_to(settings: &mut Settings, s: &sc::Settings) -> Result<(), String> {
    let mut put = |key: &str, value: &str| {
        settings
            .set(key, value)
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    put("server_address", &s.server_address)?;
    put("server_token", s.server_token.expose())?;
    put("server_machine", &s.server_machine)?;
    put("server_timeout_s", &s.server_timeout_s.to_string())?;
    put("server_retries", &s.server_retries.to_string())?;
    put("server_mdns", if s.server_mdns { "on" } else { "off" })?;
    put(
        "server_max_answer_mib",
        &s.server_max_answer_mib.to_string(),
    )
}

/// The lines of a settings file that the panel keeps: the keys of the server but the token.
pub fn lines_of(settings: &Settings) -> Vec<(&'static str, String)> {
    vec![
        ("server_address", settings.server_address.clone()),
        ("server_machine", settings.server_machine.clone()),
        ("server_timeout_s", settings.server_timeout_s.to_string()),
        ("server_retries", settings.server_retries.to_string()),
        (
            "server_mdns",
            if settings.server_mdns { "on" } else { "off" }.to_string(),
        ),
        (
            "server_max_answer_mib",
            settings.server_max_answer_mib.to_string(),
        ),
    ]
}

/// Puts the lines in the text of a settings file: a line of the same key is replaced where it is (and a second one of the same key is dropped), a key that was not there is added at
/// the end, and every other line, comments included, is left as it was. A line that carries the token is never written.
pub fn merge_lines(text: &str, lines: &[(&str, String)]) -> String {
    let mut done: Vec<&str> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let key = raw
            .split('#')
            .next()
            .and_then(|code| code.split_once('='))
            .map(|(k, _)| k.trim());
        match key.and_then(|k| lines.iter().find(|(name, _)| name.eq_ignore_ascii_case(k))) {
            Some((name, value)) => {
                if !done.contains(name) {
                    done.push(name);
                    out.push(format!("{name} = {value}"));
                }
            }
            None => out.push(raw.to_string()),
        }
    }
    for (name, value) in lines {
        if !done.contains(name) && *name != "server_token" {
            out.push(format!("{name} = {value}"));
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// Takes lines out of the text of a settings file: every line of one of the keys is dropped, and every other line, comments included, is left as it was.
pub fn drop_keys(text: &str, keys: &[&str]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for raw in text.lines() {
        let key = raw
            .split('#')
            .next()
            .and_then(|code| code.split_once('='))
            .map(|(k, _)| k.trim());
        if key.is_some_and(|k| keys.iter().any(|name| name.eq_ignore_ascii_case(k))) {
            continue;
        }
        out.push(raw);
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// What the person applied: the lines to keep in the settings file.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub lines: Vec<(&'static str, String)>,
}

/// The panel and the clients it works on.
pub struct ServerUi {
    pub panel: ServerPanel,
    /// The client when there is no address (the panel works on it empty).
    bare: sc::Client,
    /// The client of the queue, once there is an address.
    bridge: Option<ServerClient>,
}

impl std::fmt::Debug for ServerUi {
    // neither the token nor a password is written out
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerUi")
            .field("link", &self.client().link())
            .field("logged_in", &self.client().session().is_some())
            .field("has_address", &self.bridge.is_some())
            .finish()
    }
}

impl ServerUi {
    /// The panel for the settings of the program, over the plain HTTP client of the suite. Settings the client does not accept start the panel empty, for the person to fix.
    pub fn new(settings: &Settings) -> ServerUi {
        let s = settings_of(settings).unwrap_or_default();
        ServerUi {
            panel: ServerPanel::new(&s, APP),
            bare: sc::Client::connect(s),
            bridge: ServerClient::connect(settings.clone()).ok(),
        }
    }

    /// The panel over a client that was made apart (a server in memory, a tunnel).
    pub fn with_bridge(bridge: ServerClient) -> ServerUi {
        let s = settings_of(bridge.settings()).unwrap_or_default();
        ServerUi {
            panel: ServerPanel::new(&s, APP),
            bare: sc::Client::connect(s),
            bridge: Some(bridge),
        }
    }

    fn client(&self) -> &sc::Client {
        match &self.bridge {
            Some(b) => b.inner(),
            None => &self.bare,
        }
    }

    /// The client of the queue, when there is an address to talk to.
    pub fn device_client(&mut self) -> Option<&mut ServerClient> {
        self.bridge.as_mut()
    }

    /// Whether there is a server to talk to.
    pub fn has_server(&self) -> bool {
        self.bridge.is_some()
    }

    /// Reads the state of the link again (the window calls it every frame; it only reads).
    pub fn refresh(&mut self) {
        let Self {
            panel,
            bare,
            bridge,
        } = self;
        match bridge {
            Some(b) => panel.refresh(b.inner()),
            None => panel.refresh(bare),
        };
    }

    /// Does what the window asked. When the person applied the settings they are put in `settings`, the clients are made again with them and the lines to keep in the file are
    /// given.
    pub fn handle(&mut self, action: Action, settings: &mut Settings) -> Option<Applied> {
        let Self {
            panel,
            bare,
            bridge,
        } = self;
        let client = match bridge.as_mut() {
            Some(b) => b.inner_mut(),
            None => bare,
        };
        let applied = panel.handle(action, client)?;
        apply_to(settings, &applied).ok()?;
        self.bare = sc::Client::connect(applied);
        self.bridge = ServerClient::connect(settings.clone()).ok();
        self.refresh();
        Some(Applied {
            lines: lines_of(settings),
        })
    }

    /// Starts again from the settings of a configuration (when another file of settings is loaded); a login is let go.
    pub fn reload(&mut self, config: &AppConfig) {
        *self = ServerUi::new(&config.server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};

    fn settings(address: &str, token: &str) -> Settings {
        let mut s = Settings::default();
        s.set("server_address", address).unwrap();
        s.set("server_token", token).unwrap();
        s
    }

    #[test]
    fn the_languages_of_the_window_go_to_the_languages_of_the_panel_and_of_the_client() {
        let mut seen = std::collections::HashSet::new();
        for l in Lang::ALL {
            assert_eq!(view_lang(l).code(), l.code());
            assert!(seen.insert(format!("{:?}", client_lang(l))), "{}", l.code());
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn the_settings_go_to_the_client_and_back_and_an_empty_address_is_allowed() {
        let mut s = settings("10.0.0.5:18200", "secret-token");
        s.set("server_machine", "workshop").unwrap();
        s.set("server_timeout_s", "30").unwrap();
        s.set("server_mdns", "on").unwrap();
        let c = settings_of(&s).unwrap();
        assert_eq!(c.server_address, "10.0.0.5:18200");
        assert_eq!(c.server_token.expose(), "secret-token");
        assert_eq!(
            (c.server_timeout_s, c.server_retries, c.server_mdns),
            (30, 2, true)
        );
        let mut back = Settings::default();
        apply_to(&mut back, &c).unwrap();
        assert_eq!(back, s);
        // no address: the panel starts empty
        let empty = settings_of(&Settings::default()).unwrap();
        assert!(empty.server_address.is_empty());
    }

    #[test]
    fn the_lines_to_keep_never_carry_the_token() {
        let s = settings("10.0.0.5:18200", "secret-token");
        let lines = lines_of(&s);
        assert!(lines
            .iter()
            .all(|(k, v)| *k != "server_token" && !v.contains("secret-token")));
        assert_eq!(lines.len(), 6);
        let text = merge_lines("", &lines);
        assert!(!text.contains("secret-token") && !text.contains("server_token"));
        // even if a caller asks for it
        let sneaky = merge_lines("", &[("server_token", "secret-token".to_string())]);
        assert!(!sneaky.contains("secret-token"), "{sneaky}");
    }

    #[test]
    fn merging_lines_keeps_the_rest_of_the_file_as_it_was() {
        let before = "# my printers\nlang = es\nserver_address = old:1 # the old one\nserver_retries = 5\ndefault_printer = prusa-mk4s\nserver_address = twice:2\n";
        let lines = vec![
            ("server_address", "new:2".to_string()),
            ("server_retries", "1".to_string()),
            ("server_mdns", "on".to_string()),
        ];
        let after = merge_lines(before, &lines);
        assert_eq!(
            after,
            "# my printers\nlang = es\nserver_address = new:2\nserver_retries = 1\ndefault_printer = prusa-mk4s\nserver_mdns = on\n"
        );
        // the result is a file the program reads
        let c = AppConfig::from_text(&after).unwrap();
        assert_eq!(c.server.server_address, "new:2");
        assert_eq!(c.default_printer, "prusa-mk4s");
        // merging again changes nothing
        assert_eq!(merge_lines(&after, &lines), after);
    }

    #[test]
    fn without_an_address_there_is_a_panel_and_no_queue_and_with_one_both() {
        let mut ui = ServerUi::new(&Settings::default());
        assert!(!ui.has_server() && ui.device_client().is_none());
        ui.refresh();
        let mut ui = ServerUi::new(&settings("192.168.0.180:18200", ""));
        assert!(ui.has_server() && ui.device_client().is_some());
        assert!(!format!("{ui:?}").contains("secret"));
    }

    #[test]
    fn applying_the_form_makes_the_clients_again_and_gives_the_lines_to_keep() {
        let sim = SimPrinterServer::new();
        let start = settings("192.168.0.180:18200", TOKEN);
        let bridge = ServerClient::new(start.clone(), Box::new(sim)).unwrap();
        let mut ui = ServerUi::with_bridge(bridge);
        assert!(ui.has_server());
        let mut cfg = AppConfig {
            server: start,
            ..AppConfig::default()
        };
        // the person empties the address and applies: there is no server any more
        ui.panel.form.address.clear();
        let applied = ui.handle(Action::Apply, &mut cfg.server);
        let applied = applied.expect("the form was valid");
        assert!(applied
            .lines
            .iter()
            .any(|(k, v)| *k == "server_address" && v.is_empty()));
        assert!(cfg.server.server_address.is_empty());
        assert!(!ui.has_server());
        // a second apply with an address that is not one is refused and changes nothing
        ui.panel.form.address = "http://wrong".to_string();
        assert!(ui.handle(Action::Apply, &mut cfg.server).is_none());
        assert!(cfg.server.server_address.is_empty());
    }

    #[test]
    fn lines_are_taken_out_of_a_settings_file_and_the_rest_is_left_as_it_was() {
        let text = "# my settings
lang = es
printer_limits_a = 1,2,none # mine
PRINTER_LIMITS_B = 3,4,none
grid_mm = 20
";
        let out = drop_keys(text, &["printer_limits_a", "printer_limits_b"]);
        assert_eq!(
            out,
            "# my settings
lang = es
grid_mm = 20
"
        );
        assert_eq!(drop_keys(text, &[]), text);
        assert_eq!(
            drop_keys("", &["x"]),
            "
"
        );
        // what is merged and then dropped leaves the file as it was
        let merged = merge_lines(text, &[("theme", "light".to_string())]);
        assert_eq!(drop_keys(&merged, &["theme"]), drop_keys(text, &[]));
    }
}
