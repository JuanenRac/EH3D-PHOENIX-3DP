//! The settings file of the program: the limits of the readers, the link to the server and the language, in one file of `key = value` lines.
//! A key that no part knows is an error that names the line, so a typo is not silently ignored.

use crate::i18n::Lang;
use p3dp_core::kv;
use p3dp_core::Limits;
use p3dp_server_bridge::Settings;

/// The settings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppConfig {
    pub limits: Limits,
    pub server: Settings,
    pub lang: Option<Lang>,
    /// The id of the printer profile to start with (empty: none).
    pub default_printer: String,
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
                other => return Err(at(format!("the key {other} is not known"))),
            }
        }
        Ok(c)
    }

    /// The settings as text (the token is `***` unless `reveal`).
    pub fn to_text(&self, reveal: bool) -> String {
        let mut s = String::new();
        if let Some(l) = self.lang {
            s.push_str(&format!("lang = {}\n", l.code()));
        }
        s.push_str(&format!("default_printer = {}\n", self.default_printer));
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
