//! The seam where a TLS connection would be plugged in, and a simulator behind it. The program has no TLS of its own (the standard library has none and the build takes no crate from
//! outside), so the link to a printer that speaks TLS (the MQTT and the FTP of a Bambu Lab printer in LAN mode) is written against two small traits: a `TlsConnector` that opens a
//! channel and a `TlsChannel` that carries bytes and tells the certificate the other side showed. What is decided here does not depend on any library:
//!
//! * a channel is handed out only by `connect_pinned`, after the certificate was compared with the fingerprint that was pinned, so no byte of the program can be written to a
//!   printer that did not show that certificate (the type of the channel that the link works with can only be made that way);
//! * a certificate that is not the pinned one, a handshake that fails and a certificate that changes between two connections are refused, and a channel that was opened is closed
//!   again, never left half open;
//! * with no connector (`NoTls`) every secure connection is refused and says why.
//!
//! The real connector would be a small crate outside this workspace (so the offline cache of the continuous integration is not touched) that implements `TlsConnector` over a TLS
//! library, with the check of `pin` as the only way to accept a certificate. It is not written: `sim` holds a printer made in memory that speaks MQTT behind the same traits, so the
//! logic above and the exchange of the link run on a computer. Nothing here has opened a real TLS connection.

use crate::error::LinkError;
use crate::pin::{self, Fingerprint};

/// A channel of bytes to the other side of a TLS connection.
pub trait TlsChannel {
    /// The certificate the other side showed (DER bytes).
    fn peer_certificate_der(&self) -> &[u8];
    /// Writes all the bytes or fails.
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError>;
    /// Reads what has arrived, at least one byte; `Ok(0)` means that the other side closed the channel.
    fn read_some(&mut self, buf: &mut [u8]) -> Result<usize, LinkError>;
    /// Closes the channel (more than once is not an error).
    fn close(&mut self);
}

/// Something that opens TLS channels.
pub trait TlsConnector {
    /// Opens a channel to a host and a port and finishes the handshake. It does not decide whether the certificate is to be trusted: `connect_pinned` does.
    fn connect(&mut self, host: &str, port: u16) -> Result<Box<dyn TlsChannel>, LinkError>;
}

/// The connector of a build that has no TLS: it refuses and says why.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTls;

/// What `NoTls` says.
pub const NO_TLS: &str = "this build has no TLS connector: reach the printer through the EH3D server, or plug a connector in";

impl TlsConnector for NoTls {
    fn connect(&mut self, _host: &str, _port: u16) -> Result<Box<dyn TlsChannel>, LinkError> {
        Err(LinkError::Experimental(NO_TLS))
    }
}

/// A channel whose certificate has been compared with the pinned fingerprint. It can only be made by `connect_pinned`.
pub struct PinnedChannel {
    inner: Box<dyn TlsChannel>,
    seen: Fingerprint,
    closed: bool,
}

impl PinnedChannel {
    /// The fingerprint of the certificate that was shown (it is the pinned one).
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.seen
    }

    pub fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        if self.closed {
            return Err(LinkError::NoConnection);
        }
        self.inner.write_all(data)
    }

    pub fn read_some(&mut self, buf: &mut [u8]) -> Result<usize, LinkError> {
        if self.closed {
            return Err(LinkError::NoConnection);
        }
        self.inner.read_some(buf)
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.inner.close();
        }
    }
}

impl Drop for PinnedChannel {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for PinnedChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedChannel")
            .field("fingerprint", &self.seen)
            .field("closed", &self.closed)
            .finish()
    }
}

/// Opens a channel and lets it through only if the certificate is the pinned one. A channel that is refused is closed before anything is written to it.
pub fn connect_pinned(
    connector: &mut dyn TlsConnector,
    host: &str,
    port: u16,
    pinned: &Fingerprint,
) -> Result<PinnedChannel, LinkError> {
    let mut channel = connector.connect(host, port)?;
    if let Err(e) = pin::verify(channel.peer_certificate_der(), Some(pinned)) {
        channel.close();
        return Err(e);
    }
    let seen = Fingerprint::of_der(channel.peer_certificate_der());
    Ok(PinnedChannel {
        inner: channel,
        seen,
        closed: false,
    })
}

/// A printer made in memory, behind the same traits.
pub mod sim {
    use super::{TlsChannel, TlsConnector};
    use crate::error::LinkError;
    use crate::mqtt::{self, ConnectCode, Packet};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    /// Where the handshake of the simulated printer fails.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Failure {
        /// The connection is refused before any certificate is shown.
        Refused,
        /// The handshake is cut after the certificate was shown: the channel is made and then broken.
        BrokenAfterCertificate,
    }

    /// What the printer saw, for the tests to look at.
    #[derive(Debug, Default, Clone, PartialEq)]
    pub struct Seen {
        pub connections: u32,
        /// Channels that were opened and not closed yet.
        pub open: i32,
        /// Bytes that reached the printer, over all the channels.
        pub bytes: usize,
        /// The controls that reached the printer (`pause`, `resume`, `stop`).
        pub controls: Vec<String>,
        /// The messages of a request for the whole state.
        pub pushalls: u32,
        pub bad_logins: u32,
    }

    /// The state of the printer, as the report tells it.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Machine {
        pub gcode_state: String,
        pub nozzle: f64,
        pub nozzle_target: f64,
        pub bed: f64,
        pub bed_target: f64,
        pub chamber: Option<f64>,
        pub percent: u32,
        pub remaining_min: u32,
        pub file: String,
        pub print_error: u32,
    }

    impl Machine {
        /// An idle printer at room temperature (the figures of a printer that was seen idle).
        pub fn idle() -> Machine {
            Machine {
                gcode_state: "IDLE".to_string(),
                nozzle: 18.0,
                nozzle_target: 0.0,
                bed: 20.0,
                bed_target: 0.0,
                chamber: None,
                percent: 0,
                remaining_min: 0,
                file: String::new(),
                print_error: 0,
            }
        }

        /// The report that the printer sends (the shape of the messages of the printers of the family; only the fields that the link reads).
        pub fn report(&self) -> String {
            let mut s = format!(
                "{{\"print\":{{\"gcode_state\":\"{}\",\"nozzle_temper\":{},\"nozzle_target_temper\":{},\"bed_temper\":{},\"bed_target_temper\":{},\"mc_percent\":{},\"mc_remaining_time\":{},\"subtask_name\":\"{}\",\"print_error\":{}",
                self.gcode_state,
                self.nozzle,
                self.nozzle_target,
                self.bed,
                self.bed_target,
                self.percent,
                self.remaining_min,
                self.file,
                self.print_error
            );
            if let Some(c) = self.chamber {
                s.push_str(&format!(",\"chamber_temper\":{c}"));
            }
            s.push_str("}}");
            s
        }
    }

    struct Core {
        certificate: Vec<u8>,
        serial: String,
        access_code: String,
        machine: Machine,
        failure: Option<Failure>,
        seen: Seen,
        /// A certificate to show on the next connection only (a printer that changes it between two connections).
        next_certificate: Option<Vec<u8>>,
    }

    /// A simulated printer that is also its own connector. Cloning gives another handle on the same printer.
    #[derive(Clone)]
    pub struct SimPrinter(Rc<RefCell<Core>>);

    impl SimPrinter {
        pub fn new(certificate: &[u8], serial: &str, access_code: &str) -> SimPrinter {
            SimPrinter(Rc::new(RefCell::new(Core {
                certificate: certificate.to_vec(),
                serial: serial.to_string(),
                access_code: access_code.to_string(),
                machine: Machine::idle(),
                failure: None,
                seen: Seen::default(),
                next_certificate: None,
            })))
        }

        pub fn set_machine(&self, m: Machine) {
            self.0.borrow_mut().machine = m;
        }

        pub fn machine(&self) -> Machine {
            self.0.borrow().machine.clone()
        }

        pub fn fail(&self, f: Option<Failure>) {
            self.0.borrow_mut().failure = f;
        }

        pub fn show_other_certificate_once(&self, der: &[u8]) {
            self.0.borrow_mut().next_certificate = Some(der.to_vec());
        }

        pub fn seen(&self) -> Seen {
            self.0.borrow().seen.clone()
        }
    }

    impl TlsConnector for SimPrinter {
        fn connect(&mut self, _host: &str, _port: u16) -> Result<Box<dyn TlsChannel>, LinkError> {
            let mut core = self.0.borrow_mut();
            core.seen.connections += 1;
            if core.failure == Some(Failure::Refused) {
                return Err(LinkError::NoConnection);
            }
            let cert = core
                .next_certificate
                .take()
                .unwrap_or_else(|| core.certificate.clone());
            core.seen.open += 1;
            let broken = core.failure == Some(Failure::BrokenAfterCertificate);
            drop(core);
            Ok(Box::new(SimChannel {
                printer: self.0.clone(),
                certificate: cert,
                inbox: Vec::new(),
                outbox: VecDeque::new(),
                logged_in: false,
                subscribed: false,
                closed: false,
                broken,
            }))
        }
    }

    struct SimChannel {
        printer: Rc<RefCell<Core>>,
        certificate: Vec<u8>,
        inbox: Vec<u8>,
        outbox: VecDeque<u8>,
        logged_in: bool,
        subscribed: bool,
        closed: bool,
        broken: bool,
    }

    impl SimChannel {
        fn answer(&mut self, p: Packet) {
            let mut core = self.printer.borrow_mut();
            let mut out: Vec<Packet> = Vec::new();
            match p {
                Packet::Connect {
                    username, password, ..
                } => {
                    let ok = username.as_deref() == Some("bblp")
                        && password.map(|s| s.expose().to_string()).as_deref()
                            == Some(core.access_code.as_str());
                    if !ok {
                        core.seen.bad_logins += 1;
                    }
                    self.logged_in = ok;
                    out.push(Packet::ConnAck {
                        session_present: false,
                        code: if ok {
                            ConnectCode::Accepted
                        } else {
                            ConnectCode::BadCredentials
                        },
                    });
                }
                Packet::Subscribe { id, topic } if self.logged_in => {
                    self.subscribed = topic == format!("device/{}/report", core.serial);
                    out.push(Packet::SubAck {
                        id,
                        granted: if self.subscribed { 0 } else { 0x80 },
                    });
                }
                Packet::Publish { topic, payload, .. }
                    if self.logged_in && topic == format!("device/{}/request", core.serial) =>
                {
                    let text = String::from_utf8_lossy(&payload).to_string();
                    if text.contains("\"pushall\"") {
                        core.seen.pushalls += 1;
                        if self.subscribed {
                            out.push(Packet::Publish {
                                topic: format!("device/{}/report", core.serial),
                                payload: core.machine.report().into_bytes(),
                                retain: false,
                            });
                        }
                    }
                    for control in ["pause", "resume", "stop"] {
                        if text.contains(&format!("\"command\":\"{control}\"")) {
                            core.seen.controls.push(control.to_string());
                            core.machine.gcode_state = match control {
                                "pause" => "PAUSE",
                                "resume" => "RUNNING",
                                _ => "IDLE",
                            }
                            .to_string();
                        }
                    }
                }
                _ => {}
            }
            drop(core);
            for o in out {
                if let Ok(b) = mqtt::encode(&o) {
                    self.outbox.extend(b);
                }
            }
        }
    }

    impl TlsChannel for SimChannel {
        fn peer_certificate_der(&self) -> &[u8] {
            &self.certificate
        }

        fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
            if self.closed || self.broken {
                return Err(LinkError::NoConnection);
            }
            self.printer.borrow_mut().seen.bytes += data.len();
            self.inbox.extend_from_slice(data);
            loop {
                match mqtt::decode(&self.inbox) {
                    Ok(Some((p, used))) => {
                        self.inbox.drain(..used);
                        self.answer(p);
                    }
                    Ok(None) => break,
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }

        fn read_some(&mut self, buf: &mut [u8]) -> Result<usize, LinkError> {
            if self.closed {
                return Ok(0);
            }
            if self.broken {
                return Err(LinkError::NoConnection);
            }
            // the printer answers in the pieces of a stream: a few bytes at a time, so the reader has to join them
            let n = buf.len().min(self.outbox.len()).min(7);
            for slot in buf.iter_mut().take(n) {
                *slot = self.outbox.pop_front().unwrap_or(0);
            }
            if n == 0 {
                return Err(LinkError::Timeout);
            }
            Ok(n)
        }

        fn close(&mut self) {
            if !self.closed {
                self.closed = true;
                self.printer.borrow_mut().seen.open -= 1;
            }
        }
    }

    impl Drop for SimChannel {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sim::{Failure, SimPrinter};
    use super::*;

    const CERT: &[u8] = b"the certificate of the printer, as DER bytes";
    const OTHER: &[u8] = b"the certificate of somebody else on the network";

    fn pinned() -> Fingerprint {
        Fingerprint::of_der(CERT)
    }

    #[test]
    fn only_the_pinned_certificate_gets_a_channel_and_no_byte_reaches_another_printer() {
        let mut printer = SimPrinter::new(CERT, "01P00A123456789", "12345678");
        let ch = connect_pinned(&mut printer, "192.168.0.14", 8883, &pinned()).unwrap();
        assert_eq!(ch.fingerprint(), &pinned());
        drop(ch);
        let mut other = SimPrinter::new(OTHER, "01P00A123456789", "12345678");
        let e = connect_pinned(&mut other, "192.168.0.14", 8883, &pinned()).unwrap_err();
        assert!(e.to_string().contains("not the one pinned"), "{e}");
        let s = other.seen();
        assert_eq!((s.connections, s.bytes, s.open), (1, 0, 0), "{s:?}");
    }

    #[test]
    fn a_handshake_that_fails_leaves_no_channel_open() {
        let mut printer = SimPrinter::new(CERT, "01P00A123456789", "12345678");
        printer.fail(Some(Failure::Refused));
        assert_eq!(
            connect_pinned(&mut printer, "h", 8883, &pinned()).unwrap_err(),
            LinkError::NoConnection
        );
        assert_eq!(printer.seen().open, 0);
        printer.fail(Some(Failure::BrokenAfterCertificate));
        let mut ch = connect_pinned(&mut printer, "h", 8883, &pinned()).unwrap();
        assert_eq!(ch.write_all(b"x"), Err(LinkError::NoConnection));
        drop(ch);
        assert_eq!(printer.seen().open, 0);
    }

    #[test]
    fn a_channel_is_closed_when_it_is_dropped_and_not_used_after_it_is_closed() {
        let mut printer = SimPrinter::new(CERT, "01P00A123456789", "12345678");
        let mut ch = connect_pinned(&mut printer, "h", 8883, &pinned()).unwrap();
        assert_eq!(printer.seen().open, 1);
        ch.close();
        ch.close();
        assert_eq!(printer.seen().open, 0);
        assert_eq!(ch.write_all(b"x"), Err(LinkError::NoConnection));
        assert_eq!(ch.read_some(&mut [0u8; 4]), Err(LinkError::NoConnection));
        let ch2 = connect_pinned(&mut printer, "h", 8883, &pinned()).unwrap();
        assert_eq!(printer.seen().open, 1);
        drop(ch2);
        assert_eq!(printer.seen().open, 0);
    }

    #[test]
    fn a_certificate_that_changes_between_two_connections_is_refused() {
        let mut printer = SimPrinter::new(CERT, "01P00A123456789", "12345678");
        assert!(connect_pinned(&mut printer, "h", 8883, &pinned()).is_ok());
        printer.show_other_certificate_once(OTHER);
        assert!(connect_pinned(&mut printer, "h", 8883, &pinned()).is_err());
        // the next one is the printer again
        assert!(connect_pinned(&mut printer, "h", 8883, &pinned()).is_ok());
        assert_eq!(printer.seen().open, 0);
    }

    #[test]
    fn with_no_connector_every_secure_connection_is_refused_and_says_why() {
        let e = connect_pinned(&mut NoTls, "h", 8883, &pinned()).unwrap_err();
        assert_eq!(e, LinkError::Experimental(NO_TLS));
        assert!(e.to_string().contains("EH3D server"));
    }
}
