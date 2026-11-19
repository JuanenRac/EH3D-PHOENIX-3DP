//! A secret (a token, an access code) that never prints itself: its `Debug` and `Display` write `***`.

use std::fmt;

/// A piece of text that must not reach a log or a screen. The only way to read it is `expose`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(text: &str) -> Secret {
        Secret(text.to_string())
    }

    /// The text itself: only the code that sends it over the wire calls this.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_never_prints_itself() {
        let s = Secret::new("hunter2");
        assert!(!format!("{s:?}").contains("hunter2"));
        assert!(!format!("{s}").contains("hunter2"));
        assert_eq!(s.expose(), "hunter2");
        assert!(!s.is_empty());
        assert!(Secret::default().is_empty());
    }
}
