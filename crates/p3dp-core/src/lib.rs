//! PHOENIX-3DP core: units in nanometres, the mesh and its checks, the limits that can be configured, a small JSON reader, a secret that never prints itself and the machine
//! types (dialect and temperature limits) that every other crate shares. Plain safe Rust with no dependency.
#![forbid(unsafe_code)]

pub mod error;
pub mod json;
pub mod kv;
pub mod limits;
pub mod machine;
pub mod mesh;
pub mod rng;
pub mod secret;
pub mod units;

pub use error::CoreError;
pub use limits::Limits;
pub use machine::{Dialect, TempLimits};
pub use mesh::{Mesh, Point3};
pub use secret::Secret;
pub use units::Nm;
