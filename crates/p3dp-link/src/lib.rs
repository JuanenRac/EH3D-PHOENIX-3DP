//! PHOENIX-3DP link: how the program talks to a printer. A driver turns the normalized commands into the requests of one protocol and the answers into one normalized
//! state. The protocol logic runs over a `HttpTransport`; the tests give it an in-memory simulator of the server instead of a network, so nothing here has spoken to a real printer.
#![forbid(unsafe_code)]

pub mod backoff;
pub mod bambu;
pub mod bambu_lan;
pub mod driver;
pub mod error;
pub mod ftps;
pub mod moonraker;
pub mod mqtt;
pub mod octoprint;
pub mod pin;
pub mod retry;
pub mod sim;
pub mod state_map;
pub mod tcp;
pub mod tls;
pub mod transport;
pub mod types;

pub use driver::PrinterDriver;
pub use error::LinkError;
pub use tcp::{TcpLimits, TcpTransport};
pub use transport::{HttpRequest, HttpResponse, HttpTransport, Method};
pub use types::{Caps, Command, Family, JobInfo, JobState, PrinterState, Temperature};
