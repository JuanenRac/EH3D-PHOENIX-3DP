//! The normalized state and the normalized commands: every protocol is mapped onto these, so the rest of the program knows one language.

use p3dp_core::machine::Zone;
use std::fmt;

/// The family of protocols of a printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Moonraker,
    OctoPrint,
    PrusaLink,
    Duet,
    Bambu,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Moonraker => "moonraker",
            Family::OctoPrint => "octoprint",
            Family::PrusaLink => "prusalink",
            Family::Duet => "duet",
            Family::Bambu => "bambu",
        }
    }
}

/// What a printer is doing, in one list for every protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    Offline,
    Idle,
    Preparing,
    Printing,
    Pausing,
    Paused,
    Finishing,
    Finished,
    Failed,
    Cancelling,
    Cancelled,
    Error,
    Emergency,
    Updating,
    Busy,
}

impl JobState {
    pub const ALL: [JobState; 15] = [
        JobState::Offline,
        JobState::Idle,
        JobState::Preparing,
        JobState::Printing,
        JobState::Pausing,
        JobState::Paused,
        JobState::Finishing,
        JobState::Finished,
        JobState::Failed,
        JobState::Cancelling,
        JobState::Cancelled,
        JobState::Error,
        JobState::Emergency,
        JobState::Updating,
        JobState::Busy,
    ];

    /// The key of the state (the program translates the key, the raw text of the printer is kept apart).
    pub fn key(self) -> &'static str {
        match self {
            JobState::Offline => "offline",
            JobState::Idle => "idle",
            JobState::Preparing => "preparing",
            JobState::Printing => "printing",
            JobState::Pausing => "pausing",
            JobState::Paused => "paused",
            JobState::Finishing => "finishing",
            JobState::Finished => "finished",
            JobState::Failed => "failed",
            JobState::Cancelling => "cancelling",
            JobState::Cancelled => "cancelled",
            JobState::Error => "error",
            JobState::Emergency => "emergency",
            JobState::Updating => "updating",
            JobState::Busy => "busy",
        }
    }

    /// Whether a job is under way (the printer should not be given another).
    pub fn is_active(self) -> bool {
        matches!(
            self,
            JobState::Preparing
                | JobState::Printing
                | JobState::Pausing
                | JobState::Paused
                | JobState::Finishing
                | JobState::Cancelling
        )
    }

    /// Whether the printer needs a person (an error or an emergency stop).
    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            JobState::Error | JobState::Emergency | JobState::Failed
        )
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// The temperature of one heater.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Temperature {
    pub zone: Zone,
    pub actual: f64,
    pub target: f64,
}

/// The job that is under way.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JobInfo {
    pub file: String,
    /// 0 to 100.
    pub progress_pct: f64,
    pub elapsed_s: Option<u64>,
    pub remaining_s: Option<u64>,
}

/// The state of a printer as every driver reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct PrinterState {
    pub family: Family,
    pub state: JobState,
    /// What the printer said, as it said it.
    pub raw_state: String,
    pub temperatures: Vec<Temperature>,
    pub job: Option<JobInfo>,
    pub alerts: Vec<String>,
}

impl PrinterState {
    /// The temperature of a heater, if the printer reported it.
    pub fn temp(&self, zone: Zone) -> Option<&Temperature> {
        self.temperatures.iter().find(|t| t.zone == zone)
    }

    /// A state for a printer that could not be reached.
    pub fn offline(family: Family) -> PrinterState {
        PrinterState {
            family,
            state: JobState::Offline,
            raw_state: String::new(),
            temperatures: Vec::new(),
            job: None,
            alerts: Vec::new(),
        }
    }
}

/// What a driver can do, as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps(pub u32);

impl Caps {
    pub const PAUSE: u32 = 1;
    pub const RESUME: u32 = 1 << 1;
    pub const CANCEL: u32 = 1 << 2;
    pub const EMERGENCY_STOP: u32 = 1 << 3;
    pub const GCODE: u32 = 1 << 4;
    pub const UPLOAD: u32 = 1 << 5;
    pub const FILE_LIST: u32 = 1 << 6;
    pub const SET_NOZZLE: u32 = 1 << 7;
    pub const SET_BED: u32 = 1 << 8;
    pub const SET_CHAMBER: u32 = 1 << 9;
    pub const START: u32 = 1 << 10;
    pub const FAN: u32 = 1 << 11;

    pub fn contains(self, bit: u32) -> bool {
        self.0 & bit == bit
    }

    pub fn with(self, bit: u32) -> Caps {
        Caps(self.0 | bit)
    }
}

/// A command for a printer, the same for every protocol. A driver that cannot do one answers `Capability`; it never does it by guess.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Starts a file that is already on the printer.
    Start {
        file: String,
    },
    Pause,
    Resume,
    Cancel,
    EmergencyStop,
    SetTemp {
        zone: Zone,
        target_c: f64,
    },
    /// The part fan, 0 to 100.
    SetFan {
        percent: f64,
    },
    /// One line of G-code (it needs the switch of raw G-code on, and it is checked against the temperature limits).
    Gcode {
        line: String,
    },
}

impl Command {
    /// The capability bit the command needs.
    pub fn needs(&self) -> u32 {
        match self {
            Command::Start { .. } => Caps::START,
            Command::Pause => Caps::PAUSE,
            Command::Resume => Caps::RESUME,
            Command::Cancel => Caps::CANCEL,
            Command::EmergencyStop => Caps::EMERGENCY_STOP,
            Command::SetTemp {
                zone: Zone::Nozzle, ..
            } => Caps::SET_NOZZLE,
            Command::SetTemp {
                zone: Zone::Bed, ..
            } => Caps::SET_BED,
            Command::SetTemp {
                zone: Zone::Chamber,
                ..
            } => Caps::SET_CHAMBER,
            Command::SetFan { .. } => Caps::FAN,
            Command::Gcode { .. } => Caps::GCODE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_keys_are_unique_and_complete() {
        let mut keys: Vec<&str> = JobState::ALL.iter().map(|s| s.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 15);
    }

    #[test]
    fn active_and_attention() {
        assert!(JobState::Printing.is_active() && JobState::Paused.is_active());
        assert!(!JobState::Idle.is_active() && !JobState::Finished.is_active());
        assert!(JobState::Emergency.needs_attention() && !JobState::Printing.needs_attention());
    }

    #[test]
    fn caps_bits() {
        let c = Caps::default().with(Caps::PAUSE).with(Caps::CANCEL);
        assert!(c.contains(Caps::PAUSE) && c.contains(Caps::CANCEL));
        assert!(!c.contains(Caps::UPLOAD));
        assert_eq!(Command::Pause.needs(), Caps::PAUSE);
        assert_eq!(
            Command::SetTemp {
                zone: Zone::Bed,
                target_c: 1.0
            }
            .needs(),
            Caps::SET_BED
        );
    }

    #[test]
    fn the_offline_state_has_nothing() {
        let s = PrinterState::offline(Family::Moonraker);
        assert_eq!(s.state, JobState::Offline);
        assert!(s.temp(Zone::Nozzle).is_none());
        assert_eq!(Family::Bambu.name(), "bambu");
    }
}
