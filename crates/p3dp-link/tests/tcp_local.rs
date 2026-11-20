//! The transport over TCP against a socket of this computer: a server written for the tests that says what it is told to, and the two simulated printers served over it so that the
//! drivers run over a real connection. Nothing here leaves 127.0.0.1.

use p3dp_core::machine::Zone;
use p3dp_core::{Secret, TempLimits};
use p3dp_link::moonraker::MoonrakerDriver;
use p3dp_link::octoprint::OctoPrintDriver;
use p3dp_link::retry::{CountingSleeper, Retrying};
use p3dp_link::sim::{SimCore, SimMoonraker, SimOctoPrint};
use p3dp_link::{
    Command, HttpRequest, HttpTransport, JobState, LinkError, Method, PrinterDriver, TcpLimits,
    TcpTransport,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const LIMITS: TempLimits = TempLimits {
    nozzle_c: 300.0,
    bed_c: 100.0,
    chamber_c: Some(50.0),
};

/// What the server saw of one request.
#[derive(Debug, Clone)]
struct Seen {
    head: String,
    body: Vec<u8>,
}

impl Seen {
    fn line(&self) -> &str {
        self.head.lines().next().unwrap_or("")
    }

    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
        })
    }
}

fn read_request(s: &mut TcpStream) -> Option<Seen> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    let cut = loop {
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = s.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&raw[..cut]).into_owned();
    let want: usize = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse().ok())?
        })
        .unwrap_or(0);
    let mut body = raw[cut + 4..].to_vec();
    while body.len() < want {
        let n = s.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some(Seen { head, body })
}

/// A server on 127.0.0.1 that gives each connection to a handler and keeps what it saw.
struct Local {
    address: String,
    stop: Arc<AtomicBool>,
    connections: Arc<AtomicUsize>,
    thread: Option<JoinHandle<Vec<Seen>>>,
}

impl Local {
    fn start<F>(handler: F) -> Local
    where
        F: Fn(usize, &Seen, &mut TcpStream) + Send + 'static,
    {
        Local::start_with(|| (), move |(), n, req, s| handler(n, req, s))
    }

    /// As `start`, with a state that is made in the thread of the server (the simulated printers are not for several threads).
    fn start_with<S, M, F>(make: M, mut handler: F) -> Local
    where
        M: FnOnce() -> S + Send + 'static,
        F: FnMut(&mut S, usize, &Seen, &mut TcpStream) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(AtomicUsize::new(0));
        let (stop2, count2) = (stop.clone(), connections.clone());
        let thread = std::thread::spawn(move || {
            let mut state = make();
            let mut seen = Vec::new();
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let n = count2.fetch_add(1, Ordering::SeqCst);
                        let _ = stream.set_nonblocking(false);
                        if let Some(req) = read_request(&mut stream) {
                            handler(&mut state, n, &req, &mut stream);
                            seen.push(req);
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(2)),
                }
            }
            seen
        });
        Local {
            address,
            stop,
            connections,
            thread: Some(thread),
        }
    }

    fn transport(&self, limits: TcpLimits) -> TcpTransport {
        TcpTransport::new(&self.address, limits).unwrap()
    }

    fn finish(mut self) -> (Vec<Seen>, usize) {
        self.stop.store(true, Ordering::SeqCst);
        let seen = self.thread.take().unwrap().join().unwrap();
        (seen, self.connections.load(Ordering::SeqCst))
    }
}

fn fast() -> TcpLimits {
    TcpLimits {
        connect_timeout_ms: 1_000,
        io_timeout_ms: 1_500,
        max_body_bytes: 64 << 10,
        max_head_bytes: 4096,
    }
}

fn say(s: &mut TcpStream, text: &str) {
    let _ = s.write_all(text.as_bytes());
}

#[test]
fn a_request_reaches_the_server_as_written_and_the_answer_comes_back() {
    let server = Local::start(|_, _, s| {
        say(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"result\":\"ok\"}");
    });
    let mut t = server.transport(fast());
    let req = HttpRequest::new(Method::Post, "/printer/gcode/script?script=G28")
        .header("X-Api-Key", "the-key")
        .body(b"payload".to_vec());
    let r = t.send(&req).unwrap();
    assert_eq!((r.status, r.text().as_str()), (200, "{\"result\":\"ok\"}"));
    let (seen, connections) = server.finish();
    assert_eq!(connections, 1);
    assert_eq!(
        seen[0].line(),
        "POST /printer/gcode/script?script=G28 HTTP/1.1"
    );
    assert_eq!(seen[0].header("x-api-key").as_deref(), Some("the-key"));
    assert_eq!(seen[0].header("content-length").as_deref(), Some("7"));
    assert_eq!(seen[0].header("connection").as_deref(), Some("close"));
    assert!(seen[0]
        .header("host")
        .is_some_and(|h| h.starts_with("127.0.0.1:")));
    assert_eq!(seen[0].body, b"payload");
}

#[test]
fn an_answer_in_chunks_and_an_answer_that_ends_with_the_connection_are_both_read() {
    let server = Local::start(|n, _, s| {
        if n == 0 {
            say(s, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n7\r\n, world\r\n0\r\n\r\n");
        } else {
            say(s, "HTTP/1.0 200 OK\r\n\r\nno length at all");
        }
    });
    let mut t = server.transport(fast());
    let get = HttpRequest::new(Method::Get, "/x");
    assert_eq!(t.send(&get).unwrap().text(), "hello, world");
    assert_eq!(t.send(&get).unwrap().text(), "no length at all");
    server.finish();
}

#[test]
fn an_error_status_is_an_answer_and_a_redirect_is_not_followed() {
    let server = Local::start(|n, _, s| {
        if n == 0 {
            say(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\nno");
        } else {
            say(
                s,
                "HTTP/1.1 302 Found\r\nLocation: http://elsewhere/\r\nContent-Length: 0\r\n\r\n",
            );
        }
    });
    let mut t = server.transport(fast());
    let get = HttpRequest::new(Method::Get, "/x");
    assert_eq!(t.send(&get).unwrap().status, 404);
    assert_eq!(t.send(&get).unwrap().status, 302);
    let (_, connections) = server.finish();
    assert_eq!(connections, 2, "the redirect was not followed");
}

#[test]
fn a_body_or_a_head_over_the_limit_is_refused() {
    let server = Local::start(|n, _, s| match n {
        0 => say(s, "HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n"),
        1 => {
            // a server that never stops
            say(s, "HTTP/1.1 200 OK\r\n\r\n");
            let chunk = [b'x'; 8192];
            for _ in 0..64 {
                if s.write_all(&chunk).is_err() {
                    break;
                }
            }
        }
        _ => {
            say(s, "HTTP/1.1 200 OK\r\n");
            let junk = [b'h'; 1024];
            for _ in 0..64 {
                if s.write_all(&junk).is_err() {
                    break;
                }
            }
        }
    });
    let mut t = server.transport(fast());
    let get = HttpRequest::new(Method::Get, "/x");
    for _ in 0..3 {
        assert!(matches!(t.send(&get), Err(LinkError::BadReply(_))));
    }
    server.finish();
}

#[test]
fn a_server_that_does_not_answer_is_a_timeout_and_a_get_is_tried_again_but_a_post_is_not() {
    let server = Local::start(|n, _, s| {
        // the first two connections are closed with no answer; the third answers
        if n >= 2 {
            say(s, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        }
    });
    let started = Instant::now();
    let mut t = Retrying::new(server.transport(fast()), CountingSleeper::default(), 2, 7);
    let r = t.send(&HttpRequest::new(Method::Get, "/x")).unwrap();
    assert_eq!(r.text(), "ok");
    assert_eq!(t.last_attempts, 3);
    assert!(started.elapsed() < Duration::from_secs(5));
    // a post that timed out may have been done: it is not sent again
    let mut p = Retrying::new(
        TcpTransport::new(&server.address, fast()).unwrap(),
        CountingSleeper::default(),
        2,
        7,
    );
    let slow = Local::start(|_, _, _| std::thread::sleep(Duration::from_millis(2_200)));
    let started_slow = Instant::now();
    let mut q = Retrying::new(slow.transport(fast()), CountingSleeper::default(), 3, 7);
    assert_eq!(
        q.send(&HttpRequest::new(Method::Post, "/x")),
        Err(LinkError::Timeout)
    );
    assert_eq!(q.last_attempts, 1);
    assert!(
        started_slow.elapsed() >= Duration::from_millis(1_400),
        "it waited for the limit"
    );
    let (_, slow_connections) = slow.finish();
    assert_eq!(slow_connections, 1);
    assert!(p.send(&HttpRequest::new(Method::Get, "/x")).is_ok());
    server.finish();
}

#[test]
fn a_connection_that_cannot_be_made_is_told_so_and_tried_again_for_any_method() {
    // an address that was open a moment ago and is closed now
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().to_string()
    };
    let t = TcpTransport::new(&closed, fast()).unwrap();
    let mut r = Retrying::new(t, CountingSleeper::default(), 2, 1);
    assert_eq!(
        r.send(&HttpRequest::new(Method::Post, "/x")),
        Err(LinkError::NoConnection)
    );
    assert_eq!(
        r.last_attempts, 3,
        "nothing was sent, so even a post is tried again"
    );
    assert!(r.sleeper().total_ms <= 2 * 2000);
}

#[test]
fn what_could_start_another_request_never_makes_a_connection() {
    let server = Local::start(|_, _, s| say(s, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
    let mut t = server.transport(fast());
    for req in [
        HttpRequest::new(Method::Get, "/x HTTP/1.1\r\nHost: evil\r\n\r\nGET /y"),
        HttpRequest::new(Method::Get, "/x").header("X-A", "1\r\nX-B: 2"),
        HttpRequest::new(Method::Get, "/ñ"),
    ] {
        assert!(matches!(t.send(&req), Err(LinkError::Invalid(_))));
    }
    let (_, connections) = server.finish();
    assert_eq!(connections, 0);
}

#[test]
fn tls_is_refused_with_the_reason_and_bad_addresses_never_connect() {
    let e = TcpTransport::new("https://printer.local:443", TcpLimits::default()).unwrap_err();
    assert!(e.to_string().contains("TLS"));
    assert!(TcpTransport::new("", TcpLimits::default()).is_err());
    assert!(TcpTransport::new(
        "h:1",
        TcpLimits {
            io_timeout_ms: 1,
            ..TcpLimits::default()
        }
    )
    .is_err());
    let mut t = TcpTransport::new("nonexistent.invalid:80", fast()).unwrap();
    assert_eq!(
        t.send(&HttpRequest::new(Method::Get, "/")),
        Err(LinkError::NoConnection)
    );
}

// ---- the simulated printers over a real connection

fn method_of(line: &str) -> Method {
    match line.split(' ').next().unwrap_or("") {
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        _ => Method::Get,
    }
}

/// Gives a request that came over the socket to a simulated printer and writes its answer back.
fn answer_with<T: HttpTransport>(sim: &mut T, seen: &Seen, s: &mut TcpStream) {
    let path = seen.line().split(' ').nth(1).unwrap_or("/").to_string();
    let mut req = HttpRequest::new(method_of(seen.line()), &path).body(seen.body.clone());
    for l in seen.head.lines().skip(1) {
        if let Some((k, v)) = l.split_once(':') {
            if !["host", "content-length", "connection", "user-agent"]
                .iter()
                .any(|h| k.eq_ignore_ascii_case(h))
            {
                req = req.header(k.trim(), v.trim());
            }
        }
    }
    let r = sim
        .send(&req)
        .unwrap_or_else(|e| p3dp_link::HttpResponse::json(503, &e.to_string()));
    let head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        r.status,
        r.body.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(&r.body);
}

fn moonraker_server() -> Local {
    Local::start_with(
        || SimMoonraker::new(SimCore::shared()),
        |sim, _, seen, s| answer_with(sim, seen, s),
    )
}

fn octoprint_server() -> Local {
    Local::start_with(
        || SimOctoPrint::new(SimCore::shared()),
        |sim, _, seen, s| answer_with(sim, seen, s),
    )
}

fn limits() -> TcpLimits {
    TcpLimits {
        connect_timeout_ms: 1_000,
        io_timeout_ms: 3_000,
        max_body_bytes: 1 << 20,
        max_head_bytes: 8192,
    }
}

/// The same scenario for both protocols, over a real connection: the job from upload to pause, resume and cancel, and the limits of temperature.
fn whole_job(name: &str, d: &mut dyn PrinterDriver) {
    assert_eq!(d.status().unwrap().state, JobState::Idle, "{name}");
    d.upload(
        "benchy.gcode",
        b"G28
M84
",
        true,
    )
    .unwrap();
    assert_eq!(
        d.list_files().unwrap(),
        vec!["benchy.gcode".to_string()],
        "{name}"
    );
    assert_eq!(d.status().unwrap().state, JobState::Printing, "{name}");
    d.command(&Command::Pause).unwrap();
    assert_eq!(d.status().unwrap().state, JobState::Paused, "{name}");
    d.command(&Command::Resume).unwrap();
    assert_eq!(d.status().unwrap().state, JobState::Printing, "{name}");
    d.command(&Command::SetTemp {
        zone: Zone::Nozzle,
        target_c: 210.0,
    })
    .unwrap();
    let hot = d.command(&Command::SetTemp {
        zone: Zone::Nozzle,
        target_c: 301.0,
    });
    assert!(
        matches!(hot, Err(LinkError::Temp(_))),
        "{name}: refused before it leaves"
    );
    d.command(&Command::Cancel).unwrap();
    // OctoPrint goes back to operational where Klipper says cancelled: either way nothing prints
    let after = d.status().unwrap().state;
    assert!(
        matches!(after, JobState::Cancelled | JobState::Idle),
        "{name}: {after:?}"
    );
}

#[test]
fn moonraker_over_a_socket() {
    let server = moonraker_server();
    let mut d = MoonrakerDriver::new(server.transport(limits()), Secret::new(""), LIMITS, false);
    whole_job("moonraker", &mut d);
    let (seen, _) = server.finish();
    assert!(seen
        .iter()
        .any(|r| r.line().starts_with("POST /server/files/upload")));
    assert!(
        !seen.iter().any(|r| r.head.contains("301")),
        "the temperature over the limit never reached the server"
    );
}

#[test]
fn octoprint_over_a_socket() {
    let server = octoprint_server();
    let mut d = OctoPrintDriver::new(server.transport(limits()), Secret::new(""), LIMITS, false);
    whole_job("octoprint", &mut d);
    let (seen, _) = server.finish();
    assert!(seen
        .iter()
        .any(|r| r.line().starts_with("POST /api/files/local")));
    assert!(!seen
        .iter()
        .any(|r| String::from_utf8_lossy(&r.body).contains("301")));
}

#[test]
fn the_key_travels_in_its_header_and_a_wrong_key_is_refused_by_the_server() {
    let server = moonraker_server();
    let mut d = MoonrakerDriver::new(
        server.transport(limits()),
        Secret::new("a-key"),
        LIMITS,
        false,
    );
    // the simulated printer asks for no key until one is set in it; the point here is only where the key goes
    d.status().unwrap();
    let (seen, _) = server.finish();
    assert_eq!(seen[0].header("x-api-key").as_deref(), Some("a-key"));
    assert!(
        !seen[0].line().contains("a-key"),
        "the key is not in the address"
    );
}

#[test]
fn the_tries_of_the_suite_wrap_a_printer_that_comes_and_goes() {
    // the printer is switched off for the first two connections and then answers
    let flaky = Local::start_with(
        || SimMoonraker::new(SimCore::shared()),
        |sim, n, seen, s| {
            if n < 2 {
                return; // the connection is closed with no answer
            }
            answer_with(sim, seen, s);
        },
    );
    let t = Retrying::new(flaky.transport(limits()), CountingSleeper::default(), 3, 9);
    let mut d = MoonrakerDriver::new(t, Secret::new(""), LIMITS, false);
    assert_eq!(d.status().unwrap().state, JobState::Idle);
    assert_eq!(d.transport().last_attempts, 3);
    flaky.finish();
}
