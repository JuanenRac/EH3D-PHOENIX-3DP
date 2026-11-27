//! PHOENIX-3DP: a program to prepare and send 3D prints. This library holds everything the command line and the window use; the window only draws what is decided here.
//! Everything has been tried against simulators only: no printer has been connected.
#![forbid(unsafe_code)]

pub mod app_state;
pub mod cli;
pub mod config;
pub mod demo;
pub mod device;
pub mod eh3dcli;
pub mod eh3dfile;
pub mod held;
pub mod i18n;
pub mod import;
pub mod keys;
pub mod limits_ui;
pub mod lists;
pub mod look;
pub mod machine_view;
pub mod objects;
pub mod package;
pub mod panels;
pub mod preflight;
pub mod preview;
pub mod project;
pub mod report;
pub mod server_ui;
pub mod shade;
pub mod shot;
pub mod slice_job;
pub mod studio;
pub mod view;

// what the window shows and asks about an `.eh3d` project, and its words, are shared by the programs (crate `eh3d-project` of EH3D-COMMON)
pub use eh3d_project::suitetext;
pub use eh3d_project::suiteview as suitecore;

#[cfg(feature = "gui")]
pub mod gui;

/// The version of the program.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
