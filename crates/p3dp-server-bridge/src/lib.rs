//! PHOENIX-3DP server bridge: the program is one more client of the EH3D server, which holds the printers, the queue and the permissions. This crate is the thin layer
//! that keeps the settings of that link and speaks the draft routes of the printer plugin of the server over a transport. It has no network of its own: a transport is given
//! to it, and the tests give it a server in memory. When the common client of the suite is taken in, this layer becomes a call to it.
#![forbid(unsafe_code)]

pub mod client;
pub mod settings;
pub mod sim;

pub use client::{LinkState, ServerClient};
pub use settings::{Connection, Settings};
