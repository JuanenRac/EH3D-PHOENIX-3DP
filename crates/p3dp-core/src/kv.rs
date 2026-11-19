//! The settings file: lines of `key = value`, `#` starts a comment, blank lines are ignored. One reader for every file of settings of the program.

use crate::error::CoreError;

/// One line of a settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The line number, starting at 1.
    pub line: usize,
    pub key: String,
    pub value: String,
}

/// The largest settings file that is read (1 MiB): a bigger one is not a settings file.
pub const MAX_BYTES: usize = 1 << 20;

/// Reads the text of a settings file. A line with no `=` or an empty key is an error that names the line.
pub fn parse(text: &str) -> Result<Vec<Entry>, CoreError> {
    if text.len() > MAX_BYTES {
        return Err(CoreError::Invalid(format!(
            "the settings are {} bytes, more than {MAX_BYTES}",
            text.len()
        )));
    }
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(CoreError::Invalid(format!(
                "line {}: expected key = value",
                i + 1
            )));
        };
        let key = k.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(CoreError::Invalid(format!(
                "line {}: the key is empty or has spaces",
                i + 1
            )));
        }
        let value = v.split(" #").next().unwrap_or("").trim();
        out.push(Entry {
            line: i + 1,
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    Ok(out)
}

/// Reads a whole number in a range; the error names the key.
pub fn int_in(key: &str, value: &str, min: i64, max: i64) -> Result<i64, CoreError> {
    let n: i64 = value
        .trim()
        .parse()
        .map_err(|_| CoreError::Invalid(format!("{key}: {value:?} is not a whole number")))?;
    if n < min || n > max {
        return Err(CoreError::OutOfRange {
            what: key.to_string(),
            value: n as f64,
            min: min as f64,
            max: max as f64,
        });
    }
    Ok(n)
}

/// Reads a finite number in a range; the error names the key.
pub fn float_in(key: &str, value: &str, min: f64, max: f64) -> Result<f64, CoreError> {
    let n: f64 = value
        .trim()
        .parse()
        .map_err(|_| CoreError::Invalid(format!("{key}: {value:?} is not a number")))?;
    if !n.is_finite() {
        return Err(CoreError::Invalid(format!(
            "{key}: {value:?} is not a finite number"
        )));
    }
    if n < min || n > max {
        return Err(CoreError::OutOfRange {
            what: key.to_string(),
            value: n,
            min,
            max,
        });
    }
    Ok(n)
}

/// Reads `on` or `off` (also `true`/`false`, `1`/`0`).
pub fn on_off(key: &str, value: &str) -> Result<bool, CoreError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => Ok(true),
        "off" | "false" | "0" | "no" => Ok(false),
        _ => Err(CoreError::Invalid(format!("{key}: expected on or off"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keys_comments_and_blanks() {
        let e = parse("# a\n\nserver_address = 10.0.0.2:18200\nlimit_x=3 # note\n").unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].key, "server_address");
        assert_eq!(e[0].line, 3);
        assert_eq!(e[1].value, "3");
    }

    #[test]
    fn a_bad_line_names_its_number() {
        let err = parse("a=1\nnonsense\n").unwrap_err().to_string();
        assert!(err.contains("line 2"));
        assert!(parse("= 3").is_err());
        assert!(parse("a b = 3").is_err());
    }

    #[test]
    fn ranges_are_checked() {
        assert_eq!(int_in("k", "5", 1, 10).unwrap(), 5);
        assert!(int_in("k", "11", 1, 10).is_err());
        assert!(int_in("k", "x", 1, 10).is_err());
        assert!(float_in("k", "NaN", 0.0, 1.0).is_err());
        assert!(float_in("k", "inf", 0.0, 1.0).is_err());
        assert_eq!(float_in("k", "0.5", 0.0, 1.0).unwrap(), 0.5);
        assert!(on_off("k", "on").unwrap());
        assert!(!on_off("k", "Off").unwrap());
        assert!(on_off("k", "maybe").is_err());
    }

    #[test]
    fn a_huge_file_is_refused() {
        let big = "a=1\n".repeat(MAX_BYTES / 4 + 10);
        assert!(parse(&big).is_err());
    }
}
