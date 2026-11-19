//! The errors of the readers and writers.

use p3dp_core::CoreError;
use std::fmt;

/// Why a file was not read or not written.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatError {
    /// The file is larger than the limit.
    TooLarge { bytes: u64, max: u64 },
    /// The file is damaged or is not what it says: the format and the reason.
    Malformed {
        format: &'static str,
        reason: String,
    },
    /// The file is valid but uses something this program does not read (the format and what).
    Unsupported { format: &'static str, what: String },
    /// A value or a limit of the core.
    Core(CoreError),
}

impl FormatError {
    pub fn malformed(format: &'static str, reason: impl Into<String>) -> FormatError {
        FormatError::Malformed {
            format,
            reason: reason.into(),
        }
    }

    pub fn unsupported(format: &'static str, what: impl Into<String>) -> FormatError {
        FormatError::Unsupported {
            format,
            what: what.into(),
        }
    }
}

impl From<CoreError> for FormatError {
    fn from(e: CoreError) -> FormatError {
        FormatError::Core(e)
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::TooLarge { bytes, max } => {
                write!(f, "the file is {bytes} bytes, more than the limit of {max}")
            }
            FormatError::Malformed { format, reason } => write!(f, "{format}: {reason}"),
            FormatError::Unsupported { format, what } => {
                write!(f, "{format}: {what} is not supported")
            }
            FormatError::Core(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FormatError {}
