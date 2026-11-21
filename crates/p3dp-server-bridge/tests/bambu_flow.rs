//! A Bambu Lab printer in LAN mode through the server: the program makes the machine with the fingerprint of the certificate pinned, never opens TLS itself, and tells the three
//! troubles of the link by the stable codes of the server. Everything runs against the server in memory.

use eh3d_serverclient as sc;
use p3dp_core::TempLimits;
use p3dp_server_bridge::sim::{SimPrinterServer, TOKEN};
use p3dp_server_bridge::{
    normalize_fingerprint, BambuProblem, BridgeError, MachineConnection, ServerClient, Settings,
    SwitchRead,
};

const SAFE: &str = "G28\nM140 S60\nM104 S210\nG1 X10 Y10 E1 F1200\nM104 S0\nM140 S0\n";
/// The fingerprint the simulated printer shows.
const SHOWN: &str = "abababababababababababababababababababababababababababababababab";
const CODE: &str = "k7Qx9pWz";

fn client() -> (ServerClient, SimPrinterServer) {
    let sim = SimPrinterServer::new();
    let mut s = Settings::default();
    s.set("server_address", "192.168.0.180:18200").unwrap();
    s.set("server_token", TOKEN).unwrap();
    let c = ServerClient::new(s, Box::new(sim.clone()))
        .unwrap()
        .with_sleeper(Box::new(sim.auth.sleeper()));
    (c, sim)
}

fn limits() -> TempLimits {
    TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    }
}

fn bambu(host: &str, fingerprint: &str) -> MachineConnection {
    MachineConnection::BambuLan {
        host: host.into(),
        serial: "01P00A123456789".into(),
        fingerprint: fingerprint.into(),
        access_code: CODE.into(),
    }
}

fn colons(hex: &str) -> String {
    hex.as_bytes()
        .chunks(2)
        .map(|p| String::from_utf8_lossy(p).to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

#[test]
fn a_bambu_machine_is_made_with_the_fingerprint_pinned_and_the_code_stays_secret() {
    let (mut c, sim) = client();
    let conn = bambu("sim", &colons(SHOWN));
    let p = c
        .create_machine("a1-lan", "Bambu A1", "bambu_a1", &conn)
        .unwrap();
    assert_eq!(p.connection, "bambu_lan");
    let body = sim.with_state(|s| s.posted[0].clone());
    // the fingerprint goes the way the server keeps it, and the code goes once
    assert!(body.contains("\"kind\":\"bambu_lan\""), "{body}");
    assert!(
        body.contains(&format!("\"fingerprint\":\"{SHOWN}\"")),
        "{body}"
    );
    assert!(body.contains(&format!("\"api_key\":\"{CODE}\"")), "{body}");
    // nothing that is printed holds the code
    assert!(!format!("{conn:?}").contains(CODE));
    assert!(format!("{conn:?}").contains("<hidden>"));
    let moon = MachineConnection::Moonraker {
        url: "http://x:7125".into(),
        api_key: "SECRETKEY".into(),
    };
    assert!(!format!("{moon:?}").contains("SECRETKEY"));
    assert!(!sim
        .with_state(|s| format!("{:?}", s.machines))
        .contains(CODE));
}

#[test]
fn a_machine_without_a_good_fingerprint_is_never_made_and_nothing_is_sent() {
    let (mut c, sim) = client();
    for bad in ["", "abcd", &"zz".repeat(32), &"ab".repeat(33)] {
        let e = c
            .create_machine("a1-lan", "Bambu A1", "bambu_a1", &bambu("sim", bad))
            .unwrap_err();
        assert!(
            matches!(&e, BridgeError::Invalid(t) if t.contains("fingerprint")),
            "{bad:?}: {e}"
        );
    }
    for (host, serial, code) in [
        ("", "01P00A123456789", CODE),
        ("a b", "01P00A123456789", CODE),
        ("sim", "short", CODE),
        ("sim", "01P00A12345678;", CODE),
        ("sim", "01P00A123456789", ""),
    ] {
        let conn = MachineConnection::BambuLan {
            host: host.into(),
            serial: serial.into(),
            fingerprint: SHOWN.into(),
            access_code: code.into(),
        };
        assert!(c.create_machine("a1-lan", "n", "bambu_a1", &conn).is_err());
    }
    assert!(sim.with_state(|s| s.posted.is_empty()), "no request left");
    assert_eq!(
        normalize_fingerprint(&colons(SHOWN)).as_deref(),
        Some(SHOWN)
    );
    assert_eq!(normalize_fingerprint("AB CD"), None);
}

#[test]
fn a_real_address_is_refused_by_the_server_while_its_switch_is_off() {
    let (mut c, sim) = client();
    c.create_machine(
        "a1-lan",
        "Bambu A1",
        "bambu_a1",
        &bambu("192.168.0.60", SHOWN),
    )
    .unwrap();
    let e = c.enqueue("a1-lan", "cube", SAFE, &limits()).unwrap_err();
    assert_eq!(e.code(), Some("bambu_lan_off"));
    assert_eq!(e.bambu_problem(), Some(BambuProblem::LinkOff));
    // the switch is read with its tag, and when the owner turns it on the job goes in
    let SwitchRead::Value { enabled, .. } = c.bambu_lan_switch(None).unwrap() else {
        panic!("a first read has a value");
    };
    assert!(!enabled);
    sim.with_state(|s| {
        s.bambu_lan_enabled = true;
        s.values_version += 1;
    });
    let SwitchRead::Value { enabled, .. } = c.bambu_lan_switch(None).unwrap() else {
        panic!("a first read has a value");
    };
    assert!(enabled);
    assert!(c.enqueue("a1-lan", "cube", SAFE, &limits()).is_ok());
}

#[test]
fn a_certificate_that_is_not_the_pinned_one_is_untrusted_and_nothing_is_queued() {
    let (mut c, sim) = client();
    let other = "cd".repeat(32);
    c.create_machine("a1-lan", "Bambu A1", "bambu_a1", &bambu("sim", &other))
        .unwrap();
    let e = c.enqueue("a1-lan", "cube", SAFE, &limits()).unwrap_err();
    assert_eq!(e.bambu_problem(), Some(BambuProblem::Untrusted));
    assert_eq!(e.code(), Some("printer_untrusted"));
    assert!(sim.with_state(|s| s.jobs.is_empty()));
    // the right fingerprint goes through
    c.create_machine("a1-ok", "Bambu A1", "bambu_a1", &bambu("sim", SHOWN))
        .unwrap();
    assert!(c.enqueue("a1-ok", "cube", SAFE, &limits()).is_ok());
}

#[test]
fn a_printer_that_does_not_answer_is_told_apart() {
    let (mut c, sim) = client();
    c.create_machine("a1-lan", "Bambu A1", "bambu_a1", &bambu("sim", SHOWN))
        .unwrap();
    sim.with_state(|s| {
        s.machines.iter_mut().for_each(|m| {
            if m.id == "a1-lan" {
                m.online = false;
            }
        })
    });
    let e = c.enqueue("a1-lan", "cube", SAFE, &limits()).unwrap_err();
    assert_eq!(e.bambu_problem(), Some(BambuProblem::Disconnected));
    // any other refusal is not a trouble of the Bambu link
    let other = c.printer("nope").unwrap_err();
    assert_eq!(other.bambu_problem(), None);
}

#[test]
fn the_three_codes_are_told_in_eight_languages_and_are_stable() {
    for p in [
        BambuProblem::LinkOff,
        BambuProblem::Untrusted,
        BambuProblem::Disconnected,
    ] {
        let e = BridgeError::Server(sc::ClientError::Server {
            status: if p == BambuProblem::LinkOff { 409 } else { 502 },
            code: Some(p.code().to_string()),
            detail: String::new(),
        });
        assert_eq!(e.bambu_problem(), Some(p));
        let texts: Vec<String> = sc::Lang::ALL.iter().map(|l| e.text(*l)).collect();
        assert!(
            texts.iter().all(|t| !t.is_empty() && t != p.code()),
            "{p:?}"
        );
        for (i, a) in texts.iter().enumerate() {
            for b in &texts[i + 1..] {
                assert_ne!(a, b, "{p:?}: two languages say the same");
            }
        }
    }
    assert_eq!(BambuProblem::LinkOff.code(), "bambu_lan_off");
    assert_eq!(BambuProblem::Untrusted.code(), "printer_untrusted");
    assert_eq!(BambuProblem::Disconnected.code(), "printer_disconnected");
}

#[test]
fn the_switch_comes_back_as_not_changed_while_its_tag_is_the_same() {
    let (mut c, sim) = client();
    let SwitchRead::Value { etag, .. } = c.bambu_lan_switch(None).unwrap() else {
        panic!("a first read has a value");
    };
    let tag = etag.expect("the server gives a tag");
    assert_eq!(
        c.bambu_lan_switch(Some(&tag)).unwrap(),
        SwitchRead::Unchanged
    );
    sim.with_state(|s| {
        s.bambu_lan_enabled = true;
        s.values_version += 1;
    });
    match c.bambu_lan_switch(Some(&tag)).unwrap() {
        SwitchRead::Value { enabled, etag } => {
            assert!(enabled);
            assert_ne!(etag.as_deref(), Some(tag.as_str()));
        }
        SwitchRead::Unchanged => panic!("the tag changed"),
    }
    // without a login the read is refused like every other
    let mut s = Settings::default();
    s.set("server_address", "192.168.0.180:18200").unwrap();
    let mut anon = ServerClient::new(s, Box::new(SimPrinterServer::new())).unwrap();
    assert!(anon.bambu_lan_switch(None).is_err());
}
