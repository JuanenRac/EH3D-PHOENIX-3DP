//! The client against the server in memory.

use p3dp_core::rng::Rng;
use p3dp_link::retry::CountingSleeper;
use p3dp_link::sim::Fault;
use p3dp_link::LinkError;
use p3dp_server_bridge::sim::{SimBridgeServer, TOKEN};
use p3dp_server_bridge::{LinkState, ServerClient, Settings};

fn settings(token: &str) -> Settings {
    let mut s = Settings::default();
    s.set("server_address", "192.168.0.180:18200").unwrap();
    s.set("server_token", token).unwrap();
    s.set("server_machine", "workshop").unwrap();
    s
}

fn client(
    token: &str,
) -> (
    ServerClient<SimBridgeServer, CountingSleeper>,
    SimBridgeServer,
) {
    let sim = SimBridgeServer::new();
    let c = ServerClient::new(settings(token), sim.clone(), CountingSleeper::default(), 5).unwrap();
    (c, sim)
}

#[test]
fn the_health_check_sends_no_token_and_the_rest_does() {
    let (mut c, sim) = client(TOKEN);
    assert!(c.health().unwrap());
    assert_eq!(c.link(), LinkState::Connected);
    c.printers().unwrap();
    let seen = sim.state.borrow().auth_seen.clone();
    assert_eq!(seen[0], None, "the token must not go with the health check");
    assert_eq!(seen[1], Some(format!("Bearer {TOKEN}")));
}

#[test]
fn printers_state_and_queue() {
    let (mut c, _) = client(TOKEN);
    let p = c.printers().unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].id, "p1s-1");
    assert_eq!(c.printer_state("mk4-1").unwrap(), "printing");
    assert!(matches!(
        c.printer_state("nope"),
        Err(LinkError::Http { status: 404, .. })
    ));
    assert!(c.printer_state("../x").is_err());
    assert!(c.queue().unwrap().is_empty());
}

#[test]
fn a_job_joins_the_queue_and_never_asks_to_start_by_itself() {
    let (mut c, sim) = client(TOKEN);
    let j = c.enqueue("benchy.gcode", Some("p1s-1")).unwrap();
    assert_eq!(
        (j.file.as_str(), j.printer.as_str(), j.state.as_str()),
        ("benchy.gcode", "p1s-1", "queued")
    );
    assert_eq!(c.queue().unwrap().len(), 1);
    let body = sim.state.borrow().posted[0].clone();
    assert!(
        body.contains("\"auto_start\":false") && body.contains("\"confirm_before_start\":true"),
        "{body}"
    );
    assert!(c.enqueue("../evil.gcode", None).is_err());
    assert!(c.enqueue("a.gcode", Some("a b")).is_err());
    assert!(matches!(
        c.enqueue("a.gcode", Some("ghost")),
        Err(LinkError::Http { status: 404, .. })
    ));
    assert_eq!(
        sim.state.borrow().posted.len(),
        2,
        "the refused ones must not reach the server"
    );
}

#[test]
fn a_bad_token_logs_the_client_out_and_is_not_retried() {
    let (mut c, sim) = client("wrong");
    assert_eq!(c.printers().unwrap_err(), LinkError::Auth);
    assert_eq!(c.link(), LinkState::LoggedOut);
    assert_eq!(sim.state.borrow().requests, 1);
}

#[test]
fn retries_follow_the_rules_of_the_suite() {
    let (mut c, sim) = client(TOKEN);
    sim.state.borrow_mut().faults.push_back(Fault::NoConnection);
    sim.state.borrow_mut().faults.push_back(Fault::Timeout);
    assert!(c.printers().is_ok());
    assert_eq!(c.last_attempts(), 3);
    // the default is two retries: a third failure stops it, and the link says so
    for _ in 0..3 {
        sim.state.borrow_mut().faults.push_back(Fault::NoConnection);
    }
    assert_eq!(c.printers().unwrap_err(), LinkError::NoConnection);
    assert_eq!(c.link(), LinkState::Disconnected);
    // a POST is never sent again after a time-out
    sim.state.borrow_mut().faults.push_back(Fault::Timeout);
    let before = sim.state.borrow().requests;
    assert_eq!(c.enqueue("a.gcode", None).unwrap_err(), LinkError::Timeout);
    assert_eq!(sim.state.borrow().requests, before + 1);
}

#[test]
fn a_server_that_is_busy_is_waited_for_on_reads() {
    let (mut c, sim) = client(TOKEN);
    sim.state
        .borrow_mut()
        .faults
        .push_back(Fault::Status(503, "busy".into()));
    assert!(c.queue().is_ok());
    assert_eq!(c.last_attempts(), 2);
}

#[test]
fn the_server_policy_is_respected() {
    let (_, sim) = client(TOKEN);
    // a hand-made request asking to start by itself is refused by a server whose policy is off, so the program can never trigger it by mistake
    let mut s = SimBridgeServer {
        state: sim.state.clone(),
    };
    let req = p3dp_link::HttpRequest::new(p3dp_link::Method::Post, "/api/v1/printer3d/queue")
        .header("Authorization", &format!("Bearer {TOKEN}"))
        .body(b"{\"file\":\"a.gcode\",\"auto_start\":true}".to_vec());
    use p3dp_link::HttpTransport;
    assert_eq!(s.send(&req).unwrap().status, 403);
}

#[test]
fn without_an_address_there_is_no_client() {
    let r = ServerClient::new(
        Settings::default(),
        SimBridgeServer::new(),
        CountingSleeper::default(),
        1,
    );
    assert!(r.is_err());
}

#[test]
fn an_answer_that_is_too_big_is_refused() {
    let (mut c, sim) = client(TOKEN);
    {
        let mut st = sim.state.borrow_mut();
        for i in 0..40_000 {
            st.queue.push((
                format!("job-{i}"),
                "x".repeat(60) + ".gcode",
                String::new(),
                "queued".into(),
            ));
        }
    }
    let mut s = settings(TOKEN);
    s.set("server_max_answer_mib", "1").unwrap();
    let mut small = ServerClient::new(s, sim.clone(), CountingSleeper::default(), 1).unwrap();
    assert!(matches!(small.queue(), Err(LinkError::BadReply(_))));
    assert_eq!(c.queue().unwrap().len(), 40_000);
}

#[test]
fn random_faults_never_leave_the_client_confused() {
    let mut rng = Rng::new(404);
    let (mut c, sim) = client(TOKEN);
    for _ in 0..300 {
        for _ in 0..rng.below(4) {
            let f = match rng.below(4) {
                0 => Fault::NoConnection,
                1 => Fault::Timeout,
                2 => Fault::Status(503, "busy".into()),
                _ => Fault::Status(500, "boom".into()),
            };
            sim.state.borrow_mut().faults.push_back(f);
        }
        let _ = c.printers();
        let _ = c.queue();
        let _ = c.health();
        sim.state.borrow_mut().faults.clear();
        assert!(c.printers().is_ok());
        assert_eq!(c.link(), LinkState::Connected);
    }
}
