//! The OctoPrint driver: the REST interface, written from its public documentation. It works over any `HttpTransport`.

use crate::driver::{guard, PrinterDriver};
use crate::error::LinkError;
use crate::moonraker::check_status;
use crate::state_map::{self, OctoFlags};
use crate::transport::{
    multipart, percent_encode, safe_file_name, HttpRequest, HttpResponse, HttpTransport, Method,
    Part,
};
use crate::types::{Caps, Command, Family, JobInfo, PrinterState, Temperature};
use p3dp_core::json::{self, Value};
use p3dp_core::machine::Zone;
use p3dp_core::{Secret, TempLimits};

/// What the driver can do over OctoPrint.
pub const CAPS: u32 = Caps::PAUSE
    | Caps::RESUME
    | Caps::CANCEL
    | Caps::EMERGENCY_STOP
    | Caps::GCODE
    | Caps::UPLOAD
    | Caps::FILE_LIST
    | Caps::SET_NOZZLE
    | Caps::SET_BED
    | Caps::SET_CHAMBER
    | Caps::START
    | Caps::FAN;

/// The OctoPrint driver.
pub struct OctoPrintDriver<T: HttpTransport> {
    transport: T,
    key: Secret,
    limits: TempLimits,
    allow_raw_gcode: bool,
}

impl<T: HttpTransport> OctoPrintDriver<T> {
    pub fn new(
        transport: T,
        key: Secret,
        limits: TempLimits,
        allow_raw_gcode: bool,
    ) -> OctoPrintDriver<T> {
        OctoPrintDriver {
            transport,
            key,
            limits,
            allow_raw_gcode,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn call_raw(
        &mut self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<HttpResponse, LinkError> {
        let mut req = HttpRequest::new(method, path).body(body);
        if !self.key.is_empty() {
            req = req.header("X-Api-Key", self.key.expose());
        }
        if !content_type.is_empty() {
            req = req.header("Content-Type", content_type);
        }
        self.transport.send(&req)
    }

    fn call(
        &mut self,
        method: Method,
        path: &str,
        body: &Value,
    ) -> Result<HttpResponse, LinkError> {
        let bytes = if matches!(body, Value::Null) {
            Vec::new()
        } else {
            body.to_json().into_bytes()
        };
        let r = self.call_raw(method, path, bytes, "application/json")?;
        check_status(r)
    }

    fn gcode(&mut self, line: &str) -> Result<(), LinkError> {
        self.call(
            Method::Post,
            "/api/printer/command",
            &Value::object(vec![("command", Value::Str(line.to_string()))]),
        )
        .map(|_| ())
    }
}

fn flag(flags: &Value, key: &str) -> bool {
    flags.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn temp(v: &Value, key: &str, zone: Zone) -> Option<Temperature> {
    let t = v.path(&["temperature", key])?;
    let actual = t.get("actual").and_then(Value::as_f64)?;
    let target = t.get("target").and_then(Value::as_f64).unwrap_or(0.0);
    Some(Temperature {
        zone,
        actual,
        target,
    })
}

impl<T: HttpTransport> PrinterDriver for OctoPrintDriver<T> {
    fn family(&self) -> Family {
        Family::OctoPrint
    }

    fn capabilities(&self) -> Caps {
        Caps(CAPS)
    }

    fn status(&mut self) -> Result<PrinterState, LinkError> {
        let r = match self.call_raw(Method::Get, "/api/printer", Vec::new(), "") {
            Ok(r) => r,
            Err(LinkError::NoConnection | LinkError::Timeout) => {
                return Ok(PrinterState::offline(Family::OctoPrint))
            }
            Err(e) => return Err(e),
        };
        if r.status == 409 {
            // OctoPrint is there, but the printer is not connected to it
            return Ok(PrinterState {
                alerts: vec!["the printer is not connected to OctoPrint".to_string()],
                ..PrinterState::offline(Family::OctoPrint)
            });
        }
        let r = check_status(r)?;
        let v = json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
        let flags = v
            .path(&["state", "flags"])
            .ok_or_else(|| LinkError::BadReply("no state.flags".to_string()))?;
        let f = OctoFlags {
            operational: flag(flags, "operational"),
            printing: flag(flags, "printing"),
            paused: flag(flags, "paused"),
            pausing: flag(flags, "pausing"),
            cancelling: flag(flags, "cancelling"),
            error: flag(flags, "error"),
            closed_or_error: flag(flags, "closedOrError"),
        };
        let mut temperatures = Vec::new();
        temperatures.extend(temp(&v, "tool0", Zone::Nozzle));
        temperatures.extend(temp(&v, "bed", Zone::Bed));
        temperatures.extend(temp(&v, "chamber", Zone::Chamber));
        let job = if f.printing || f.paused {
            let j = self.call(Method::Get, "/api/job", &Value::Null)?;
            let jv = json::parse(&j.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
            let file = jv
                .path(&["job", "file", "name"])
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let p = jv.get("progress");
            Some(JobInfo {
                file,
                progress_pct: p
                    .and_then(|p| p.get("completion"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .clamp(0.0, 100.0),
                elapsed_s: p
                    .and_then(|p| p.get("printTime"))
                    .and_then(Value::as_f64)
                    .filter(|v| *v >= 0.0)
                    .map(|v| v as u64),
                remaining_s: p
                    .and_then(|p| p.get("printTimeLeft"))
                    .and_then(Value::as_f64)
                    .filter(|v| *v >= 0.0)
                    .map(|v| v as u64),
            })
        } else {
            None
        };
        let raw = v
            .path(&["state", "text"])
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(PrinterState {
            family: Family::OctoPrint,
            state: state_map::octoprint(&f),
            raw_state: raw,
            temperatures,
            job,
            alerts: Vec::new(),
        })
    }

    fn command(&mut self, command: &Command) -> Result<(), LinkError> {
        guard(
            self.capabilities(),
            command,
            &self.limits,
            self.allow_raw_gcode,
        )?;
        let obj = |pairs: Vec<(&str, Value)>| Value::object(pairs);
        let s = |t: &str| Value::Str(t.to_string());
        match command {
            Command::Start { file } => self
                .call(
                    Method::Post,
                    &format!("/api/files/local/{}", percent_encode(file)),
                    &obj(vec![("command", s("select")), ("print", Value::Bool(true))]),
                )
                .map(|_| ()),
            Command::Pause => self
                .call(
                    Method::Post,
                    "/api/job",
                    &obj(vec![("command", s("pause")), ("action", s("pause"))]),
                )
                .map(|_| ()),
            Command::Resume => self
                .call(
                    Method::Post,
                    "/api/job",
                    &obj(vec![("command", s("pause")), ("action", s("resume"))]),
                )
                .map(|_| ()),
            Command::Cancel => self
                .call(
                    Method::Post,
                    "/api/job",
                    &obj(vec![("command", s("cancel"))]),
                )
                .map(|_| ()),
            Command::EmergencyStop => self.gcode("M112"),
            Command::SetTemp {
                zone: Zone::Nozzle,
                target_c,
            } => self
                .call(
                    Method::Post,
                    "/api/printer/tool",
                    &obj(vec![
                        ("command", s("target")),
                        ("targets", obj(vec![("tool0", Value::Num(*target_c))])),
                    ]),
                )
                .map(|_| ()),
            Command::SetTemp {
                zone: Zone::Bed,
                target_c,
            } => self
                .call(
                    Method::Post,
                    "/api/printer/bed",
                    &obj(vec![
                        ("command", s("target")),
                        ("target", Value::Num(*target_c)),
                    ]),
                )
                .map(|_| ()),
            Command::SetTemp {
                zone: Zone::Chamber,
                target_c,
            } => self
                .call(
                    Method::Post,
                    "/api/printer/chamber",
                    &obj(vec![
                        ("command", s("target")),
                        ("target", Value::Num(*target_c)),
                    ]),
                )
                .map(|_| ()),
            Command::SetFan { percent } => self.gcode(&format!(
                "M106 S{}",
                (percent * 255.0 / 100.0).round() as u32
            )),
            Command::Gcode { line } => self.gcode(line),
        }
    }

    fn upload(&mut self, name: &str, data: &[u8], start: bool) -> Result<(), LinkError> {
        safe_file_name(name)?;
        let boundary = "p3dpOctoPrintBoundary5c21";
        let parts = [
            Part {
                name: "select".to_string(),
                filename: None,
                data: if start {
                    b"true".to_vec()
                } else {
                    b"false".to_vec()
                },
            },
            Part {
                name: "print".to_string(),
                filename: None,
                data: if start {
                    b"true".to_vec()
                } else {
                    b"false".to_vec()
                },
            },
            Part {
                name: "file".to_string(),
                filename: Some(name.to_string()),
                data: data.to_vec(),
            },
        ];
        let body = multipart(boundary, &parts)?;
        let r = self.call_raw(
            Method::Post,
            "/api/files/local",
            body,
            &format!("multipart/form-data; boundary={boundary}"),
        )?;
        check_status(r).map(|_| ())
    }

    fn list_files(&mut self) -> Result<Vec<String>, LinkError> {
        let r = self.call(Method::Get, "/api/files/local", &Value::Null)?;
        let v = json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
        let list = v
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| LinkError::BadReply("no files list".to_string()))?;
        Ok(list
            .iter()
            .filter_map(|f| f.get("name").and_then(Value::as_str).map(str::to_string))
            .collect())
    }
}
