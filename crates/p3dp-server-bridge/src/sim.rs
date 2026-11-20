//! The server in memory: a double of the `printer3d` routes of the server, over the server in memory of the suite (`eh3d_serverclient::sim::SimServer`), which does the login, the
//! renewal, the health check and the failures on demand. It is a double, not the server: it keeps the order of the answers and the codes of the refusals that the real server has,
//! and it checks a program against the limits of the machine the way the real one does, but it prints nothing and heats nothing.

use eh3d_serverclient as sc;
use eh3d_serverclient::sim::SimServer;
use p3dp_core::json::{self, Value};
use p3dp_core::TempLimits;
use p3dp_gcode::check_program;
use std::sync::{Arc, Mutex};

/// The token of a program that the simulated server accepts.
pub const TOKEN: &str = "program-token";

/// A machine of the simulated server.
#[derive(Debug, Clone)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub plugin: String,
    pub model: String,
    pub connection: String,
    pub online: bool,
    pub latched: bool,
    pub limits: TempLimits,
    pub auto_start_queue: bool,
}

/// A job of the simulated server.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub machine: String,
    pub name: String,
    pub state: String,
    pub percent: u64,
    pub message: String,
    pub text: String,
}

/// What the simulated server holds.
#[derive(Debug, Default)]
pub struct State {
    pub machines: Vec<Machine>,
    pub jobs: Vec<Job>,
    /// `METHOD path` of every request to the routes of the machines, in order.
    pub requests: Vec<String>,
    /// The bodies of the requests that were posted.
    pub posted: Vec<String>,
}

/// A handle to the simulated server; clones share the state.
#[derive(Clone)]
pub struct SimPrinterServer {
    /// The server of the suite underneath: scripts failures, tells which requests carried a token, counts the requests.
    pub auth: SimServer,
    state: Arc<Mutex<State>>,
}

impl Default for SimPrinterServer {
    fn default() -> SimPrinterServer {
        SimPrinterServer::new()
    }
}

fn limits(nozzle: f64, bed: f64, chamber: Option<f64>) -> TempLimits {
    TempLimits {
        nozzle_c: nozzle,
        bed_c: bed,
        chamber_c: chamber,
    }
}

fn reply(status: u16, v: &Value) -> sc::Reply {
    sc::Reply {
        status,
        body: v.to_json(),
    }
}

fn error(status: u16, code: &str) -> sc::Reply {
    reply(
        status,
        &Value::object(vec![
            ("error", Value::Str(code.to_string())),
            ("detail", Value::Str(String::new())),
        ]),
    )
}

fn machine_view(m: &Machine, jobs: &[Job]) -> Value {
    let active = jobs
        .iter()
        .find(|j| j.machine == m.id && matches!(j.state.as_str(), "running" | "paused"));
    let busy = active.is_some() || m.latched;
    Value::object(vec![
        ("id", Value::Str(m.id.clone())),
        ("name", Value::Str(m.name.clone())),
        ("plugin", Value::Str(m.plugin.clone())),
        (
            "state",
            Value::Str(if busy { "busy" } else { "idle" }.to_string()),
        ),
        (
            "job",
            active.map_or(Value::Null, |j| Value::Str(j.id.clone())),
        ),
        (
            "detail",
            Value::object(vec![
                ("model", Value::Str(m.model.clone())),
                ("connection", Value::Str(m.connection.clone())),
                ("online", Value::Bool(m.online)),
                ("latched", Value::Bool(m.latched)),
                (
                    "limits_c",
                    Value::object(vec![
                        ("hotend", Value::Num(m.limits.nozzle_c)),
                        ("bed", Value::Num(m.limits.bed_c)),
                        ("chamber", Value::Num(m.limits.chamber_c.unwrap_or(0.0))),
                    ]),
                ),
                ("auto_start_queue", Value::Bool(m.auto_start_queue)),
            ]),
        ),
    ])
}

fn job_view(j: &Job, position: Option<usize>) -> Value {
    Value::object(vec![
        ("id", Value::Str(j.id.clone())),
        ("machine", Value::Str(j.machine.clone())),
        ("name", Value::Str(j.name.clone())),
        ("state", Value::Str(j.state.clone())),
        ("percent", Value::Num(j.percent as f64)),
        ("message", Value::Str(j.message.clone())),
        (
            "position",
            position.map_or(Value::Null, |p| Value::Num(p as f64)),
        ),
    ])
}

impl SimPrinterServer {
    /// A server with two 3D printers (a Bambu P1S model and a generic one that is offline) and one machine of another plugin, which the client must leave alone.
    pub fn new() -> SimPrinterServer {
        let machine = |id: &str, name: &str, plugin: &str, model: &str, online: bool| Machine {
            id: id.to_string(),
            name: name.to_string(),
            plugin: plugin.to_string(),
            model: model.to_string(),
            connection: "demo".to_string(),
            online,
            latched: false,
            limits: if model == "bambu_p1s" {
                limits(300.0, 100.0, None)
            } else {
                limits(260.0, 100.0, Some(60.0))
            },
            auto_start_queue: false,
        };
        let state = State {
            machines: vec![
                machine("p1s-1", "Bambu P1S", "printer3d", "bambu_p1s", true),
                machine("mk4-1", "Prusa MK4S", "printer3d", "generic", false),
                machine("router-1", "Router", "cnc", "", true),
            ],
            ..State::default()
        };
        SimPrinterServer {
            auth: SimServer::new(),
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// A look at what the server holds.
    pub fn with_state<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let mut g = self.state.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut g)
    }

    /// The pauses between tries, the requests that carried a token and the like are in `auth`.
    fn work(&self, req: &sc::Request) -> sc::Reply {
        self.with_state(|s| route(s, req))
    }
}

fn position_of(jobs: &[Job], job: &Job) -> Option<usize> {
    if job.state != "queued" {
        return None;
    }
    jobs.iter()
        .filter(|j| j.machine == job.machine && j.state == "queued")
        .position(|j| j.id == job.id)
}

fn route(s: &mut State, req: &sc::Request) -> sc::Reply {
    s.requests.push(format!("{} {}", req.method, req.path));
    let (path, query) = req.path.split_once('?').unwrap_or((&req.path, ""));
    let tail = path.trim_start_matches("/api/v1/work/");
    let parts: Vec<&str> = tail.split('/').collect();
    match (req.method.as_str(), parts.as_slice()) {
        ("GET", ["machines"]) => {
            let list: Vec<Value> = s
                .machines
                .iter()
                .map(|m| machine_view(m, &s.jobs))
                .collect();
            reply(200, &Value::object(vec![("machines", Value::Arr(list))]))
        }
        ("GET", ["machines", id]) => match s.machines.iter().find(|m| m.id == *id) {
            Some(m) => reply(200, &machine_view(m, &s.jobs)),
            None => error(404, "unknown_machine"),
        },
        ("GET", ["machines", id, "temperatures"]) => match printer(s, id) {
            Some(m) => {
                let heater = |name: &str, current: f64, limit: f64| {
                    Value::object(vec![
                        ("name", Value::Str(name.to_string())),
                        ("current_c", Value::Num(current)),
                        ("target_c", Value::Num(0.0)),
                        ("power_pct", Value::Num(0.0)),
                        ("limit_c", Value::Num(limit)),
                    ])
                };
                let mut list = vec![
                    heater("hotend", 21.5, m.limits.nozzle_c),
                    heater("bed", 21.0, m.limits.bed_c),
                ];
                if let Some(c) = m.limits.chamber_c {
                    list.push(heater("chamber", 21.0, c));
                }
                reply(
                    200,
                    &Value::object(vec![
                        ("heaters", Value::Arr(list)),
                        ("history", Value::Arr(Vec::new())),
                        ("t_s", Value::Num(1.0)),
                    ]),
                )
            }
            None => error(404, "unknown_machine"),
        },
        ("GET", ["machines", id, "layer"]) => match printer(s, id) {
            Some(_) => reply(
                200,
                &Value::object(vec![
                    ("layer", Value::Num(12.0)),
                    ("layers", Value::Num(100.0)),
                    ("z_mm", Value::Num(2.4)),
                    ("progress_pct", Value::Num(12.0)),
                    ("eta_s", Value::Num(1800.0)),
                ]),
            ),
            None => error(404, "unknown_machine"),
        },
        ("POST", ["machines", id, "emergency-stop"]) => {
            let Some(i) = s
                .machines
                .iter()
                .position(|m| m.id == *id && m.plugin == "printer3d")
            else {
                return error(404, "unknown_machine");
            };
            s.machines[i].latched = true;
            let mut stopped = false;
            for j in s
                .jobs
                .iter_mut()
                .filter(|j| j.machine == *id && matches!(j.state.as_str(), "running" | "paused"))
            {
                j.state = "cancelled".to_string();
                stopped = true;
            }
            reply(
                200,
                &Value::object(vec![
                    ("stopped", Value::Bool(stopped)),
                    ("latched", Value::Bool(true)),
                    ("printer_reached", Value::Bool(s.machines[i].online)),
                ]),
            )
        }
        ("POST", ["machines", id, "release"]) => {
            match s
                .machines
                .iter_mut()
                .find(|m| m.id == *id && m.plugin == "printer3d")
            {
                Some(m) => {
                    m.latched = false;
                    reply(200, &Value::object(vec![("ok", Value::Bool(true))]))
                }
                None => error(404, "unknown_machine"),
            }
        }
        ("POST", ["machines"]) => create_machine(s, req),
        ("GET", ["jobs"]) => {
            let wanted = query
                .split('&')
                .find_map(|p| p.strip_prefix("machine="))
                .map(str::to_string);
            let items: Vec<Value> = s
                .jobs
                .iter()
                .rev()
                .filter(|j| wanted.as_deref().is_none_or(|m| j.machine == m))
                .map(|j| job_view(j, position_of(&s.jobs, j)))
                .collect();
            reply(
                200,
                &Value::object(vec![
                    ("total", Value::Num(items.len() as f64)),
                    ("offset", Value::Num(0.0)),
                    ("limit", Value::Num(50.0)),
                    ("items", Value::Arr(items)),
                ]),
            )
        }
        ("POST", ["jobs"]) => create_job(s, req),
        ("GET", ["jobs", id]) => match s.jobs.iter().find(|j| j.id == *id) {
            Some(j) => reply(200, &job_view(j, position_of(&s.jobs, j))),
            None => error(404, "unknown_job"),
        },
        ("POST", ["jobs", id, action]) => job_action(s, id, action),
        _ => error(404, "not_found"),
    }
}

fn printer<'a>(s: &'a State, id: &str) -> Option<&'a Machine> {
    s.machines
        .iter()
        .find(|m| m.id == id && m.plugin == "printer3d")
}

fn create_machine(s: &mut State, req: &sc::Request) -> sc::Reply {
    let text = req.body.clone().unwrap_or_default();
    s.posted.push(text.clone());
    let Ok(v) = json::parse(&text) else {
        return error(400, "bad_json");
    };
    let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (id, name, plugin) = (get("id"), get("name"), get("plugin"));
    if id.is_empty() || name.is_empty() || plugin != "printer3d" {
        return error(422, "bad_settings");
    }
    if s.machines.iter().any(|m| m.id == id) {
        return error(409, "machine_exists");
    }
    let settings = v.get("settings");
    let model = settings
        .and_then(|x| x.get("model"))
        .and_then(Value::as_str)
        .unwrap_or("generic")
        .to_string();
    let kind = settings
        .and_then(|x| x.get("connection"))
        .and_then(|c| c.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("demo")
        .to_string();
    let lim = match model.as_str() {
        "bambu_a1" | "bambu_p1s" => limits(300.0, 100.0, None),
        "bambu_a1_mini" => limits(300.0, 80.0, None),
        "generic" => limits(260.0, 100.0, Some(60.0)),
        _ => return error(422, "bad_settings"),
    };
    s.machines.push(Machine {
        id: id.clone(),
        name,
        plugin,
        model,
        connection: kind,
        online: true,
        latched: false,
        limits: lim,
        auto_start_queue: false,
    });
    let last = &s.machines[s.machines.len() - 1];
    reply(201, &machine_view(last, &s.jobs))
}

fn create_job(s: &mut State, req: &sc::Request) -> sc::Reply {
    let text = req.body.clone().unwrap_or_default();
    s.posted.push(text.clone());
    let Ok(v) = json::parse(&text) else {
        return error(400, "bad_json");
    };
    let machine = v.get("machine").and_then(Value::as_str).unwrap_or("");
    let Some(m) = printer(s, machine).cloned() else {
        return error(404, "unknown_machine");
    };
    if m.latched {
        return error(409, "printer_latched");
    }
    let request = v.get("request");
    let program = request
        .and_then(|r| r.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if request.and_then(|r| r.get("kind")).and_then(Value::as_str) != Some("print")
        || program.is_empty()
    {
        return error(422, "bad_request");
    }
    // the real server reads the program with the limits of the machine and refuses the whole program
    if !check_program(program, &m.limits).is_empty() {
        return error(422, "program_over_limit");
    }
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .unwrap_or("job")
        .to_string();
    let job = Job {
        id: format!("w-{}", s.jobs.len() + 1),
        machine: m.id.clone(),
        name,
        state: "queued".to_string(),
        percent: 0,
        message: String::new(),
        text: program.to_string(),
    };
    s.jobs.push(job);
    let last = &s.jobs[s.jobs.len() - 1];
    let out = job_view(last, position_of(&s.jobs, last));
    reply(201, &out)
}

fn job_action(s: &mut State, id: &str, action: &str) -> sc::Reply {
    let Some(i) = s.jobs.iter().position(|j| j.id == id) else {
        return error(404, "unknown_job");
    };
    let machine = s.jobs[i].machine.clone();
    let state = s.jobs[i].state.clone();
    let wanted: &[&str] = match action {
        "start" => &["queued"],
        "pause" => &["running"],
        "resume" => &["paused"],
        "cancel" => &["queued", "running", "paused"],
        _ => return error(404, "not_found"),
    };
    if !wanted.contains(&state.as_str()) {
        return error(409, "not_in_state");
    }
    if action == "start" {
        if printer(s, &machine).is_some_and(|m| m.latched) {
            return error(409, "printer_latched");
        }
        let busy = s
            .jobs
            .iter()
            .any(|j| j.machine == machine && matches!(j.state.as_str(), "running" | "paused"));
        if busy {
            return error(409, "machine_busy");
        }
    }
    s.jobs[i].state = match action {
        "start" | "resume" => "running",
        "pause" => "paused",
        _ => "cancelled",
    }
    .to_string();
    let job = s.jobs[i].clone();
    reply(200, &job_view(&job, position_of(&s.jobs, &job)))
}

impl sc::Transport for SimPrinterServer {
    fn send(&self, request: &sc::Request) -> Result<sc::Reply, sc::TransportError> {
        if request.path.starts_with("/api/v1/work/") {
            // the login and the failures on demand are the suite's: a request of the same kind goes to its server first
            let mut probe = request.clone();
            probe.method = "GET".to_string();
            probe.path = "/api/v1/echo".to_string();
            probe.body = None;
            let checked = self.auth.send(&probe)?;
            if checked.status != 200 {
                return Ok(checked);
            }
            return Ok(self.work(request));
        }
        self.auth.send(request)
    }
}
