//! The client of the `printer3d` plugin of the server. It speaks the routes of the machine plugins of the server (`/api/v1/work/machines`, `/api/v1/work/jobs`, the routes of the
//! printer under a machine) over the one client of the suite, so the login, the renewal, the retries, the state of the link and the text of every refusal are the ones the other
//! programs have. What is this program's own is here: the checks of a program before it leaves (the limits of temperature of the model, refused and never cut down), the job that
//! joins the queue and waits for a person, the stop, and the words of the server about a printer.
//!
//! A job is only ever **queued** by `enqueue`. It starts when a person says so: `start` needs a [`Confirmation`], which only a call that stands for that person's click can make.
//! Whether the server starts queued jobs by itself (`auto_start_queue`) is the server's policy, off by default, and this program never asks for it.

use crate::settings::Settings;
use eh3d_serverclient as sc;
use p3dp_core::json::{self, Value};
use p3dp_core::machine::TempError;
use p3dp_core::TempLimits;
use p3dp_gcode::check_program;
use std::fmt;

pub use sc::Link as LinkState;

/// The name this program gives the server when it logs in. A server that only knows the applications of its list needs this name in `auth_extra_apps`, or a token of a program.
pub const APP: &str = "phoenix-3dp";

/// The most bytes of a program that one job carries.
pub const MAX_PROGRAM_BYTES: usize = 64 << 20;

/// The longest name of a job.
const MAX_NAME_CHARS: usize = 80;

/// Why a call did not end well.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// The server, or the network on the way to it, said no: the error of the client of the suite, which has the text in eight languages.
    Server(sc::ClientError),
    /// A temperature of the program is above what the model allows: nothing was sent. `line` counts from 1.
    Temp { line: usize, error: TempError },
    /// A value that is not allowed (an id, a name, an empty program): nothing was sent.
    Invalid(String),
    /// The server answered something that cannot be read.
    BadReply(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::Server(e) => write!(f, "{e}"),
            BridgeError::Temp { line, error } => write!(f, "line {line}: {error}"),
            BridgeError::Invalid(s) => f.write_str(s),
            BridgeError::BadReply(s) => write!(f, "the answer could not be understood: {s}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<sc::ClientError> for BridgeError {
    fn from(e: sc::ClientError) -> BridgeError {
        BridgeError::Server(e)
    }
}

impl BridgeError {
    /// The text for the person in their language: what the server said in its code when it gave one the suite knows, else the plain words of the failure.
    pub fn text(&self, lang: sc::Lang) -> String {
        match self {
            BridgeError::Server(e) => e.text(lang),
            other => other.to_string(),
        }
    }

    /// Whether a new login would cure it.
    pub fn needs_login(&self) -> bool {
        matches!(self, BridgeError::Server(e) if e.needs_login())
    }

    /// The code of the refusal of the server (`program_over_limit`, `printer_latched`...), when it gave one.
    pub fn code(&self) -> Option<&str> {
        match self {
            BridgeError::Server(e) => e.code(),
            _ => None,
        }
    }
}

/// The word of a person: a job starts only with one of these. It is made by the call that stands for the click on "start" (or the confirmation typed in the command line), so a
/// piece of code that only has the id of a job cannot move a printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmation(());

impl Confirmation {
    /// Call it only where a person has just said "start this job".
    pub fn by_person() -> Confirmation {
        Confirmation(())
    }
}

/// A printer as the server lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct PrinterInfo {
    pub id: String,
    pub name: String,
    /// `idle` or `busy` (a latched machine is `busy`).
    pub state: String,
    /// The job of the generic queue that has the machine now.
    pub job: Option<String>,
    /// The model of the profile of the server (`generic`, `bambu_a1`...).
    pub model: String,
    /// `demo`, `moonraker` or `octoprint`.
    pub connection: String,
    pub online: bool,
    /// Latched by an emergency stop or by a fault until an administrator releases it.
    pub latched: bool,
    /// The limits of temperature the server keeps for this machine.
    pub limits: Option<TempLimits>,
    /// Whether the server starts the queued jobs by itself: the policy of the server, off by default.
    pub auto_start_queue: bool,
}

/// A heater of a printer.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaterInfo {
    pub name: String,
    pub current_c: f64,
    pub target_c: f64,
    pub power_pct: f64,
    pub limit_c: f64,
}

/// The layer a print is at.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub layer: u64,
    pub layers: u64,
    pub z_mm: f64,
    pub progress_pct: f64,
    pub eta_s: Option<f64>,
}

/// A job of the queue of the server.
#[derive(Debug, Clone, PartialEq)]
pub struct JobInfo {
    pub id: String,
    pub machine: String,
    pub name: String,
    /// `queued`, `running`, `paused`, `done`, `failed` or `cancelled`.
    pub state: String,
    pub percent: u64,
    pub message: String,
    /// Its place among the queued jobs of the machine.
    pub position: Option<u64>,
}

impl JobInfo {
    /// The job is over, one way or another.
    pub fn finished(&self) -> bool {
        matches!(self.state.as_str(), "done" | "failed" | "cancelled")
    }

    /// The job waits for a person to start it.
    pub fn waiting(&self) -> bool {
        self.state == "queued"
    }
}

/// How the server reaches the printer it is told about when a machine is made.
#[derive(Debug, Clone, PartialEq)]
pub enum MachineConnection {
    /// The simulator of the server: no printer.
    Demo,
    /// Klipper through Moonraker: the address and, when it has one, the key. The key is written to the server once and never read back.
    Moonraker { url: String, api_key: String },
    /// OctoPrint: the address and the key.
    OctoPrint { url: String, api_key: String },
}

/// The model of the profile of the server that goes with a profile of the catalog of the program. The server keeps a short list of models with the limits of the maker; a printer it
/// does not list gets `generic`, the conservative limits of the server, which are the safe side.
pub fn server_model_for(profile_id: &str) -> &'static str {
    match profile_id {
        "bambu-lab-a1" => "bambu_a1",
        "bambu-lab-a1-mini" => "bambu_a1_mini",
        "bambu-lab-p1s" => "bambu_p1s",
        _ => "generic",
    }
}

/// The lowest of two sets of limits: a heater may be asked no more than the stricter of the model of the program and the model of the server. A chamber that either of them lacks is
/// a chamber that is not there.
pub fn stricter(a: &TempLimits, b: &TempLimits) -> TempLimits {
    TempLimits {
        nozzle_c: a.nozzle_c.min(b.nozzle_c),
        bed_c: a.bed_c.min(b.bed_c),
        chamber_c: match (a.chamber_c, b.chamber_c) {
            (Some(x), Some(y)) => Some(x.min(y)),
            _ => None,
        },
    }
}

fn path_id(id: &str) -> Result<String, BridgeError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        || id.starts_with('.')
    {
        return Err(BridgeError::Invalid(format!("{id:?} is not a valid id")));
    }
    Ok(sc::path_segment(id))
}

fn parse(reply: &sc::Reply) -> Result<Value, BridgeError> {
    json::parse(&reply.body).map_err(|e| BridgeError::BadReply(e.to_string()))
}

fn text_of(v: &Value, key: &str) -> Result<String, BridgeError> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| BridgeError::BadReply(format!("the answer has no {key}")))
}

fn text_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn number_or(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}

fn whole_or(v: &Value, key: &str) -> u64 {
    let n = number_or(v, key, 0.0);
    if (0.0..1.8e19).contains(&n) {
        n as u64
    } else {
        0
    }
}

fn job_of(v: &Value) -> Result<JobInfo, BridgeError> {
    Ok(JobInfo {
        id: text_of(v, "id")?,
        machine: text_of(v, "machine")?,
        name: text_or(v, "name"),
        state: text_of(v, "state")?,
        percent: whole_or(v, "percent"),
        message: text_or(v, "message"),
        position: v
            .get("position")
            .and_then(Value::as_f64)
            .filter(|p| *p >= 0.0)
            .map(|p| p as u64),
    })
}

fn printer_of(v: &Value) -> Result<PrinterInfo, BridgeError> {
    let detail = v.get("detail");
    let limits = detail.and_then(|d| d.get("limits_c")).and_then(|l| {
        let nozzle_c = l.get("hotend").and_then(Value::as_f64)?;
        let bed_c = l.get("bed").and_then(Value::as_f64)?;
        let chamber = l.get("chamber").and_then(Value::as_f64).unwrap_or(0.0);
        Some(TempLimits {
            nozzle_c,
            bed_c,
            chamber_c: (chamber > 0.0).then_some(chamber),
        })
    });
    let flag = |key: &str| {
        detail
            .and_then(|d| d.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    Ok(PrinterInfo {
        id: text_of(v, "id")?,
        name: text_of(v, "name")?,
        state: text_of(v, "state")?,
        job: v.get("job").and_then(Value::as_str).map(str::to_string),
        model: detail.map_or_else(String::new, |d| text_or(d, "model")),
        connection: detail.map_or_else(String::new, |d| text_or(d, "connection")),
        online: flag("online"),
        latched: flag("latched"),
        limits,
        auto_start_queue: flag("auto_start_queue"),
    })
}

/// The client.
pub struct ServerClient {
    inner: sc::Client,
    settings: Settings,
    max_program_bytes: usize,
}

impl ServerClient {
    /// A client over a transport (a server in memory for the tests, a tunnel). The settings must have an address and the link settings must be accepted by the client of the suite.
    pub fn new(
        settings: Settings,
        transport: Box<dyn sc::Transport>,
    ) -> Result<ServerClient, BridgeError> {
        let link = link_settings(&settings)?;
        Ok(ServerClient {
            inner: sc::Client::new(link, transport),
            settings,
            max_program_bytes: MAX_PROGRAM_BYTES,
        })
    }

    /// A client over plain HTTP to the address of the settings.
    pub fn connect(settings: Settings) -> Result<ServerClient, BridgeError> {
        let link = link_settings(&settings)?;
        Ok(ServerClient {
            inner: sc::Client::connect(link),
            settings,
            max_program_bytes: MAX_PROGRAM_BYTES,
        })
    }

    /// Another sleeper for the pauses between tries (the tests give one that does not wait).
    pub fn with_sleeper(mut self, sleeper: Box<dyn sc::Sleeper>) -> ServerClient {
        self.inner = self.inner.with_sleeper(sleeper);
        self
    }

    /// The most bytes of a program that a job may carry (1 byte to the 64 MiB the program allows).
    pub fn with_max_program_bytes(mut self, bytes: usize) -> ServerClient {
        self.max_program_bytes = bytes.clamp(1, MAX_PROGRAM_BYTES);
        self
    }

    pub fn link(&self) -> LinkState {
        self.inner.link()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The client of the suite underneath, for what it holds besides the routes of the printers (the metrics of the link, the sessions).
    pub fn inner(&self) -> &sc::Client {
        &self.inner
    }

    /// Asks whether the server is there. No token is sent.
    pub fn health(&mut self) -> Result<bool, BridgeError> {
        Ok(self.inner.health()?.status == "ok")
    }

    /// Logs in for this program. The password is sent once and kept nowhere.
    pub fn login(&mut self, name: &str, password: &str) -> Result<sc::Session, BridgeError> {
        Ok(self.inner.login(name, password, APP)?)
    }

    fn get(&mut self, path: &str) -> Result<Value, BridgeError> {
        let r = self.inner.get(path)?;
        parse(&r)
    }

    fn post(&mut self, path: &str, body: Option<&Value>) -> Result<Value, BridgeError> {
        let text = body.map(Value::to_json);
        let r = self.inner.post(path, text.as_deref())?;
        if r.body.trim().is_empty() {
            return Ok(Value::Null);
        }
        parse(&r)
    }

    /// The 3D printers of the server (the machines of the plugin `printer3d`; the others are not this program's).
    pub fn printers(&mut self) -> Result<Vec<PrinterInfo>, BridgeError> {
        let v = self.get("/api/v1/work/machines")?;
        let list = v
            .get("machines")
            .and_then(Value::as_array)
            .ok_or_else(|| BridgeError::BadReply("no machines list".to_string()))?;
        list.iter()
            .filter(|m| m.get("plugin").and_then(Value::as_str) == Some("printer3d"))
            .map(printer_of)
            .collect()
    }

    /// One printer.
    pub fn printer(&mut self, id: &str) -> Result<PrinterInfo, BridgeError> {
        let v = self.get(&format!("/api/v1/work/machines/{}", path_id(id)?))?;
        if v.get("plugin").and_then(Value::as_str) != Some("printer3d") {
            return Err(BridgeError::Invalid(format!(
                "{id:?} is not a 3D printer of the server"
            )));
        }
        printer_of(&v)
    }

    /// The heaters of a printer with what they read, what they aim at and the limit the server keeps.
    pub fn temperatures(&mut self, id: &str) -> Result<Vec<HeaterInfo>, BridgeError> {
        let v = self.get(&format!(
            "/api/v1/work/machines/{}/temperatures",
            path_id(id)?
        ))?;
        let list = v
            .get("heaters")
            .and_then(Value::as_array)
            .ok_or_else(|| BridgeError::BadReply("no heaters list".to_string()))?;
        list.iter()
            .map(|h| {
                Ok(HeaterInfo {
                    name: text_of(h, "name")?,
                    current_c: number_or(h, "current_c", 0.0),
                    target_c: number_or(h, "target_c", 0.0),
                    power_pct: number_or(h, "power_pct", 0.0),
                    limit_c: number_or(h, "limit_c", 0.0),
                })
            })
            .collect()
    }

    /// The layer a print is at.
    pub fn layer(&mut self, id: &str) -> Result<LayerInfo, BridgeError> {
        let v = self.get(&format!("/api/v1/work/machines/{}/layer", path_id(id)?))?;
        Ok(LayerInfo {
            layer: whole_or(&v, "layer"),
            layers: whole_or(&v, "layers"),
            z_mm: number_or(&v, "z_mm", 0.0),
            progress_pct: number_or(&v, "progress_pct", 0.0),
            eta_s: v
                .get("eta_s")
                .and_then(Value::as_f64)
                .filter(|s| s.is_finite()),
        })
    }

    /// The jobs of the queue, newest first; with a machine, only its jobs.
    pub fn queue(&mut self, machine: Option<&str>) -> Result<Vec<JobInfo>, BridgeError> {
        let path = match machine {
            Some(m) => format!("/api/v1/work/jobs?machine={}", path_id(m)?),
            None => "/api/v1/work/jobs".to_string(),
        };
        let v = self.get(&path)?;
        let list = v
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| BridgeError::BadReply("no items list".to_string()))?;
        list.iter().map(job_of).collect()
    }

    /// One job.
    pub fn job(&mut self, id: &str) -> Result<JobInfo, BridgeError> {
        job_of(&self.get(&format!("/api/v1/work/jobs/{}", path_id(id)?))?)
    }

    /// Puts a program in the queue of a printer. The program is read first: a temperature above the `limits` is refused with its line and nothing is sent (the value is never lowered
    /// to the limit). The job joins the queue and **waits**: it starts with [`ServerClient::start`] and a [`Confirmation`], or when the server's own policy says so.
    pub fn enqueue(
        &mut self,
        machine: &str,
        name: &str,
        gcode: &str,
        limits: &TempLimits,
    ) -> Result<JobInfo, BridgeError> {
        path_id(machine)?;
        if name.chars().count() > MAX_NAME_CHARS
            || name.trim().is_empty()
            || name.chars().any(char::is_control)
        {
            return Err(BridgeError::Invalid(format!(
                "the name of a job is 1 to {MAX_NAME_CHARS} characters with no control character"
            )));
        }
        if gcode.trim().is_empty() {
            return Err(BridgeError::Invalid("the program is empty".to_string()));
        }
        if gcode.len() > self.max_program_bytes {
            return Err(BridgeError::Invalid(format!(
                "the program has {} bytes, more than the {} a job may carry",
                gcode.len(),
                self.max_program_bytes
            )));
        }
        if let Some(v) = check_program(gcode, limits).into_iter().next() {
            return Err(BridgeError::Temp {
                line: v.line,
                error: v.error,
            });
        }
        let request = Value::object(vec![
            ("kind", Value::Str("print".to_string())),
            ("text", Value::Str(gcode.to_string())),
            ("preheat", Value::Bool(false)),
        ]);
        let body = Value::object(vec![
            ("machine", Value::Str(machine.to_string())),
            ("request", request),
            ("name", Value::Str(name.to_string())),
            ("app", Value::Str(APP.to_string())),
        ]);
        job_of(&self.post("/api/v1/work/jobs", Some(&body))?)
    }

    /// Starts a queued job. It needs the word of a person.
    pub fn start(&mut self, job: &str, _by: Confirmation) -> Result<JobInfo, BridgeError> {
        job_of(&self.post(&format!("/api/v1/work/jobs/{}/start", path_id(job)?), None)?)
    }

    /// Holds a running job.
    pub fn pause(&mut self, job: &str) -> Result<JobInfo, BridgeError> {
        job_of(&self.post(&format!("/api/v1/work/jobs/{}/pause", path_id(job)?), None)?)
    }

    /// Lets a held job go on.
    pub fn resume(&mut self, job: &str) -> Result<JobInfo, BridgeError> {
        job_of(&self.post(&format!("/api/v1/work/jobs/{}/resume", path_id(job)?), None)?)
    }

    /// Cancels a queued job or stops a running one (the heaters go off).
    pub fn cancel(&mut self, job: &str) -> Result<JobInfo, BridgeError> {
        job_of(&self.post(&format!("/api/v1/work/jobs/{}/cancel", path_id(job)?), None)?)
    }

    /// The emergency stop of a printer: heaters off and motors released at once, the job ends and the machine is latched until an administrator releases it. The answer says whether
    /// the server reached the printer.
    pub fn emergency_stop(&mut self, machine: &str) -> Result<bool, BridgeError> {
        let v = self.post(
            &format!("/api/v1/work/machines/{}/emergency-stop", path_id(machine)?),
            None,
        )?;
        Ok(v.get("printer_reached")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Releases the latch of a printer (an administrator's call).
    pub fn release(&mut self, machine: &str) -> Result<(), BridgeError> {
        self.post(
            &format!("/api/v1/work/machines/{}/release", path_id(machine)?),
            None,
        )?;
        Ok(())
    }

    /// Makes a machine of the plugin `printer3d` on the server (an administrator's call). `model` is a model of the server (see [`server_model_for`]); the limits of temperature
    /// of the model are the server's own, and the program does not send others. The key of the printer is written once and never shown again.
    pub fn create_machine(
        &mut self,
        id: &str,
        name: &str,
        model: &str,
        connection: &MachineConnection,
    ) -> Result<PrinterInfo, BridgeError> {
        path_id(id)?;
        if name.trim().is_empty() || name.chars().count() > MAX_NAME_CHARS {
            return Err(BridgeError::Invalid(format!(
                "the name of a machine is 1 to {MAX_NAME_CHARS} characters"
            )));
        }
        let connection = match connection {
            MachineConnection::Demo => Value::object(vec![("kind", Value::Str("demo".into()))]),
            MachineConnection::Moonraker { url, api_key } => Value::object(vec![
                ("kind", Value::Str("moonraker".into())),
                ("url", Value::Str(url.clone())),
                ("api_key", Value::Str(api_key.clone())),
            ]),
            MachineConnection::OctoPrint { url, api_key } => Value::object(vec![
                ("kind", Value::Str("octoprint".into())),
                ("url", Value::Str(url.clone())),
                ("api_key", Value::Str(api_key.clone())),
            ]),
        };
        let settings = Value::object(vec![
            ("model", Value::Str(model.to_string())),
            ("connection", connection),
        ]);
        let body = Value::object(vec![
            ("id", Value::Str(id.to_string())),
            ("name", Value::Str(name.to_string())),
            ("plugin", Value::Str("printer3d".to_string())),
            ("settings", settings),
        ]);
        self.post("/api/v1/work/machines", Some(&body))?;
        self.printer(id)
    }
}

/// The settings of the link as the client of the suite takes them: every key goes through its own check, so a value the suite would refuse is refused here too.
pub fn link_settings(settings: &Settings) -> Result<sc::Settings, BridgeError> {
    if settings.server_address.is_empty() {
        return Err(BridgeError::Invalid(
            "server_address is empty: there is no server to talk to".to_string(),
        ));
    }
    let mut link = sc::Settings::default();
    let pairs = [
        ("server_address", settings.server_address.clone()),
        ("server_token", settings.server_token.expose().to_string()),
        ("server_machine", settings.server_machine.clone()),
        ("server_timeout_s", settings.server_timeout_s.to_string()),
        ("server_retries", settings.server_retries.to_string()),
        (
            "server_mdns",
            if settings.server_mdns { "on" } else { "off" }.to_string(),
        ),
        (
            "server_max_answer_mib",
            settings.server_max_answer_mib.to_string(),
        ),
    ];
    for (key, value) in pairs {
        link.set(key, &value)
            .map_err(|e| BridgeError::Invalid(e.to_string()))?;
    }
    Ok(link)
}
