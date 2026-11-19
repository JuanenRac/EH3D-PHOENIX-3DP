//! How a request travels. The drivers only know this trait; a real network transport and the simulators both implement it.

use crate::error::LinkError;

/// An HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    /// A method that can be sent again without doing the thing twice.
    pub fn is_idempotent(self) -> bool {
        !matches!(self, Method::Post)
    }

    pub fn name(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
        }
    }
}

/// A request: the path with its query, the headers and the body.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn new(method: Method, path: &str) -> HttpRequest {
        HttpRequest {
            method,
            path: path.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> HttpRequest {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> HttpRequest {
        self.body = body;
        self
    }

    /// The value of a header (the name is compared without regard to case).
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The path without the query.
    pub fn route(&self) -> &str {
        self.path.split('?').next().unwrap_or("")
    }

    /// The value of a parameter of the query, decoded.
    pub fn query(&self, key: &str) -> Option<String> {
        let q = self.path.split_once('?')?.1;
        q.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            if percent_decode(k) == key {
                Some(percent_decode(v))
            } else {
                None
            }
        })
    }
}

/// An answer.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
        }
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Sends a request and gives the answer. An answer with an error status is still an `Ok`; only a request that did not get one is an `Err`.
pub trait HttpTransport {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError>;
}

/// Encodes a text for a path or a query: everything but letters, digits and `-._~` becomes `%XX`.
pub fn percent_encode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// The inverse of `percent_encode` (`+` is a space; a bad escape is kept as it is).
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|t| u8::from_str_radix(t, 16).ok());
                if let Some(v) = h {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A file name that is safe to send and to put in a path: no separators, no `..`, no control characters, at most 128 bytes, and a known extension.
pub fn safe_file_name(name: &str) -> Result<&str, LinkError> {
    let bad =
        |why: &str| LinkError::Invalid(format!("the file name {name:?} is not allowed: {why}"));
    if name.is_empty() || name.len() > 128 {
        return Err(bad("it must be 1 to 128 bytes"));
    }
    if name.contains(['/', '\\']) || name == "." || name.contains("..") {
        return Err(bad("no folders and no .."));
    }
    if name.chars().any(char::is_control) || name.starts_with(' ') || name.ends_with(' ') {
        return Err(bad("no control characters and no spaces at the ends"));
    }
    let lower = name.to_ascii_lowercase();
    if ![".gcode", ".gco", ".bgcode", ".3mf"]
        .iter()
        .any(|e| lower.ends_with(e))
    {
        return Err(bad("it must end in .gcode, .gco, .bgcode or .3mf"));
    }
    Ok(name)
}

/// One part of a `multipart/form-data` body.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub filename: Option<String>,
    pub data: Vec<u8>,
}

/// Builds a `multipart/form-data` body. The boundary must not appear in a part.
pub fn multipart(boundary: &str, parts: &[Part]) -> Result<Vec<u8>, LinkError> {
    let mut out = Vec::new();
    for p in parts {
        if p.name.contains(['"', '\r', '\n'])
            || p.filename
                .as_deref()
                .is_some_and(|f| f.contains(['"', '\r', '\n']))
        {
            return Err(LinkError::Invalid(
                "a name of a form part has a quote or a line break".to_string(),
            ));
        }
        let b = boundary.as_bytes();
        if !b.is_empty() && p.data.windows(b.len()).any(|w| w == b) {
            return Err(LinkError::Invalid(
                "the boundary of the form appears in the data".to_string(),
            ));
        }
        out.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"",
                p.name
            )
            .as_bytes(),
        );
        if let Some(f) = &p.filename {
            out.extend_from_slice(
                format!("; filename=\"{f}\"\r\nContent-Type: application/octet-stream").as_bytes(),
            );
        }
        out.extend_from_slice(b"\r\n\r\n");
        out.extend_from_slice(&p.data);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok(out)
}

/// The boundary named in a `Content-Type` header.
pub fn boundary_of(content_type: &str) -> Option<&str> {
    let (_, b) = content_type.split_once("boundary=")?;
    Some(b.trim().trim_matches('"')).filter(|b| !b.is_empty())
}

/// Reads a `multipart/form-data` body (the simulators use it to see what was uploaded).
pub fn parse_multipart(body: &[u8], boundary: &str) -> Result<Vec<Part>, LinkError> {
    let bad = |s: &str| LinkError::BadReply(format!("multipart: {s}"));
    let delim = format!("--{boundary}");
    let d = delim.as_bytes();
    let find = |hay: &[u8], needle: &[u8], from: usize| -> Option<usize> {
        if needle.is_empty() || hay.len() < needle.len() || from > hay.len() - needle.len() {
            return None;
        }
        (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
    };
    let mut parts = Vec::new();
    let mut pos = find(body, d, 0).ok_or_else(|| bad("no boundary"))?;
    loop {
        let after = pos + d.len();
        if body[after..].starts_with(b"--") {
            return Ok(parts);
        }
        let start = after
            + body[after..]
                .starts_with(b"\r\n")
                .then_some(2)
                .ok_or_else(|| bad("a part does not start with a line break"))?;
        let head_end =
            find(body, b"\r\n\r\n", start).ok_or_else(|| bad("a part has no header end"))?;
        let head = String::from_utf8_lossy(&body[start..head_end]).into_owned();
        let data_start = head_end + 4;
        let next = find(body, format!("\r\n{delim}").as_bytes(), data_start)
            .ok_or_else(|| bad("a part is not closed"))?;
        let attr = |key: &str| -> Option<String> {
            let k = format!("{key}=\"");
            let i = head.find(&k)? + k.len();
            let j = head[i..].find('"')?;
            Some(head[i..i + j].to_string())
        };
        parts.push(Part {
            name: attr("name").ok_or_else(|| bad("a part has no name"))?,
            filename: attr("filename"),
            data: body[data_start..next].to_vec(),
        });
        pos = next + 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_round_trips() {
        for s in ["a b.gcode", "ñandú/?&=%", "plain-name_1.gcode", "", "100%"] {
            assert_eq!(percent_decode(&percent_encode(s)), s);
        }
        assert_eq!(percent_encode("a b&c"), "a%20b%26c");
        assert_eq!(percent_decode("a+b%2"), "a b%2");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn requests_know_their_query() {
        let r = HttpRequest::new(Method::Post, "/printer/gcode/script?script=M104%20S200&x=1")
            .header("X-Api-Key", "k");
        assert_eq!(r.route(), "/printer/gcode/script");
        assert_eq!(r.query("script").as_deref(), Some("M104 S200"));
        assert_eq!(r.query("x").as_deref(), Some("1"));
        assert_eq!(r.query("nothing"), None);
        assert_eq!(r.header_value("x-api-key"), Some("k"));
        assert!(Method::Get.is_idempotent() && !Method::Post.is_idempotent());
        assert_eq!(Method::Put.name(), "PUT");
    }

    #[test]
    fn file_names_are_checked() {
        for ok in ["a.gcode", "Benchy v2.GCO", "x.3mf", "m.bgcode"] {
            assert!(safe_file_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "a/b.gcode",
            "..\\x.gcode",
            "a..gcode",
            "a.txt",
            "a\n.gcode",
            " a.gcode",
            &"x".repeat(130),
        ] {
            assert!(safe_file_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn multipart_round_trips() {
        let parts = vec![
            Part {
                name: "print".into(),
                filename: None,
                data: b"false".to_vec(),
            },
            Part {
                name: "file".into(),
                filename: Some("a.gcode".into()),
                data: b"G28\r\nM84\r\n".to_vec(),
            },
            Part {
                name: "empty".into(),
                filename: None,
                data: vec![],
            },
        ];
        let body = multipart("XBOUNDARYX", &parts).unwrap();
        assert_eq!(parse_multipart(&body, "XBOUNDARYX").unwrap(), parts);
        assert_eq!(
            boundary_of("multipart/form-data; boundary=\"abc\""),
            Some("abc")
        );
        assert_eq!(boundary_of("text/plain"), None);
    }

    #[test]
    fn multipart_refuses_what_could_break_it() {
        let p = |n: &str, d: &[u8]| Part {
            name: n.into(),
            filename: None,
            data: d.to_vec(),
        };
        assert!(multipart("B", &[p("a\"b", b"x")]).is_err());
        assert!(multipart("BOUND", &[p("a", b"xxBOUNDxx")]).is_err());
        for bad in [
            &b""[..],
            b"--B\r\nno end",
            b"--B\r\nContent-Disposition: form-data\r\n\r\nx\r\n--B--",
            b"garbage",
        ] {
            assert!(parse_multipart(bad, "B").is_err());
        }
    }
}
