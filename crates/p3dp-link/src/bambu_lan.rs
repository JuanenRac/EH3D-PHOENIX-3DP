//! The stand-alone link to a Bambu Lab printer in LAN mode, over the seam of `tls`: it opens a channel only to the pinned certificate, logs in to the MQTT of the printer with the
//! access code, asks for the whole state, reads the report into the normalized state, and sends a pause, a resume or a stop. It is tried against the printer made in memory of
//! `tls::sim`; no real TLS connector exists in this repository and nothing here has talked to a printer. The upload of a file (FTP over TLS) is not built, nor are the start of a
//! file and the temperatures, and the emergency stop is refused: the printers of the family have no command for it that this program knows.

use crate::bambu::{self, map_gcode_state, LanSettings, MQTT_PORT};
use crate::driver::PrinterDriver;
use crate::error::LinkError;
use crate::mqtt::{self, Packet};
use crate::tls::{connect_pinned, PinnedChannel, TlsConnector};
use crate::types::{Caps, Command, Family, JobInfo, PrinterState, Temperature};
use p3dp_core::json::{self, Value};
use p3dp_core::machine::Zone;

/// The most reads the link makes while it waits for one packet, and the most bytes it keeps: a printer that sends nothing or too much is an error, not a wait.
const MAX_READS: usize = 400;
const MAX_BUFFER: usize = 1 << 20;

fn number(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|x| x.is_finite())
}

/// Reads a report of the printer into the normalized state. The report has to carry the `print` object with a `gcode_state`; the figures that are missing are left out (the
/// printers send the changes alone after the first full report), and a figure that is not a number is left out too.
pub fn parse_report(text: &str) -> Result<PrinterState, LinkError> {
    let v = json::parse(text)
        .map_err(|e| LinkError::BadReply(format!("the report is not JSON: {e}")))?;
    let print = v
        .get("print")
        .ok_or_else(|| LinkError::BadReply("the report has no print object".to_string()))?;
    let raw = print
        .get("gcode_state")
        .and_then(Value::as_str)
        .ok_or_else(|| LinkError::BadReply("the report has no gcode_state".to_string()))?;
    let state = map_gcode_state(raw);
    let mut temperatures = Vec::new();
    for (zone, actual, target) in [
        (Zone::Nozzle, "nozzle_temper", "nozzle_target_temper"),
        (Zone::Bed, "bed_temper", "bed_target_temper"),
        (Zone::Chamber, "chamber_temper", "chamber_target_temper"),
    ] {
        if let Some(a) = number(print, actual) {
            temperatures.push(Temperature {
                zone,
                actual: a,
                target: number(print, target).unwrap_or(0.0).max(0.0),
            });
        }
    }
    let file = print
        .get("subtask_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let job = if state.is_active() || !file.is_empty() {
        Some(JobInfo {
            file,
            progress_pct: number(print, "mc_percent").unwrap_or(0.0).clamp(0.0, 100.0),
            elapsed_s: None,
            // the printer says minutes
            remaining_s: number(print, "mc_remaining_time")
                .filter(|m| *m >= 0.0)
                .map(|m| (m * 60.0).round() as u64),
        })
    } else {
        None
    };
    let mut alerts = Vec::new();
    if let Some(code) = number(print, "print_error").filter(|c| *c > 0.0 && *c < 4_000_000_000.0) {
        alerts.push(format!("print_error {:08X}", code as u32));
    }
    Ok(PrinterState {
        family: Family::Bambu,
        state,
        raw_state: raw.to_string(),
        temperatures,
        job,
        alerts,
    })
}

/// The driver of a Bambu Lab printer in LAN mode, over a connector that the owner of the program plugs in. It makes one short connection for each call (a poll), as the drivers
/// of the HTTP printers do.
pub struct BambuLanDriver {
    settings: LanSettings,
    connector: Box<dyn TlsConnector>,
    sequence: u32,
}

impl std::fmt::Debug for BambuLanDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BambuLanDriver")
            .field("settings", &self.settings)
            .field("sequence", &self.sequence)
            .finish()
    }
}

/// A packet reader over a channel: it joins the pieces of the stream and hands out one packet at a time.
struct Reader<'a> {
    channel: &'a mut PinnedChannel,
    buffer: Vec<u8>,
}

impl Reader<'_> {
    fn next(&mut self) -> Result<Packet, LinkError> {
        for _ in 0..MAX_READS {
            if let Some((p, used)) = mqtt::decode(&self.buffer)? {
                self.buffer.drain(..used);
                return Ok(p);
            }
            let mut chunk = [0u8; 4096];
            let n = self.channel.read_some(&mut chunk)?;
            if n == 0 {
                return Err(LinkError::BadReply(
                    "the printer closed the connection".to_string(),
                ));
            }
            self.buffer.extend_from_slice(&chunk[..n]);
            if self.buffer.len() > MAX_BUFFER {
                return Err(LinkError::BadReply(
                    "the printer sent more than a megabyte without a whole packet".to_string(),
                ));
            }
        }
        Err(LinkError::Timeout)
    }
}

impl BambuLanDriver {
    /// A driver for the settings of the link and a connector.
    pub fn new(settings: LanSettings, connector: Box<dyn TlsConnector>) -> BambuLanDriver {
        BambuLanDriver {
            settings,
            connector,
            sequence: 0,
        }
    }

    /// It is not experimental in what it builds, but nothing has run on a printer.
    pub fn is_experimental(&self) -> bool {
        true
    }

    fn next_sequence(&mut self) -> u32 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    /// Opens the channel to the pinned certificate and logs in. The channel comes back with nothing read from it but the answer to the login.
    fn open(
        &mut self,
        sequence: u32,
    ) -> Result<(PinnedChannel, Vec<u8>, bambu::LanStart), LinkError> {
        let mut channel = connect_pinned(
            self.connector.as_mut(),
            &self.settings.host,
            MQTT_PORT,
            &self.settings.fingerprint,
        )?;
        let start = self
            .settings
            .start_packets(&format!("phoenix-3dp-{sequence}"), sequence)?;
        channel.write_all(&start.connect)?;
        let mut reader = Reader {
            channel: &mut channel,
            buffer: Vec::new(),
        };
        match reader.next()? {
            Packet::ConnAck { code, .. } => code.into_result()?,
            other => {
                return Err(LinkError::BadReply(format!(
                    "the printer answered the login with {other:?}"
                )))
            }
        }
        let rest = std::mem::take(&mut reader.buffer);
        Ok((channel, rest, start))
    }

    fn control(&mut self, command: &str) -> Result<(), LinkError> {
        let sequence = self.next_sequence();
        let (mut channel, _rest, _start) = self.open(sequence)?;
        channel.write_all(&self.settings.control_packet(sequence, command)?)?;
        if let Ok(bye) = mqtt::encode(&Packet::Disconnect) {
            let _ = channel.write_all(&bye);
        }
        channel.close();
        Ok(())
    }
}

impl PrinterDriver for BambuLanDriver {
    fn family(&self) -> Family {
        Family::Bambu
    }

    fn capabilities(&self) -> Caps {
        Caps::default()
            .with(Caps::PAUSE)
            .with(Caps::RESUME)
            .with(Caps::CANCEL)
    }

    fn status(&mut self) -> Result<PrinterState, LinkError> {
        let sequence = self.next_sequence();
        let (mut channel, rest, start) = self.open(sequence)?;
        let topic = bambu::report_topic(&self.settings.serial)?;
        let result = (|| {
            channel.write_all(&start.subscribe)?;
            let mut reader = Reader {
                channel: &mut channel,
                buffer: rest,
            };
            match reader.next()? {
                Packet::SubAck { granted: 0, .. } => {}
                Packet::SubAck { .. } => {
                    return Err(LinkError::Forbidden);
                }
                other => {
                    return Err(LinkError::BadReply(format!(
                        "the printer answered the subscription with {other:?}"
                    )))
                }
            }
            reader.channel.write_all(&start.pushall)?;
            // other messages may come first: the first report with a state is the answer
            for _ in 0..16 {
                if let Packet::Publish {
                    topic: t, payload, ..
                } = reader.next()?
                {
                    if t != topic {
                        continue;
                    }
                    if let Ok(text) = std::str::from_utf8(&payload) {
                        if text.contains("gcode_state") {
                            return parse_report(text);
                        }
                    }
                }
            }
            Err(LinkError::BadReply(
                "no report with a state came after the request".to_string(),
            ))
        })();
        if let Ok(bye) = mqtt::encode(&Packet::Disconnect) {
            let _ = channel.write_all(&bye);
        }
        channel.close();
        result
    }

    fn command(&mut self, command: &Command) -> Result<(), LinkError> {
        match command {
            Command::Pause => self.control("pause"),
            Command::Resume => self.control("resume"),
            Command::Cancel => self.control("stop"),
            Command::EmergencyStop => Err(LinkError::Capability("stop in an emergency")),
            Command::Start { .. } => Err(LinkError::Capability("start a file")),
            Command::SetTemp { .. } => Err(LinkError::Capability("set a temperature")),
            Command::SetFan { .. } => Err(LinkError::Capability("set the fan")),
            Command::Gcode { .. } => Err(LinkError::Capability("run G-code")),
        }
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

/// Whether a state is one that needs a person, in the words of this link (kept here so the screens and the queue can ask without a driver).
pub fn needs_attention(s: &PrinterState) -> bool {
    s.state.needs_attention() || !s.alerts.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::Fingerprint;
    use crate::tls::sim::{Failure, Machine, SimPrinter};
    use crate::tls::NoTls;
    use crate::types::JobState;
    use p3dp_core::rng::Rng;

    const CERT: &[u8] = b"the certificate of the printer, as DER bytes";
    const SERIAL: &str = "01P00A123456789";
    const CODE: &str = "Ab7Zk39Q";

    fn lan(cert: &[u8]) -> LanSettings {
        LanSettings::new(
            "192.168.0.14",
            SERIAL,
            CODE,
            &Fingerprint::of_der(cert).to_hex(),
        )
        .unwrap()
    }

    fn driver(printer: &SimPrinter) -> BambuLanDriver {
        BambuLanDriver::new(lan(CERT), Box::new(printer.clone()))
    }

    #[test]
    fn an_idle_printer_is_read_into_the_normalized_state() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        let mut d = driver(&printer);
        let s = d.status().unwrap();
        assert_eq!(s.family, Family::Bambu);
        assert_eq!(s.state, JobState::Idle);
        assert_eq!(s.raw_state, "IDLE");
        assert_eq!(s.temp(Zone::Nozzle).unwrap().actual, 18.0);
        assert_eq!(s.temp(Zone::Bed).unwrap().actual, 20.0);
        assert!(s.temp(Zone::Chamber).is_none());
        assert!(s.job.is_none() && s.alerts.is_empty());
        // the connection is closed again and the printer saw one request for the state
        let seen = printer.seen();
        assert_eq!((seen.open, seen.pushalls, seen.connections), (0, 1, 1));
        assert!(!needs_attention(&s));
    }

    #[test]
    fn a_print_under_way_gives_the_job_the_temperatures_and_the_time_left_in_seconds() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        printer.set_machine(Machine {
            gcode_state: "RUNNING".to_string(),
            nozzle: 219.5,
            nozzle_target: 220.0,
            bed: 59.8,
            bed_target: 60.0,
            chamber: Some(31.0),
            percent: 42,
            remaining_min: 75,
            file: "goblet".to_string(),
            print_error: 0,
        });
        let s = driver(&printer).status().unwrap();
        assert_eq!(s.state, JobState::Printing);
        assert_eq!(s.temp(Zone::Nozzle).unwrap().target, 220.0);
        assert_eq!(s.temp(Zone::Chamber).unwrap().actual, 31.0);
        let job = s.job.unwrap();
        assert_eq!(job.file, "goblet");
        assert_eq!(job.progress_pct, 42.0);
        assert_eq!(job.remaining_s, Some(4500));
    }

    #[test]
    fn an_error_code_is_an_alert_in_hexadecimal() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        let mut m = Machine::idle();
        m.gcode_state = "FAILED".to_string();
        m.print_error = 0x0500_4003;
        printer.set_machine(m);
        let s = driver(&printer).status().unwrap();
        assert_eq!(s.state, JobState::Failed);
        assert_eq!(s.alerts, vec!["print_error 05004003".to_string()]);
        assert!(needs_attention(&s));
    }

    #[test]
    fn pause_resume_and_stop_reach_the_printer_and_nothing_else_does() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        let mut d = driver(&printer);
        d.command(&Command::Pause).unwrap();
        assert_eq!(printer.machine().gcode_state, "PAUSE");
        d.command(&Command::Resume).unwrap();
        assert_eq!(printer.machine().gcode_state, "RUNNING");
        d.command(&Command::Cancel).unwrap();
        assert_eq!(printer.seen().controls, vec!["pause", "resume", "stop"]);
        let sent = printer.seen().bytes;
        for c in [
            Command::EmergencyStop,
            Command::Start {
                file: "a.3mf".into(),
            },
            Command::SetFan { percent: 10.0 },
        ] {
            assert!(
                matches!(d.command(&c), Err(LinkError::Capability(_))),
                "{c:?}"
            );
        }
        assert_eq!(
            printer.seen().bytes,
            sent,
            "a refused command writes nothing"
        );
        assert!(matches!(
            d.upload("a", b"x", false),
            Err(LinkError::Experimental(_))
        ));
        assert!(d.capabilities().contains(Caps::PAUSE | Caps::CANCEL));
        assert!(!d.capabilities().contains(Caps::EMERGENCY_STOP));
    }

    #[test]
    fn a_printer_with_another_certificate_gets_not_one_byte_and_the_error_says_so() {
        let other = SimPrinter::new(b"somebody else on the network", SERIAL, CODE);
        let mut d = BambuLanDriver::new(lan(CERT), Box::new(other.clone()));
        let e = d.status().unwrap_err();
        assert!(e.to_string().contains("not the one pinned"), "{e}");
        assert!(d.command(&Command::Pause).is_err());
        let s = other.seen();
        assert_eq!((s.bytes, s.open), (0, 0));
    }

    #[test]
    fn a_wrong_access_code_is_an_authentication_error_and_the_channel_is_closed() {
        let printer = SimPrinter::new(CERT, SERIAL, "Zz0Yy9Xx");
        let mut d = driver(&printer);
        assert_eq!(d.status().unwrap_err(), LinkError::Auth);
        assert_eq!(printer.seen().bad_logins, 1);
        assert_eq!(printer.seen().open, 0);
    }

    #[test]
    fn a_printer_that_cannot_be_reached_or_breaks_the_handshake_is_no_connection() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        let mut d = driver(&printer);
        for f in [Failure::Refused, Failure::BrokenAfterCertificate] {
            printer.fail(Some(f));
            assert_eq!(d.status().unwrap_err(), LinkError::NoConnection, "{f:?}");
            assert_eq!(printer.seen().open, 0);
        }
        printer.fail(None);
        assert!(d.status().is_ok());
        // with no connector at all, the answer is the reason
        let mut none = BambuLanDriver::new(lan(CERT), Box::new(NoTls));
        assert!(none
            .status()
            .unwrap_err()
            .to_string()
            .contains("no TLS connector"));
    }

    #[test]
    fn the_access_code_never_appears_in_what_the_driver_prints() {
        let printer = SimPrinter::new(CERT, SERIAL, CODE);
        let d = driver(&printer);
        let shown = format!("{d:?}");
        assert!(!shown.contains(CODE), "{shown}");
    }

    #[test]
    fn reports_with_pieces_missing_or_damaged_are_read_or_refused_and_never_panic() {
        // a partial report: only the state
        let s = parse_report(r#"{"print":{"gcode_state":"PAUSE"}}"#).unwrap();
        assert_eq!(s.state, JobState::Paused);
        assert!(s.temperatures.is_empty());
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"print":{}}"#,
            r#"{"print":{"gcode_state":5}}"#,
            r#"{"info":{"gcode_state":"IDLE"}}"#,
        ] {
            assert!(
                matches!(parse_report(bad), Err(LinkError::BadReply(_))),
                "{bad}"
            );
        }
        // figures that are not numbers are left out, a percentage is kept in 0 to 100
        let s = parse_report(
            r#"{"print":{"gcode_state":"RUNNING","nozzle_temper":"hot","bed_temper":55,"mc_percent":250,"mc_remaining_time":-4}}"#,
        )
        .unwrap();
        assert!(s.temp(Zone::Nozzle).is_none());
        assert_eq!(s.temp(Zone::Bed).unwrap().actual, 55.0);
        let job = s.job.unwrap();
        assert_eq!((job.progress_pct, job.remaining_s), (100.0, None));
        // a seeded fuzz: cutting and damaging a good report at every position is an error or a state
        let good = Machine::idle().report();
        let mut rng = Rng::new(0xba3b_0001);
        for cut in 0..good.len() {
            let _ = parse_report(&good[..cut]);
            let mut bytes = good.clone().into_bytes();
            bytes[cut] = rng.below(128) as u8;
            if let Ok(t) = String::from_utf8(bytes) {
                let _ = parse_report(&t);
            }
        }
    }
}
