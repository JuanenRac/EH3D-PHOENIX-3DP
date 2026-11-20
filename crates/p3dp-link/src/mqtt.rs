//! The part of MQTT 3.1.1 that a link to a printer needs, as bytes and nothing else: the packets are built and read here and no socket is opened. A client of a printer sends
//! CONNECT (with a user and a password), SUBSCRIBE to one topic, PUBLISH at quality 0 and PINGREQ, and reads CONNACK, SUBACK, PUBLISH and PINGRESP. Quality 1 and 2 are
//! refused on purpose (they need a store of unfinished packets, and a printer does not need them for a state and a few controls).
//!
//! Every length is checked before it is used: a packet longer than `MAX_PACKET`, a topic with a wildcard in a PUBLISH, a string that does not fit the two bytes of its length are
//! errors, and a damaged packet is an error, never a panic. The tests follow the byte examples of the specification.

use crate::error::LinkError;
use p3dp_core::Secret;

/// The biggest packet that is built or read (the whole state of a printer is a few tens of kilobytes).
pub const MAX_PACKET: usize = 1 << 20;

/// What the broker answered to CONNECT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectCode {
    Accepted,
    BadProtocol,
    IdentifierRejected,
    ServerUnavailable,
    BadCredentials,
    NotAuthorized,
}

impl ConnectCode {
    pub fn from_byte(b: u8) -> Result<ConnectCode, LinkError> {
        Ok(match b {
            0 => ConnectCode::Accepted,
            1 => ConnectCode::BadProtocol,
            2 => ConnectCode::IdentifierRejected,
            3 => ConnectCode::ServerUnavailable,
            4 => ConnectCode::BadCredentials,
            5 => ConnectCode::NotAuthorized,
            other => {
                return Err(LinkError::BadReply(format!(
                    "the answer to the connection has the code {other}, which MQTT does not define"
                )))
            }
        })
    }

    /// The error the code stands for; `Ok` for an accepted connection.
    pub fn into_result(self) -> Result<(), LinkError> {
        match self {
            ConnectCode::Accepted => Ok(()),
            ConnectCode::BadCredentials | ConnectCode::NotAuthorized => Err(LinkError::Auth),
            ConnectCode::ServerUnavailable => Err(LinkError::NoConnection),
            ConnectCode::BadProtocol => Err(LinkError::BadReply(
                "the broker does not speak MQTT 3.1.1".to_string(),
            )),
            ConnectCode::IdentifierRejected => Err(LinkError::Invalid(
                "the broker refused the identifier of the client".to_string(),
            )),
        }
    }
}

/// A packet.
#[derive(Clone, PartialEq)]
pub enum Packet {
    Connect {
        client_id: String,
        username: Option<String>,
        password: Option<Secret>,
        keep_alive_s: u16,
    },
    ConnAck {
        session_present: bool,
        code: ConnectCode,
    },
    Publish {
        topic: String,
        payload: Vec<u8>,
        retain: bool,
    },
    Subscribe {
        id: u16,
        topic: String,
    },
    SubAck {
        id: u16,
        /// 0 is granted at quality 0; 0x80 is a refusal.
        granted: u8,
    },
    PingReq,
    PingResp,
    Disconnect,
}

// the password never reaches a log: Secret writes ***
impl std::fmt::Debug for Packet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Packet::Connect {
                client_id,
                username,
                password,
                keep_alive_s,
            } => f
                .debug_struct("Connect")
                .field("client_id", client_id)
                .field("username", username)
                .field("password", password)
                .field("keep_alive_s", keep_alive_s)
                .finish(),
            Packet::ConnAck {
                session_present,
                code,
            } => f
                .debug_struct("ConnAck")
                .field("session_present", session_present)
                .field("code", code)
                .finish(),
            Packet::Publish {
                topic,
                payload,
                retain,
            } => f
                .debug_struct("Publish")
                .field("topic", topic)
                .field("bytes", &payload.len())
                .field("retain", retain)
                .finish(),
            Packet::Subscribe { id, topic } => f
                .debug_struct("Subscribe")
                .field("id", id)
                .field("topic", topic)
                .finish(),
            Packet::SubAck { id, granted } => f
                .debug_struct("SubAck")
                .field("id", id)
                .field("granted", granted)
                .finish(),
            Packet::PingReq => f.write_str("PingReq"),
            Packet::PingResp => f.write_str("PingResp"),
            Packet::Disconnect => f.write_str("Disconnect"),
        }
    }
}

fn invalid(s: impl Into<String>) -> LinkError {
    LinkError::Invalid(s.into())
}

fn bad(s: impl Into<String>) -> LinkError {
    LinkError::BadReply(s.into())
}

/// Writes a string with the two bytes of its length in front, as MQTT does.
fn put_str(out: &mut Vec<u8>, what: &str, s: &str) -> Result<(), LinkError> {
    if s.len() > 65_535 {
        return Err(invalid(format!("the {what} is longer than 65535 bytes")));
    }
    if s.contains('\0') {
        return Err(invalid(format!("the {what} holds a NUL character")));
    }
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

/// The "remaining length" of a packet: seven bits at a time, the high bit saying there is another byte (1 to 4 bytes, up to 268435455).
pub fn encode_length(mut n: usize, out: &mut Vec<u8>) -> Result<(), LinkError> {
    if n > 268_435_455 {
        return Err(invalid("a packet cannot be longer than 268435455 bytes"));
    }
    loop {
        let mut byte = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            return Ok(());
        }
    }
}

/// Reads the remaining length at the start of `buf`: the value and how many bytes it took, or `None` when the bytes are not all there yet.
pub fn decode_length(buf: &[u8]) -> Result<Option<(usize, usize)>, LinkError> {
    let mut value = 0usize;
    for (i, b) in buf.iter().take(4).enumerate() {
        value += usize::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(Some((value, i + 1)));
        }
    }
    if buf.len() >= 4 {
        return Err(bad("the length of a packet takes more than four bytes"));
    }
    Ok(None)
}

fn finish(first: u8, body: Vec<u8>) -> Result<Vec<u8>, LinkError> {
    if body.len() > MAX_PACKET {
        return Err(invalid(format!(
            "a packet of {} bytes is longer than the limit of {MAX_PACKET}",
            body.len()
        )));
    }
    let mut out = vec![first];
    encode_length(body.len(), &mut out)?;
    out.extend(body);
    Ok(out)
}

/// The topic of a PUBLISH or a SUBSCRIBE: not empty, and for a PUBLISH without wildcards.
fn check_topic(topic: &str, publishing: bool) -> Result<(), LinkError> {
    if topic.is_empty() {
        return Err(invalid("a topic cannot be empty"));
    }
    if publishing && (topic.contains('+') || topic.contains('#')) {
        return Err(invalid(
            "a topic that is published to cannot hold a wildcard",
        ));
    }
    Ok(())
}

/// The bytes of a packet.
pub fn encode(p: &Packet) -> Result<Vec<u8>, LinkError> {
    match p {
        Packet::Connect {
            client_id,
            username,
            password,
            keep_alive_s,
        } => {
            if client_id.is_empty() {
                return Err(invalid("the client needs an identifier"));
            }
            if password.is_some() && username.is_none() {
                return Err(invalid("MQTT 3.1.1 sends a password only with a user name"));
            }
            let mut flags = 0x02u8; // a clean session
            if username.is_some() {
                flags |= 0x80;
            }
            if password.is_some() {
                flags |= 0x40;
            }
            let mut body = Vec::new();
            put_str(&mut body, "protocol name", "MQTT")?;
            body.push(4);
            body.push(flags);
            body.extend_from_slice(&keep_alive_s.to_be_bytes());
            put_str(&mut body, "client identifier", client_id)?;
            if let Some(u) = username {
                put_str(&mut body, "user name", u)?;
            }
            if let Some(pw) = password {
                put_str(&mut body, "password", pw.expose())?;
            }
            finish(0x10, body)
        }
        Packet::ConnAck {
            session_present,
            code,
        } => {
            let code = match code {
                ConnectCode::Accepted => 0,
                ConnectCode::BadProtocol => 1,
                ConnectCode::IdentifierRejected => 2,
                ConnectCode::ServerUnavailable => 3,
                ConnectCode::BadCredentials => 4,
                ConnectCode::NotAuthorized => 5,
            };
            finish(0x20, vec![u8::from(*session_present), code])
        }
        Packet::Publish {
            topic,
            payload,
            retain,
        } => {
            check_topic(topic, true)?;
            let mut body = Vec::new();
            put_str(&mut body, "topic", topic)?;
            body.extend_from_slice(payload);
            finish(0x30 | u8::from(*retain), body)
        }
        Packet::Subscribe { id, topic } => {
            check_topic(topic, false)?;
            if *id == 0 {
                return Err(invalid("the identifier of a subscription cannot be 0"));
            }
            let mut body = id.to_be_bytes().to_vec();
            put_str(&mut body, "topic", topic)?;
            body.push(0); // quality 0
            finish(0x82, body)
        }
        Packet::SubAck { id, granted } => {
            let mut body = id.to_be_bytes().to_vec();
            body.push(*granted);
            finish(0x90, body)
        }
        Packet::PingReq => Ok(vec![0xC0, 0]),
        Packet::PingResp => Ok(vec![0xD0, 0]),
        Packet::Disconnect => Ok(vec![0xE0, 0]),
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LinkError> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.buf.len());
        let end = end.ok_or_else(|| bad("a packet is shorter than what it says"))?;
        let out = &self.buf[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u16(&mut self) -> Result<u16, LinkError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn string(&mut self) -> Result<String, LinkError> {
        let n = usize::from(self.u16()?);
        String::from_utf8(self.take(n)?.to_vec())
            .map_err(|_| bad("a string of a packet is not UTF-8"))
    }

    fn rest(&mut self) -> &'a [u8] {
        let out = &self.buf[self.at..];
        self.at = self.buf.len();
        out
    }

    fn done(&self) -> Result<(), LinkError> {
        if self.at == self.buf.len() {
            Ok(())
        } else {
            Err(bad("a packet has bytes left over"))
        }
    }
}

/// Reads one packet from the start of `buf`. `Ok(None)` says the bytes are not all there yet (read more and call again); `Ok(Some((packet, used)))` gives the packet and how many
/// bytes it took. A packet of quality 1 or 2, a type this client does not use, or damage is an error.
pub fn decode(buf: &[u8]) -> Result<Option<(Packet, usize)>, LinkError> {
    let Some(&first) = buf.first() else {
        return Ok(None);
    };
    let Some((len, used)) = decode_length(&buf[1..])? else {
        return Ok(None);
    };
    if len > MAX_PACKET {
        return Err(bad(format!(
            "a packet of {len} bytes is longer than the limit of {MAX_PACKET}"
        )));
    }
    let start = 1 + used;
    let Some(end) = start.checked_add(len).filter(|e| *e <= buf.len()) else {
        return Ok(None);
    };
    let mut r = Reader {
        buf: &buf[start..end],
        at: 0,
    };
    let packet = match first >> 4 {
        1 => {
            if r.string()? != "MQTT" || r.take(1)?[0] != 4 {
                return Err(bad("not MQTT 3.1.1"));
            }
            let flags = r.take(1)?[0];
            let keep_alive_s = r.u16()?;
            let client_id = r.string()?;
            let username = (flags & 0x80 != 0).then(|| r.string()).transpose()?;
            let password = (flags & 0x40 != 0)
                .then(|| r.string().map(|s| Secret::new(&s)))
                .transpose()?;
            Packet::Connect {
                client_id,
                username,
                password,
                keep_alive_s,
            }
        }
        2 => {
            let flags = r.take(1)?[0];
            Packet::ConnAck {
                session_present: flags & 1 == 1,
                code: ConnectCode::from_byte(r.take(1)?[0])?,
            }
        }
        3 => {
            let qos = (first >> 1) & 3;
            if qos != 0 {
                return Err(bad(format!(
                    "a packet of quality {qos} was sent: only quality 0 is used here"
                )));
            }
            let topic = r.string()?;
            Packet::Publish {
                topic,
                payload: r.rest().to_vec(),
                retain: first & 1 == 1,
            }
        }
        8 => {
            let id = r.u16()?;
            let topic = r.string()?;
            r.take(1)?;
            Packet::Subscribe { id, topic }
        }
        9 => Packet::SubAck {
            id: r.u16()?,
            granted: r.take(1)?[0],
        },
        12 => Packet::PingReq,
        13 => Packet::PingResp,
        14 => Packet::Disconnect,
        other => return Err(bad(format!("a packet of type {other} is not used here"))),
    };
    r.done()?;
    Ok(Some((packet, end)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::rng::Rng;

    #[test]
    fn the_length_is_written_in_seven_bits_as_the_specification_shows() {
        for (n, bytes) in [
            (0usize, vec![0x00]),
            (127, vec![0x7F]),
            (128, vec![0x80, 0x01]),
            (16_383, vec![0xFF, 0x7F]),
            (16_384, vec![0x80, 0x80, 0x01]),
            (2_097_151, vec![0xFF, 0xFF, 0x7F]),
            (2_097_152, vec![0x80, 0x80, 0x80, 0x01]),
            (268_435_455, vec![0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut out = Vec::new();
            encode_length(n, &mut out).unwrap();
            assert_eq!(out, bytes, "{n}");
            assert_eq!(decode_length(&bytes).unwrap(), Some((n, bytes.len())));
        }
        assert!(encode_length(268_435_456, &mut Vec::new()).is_err());
        // not all the bytes are there; five bytes of continuation
        assert_eq!(decode_length(&[0x80]).unwrap(), None);
        assert!(decode_length(&[0x80, 0x80, 0x80, 0x80, 0x01]).is_err());
    }

    #[test]
    fn the_packets_are_the_bytes_of_the_examples() {
        let connect = Packet::Connect {
            client_id: "c".to_string(),
            username: None,
            password: None,
            keep_alive_s: 60,
        };
        assert_eq!(
            encode(&connect).unwrap(),
            [0x10, 0x0D, 0, 4, b'M', b'Q', b'T', b'T', 4, 0x02, 0, 60, 0, 1, b'c']
        );
        let publish = Packet::Publish {
            topic: "a/b".to_string(),
            payload: b"hi".to_vec(),
            retain: false,
        };
        assert_eq!(
            encode(&publish).unwrap(),
            [0x30, 0x07, 0, 3, b'a', b'/', b'b', b'h', b'i']
        );
        let sub = Packet::Subscribe {
            id: 1,
            topic: "t".to_string(),
        };
        assert_eq!(encode(&sub).unwrap(), [0x82, 0x06, 0, 1, 0, 1, b't', 0]);
        assert_eq!(encode(&Packet::PingReq).unwrap(), [0xC0, 0]);
        assert_eq!(encode(&Packet::PingResp).unwrap(), [0xD0, 0]);
        assert_eq!(encode(&Packet::Disconnect).unwrap(), [0xE0, 0]);
        let ack = Packet::ConnAck {
            session_present: false,
            code: ConnectCode::Accepted,
        };
        assert_eq!(encode(&ack).unwrap(), [0x20, 0x02, 0, 0]);
    }

    #[test]
    fn a_connect_with_a_user_and_a_password_sets_the_flags_and_hides_the_password() {
        let p = Packet::Connect {
            client_id: "phoenix".to_string(),
            username: Some("bblp".to_string()),
            password: Some(Secret::new("12345678")),
            keep_alive_s: 30,
        };
        let bytes = encode(&p).unwrap();
        assert_eq!(bytes[9], 0xC2, "user, password and a clean session");
        assert!(!format!("{p:?}").contains("12345678"));
        let (back, used) = decode(&bytes).unwrap().unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(back, p);
        // a password with no user is not allowed by 3.1.1
        let no_user = Packet::Connect {
            client_id: "x".to_string(),
            username: None,
            password: Some(Secret::new("pw")),
            keep_alive_s: 30,
        };
        assert!(encode(&no_user).is_err());
        assert!(encode(&Packet::Connect {
            client_id: String::new(),
            username: None,
            password: None,
            keep_alive_s: 1
        })
        .is_err());
    }

    #[test]
    fn every_packet_comes_back_as_it_went_and_a_short_buffer_asks_for_more() {
        let packets = [
            Packet::Connect {
                client_id: "abc".to_string(),
                username: Some("u".to_string()),
                password: Some(Secret::new("p")),
                keep_alive_s: 15,
            },
            Packet::ConnAck {
                session_present: true,
                code: ConnectCode::NotAuthorized,
            },
            Packet::Publish {
                topic: "device/ABCDEFGH/request".to_string(),
                payload: vec![7; 300],
                retain: true,
            },
            Packet::Subscribe {
                id: 513,
                topic: "device/ABCDEFGH/report".to_string(),
            },
            Packet::SubAck {
                id: 513,
                granted: 0,
            },
            Packet::PingReq,
            Packet::PingResp,
            Packet::Disconnect,
        ];
        for p in &packets {
            let b = encode(p).unwrap();
            assert_eq!(decode(&b).unwrap(), Some((p.clone(), b.len())), "{p:?}");
            // cut at every place: it asks for more, it does not panic and it does not guess
            for cut in 0..b.len() {
                assert_eq!(decode(&b[..cut]).unwrap(), None, "{p:?} cut at {cut}");
            }
        }
        // two packets in a row: the first is read and its length is given
        let mut both = encode(&Packet::PingResp).unwrap();
        both.extend(encode(&Packet::Disconnect).unwrap());
        let (first, used) = decode(&both).unwrap().unwrap();
        assert_eq!((first, used), (Packet::PingResp, 2));
    }

    #[test]
    fn a_state_that_is_published_is_read_back_whole() {
        let state = br#"{"print":{"gcode_state":"RUNNING","mc_percent":42}}"#;
        let b = encode(&Packet::Publish {
            topic: "device/ABCDEFGH/report".to_string(),
            payload: state.to_vec(),
            retain: false,
        })
        .unwrap();
        match decode(&b).unwrap().unwrap().0 {
            Packet::Publish { topic, payload, .. } => {
                assert_eq!(topic, "device/ABCDEFGH/report");
                assert_eq!(payload, state);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn what_this_client_does_not_use_or_cannot_trust_is_refused() {
        // quality 1 and 2
        for first in [0x32u8, 0x34] {
            let body = [0, 1, b't', 0, 1];
            let mut b = vec![first, body.len() as u8];
            b.extend(body);
            let e = decode(&b).unwrap_err().to_string();
            assert!(e.contains("only quality 0"), "{e}");
        }
        // a wildcard in a topic that is published to, an empty topic, a NUL, a subscription 0
        for topic in ["a/+", "a/#", "", "a\0b"] {
            let p = Packet::Publish {
                topic: topic.to_string(),
                payload: vec![],
                retain: false,
            };
            assert!(encode(&p).is_err(), "{topic:?}");
        }
        assert!(encode(&Packet::Subscribe {
            id: 0,
            topic: "t".to_string()
        })
        .is_err());
        assert!(encode(&Packet::Subscribe {
            id: 1,
            topic: "device/+/report".to_string()
        })
        .is_ok());
        // a string that is longer than the two bytes of its length can say
        let long = Packet::Publish {
            topic: "x".repeat(70_000),
            payload: vec![],
            retain: false,
        };
        assert!(encode(&long).is_err());
        // a type that is not used, a protocol that is not 3.1.1, a code that is not defined, bytes left over
        assert!(decode(&[0x60, 0]).is_err());
        let mut v3 = encode(&Packet::Connect {
            client_id: "c".to_string(),
            username: None,
            password: None,
            keep_alive_s: 1,
        })
        .unwrap();
        v3[8] = 3;
        assert!(decode(&v3).is_err());
        assert!(decode(&[0x20, 2, 0, 9]).is_err());
        assert!(decode(&[0xC0, 1, 0]).is_err());
        // a packet that says it is longer than the limit is refused before any wait for its bytes
        let mut huge = vec![0x30];
        encode_length(MAX_PACKET + 1, &mut huge).unwrap();
        assert!(decode(&huge).is_err());
    }

    #[test]
    fn the_answer_to_a_connection_is_told_in_the_errors_of_the_link() {
        assert!(ConnectCode::from_byte(0).unwrap().into_result().is_ok());
        assert_eq!(
            ConnectCode::from_byte(4).unwrap().into_result(),
            Err(LinkError::Auth)
        );
        assert_eq!(
            ConnectCode::from_byte(5).unwrap().into_result(),
            Err(LinkError::Auth)
        );
        assert_eq!(
            ConnectCode::from_byte(3).unwrap().into_result(),
            Err(LinkError::NoConnection)
        );
        assert!(ConnectCode::from_byte(1).unwrap().into_result().is_err());
        assert!(ConnectCode::from_byte(2).unwrap().into_result().is_err());
        assert!(ConnectCode::from_byte(6).is_err());
    }

    #[test]
    fn noise_and_changed_packets_never_panic() {
        let mut rng = Rng::new(0x4D51_5454);
        let good = encode(&Packet::Connect {
            client_id: "phoenix".to_string(),
            username: Some("bblp".to_string()),
            password: Some(Secret::new("12345678")),
            keep_alive_s: 30,
        })
        .unwrap();
        for _ in 0..2000 {
            let n = rng.below(60) as usize;
            let _ = decode(&rng.bytes(n));
            let mut b = good.clone();
            for _ in 0..1 + rng.below(3) {
                let i = rng.below(b.len() as u64) as usize;
                b[i] = rng.below(256) as u8;
            }
            let _ = decode(&b);
        }
    }
}
