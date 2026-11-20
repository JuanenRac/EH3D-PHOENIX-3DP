//! Bambu Lab over the local network: EXPERIMENTAL and not connected to anything. This file holds the part of the design that is plain data and can be tested without a
//! printer: the topics, the shape of the messages, the table of states and the checks of the serial number. The driver itself answers `Experimental` to everything.
//! The cloud is not part of it. Nothing here has been tried on a Bambu printer.

use crate::driver::PrinterDriver;
use crate::error::LinkError;
use crate::ftps;
use crate::mqtt::{self, Packet};
use crate::pin::Fingerprint;
use crate::types::{Caps, Command, Family, JobState, PrinterState};
use p3dp_core::json::Value;
use p3dp_core::Secret;

/// What this module is, in one sentence, for the screens that show it.
pub const NOTICE: &str = "Bambu LAN is experimental: it is designed and its messages are built, but it does not connect and has not been tried on a printer.";

/// The `gcode_state` the printers report, in the one list of the program.
pub fn map_gcode_state(s: &str) -> JobState {
    match s.to_ascii_uppercase().as_str() {
        "IDLE" => JobState::Idle,
        "PREPARE" => JobState::Preparing,
        "RUNNING" => JobState::Printing,
        "PAUSE" => JobState::Paused,
        "FINISH" => JobState::Finished,
        "FAILED" => JobState::Failed,
        _ => JobState::Busy,
    }
}

/// Whether a text can be a serial number (8 to 20 letters and digits). It goes into a topic, so nothing else is allowed.
pub fn valid_serial(s: &str) -> bool {
    (8..=20).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The topic the printer publishes its state on.
pub fn report_topic(serial: &str) -> Result<String, LinkError> {
    if !valid_serial(serial) {
        return Err(LinkError::Invalid(
            "the serial number must be 8 to 20 letters and digits".to_string(),
        ));
    }
    Ok(format!("device/{serial}/report"))
}

/// The topic the printer listens on.
pub fn request_topic(serial: &str) -> Result<String, LinkError> {
    if !valid_serial(serial) {
        return Err(LinkError::Invalid(
            "the serial number must be 8 to 20 letters and digits".to_string(),
        ));
    }
    Ok(format!("device/{serial}/request"))
}

/// The message that asks for the whole state.
pub fn pushall_payload(sequence: u32) -> String {
    Value::object(vec![(
        "pushing",
        Value::object(vec![
            ("sequence_id", Value::Str(sequence.to_string())),
            ("command", Value::Str("pushall".to_string())),
        ]),
    )])
    .to_json()
}

/// The message of a control of the print: `pause`, `resume` or `stop`.
pub fn control_payload(sequence: u32, command: &str) -> Result<String, LinkError> {
    if !matches!(command, "pause" | "resume" | "stop") {
        return Err(LinkError::Invalid(format!(
            "{command:?} is not a control the design knows"
        )));
    }
    Ok(Value::object(vec![(
        "print",
        Value::object(vec![
            ("sequence_id", Value::Str(sequence.to_string())),
            ("command", Value::Str(command.to_string())),
            ("param", Value::Str(String::new())),
        ]),
    )])
    .to_json())
}

/// The port of MQTT over TLS on the printer.
pub const MQTT_PORT: u16 = 8883;
/// The port of FTP over implicit TLS on the printer.
pub const FTPS_PORT: u16 = 990;
/// The user name of the local link; the password is the access code shown on the screen of the printer.
pub const LAN_USER: &str = "bblp";

/// What the local link to one printer needs. The fingerprint of the certificate is not optional: a printer answers with a certificate it made itself, and without a fingerprint to
/// compare it with there is nothing to tell the printer from anything else on the network, so the settings are refused.
#[derive(Clone, PartialEq, Eq)]
pub struct LanSettings {
    pub host: String,
    pub serial: String,
    pub access_code: Secret,
    pub fingerprint: Fingerprint,
}

impl std::fmt::Debug for LanSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanSettings")
            .field("host", &self.host)
            .field("serial", &self.serial)
            .field("access_code", &self.access_code)
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl LanSettings {
    /// Checks and keeps the settings. The host is a name or an address (letters, digits, dots and hyphens), the access code is the 8 letters and digits of the screen, and the
    /// fingerprint is the 64 hexadecimal digits of the SHA-256 of the certificate.
    pub fn new(
        host: &str,
        serial: &str,
        access_code: &str,
        fingerprint: &str,
    ) -> Result<LanSettings, LinkError> {
        if host.is_empty()
            || host.len() > 253
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            return Err(LinkError::Invalid(
                "the host is a name or an address: letters, digits, dots and hyphens".to_string(),
            ));
        }
        report_topic(serial)?;
        if access_code.len() != 8 || !access_code.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(LinkError::Invalid(
                "the access code is the 8 letters and digits that the screen of the printer shows"
                    .to_string(),
            ));
        }
        if fingerprint.trim().is_empty() {
            return Err(LinkError::Invalid(
                "the fingerprint of the certificate of the printer is required: the link is refused without one to check the certificate against"
                    .to_string(),
            ));
        }
        Ok(LanSettings {
            host: host.to_string(),
            serial: serial.to_string(),
            access_code: Secret::new(access_code),
            fingerprint: Fingerprint::parse(fingerprint)?,
        })
    }

    /// The packets of the start of the link, built and not sent: CONNECT with the user and the access code, SUBSCRIBE to the report topic, and the request for the whole state.
    pub fn start_packets(&self, client_id: &str, sequence: u32) -> Result<LanStart, LinkError> {
        let connect = mqtt::encode(&Packet::Connect {
            client_id: client_id.to_string(),
            username: Some(LAN_USER.to_string()),
            password: Some(self.access_code.clone()),
            keep_alive_s: 30,
        })?;
        let subscribe = mqtt::encode(&Packet::Subscribe {
            id: 1,
            topic: report_topic(&self.serial)?,
        })?;
        let pushall = mqtt::encode(&Packet::Publish {
            topic: request_topic(&self.serial)?,
            payload: pushall_payload(sequence).into_bytes(),
            retain: false,
        })?;
        Ok(LanStart {
            connect,
            subscribe,
            pushall,
        })
    }

    /// The packet of a control of the print.
    pub fn control_packet(&self, sequence: u32, command: &str) -> Result<Vec<u8>, LinkError> {
        mqtt::encode(&Packet::Publish {
            topic: request_topic(&self.serial)?,
            payload: control_payload(sequence, command)?.into_bytes(),
            retain: false,
        })
    }

    /// The commands of the upload of a file by FTP over TLS.
    pub fn upload_steps(&self, name: &str) -> Result<Vec<ftps::Step>, LinkError> {
        ftps::upload_steps(LAN_USER, &self.access_code, name)
    }

    /// Whether the certificate that a printer presented (its DER bytes) is the one these settings pin.
    pub fn check_certificate(&self, der: &[u8]) -> Result<(), LinkError> {
        crate::pin::verify(der, Some(&self.fingerprint))
    }
}

/// The packets that start the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanStart {
    pub connect: Vec<u8>,
    pub subscribe: Vec<u8>,
    pub pushall: Vec<u8>,
}

/// The driver of the design: it exists so that the rest of the program can already name it, and it refuses to do anything.
#[derive(Debug, Clone)]
pub struct BambuDriver {
    serial: String,
}

impl BambuDriver {
    /// A driver for a serial number (it is checked).
    pub fn new(serial: &str) -> Result<BambuDriver, LinkError> {
        report_topic(serial)?;
        Ok(BambuDriver {
            serial: serial.to_string(),
        })
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// It is experimental.
    pub fn is_experimental(&self) -> bool {
        true
    }
}

impl PrinterDriver for BambuDriver {
    fn family(&self) -> Family {
        Family::Bambu
    }

    fn capabilities(&self) -> Caps {
        Caps::default()
    }

    fn status(&mut self) -> Result<PrinterState, LinkError> {
        Err(LinkError::Experimental(
            "the link to the printer is not built",
        ))
    }

    fn command(&mut self, _command: &Command) -> Result<(), LinkError> {
        Err(LinkError::Experimental(
            "the link to the printer is not built",
        ))
    }

    fn upload(&mut self, _name: &str, _data: &[u8], _start: bool) -> Result<(), LinkError> {
        Err(LinkError::Experimental(
            "the upload to the printer is not built",
        ))
    }

    fn list_files(&mut self) -> Result<Vec<String>, LinkError> {
        Err(LinkError::Experimental("the list of files is not built"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::json;

    #[test]
    fn the_state_table() {
        for (s, want) in [
            ("IDLE", JobState::Idle),
            ("PREPARE", JobState::Preparing),
            ("RUNNING", JobState::Printing),
            ("PAUSE", JobState::Paused),
            ("FINISH", JobState::Finished),
            ("FAILED", JobState::Failed),
            ("running", JobState::Printing),
            ("SLICING", JobState::Busy),
        ] {
            assert_eq!(map_gcode_state(s), want, "{s}");
        }
    }

    #[test]
    fn serial_numbers_go_into_topics_only_if_they_are_plain() {
        assert_eq!(
            report_topic("01P00A123456789").unwrap(),
            "device/01P00A123456789/report"
        );
        assert_eq!(
            request_topic("01P00A123456789").unwrap(),
            "device/01P00A123456789/request"
        );
        for bad in [
            "",
            "short",
            "has space 12345",
            "a/b/c/d/e/f/g/h",
            "#",
            "01P00A123456789\n",
            &"A".repeat(21),
        ] {
            assert!(!valid_serial(bad), "{bad:?}");
            assert!(report_topic(bad).is_err());
        }
    }

    #[test]
    fn messages_are_valid_json_of_the_right_shape() {
        let v = json::parse(&pushall_payload(7)).unwrap();
        assert_eq!(
            v.path(&["pushing", "command"]).and_then(Value::as_str),
            Some("pushall")
        );
        assert_eq!(
            v.path(&["pushing", "sequence_id"]).and_then(Value::as_str),
            Some("7")
        );
        for c in ["pause", "resume", "stop"] {
            let v = json::parse(&control_payload(1, c).unwrap()).unwrap();
            assert_eq!(
                v.path(&["print", "command"]).and_then(Value::as_str),
                Some(c)
            );
        }
        assert!(control_payload(1, "start_with_gcode_injection").is_err());
    }

    const CODE: &str = "Ab12Cd34";
    const DER: &[u8] = b"the self-signed certificate of a printer";

    fn lan() -> LanSettings {
        LanSettings::new(
            "192.168.1.50",
            "01P00A123456789",
            CODE,
            &Fingerprint::of_der(DER).to_hex(),
        )
        .unwrap()
    }

    #[test]
    fn the_settings_of_the_local_link_need_everything_and_above_all_the_fingerprint() {
        let l = lan();
        assert_eq!(
            (l.host.as_str(), l.serial.as_str()),
            ("192.168.1.50", "01P00A123456789")
        );
        let fp = Fingerprint::of_der(DER).to_hex();
        for (host, serial, code, fingerprint, want) in [
            ("192.168.1.50", "01P00A123456789", CODE, "", "fingerprint"),
            (
                "192.168.1.50",
                "01P00A123456789",
                CODE,
                "   ",
                "fingerprint",
            ),
            (
                "192.168.1.50",
                "01P00A123456789",
                CODE,
                "not a fingerprint",
                "fingerprint",
            ),
            (
                "192.168.1.50",
                "01P00A123456789",
                "short",
                fp.as_str(),
                "access code",
            ),
            (
                "192.168.1.50",
                "01P00A123456789",
                "Ab12Cd3!",
                fp.as_str(),
                "access code",
            ),
            ("", "01P00A123456789", CODE, fp.as_str(), "host"),
            ("host name", "01P00A123456789", CODE, fp.as_str(), "host"),
            ("a/b", "01P00A123456789", CODE, fp.as_str(), "host"),
            ("192.168.1.50", "bad", CODE, fp.as_str(), "serial"),
        ] {
            let e = LanSettings::new(host, serial, code, fingerprint)
                .unwrap_err()
                .to_string();
            assert!(e.contains(want), "{e}");
        }
        // the access code does not reach a log
        assert!(!format!("{l:?}").contains(CODE));
    }

    #[test]
    fn the_packets_that_start_the_link_say_what_the_design_says_and_read_back() {
        let l = lan();
        let s = l.start_packets("phoenix-1", 5).unwrap();
        match mqtt::decode(&s.connect).unwrap().unwrap().0 {
            Packet::Connect {
                client_id,
                username,
                password,
                keep_alive_s,
            } => {
                assert_eq!(client_id, "phoenix-1");
                assert_eq!(username.as_deref(), Some("bblp"));
                assert_eq!(password.unwrap().expose(), CODE);
                assert_eq!(keep_alive_s, 30);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            mqtt::decode(&s.subscribe).unwrap().unwrap().0,
            Packet::Subscribe {
                id: 1,
                topic: "device/01P00A123456789/report".to_string()
            }
        );
        match mqtt::decode(&s.pushall).unwrap().unwrap().0 {
            Packet::Publish { topic, payload, .. } => {
                assert_eq!(topic, "device/01P00A123456789/request");
                let v = p3dp_core::json::parse(std::str::from_utf8(&payload).unwrap()).unwrap();
                assert_eq!(
                    v.path(&["pushing", "sequence_id"]).and_then(Value::as_str),
                    Some("5")
                );
            }
            other => panic!("{other:?}"),
        }
        // a control goes to the request topic and only the three controls of the design exist
        for c in ["pause", "resume", "stop"] {
            assert!(l.control_packet(2, c).is_ok());
        }
        assert!(l.control_packet(2, "gcode_line").is_err());
        // the upload commands carry the access code only in PASS, and the log does not
        let steps = l.upload_steps("cube.gcode.3mf").unwrap();
        assert!(steps.iter().all(|s| !s.masked().contains(CODE)));
        assert!(steps[0].line.starts_with("USER bblp"));
    }

    /// A broker made of a few lines, to run the whole exchange in memory: it accepts one user and one password, grants one subscription and answers the request for the state.
    fn broker(inbox: &[u8], user: &str, pass: &str, serial: &str) -> Vec<Packet> {
        let mut out = Vec::new();
        let mut rest = inbox;
        let mut subscribed = false;
        while let Some((p, used)) = mqtt::decode(rest).unwrap() {
            rest = &rest[used..];
            match p {
                Packet::Connect {
                    username, password, ..
                } => {
                    let ok = username.as_deref() == Some(user)
                        && password.map(|p| p.expose().to_string()).as_deref() == Some(pass);
                    out.push(Packet::ConnAck {
                        session_present: false,
                        code: if ok {
                            mqtt::ConnectCode::Accepted
                        } else {
                            mqtt::ConnectCode::BadCredentials
                        },
                    });
                    if !ok {
                        return out;
                    }
                }
                Packet::Subscribe { id, topic } => {
                    subscribed = topic == format!("device/{serial}/report");
                    out.push(Packet::SubAck {
                        id,
                        granted: if subscribed { 0 } else { 0x80 },
                    });
                }
                Packet::Publish { topic, payload, .. }
                    if subscribed
                        && topic == format!("device/{serial}/request")
                        && payload.windows(7).any(|w| w == b"pushall") =>
                {
                    out.push(Packet::Publish {
                        topic: format!("device/{serial}/report"),
                        payload: br#"{"print":{"gcode_state":"RUNNING"}}"#.to_vec(),
                        retain: false,
                    });
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn the_whole_exchange_runs_against_a_broker_in_memory_and_a_wrong_code_is_refused() {
        let l = lan();
        let s = l.start_packets("phoenix-1", 1).unwrap();
        let mut inbox = s.connect.clone();
        inbox.extend(&s.subscribe);
        inbox.extend(&s.pushall);
        let answers = broker(&inbox, "bblp", CODE, "01P00A123456789");
        assert_eq!(answers.len(), 3);
        assert!(matches!(
            answers[0],
            Packet::ConnAck {
                code: mqtt::ConnectCode::Accepted,
                ..
            }
        ));
        assert!(matches!(answers[1], Packet::SubAck { granted: 0, .. }));
        match &answers[2] {
            Packet::Publish { payload, .. } => {
                let v = p3dp_core::json::parse(std::str::from_utf8(payload).unwrap()).unwrap();
                let state = v
                    .path(&["print", "gcode_state"])
                    .and_then(Value::as_str)
                    .unwrap();
                assert_eq!(map_gcode_state(state), JobState::Printing);
            }
            other => panic!("{other:?}"),
        }
        // the wrong code: the broker answers CONNACK with a refusal and goes no further
        let answers = broker(&inbox, "bblp", "00000000", "01P00A123456789");
        assert_eq!(answers.len(), 1);
        match &answers[0] {
            Packet::ConnAck { code, .. } => assert_eq!(code.into_result(), Err(LinkError::Auth)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_the_pinned_certificate_is_taken() {
        let l = lan();
        assert!(l.check_certificate(DER).is_ok());
        assert!(l
            .check_certificate(b"somebody else on the network")
            .is_err());
    }

    #[test]
    fn the_driver_refuses_everything() {
        let mut d = BambuDriver::new("01P00A123456789").unwrap();
        assert!(d.is_experimental());
        assert_eq!(d.family(), Family::Bambu);
        assert_eq!(d.capabilities(), Caps::default());
        assert!(matches!(d.status(), Err(LinkError::Experimental(_))));
        assert!(matches!(
            d.command(&Command::Pause),
            Err(LinkError::Experimental(_))
        ));
        assert!(matches!(
            d.upload("a.3mf", b"x", false),
            Err(LinkError::Experimental(_))
        ));
        assert!(matches!(d.list_files(), Err(LinkError::Experimental(_))));
        assert!(BambuDriver::new("bad").is_err());
        assert!(NOTICE.contains("experimental"));
        assert_eq!(d.serial(), "01P00A123456789");
    }
}
