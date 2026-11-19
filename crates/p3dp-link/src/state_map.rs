//! The tables that turn what each protocol calls a state into the one list of the program.

use crate::types::JobState;

/// Moonraker/Klipper: the `state` of `print_stats` and the `state` of `webhooks`. A Klipper that has shut down is an emergency, whatever the print says.
pub fn moonraker(print_state: &str, webhooks_state: &str) -> JobState {
    match webhooks_state.to_ascii_lowercase().as_str() {
        "shutdown" => return JobState::Emergency,
        "error" => return JobState::Error,
        "startup" => return JobState::Busy,
        _ => {}
    }
    match print_state.to_ascii_lowercase().as_str() {
        "standby" => JobState::Idle,
        "printing" => JobState::Printing,
        "paused" => JobState::Paused,
        "complete" => JobState::Finished,
        "cancelled" => JobState::Cancelled,
        "error" => JobState::Error,
        _ => JobState::Busy,
    }
}

/// The flags of the state of OctoPrint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OctoFlags {
    pub operational: bool,
    pub printing: bool,
    pub paused: bool,
    pub pausing: bool,
    pub cancelling: bool,
    pub error: bool,
    pub closed_or_error: bool,
}

/// OctoPrint: the flags, the most serious first.
pub fn octoprint(f: &OctoFlags) -> JobState {
    if f.error {
        JobState::Error
    } else if f.closed_or_error && !f.operational {
        JobState::Offline
    } else if f.cancelling {
        JobState::Cancelling
    } else if f.pausing {
        JobState::Pausing
    } else if f.paused {
        JobState::Paused
    } else if f.printing {
        JobState::Printing
    } else if f.operational {
        JobState::Idle
    } else {
        JobState::Offline
    }
}

/// PrusaLink.
pub fn prusalink(state: &str) -> JobState {
    match state.to_ascii_uppercase().as_str() {
        "IDLE" | "READY" => JobState::Idle,
        "BUSY" => JobState::Busy,
        "PRINTING" => JobState::Printing,
        "PAUSED" => JobState::Paused,
        "FINISHED" => JobState::Finished,
        "STOPPED" => JobState::Cancelled,
        "ERROR" | "ATTENTION" => JobState::Error,
        _ => JobState::Busy,
    }
}

/// RepRapFirmware (Duet): the `status` of the object model.
pub fn rrf(status: &str) -> JobState {
    match status.to_ascii_lowercase().as_str() {
        "idle" => JobState::Idle,
        "processing" | "resuming" => JobState::Printing,
        "paused" => JobState::Paused,
        "pausing" => JobState::Pausing,
        "cancelling" => JobState::Cancelling,
        "halted" => JobState::Emergency,
        "off" | "disconnected" => JobState::Offline,
        "updating" => JobState::Updating,
        "starting" => JobState::Preparing,
        _ => JobState::Busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moonraker_table() {
        for (p, w, want) in [
            ("standby", "ready", JobState::Idle),
            ("printing", "ready", JobState::Printing),
            ("paused", "ready", JobState::Paused),
            ("complete", "ready", JobState::Finished),
            ("cancelled", "ready", JobState::Cancelled),
            ("error", "ready", JobState::Error),
            ("printing", "shutdown", JobState::Emergency),
            ("standby", "error", JobState::Error),
            ("standby", "startup", JobState::Busy),
            ("surprise", "ready", JobState::Busy),
            ("PRINTING", "READY", JobState::Printing),
        ] {
            assert_eq!(moonraker(p, w), want, "{p}/{w}");
        }
    }

    #[test]
    fn octoprint_table() {
        let f = OctoFlags::default();
        assert_eq!(octoprint(&f), JobState::Offline);
        assert_eq!(
            octoprint(&OctoFlags {
                operational: true,
                ..f
            }),
            JobState::Idle
        );
        assert_eq!(
            octoprint(&OctoFlags {
                operational: true,
                printing: true,
                ..f
            }),
            JobState::Printing
        );
        assert_eq!(
            octoprint(&OctoFlags {
                operational: true,
                printing: true,
                paused: true,
                ..f
            }),
            JobState::Paused
        );
        assert_eq!(
            octoprint(&OctoFlags {
                operational: true,
                printing: true,
                pausing: true,
                ..f
            }),
            JobState::Pausing
        );
        assert_eq!(
            octoprint(&OctoFlags {
                operational: true,
                printing: true,
                cancelling: true,
                ..f
            }),
            JobState::Cancelling
        );
        assert_eq!(
            octoprint(&OctoFlags {
                error: true,
                operational: true,
                ..f
            }),
            JobState::Error
        );
        assert_eq!(
            octoprint(&OctoFlags {
                closed_or_error: true,
                ..f
            }),
            JobState::Offline
        );
    }

    #[test]
    fn prusalink_and_rrf_tables() {
        for (s, want) in [
            ("IDLE", JobState::Idle),
            ("busy", JobState::Busy),
            ("PRINTING", JobState::Printing),
            ("PAUSED", JobState::Paused),
            ("FINISHED", JobState::Finished),
            ("STOPPED", JobState::Cancelled),
            ("ERROR", JobState::Error),
            ("ATTENTION", JobState::Error),
            ("???", JobState::Busy),
        ] {
            assert_eq!(prusalink(s), want, "{s}");
        }
        for (s, want) in [
            ("idle", JobState::Idle),
            ("processing", JobState::Printing),
            ("pausing", JobState::Pausing),
            ("paused", JobState::Paused),
            ("resuming", JobState::Printing),
            ("cancelling", JobState::Cancelling),
            ("halted", JobState::Emergency),
            ("off", JobState::Offline),
            ("updating", JobState::Updating),
            ("changingTool", JobState::Busy),
        ] {
            assert_eq!(rrf(s), want, "{s}");
        }
    }
}
