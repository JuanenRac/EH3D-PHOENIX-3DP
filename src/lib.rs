//! PHOENIX-3DP: a program to prepare and send 3D prints. This library holds everything the command line and the window use; the window only draws what is decided here.
//! Everything has been tried against simulators only: no printer has been connected.
#![forbid(unsafe_code)]

pub mod app_state;
pub mod cli;
pub mod config;
pub mod demo;
pub mod device;
pub mod i18n;
pub mod keys;
pub mod limits_ui;
pub mod lists;
pub mod look;
pub mod objects;
pub mod preview;
pub mod project;
pub mod report;
pub mod server_ui;
pub mod shade;
pub mod shot;
pub mod slice_job;
pub mod studio;
pub mod view;

#[cfg(feature = "gui")]
pub mod gui;

/// The version of the program.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
