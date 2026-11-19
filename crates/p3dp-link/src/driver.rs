//! The trait of a driver: what the rest of the program asks of a printer, whatever its protocol.

use crate::error::LinkError;
use crate::types::{Caps, Command, Family, PrinterState};

/// A printer, as the program sees it. Reading the state is a call that is made again and again (a poll); the push of the printers that send their state on their own
/// is a later step of the roadmap.
pub trait PrinterDriver {
    fn family(&self) -> Family;

    /// What this driver can do.
    fn capabilities(&self) -> Caps;

    /// The state now.
    fn status(&mut self) -> Result<PrinterState, LinkError>;

    /// Does a command. A command the printer cannot do is an error (`Capability`), and a temperature above the limit of the model is an error (`Temp`) that sends nothing.
    fn command(&mut self, command: &Command) -> Result<(), LinkError>;

    /// Sends a file to the printer; with `start` it is started once it is there.
    fn upload(&mut self, name: &str, data: &[u8], start: bool) -> Result<(), LinkError>;

    /// The names of the files that the printer has.
    fn list_files(&mut self) -> Result<Vec<String>, LinkError>;
}

use crate::transport::safe_file_name;
use p3dp_core::machine::Zone;
use p3dp_core::TempLimits;
use p3dp_gcode::check_program;

/// The checks that every driver makes before it sends anything: the capability exists, a temperature is inside the limits of the model (refused, never cut down),
/// a fan speed is between 0 and 100, a file name is safe and a line of raw G-code is allowed, is one line and keeps to the limits.
pub fn guard(
    caps: Caps,
    command: &Command,
    limits: &TempLimits,
    allow_raw_gcode: bool,
) -> Result<(), LinkError> {
    if !caps.contains(command.needs()) {
        return Err(LinkError::Capability(match command {
            Command::Start { .. } => "start a file",
            Command::Pause => "pause",
            Command::Resume => "resume",
            Command::Cancel => "cancel",
            Command::EmergencyStop => "stop in an emergency",
            Command::SetTemp {
                zone: Zone::Nozzle, ..
            } => "set the nozzle temperature",
            Command::SetTemp {
                zone: Zone::Bed, ..
            } => "set the bed temperature",
            Command::SetTemp {
                zone: Zone::Chamber,
                ..
            } => "set the chamber temperature",
            Command::SetFan { .. } => "set the fan",
            Command::Gcode { .. } => "run G-code",
        }));
    }
    match command {
        Command::Start { file } => {
            safe_file_name(file)?;
        }
        Command::SetTemp { zone, target_c } => {
            limits.check(*zone, *target_c)?;
        }
        Command::SetFan { percent } => {
            if !percent.is_finite() || !(0.0..=100.0).contains(percent) {
                return Err(LinkError::Invalid(format!(
                    "the fan speed {percent} is not between 0 and 100"
                )));
            }
        }
        Command::Gcode { line } => {
            if !allow_raw_gcode {
                return Err(LinkError::Disabled("raw G-code"));
            }
            if line.contains(['\n', '\r', '\0']) || line.len() > 256 {
                return Err(LinkError::Invalid(
                    "raw G-code must be one line of at most 256 bytes".to_string(),
                ));
            }
            if let Some(v) = check_program(line, limits).first() {
                return Err(LinkError::Temp(v.error.clone()));
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: TempLimits = TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    };

    fn all() -> Caps {
        Caps(u32::MAX)
    }

    #[test]
    fn a_missing_capability_is_refused() {
        let e = guard(Caps::default(), &Command::Pause, &L, false).unwrap_err();
        assert_eq!(e, LinkError::Capability("pause"));
        assert!(guard(all(), &Command::Pause, &L, false).is_ok());
    }

    #[test]
    fn temperatures_are_refused_not_cut() {
        let hot = Command::SetTemp {
            zone: Zone::Nozzle,
            target_c: 301.0,
        };
        assert!(matches!(
            guard(all(), &hot, &L, false),
            Err(LinkError::Temp(_))
        ));
        let ok = Command::SetTemp {
            zone: Zone::Nozzle,
            target_c: 300.0,
        };
        assert!(guard(all(), &ok, &L, false).is_ok());
        let chamber = Command::SetTemp {
            zone: Zone::Chamber,
            target_c: 30.0,
        };
        assert!(guard(all(), &chamber, &L, false).is_err());
    }

    #[test]
    fn raw_gcode_needs_the_switch_and_keeps_to_the_limits() {
        let g = |l: &str| Command::Gcode {
            line: l.to_string(),
        };
        assert_eq!(
            guard(all(), &g("G28"), &L, false),
            Err(LinkError::Disabled("raw G-code"))
        );
        assert!(guard(all(), &g("G28"), &L, true).is_ok());
        assert!(matches!(
            guard(all(), &g("M104 S999"), &L, true),
            Err(LinkError::Temp(_))
        ));
        assert!(guard(all(), &g("G28\nM104 S999"), &L, true).is_err());
        assert!(guard(all(), &g(&"G".repeat(300)), &L, true).is_err());
    }

    #[test]
    fn files_and_fans_are_checked() {
        assert!(guard(
            all(),
            &Command::Start {
                file: "../x.gcode".into()
            },
            &L,
            false
        )
        .is_err());
        assert!(guard(
            all(),
            &Command::Start {
                file: "x.gcode".into()
            },
            &L,
            false
        )
        .is_ok());
        assert!(guard(all(), &Command::SetFan { percent: 101.0 }, &L, false).is_err());
        assert!(guard(all(), &Command::SetFan { percent: f64::NAN }, &L, false).is_err());
    }
}
