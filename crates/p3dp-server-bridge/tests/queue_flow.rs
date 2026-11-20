//! The rules of the queue against the server in memory, over the one client of the suite.

use eh3d_serverclient as sc;
use eh3d_serverclient::sim::Fault;
use p3dp_core::TempLimits;
use p3dp_server_bridge::queue::{board, QueuePolicy};
use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};
use p3dp_server_bridge::{BridgeError, Confirmation, ServerClient, Settings};

const SAFE: &str = "G28\nM140 S60\nM104 S210\nG1 X10 Y10 E1 F1200\nM104 S0\nM140 S0\n";

fn limits() -> TempLimits {
    TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    }
}

fn client() -> (ServerClient, SimPrinterServer) {
    let mut s = Settings::default();
    s.set("server_address", "192.168.0.180:18200").unwrap();
    s.set("server_token", TOKEN).unwrap();
    s.set("server_machine", "workshop").unwrap();
    // the tests fix their own retry settings: one try, no waiting
    s.set("server_retries", "0").unwrap();
    let sim = SimPrinterServer::new();
    let c = ServerClient::new(s, Box::new(sim.clone()))
        .unwrap()
        .with_sleeper(Box::new(sim.auth.sleeper()));
    (c, sim)
}

#[test]
fn a_repeated_name_is_refused_before_anything_is_sent_and_a_new_one_goes_in() {
    let (mut c, sim) = client();
    let p = QueuePolicy::default();
    let a = c
        .enqueue_checked("p1s-1", "benchy", SAFE, &limits(), &p)
        .unwrap();
    assert!(a.waiting());
    let sent = sim.with_state(|s| s.posted.len());
    let e = c
        .enqueue_checked("p1s-1", "Benchy", SAFE, &limits(), &p)
        .unwrap_err();
    match &e {
        BridgeError::Invalid(m) => {
            assert!(m.contains("already waiting") && m.contains(&a.id), "{m}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        sim.with_state(|s| s.posted.len()),
        sent,
        "nothing was posted for the refused job"
    );
    c.enqueue_checked("p1s-1", "cube", SAFE, &limits(), &p)
        .unwrap();
    assert_eq!(c.queue(Some("p1s-1")).unwrap().len(), 2);
    // once the first is cancelled its name is free again
    c.cancel(&a.id).unwrap();
    c.enqueue_checked("p1s-1", "benchy", SAFE, &limits(), &p)
        .unwrap();
}

#[test]
fn a_full_line_is_refused_and_the_board_shows_the_machine_with_what_runs_and_what_waits() {
    let (mut c, _) = client();
    let p = QueuePolicy {
        refuse_duplicates: true,
        max_waiting: 2,
    };
    let a = c
        .enqueue_checked("p1s-1", "a", SAFE, &limits(), &p)
        .unwrap();
    c.enqueue_checked("p1s-1", "b", SAFE, &limits(), &p)
        .unwrap();
    let full = c
        .enqueue_checked("p1s-1", "c", SAFE, &limits(), &p)
        .unwrap_err()
        .to_string();
    assert!(full.contains("2 jobs waiting"), "{full}");
    c.start(&a.id, Confirmation::by_person()).unwrap();
    // one is running now, so there is room for one more
    c.enqueue_checked("p1s-1", "c", SAFE, &limits(), &p)
        .unwrap();
    let b = board(&c.queue(None).unwrap());
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].machine, "p1s-1");
    assert_eq!(b[0].active.len(), 1);
    assert_eq!(b[0].active[0].name, "a");
    assert_eq!(
        b[0].waiting
            .iter()
            .map(|j| j.name.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "c"]
    );
}

#[test]
fn a_failure_of_the_network_while_looking_at_the_queue_stops_the_job_with_the_message_of_the_client(
) {
    let (mut c, sim) = client();
    sim.auth.script(vec![Fault::NoConnection]);
    let e = c
        .enqueue_checked("p1s-1", "x", SAFE, &limits(), &QueuePolicy::default())
        .unwrap_err();
    assert!(
        matches!(
            e,
            BridgeError::Server(sc::ClientError::Unreachable { attempts: 1, .. })
        ),
        "{e:?}"
    );
    assert!(!e.to_string().is_empty());
    assert_eq!(sim.with_state(|s| s.posted.len()), 0);
}
