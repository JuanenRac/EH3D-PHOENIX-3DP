//! PHOENIX-3DP slicer: a minimal slicer written from the geometry of the problem. It cuts a closed mesh in flat layers, lays perimeters and a rectilinear infill with solid skins
//! on the layers of the top and the bottom, and writes G-code for the dialects of the program through its writer. The plan of the paths per layer is data (a preview that needs no
//! window), and it is what the G-code is made from.
//!
//! Every limit is a refusal: a temperature over what the model allows, a model that does not fit the bed, a mesh that is not closed, a setting out of its range, a G-code line
//! that would leave the bed, are all errors that name the cause, and nothing is cut down to make them pass. The infill may be rectilinear, grid or gyroid, the perimeters may
//! be narrower where a part is thin, the seams may be put at the rear or lined up, a skin over nothing is a bridge, the overhangs may get plain supports, and the head may lift on
//! a travel that retracts; the time is estimated with the acceleration of the machine. A print may use several materials, divided by height, with a purge tower (the module `multi`); painting a colour on a surface is not done.
#![forbid(unsafe_code)]

pub mod adaptive;
pub mod adhesion;
pub mod config;
pub mod cut;
pub mod emit;
pub mod error;
pub mod geom;
pub mod infill;
pub mod machine_check;
pub mod multi;
pub mod plan;
pub mod report;
pub mod support;
pub mod timing;

pub use config::{Adhesion, InfillPattern, Placement, Seam, SliceConfig, Supports, ThinRegions};
pub use emit::{LayerSummary, Move, TimeCheck, UserGcode};
pub use error::SliceError;
pub use multi::{MultiConfig, MultiJob, MultiReport, Tool, ToolChange};
pub use plan::{plan, Kind, Layer, Machine, Path, Plan, Warning};

/// The version of the slicer, written in the header of the G-code.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Slices a mesh for a machine and gives the plan and the G-code.
pub fn slice(
    mesh: &p3dp_core::Mesh,
    machine: &Machine,
    config: &SliceConfig,
) -> Result<(Plan, String), SliceError> {
    let p = plan(mesh, machine, config)?;
    let g = p.gcode(VERSION)?;
    Ok((p, g))
}
