//! Bambu Lab over the local network: EXPERIMENTAL and not connected to anything. This file holds the part of the design that is plain data and can be tested without a
//! printer: the topics, the shape of the messages, the table of states and the checks of the serial number. The driver itself answers `Experimental` to everything.
//! The cloud is not part of it. Nothing here has been tried on a Bambu printer.

use crate::driver::PrinterDriver;
use crate::error::LinkError;
use crate::types::{Caps, Command, Family, JobState, PrinterState};
use p3dp_core::json::Value;

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
