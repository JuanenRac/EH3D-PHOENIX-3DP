//! PHOENIX-3DP formats: the files a model comes in (STL, OBJ, basic 3MF) and the files the program writes (binary STL, OBJ, its own 3MF, G-code).
//! Every reader checks the size before it allocates and refuses with a reason what it does not understand; none of them panics on a damaged file.
#![forbid(unsafe_code)]

pub mod crc32;
pub mod detect;
pub mod error;
pub mod gcode_file;
pub mod inflate;
pub mod obj;
pub mod project3mf;
pub mod stl;
pub mod threemf;
pub mod xml;
pub mod zip;

pub use detect::{read_mesh, write_mesh, Format};
pub use error::FormatError;
