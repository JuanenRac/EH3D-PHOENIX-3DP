//! The Moonraker driver (the server of Klipper): the HTTP interface, written from its public documentation. It works over any `HttpTransport`.
//! The WebSocket push is a later step; here the state is read by asking for it.

use crate::driver::{guard, PrinterDriver};
use crate::error::LinkError;
use crate::state_map;
use crate::transport::{
    multipart, percent_encode, safe_file_name, HttpRequest, HttpResponse, HttpTransport, Method,
    Part,
};
use crate::types::{Caps, Command, Family, JobInfo, PrinterState, Temperature};
use p3dp_core::json::{self, Value};
use p3dp_core::machine::Zone;
use p3dp_core::{Secret, TempLimits};

/// What the driver can do over Moonraker (a chamber heater is not named in a way that is the same on every printer, so it is not offered).
pub const CAPS: u32 = Caps::PAUSE
    | Caps::RESUME
    | Caps::CANCEL
    | Caps::EMERGENCY_STOP
    | Caps::GCODE
    | Caps::UPLOAD
    | Caps::FILE_LIST
    | Caps::SET_NOZZLE
    | Caps::SET_BED
    | Caps::START
    | Caps::FAN;

/// The Moonraker driver.
pub struct MoonrakerDriver<T: HttpTransport> {
    transport: T,
    key: Secret,
    limits: TempLimits,
    allow_raw_gcode: bool,
}

/// Turns an answer into a value or an error: 401 is `Auth`, 403 is `Forbidden`, any other error status is `Http` with what the server said.
pub fn check_status(r: HttpResponse) -> Result<HttpResponse, LinkError> {
    match r.status {
        200..=299 => Ok(r),
        401 => Err(LinkError::Auth),
        403 => Err(LinkError::Forbidden),
        status => {
            let text = r.text();
            let detail = json::parse(&text)
                .ok()
                .and_then(|v| {
                    v.get("error").map(|e| {
                        e.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| e.to_json())
                    })
                })
                .unwrap_or(text);
            Err(LinkError::Http {
                status,
                detail: detail.chars().take(200).collect(),
            })
        }
    }
}

impl<T: HttpTransport> MoonrakerDriver<T> {
    /// A driver. `allow_raw_gcode` is the switch of raw G-code (off by default in the program).
    pub fn new(
        transport: T,
        key: Secret,
        limits: TempLimits,
        allow_raw_gcode: bool,
    ) -> MoonrakerDriver<T> {
        MoonrakerDriver {
            transport,
            key,
            limits,
            allow_raw_gcode,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn call(
        &mut self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<HttpResponse, LinkError> {
        let mut req = HttpRequest::new(method, path).body(body);
        if !self.key.is_empty() {
            req = req.header("X-Api-Key", self.key.expose());
        }
        if let Some(ct) = content_type {
            req = req.header("Content-Type", ct);
        }
        check_status(self.transport.send(&req)?)
    }

    fn script(&mut self, script: &str) -> Result<(), LinkError> {
        self.call(
            Method::Post,
            &format!("/printer/gcode/script?script={}", percent_encode(script)),
            Vec::new(),
            None,
        )
        .map(|_| ())
    }
}

fn read_temp(status: &Value, object: &str, zone: Zone) -> Option<Temperature> {
    let o = status.get(object)?;
    let actual = o.get("temperature")?.as_f64()?;
    let target = o.get("target").and_then(Value::as_f64).unwrap_or(0.0);
    Some(Temperature {
        zone,
        actual,
        target,
    })
}

impl<T: HttpTransport> PrinterDriver for MoonrakerDriver<T> {
    fn family(&self) -> Family {
        Family::Moonraker
    }

    fn capabilities(&self) -> Caps {
        Caps(CAPS)
    }

    fn status(&mut self) -> Result<PrinterState, LinkError> {
        let r = match self.call(
            Method::Get,
            "/printer/objects/query?print_stats&extruder&heater_bed&virtual_sdcard&webhooks",
            Vec::new(),
            None,
        ) {
            Ok(r) => r,
            Err(LinkError::NoConnection | LinkError::Timeout) => {
                return Ok(PrinterState::offline(Family::Moonraker))
            }
            Err(e) => return Err(e),
        };
        let v = json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
        let status = v
            .path(&["result", "status"])
            .ok_or_else(|| LinkError::BadReply("no result.status".to_string()))?;
        let ps = status.get("print_stats");
        let print_state = ps
            .and_then(|p| p.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let web = status.get("webhooks");
        let web_state = web
            .and_then(|w| w.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("ready");
        let state = state_map::moonraker(print_state, web_state);
        let mut temperatures = Vec::new();
        temperatures.extend(read_temp(status, "extruder", Zone::Nozzle));
        temperatures.extend(read_temp(status, "heater_bed", Zone::Bed));
        let file = ps
            .and_then(|p| p.get("filename"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let progress = status
            .path(&["virtual_sdcard", "progress"])
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let elapsed = ps
            .and_then(|p| p.get("print_duration"))
            .and_then(Value::as_f64)
            .filter(|d| *d >= 0.0);
        let job = (!file.is_empty()).then(|| JobInfo {
            file: file.to_string(),
            progress_pct: progress * 100.0,
            elapsed_s: elapsed.map(|d| d as u64),
            remaining_s: elapsed
                .filter(|_| progress > 0.0)
                .map(|d| (d * (1.0 - progress) / progress) as u64),
        });
        let mut alerts = Vec::new();
        if let Some(m) = web
            .and_then(|w| w.get("state_message"))
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
        {
            alerts.push(m.chars().take(200).collect());
        }
        Ok(PrinterState {
            family: Family::Moonraker,
            state,
            raw_state: print_state.to_string(),
            temperatures,
            job,
            alerts,
        })
    }

    fn command(&mut self, command: &Command) -> Result<(), LinkError> {
        guard(
            self.capabilities(),
            command,
            &self.limits,
            self.allow_raw_gcode,
        )?;
        match command {
            Command::Start { file } => self
                .call(
                    Method::Post,
                    &format!("/printer/print/start?filename={}", percent_encode(file)),
                    Vec::new(),
                    None,
                )
                .map(|_| ()),
            Command::Pause => self
                .call(Method::Post, "/printer/print/pause", Vec::new(), None)
                .map(|_| ()),
            Command::Resume => self
                .call(Method::Post, "/printer/print/resume", Vec::new(), None)
                .map(|_| ()),
            Command::Cancel => self
                .call(Method::Post, "/printer/print/cancel", Vec::new(), None)
                .map(|_| ()),
            Command::EmergencyStop => self
                .call(Method::Post, "/printer/emergency_stop", Vec::new(), None)
                .map(|_| ()),
            Command::SetTemp {
                zone: Zone::Nozzle,
                target_c,
            } => self.script(&format!(
                "SET_HEATER_TEMPERATURE HEATER=extruder TARGET={}",
                p3dp_gcode::writer::num(*target_c)
            )),
            Command::SetTemp {
                zone: Zone::Bed,
                target_c,
            } => self.script(&format!(
                "SET_HEATER_TEMPERATURE HEATER=heater_bed TARGET={}",
                p3dp_gcode::writer::num(*target_c)
            )),
            Command::SetTemp { .. } => Err(LinkError::Capability("set the chamber temperature")),
            Command::SetFan { percent } => self.script(&format!(
                "M106 S{}",
                (percent * 255.0 / 100.0).round() as u32
            )),
            Command::Gcode { line } => self.script(line),
        }
    }

    fn upload(&mut self, name: &str, data: &[u8], start: bool) -> Result<(), LinkError> {
        safe_file_name(name)?;
        let boundary = "p3dpMoonrakerBoundary7f3a";
        let parts = [
            Part {
                name: "root".to_string(),
                filename: None,
                data: b"gcodes".to_vec(),
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
        self.call(
            Method::Post,
            "/server/files/upload",
            body,
            Some(&format!("multipart/form-data; boundary={boundary}")),
        )
        .map(|_| ())
    }

    fn list_files(&mut self) -> Result<Vec<String>, LinkError> {
        let r = self.call(
            Method::Get,
            "/server/files/list?root=gcodes",
            Vec::new(),
            None,
        )?;
        let v = json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
        let list = v
            .get("result")
            .and_then(Value::as_array)
            .ok_or_else(|| LinkError::BadReply("no result list".to_string()))?;
        Ok(list
            .iter()
            .filter_map(|f| f.get("path").and_then(Value::as_str).map(str::to_string))
            .collect())
    }
}
