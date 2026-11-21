//! PHOENIX-3DP profiles: what is known about each printer model, written from public data (never copied from the profile files of another program), with the
//! flag that says whether anyone has tried it on a machine.
#![forbid(unsafe_code)]

pub mod catalog;
pub mod filament;
pub mod multi;
pub mod profile;
pub mod resin;

pub use catalog::Catalog;
pub use filament::{FilamentCatalog, FilamentProfile, Material, Nozzle};
pub use multi::{caps_of, MultiCaps, MultiKind};
pub use profile::{
    Change, Connectivity, Edited, LimitSource, Notice, PrinterProfile, ProfileError, Technology,
};
pub use resin::{ResinCatalog, ResinKind, ResinProfile, ResinUse, Wash};
