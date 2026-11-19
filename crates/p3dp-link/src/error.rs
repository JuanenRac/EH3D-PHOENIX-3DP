//! The errors of the link.

use p3dp_core::machine::TempError;
use std::fmt;

/// What can go wrong when talking to a printer or a server.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkError {
    /// The connection could not be made (nothing was sent).
    NoConnection,
    /// The request was sent and no answer came in time.
    Timeout,
    /// The key or the token was refused (401).
    Auth,
    /// The key is good but may not do this (403).
    Forbidden,
    /// Any other answer of an error, with its status and what the other side said.
    Http { status: u16, detail: String },
    /// The answer could not be understood.
    BadReply(String),
    /// A temperature that the model does not allow: nothing was sent.
    Temp(TempError),
    /// The printer does not have this capability: it is refused, never done by guess.
    Capability(&'static str),
    /// Something that is switched off in the settings (raw G-code, for one).
    Disabled(&'static str),
    /// A value that is not allowed (a file name, a number).
    Invalid(String),
    /// Part of the design that has not been built or tried.
    Experimental(&'static str),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::NoConnection => f.write_str("the connection could not be made"),
            LinkError::Timeout => f.write_str("no answer in time"),
            LinkError::Auth => f.write_str("the key or the token was refused"),
            LinkError::Forbidden => f.write_str("the key may not do this"),
            LinkError::Http { status, detail } => {
                write!(f, "the other side answered {status}: {detail}")
            }
            LinkError::BadReply(s) => write!(f, "the answer could not be understood: {s}"),
            LinkError::Temp(t) => write!(f, "{t}"),
            LinkError::Capability(c) => write!(f, "this printer cannot {c}"),
            LinkError::Disabled(c) => write!(f, "{c} is switched off in the settings"),
            LinkError::Invalid(s) => f.write_str(s),
            LinkError::Experimental(s) => write!(f, "experimental, not built yet: {s}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<TempError> for LinkError {
    fn from(e: TempError) -> LinkError {
        LinkError::Temp(e)
    }
}

impl LinkError {
    /// Whether trying the same request again could work (a connection that was not made, a time-out, a server that is busy).
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            LinkError::NoConnection
                | LinkError::Timeout
                | LinkError::Http {
                    status: 502..=504,
                    ..
                }
        )
    }
}
