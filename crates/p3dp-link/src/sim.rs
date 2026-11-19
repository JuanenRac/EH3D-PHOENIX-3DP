//! The simulators: a printer in memory with the HTTP of Moonraker or of OctoPrint in front of it. They are doubles written from the public description of those
//! protocols, to test the drivers without a network; they are not the real servers and nothing proves a real one answers the same way.

use crate::error::LinkError;
use crate::transport::{
    boundary_of, parse_multipart, HttpRequest, HttpResponse, HttpTransport, Method,
};
use p3dp_core::json::{self, Value};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// What the simulated printer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimState {
    Idle,
    Printing,
    Paused,
    Complete,
    Cancelled,
    Error,
    /// The firmware has stopped itself (an emergency stop, or a temperature beyond its own maximum).
    Shutdown,
}

/// A failure the simulator produces on the next request.
#[derive(Debug, Clone, PartialEq)]
pub enum Fault {
    NoConnection,
    Timeout,
    Status(u16, String),
}

/// The printer in memory.
#[derive(Debug)]
pub struct SimCore {
    pub state: SimState,
    pub nozzle: f64,
    pub nozzle_target: f64,
    pub bed: f64,
    pub bed_target: f64,
    pub chamber: f64,
    pub chamber_target: f64,
    pub fan_pct: f64,
    pub file: Option<String>,
    /// 0 to 1.
    pub progress: f64,
    pub elapsed_s: f64,
    pub total_s: f64,
    pub files: Vec<(String, Vec<u8>)>,
    /// Every line of G-code the printer was given.
    pub gcode_log: Vec<String>,
    /// The highest nozzle and bed targets that were ever asked of it.
    pub max_nozzle_target_seen: f64,
    pub max_bed_target_seen: f64,
    /// The firmware's own maximum: above it the printer shuts down.
    pub hard_max_nozzle: f64,
    pub hard_max_bed: f64,
    /// The key it asks for (empty: none).
    pub api_key: String,
    pub faults: VecDeque<Fault>,
    /// Every request that arrived, as `METHOD route`.
    pub requests: Vec<String>,
    /// For OctoPrint: whether the serial connection of the printer is up.
    pub connected: bool,
}

/// A printer that several handles share (the driver owns the transport; the test keeps a handle to move time).
pub type Shared = Rc<RefCell<SimCore>>;

impl SimCore {
    pub fn new() -> SimCore {
        SimCore {
            state: SimState::Idle,
            nozzle: 22.0,
            nozzle_target: 0.0,
            bed: 22.0,
            bed_target: 0.0,
            chamber: 22.0,
            chamber_target: 0.0,
            fan_pct: 0.0,
            file: None,
            progress: 0.0,
            elapsed_s: 0.0,
            total_s: 600.0,
            files: Vec::new(),
            gcode_log: Vec::new(),
            max_nozzle_target_seen: 0.0,
            max_bed_target_seen: 0.0,
            hard_max_nozzle: 320.0,
            hard_max_bed: 130.0,
            api_key: String::new(),
            faults: VecDeque::new(),
            requests: Vec::new(),
            connected: true,
        }
    }

    pub fn shared() -> Shared {
        Rc::new(RefCell::new(SimCore::new()))
    }

    fn approach(value: f64, target: f64, rate: f64, secs: f64, ambient: f64) -> f64 {
        let goal = if target > 0.0 { target } else { ambient };
        let step = rate * secs;
        if (goal - value).abs() <= step {
            goal
        } else if goal > value {
            value + step
        } else {
            value - step
        }
    }

    /// Lets time pass: the heaters move towards their targets and a print advances.
    pub fn advance(&mut self, secs: f64) {
        if self.state == SimState::Shutdown {
            self.nozzle_target = 0.0;
            self.bed_target = 0.0;
        }
        self.nozzle = SimCore::approach(self.nozzle, self.nozzle_target, 5.0, secs, 22.0);
        self.bed = SimCore::approach(self.bed, self.bed_target, 1.5, secs, 22.0);
        self.chamber = SimCore::approach(self.chamber, self.chamber_target, 0.5, secs, 22.0);
        if self.state == SimState::Printing {
            self.elapsed_s += secs;
            self.progress = (self.progress + secs / self.total_s).min(1.0);
            if self.progress >= 1.0 {
                self.state = SimState::Complete;
                self.nozzle_target = 0.0;
                self.bed_target = 0.0;
            }
        }
    }

    pub fn shut_down(&mut self) {
        self.state = SimState::Shutdown;
        self.nozzle_target = 0.0;
        self.bed_target = 0.0;
    }

    fn set_nozzle_target(&mut self, t: f64) {
        self.max_nozzle_target_seen = self.max_nozzle_target_seen.max(t);
        if t > self.hard_max_nozzle {
            self.shut_down(); // the firmware's MAXTEMP
        } else if self.state != SimState::Shutdown {
            self.nozzle_target = t;
        }
    }

    fn set_bed_target(&mut self, t: f64) {
        self.max_bed_target_seen = self.max_bed_target_seen.max(t);
        if t > self.hard_max_bed {
            self.shut_down();
        } else if self.state != SimState::Shutdown {
            self.bed_target = t;
        }
    }

    /// Runs one line of G-code (the few commands the drivers send).
    pub fn gcode(&mut self, line: &str) {
        let code = line.split(';').next().unwrap_or("").trim();
        if code.is_empty() {
            return;
        }
        self.gcode_log.push(code.to_string());
        let toks: Vec<&str> = code.split_ascii_whitespace().collect();
        let s = toks
            .iter()
            .skip(1)
            .find_map(|t| t.strip_prefix('S').and_then(|v| v.parse::<f64>().ok()));
        match toks[0].to_ascii_uppercase().as_str() {
            "M104" | "M109" => self.set_nozzle_target(s.unwrap_or(0.0)),
            "M140" | "M190" => self.set_bed_target(s.unwrap_or(0.0)),
            "M141" | "M191" => self.chamber_target = s.unwrap_or(0.0),
            "G10" => self.set_nozzle_target(s.unwrap_or(0.0)),
            "M106" => self.fan_pct = (s.unwrap_or(255.0) / 255.0 * 100.0).clamp(0.0, 100.0),
            "M107" => self.fan_pct = 0.0,
            "M112" => self.shut_down(),
            "SET_HEATER_TEMPERATURE" => {
                let get = |k: &str| {
                    toks.iter().skip(1).find_map(|t| {
                        let (a, b) = t.split_once('=')?;
                        a.eq_ignore_ascii_case(k).then(|| b.to_string())
                    })
                };
                let target: f64 = get("TARGET").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                match get("HEATER").as_deref() {
                    Some("extruder") => self.set_nozzle_target(target),
                    Some("heater_bed") => self.set_bed_target(target),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn start(&mut self, file: &str) -> Result<(), String> {
        if self.state == SimState::Shutdown {
            return Err("the printer is shut down".to_string());
        }
        if matches!(self.state, SimState::Printing | SimState::Paused) {
            return Err("a print is already under way".to_string());
        }
        if !self.files.iter().any(|(n, _)| n == file) {
            return Err(format!("the file {file} is not on the printer"));
        }
        self.file = Some(file.to_string());
        self.progress = 0.0;
        self.elapsed_s = 0.0;
        self.state = SimState::Printing;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.state == SimState::Printing {
            self.state = SimState::Paused;
            Ok(())
        } else {
            Err("there is no print to pause".to_string())
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        if self.state == SimState::Paused {
            self.state = SimState::Printing;
            Ok(())
        } else {
            Err("there is no paused print".to_string())
        }
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        if matches!(self.state, SimState::Printing | SimState::Paused) {
            self.state = SimState::Cancelled;
            self.nozzle_target = 0.0;
            self.bed_target = 0.0;
            Ok(())
        } else {
            Err("there is no print to cancel".to_string())
        }
    }

    /// Takes the next fault if there is one.
    fn next_fault(&mut self) -> Option<Fault> {
        self.faults.pop_front()
    }

    fn store(&mut self, name: &str, data: Vec<u8>) {
        self.files.retain(|(n, _)| n != name);
        self.files.push((name.to_string(), data));
    }
}

impl Default for SimCore {
    fn default() -> SimCore {
        SimCore::new()
    }
}

fn faulted(
    core: &Shared,
    req: &HttpRequest,
    key_header: &str,
) -> Option<Result<HttpResponse, LinkError>> {
    let mut c = core.borrow_mut();
    c.requests
        .push(format!("{} {}", req.method.name(), req.route()));
    match c.next_fault() {
        Some(Fault::NoConnection) => return Some(Err(LinkError::NoConnection)),
        Some(Fault::Timeout) => return Some(Err(LinkError::Timeout)),
        Some(Fault::Status(s, m)) => {
            return Some(Ok(HttpResponse::json(
                s,
                &Value::object(vec![("error", Value::Str(m))]).to_json(),
            )))
        }
        None => {}
    }
    if !c.api_key.is_empty() && req.header_value(key_header) != Some(c.api_key.as_str()) {
        return Some(Ok(HttpResponse::json(401, "{\"error\":\"unauthorized\"}")));
    }
    None
}

fn uploaded(req: &HttpRequest) -> Result<Vec<crate::transport::Part>, String> {
    let ct = req.header_value("Content-Type").ok_or("no content type")?;
    let b = boundary_of(ct).ok_or("no boundary")?;
    parse_multipart(&req.body, b).map_err(|e| e.to_string())
}

fn err_json(status: u16, msg: &str) -> HttpResponse {
    HttpResponse::json(
        status,
        &Value::object(vec![("error", Value::Str(msg.to_string()))]).to_json(),
    )
}

fn num(v: f64) -> Value {
    Value::Num((v * 100.0).round() / 100.0)
}

/// The simulated Moonraker server.
#[derive(Debug, Clone)]
pub struct SimMoonraker {
    pub core: Shared,
}

impl SimMoonraker {
    pub fn new(core: Shared) -> SimMoonraker {
        SimMoonraker { core }
    }

    fn handle(&self, req: &HttpRequest) -> HttpResponse {
        let mut c = self.core.borrow_mut();
        let ok = || HttpResponse::json(200, "{\"result\":\"ok\"}");
        let done = |r: Result<(), String>| match r {
            Ok(()) => ok(),
            Err(e) => err_json(400, &e),
        };
        match (req.method, req.route()) {
            (Method::Get, "/printer/objects/query") => {
                let print_state = match c.state {
                    SimState::Idle => "standby",
                    SimState::Printing => "printing",
                    SimState::Paused => "paused",
                    SimState::Complete => "complete",
                    SimState::Cancelled => "cancelled",
                    SimState::Error | SimState::Shutdown => "error",
                };
                let web = if c.state == SimState::Shutdown {
                    "shutdown"
                } else {
                    "ready"
                };
                let status = Value::object(vec![
                    (
                        "print_stats",
                        Value::object(vec![
                            ("state", Value::Str(print_state.to_string())),
                            ("filename", Value::Str(c.file.clone().unwrap_or_default())),
                            ("print_duration", num(c.elapsed_s)),
                        ]),
                    ),
                    (
                        "extruder",
                        Value::object(vec![
                            ("temperature", num(c.nozzle)),
                            ("target", num(c.nozzle_target)),
                        ]),
                    ),
                    (
                        "heater_bed",
                        Value::object(vec![
                            ("temperature", num(c.bed)),
                            ("target", num(c.bed_target)),
                        ]),
                    ),
                    (
                        "virtual_sdcard",
                        Value::object(vec![("progress", Value::Num(c.progress))]),
                    ),
                    (
                        "webhooks",
                        Value::object(vec![
                            ("state", Value::Str(web.to_string())),
                            ("state_message", Value::Str(String::new())),
                        ]),
                    ),
                ]);
                HttpResponse::json(
                    200,
                    &Value::object(vec![("result", Value::object(vec![("status", status)]))])
                        .to_json(),
                )
            }
            (Method::Post, "/printer/print/start") => match req.query("filename") {
                Some(f) => done(c.start(&f)),
                None => err_json(400, "filename is needed"),
            },
            (Method::Post, "/printer/print/pause") => done(c.pause()),
            (Method::Post, "/printer/print/resume") => done(c.resume()),
            (Method::Post, "/printer/print/cancel") => done(c.cancel()),
            (Method::Post, "/printer/emergency_stop") => {
                c.shut_down();
                ok()
            }
            (Method::Post, "/printer/gcode/script") => match req.query("script") {
                Some(s) => {
                    for l in s.split('\n') {
                        c.gcode(l);
                    }
                    ok()
                }
                None => err_json(400, "script is needed"),
            },
            (Method::Post, "/server/files/upload") => match uploaded(req) {
                Ok(parts) => {
                    let Some(file) = parts.iter().find(|p| p.name == "file") else {
                        return err_json(400, "no file");
                    };
                    let name = file.filename.clone().unwrap_or_default();
                    c.store(&name, file.data.clone());
                    let start = parts.iter().any(|p| p.name == "print" && p.data == b"true");
                    let started = start && c.start(&name).is_ok();
                    let body = Value::object(vec![(
                        "result",
                        Value::object(vec![
                            ("item", Value::object(vec![("path", Value::Str(name))])),
                            ("print_started", Value::Bool(started)),
                        ]),
                    )]);
                    HttpResponse::json(201, &body.to_json())
                }
                Err(e) => err_json(400, &e),
            },
            (Method::Get, "/server/files/list") => {
                let list: Vec<Value> = c
                    .files
                    .iter()
                    .map(|(n, d)| {
                        Value::object(vec![
                            ("path", Value::Str(n.clone())),
                            ("size", Value::Num(d.len() as f64)),
                        ])
                    })
                    .collect();
                HttpResponse::json(
                    200,
                    &Value::object(vec![("result", Value::Arr(list))]).to_json(),
                )
            }
            _ => err_json(404, "not found"),
        }
    }
}

impl HttpTransport for SimMoonraker {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        if let Some(r) = faulted(&self.core, req, "X-Api-Key") {
            return r;
        }
        Ok(self.handle(req))
    }
}

/// The simulated OctoPrint server.
#[derive(Debug, Clone)]
pub struct SimOctoPrint {
    pub core: Shared,
}

impl SimOctoPrint {
    pub fn new(core: Shared) -> SimOctoPrint {
        SimOctoPrint { core }
    }

    fn handle(&self, req: &HttpRequest) -> HttpResponse {
        let mut c = self.core.borrow_mut();
        let ok = || HttpResponse {
            status: 204,
            body: Vec::new(),
        };
        let done = |r: Result<(), String>| match r {
            Ok(()) => ok(),
            Err(e) => err_json(409, &e),
        };
        let body = || json::parse(&String::from_utf8_lossy(&req.body)).ok();
        let route = req.route().to_string();
        match (req.method, route.as_str()) {
            (Method::Get, "/api/version") => HttpResponse::json(
                200,
                "{\"api\":\"0.1\",\"server\":\"sim\",\"text\":\"OctoPrint simulator\"}",
            ),
            (Method::Get, "/api/printer") => {
                if !c.connected {
                    return HttpResponse::json(409, "Printer is not operational");
                }
                let error = matches!(c.state, SimState::Error | SimState::Shutdown);
                let printing = c.state == SimState::Printing;
                let paused = c.state == SimState::Paused;
                let text = match c.state {
                    SimState::Printing => "Printing",
                    SimState::Paused => "Paused",
                    SimState::Error | SimState::Shutdown => "Error",
                    _ => "Operational",
                };
                let flags = Value::object(vec![
                    ("operational", Value::Bool(!error)),
                    ("printing", Value::Bool(printing || paused)),
                    ("paused", Value::Bool(paused)),
                    ("pausing", Value::Bool(false)),
                    ("cancelling", Value::Bool(false)),
                    ("error", Value::Bool(error)),
                    ("closedOrError", Value::Bool(error)),
                    ("ready", Value::Bool(c.state == SimState::Idle)),
                ]);
                let temp =
                    |a: f64, t: f64| Value::object(vec![("actual", num(a)), ("target", num(t))]);
                let body = Value::object(vec![
                    (
                        "state",
                        Value::object(vec![
                            ("text", Value::Str(text.to_string())),
                            ("flags", flags),
                        ]),
                    ),
                    (
                        "temperature",
                        Value::object(vec![
                            ("tool0", temp(c.nozzle, c.nozzle_target)),
                            ("bed", temp(c.bed, c.bed_target)),
                            ("chamber", temp(c.chamber, c.chamber_target)),
                        ]),
                    ),
                ]);
                HttpResponse::json(200, &body.to_json())
            }
            (Method::Get, "/api/job") => {
                let left = if c.progress > 0.0 {
                    Value::Num((c.total_s * (1.0 - c.progress)).round())
                } else {
                    Value::Null
                };
                let body = Value::object(vec![
                    (
                        "job",
                        Value::object(vec![(
                            "file",
                            Value::object(vec![(
                                "name",
                                c.file.clone().map(Value::Str).unwrap_or(Value::Null),
                            )]),
                        )]),
                    ),
                    (
                        "progress",
                        Value::object(vec![
                            ("completion", num(c.progress * 100.0)),
                            ("printTime", Value::Num(c.elapsed_s.round())),
                            ("printTimeLeft", left),
                        ]),
                    ),
                ]);
                HttpResponse::json(200, &body.to_json())
            }
            (Method::Post, "/api/job") => {
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                match b.get("command").and_then(Value::as_str) {
                    Some("start") => match c.file.clone() {
                        Some(f) => done(c.start(&f)),
                        None => err_json(409, "no file selected"),
                    },
                    Some("cancel") => done(c.cancel()),
                    Some("pause") => match b.get("action").and_then(Value::as_str) {
                        Some("pause") => done(c.pause()),
                        Some("resume") => done(c.resume()),
                        _ => err_json(400, "action is needed"),
                    },
                    _ => err_json(400, "unknown command"),
                }
            }
            (Method::Post, "/api/printer/tool") => {
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                match b.path(&["targets", "tool0"]).and_then(Value::as_f64) {
                    Some(t) => {
                        c.set_nozzle_target(t);
                        ok()
                    }
                    None => err_json(400, "targets.tool0 is needed"),
                }
            }
            (Method::Post, "/api/printer/bed") => {
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                match b.get("target").and_then(Value::as_f64) {
                    Some(t) => {
                        c.set_bed_target(t);
                        ok()
                    }
                    None => err_json(400, "target is needed"),
                }
            }
            (Method::Post, "/api/printer/chamber") => {
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                match b.get("target").and_then(Value::as_f64) {
                    Some(t) => {
                        c.chamber_target = t;
                        ok()
                    }
                    None => err_json(400, "target is needed"),
                }
            }
            (Method::Post, "/api/printer/command") => {
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                let mut lines: Vec<String> = Vec::new();
                if let Some(s) = b.get("command").and_then(Value::as_str) {
                    lines.push(s.to_string());
                }
                if let Some(a) = b.get("commands").and_then(Value::as_array) {
                    lines.extend(a.iter().filter_map(|v| v.as_str().map(str::to_string)));
                }
                for l in &lines {
                    c.gcode(l);
                }
                ok()
            }
            (Method::Post, "/api/files/local") => match uploaded(req) {
                Ok(parts) => {
                    let Some(file) = parts.iter().find(|p| p.name == "file") else {
                        return err_json(400, "no file");
                    };
                    let name = file.filename.clone().unwrap_or_default();
                    c.store(&name, file.data.clone());
                    let flag = |k: &str| parts.iter().any(|p| p.name == k && p.data == b"true");
                    if flag("select") || flag("print") {
                        c.file = Some(name.clone());
                    }
                    let started = flag("print") && c.start(&name).is_ok();
                    let body = Value::object(vec![
                        ("done", Value::Bool(true)),
                        (
                            "files",
                            Value::object(vec![(
                                "local",
                                Value::object(vec![("name", Value::Str(name))]),
                            )]),
                        ),
                        ("printing", Value::Bool(started)),
                    ]);
                    HttpResponse::json(201, &body.to_json())
                }
                Err(e) => err_json(400, &e),
            },
            (Method::Get, "/api/files/local") => {
                let list: Vec<Value> = c
                    .files
                    .iter()
                    .map(|(n, _)| {
                        Value::object(vec![
                            ("name", Value::Str(n.clone())),
                            ("type", Value::Str("machinecode".to_string())),
                        ])
                    })
                    .collect();
                HttpResponse::json(
                    200,
                    &Value::object(vec![("files", Value::Arr(list))]).to_json(),
                )
            }
            (Method::Post, r) if r.starts_with("/api/files/local/") => {
                let name = crate::transport::percent_decode(&r["/api/files/local/".len()..]);
                let Some(b) = body() else {
                    return err_json(400, "bad json");
                };
                if !c.files.iter().any(|(n, _)| *n == name) {
                    return err_json(404, "no such file");
                }
                c.file = Some(name.clone());
                if b.get("print").and_then(Value::as_bool) == Some(true) {
                    return done(c.start(&name));
                }
                ok()
            }
            _ => err_json(404, "not found"),
        }
    }
}

impl HttpTransport for SimOctoPrint {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        if let Some(r) = faulted(&self.core, req, "X-Api-Key") {
            return r;
        }
        Ok(self.handle(req))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heaters_approach_their_targets_and_never_overshoot() {
        let mut c = SimCore::new();
        c.gcode("M104 S200");
        c.gcode("M140 S60");
        for _ in 0..200 {
            c.advance(1.0);
            assert!(c.nozzle <= 200.0 && c.bed <= 60.0);
        }
        assert_eq!((c.nozzle, c.bed), (200.0, 60.0));
        c.gcode("M104 S0");
        for _ in 0..100 {
            c.advance(1.0);
        }
        assert_eq!(c.nozzle, 22.0);
    }

    #[test]
    fn a_target_over_the_firmware_maximum_shuts_it_down() {
        let mut c = SimCore::new();
        c.gcode("M104 S400");
        assert_eq!(c.state, SimState::Shutdown);
        assert_eq!(c.max_nozzle_target_seen, 400.0);
        assert_eq!(c.nozzle_target, 0.0);
        assert!(c.start("a.gcode").is_err());
    }

    #[test]
    fn a_print_runs_pauses_and_completes() {
        let mut c = SimCore::new();
        c.store("a.gcode", vec![1]);
        c.total_s = 100.0;
        assert!(c.start("b.gcode").is_err());
        c.start("a.gcode").unwrap();
        assert!(c.start("a.gcode").is_err());
        c.advance(50.0);
        assert!((c.progress - 0.5).abs() < 1e-9);
        c.pause().unwrap();
        c.advance(50.0);
        assert!((c.progress - 0.5).abs() < 1e-9);
        c.resume().unwrap();
        c.advance(60.0);
        assert_eq!(c.state, SimState::Complete);
        assert!(c.pause().is_err() && c.resume().is_err() && c.cancel().is_err());
    }

    #[test]
    fn gcode_commands_set_the_fan_and_stop() {
        let mut c = SimCore::new();
        c.gcode("M106 S128 ; half");
        assert!((c.fan_pct - 50.2).abs() < 0.1);
        c.gcode("M107");
        assert_eq!(c.fan_pct, 0.0);
        c.gcode("SET_HEATER_TEMPERATURE HEATER=heater_bed TARGET=70");
        assert_eq!(c.bed_target, 70.0);
        c.gcode("M112");
        assert_eq!(c.state, SimState::Shutdown);
        assert!(c.gcode_log.len() >= 4);
    }

    #[test]
    fn faults_and_keys_come_before_the_routes() {
        let core = SimCore::shared();
        core.borrow_mut().api_key = "secret".into();
        core.borrow_mut().faults.push_back(Fault::Timeout);
        core.borrow_mut()
            .faults
            .push_back(Fault::Status(503, "busy".into()));
        let mut m = SimMoonraker::new(core.clone());
        let get = HttpRequest::new(Method::Get, "/printer/objects/query");
        assert_eq!(m.send(&get), Err(LinkError::Timeout));
        assert_eq!(m.send(&get).unwrap().status, 503);
        assert_eq!(m.send(&get).unwrap().status, 401);
        assert_eq!(
            m.send(&get.clone().header("X-Api-Key", "secret"))
                .unwrap()
                .status,
            200
        );
        assert_eq!(
            m.send(&HttpRequest::new(Method::Get, "/nowhere").header("X-Api-Key", "secret"))
                .unwrap()
                .status,
            404
        );
        assert_eq!(core.borrow().requests.len(), 5);
    }
}
