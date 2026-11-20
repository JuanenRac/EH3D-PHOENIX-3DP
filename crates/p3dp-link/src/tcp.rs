//! A transport over a plain TCP connection: HTTP/1.1, one request per connection, written with the standard library only. The drivers speak to an [`HttpTransport`], so this is what
//! lets them reach a Moonraker or an OctoPrint on the local network.
//!
//! What it does not do, on purpose: no TLS (an address with `https://` is refused with the reason; put a tunnel or a proxy in front), no redirects (a `3xx` is an answer like any other and
//! is not followed), no keep-alive. What it keeps to: a limit of time to connect and a limit for each read and write, a limit on the size of the head and of the body of an answer (a
//! server that never stops talking is cut off), and no byte of a request that could start another one (a line break in a path or in a header is refused before a connection is made).
//! A connection that could not be made is `NoConnection` (nothing was sent, so any request may be tried again); anything that breaks after the request left is `Timeout`, which the retry rules
//! try again only for a request that can be sent twice.

use crate::error::LinkError;
use crate::retry::{Retrying, ThreadSleeper};
use crate::transport::{HttpRequest, HttpResponse, HttpTransport};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// The limits of a connection. Every value is checked; a value outside its range is an error, never lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpLimits {
    /// How long to wait for the connection, 100 to 60000 ms.
    pub connect_timeout_ms: u64,
    /// How long one read or one write may wait, 100 to 600000 ms.
    pub io_timeout_ms: u64,
    /// The most bytes of a body that are read, 1 KiB to 256 MiB.
    pub max_body_bytes: u64,
    /// The most bytes of the head of an answer (the status line and the headers), 512 to 65536.
    pub max_head_bytes: usize,
}

impl Default for TcpLimits {
    fn default() -> TcpLimits {
        TcpLimits {
            connect_timeout_ms: 5_000,
            io_timeout_ms: 15_000,
            max_body_bytes: 8 << 20,
            max_head_bytes: 16 << 10,
        }
    }
}

impl TcpLimits {
    /// Checks every value against its range.
    pub fn validate(&self) -> Result<(), LinkError> {
        let bad = |what: &str, lo: u64, hi: u64| {
            LinkError::Invalid(format!("{what} must be from {lo} to {hi}"))
        };
        if !(100..=60_000).contains(&self.connect_timeout_ms) {
            return Err(bad("connect_timeout_ms", 100, 60_000));
        }
        if !(100..=600_000).contains(&self.io_timeout_ms) {
            return Err(bad("io_timeout_ms", 100, 600_000));
        }
        if !((1 << 10)..=(256 << 20)).contains(&self.max_body_bytes) {
            return Err(bad("max_body_bytes", 1 << 10, 256 << 20));
        }
        if !(512..=65_536).contains(&self.max_head_bytes) {
            return Err(bad("max_head_bytes", 512, 65_536));
        }
        Ok(())
    }
}

/// A transport over TCP to one address.
#[derive(Debug, Clone)]
pub struct TcpTransport {
    authority: String,
    limits: TcpLimits,
}

/// The `host:port` of an address as a person writes it: `host:port`, `http://host:port` or `http://host:port/` (and, for a host with no port, port 80 of `http://`).
pub fn authority_of(address: &str) -> Result<String, LinkError> {
    let bad =
        |why: &str| LinkError::Invalid(format!("the address {address:?} is not usable: {why}"));
    let a = address.trim();
    if a.to_ascii_lowercase().starts_with("https://") {
        return Err(bad(
            "TLS is not spoken here; use http:// inside the local network or put a tunnel in front",
        ));
    }
    let (rest, had_scheme) = match a
        .strip_prefix("http://")
        .or_else(|| a.strip_prefix("HTTP://"))
    {
        Some(r) => (r, true),
        None => (a, false),
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() {
        return Err(bad("it is empty"));
    }
    if rest.contains(['/', '?', '#', '@', ' ', '\\']) || rest.chars().any(char::is_control) {
        return Err(bad("no path, no user and no spaces"));
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(':') || h.starts_with('[') => (h, Some(p)),
        _ => (rest, None),
    };
    let host_ok = if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !inner.is_empty()
            && inner
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
    } else {
        !host.is_empty()
            && host.len() <= 253
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
    };
    if !host_ok {
        return Err(bad("the host has characters that a host cannot have"));
    }
    let port = match port {
        Some(p) => p
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .ok_or_else(|| bad("the port must be a number from 1 to 65535"))?,
        None if had_scheme => 80,
        None => return Err(bad("the port is missing (host:port)")),
    };
    Ok(format!("{host}:{port}"))
}

impl TcpTransport {
    /// A transport to an address, with the limits.
    pub fn new(address: &str, limits: TcpLimits) -> Result<TcpTransport, LinkError> {
        limits.validate()?;
        Ok(TcpTransport {
            authority: authority_of(address)?,
            limits,
        })
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn limits(&self) -> &TcpLimits {
        &self.limits
    }

    /// The same transport with the tries of the suite around it: a connection that failed is tried again for any method, a time-out only for a method that can be repeated
    /// (never a `POST`), with growing pauses of at most two seconds. `retries` is 0 to 5.
    pub fn with_retries(self, retries: u32, seed: u64) -> Retrying<TcpTransport, ThreadSleeper> {
        Retrying::new(self, ThreadSleeper, retries, seed)
    }
}

/// A header name is a token: letters, digits and a few signs.
fn token_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// The bytes of a request, or the reason it cannot be sent: a path with a space, a control character or a letter outside ASCII, a header with a line break, a header that the transport
/// writes itself.
pub fn encode_request(req: &HttpRequest, authority: &str) -> Result<Vec<u8>, LinkError> {
    let bad = |why: String| LinkError::Invalid(why);
    if !req.path.starts_with('/') {
        return Err(bad("the path of a request starts with /".to_string()));
    }
    if req.path.len() > 8192
        || req
            .path
            .bytes()
            .any(|b| b <= b' ' || b == 0x7f || b >= 0x80)
    {
        return Err(bad(
            "the path of a request has a space, a control character or a letter that must be percent-encoded"
                .to_string(),
        ));
    }
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nUser-Agent: phoenix-3dp\r\n",
        req.method.name(),
        req.path
    );
    for (k, v) in &req.headers {
        if !token_ok(k) {
            return Err(bad(format!("{k:?} is not a name of a header")));
        }
        if ["host", "content-length", "connection", "transfer-encoding"]
            .iter()
            .any(|w| k.eq_ignore_ascii_case(w))
        {
            return Err(bad(format!("the header {k} is written by the transport")));
        }
        if v.len() > 4096 || v.bytes().any(|b| (b < b' ' && b != b'\t') || b == 0x7f) {
            return Err(bad(format!(
                "the value of the header {k} has a line break or a control character"
            )));
        }
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    let sends_body = !req.body.is_empty()
        || matches!(
            req.method,
            crate::transport::Method::Post | crate::transport::Method::Put
        );
    if sends_body {
        head.push_str(&format!("Content-Length: {}\r\n", req.body.len()));
    }
    head.push_str("\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(&req.body);
    Ok(out)
}

/// What the head of an answer says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub status: u16,
    pub content_length: Option<u64>,
    pub chunked: bool,
}

/// Reads the status line and the headers.
pub fn parse_head(text: &str) -> Result<Head, LinkError> {
    let bad = |why: &str| LinkError::BadReply(format!("the head of the answer: {why}"));
    let mut lines = text.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_ascii_whitespace();
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/1.") {
        return Err(bad("it is not HTTP/1.x"));
    }
    let status: u16 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .filter(|s| (100..=599).contains(s))
        .ok_or_else(|| bad("the status is not a number from 100 to 599"))?;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        match k.as_str() {
            "content-length" => {
                let n: u64 = v
                    .parse()
                    .map_err(|_| bad("the Content-Length is not a number"))?;
                if content_length.is_some_and(|old| old != n) {
                    return Err(bad("two different Content-Length headers"));
                }
                content_length = Some(n);
            }
            "transfer-encoding" => {
                chunked = chunked || v.to_ascii_lowercase().contains("chunked");
            }
            _ => {}
        }
    }
    if chunked && content_length.is_some() {
        // both at once is how a request is smuggled past a proxy: refuse it
        return Err(bad("both Content-Length and chunked transfer coding"));
    }
    Ok(Head {
        status,
        content_length,
        chunked,
    })
}

/// Joins the chunks of a body. `limit` is the most bytes of body; a chunk past it is an error. Returns the body and whether the last chunk was seen.
pub fn decode_chunks(mut rest: &[u8], limit: u64) -> Result<(Vec<u8>, bool), LinkError> {
    let bad = |why: &str| LinkError::BadReply(format!("a chunked body: {why}"));
    let mut out: Vec<u8> = Vec::new();
    loop {
        let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") else {
            return Ok((out, false));
        };
        let size_text = String::from_utf8_lossy(&rest[..eol]).into_owned();
        let size_text = size_text.split(';').next().unwrap_or("").trim().to_string();
        let size = usize::from_str_radix(&size_text, 16)
            .map_err(|_| bad("a chunk with a size that is not hexadecimal"))?;
        rest = &rest[eol + 2..];
        if size == 0 {
            return Ok((out, true));
        }
        if (out.len() as u64).saturating_add(size as u64) > limit {
            return Err(bad("larger than the limit"));
        }
        if rest.len() < size + 2 {
            return Ok((out, false));
        }
        out.extend_from_slice(&rest[..size]);
        if &rest[size..size + 2] != b"\r\n" {
            return Err(bad("a chunk is not followed by a line break"));
        }
        rest = &rest[size + 2..];
    }
}

fn io_error(e: &std::io::Error) -> LinkError {
    match e.kind() {
        ErrorKind::ConnectionRefused | ErrorKind::AddrNotAvailable | ErrorKind::NotConnected => {
            LinkError::NoConnection
        }
        // the request may have left: a broken connection is told as a time-out, which is only tried again for a request that can be sent twice
        _ => LinkError::Timeout,
    }
}

impl HttpTransport for TcpTransport {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        // a request that cannot be written safely never makes a connection
        let bytes = encode_request(req, &self.authority)?;
        let addrs: Vec<_> = self
            .authority
            .to_socket_addrs()
            .map_err(|_| LinkError::NoConnection)?
            .collect();
        let connect = Duration::from_millis(self.limits.connect_timeout_ms);
        let mut stream = addrs
            .iter()
            .find_map(|a| TcpStream::connect_timeout(a, connect).ok())
            .ok_or(LinkError::NoConnection)?;
        let io = Duration::from_millis(self.limits.io_timeout_ms);
        stream
            .set_read_timeout(Some(io))
            .and_then(|()| stream.set_write_timeout(Some(io)))
            .map_err(|e| io_error(&e))?;
        let _ = stream.set_nodelay(true);
        stream.write_all(&bytes).map_err(|e| io_error(&e))?;
        read_answer(&mut stream, &self.limits)
    }
}

/// Reads an answer from a stream within the limits.
fn read_answer<R: Read>(stream: &mut R, limits: &TcpLimits) -> Result<HttpResponse, LinkError> {
    let mut raw: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    // the head
    let (head, mut body) = loop {
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let head_text = String::from_utf8_lossy(&raw[..i]).into_owned();
            let head = parse_head(&head_text)?;
            let body = raw[i + 4..].to_vec();
            if head.status == 100 {
                // the server said "go on" to a request that did not ask for it: the real answer follows
                raw = body;
                continue;
            }
            break (head, body);
        }
        if raw.len() > limits.max_head_bytes {
            return Err(LinkError::BadReply(
                "the head of the answer is larger than the limit".to_string(),
            ));
        }
        let n = stream.read(&mut buf).map_err(|e| io_error(&e))?;
        if n == 0 {
            return Err(LinkError::Timeout);
        }
        raw.extend_from_slice(&buf[..n]);
    };
    let too_large = || {
        LinkError::BadReply(format!(
            "the body of the answer is larger than {} bytes",
            limits.max_body_bytes
        ))
    };
    if let Some(n) = head.content_length {
        if n > limits.max_body_bytes {
            return Err(too_large());
        }
    }
    let no_body = head.status == 204 || head.status == 304 || (100..200).contains(&head.status);
    if no_body {
        return Ok(HttpResponse {
            status: head.status,
            body: Vec::new(),
        });
    }
    loop {
        if head.chunked {
            // the last chunk ends the body; looking for it only when the data ends like it keeps a long body from being decoded again after every read
            if body.ends_with(b"0\r\n\r\n") {
                let (decoded, finished) = decode_chunks(&body, limits.max_body_bytes)?;
                if finished {
                    return Ok(HttpResponse {
                        status: head.status,
                        body: decoded,
                    });
                }
            }
        } else if let Some(n) = head.content_length {
            if body.len() as u64 >= n {
                body.truncate(n as usize);
                return Ok(HttpResponse {
                    status: head.status,
                    body,
                });
            }
        }
        if body.len() as u64
            > limits
                .max_body_bytes
                .saturating_add(limits.max_head_bytes as u64)
        {
            return Err(too_large());
        }
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(e) => return Err(io_error(&e)),
        };
        if n == 0 {
            // the connection closed: that ends a body with no length; with a length or chunks it is a cut answer
            if !head.chunked && head.content_length.is_none() {
                if body.len() as u64 > limits.max_body_bytes {
                    return Err(too_large());
                }
                return Ok(HttpResponse {
                    status: head.status,
                    body,
                });
            }
            return Err(LinkError::Timeout);
        }
        body.extend_from_slice(&buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Method;

    #[test]
    fn addresses_are_read_the_way_people_write_them() {
        assert_eq!(
            authority_of("192.168.0.50:7125").unwrap(),
            "192.168.0.50:7125"
        );
        assert_eq!(
            authority_of("http://printer.local:7125/").unwrap(),
            "printer.local:7125"
        );
        assert_eq!(
            authority_of("http://octopi.local").unwrap(),
            "octopi.local:80"
        );
        assert_eq!(authority_of("[::1]:5000").unwrap(), "[::1]:5000");
        for bad in [
            "",
            "   ",
            "https://x:1",
            "HTTPS://x:1",
            "host",
            "host:0",
            "host:70000",
            "host:abc",
            "ho st:1",
            "host:1/path",
            "user@host:1",
            "host:1?x=1",
            "http://",
            "a\nb:1",
            "ho$t:1",
        ] {
            assert!(authority_of(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn limits_are_checked_not_lowered() {
        let with = |f: fn(&mut TcpLimits)| {
            let mut l = TcpLimits::default();
            f(&mut l);
            l
        };
        assert!(TcpLimits::default().validate().is_ok());
        assert!(with(|l| l.connect_timeout_ms = 10).validate().is_err());
        assert!(with(|l| l.io_timeout_ms = 700_000).validate().is_err());
        assert!(with(|l| l.max_head_bytes = 100).validate().is_err());
        assert!(TcpTransport::new("h:1", with(|l| l.max_body_bytes = 0)).is_err());
        assert!(TcpTransport::new("h:1", with(|l| l.max_body_bytes = 300 << 20)).is_err());
        assert!(TcpTransport::new("h:1", with(|l| l.max_body_bytes = 1 << 10)).is_ok());
    }

    #[test]
    fn a_request_is_written_with_its_own_length_and_closes_the_connection() {
        let r = HttpRequest::new(Method::Post, "/printer/print/start?filename=a%20b.gcode")
            .header("X-Api-Key", "k")
            .body(b"hello".to_vec());
        let text = String::from_utf8(encode_request(&r, "h:1").unwrap()).unwrap();
        assert!(text.starts_with(
            "POST /printer/print/start?filename=a%20b.gcode HTTP/1.1\r\nHost: h:1\r\n"
        ));
        assert!(text.contains("Connection: close\r\n") && text.contains("X-Api-Key: k\r\n"));
        assert!(text.ends_with("Content-Length: 5\r\n\r\nhello"));
        let g = HttpRequest::new(Method::Get, "/server/info");
        let text = String::from_utf8(encode_request(&g, "h:1").unwrap()).unwrap();
        assert!(!text.contains("Content-Length"));
        let p = HttpRequest::new(Method::Post, "/x");
        assert!(String::from_utf8(encode_request(&p, "h:1").unwrap())
            .unwrap()
            .contains("Content-Length: 0\r\n"));
    }

    #[test]
    fn nothing_that_could_start_another_request_is_written() {
        let h = |k: &str, v: &str| HttpRequest::new(Method::Get, "/x").header(k, v);
        for r in [
            HttpRequest::new(Method::Get, "/x\r\nHost: evil"),
            HttpRequest::new(Method::Get, "/a b"),
            HttpRequest::new(Method::Get, "/ñ"),
            HttpRequest::new(Method::Get, "x"),
            HttpRequest::new(Method::Get, ""),
            HttpRequest::new(Method::Get, &format!("/{}", "a".repeat(9000))),
            h("X-Key", "a\r\nX-Other: 1"),
            h("X-Key", "a\nb"),
            h("X Key", "a"),
            h("", "a"),
            h("Host", "evil"),
            h("content-length", "1"),
            h("Transfer-Encoding", "chunked"),
            h("Connection", "keep-alive"),
        ] {
            assert!(encode_request(&r, "h:1").is_err(), "{:?}", r.path);
        }
    }

    #[test]
    fn the_head_of_an_answer_is_read_strictly() {
        let h = parse_head("HTTP/1.1 200 OK\r\nContent-Length: 12\r\nX-A: b").unwrap();
        assert_eq!(
            (h.status, h.content_length, h.chunked),
            (200, Some(12), false)
        );
        let h = parse_head("HTTP/1.0 404 Not Found\r\ntransfer-encoding: Chunked").unwrap();
        assert_eq!((h.status, h.chunked), (404, true));
        for bad in [
            "",
            "SMTP/1.1 200 OK",
            "HTTP/2 200",
            "HTTP/1.1 abc",
            "HTTP/1.1 99",
            "HTTP/1.1 600",
            "HTTP/1.1 200\r\nContent-Length: x",
            "HTTP/1.1 200\r\nContent-Length: 1\r\nContent-Length: 2",
            "HTTP/1.1 200\r\nContent-Length: 1\r\nTransfer-Encoding: chunked",
        ] {
            assert!(parse_head(bad).is_err(), "{bad:?}");
        }
        // the same length twice is not a conflict
        assert!(parse_head("HTTP/1.1 200\r\nContent-Length: 3\r\nContent-Length: 3").is_ok());
    }

    #[test]
    fn chunks_are_joined_and_damage_is_an_error() {
        let ok = b"5\r\nhello\r\n6;ext=1\r\n world\r\n0\r\n\r\n";
        assert_eq!(
            decode_chunks(ok, 100).unwrap(),
            (b"hello world".to_vec(), true)
        );
        // a body that is not finished yet
        assert_eq!(
            decode_chunks(b"5\r\nhel", 100).unwrap(),
            (Vec::new(), false)
        );
        assert_eq!(
            decode_chunks(b"5\r\nhello\r\n", 100).unwrap(),
            (b"hello".to_vec(), false)
        );
        assert!(decode_chunks(b"zz\r\nhello\r\n0\r\n\r\n", 100).is_err());
        assert!(decode_chunks(b"5\r\nhelloXX0\r\n\r\n", 100).is_err());
        assert!(decode_chunks(b"5\r\nhello\r\n0\r\n\r\n", 4).is_err());
        assert!(decode_chunks(b"ffffffffffffffff\r\n", 100).is_err());
    }

    #[test]
    fn an_answer_is_read_from_any_stream_within_the_limits() {
        let small = TcpLimits {
            max_body_bytes: 1024,
            ..TcpLimits::default()
        };
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhelloEXTRA";
        let r = read_answer(&mut s, &small).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, &b"hello"[..]));
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\n\r\nuntil the end";
        assert_eq!(read_answer(&mut s, &small).unwrap().body, b"until the end");
        let mut s: &[u8] = b"HTTP/1.1 204 No Content\r\n\r\n";
        assert!(read_answer(&mut s, &small).unwrap().body.is_empty());
        let mut s: &[u8] =
            b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok";
        let r = read_answer(&mut s, &small).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (201, &b"ok"[..]));
        // cut: the length promised was not sent
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\n\r\nshort";
        assert_eq!(read_answer(&mut s, &small), Err(LinkError::Timeout));
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel";
        assert_eq!(read_answer(&mut s, &small), Err(LinkError::Timeout));
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\nCont";
        assert_eq!(read_answer(&mut s, &small), Err(LinkError::Timeout));
        // too large, by its length and by what is really sent
        let mut s: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 5000\r\n\r\n";
        assert!(matches!(
            read_answer(&mut s, &small),
            Err(LinkError::BadReply(_))
        ));
        let mut big = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        big.extend(std::iter::repeat_n(b'x', 100_000));
        let mut s: &[u8] = &big;
        assert!(matches!(
            read_answer(&mut s, &small),
            Err(LinkError::BadReply(_))
        ));
        let mut head_bomb = b"HTTP/1.1 200 OK\r\n".to_vec();
        head_bomb.extend(std::iter::repeat_n(b'a', 100_000));
        let mut s: &[u8] = &head_bomb;
        assert!(matches!(
            read_answer(&mut s, &small),
            Err(LinkError::BadReply(_))
        ));
    }
}
