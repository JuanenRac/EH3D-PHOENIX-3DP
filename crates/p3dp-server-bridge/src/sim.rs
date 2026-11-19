//! The server in memory: a double of the draft routes of the printer plugin, so that the client is tested without a network. It is not the server.

use p3dp_core::json::{self, Value};
use p3dp_link::sim::Fault;
use p3dp_link::{HttpRequest, HttpResponse, HttpTransport, LinkError, Method};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The token that the simulated server accepts.
pub const TOKEN: &str = "program-token";

/// What the simulated server holds.
#[derive(Debug, Default)]
pub struct State {
    pub printers: Vec<(String, String, String)>,
    /// `(id, file, printer, state)`
    pub queue: Vec<(String, String, String, String)>,
    pub faults: VecDeque<Fault>,
    /// The bodies of the requests that were posted.
    pub posted: Vec<String>,
    /// The headers `Authorization` that arrived, as they arrived (the tests look for the token that must not be sent).
    pub auth_seen: Vec<Option<String>>,
    /// Whether the server's policy lets a job start by itself. Off by default, like the real one.
    pub auto_start_allowed: bool,
    pub requests: usize,
}

/// A handle to the simulated server; clones share the state.
#[derive(Debug, Clone)]
pub struct SimBridgeServer {
    pub state: Rc<RefCell<State>>,
}

impl Default for SimBridgeServer {
    fn default() -> SimBridgeServer {
        SimBridgeServer::new()
    }
}

fn reply(status: u16, v: &Value) -> HttpResponse {
    HttpResponse::json(status, &v.to_json())
}

fn error(status: u16, code: &str) -> HttpResponse {
    reply(
        status,
        &Value::object(vec![("error", Value::Str(code.to_string()))]),
    )
}

impl SimBridgeServer {
    pub fn new() -> SimBridgeServer {
        let s = State {
            printers: vec![
                (
                    "p1s-1".to_string(),
                    "Bambu P1S".to_string(),
                    "idle".to_string(),
                ),
                (
                    "mk4-1".to_string(),
                    "Prusa MK4S".to_string(),
                    "printing".to_string(),
                ),
            ],
            ..State::default()
        };
        SimBridgeServer {
            state: Rc::new(RefCell::new(s)),
        }
    }

    fn route(&self, req: &HttpRequest) -> HttpResponse {
        let mut s = self.state.borrow_mut();
        let route = req.route().to_string();
        if (req.method, route.as_str()) == (Method::Get, "/api/v1/health") {
            return reply(
                200,
                &Value::object(vec![("status", Value::Str("ok".to_string()))]),
            );
        }
        if req.header_value("Authorization") != Some(&format!("Bearer {TOKEN}")) {
            return error(401, "unauthorized");
        }
        match (req.method, route.as_str()) {
            (Method::Get, "/api/v1/printer3d/printers") => {
                let list: Vec<Value> = s
                    .printers
                    .iter()
                    .map(|(id, name, st)| {
                        Value::object(vec![
                            ("id", Value::Str(id.clone())),
                            ("name", Value::Str(name.clone())),
                            ("state", Value::Str(st.clone())),
                        ])
                    })
                    .collect();
                reply(200, &Value::object(vec![("printers", Value::Arr(list))]))
            }
            (Method::Get, r)
                if r.starts_with("/api/v1/printer3d/printers/") && r.ends_with("/state") =>
            {
                let id = &r["/api/v1/printer3d/printers/".len()..r.len() - "/state".len()];
                match s.printers.iter().find(|p| p.0 == id) {
                    Some(p) => reply(
                        200,
                        &Value::object(vec![
                            ("id", Value::Str(p.0.clone())),
                            ("state", Value::Str(p.2.clone())),
                        ]),
                    ),
                    None => error(404, "printer_not_found"),
                }
            }
            (Method::Get, "/api/v1/printer3d/queue") => {
                let list: Vec<Value> = s.queue.iter().map(job_json).collect();
                reply(200, &Value::object(vec![("jobs", Value::Arr(list))]))
            }
            (Method::Post, "/api/v1/printer3d/queue") => {
                let text = String::from_utf8_lossy(&req.body).into_owned();
                s.posted.push(text.clone());
                let Ok(v) = json::parse(&text) else {
                    return error(400, "bad_json");
                };
                let Some(file) = v.get("file").and_then(Value::as_str) else {
                    return error(400, "file_needed");
                };
                if v.get("auto_start").and_then(Value::as_bool) == Some(true)
                    && !s.auto_start_allowed
                {
                    return error(403, "auto_start_disabled");
                }
                let printer = v
                    .get("printer")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !printer.is_empty() && !s.printers.iter().any(|p| p.0 == printer) {
                    return error(404, "printer_not_found");
                }
                let item = (
                    format!("job-{}", s.queue.len() + 1),
                    file.to_string(),
                    printer,
                    "queued".to_string(),
                );
                let out = job_json(&item);
                s.queue.push(item);
                reply(201, &out)
            }
            _ => error(404, "not_found"),
        }
    }
}

fn job_json(j: &(String, String, String, String)) -> Value {
    Value::object(vec![
        ("id", Value::Str(j.0.clone())),
        ("file", Value::Str(j.1.clone())),
        ("printer", Value::Str(j.2.clone())),
        ("state", Value::Str(j.3.clone())),
    ])
}

impl HttpTransport for SimBridgeServer {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        {
            let mut s = self.state.borrow_mut();
            s.requests += 1;
            s.auth_seen
                .push(req.header_value("Authorization").map(str::to_string));
            match s.faults.pop_front() {
                Some(Fault::NoConnection) => return Err(LinkError::NoConnection),
                Some(Fault::Timeout) => return Err(LinkError::Timeout),
                Some(Fault::Status(code, msg)) => return Ok(error(code, &msg)),
                None => {}
            }
        }
        Ok(self.route(req))
    }
}
