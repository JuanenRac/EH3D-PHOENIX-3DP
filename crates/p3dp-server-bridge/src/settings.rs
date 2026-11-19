//! The settings of the link to the server: the same keys, ranges and defaults as the common client of the suite, plus the switch of the stand-alone mode.

use p3dp_core::kv;
use p3dp_core::{CoreError, Secret};
use p3dp_link::transport::percent_encode;

/// The keys of the link.
#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    /// `host:port`, or empty when there is no server.
    pub server_address: String,
    /// The token of the program (never shown, never logged).
    pub server_token: Secret,
    /// The machine the jobs are for.
    pub server_machine: String,
    /// How long one try may take, 1 to 120 seconds.
    pub server_timeout_s: u32,
    /// How many more tries after a failure on the network, 0 to 5.
    pub server_retries: u32,
    /// Look for the server on the local network (off by default; nothing announces it yet).
    pub server_mdns: bool,
    /// The most MiB read from one answer, 1 to 256.
    pub server_max_answer_mib: u32,
    /// The stand-alone mode: talk to a printer directly when there is no server. Off by default.
    pub direct_mode: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            server_address: String::new(),
            server_token: Secret::default(),
            server_machine: String::new(),
            server_timeout_s: 15,
            server_retries: 2,
            server_mdns: false,
            server_max_answer_mib: 8,
            direct_mode: false,
        }
    }
}

// the token never reaches a log through `{:?}`
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("server_address", &self.server_address)
            .field("server_token", &self.server_token)
            .field("server_machine", &self.server_machine)
            .field("server_timeout_s", &self.server_timeout_s)
            .field("server_retries", &self.server_retries)
            .field("server_mdns", &self.server_mdns)
            .field("server_max_answer_mib", &self.server_max_answer_mib)
            .field("direct_mode", &self.direct_mode)
            .finish()
    }
}

/// How the program reaches its printers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    /// Through the server at this address (the normal way).
    ViaServer(String),
    /// There is no address but discovery is on; the program has no resolver, so this is the state "look for it".
    Discover,
    /// Stand-alone: straight to a printer, with no shared queue and no permissions.
    Direct,
    /// Nothing is set: nothing is sent anywhere.
    NotConfigured,
}

/// Checks a `host:port`.
pub fn valid_address(a: &str) -> Result<(), String> {
    if a.is_empty() {
        return Ok(());
    }
    if a.contains("://") {
        return Err("write host:port, without http:// or a path".to_string());
    }
    if a.contains(char::is_whitespace)
        || a.contains('/')
        || a.contains('?')
        || a.contains('#')
        || a.contains('@')
    {
        return Err("an address has no spaces, no path and no user".to_string());
    }
    let (host, port) = a
        .rsplit_once(':')
        .ok_or("the port is missing (host:port)")?;
    let host_ok = if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !inner.is_empty()
            && inner
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
    } else {
        !host.is_empty()
            && host.len() <= 253
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
    };
    if !host_ok {
        return Err("the host has characters that a host cannot have".to_string());
    }
    match port.parse::<u32>() {
        Ok(p) if (1..=65535).contains(&p) => Ok(()),
        _ => Err("the port must be a number from 1 to 65535".to_string()),
    }
}

fn is_local_host(host: &str) -> bool {
    if matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
    {
        return true;
    }
    if let Some(rest) = host.strip_prefix("172.") {
        if let Some(Ok(second)) = rest.split('.').next().map(str::parse::<u8>) {
            return (16..=31).contains(&second);
        }
    }
    host.ends_with(".local")
}

impl Settings {
    pub const KEYS: [&'static str; 8] = [
        "server_address",
        "server_token",
        "server_machine",
        "server_timeout_s",
        "server_retries",
        "server_mdns",
        "server_max_answer_mib",
        "direct_mode",
    ];

    /// Sets one key. A value out of its range is an error that names the key and changes nothing; a key that is not one of these gives `Ok(false)` (it belongs to the program).
    pub fn set(&mut self, key: &str, value: &str) -> Result<bool, CoreError> {
        match key {
            "server_address" => {
                let v = value.trim();
                valid_address(v).map_err(|e| CoreError::Invalid(format!("server_address: {e}")))?;
                self.server_address = v.to_string();
            }
            "server_token" => {
                let v = value.trim();
                if v.contains(char::is_whitespace) || v.len() > 512 {
                    return Err(CoreError::Invalid(
                        "server_token: no spaces and at most 512 characters".to_string(),
                    ));
                }
                self.server_token = Secret::new(v);
            }
            "server_machine" => {
                let v = value.trim();
                if v.len() > 64
                    || !v
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                {
                    return Err(CoreError::Invalid(
                        "server_machine: letters, digits, - _ . and at most 64 characters"
                            .to_string(),
                    ));
                }
                self.server_machine = v.to_string();
            }
            "server_timeout_s" => self.server_timeout_s = kv::int_in(key, value, 1, 120)? as u32,
            "server_retries" => self.server_retries = kv::int_in(key, value, 0, 5)? as u32,
            "server_mdns" => self.server_mdns = kv::on_off(key, value)?,
            "server_max_answer_mib" => {
                self.server_max_answer_mib = kv::int_in(key, value, 1, 256)? as u32
            }
            "direct_mode" => self.direct_mode = kv::on_off(key, value)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Reads the keys it knows from the text of a settings file; the error names the line.
    pub fn from_text(text: &str) -> Result<Settings, CoreError> {
        let mut s = Settings::default();
        for e in kv::parse(text)? {
            s.set(&e.key, &e.value)
                .map_err(|err| CoreError::Invalid(format!("line {}: {err}", e.line)))?;
        }
        Ok(s)
    }

    /// The settings as text. The token is written as `***` unless `reveal` is true (only the program that writes its own file asks for it).
    pub fn to_text(&self, reveal: bool) -> String {
        let token = if self.server_token.is_empty() {
            String::new()
        } else if reveal {
            self.server_token.expose().to_string()
        } else {
            "***".to_string()
        };
        format!(
            "server_address = {}\nserver_token = {}\nserver_machine = {}\nserver_timeout_s = {}\nserver_retries = {}\nserver_mdns = {}\nserver_max_answer_mib = {}\ndirect_mode = {}\n",
            self.server_address,
            token,
            self.server_machine,
            self.server_timeout_s,
            self.server_retries,
            if self.server_mdns { "on" } else { "off" },
            self.server_max_answer_mib,
            if self.direct_mode { "on" } else { "off" }
        )
    }

    /// How the program reaches its printers: through the server if there is an address, else discovery if it is on, else straight to a printer if the stand-alone mode is on.
    pub fn connection(&self) -> Connection {
        if !self.server_address.is_empty() {
            Connection::ViaServer(self.server_address.clone())
        } else if self.server_mdns {
            Connection::Discover
        } else if self.direct_mode {
            Connection::Direct
        } else {
            Connection::NotConfigured
        }
    }

    /// A warning when the token would travel in the clear to an address that is not in the local network.
    pub fn plain_token_warning(&self) -> Option<&'static str> {
        if self.server_token.is_empty() || self.server_address.is_empty() {
            return None;
        }
        let host = self
            .server_address
            .rsplit_once(':')
            .map_or(self.server_address.as_str(), |(h, _)| h);
        if is_local_host(host) {
            None
        } else {
            Some("the token travels without encryption to an address outside the local network: put the server behind HTTPS")
        }
    }

    /// The path of a route of this machine, `/api/v1/machines/<machine>/<rest>`, with the name made safe for a path.
    pub fn machine_path(&self, rest: &str) -> String {
        format!(
            "/api/v1/machines/{}/{}",
            percent_encode(&self.server_machine),
            rest.trim_start_matches('/')
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_ones_of_the_common_client() {
        let s = Settings::default();
        assert_eq!(
            (
                s.server_timeout_s,
                s.server_retries,
                s.server_max_answer_mib
            ),
            (15, 2, 8)
        );
        assert!(!s.server_mdns && !s.direct_mode);
        assert_eq!(s.connection(), Connection::NotConfigured);
        assert_eq!(Settings::from_text(&s.to_text(true)).unwrap(), s);
    }

    #[test]
    fn every_key_is_known_and_ranges_are_enforced() {
        let mut s = Settings::default();
        for (k, ok, bad) in [
            ("server_timeout_s", "120", "121"),
            ("server_timeout_s", "1", "0"),
            ("server_retries", "5", "6"),
            ("server_max_answer_mib", "256", "257"),
            ("server_mdns", "on", "maybe"),
            ("direct_mode", "off", "2x"),
            ("server_address", "10.0.0.5:18200", "http://x:1"),
            ("server_machine", "bungard-1", "a b"),
            ("server_token", "abc123", "a b"),
        ] {
            assert_eq!(s.set(k, ok), Ok(true), "{k}");
            assert!(s.set(k, bad).is_err(), "{k} {bad}");
        }
        assert_eq!(s.set("something_else", "1"), Ok(false));
        let text = s.to_text(false);
        for k in Settings::KEYS {
            assert!(text.contains(k), "{k}");
        }
    }

    #[test]
    fn a_bad_value_changes_nothing() {
        let mut s = Settings::default();
        s.set("server_retries", "4").unwrap();
        assert!(s.set("server_retries", "9").is_err());
        assert_eq!(s.server_retries, 4);
    }

    #[test]
    fn addresses() {
        for ok in [
            "",
            "localhost:18200",
            "10.0.0.2:80",
            "my-server.local:443",
            "[::1]:18200",
            "[fe80::1]:1",
        ] {
            assert!(valid_address(ok).is_ok(), "{ok}");
        }
        for bad in [
            "host",
            "host:0",
            "host:70000",
            "host:abc",
            "http://h:1",
            "h:1/x",
            "h :1",
            "user@h:1",
            ":80",
            "[]:1",
            "ho!st:1",
            "[xyz]:1",
        ] {
            assert!(valid_address(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_token_is_never_shown() {
        let mut s = Settings::default();
        s.set("server_token", "topsecret").unwrap();
        assert!(!format!("{s:?}").contains("topsecret"));
        assert!(!s.to_text(false).contains("topsecret"));
        assert!(s.to_text(false).contains("***"));
        assert!(s.to_text(true).contains("topsecret"));
    }

    #[test]
    fn the_connection_depends_on_what_is_set() {
        let mut s = Settings::default();
        s.set("direct_mode", "on").unwrap();
        assert_eq!(s.connection(), Connection::Direct);
        s.set("server_mdns", "on").unwrap();
        assert_eq!(s.connection(), Connection::Discover);
        s.set("server_address", "10.0.0.1:18200").unwrap();
        assert_eq!(
            s.connection(),
            Connection::ViaServer("10.0.0.1:18200".into())
        );
    }

    #[test]
    fn the_warning_for_a_token_in_the_clear() {
        let mut s = Settings::default();
        s.set("server_token", "t").unwrap();
        s.set("server_address", "192.168.0.180:18200").unwrap();
        assert!(s.plain_token_warning().is_none());
        for local in [
            "localhost:1",
            "127.0.0.1:1",
            "10.1.2.3:1",
            "172.20.0.1:1",
            "nas.local:1",
        ] {
            s.set("server_address", local).unwrap();
            assert!(s.plain_token_warning().is_none(), "{local}");
        }
        for far in ["example.org:1", "8.8.8.8:1", "172.32.0.1:1"] {
            s.set("server_address", far).unwrap();
            assert!(s.plain_token_warning().is_some(), "{far}");
        }
        s.set("server_token", "").unwrap();
        assert!(s.plain_token_warning().is_none());
    }

    #[test]
    fn the_machine_goes_into_a_path_safely() {
        let mut s = Settings::default();
        s.set("server_machine", "p1s.1").unwrap();
        assert_eq!(s.machine_path("/jobs"), "/api/v1/machines/p1s.1/jobs");
        assert_eq!(s.machine_path("jobs"), "/api/v1/machines/p1s.1/jobs");
    }

    #[test]
    fn a_file_names_the_line_that_is_wrong() {
        let e = Settings::from_text("server_retries = 3\nserver_timeout_s = 500\n")
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("line 2") && e.contains("server_timeout_s"),
            "{e}"
        );
        let s = Settings::from_text("# mine\nserver_address = a:1\nsome_other_key = 3\n").unwrap();
        assert_eq!(s.server_address, "a:1");
    }
}
