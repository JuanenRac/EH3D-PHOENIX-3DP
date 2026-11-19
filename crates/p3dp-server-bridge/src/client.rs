//! The client of the printer plugin of the server: the draft routes under `/api/v1/printer3d/`, with the token as a bearer, the retry rules of the suite and a limit on the
//! size of an answer. The routes are a draft that the server does not serve yet; the server in memory of this crate is a double of them.

use crate::settings::Settings;
use p3dp_core::json::{self, Value};
use p3dp_link::moonraker::check_status;
use p3dp_link::retry::{Retrying, Sleeper};
use p3dp_link::transport::{percent_encode, safe_file_name};
use p3dp_link::{HttpRequest, HttpResponse, HttpTransport, LinkError, Method};

/// The corner of a window: how the last request went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Unknown,
    Connected,
    /// The last request did not reach the server.
    Disconnected,
    /// The server answered that the token is not good.
    LoggedOut,
}

/// A printer as the server lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterInfo {
    pub id: String,
    pub name: String,
    pub state: String,
}

/// A job in the queue of the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub id: String,
    pub file: String,
    pub printer: String,
    pub state: String,
}

/// The client.
pub struct ServerClient<T: HttpTransport, S: Sleeper> {
    settings: Settings,
    transport: Retrying<T, S>,
    link: LinkState,
}

fn path_id(id: &str) -> Result<String, LinkError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(LinkError::Invalid(format!("{id:?} is not a valid id")));
    }
    Ok(percent_encode(id))
}

fn text_of(v: &Value, key: &str) -> Result<String, LinkError> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| LinkError::BadReply(format!("the answer has no {key}")))
}

impl<T: HttpTransport, S: Sleeper> ServerClient<T, S> {
    /// A client. Without an address nothing can be sent, and that is an error here, not a silent no-op.
    pub fn new(
        settings: Settings,
        inner: T,
        sleeper: S,
        seed: u64,
    ) -> Result<ServerClient<T, S>, LinkError> {
        if settings.server_address.is_empty() {
            return Err(LinkError::Invalid(
                "server_address is empty: there is no server to talk to".to_string(),
            ));
        }
        let retries = settings.server_retries;
        Ok(ServerClient {
            settings,
            transport: Retrying::new(inner, sleeper, retries, seed),
            link: LinkState::Unknown,
        })
    }

    pub fn link(&self) -> LinkState {
        self.link
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// How many tries the last request needed.
    pub fn last_attempts(&self) -> u32 {
        self.transport.last_attempts
    }

    fn request(
        &mut self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        with_token: bool,
    ) -> Result<HttpResponse, LinkError> {
        let mut req = HttpRequest::new(method, path).header("Accept", "application/json");
        if let Some(b) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(b.to_json().into_bytes());
        }
        if with_token && !self.settings.server_token.is_empty() {
            req = req.header(
                "Authorization",
                &format!("Bearer {}", self.settings.server_token.expose()),
            );
        }
        let result = self.transport.send(&req);
        self.link = match &result {
            Err(LinkError::NoConnection | LinkError::Timeout) => LinkState::Disconnected,
            Ok(r) if r.status == 401 => LinkState::LoggedOut,
            Ok(r) if matches!(r.status, 502..=504) => LinkState::Disconnected,
            Ok(_) => LinkState::Connected,
            Err(_) => self.link,
        };
        let r = check_status(result?)?;
        if r.body.len() as u64 > u64::from(self.settings.server_max_answer_mib) << 20 {
            return Err(LinkError::BadReply(format!(
                "the answer is {} bytes, more than server_max_answer_mib",
                r.body.len()
            )));
        }
        Ok(r)
    }

    fn json(
        &mut self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, LinkError> {
        let r = self.request(method, path, body, true)?;
        json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))
    }

    /// Asks whether the server is there. No token is sent.
    pub fn health(&mut self) -> Result<bool, LinkError> {
        let r = self.request(Method::Get, "/api/v1/health", None, false)?;
        let v = json::parse(&r.text()).map_err(|e| LinkError::BadReply(e.to_string()))?;
        Ok(v.get("status").and_then(Value::as_str) == Some("ok"))
    }

    /// The printers of the server.
    pub fn printers(&mut self) -> Result<Vec<PrinterInfo>, LinkError> {
        let v = self.json(Method::Get, "/api/v1/printer3d/printers", None)?;
        let list = v
            .get("printers")
            .and_then(Value::as_array)
            .ok_or_else(|| LinkError::BadReply("no printers list".to_string()))?;
        list.iter()
            .map(|p| {
                Ok(PrinterInfo {
                    id: text_of(p, "id")?,
                    name: text_of(p, "name")?,
                    state: text_of(p, "state")?,
                })
            })
            .collect()
    }

    /// The normalized state key of one printer.
    pub fn printer_state(&mut self, id: &str) -> Result<String, LinkError> {
        let v = self.json(
            Method::Get,
            &format!("/api/v1/printer3d/printers/{}/state", path_id(id)?),
            None,
        )?;
        text_of(&v, "state")
    }

    /// The queue.
    pub fn queue(&mut self) -> Result<Vec<QueueItem>, LinkError> {
        let v = self.json(Method::Get, "/api/v1/printer3d/queue", None)?;
        let list = v
            .get("jobs")
            .and_then(Value::as_array)
            .ok_or_else(|| LinkError::BadReply("no jobs list".to_string()))?;
        list.iter().map(item_of).collect()
    }

    /// Puts a job in the queue. The job never starts by itself: the request always says `auto_start: false` and asks for the confirmation of a person; whether the
    /// server may start jobs on its own is the server's policy, and the program does not ask for it.
    pub fn enqueue(&mut self, file: &str, printer: Option<&str>) -> Result<QueueItem, LinkError> {
        safe_file_name(file)?;
        let mut pairs = vec![
            ("file", Value::Str(file.to_string())),
            ("auto_start", Value::Bool(false)),
            ("confirm_before_start", Value::Bool(true)),
        ];
        if let Some(p) = printer {
            path_id(p)?;
            pairs.push(("printer", Value::Str(p.to_string())));
        }
        let v = self.json(
            Method::Post,
            "/api/v1/printer3d/queue",
            Some(&Value::object(pairs)),
        )?;
        item_of(&v)
    }
}

fn item_of(v: &Value) -> Result<QueueItem, LinkError> {
    Ok(QueueItem {
        id: text_of(v, "id")?,
        file: text_of(v, "file")?,
        printer: v
            .get("printer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        state: text_of(v, "state")?,
    })
}
