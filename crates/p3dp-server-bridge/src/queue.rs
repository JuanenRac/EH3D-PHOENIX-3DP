//! The queue as a person reads it, and the rules for putting a job in it. The jobs of the server come as one list; here they are grouped by machine (what runs, what waits in its order,
//! what is over), and a job is only put in when it does not repeat one that already waits and the machine's line is not too long. Both limits are set in the settings file of the program
//! (`queue_refuse_duplicates`, `queue_max_waiting`); none of this starts anything, and the server keeps its own rules.

use crate::client::{BridgeError, JobInfo, ServerClient};
use p3dp_core::TempLimits;

/// The rules for putting a job in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuePolicy {
    /// Refuse a job whose name is that of one that is still waiting, running or held on the same machine.
    pub refuse_duplicates: bool,
    /// The most jobs that may wait on one machine.
    pub max_waiting: usize,
}

impl Default for QueuePolicy {
    fn default() -> QueuePolicy {
        QueuePolicy {
            refuse_duplicates: true,
            max_waiting: 20,
        }
    }
}

/// The jobs of one machine.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineQueue {
    pub machine: String,
    /// The jobs that are running or held (a machine runs one at a time; the list is what the server says).
    pub active: Vec<JobInfo>,
    /// The jobs that wait for a person to start them, in the order of their place in the queue.
    pub waiting: Vec<JobInfo>,
    /// The jobs that are over, in the order the server gave them (newest first).
    pub finished: Vec<JobInfo>,
}

/// Groups the jobs of the server by machine; the machines come in the order of their names. A job in a state this program does not know counts as active, so it is never hidden.
pub fn board(jobs: &[JobInfo]) -> Vec<MachineQueue> {
    let mut out: Vec<MachineQueue> = Vec::new();
    for j in jobs {
        let at = match out.iter().position(|m| m.machine == j.machine) {
            Some(i) => i,
            None => {
                out.push(MachineQueue {
                    machine: j.machine.clone(),
                    active: Vec::new(),
                    waiting: Vec::new(),
                    finished: Vec::new(),
                });
                out.len() - 1
            }
        };
        let m = &mut out[at];
        if j.finished() {
            m.finished.push(j.clone());
        } else if j.waiting() {
            m.waiting.push(j.clone());
        } else {
            m.active.push(j.clone());
        }
    }
    for m in &mut out {
        // a job with no place goes last, and two with the same place keep the order of their ids
        m.waiting.sort_by(|a, b| {
            a.position
                .unwrap_or(u64::MAX)
                .cmp(&b.position.unwrap_or(u64::MAX))
                .then(a.id.cmp(&b.id))
        });
    }
    out.sort_by(|a, b| a.machine.cmp(&b.machine));
    out
}

/// Whether a job may be put in the queue, given the jobs the server lists: the same name twice on a machine, or a line that is full, is refused with the reason and nothing is sent.
pub fn check_enqueue(
    existing: &[JobInfo],
    machine: &str,
    name: &str,
    policy: &QueuePolicy,
) -> Result<(), BridgeError> {
    let here: Vec<&JobInfo> = existing
        .iter()
        .filter(|j| j.machine == machine && !j.finished())
        .collect();
    if policy.refuse_duplicates {
        let want = name.trim().to_lowercase();
        if let Some(d) = here.iter().find(|j| j.name.trim().to_lowercase() == want) {
            return Err(BridgeError::Invalid(format!(
                "a job named {:?} is already {} on {machine} (job {}); cancel it or give the new one another name",
                d.name,
                if d.waiting() { "waiting" } else { "in progress" },
                d.id
            )));
        }
    }
    let waiting = here.iter().filter(|j| j.waiting()).count();
    if waiting >= policy.max_waiting {
        return Err(BridgeError::Invalid(format!(
            "{machine} already has {waiting} jobs waiting, the most the settings allow ({})",
            policy.max_waiting
        )));
    }
    Ok(())
}

impl ServerClient {
    /// Puts a program in the queue like [`ServerClient::enqueue`], after looking at the queue of the machine: a repeated name or a full line is refused before anything is sent.
    pub fn enqueue_checked(
        &mut self,
        machine: &str,
        name: &str,
        gcode: &str,
        limits: &TempLimits,
        policy: &QueuePolicy,
    ) -> Result<JobInfo, BridgeError> {
        let existing = self.queue(Some(machine))?;
        check_enqueue(&existing, machine, name, policy)?;
        self.enqueue(machine, name, gcode, limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, machine: &str, name: &str, state: &str, position: Option<u64>) -> JobInfo {
        JobInfo {
            id: id.to_string(),
            machine: machine.to_string(),
            name: name.to_string(),
            state: state.to_string(),
            percent: 0,
            message: String::new(),
            position,
        }
    }

    #[test]
    fn the_board_groups_the_jobs_by_machine_and_puts_the_waiting_ones_in_their_order() {
        let jobs = vec![
            job("w-5", "mk4", "late", "queued", Some(2)),
            job("w-1", "a1", "done one", "done", None),
            job("w-4", "mk4", "early", "queued", Some(0)),
            job("w-3", "mk4", "running", "running", None),
            job("w-6", "mk4", "none", "queued", None),
            job("w-7", "mk4", "mid", "queued", Some(1)),
            job("w-2", "a1", "held", "paused", None),
            job("w-8", "a1", "odd", "mystery", None),
            job("w-9", "a1", "failed", "failed", None),
        ];
        let b = board(&jobs);
        assert_eq!(
            b.iter().map(|m| m.machine.as_str()).collect::<Vec<_>>(),
            vec!["a1", "mk4"]
        );
        let a1 = &b[0];
        assert_eq!(
            a1.active.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(),
            vec!["w-2", "w-8"],
            "a state nobody knows is active, not hidden"
        );
        assert_eq!(
            a1.finished
                .iter()
                .map(|j| j.id.as_str())
                .collect::<Vec<_>>(),
            vec!["w-1", "w-9"]
        );
        assert!(a1.waiting.is_empty());
        let mk4 = &b[1];
        assert_eq!(
            mk4.waiting
                .iter()
                .map(|j| j.id.as_str())
                .collect::<Vec<_>>(),
            vec!["w-4", "w-7", "w-5", "w-6"],
            "by place, and the job with no place last"
        );
        assert_eq!(mk4.active.len(), 1);
        assert!(board(&[]).is_empty());
    }

    #[test]
    fn a_repeated_name_or_a_full_line_is_refused_with_the_reason() {
        let jobs = vec![
            job("w-1", "mk4", "Benchy", "queued", Some(0)),
            job("w-2", "mk4", "cube", "running", None),
            job("w-3", "mk4", "old", "done", None),
            job("w-4", "a1", "other", "queued", Some(0)),
        ];
        let p = QueuePolicy::default();
        assert!(check_enqueue(&jobs, "mk4", "fresh", &p).is_ok());
        let e = check_enqueue(&jobs, "mk4", "  benchy ", &p)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("already waiting") && e.contains("w-1") && e.contains("mk4"),
            "{e}"
        );
        let e = check_enqueue(&jobs, "mk4", "CUBE", &p)
            .unwrap_err()
            .to_string();
        assert!(e.contains("in progress"), "{e}");
        // a finished job of the same name, or the same name on another machine, is no repeat
        assert!(check_enqueue(&jobs, "mk4", "old", &p).is_ok());
        assert!(check_enqueue(&jobs, "a1", "Benchy", &p).is_ok());
        // with the rule off, the repeat goes in
        let lax = QueuePolicy {
            refuse_duplicates: false,
            ..p
        };
        assert!(check_enqueue(&jobs, "mk4", "Benchy", &lax).is_ok());
        // a full line
        let one = QueuePolicy {
            max_waiting: 1,
            ..lax
        };
        let e = check_enqueue(&jobs, "mk4", "new", &one)
            .unwrap_err()
            .to_string();
        assert!(e.contains("1 jobs waiting") && e.contains("(1)"), "{e}");
        assert!(check_enqueue(
            &jobs,
            "a1",
            "new",
            &QueuePolicy {
                max_waiting: 2,
                ..lax
            }
        )
        .is_ok());
        let zero = QueuePolicy {
            max_waiting: 0,
            ..lax
        };
        assert!(
            check_enqueue(&[], "mk4", "x", &zero).is_err(),
            "a limit of zero closes the line"
        );
    }
}
