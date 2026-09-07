// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.

use super::*;

fn empty_peer() -> Peer {
    let mut p = Peer::new(true);
    p.send.clear();
    p.files = true;
    p
}
fn until(socket: &mut TcpStream, command: Command) -> Vec<u8> {
    for _ in 0..100 {
        if let Frame::Command(id, args) = raw_read(socket) {
            assert_ne!(id, Command::Err as u8, "unexpected protocol rejection");
            if id == command as u8 { return args; }
        }
    }
    panic!("missing expected command")
}
fn drain(socket: &mut TcpStream) {
    socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut bytes = vec![];
    socket.read_to_end(&mut bytes).unwrap();
    // A clean close may have EOBs remaining, never an error.
    let mut offset = 0;
    while offset < bytes.len() {
        let size = usize::from(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) & 0x7fff) + 2;
        let frame = Frame::decode(&bytes[offset..offset + size]).unwrap();
        assert!(!matches!(frame, Frame::Command(id, _) if id == Command::Err as u8));
        offset += size;
    }
}
fn finish(socket: &mut TcpStream) {
    raw_send(socket, Command::Eob, "");
    raw_send(socket, Command::Eob, "");
    drain(socket);
}
fn payload(socket: &mut TcpStream, name: &str, bytes: &[u8]) {
    raw_send(socket, Command::File, &format!("{name} {} 100 0", bytes.len()));
    socket.write_all(&Frame::Data(bytes.to_vec()).encode().unwrap()).unwrap();
    let got = until(socket, Command::Got);
    assert_eq!(std::str::from_utf8(&got).unwrap(), format!("{name} {} 100", bytes.len()));
}
#[test]
fn negotiated_empty_batches_finish_without_wait_or_flood() {
    let (p, r) = scripted_with(empty_peer(), Some("binkp/1.1"), limits(), |s| {
        until(s, Command::Eob);
        raw_send(s, Command::Eob, "");
        until(s, Command::Eob);
        raw_send(s, Command::Eob, "");
        drain(s);
    });
    assert!(r.is_ok());
    assert!(p.received.is_empty());
}
#[test]
fn negotiated_response_after_eob_and_delayed_response_are_received() {
    for delay in [0, 40] {
        let (p, r) = scripted_with(empty_peer(), Some("BINKP/1.1"), limits(), |s| {
            until(s, Command::Eob);
            raw_send(s, Command::Eob, "");
            until(s, Command::Eob);
            std::thread::sleep(Duration::from_millis(delay));
            payload(s, "RESPONSE.TXT", b"bounded reply");
            finish(s);
        });
        assert!(r.is_ok(), "{:?}", r.err());
        assert_eq!(p.received, [b"bounded reply".to_vec()]);
    }
}
#[test]
fn request_ack_can_follow_peer_eob_before_response_batch() {
    let mut p = empty_peer();
    p.send.push(b"EXACT.TXT\r\n".to_vec());
    let (p, r) = scripted_with(p, Some("binkp/1.1"), limits(), |s| {
        let args = until(s, Command::File);
        let offer = Offer::parse(std::str::from_utf8(&args).unwrap(), true).unwrap();
        assert!(matches!(raw_read(s), Frame::Data(_)));
        raw_send(s, Command::Eob, "");
        raw_send(s, Command::Got, &offer.arguments(false));
        until(s, Command::Eob);
        payload(s, "EXACT.TXT", b"response");
        finish(s);
    });
    assert!(r.is_ok(), "{:?}", r.err());
    assert_eq!(p.accepted, 1);
    assert_eq!(p.received, [b"response".to_vec()]);
}
#[test]
fn same_batch_response_and_two_eobs_remain_valid() {
    let (p, r) = scripted_with(empty_peer(), Some("binkp/1.1"), limits(), |s| {
        payload(s, "RESPONSE.TXT", b"same batch");
        finish(s);
    });
    assert!(r.is_ok());
    assert_eq!(p.received.len(), 1);
}
#[test]
fn single_batch_fallback_rejects_post_eob_file_and_late_version_upgrade() {
    for v in [None, Some("binkp/1.0"), Some("binkp/9.9")] {
        let mut p = empty_peer();
        p.send.push(vec![42; 10]); // Keep a matching acknowledgement outstanding.
        let (p, r) = scripted_with(p, v, limits(), |s| {
            until(s, Command::File);
            raw_send(s, Command::Eob, "");
            raw_send(s, Command::Nul, "VER too-late binkp/1.1");
            raw_send(s, Command::File, "LATE.TXT 3 100 0");
            loop { if matches!(raw_read(s), Frame::Command(id, _) if id == Command::Err as u8) { break; } }
        });
        assert_eq!(r.err(), Some(Error::Malformed));
        assert!(p.received.is_empty());
        assert_eq!(p.accepted, 0);
    }
}
#[test]
fn post_eob_invalid_commands_and_unannounced_data_fail_closed() {
    for command in [Some(Command::Adr), Some(Command::Got), None] {
        let (p, r) = scripted_with(empty_peer(), Some("binkp/1.1"), limits(), |s| {
            until(s, Command::Eob);
            raw_send(s, Command::Eob, "");
            until(s, Command::Eob);
            if let Some(command) = command {
                raw_send(s, command, if command == Command::Got { "NOFILE.TXT 3 100" } else { "10:100/1@synthetic" });
            } else { s.write_all(&Frame::Data(vec![1; 3]).encode().unwrap()).unwrap(); }
            loop { if matches!(raw_read(s), Frame::Command(id, _) if id == Command::Err as u8) { break; } }
        });
        assert_eq!(r.err(), Some(Error::Malformed));
        assert!(p.received.is_empty());
    }
}
#[test]
fn close_before_response_keeps_acknowledgement_but_no_payload() {
    let mut p = empty_peer();
    p.send.push(b"EXACT.TXT\r\n".to_vec());
    let (p, r) = scripted_with(p, Some("binkp/1.1"), limits(), |s| {
        let args = until(s, Command::File);
        let offer = Offer::parse(std::str::from_utf8(&args).unwrap(), true).unwrap();
        assert!(matches!(raw_read(s), Frame::Data(_)));
        raw_send(s, Command::Got, &offer.arguments(false));
        until(s, Command::Eob);
    });
    assert_eq!(r.err(), Some(Error::Interrupted));
    assert_eq!(p.accepted, 1);
    assert!(p.received.is_empty());
}
#[test]
fn post_eob_idle_wait_times_out_without_fabricating_response() {
    let short = Limits { handshake: Duration::from_secs(2), idle: Duration::from_secs(1), session: Duration::from_secs(5) };
    let (p, r) = scripted_with(empty_peer(), Some("binkp/1.1"), short, |s| {
        until(s, Command::Eob);
        raw_send(s, Command::Eob, "");
        until(s, Command::Eob);
        loop { if matches!(raw_read(s), Frame::Command(id, _) if id == Command::Err as u8) { break; } }
    });
    assert_eq!(r.err(), Some(Error::Timeout));
    assert!(p.received.is_empty());
}
#[test]
fn nr_offer_requires_matching_zero_restart_before_data_or_eob() {
    for invalid in [false, true] {
        let (p, r) = scripted_with(empty_peer(), Some("binkp/1.1"), limits(), |s| {
            raw_send(s, Command::File, "NR.TXT 3 100 -1");
            assert_eq!(until(s, Command::Get), b"NR.TXT 3 100 0");
            if invalid {
                raw_send(s, Command::Eob, "");
                loop { if matches!(raw_read(s), Frame::Command(id, _) if id == Command::Err as u8) { break; } }
            } else {
                payload(s, "NR.TXT", b"abc");
                finish(s);
            }
        });
        if invalid { assert_eq!(r.err(), Some(Error::Interrupted)); assert!(p.received.is_empty()); }
        else { assert!(r.is_ok()); assert_eq!(p.received, [b"abc".to_vec()]); }
    }
    assert!(Offer::parse("NR.TXT 3 100 -1", true).is_err());
    assert!(file_command("../NR.TXT 3 100 -1", true, true, true).is_err());
    assert!(file_command("NR.TXT 3 100 -2", true, true, true).is_err());
}
#[test]
fn requested_nr_transmission_waits_for_exact_get_and_retains_ack_authority() {
    let mut p = empty_peer(); p.send.push(b"abc".to_vec());
    let (p, r) = scripted_options(p, Some("binkp/1.1"), Some("OPT NR"), limits(), |s| {
        let args = until(s, Command::File);
        assert_eq!(args, b"0.ZIP 3 100 -1");
        // Nothing can be emitted until the matching GET, even with elapsed time.
        s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let mut probe = [0u8; 1];
        assert!(s.read(&mut probe).is_err());
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        raw_send(s, Command::Get, "0.ZIP 3 100 0");
        assert_eq!(until(s, Command::File), b"0.ZIP 3 100 0");
        assert!(matches!(raw_read(s), Frame::Data(_)));
        raw_send(s, Command::Got, "0.ZIP 3 100");
        finish(s);
    });
    assert!(r.is_ok()); assert_eq!(p.accepted, 1);
}
#[test]
fn empty_batch_transition_count_is_finite_and_activity_invalidates_completion() {
    let mut b = Batches { protocol: Protocol::MultipleBatches, ..Batches::default() };
    for _ in 0..2 { b.send_eob().unwrap(); b.receive_eob().unwrap(); }
    assert!(b.complete()); b.activity(); assert!(!b.complete());
    for _ in 2..128 { b.receive_eob().unwrap(); }
    assert_eq!(b.receive_eob().err(), Some(Error::Limit));
}

struct DynamicResponse {
    peer: Peer,
    queued: bool,
    claimed: bool,
}
impl Backend for DynamicResponse {
    fn identify(&mut self, _: &str) -> Result<Plan, Error> { Ok(self.peer.plan()) }
    fn ready(&mut self, a: &[Endpoint], c: &[String], l: u64) -> Result<Vec<Outgoing>, Error> {
        self.peer.ready(a,c,l)
    }
    fn next_batch(&mut self, files: usize, bytes: u64) -> Result<Vec<Outgoing>, Error> {
        if self.queued && !self.claimed {
            assert!(files > 0 && bytes >= 5);
            self.claimed = true;
            Ok(vec![Outgoing { key: "response".into(), offer: Offer { name: "EXACT.TXT".into(), size: 5, time: 100, offset: 0 } }])
        } else { Ok(vec![]) }
    }
    fn load(&mut self, key: &str) -> Result<Vec<u8>, Error> { assert_eq!(key, "response"); Ok(b"reply".to_vec()) }
    fn offered(&mut self, _: &str) -> Result<(), Error> { Ok(()) }
    fn accepted(&mut self, _: &str) -> Result<(), Error> { self.peer.accepted += 1; Ok(()) }
    fn receive(&mut self, bytes: &[u8]) -> Result<(), Error> {
        assert_eq!(bytes, b"EXACT.TXT\r\n"); self.queued=true; self.peer.receive(bytes)
    }
    fn accepts_file(&self, name: &str) -> bool { name == "0.ZIP" }
    fn cancelled(&self) -> bool { false }
}
#[test]
fn newly_queued_response_is_claimed_in_same_session_once_after_empty_send_batch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let mut answer = DynamicResponse { peer: empty_peer(), queued: false, claimed: false };
        let r = run(socket, None, &mut answer, limits());
        (answer,r)
    });
    let mut caller = Peer::new(false);
    caller.files=true; caller.send=vec![b"EXACT.TXT\r\n".to_vec()];
    let socket = TcpStream::connect(addr).unwrap();
    let plan=caller.plan(); let r=run(socket,Some(plan),&mut caller,limits());
    let (answer,a)=handle.join().unwrap();
    assert!(r.is_ok() && a.is_ok(), "caller={:?} answer={:?}",r.err(),a.err());
    assert_eq!(caller.accepted,1); assert_eq!(caller.received,[b"reply".to_vec()]);
    assert!(answer.claimed); assert_eq!(answer.peer.accepted,1);
}
#[test]
fn duplicate_acknowledgement_cannot_complete_a_second_transfer() {
    let mut p=empty_peer(); p.send.push(b"request".to_vec());
    let (p,r)=scripted_with(p,Some("binkp/1.1"),limits(),|s| {
        let args=until(s,Command::File);
        let offer=Offer::parse(std::str::from_utf8(&args).unwrap(),true).unwrap();
        assert!(matches!(raw_read(s),Frame::Data(_)));
        raw_send(s,Command::Got,&offer.arguments(false));
        until(s,Command::Eob);
        raw_send(s,Command::Got,&offer.arguments(false));
        loop { if matches!(raw_read(s),Frame::Command(id,_) if id==Command::Err as u8) {break;} }
    });
    assert_eq!(r.err(),Some(Error::Malformed)); assert_eq!(p.accepted,1);
}

#[test]
fn acknowledged_unanswered_request_closes_with_peer_eobs_straddling_got() {
    let mut p=empty_peer();p.send.push(b"UNGRANTED.TXT\r\n".to_vec());
    let (p,r)=scripted_with(p,Some("binkp/1.1"),limits(),|s| {
        let args=until(s,Command::File);
        let offer=Offer::parse(std::str::from_utf8(&args).unwrap(),true).unwrap();
        assert!(matches!(raw_read(s),Frame::Data(_)));
        // Exact observed native no-response order: one EOB before GOT, one after.
        raw_send(s,Command::Eob,"");
        raw_send(s,Command::Got,&offer.arguments(false));
        until(s,Command::Eob);
        raw_send(s,Command::Eob,"");
        until(s,Command::Eob);
    });
    assert!(r.is_ok(),"{:?}",r.err());
    assert_eq!(p.accepted,1);assert!(p.received.is_empty());
}

#[test]
fn asymmetric_batches_do_not_emit_a_surplus_eob_after_peer_can_close() {
    for _ in 0..8 {
        let mut caller=Peer::new(false); caller.files=true; caller.send=vec![vec![42;100]];
        let (c,a,x,y)=pair(caller,empty_peer());
        assert!(x.is_ok() && y.is_ok(),"caller={:?} answerer={:?}",x.err(),y.err());
        assert_eq!(c.accepted,1);assert_eq!(a.received.len(),1);
        let mut caller=Peer::new(false);caller.test=true;
        let (_,_,x,y)=pair(caller,Peer::new(true));
        assert!(x.is_ok() && y.is_ok(),"caller={:?} answerer={:?}",x.err(),y.err());
    }
}
