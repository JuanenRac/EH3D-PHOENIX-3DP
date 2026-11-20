//! What the Device tab shows and does about the printers of the EH3D server, with no window: the machines, their heaters, the layer, the queue, and the few things a person can
//! do. The rules the tab keeps:
//!
//! * a job never starts by itself: `ask_start` only asks, and `confirm_start` is the one place that makes the word of a person (`Confirmation::by_person`), and only for the job that
//!   was asked about, once;
//! * the emergency stop has no question: it is one call that goes at once to the machine chosen;
//! * a program is put in the queue and waits; and a failed call is kept to be shown in the language of the person, never thrown away.

use crate::i18n::{fill, Lang, Msg};
use p3dp_core::TempLimits;
use p3dp_server_bridge::{
    stricter, BridgeError, Confirmation, HeaterInfo, JobInfo, LayerInfo, PrinterInfo, QueuePolicy,
    ServerClient,
};

/// The state of the Device tab.
#[derive(Debug, Default)]
pub struct DevicePanel {
    pub machines: Vec<PrinterInfo>,
    pub selected: Option<String>,
    pub queue: Vec<JobInfo>,
    pub heaters: Vec<HeaterInfo>,
    pub layer: Option<LayerInfo>,
    /// The job a person was asked about, waiting for the answer.
    asking: Option<String>,
    /// The last call that failed.
    pub error: Option<BridgeError>,
    /// What the last call that went well did, for the status line.
    pub done: Option<Done>,
    /// The rules for putting a job in the queue (a repeated name, a full line).
    pub policy: QueuePolicy,
}

/// A thing that was done.
#[derive(Debug, Clone, PartialEq)]
pub enum Done {
    Started(String),
    Paused(String),
    Resumed(String),
    Cancelled(String),
    Queued(String),
    /// The emergency stop was sent to this machine; `true` when the server reached the printer.
    Stopped(String, bool),
}

impl DevicePanel {
    pub fn new() -> DevicePanel {
        DevicePanel::default()
    }

    /// A panel that keeps these rules when it puts a job in the queue.
    pub fn with_policy(policy: QueuePolicy) -> DevicePanel {
        DevicePanel {
            policy,
            ..DevicePanel::default()
        }
    }

    fn fail(&mut self, e: BridgeError) {
        self.done = None;
        self.error = Some(e);
    }

    /// Forgets what was read (when the server is let go).
    pub fn clear(&mut self) {
        *self = DevicePanel::with_policy(self.policy);
    }

    /// The machine chosen, as the server lists it.
    pub fn machine(&self) -> Option<&PrinterInfo> {
        let id = self.selected.as_deref()?;
        self.machines.iter().find(|m| m.id == id)
    }

    /// Reads the machines, and for the one chosen its queue, heaters and layer. Without a client everything is cleared. The first failure is kept and the rest is left as it was read.
    pub fn refresh(&mut self, client: Option<&mut ServerClient>) {
        let Some(c) = client else {
            self.clear();
            return;
        };
        self.error = None;
        match c.printers() {
            Ok(list) => self.machines = list,
            Err(e) => return self.fail(e),
        }
        let keep = self
            .selected
            .as_ref()
            .is_some_and(|s| self.machines.iter().any(|m| &m.id == s));
        if !keep {
            self.selected = self.machines.first().map(|m| m.id.clone());
        }
        let Some(id) = self.selected.clone() else {
            self.queue.clear();
            self.heaters.clear();
            self.layer = None;
            return;
        };
        let online = self.machine().is_some_and(|m| m.online);
        match c.queue(Some(&id)) {
            Ok(q) => self.queue = q,
            Err(e) => return self.fail(e),
        }
        if self
            .asking
            .as_ref()
            .is_some_and(|a| !self.queue.iter().any(|j| &j.id == a && j.waiting()))
        {
            self.asking = None;
        }
        self.heaters.clear();
        self.layer = None;
        if online {
            match c.temperatures(&id) {
                Ok(h) => self.heaters = h,
                Err(e) => return self.fail(e),
            }
            // a printer that is not printing has no layer: that is not a failure
            self.layer = c.layer(&id).ok();
        }
    }

    /// Chooses a machine (one the server listed). Whatever was asked about is dropped.
    pub fn select(&mut self, id: &str) -> bool {
        if self.machines.iter().any(|m| m.id == id) {
            self.selected = Some(id.to_string());
            self.asking = None;
            true
        } else {
            false
        }
    }

    /// Asks whether to start a job: only a job that waits in the queue of the machine chosen, on a machine that is not latched. Nothing is sent.
    pub fn ask_start(&mut self, job: &str) -> bool {
        let waits = self.queue.iter().any(|j| j.id == job && j.waiting());
        let free = self.machine().is_some_and(|m| !m.latched);
        if waits && free {
            self.asking = Some(job.to_string());
            true
        } else {
            false
        }
    }

    /// The job being asked about.
    pub fn asking(&self) -> Option<&JobInfo> {
        let id = self.asking.as_deref()?;
        self.queue.iter().find(|j| j.id == id)
    }

    /// The person said no.
    pub fn cancel_ask(&mut self) {
        self.asking = None;
    }

    /// The question in a language, with the job and the machine in it.
    pub fn question(&self, lang: Lang) -> Option<String> {
        let job = self.asking()?;
        let machine = self
            .machine()
            .map_or(job.machine.as_str(), |m| m.name.as_str());
        Some(fill(Msg::JobConfirmStart.tr(lang), &[&job.name, machine]))
    }

    /// The person said yes: the job that was asked about starts. With no question open nothing is sent.
    pub fn confirm_start(&mut self, client: Option<&mut ServerClient>) -> Option<JobInfo> {
        let job = self.asking.take()?;
        let c = client?;
        match c.start(&job, Confirmation::by_person()) {
            Ok(j) => {
                self.error = None;
                self.done = Some(Done::Started(j.name.clone()));
                self.refresh(Some(c));
                Some(j)
            }
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    fn control(
        &mut self,
        client: Option<&mut ServerClient>,
        job: &str,
        call: impl FnOnce(&mut ServerClient, &str) -> Result<JobInfo, BridgeError>,
        done: impl FnOnce(String) -> Done,
    ) -> Option<JobInfo> {
        let c = client?;
        match call(c, job) {
            Ok(j) => {
                self.error = None;
                self.done = Some(done(j.name.clone()));
                self.refresh(Some(c));
                Some(j)
            }
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    pub fn pause(&mut self, client: Option<&mut ServerClient>, job: &str) -> Option<JobInfo> {
        self.control(client, job, |c, j| c.pause(j), Done::Paused)
    }

    pub fn resume(&mut self, client: Option<&mut ServerClient>, job: &str) -> Option<JobInfo> {
        self.control(client, job, |c, j| c.resume(j), Done::Resumed)
    }

    pub fn cancel(&mut self, client: Option<&mut ServerClient>, job: &str) -> Option<JobInfo> {
        self.control(client, job, |c, j| c.cancel(j), Done::Cancelled)
    }

    /// The emergency stop of the machine chosen. It has no question in front of it. The answer says whether the server reached the printer.
    pub fn emergency_stop(&mut self, client: Option<&mut ServerClient>) -> Option<bool> {
        let id = self.selected.clone()?;
        let c = client?;
        match c.emergency_stop(&id) {
            Ok(reached) => {
                self.asking = None;
                self.error = None;
                self.done = Some(Done::Stopped(id, reached));
                self.refresh(Some(c));
                Some(reached)
            }
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    /// Puts a program in the queue of the machine chosen. The temperatures are held to the stricter of the limits of the profile and the limits the server keeps for the machine.
    /// The job waits: it starts only through `ask_start` and `confirm_start`.
    pub fn enqueue(
        &mut self,
        client: Option<&mut ServerClient>,
        name: &str,
        gcode: &str,
        profile_limits: &TempLimits,
    ) -> Option<JobInfo> {
        let id = self.selected.clone()?;
        let server_limits = self.machine().and_then(|m| m.limits);
        let c = client?;
        let limits = match server_limits {
            Some(s) => stricter(profile_limits, &s),
            None => *profile_limits,
        };
        match c.enqueue_checked(&id, name, gcode, &limits, &self.policy) {
            Ok(j) => {
                self.error = None;
                self.done = Some(Done::Queued(j.name.clone()));
                self.refresh(Some(c));
                Some(j)
            }
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    /// The text of the last failure in a language of the suite.
    pub fn error_text(&self, lang: Lang) -> Option<String> {
        self.error
            .as_ref()
            .map(|e| e.text(crate::server_ui::client_lang(lang)))
    }

    /// The lines under the list of machines: the state of the one chosen, its heaters and the layer.
    pub fn lines(&self, lang: Lang) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(m) = self.machine() {
            let state = if m.latched {
                "latched".to_string()
            } else if !m.online {
                "offline".to_string()
            } else {
                m.state.clone()
            };
            out.push(format!("{} ({}): {state}", m.name, m.model));
        }
        for h in &self.heaters {
            out.push(format!(
                "{}: {:.0} / {:.0} C (max {:.0})",
                h.name, h.current_c, h.target_c, h.limit_c
            ));
        }
        if let Some(l) = &self.layer {
            out.push(format!(
                "{}/{} - {:.1} mm - {}",
                l.layer,
                l.layers,
                l.z_mm,
                fill(Msg::Progress.tr(lang), &[&format!("{:.0}", l.progress_pct)])
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};
    use p3dp_server_bridge::Settings;

    const SAFE: &str = "G28\nM140 S60\nM104 S210\nG1 X10 Y10 E1 F1200\nM104 S0\nM140 S0\n";

    fn client(token: &str) -> (ServerClient, SimPrinterServer) {
        let sim = SimPrinterServer::new();
        let mut s = Settings::default();
        s.set("server_address", "192.168.0.180:18200").unwrap();
        s.set("server_token", token).unwrap();
        let c = ServerClient::new(s, Box::new(sim.clone()))
            .unwrap()
            .with_sleeper(Box::new(sim.auth.sleeper()));
        (c, sim)
    }

    fn limits() -> TempLimits {
        TempLimits {
            nozzle_c: 300.0,
            bed_c: 100.0,
            chamber_c: None,
        }
    }

    #[test]
    fn without_a_server_there_is_nothing_and_nothing_is_sent() {
        let mut d = DevicePanel::new();
        d.refresh(None);
        assert!(d.machines.is_empty() && d.selected.is_none() && d.queue.is_empty());
        assert!(!d.ask_start("job-1"));
        assert_eq!(d.confirm_start(None), None);
        assert_eq!(d.emergency_stop(None), None);
        assert_eq!(d.enqueue(None, "x", SAFE, &limits()), None);
        assert!(d.question(Lang::En).is_none());
        assert!(d.lines(Lang::En).is_empty());
    }

    #[test]
    fn a_repeated_job_and_a_full_line_are_refused_by_the_panel_and_the_rules_survive_a_clear() {
        let (mut c, sim) = client(TOKEN);
        let mut d = DevicePanel::with_policy(QueuePolicy {
            refuse_duplicates: true,
            max_waiting: 2,
        });
        d.refresh(Some(&mut c));
        assert!(d.enqueue(Some(&mut c), "cube", SAFE, &limits()).is_some());
        let posted = sim.with_state(|s| s.posted.len());
        assert!(d.enqueue(Some(&mut c), "Cube", SAFE, &limits()).is_none());
        let e = d.error_text(Lang::En).unwrap();
        assert!(e.contains("already waiting"), "{e}");
        assert_eq!(
            sim.with_state(|s| s.posted.len()),
            posted,
            "nothing was sent"
        );
        assert!(d.enqueue(Some(&mut c), "benchy", SAFE, &limits()).is_some());
        assert!(d.enqueue(Some(&mut c), "third", SAFE, &limits()).is_none());
        assert!(d.error_text(Lang::En).unwrap().contains("2 jobs waiting"));
        d.clear();
        assert_eq!(
            d.policy.max_waiting, 2,
            "clearing what was read keeps the rules"
        );
        assert!(d.queue.is_empty());
    }

    #[test]
    fn the_machines_the_queue_and_the_heaters_are_read_and_the_first_machine_is_chosen() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        assert!(d.error.is_none(), "{:?}", d.error);
        assert_eq!(d.machines.len(), 2);
        assert_eq!(d.selected.as_deref(), Some("p1s-1"));
        assert!(d.machine().unwrap().online);
        let lines = d.lines(Lang::En).join("\n");
        assert!(lines.contains("Bambu P1S"), "{lines}");
        // an offline machine has no heaters to read and that is not an error
        assert!(d.select("mk4-1"));
        d.refresh(Some(&mut c));
        assert!(d.error.is_none() && d.heaters.is_empty() && d.layer.is_none());
        assert!(d.lines(Lang::En)[0].contains("offline"));
        assert!(
            !d.select("router-1"),
            "a machine of another plugin is not listed"
        );
        assert_eq!(d.selected.as_deref(), Some("mk4-1"));
    }

    #[test]
    fn a_job_starts_only_after_the_question_and_only_the_one_that_was_asked() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        let a = d.enqueue(Some(&mut c), "cube", SAFE, &limits()).unwrap();
        let b = d.enqueue(Some(&mut c), "benchy", SAFE, &limits()).unwrap();
        assert!(a.waiting() && b.waiting());
        assert!(matches!(d.done, Some(Done::Queued(_))));
        // nothing was asked: confirming sends nothing, and nothing runs
        assert_eq!(d.confirm_start(Some(&mut c)), None);
        d.refresh(Some(&mut c));
        assert!(d.queue.iter().all(JobInfo::waiting), "{:?}", d.queue);
        // the question names the job and the machine
        assert!(d.ask_start(&b.id));
        let q = d.question(Lang::En).unwrap();
        assert!(
            q.contains("benchy") && q.contains("Bambu P1S") && q.contains("Nothing starts without"),
            "{q}"
        );
        assert!(d.question(Lang::Pt).unwrap().contains("Iniciar"));
        // saying no drops it
        d.cancel_ask();
        assert_eq!(d.confirm_start(Some(&mut c)), None);
        // saying yes starts that job and not the other
        assert!(d.ask_start(&b.id));
        let started = d.confirm_start(Some(&mut c)).unwrap();
        assert_eq!(started.id, b.id);
        assert!(matches!(d.done, Some(Done::Started(ref n)) if n == "benchy"));
        let state =
            |d: &DevicePanel, id: &str| d.queue.iter().find(|j| j.id == id).unwrap().state.clone();
        assert_ne!(state(&d, &b.id), "queued");
        assert_eq!(state(&d, &a.id), "queued");
        // the answer is used once: the second confirmation sends nothing
        assert_eq!(d.confirm_start(Some(&mut c)), None);
        // a job that is not waiting, or does not exist, is not asked about
        assert!(!d.ask_start(&b.id));
        assert!(!d.ask_start("ghost"));
    }

    #[test]
    fn the_question_is_dropped_when_the_machine_is_changed_or_the_job_is_gone() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        let a = d.enqueue(Some(&mut c), "cube", SAFE, &limits()).unwrap();
        assert!(d.ask_start(&a.id));
        assert!(d.select("mk4-1"));
        assert!(d.asking().is_none());
        assert!(d.select("p1s-1"));
        d.refresh(Some(&mut c));
        assert!(d.ask_start(&a.id));
        // somebody else cancels the job while the question is open
        c.cancel(&a.id).unwrap();
        d.refresh(Some(&mut c));
        assert!(d.asking().is_none());
        assert_eq!(d.confirm_start(Some(&mut c)), None);
    }

    #[test]
    fn pause_resume_and_cancel_go_through_and_are_told() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        let a = d.enqueue(Some(&mut c), "cube", SAFE, &limits()).unwrap();
        assert!(d.ask_start(&a.id));
        d.confirm_start(Some(&mut c)).unwrap();
        let p = d.pause(Some(&mut c), &a.id).unwrap();
        assert_eq!(p.state, "paused");
        assert!(matches!(d.done, Some(Done::Paused(_))));
        assert_eq!(d.resume(Some(&mut c), &a.id).unwrap().state, "running");
        let x = d.cancel(Some(&mut c), &a.id).unwrap();
        assert!(x.finished());
        assert!(matches!(d.done, Some(Done::Cancelled(_))));
    }

    #[test]
    fn the_emergency_stop_has_no_question_and_latches_the_machine() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        let a = d.enqueue(Some(&mut c), "cube", SAFE, &limits()).unwrap();
        assert!(d.ask_start(&a.id));
        let reached = d.emergency_stop(Some(&mut c));
        assert!(reached.is_some(), "{:?}", d.error);
        assert!(matches!(d.done, Some(Done::Stopped(ref m, _)) if m == "p1s-1"));
        assert!(d.machine().unwrap().latched);
        // the question that was open is gone and a latched machine is not asked about
        assert!(d.asking().is_none());
        assert!(!d.ask_start(&a.id));
        assert!(d.lines(Lang::En)[0].contains("latched"));
    }

    #[test]
    fn a_program_over_the_limit_is_refused_and_the_error_is_kept_in_the_language_of_the_person() {
        let (mut c, _sim) = client(TOKEN);
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        let hot = "G28\nM104 S350\n";
        assert_eq!(d.enqueue(Some(&mut c), "hot", hot, &limits()), None);
        assert!(matches!(d.error, Some(BridgeError::Temp { .. })));
        assert!(d.error_text(Lang::En).unwrap().contains("350"));
        assert!(d.queue.is_empty());
        // the limits of the profile that are stricter than the server's win
        let strict = TempLimits {
            nozzle_c: 200.0,
            ..limits()
        };
        assert_eq!(d.enqueue(Some(&mut c), "warm", SAFE, &strict), None);
        assert!(d.error.is_some());
    }

    #[test]
    fn a_token_the_server_does_not_know_is_an_error_that_asks_for_a_login() {
        let (mut c, _sim) = client("not-the-token");
        let mut d = DevicePanel::new();
        d.refresh(Some(&mut c));
        assert!(d.machines.is_empty());
        assert!(
            d.error.as_ref().is_some_and(BridgeError::needs_login),
            "{:?}",
            d.error
        );
        assert!(d.error_text(Lang::Es).is_some());
    }
}
