//! The errors of the core: each one names the value and the reason.

use std::fmt;

/// What can go wrong in the core.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    /// A number that is not finite (NaN or infinity) where a measure was needed.
    NotFinite(&'static str),
    /// A number outside the range that is allowed.
    OutOfRange {
        what: String,
        value: f64,
        min: f64,
        max: f64,
    },
    /// More items than the limit allows.
    TooMany {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// A mesh or a list with nothing in it.
    Empty(&'static str),
    /// Anything else that makes no sense, with the reason.
    Invalid(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::NotFinite(what) => write!(f, "{what} is not a finite number"),
            CoreError::OutOfRange {
                what,
                value,
                min,
                max,
            } => write!(f, "{what} is {value}, outside {min} to {max}"),
            CoreError::TooMany { what, count, max } => {
                write!(f, "{count} {what}, more than the limit of {max}")
            }
            CoreError::Empty(what) => write!(f, "{what} is empty"),
            CoreError::Invalid(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for CoreError {}
