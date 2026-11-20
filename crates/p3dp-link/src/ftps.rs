//! The commands and the replies of the upload of a file to a printer by FTP over TLS, as text and nothing else: no socket is opened and nothing here is run. A command is built
//! only from a short list of verbs, with an argument that cannot hold a line break (so a name cannot start a second command), and a reply is read with the rules of the protocol
//! (a code of three digits, and a reply of several lines that ends with the same code and a space). The address in a reply to PASV must be the address of the control
//! connection: a printer (or whoever answers for it) that points the data connection somewhere else is refused.

use crate::error::LinkError;
use crate::transport::safe_file_name;
use p3dp_core::Secret;

/// The most bytes of one reply that are read.
pub const MAX_REPLY: usize = 16 * 1024;

fn invalid(s: impl Into<String>) -> LinkError {
    LinkError::Invalid(s.into())
}

/// The verbs this client sends.
const VERBS: [&str; 9] = [
    "USER", "PASS", "PBSZ", "PROT", "TYPE", "PASV", "STOR", "QUIT", "NOOP",
];

/// Builds a command line (with its CR LF). The verb must be one of the list, and the argument must have no CR, LF or NUL.
pub fn command(verb: &str, arg: Option<&str>) -> Result<String, LinkError> {
    if !VERBS.contains(&verb) {
        return Err(invalid(format!(
            "{verb:?} is not a command this client sends"
        )));
    }
    match arg {
        Some(a) => {
            if a.is_empty() || a.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0)) {
                return Err(invalid(
                    "the argument of a command is not empty and has no line break",
                ));
            }
            Ok(format!("{verb} {a}\r\n"))
        }
        None => Ok(format!("{verb}\r\n")),
    }
}

/// A reply: the code and its text, one line each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    pub lines: Vec<String>,
}

impl Reply {
    /// The first digit says how it went: 1 is "wait for more", 2 is done, 3 is "send the next part", 4 is "try again", 5 is "no".
    pub fn class(&self) -> u16 {
        self.code / 100
    }

    pub fn text(&self) -> String {
        self.lines.join(" ")
    }
}

/// Reads one reply from the start of `buf`. `Ok(None)` says it is not all there yet; `Ok(Some((reply, used)))` gives the reply and how many bytes it took.
pub fn parse_reply(buf: &[u8]) -> Result<Option<(Reply, usize)>, LinkError> {
    if buf.len() > MAX_REPLY {
        return Err(LinkError::BadReply(format!(
            "a reply longer than {MAX_REPLY} bytes"
        )));
    }
    let mut at = 0usize;
    let mut code: Option<u16> = None;
    let mut lines = Vec::new();
    while let Some(end) = buf[at..].windows(2).position(|w| w == b"\r\n") {
        let raw = &buf[at..at + end];
        at += end + 2;
        let line = std::str::from_utf8(raw)
            .map_err(|_| LinkError::BadReply("a reply is not text".to_string()))?;
        match code {
            None => {
                let digits = line
                    .get(..3)
                    .filter(|d| d.bytes().all(|b| b.is_ascii_digit()));
                let Some(d) = digits else {
                    return Err(LinkError::BadReply(
                        "a reply does not start with a code of three digits".to_string(),
                    ));
                };
                let c: u16 = d.parse().unwrap_or(0);
                if !(100..=699).contains(&c) {
                    return Err(LinkError::BadReply(format!("the code {c} is not one")));
                }
                match line.as_bytes().get(3) {
                    Some(b' ') | None => {
                        lines.push(line.get(4..).unwrap_or("").to_string());
                        return Ok(Some((Reply { code: c, lines }, at)));
                    }
                    Some(b'-') => {
                        code = Some(c);
                        lines.push(line[4..].to_string());
                    }
                    Some(_) => {
                        return Err(LinkError::BadReply(
                            "a code is followed by a space or a hyphen".to_string(),
                        ))
                    }
                }
            }
            Some(c) => {
                let closing = format!("{c} ");
                if let Some(rest) = line.strip_prefix(&closing) {
                    lines.push(rest.to_string());
                    return Ok(Some((Reply { code: c, lines }, at)));
                }
                lines.push(line.to_string());
            }
        }
    }
    Ok(None)
}

/// The address and port in the reply to PASV (`227 Entering Passive Mode (192,168,1,5,31,64)`), kept only when the address is the one of the control connection.
pub fn parse_pasv(reply: &Reply, control_host: &str) -> Result<(String, u16), LinkError> {
    if reply.code != 227 {
        return Err(LinkError::BadReply(format!(
            "the answer to PASV has the code {}, not 227",
            reply.code
        )));
    }
    let text = reply.text();
    let (open, close) = match (text.find('('), text.rfind(')')) {
        (Some(o), Some(c)) if o < c => (o, c),
        _ => {
            return Err(LinkError::BadReply(
                "the answer to PASV has no address in brackets".to_string(),
            ))
        }
    };
    let parts: Vec<u16> = text[open + 1..close]
        .split(',')
        .map(|p| p.trim().parse::<u16>())
        .collect::<Result<_, _>>()
        .map_err(|_| LinkError::BadReply("the address of PASV is not six numbers".to_string()))?;
    if parts.len() != 6 || parts.iter().any(|p| *p > 255) {
        return Err(LinkError::BadReply(
            "the address of PASV is not six numbers from 0 to 255".to_string(),
        ));
    }
    let host = format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]);
    let port = parts[4] * 256 + parts[5];
    if port < 1024 {
        return Err(LinkError::BadReply(format!(
            "the data port {port} is a privileged one"
        )));
    }
    if host != control_host {
        return Err(LinkError::BadReply(format!(
            "the data connection is pointed at {host} and the control connection is to {control_host}: refused"
        )));
    }
    Ok((host, port))
}

/// One step of the upload: what is sent and the classes of reply that let it go on.
#[derive(Clone, PartialEq, Eq)]
pub struct Step {
    pub line: String,
    /// The classes (first digit) of the replies that are good.
    pub expect: &'static [u16],
    secret: bool,
}

impl Step {
    /// The line as it may be written in a log: the password is not.
    pub fn masked(&self) -> String {
        if self.secret {
            "PASS ***".to_string()
        } else {
            self.line.trim_end().to_string()
        }
    }
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Step({:?} expects {:?})", self.masked(), self.expect)
    }
}

/// The commands of an upload over an FTP connection that is already in TLS: the user, the password (the access code of the printer), the protection of the data connection, a
/// binary transfer, the passive address, the store of the file and the end. The name is checked like every other file name of the program.
pub fn upload_steps(user: &str, password: &Secret, name: &str) -> Result<Vec<Step>, LinkError> {
    let name = safe_file_name(name)?;
    if user.is_empty() {
        return Err(invalid("the user name is empty"));
    }
    let step = |verb: &str, arg: Option<&str>, expect: &'static [u16], secret: bool| {
        command(verb, arg).map(|line| Step {
            line,
            expect,
            secret,
        })
    };
    Ok(vec![
        step("USER", Some(user), &[2, 3], false)?,
        step("PASS", Some(password.expose()), &[2], true)?,
        step("PBSZ", Some("0"), &[2], false)?,
        step("PROT", Some("P"), &[2], false)?,
        step("TYPE", Some("I"), &[2], false)?,
        step("PASV", None, &[2], false)?,
        step("STOR", Some(name), &[1, 2], false)?,
        step("QUIT", None, &[2], false)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::rng::Rng;

    #[test]
    fn a_command_has_one_line_and_a_verb_from_the_list() {
        assert_eq!(command("USER", Some("bblp")).unwrap(), "USER bblp\r\n");
        assert_eq!(command("PASV", None).unwrap(), "PASV\r\n");
        for bad in ["DELE", "user", "RETR", "SITE", ""] {
            assert!(command(bad, None).is_err(), "{bad:?}");
        }
        // a name cannot start a second command
        for arg in ["a\r\nDELE x", "a\nb", "a\rb", "a\0b", ""] {
            assert!(command("STOR", Some(arg)).is_err(), "{arg:?}");
        }
    }

    #[test]
    fn replies_of_one_line_and_of_several_lines_are_read_with_the_rules_of_the_protocol() {
        let (r, used) = parse_reply(b"220 Welcome\r\n").unwrap().unwrap();
        assert_eq!((r.code, r.class(), used), (220, 2, 13));
        assert_eq!(r.text(), "Welcome");
        // several lines: closed by the same code and a space; a line in between that starts with other digits is text
        let multi = b"211-Features:\r\n 226 not the end\r\n PASV\r\n211 End\r\nNEXT";
        let (r, used) = parse_reply(multi).unwrap().unwrap();
        assert_eq!(r.code, 211);
        assert_eq!(r.lines.len(), 4);
        assert_eq!(&multi[used..], b"NEXT");
        // not all there
        assert_eq!(parse_reply(b"220 Welc").unwrap(), None);
        assert_eq!(parse_reply(b"211-Start\r\nmore\r\n").unwrap(), None);
        assert_eq!(parse_reply(b"").unwrap(), None);
        // a code alone
        assert_eq!(parse_reply(b"200\r\n").unwrap().unwrap().0.code, 200);
        for bad in [
            &b"hello\r\n"[..],
            b"22 x\r\n",
            b"099 x\r\n",
            b"700 x\r\n",
            b"220x\r\n",
            b"\xff\xfe\xfd ok\r\n",
        ] {
            assert!(parse_reply(bad).is_err(), "{bad:?}");
        }
        assert!(parse_reply(&vec![b'a'; MAX_REPLY + 1]).is_err());
    }

    #[test]
    fn the_passive_address_must_be_that_of_the_control_connection() {
        let reply = |t: &str| Reply {
            code: 227,
            lines: vec![t.to_string()],
        };
        let r = reply("Entering Passive Mode (192,168,1,5,31,64).");
        assert_eq!(
            parse_pasv(&r, "192.168.1.5").unwrap(),
            ("192.168.1.5".to_string(), 8000)
        );
        // pointed somewhere else (a bounce), a port below 1024, a number out of range, five numbers, no brackets, another code
        for (text, control) in [
            ("Entering Passive Mode (10,0,0,9,31,64)", "192.168.1.5"),
            ("Entering Passive Mode (192,168,1,5,0,21)", "192.168.1.5"),
            (
                "Entering Passive Mode (192,168,1,256,31,64)",
                "192.168.1.256",
            ),
            ("Entering Passive Mode (192,168,1,5,31)", "192.168.1.5"),
            ("Entering Passive Mode 192,168,1,5,31,64", "192.168.1.5"),
            ("Entering Passive Mode (a,b,c,d,e,f)", "192.168.1.5"),
            ("Entering Passive Mode )192,168,1,5,31,64(", "192.168.1.5"),
        ] {
            assert!(parse_pasv(&reply(text), control).is_err(), "{text}");
        }
        let wrong = Reply {
            code: 500,
            lines: vec!["no".to_string()],
        };
        assert!(parse_pasv(&wrong, "192.168.1.5").is_err());
    }

    #[test]
    fn the_steps_of_an_upload_are_in_order_and_the_password_is_not_in_the_log() {
        let steps = upload_steps("bblp", &Secret::new("12345678"), "cube.gcode.3mf").unwrap();
        let masked: Vec<String> = steps.iter().map(Step::masked).collect();
        assert_eq!(
            masked,
            [
                "USER bblp",
                "PASS ***",
                "PBSZ 0",
                "PROT P",
                "TYPE I",
                "PASV",
                "STOR cube.gcode.3mf",
                "QUIT"
            ]
        );
        assert_eq!(steps[1].line, "PASS 12345678\r\n");
        assert!(!format!("{steps:?}").contains("12345678"));
        // the data connection is protected before anything is stored
        let prot = masked.iter().position(|l| l == "PROT P").unwrap();
        let stor = masked.iter().position(|l| l.starts_with("STOR")).unwrap();
        assert!(prot < stor);
        for bad in ["", "../x.3mf", "a/b.3mf", "a\r\nDELE b", "a b\n"] {
            assert!(
                upload_steps("bblp", &Secret::new("x"), bad).is_err(),
                "{bad:?}"
            );
        }
        assert!(upload_steps("", &Secret::new("x"), "a.3mf").is_err());
    }

    #[test]
    fn noise_never_panics_the_reader_of_replies() {
        let mut rng = Rng::new(0xF7_95);
        for _ in 0..3000 {
            let n = rng.below(80) as usize;
            let mut b = rng.bytes(n);
            if rng.below(2) == 0 {
                b.extend_from_slice(b"\r\n");
            }
            let _ = parse_reply(&b);
            let _ = parse_pasv(
                &Reply {
                    code: 227,
                    lines: vec![String::from_utf8_lossy(&b).into_owned()],
                },
                "1.2.3.4",
            );
        }
    }
}
