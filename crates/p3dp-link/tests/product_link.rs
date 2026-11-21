//! The rules of the link at their exact edges: the tables of states, the pauses and the tries again, the helpers of the requests and the multipart bodies.

use p3dp_core::machine::TempError;
use p3dp_core::rng::Rng;
use p3dp_link::backoff::Backoff;
use p3dp_link::retry::{CountingSleeper, Retrying};
use p3dp_link::state_map::{self, OctoFlags};
use p3dp_link::transport::{
    boundary_of, multipart, parse_multipart, percent_decode, percent_encode, safe_file_name, Part,
};
use p3dp_link::{HttpRequest, HttpResponse, HttpTransport, JobState, LinkError, Method};

// ---- the tables of states ----

#[test]
fn moonraker_a_klipper_that_shut_down_is_an_emergency_whatever_the_print_says() {
    for print in ["printing", "standby", "complete", "", "nonsense"] {
        assert_eq!(state_map::moonraker(print, "shutdown"), JobState::Emergency);
        assert_eq!(state_map::moonraker(print, "SHUTDOWN"), JobState::Emergency);
    }
    assert_eq!(state_map::moonraker("printing", "error"), JobState::Error);
    assert_eq!(state_map::moonraker("printing", "startup"), JobState::Busy);
}

#[test]
fn moonraker_the_print_states_have_one_state_each() {
    let table = [
        ("standby", JobState::Idle),
        ("printing", JobState::Printing),
        ("PAUSED", JobState::Paused),
        ("Complete", JobState::Finished),
        ("cancelled", JobState::Cancelled),
        ("error", JobState::Error),
        ("something new", JobState::Busy),
        ("", JobState::Busy),
    ];
    for (print, want) in table {
        assert_eq!(state_map::moonraker(print, "ready"), want, "{print:?}");
    }
}

#[test]
fn octoprint_the_most_serious_flag_wins() {
    let all = OctoFlags {
        operational: true,
        printing: true,
        paused: true,
        pausing: true,
        cancelling: true,
        error: true,
        closed_or_error: true,
    };
    assert_eq!(state_map::octoprint(&all), JobState::Error);
    let f = OctoFlags {
        error: false,
        ..all
    };
    assert_eq!(state_map::octoprint(&f), JobState::Cancelling);
    let f = OctoFlags {
        cancelling: false,
        ..f
    };
    assert_eq!(state_map::octoprint(&f), JobState::Pausing);
    let f = OctoFlags {
        pausing: false,
        ..f
    };
    assert_eq!(state_map::octoprint(&f), JobState::Paused);
    let f = OctoFlags { paused: false, ..f };
    assert_eq!(state_map::octoprint(&f), JobState::Printing);
    let f = OctoFlags {
        printing: false,
        ..f
    };
    assert_eq!(state_map::octoprint(&f), JobState::Idle);
}

#[test]
fn octoprint_a_closed_connection_is_offline_only_when_nothing_is_operational() {
    let closed = OctoFlags {
        closed_or_error: true,
        ..OctoFlags::default()
    };
    assert_eq!(state_map::octoprint(&closed), JobState::Offline);
    let both = OctoFlags {
        operational: true,
        ..closed
    };
    assert_eq!(state_map::octoprint(&both), JobState::Idle);
    assert_eq!(
        state_map::octoprint(&OctoFlags::default()),
        JobState::Offline
    );
}

#[test]
fn prusalink_the_words_of_the_printer_are_read_in_any_case() {
    let table = [
        ("idle", JobState::Idle),
        ("READY", JobState::Idle),
        ("Busy", JobState::Busy),
        ("printing", JobState::Printing),
        ("paused", JobState::Paused),
        ("finished", JobState::Finished),
        ("stopped", JobState::Cancelled),
        ("attention", JobState::Error),
        ("ERROR", JobState::Error),
        ("", JobState::Busy),
        ("unknown", JobState::Busy),
    ];
    for (word, want) in table {
        assert_eq!(state_map::prusalink(word), want, "{word:?}");
    }
}

#[test]
fn reprapfirmware_the_status_words_have_one_state_each() {
    let table = [
        ("idle", JobState::Idle),
        ("processing", JobState::Printing),
        ("Resuming", JobState::Printing),
        ("paused", JobState::Paused),
        ("pausing", JobState::Pausing),
        ("cancelling", JobState::Cancelling),
        ("halted", JobState::Emergency),
        ("off", JobState::Offline),
        ("disconnected", JobState::Offline),
        ("updating", JobState::Updating),
        ("starting", JobState::Preparing),
        ("simulating", JobState::Busy),
    ];
    for (word, want) in table {
        assert_eq!(state_map::rrf(word), want, "{word:?}");
    }
}

#[test]
fn job_states_have_distinct_keys_that_the_display_repeats() {
    let mut keys: Vec<&str> = JobState::ALL.iter().map(|s| s.key()).collect();
    for s in JobState::ALL {
        assert_eq!(s.to_string(), s.key());
    }
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), JobState::ALL.len());
}

#[test]
fn job_states_split_into_active_and_needing_attention() {
    let active: Vec<JobState> = JobState::ALL
        .into_iter()
        .filter(|s| s.is_active())
        .collect();
    assert_eq!(
        active,
        [
            JobState::Preparing,
            JobState::Printing,
            JobState::Pausing,
            JobState::Paused,
            JobState::Finishing,
            JobState::Cancelling
        ]
    );
    let attention: Vec<JobState> = JobState::ALL
        .into_iter()
        .filter(|s| s.needs_attention())
        .collect();
    assert_eq!(
        attention,
        [JobState::Failed, JobState::Error, JobState::Emergency]
    );
    assert!(!JobState::Idle.is_active() && !JobState::Offline.needs_attention());
}

// ---- the pauses ----

#[test]
fn backoff_each_pause_is_between_half_and_the_whole_of_the_doubled_base() {
    let mut b = Backoff::new(200, 2000, 5);
    for (attempt, full) in [
        (0u32, 200u64),
        (1, 400),
        (2, 800),
        (3, 1600),
        (4, 2000),
        (9, 2000),
    ] {
        for _ in 0..50 {
            let d = b.delay_ms(attempt);
            assert!((full / 2..=full).contains(&d), "attempt {attempt}: {d}");
        }
    }
}

#[test]
fn backoff_huge_attempts_and_odd_bases_do_not_overflow() {
    let mut b = Backoff::new(0, 0, 1);
    for attempt in [0, 1, 30, 31, 1000, u32::MAX] {
        let d = b.delay_ms(attempt);
        assert!(d <= 1, "{d}");
    }
    let mut big = Backoff::new(u64::MAX, u64::MAX, 3);
    let d = big.delay_ms(u32::MAX);
    assert!(d >= u64::MAX / 2);
    let mut low = Backoff::new(500, 10, 2);
    assert!(low.delay_ms(5) <= 500);
}

#[test]
fn backoff_the_same_seed_gives_the_same_pauses() {
    let run = |seed: u64| -> Vec<u64> {
        let mut b = Backoff::new(100, 5000, seed);
        (0..12).map(|a| b.delay_ms(a)).collect()
    };
    assert_eq!(run(77), run(77));
    assert_ne!(run(77), run(78));
}

// ---- trying again ----

struct Script {
    answers: Vec<Result<HttpResponse, LinkError>>,
    sent: usize,
}

impl Script {
    fn new(answers: Vec<Result<HttpResponse, LinkError>>) -> Script {
        Script { answers, sent: 0 }
    }
}

impl HttpTransport for Script {
    fn send(&mut self, _req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        self.sent += 1;
        if self.answers.is_empty() {
            Err(LinkError::NoConnection)
        } else {
            self.answers.remove(0)
        }
    }
}

fn ok(status: u16) -> Result<HttpResponse, LinkError> {
    Ok(HttpResponse::json(status, "{}"))
}

fn get() -> HttpRequest {
    HttpRequest::new(Method::Get, "/printer/info")
}

fn post() -> HttpRequest {
    HttpRequest::new(Method::Post, "/printer/gcode/script")
}

#[test]
fn retry_a_connection_that_failed_is_tried_again_and_the_pauses_are_counted() {
    let script = Script::new(vec![
        Err(LinkError::NoConnection),
        Err(LinkError::NoConnection),
        ok(200),
    ]);
    let mut r = Retrying::new(script, CountingSleeper::default(), 3, 11);
    assert_eq!(r.send(&get()).unwrap().status, 200);
    assert_eq!(r.last_attempts, 3);
    assert_eq!(r.sleeper().calls, 2);
    let total = r.sleeper().total_ms;
    assert!((300..=600).contains(&total), "{total}");
}

#[test]
fn retry_a_post_is_tried_again_after_no_connection_but_never_after_a_time_out() {
    let mut a = Retrying::new(
        Script::new(vec![Err(LinkError::NoConnection), ok(200)]),
        CountingSleeper::default(),
        2,
        1,
    );
    assert!(a.send(&post()).is_ok());
    assert_eq!(a.inner().sent, 2);
    let mut b = Retrying::new(
        Script::new(vec![Err(LinkError::Timeout), ok(200)]),
        CountingSleeper::default(),
        2,
        1,
    );
    assert_eq!(b.send(&post()), Err(LinkError::Timeout));
    assert_eq!(b.inner().sent, 1);
    assert_eq!(b.sleeper().calls, 0);
}

#[test]
fn retry_a_get_is_tried_again_after_a_time_out_or_a_bad_gateway() {
    for first in [Err(LinkError::Timeout), ok(502), ok(503), ok(504)] {
        let mut r = Retrying::new(
            Script::new(vec![first, ok(200)]),
            CountingSleeper::default(),
            1,
            4,
        );
        assert_eq!(r.send(&get()).unwrap().status, 200);
        assert_eq!(r.last_attempts, 2);
    }
}

#[test]
fn retry_answers_that_are_not_transient_come_back_at_once() {
    for status in [400u16, 401, 404, 500, 501, 505] {
        let mut r = Retrying::new(
            Script::new(vec![ok(status), ok(200)]),
            CountingSleeper::default(),
            3,
            4,
        );
        assert_eq!(r.send(&get()).unwrap().status, status);
        assert_eq!(r.last_attempts, 1, "{status}");
    }
    let mut r = Retrying::new(
        Script::new(vec![Err(LinkError::Auth), ok(200)]),
        CountingSleeper::default(),
        3,
        4,
    );
    assert_eq!(r.send(&get()), Err(LinkError::Auth));
    assert_eq!(r.sleeper().calls, 0);
}

#[test]
fn retry_a_post_that_got_a_bad_gateway_is_not_sent_twice() {
    let mut r = Retrying::new(
        Script::new(vec![ok(503), ok(200)]),
        CountingSleeper::default(),
        3,
        4,
    );
    assert_eq!(r.send(&post()).unwrap().status, 503);
    assert_eq!(r.inner().sent, 1);
}

#[test]
fn retry_the_number_of_tries_is_capped_at_five_more() {
    let mut r = Retrying::new(Script::new(Vec::new()), CountingSleeper::default(), 99, 4);
    assert_eq!(r.send(&get()), Err(LinkError::NoConnection));
    assert_eq!(r.last_attempts, 6);
    assert_eq!(r.sleeper().calls, 5);
    let mut none = Retrying::new(Script::new(Vec::new()), CountingSleeper::default(), 0, 4);
    assert!(none.send(&get()).is_err());
    assert_eq!(none.last_attempts, 1);
    assert_eq!(none.sleeper().calls, 0);
}

#[test]
fn retry_the_same_seed_waits_the_same_total() {
    let run = |seed: u64| {
        let mut r = Retrying::new(Script::new(Vec::new()), CountingSleeper::default(), 5, seed);
        let _ = r.send(&get());
        r.sleeper().total_ms
    };
    assert_eq!(run(21), run(21));
}

// ---- the requests ----

#[test]
fn methods_say_which_can_be_sent_twice_and_how_they_are_written() {
    assert!(Method::Get.is_idempotent());
    assert!(Method::Put.is_idempotent());
    assert!(Method::Delete.is_idempotent());
    assert!(!Method::Post.is_idempotent());
    let names: Vec<&str> = [Method::Get, Method::Post, Method::Put, Method::Delete]
        .iter()
        .map(|m| m.name())
        .collect();
    assert_eq!(names, ["GET", "POST", "PUT", "DELETE"]);
}

#[test]
fn a_request_keeps_its_headers_in_order_and_finds_them_in_any_case() {
    let r = HttpRequest::new(Method::Put, "/x")
        .header("X-Api-Key", "k1")
        .header("Content-Type", "text/plain")
        .header("x-api-key", "k2")
        .body(b"hi".to_vec());
    assert_eq!(r.header_value("X-API-KEY"), Some("k1"));
    assert_eq!(r.header_value("content-type"), Some("text/plain"));
    assert_eq!(r.header_value("missing"), None);
    assert_eq!(r.headers.len(), 3);
    assert_eq!(r.body, b"hi");
}

#[test]
fn the_route_and_the_query_of_a_path_are_told_apart() {
    let r = HttpRequest::new(Method::Get, "/files?name=a%20b+c&flag&x=1=2&empty=");
    assert_eq!(r.route(), "/files");
    assert_eq!(r.query("name"), Some("a b c".to_string()));
    assert_eq!(r.query("flag"), Some(String::new()));
    assert_eq!(r.query("x"), Some("1=2".to_string()));
    assert_eq!(r.query("empty"), Some(String::new()));
    assert_eq!(r.query("none"), None);
    let plain = HttpRequest::new(Method::Get, "/files");
    assert_eq!(plain.route(), "/files");
    assert_eq!(plain.query("name"), None);
}

#[test]
fn the_first_of_two_equal_keys_of_a_query_wins() {
    let r = HttpRequest::new(Method::Get, "/p?a=1&a=2");
    assert_eq!(r.query("a"), Some("1".to_string()));
}

#[test]
fn a_response_reads_its_body_as_text_even_when_it_is_not_text() {
    let r = HttpResponse::json(200, "{\"a\":1}");
    assert_eq!(r.text(), "{\"a\":1}");
    let bad = HttpResponse {
        status: 200,
        body: vec![b'a', 0xFF, b'b'],
    };
    assert_eq!(bad.text(), "a\u{fffd}b");
}

// ---- percent coding ----

#[test]
fn percent_encoding_keeps_only_the_unreserved_characters() {
    assert_eq!(percent_encode("AZaz09-._~"), "AZaz09-._~");
    assert_eq!(percent_encode("a b/\u{e9}"), "a%20b%2F%C3%A9");
    assert_eq!(percent_encode("+&=?#%"), "%2B%26%3D%3F%23%25");
    assert_eq!(percent_encode(""), "");
}

#[test]
fn percent_decoding_leaves_broken_escapes_as_they_are() {
    assert_eq!(percent_decode("%41"), "A");
    assert_eq!(percent_decode("a%41"), "aA");
    assert_eq!(percent_decode("%zz1"), "%zz1");
    assert_eq!(percent_decode("%4"), "%4");
    assert_eq!(percent_decode("%"), "%");
    assert_eq!(percent_decode("a+b"), "a b");
    assert_eq!(percent_decode("%FF"), "\u{fffd}");
}

#[test]
fn percent_coding_round_trips_texts_from_a_fixed_seed() {
    let alphabet: Vec<char> = "abc XYZ019+&=?#%/\\.-_~\u{e9}\u{4e2d}\u{1f600}\n"
        .chars()
        .collect();
    let mut r = Rng::new(808);
    for _ in 0..300 {
        let n = r.below(20) as usize;
        let s: String = (0..n)
            .map(|_| alphabet[r.below(alphabet.len() as u64) as usize])
            .collect();
        assert_eq!(percent_decode(&percent_encode(&s)), s, "{s:?}");
    }
}

// ---- file names ----

#[test]
fn file_names_that_end_in_a_program_extension_are_allowed() {
    for name in [
        "a.gcode",
        "A.GCODE",
        "x.gco",
        "x.bgcode",
        "y.3mf",
        "\u{e9}.gcode",
        "a b.gcode",
    ] {
        assert_eq!(safe_file_name(name), Ok(name), "{name}");
    }
}

#[test]
fn file_names_have_an_exact_length_limit() {
    let at_limit = format!("{}.gcode", "a".repeat(122));
    assert_eq!(at_limit.len(), 128);
    assert!(safe_file_name(&at_limit).is_ok());
    let over = format!("{}.gcode", "a".repeat(123));
    assert!(safe_file_name(&over).is_err());
    assert!(safe_file_name("").is_err());
}

#[test]
fn file_names_that_could_leave_the_folder_or_hide_something_are_refused() {
    for bad in [
        "a/b.gcode",
        "a\\b.gcode",
        "../a.gcode",
        "a..b.gcode",
        ".",
        "a\u{7}.gcode",
        "a\n.gcode",
        " a.gcode",
        "a.gcode ",
        "a.txt",
        "gcode",
        "a.gcode.exe",
        "a.gcod",
    ] {
        let e = safe_file_name(bad).unwrap_err();
        assert!(matches!(e, LinkError::Invalid(_)), "{bad:?}");
        assert!(e.to_string().contains("not allowed"), "{bad:?}");
    }
}

// ---- multipart bodies ----

fn sample_parts() -> Vec<Part> {
    vec![
        Part {
            name: "path".to_string(),
            filename: None,
            data: b"gcodes".to_vec(),
        },
        Part {
            name: "file".to_string(),
            filename: Some("a b.gcode".to_string()),
            data: b"line1\r\nline2\r\n\r\n".to_vec(),
        },
        Part {
            name: "empty".to_string(),
            filename: None,
            data: Vec::new(),
        },
        Part {
            name: "bin".to_string(),
            filename: Some("x.bin".to_string()),
            data: (0..=255u8).collect(),
        },
    ]
}

#[test]
fn multipart_a_body_reads_back_as_the_parts_that_made_it() {
    let parts = sample_parts();
    let body = multipart("bnd7", &parts).unwrap();
    assert!(body.starts_with(b"--bnd7\r\n"));
    assert!(body.ends_with(b"--bnd7--\r\n"));
    assert_eq!(parse_multipart(&body, "bnd7").unwrap(), parts);
}

#[test]
fn multipart_no_parts_is_just_the_closing_line() {
    let body = multipart("b", &[]).unwrap();
    assert_eq!(body, b"--b--\r\n");
    assert!(parse_multipart(&body, "b").unwrap().is_empty());
}

#[test]
fn multipart_a_boundary_inside_the_data_or_a_quote_in_a_name_is_refused() {
    let in_data = [Part {
        name: "f".to_string(),
        filename: None,
        data: b"xx--bnd7xx".to_vec(),
    }];
    assert!(multipart("bnd7", &in_data).is_err());
    for (name, file) in [
        ("a\"b", None),
        ("a\nb", None),
        ("a", Some("x\".gcode")),
        ("a", Some("x\r.gcode")),
    ] {
        let p = [Part {
            name: name.to_string(),
            filename: file.map(str::to_string),
            data: Vec::new(),
        }];
        assert!(multipart("bnd7", &p).is_err(), "{name:?} {file:?}");
    }
}

#[test]
fn multipart_a_cut_body_is_an_error_and_never_a_panic() {
    let body = multipart("bnd7", &sample_parts()).unwrap();
    for n in 0..body.len() {
        let _ = parse_multipart(&body[..n], "bnd7");
    }
    assert!(parse_multipart(&body[..body.len() - 12], "bnd7").is_err());
    assert!(parse_multipart(b"no boundary at all", "bnd7").is_err());
    assert!(parse_multipart(b"", "bnd7").is_err());
}

#[test]
fn multipart_random_bytes_never_panic_the_reader() {
    let mut r = Rng::new(4242);
    for _ in 0..200 {
        let n = r.below(120) as usize;
        let mut body = b"--b\r\n".to_vec();
        body.extend(r.bytes(n));
        let _ = parse_multipart(&body, "b");
    }
}

#[test]
fn boundary_is_read_from_the_header_with_or_without_quotes() {
    assert_eq!(
        boundary_of("multipart/form-data; boundary=abc123"),
        Some("abc123")
    );
    assert_eq!(
        boundary_of("multipart/form-data; boundary=\"x y\""),
        Some("x y")
    );
    assert_eq!(boundary_of("multipart/form-data; boundary= "), None);
    assert_eq!(boundary_of("multipart/form-data; boundary=\"\""), None);
    assert_eq!(boundary_of("text/plain"), None);
}

// ---- the errors ----

#[test]
fn only_a_connection_a_time_out_and_a_bad_gateway_are_transient() {
    assert!(LinkError::NoConnection.is_transient());
    assert!(LinkError::Timeout.is_transient());
    for status in [502u16, 503, 504] {
        let e = LinkError::Http {
            status,
            detail: String::new(),
        };
        assert!(e.is_transient(), "{status}");
    }
    for status in [400u16, 500, 501, 505] {
        let e = LinkError::Http {
            status,
            detail: String::new(),
        };
        assert!(!e.is_transient(), "{status}");
    }
    for e in [
        LinkError::Auth,
        LinkError::Forbidden,
        LinkError::BadReply("x".to_string()),
        LinkError::Invalid("x".to_string()),
    ] {
        assert!(!e.is_transient(), "{e}");
    }
}

#[test]
fn the_messages_of_the_errors_carry_what_the_person_needs() {
    let e = LinkError::Http {
        status: 409,
        detail: "busy".to_string(),
    };
    assert_eq!(e.to_string(), "the other side answered 409: busy");
    assert!(LinkError::Capability("pause")
        .to_string()
        .contains("cannot pause"));
    assert!(LinkError::Disabled("raw G-code")
        .to_string()
        .contains("switched off"));
    assert!(LinkError::Experimental("tls").to_string().contains("tls"));
    let t: LinkError = TempError::NoChamber(30.0).into();
    assert_eq!(t, LinkError::Temp(TempError::NoChamber(30.0)));
    assert_eq!(t.to_string(), TempError::NoChamber(30.0).to_string());
}
