//! PHOENIX-3DP slicer: a minimal slicer written from the geometry of the problem. It cuts a closed mesh in flat layers, lays perimeters and a rectilinear infill with solid skins
//! on the layers of the top and the bottom, and writes G-code for the dialects of the program through its writer. The plan of the paths per layer is data (a preview that needs no
//! window), and it is what the G-code is made from.
//!
//! Every limit is a refusal: a temperature over what the model allows, a model that does not fit the bed, a mesh that is not closed, a setting out of its range, a G-code line
//! that would leave the bed, are all errors that name the cause, and nothing is cut down to make them pass. There is no variable line width, seam control, supports, bridges or
//! multi-material yet; the document of the roadmap says so.
#![forbid(unsafe_code)]

pub mod config;
pub mod cut;
pub mod emit;
pub mod error;
pub mod geom;
pub mod plan;

pub use config::{Placement, SliceConfig, ThinRegions};
pub use emit::{LayerSummary, Move};
pub use error::SliceError;
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
