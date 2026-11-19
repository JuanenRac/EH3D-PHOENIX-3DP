//! The G-code file: a header of comment lines and the lines of the program, with a check that no line can smuggle in another one.

use crate::error::FormatError;

/// The longest line that is written (the firmware of most printers reads 256, a few read less: this is the limit of the format, not of a machine).
pub const MAX_LINE: usize = 1024;

fn check(line: &str) -> Result<(), FormatError> {
    if line.contains(['\n', '\r', '\0']) {
        return Err(FormatError::malformed(
            "gcode",
            format!(
                "a line has a line break or a zero byte in it: {:?}",
                line.chars().take(40).collect::<String>()
            ),
        ));
    }
    if line.len() > MAX_LINE {
        return Err(FormatError::malformed(
            "gcode",
            format!("a line is {} bytes, more than {MAX_LINE}", line.len()),
        ));
    }
    Ok(())
}

/// The text of a G-code file: the header lines as comments (`; text`), a blank line and then the body. Lines end with `\n`.
pub fn write(header: &[String], body: &[String]) -> Result<String, FormatError> {
    let mut out = String::new();
    for h in header {
        check(h)?;
        out.push_str("; ");
        out.push_str(h);
        out.push('\n');
    }
    if !header.is_empty() {
        out.push('\n');
    }
    for l in body {
        check(l)?;
        out.push_str(l);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_header_and_body() {
        let t = write(
            &["generator: PHOENIX-3DP".to_string()],
            &["G28".to_string(), "M84".to_string()],
        )
        .unwrap();
        assert_eq!(t, "; generator: PHOENIX-3DP\n\nG28\nM84\n");
        assert_eq!(write(&[], &["G28".to_string()]).unwrap(), "G28\n");
    }

    #[test]
    fn a_line_cannot_carry_another_line() {
        assert!(write(&[], &["G28\nM104 S999".to_string()]).is_err());
        assert!(write(&["a\rb".to_string()], &[]).is_err());
        assert!(write(&[], &["a\0".to_string()]).is_err());
        assert!(write(&[], &["x".repeat(MAX_LINE + 1)]).is_err());
        assert!(write(&[], &["x".repeat(MAX_LINE)]).is_ok());
    }
}
