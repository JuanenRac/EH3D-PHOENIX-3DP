//! PHOENIX-3DP profiles: what is known about each printer model, written from public data (never copied from the profile files of another program), with the
//! flag that says whether anyone has tried it on a machine.
#![forbid(unsafe_code)]

pub mod catalog;
pub mod profile;

pub use catalog::Catalog;
pub use profile::{Connectivity, PrinterProfile, ProfileError, Technology};
