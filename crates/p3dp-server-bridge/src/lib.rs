//! PHOENIX-3DP server bridge: the program is one more client of the EH3D server, which holds the printers, the queue and the permissions. This crate keeps the settings of that
//! link and speaks the routes of the `printer3d` plugin of the server (`/api/v1/work/...`) over the one client of the suite (`eh3d-serverclient` of EH3D-COMMON): the same login,
//! the same renewal, the same retries (a `POST` is never sent twice), the same state of the link and the same refusals of the server in eight languages. The tests give the client a
//! server in memory and never a network.
#![forbid(unsafe_code)]

pub mod client;
pub mod queue;
pub mod settings;
pub mod sim;

pub use client::{
    normalize_fingerprint, server_model_for, stricter, BambuProblem, BridgeError, CompatShape,
    Confirmation, HeaterInfo, JobInfo, LayerInfo, LinkState, MachineConnection, PrinterInfo,
    ServerClient, SwitchRead, APP,
};
pub use queue::{board, check_enqueue, MachineQueue, QueuePolicy};
pub use settings::{Connection, Settings};
