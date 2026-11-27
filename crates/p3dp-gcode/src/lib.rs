//! PHOENIX-3DP G-code: the writers of each dialect and the checker of a program against the limits of a model.
//! A temperature or a move that is not allowed is an error and nothing is written for it: a value is never cut down to a limit.
#![forbid(unsafe_code)]

pub mod check;
pub mod motion;
pub mod template;
pub mod writer;

pub use check::{check_program, risky_commands, Risk, RiskKind, Violation};
pub use motion::{read_motion, Motion, MotionLayer, Outside};
pub use template::{check_block, BlockJob, Place};
pub use writer::{required_macros, requires_package, GcodeError, JobParams, Writer};
