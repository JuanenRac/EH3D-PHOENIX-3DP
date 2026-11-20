//! The client of the printer plugin against the server in memory, over the one client of the suite.

use eh3d_serverclient as sc;
use eh3d_serverclient::sim::Fault;
use p3dp_core::TempLimits;
use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};
use p3dp_server_bridge::{
    server_model_for, stricter, BridgeError, Confirmation, LinkState, MachineConnection,
    ServerClient, Settings,
};

const SAFE: &str = "G28\nM140 S60\nM104 S210\nG1 X10 Y10 E1 F1200\nM104 S0\nM140 S0\n";

fn settings(token: &str) -> Settings {
    let mut s = Settings::default();
    s.set("server_address", "192.168.0.180:18200").unwrap();
    s.set("server_token", token).unwrap();
    s.set("server_machine", "workshop").unwrap();
    s
}

fn client(token: &str) -> (ServerClient, SimPrinterServer) {
    let sim = SimPrinterServer::new();
    let c = ServerClient::new(settings(token), Box::new(sim.clone()))
        .unwrap()
        .with_sleeper(Box::new(sim.auth.sleeper()));
    (c, sim)
}

fn p1s_limits() -> TempLimits {
    TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    }
}

#[test]
fn the_health_check_sends_no_token_and_the_rest_does() {
    let (mut c, sim) = client(TOKEN);
    assert!(c.health().unwrap());
    assert_eq!(c.link(), LinkState::Connected);
    c.printers().unwrap();
    let seen = sim.auth.seen();
    assert_eq!(seen[0].path, "/api/v1/health");
    assert!(
        !seen[0].had_token,
        "the token must not go with the health check"
    );
    assert!(seen[1].had_token && seen[1].token_valid);
}

#[test]
fn only_the_3d_printers_are_listed_with_what_the_server_says_of_them() {
    let (mut c, _) = client(TOKEN);
    let p = c.printers().unwrap();
    assert_eq!(
        p.len(),
        2,
        "the router of the server is not a printer of this program"
    );
    assert_eq!(
        (p[0].id.as_str(), p[0].model.as_str()),
        ("p1s-1", "bambu_p1s")
    );
    assert!(p[0].online && !p[0].latched && !p[0].auto_start_queue);
    assert_eq!(p[0].limits, Some(p1s_limits()));
    assert_eq!(p[0].state, "idle");
    assert!(!p[1].online);
    assert_eq!(p[1].limits.unwrap().chamber_c, Some(60.0));
    assert_eq!(c.printer("p1s-1").unwrap().name, "Bambu P1S");
}

#[test]
fn a_machine_that_is_not_a_printer_or_does_not_exist_is_refused() {
    let (mut c, _) = client(TOKEN);
    assert!(matches!(
        c.printer("router-1"),
        Err(BridgeError::Invalid(_))
    ));
    let e = c.printer("nope").unwrap_err();
    assert_eq!(e.code(), Some("unknown_machine"));
    assert!(matches!(c.printer("../x"), Err(BridgeError::Invalid(_))));
    assert!(matches!(c.printer("a b"), Err(BridgeError::Invalid(_))));
    assert!(matches!(c.printer(""), Err(BridgeError::Invalid(_))));
}

#[test]
fn the_refusal_of_the_server_is_told_in_eight_languages() {
    let (mut c, _) = client(TOKEN);
    let e = c.printer("nope").unwrap_err();
    let texts: Vec<String> = sc::Lang::ALL.iter().map(|l| e.text(*l)).collect();
    assert!(texts
        .iter()
        .all(|t| !t.is_empty() && t != "unknown_machine"));
    for (i, a) in texts.iter().enumerate() {
        for b in &texts[i + 1..] {
            assert_ne!(a, b, "two languages say the same");
        }
    }
}

#[test]
fn heaters_and_layer_are_read() {
    let (mut c, _) = client(TOKEN);
    let h = c.temperatures("p1s-1").unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!((h[0].name.as_str(), h[0].limit_c), ("hotend", 300.0));
    assert_eq!(c.temperatures("mk4-1").unwrap().len(), 3);
    let l = c.layer("p1s-1").unwrap();
    assert_eq!((l.layer, l.layers), (12, 100));
    assert_eq!(l.eta_s, Some(1800.0));
}

#[test]
fn a_job_joins_the_queue_and_waits_for_a_person() {
    let (mut c, sim) = client(TOKEN);
    let j = c.enqueue("p1s-1", "benchy", SAFE, &p1s_limits()).unwrap();
    assert!(j.waiting() && !j.finished());
    assert_eq!((j.machine.as_str(), j.position), ("p1s-1", Some(0)));
    let body = sim.with_state(|s| s.posted[0].clone());
    assert!(
        body.contains("\"kind\":\"print\"") && body.contains("\"preheat\":false"),
        "{body}"
    );
    assert!(
        !body.contains("auto_start"),
        "the program does not ask for the queue to start by itself: {body}"
    );
    assert_eq!(c.queue(Some("p1s-1")).unwrap().len(), 1);
    assert_eq!(c.queue(None).unwrap()[0].state, "queued");
    assert_eq!(c.job(&j.id).unwrap().state, "queued");
    // nothing started by itself
    assert_eq!(c.printer("p1s-1").unwrap().state, "idle");
    let started = c.start(&j.id, Confirmation::by_person()).unwrap();
    assert_eq!(started.state, "running");
    assert_eq!(c.printer("p1s-1").unwrap().state, "busy");
}

#[test]
fn a_job_can_be_held_resumed_and_cancelled_and_only_one_runs_at_a_time() {
    let (mut c, _) = client(TOKEN);
    let a = c.enqueue("p1s-1", "a", SAFE, &p1s_limits()).unwrap();
    let b = c.enqueue("p1s-1", "b", SAFE, &p1s_limits()).unwrap();
    assert_eq!(b.position, Some(1));
    c.start(&a.id, Confirmation::by_person()).unwrap();
    let busy = c.start(&b.id, Confirmation::by_person()).unwrap_err();
    assert_eq!(busy.code(), Some("machine_busy"));
    assert_eq!(c.pause(&a.id).unwrap().state, "paused");
    assert_eq!(c.resume(&a.id).unwrap().state, "running");
    assert!(c.cancel(&a.id).unwrap().finished());
    let again = c.cancel(&a.id).unwrap_err();
    assert_eq!(again.code(), Some("not_in_state"));
    assert_eq!(
        c.start("w-99", Confirmation::by_person())
            .unwrap_err()
            .code(),
        Some("unknown_job")
    );
}

#[test]
fn a_program_over_the_limits_is_refused_with_its_line_and_nothing_is_sent() {
    let (mut c, sim) = client(TOKEN);
    let hot = "G28\nM104 S210\nM109 S301\n";
    let e = c.enqueue("p1s-1", "hot", hot, &p1s_limits()).unwrap_err();
    assert!(matches!(e, BridgeError::Temp { line: 3, .. }), "{e:?}");
    assert!(e.to_string().contains("301") && e.to_string().contains("line 3"));
    // a chamber on a printer that has none, and a bed over the limit
    assert!(c
        .enqueue("p1s-1", "x", "M141 S40\n", &p1s_limits())
        .is_err());
    assert!(c
        .enqueue("p1s-1", "x", "M140 S101\n", &p1s_limits())
        .is_err());
    assert_eq!(
        sim.with_state(|s| s.posted.len()),
        0,
        "what is refused must not reach the server"
    );
    assert!(c.queue(None).unwrap().is_empty());
}

#[test]
fn the_stricter_limits_of_the_program_and_of_the_server_are_the_ones_that_count() {
    let (mut c, _) = client(TOKEN);
    let server = c.printer("mk4-1").unwrap().limits.unwrap();
    let local = TempLimits {
        nozzle_c: 250.0,
        bed_c: 120.0,
        chamber_c: Some(80.0),
    };
    let both = stricter(&local, &server);
    assert_eq!(
        (both.nozzle_c, both.bed_c, both.chamber_c),
        (250.0, 100.0, Some(60.0))
    );
    // a printer with no chamber on either side has none
    assert_eq!(stricter(&local, &p1s_limits()).chamber_c, None);
    let warm = "M104 S255
";
    assert!(c.enqueue("mk4-1", "x", warm, &server).is_ok());
    assert!(matches!(
        c.enqueue("mk4-1", "x", warm, &both),
        Err(BridgeError::Temp { line: 1, .. })
    ));
}

#[test]
fn values_that_cannot_be_sent_are_refused_before_the_network() {
    let (mut c, sim) = client(TOKEN);
    let l = p1s_limits();
    assert!(matches!(
        c.enqueue("../p1s", "a", SAFE, &l),
        Err(BridgeError::Invalid(_))
    ));
    assert!(matches!(
        c.enqueue("p1s-1", "", SAFE, &l),
        Err(BridgeError::Invalid(_))
    ));
    assert!(matches!(
        c.enqueue("p1s-1", "a\nb", SAFE, &l),
        Err(BridgeError::Invalid(_))
    ));
    assert!(matches!(
        c.enqueue("p1s-1", &"n".repeat(81), SAFE, &l),
        Err(BridgeError::Invalid(_))
    ));
    assert!(matches!(
        c.enqueue("p1s-1", "a", "  \n ", &l),
        Err(BridgeError::Invalid(_))
    ));
    assert!(matches!(c.job("a/b"), Err(BridgeError::Invalid(_))));
    assert!(matches!(
        c.emergency_stop("a b"),
        Err(BridgeError::Invalid(_))
    ));
    assert_eq!(sim.auth.request_count(), 0);
    let mut small = c.with_max_program_bytes(10);
    assert!(matches!(
        small.enqueue("p1s-1", "a", SAFE, &l),
        Err(BridgeError::Invalid(_))
    ));
    assert_eq!(sim.auth.request_count(), 0);
}

#[test]
fn the_server_refuses_what_is_over_its_own_limits_too() {
    let (mut c, _) = client(TOKEN);
    // the program's limits are wider than the server's for this printer: the server is the second door
    let wide = TempLimits {
        nozzle_c: 400.0,
        bed_c: 120.0,
        chamber_c: Some(80.0),
    };
    let e = c.enqueue("mk4-1", "x", "M104 S290\n", &wide).unwrap_err();
    assert_eq!(e.code(), Some("program_over_limit"));
    assert!(matches!(e, BridgeError::Server(_)));
}

#[test]
fn the_emergency_stop_latches_until_it_is_released() {
    let (mut c, _) = client(TOKEN);
    let j = c.enqueue("p1s-1", "a", SAFE, &p1s_limits()).unwrap();
    c.start(&j.id, Confirmation::by_person()).unwrap();
    assert!(
        c.emergency_stop("p1s-1").unwrap(),
        "the simulated printer was reached"
    );
    assert!(c.job(&j.id).unwrap().finished());
    let p = c.printer("p1s-1").unwrap();
    assert!(p.latched && p.state == "busy");
    let e = c.enqueue("p1s-1", "b", SAFE, &p1s_limits()).unwrap_err();
    assert_eq!(e.code(), Some("printer_latched"));
    c.release("p1s-1").unwrap();
    assert!(!c.printer("p1s-1").unwrap().latched);
    assert!(c.enqueue("p1s-1", "b", SAFE, &p1s_limits()).is_ok());
    // a printer that does not answer is still latched, and the answer says it was not reached
    assert!(!c.emergency_stop("mk4-1").unwrap());
}

#[test]
fn a_network_that_fails_is_tried_again_but_a_post_is_never_sent_twice() {
    let (mut c, sim) = client(TOKEN);
    sim.auth.script(vec![Fault::Timeout, Fault::NoConnection]);
    assert_eq!(c.printers().unwrap().len(), 2, "a read is tried again");
    assert_eq!(c.link(), LinkState::Connected);
    let before = sim.auth.request_count();
    sim.auth.script(vec![Fault::Timeout]);
    let e = c.enqueue("p1s-1", "a", SAFE, &p1s_limits()).unwrap_err();
    assert!(matches!(
        e,
        BridgeError::Server(sc::ClientError::Unreachable { attempts: 1, .. })
    ));
    assert_eq!(c.link(), LinkState::Disconnected);
    assert_eq!(sim.with_state(|s| s.posted.len()), 0);
    assert_eq!(
        sim.auth.request_count(),
        before,
        "the timed-out post reached nobody"
    );
    // the server is back
    assert!(c.enqueue("p1s-1", "a", SAFE, &p1s_limits()).is_ok());
    assert_eq!(c.link(), LinkState::Connected);
}

#[test]
fn a_busy_server_is_waited_for_on_a_read() {
    let (mut c, sim) = client(TOKEN);
    sim.auth.script(vec![
        Fault::Status(503, "unavailable"),
        Fault::Status(503, "unavailable"),
    ]);
    assert!(c.queue(None).is_ok());
    assert!(!sim.auth.pauses().is_empty());
    assert!(sim.auth.pauses().iter().all(|p| p.as_millis() <= 2000));
}

#[test]
fn a_wrong_token_is_a_logged_out_link_and_asks_for_a_login() {
    let (mut c, _) = client("not-the-token");
    let e = c.printers().unwrap_err();
    assert!(e.needs_login());
    assert_eq!(c.link(), LinkState::LoggedOut);
    // with no token at all the call does not even leave
    let (mut none, sim) = client("");
    assert!(none.queue(None).unwrap_err().needs_login());
    assert_eq!(sim.auth.request_count(), 0);
}

#[test]
fn a_person_logs_in_once_and_the_token_never_shows() {
    let (mut c, sim) = client("");
    let s = c.login("admin", "secret-password").unwrap();
    assert_eq!((s.role.as_str(), s.app.as_str()), ("admin", "phoenix-3dp"));
    assert_eq!(c.printers().unwrap().len(), 2);
    assert_eq!(sim.auth.open_logins(), 1);
    let shown = format!("{c:?}", c = c.settings());
    assert!(!shown.contains("secret-password"));
    let bad = c.login("admin", "wrong").unwrap_err();
    assert!(!bad.to_string().contains("wrong"));
    let (with_token, _) = client(TOKEN);
    assert!(!format!("{:?}", with_token.settings()).contains(TOKEN));
}

#[test]
fn a_machine_is_made_with_the_model_of_its_profile() {
    let (mut c, sim) = client(TOKEN);
    let p = c
        .create_machine(
            "a1-2",
            "Bambu A1",
            server_model_for("bambu-lab-a1"),
            &MachineConnection::Demo,
        )
        .unwrap();
    assert_eq!(
        (p.model.as_str(), p.connection.as_str()),
        ("bambu_a1", "demo")
    );
    assert_eq!(p.limits, Some(p1s_limits()));
    let body = sim.with_state(|s| s.posted[0].clone());
    assert!(
        body.contains("\"plugin\":\"printer3d\"") && body.contains("\"model\":\"bambu_a1\""),
        "{body}"
    );
    let again = c
        .create_machine("a1-2", "Bambu A1", "bambu_a1", &MachineConnection::Demo)
        .unwrap_err();
    assert_eq!(again.code(), Some("machine_exists"));
    assert!(c
        .create_machine("../x", "n", "generic", &MachineConnection::Demo)
        .is_err());
    assert!(c
        .create_machine("x1", " ", "generic", &MachineConnection::Demo)
        .is_err());
    let moon = MachineConnection::Moonraker {
        url: "http://192.168.0.50:7125".into(),
        api_key: "k".into(),
    };
    assert_eq!(
        c.create_machine("voron", "Voron", "generic", &moon)
            .unwrap()
            .connection,
        "moonraker"
    );
}

#[test]
fn models_of_the_catalog_map_to_the_models_of_the_server_and_the_rest_is_generic() {
    assert_eq!(server_model_for("bambu-lab-a1"), "bambu_a1");
    assert_eq!(server_model_for("bambu-lab-a1-mini"), "bambu_a1_mini");
    assert_eq!(server_model_for("bambu-lab-p1s"), "bambu_p1s");
    assert_eq!(server_model_for("prusa-mk4s"), "generic");
    assert_eq!(server_model_for(""), "generic");
}

#[test]
fn a_client_needs_an_address_and_values_the_suite_accepts() {
    let mut s = Settings::default();
    let no_address = ServerClient::new(s.clone(), Box::new(SimPrinterServer::new()));
    assert!(matches!(no_address, Err(BridgeError::Invalid(_))));
    s.set("server_address", "10.0.0.2:18200").unwrap();
    assert!(ServerClient::new(s.clone(), Box::new(SimPrinterServer::new())).is_ok());
    let link = p3dp_server_bridge::client::link_settings(&s).unwrap();
    assert_eq!(link.server_address, "10.0.0.2:18200");
    assert_eq!((link.server_timeout_s, link.server_retries), (15, 2));
    assert!(!link.server_mdns);
}
