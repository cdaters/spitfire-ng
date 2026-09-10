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

//! D3 real loopback Telnet journeys through the native runtime and D2 custody.
#[path = "support/caller_board.rs"]
mod caller_board;
use caller_board::{has, Board, Client, MAIN};
use sf_core::{
    ConferenceAccessMode, ConferenceDefinition, MessageActor, MessageBackend, MessageKind,
    MessageVisibility, NewMessage, RuntimeDatabase, SecurityLevel,
};
use std::{
    io::Write,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

const MENU: &[u8] = b"MESSAGE MENU - Selection?";
const READ: &[u8] = b"[Q]uit:";
fn fixture(minutes: u32, registration: bool) -> Board {
    let board = Board::new(minutes, registration);
    let mut db = RuntimeDatabase::open(&board.database).unwrap();
    for (number, name, read, post) in [
        (3, "Empty", 5, 5),
        (4, "Operators Only", 100, 100),
        (5, "Announcements", 5, 100),
    ] {
        db.ensure_conference(&ConferenceDefinition {
            number,
            name: name.into(),
            description: format!("{name} conference"),
            access_mode: ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(read).unwrap(),
            post_security: SecurityLevel::new(post).unwrap(),
            public_only: true,
            caller_deletion_enabled: true,
            maximum_lines: 25,
            privileged_security_levels: vec![],
            posting_identity: None,
        })
        .unwrap();
    }
    let sysop = db.caller_by_login_identifier(b"sysop").unwrap().unwrap();
    let actor = MessageActor::new(
        sysop.id,
        SecurityLevel::new(sf_core::CallerConfig::default().sysop_security).unwrap(),
    );
    for (number, subject) in [
        (1, "Welcome"),
        (2, "First conversation"),
        (2, "Second conversation"),
        (4, "Operator discussion"),
    ] {
        let conference = db.conference(actor, number).unwrap();
        db.post(
            actor,
            NewMessage {
                identity_preview: None,
                conference_id: conference.id,
                recipient_caller_id: None,
                recipient_name: "All Callers".into(),
                subject: subject.as_bytes().to_vec(),
                body: b"Welcome to this disposable message conference.\r\n".to_vec(),
                created_at: chrono::Utc::now().timestamp(),
                parent_message_id: None,
                visibility: MessageVisibility::Public,
                kind: MessageKind::Standard,
            },
        )
        .unwrap();
    }
    db.replace_queue(actor, &[1, 2, 4]).unwrap();
    board
}
fn enter(client: &mut Client) {
    client.line(b"M");
    client.until(MENU);
}
fn select(client: &mut Client, number: &[u8]) -> Vec<u8> {
    client.line(b"C");
    let list = client.until(b"Conference Number");
    client.line(number);
    client.until(MENU);
    list
}
fn body(client: &mut Client, text: &[u8]) {
    client.until(b"1> ");
    client.line(text);
    client.until(b"2> ");
    client.line(b"/S");
    client.until(b"Save this message?");
}
fn compose(client: &mut Client, subject: &[u8], text: &[u8]) {
    client.line(b"E");
    client.until(b"To (Enter");
    client.line(b"");
    client.until(b"Subject (/A");
    client.line(subject);
    body(client, text);
}
fn scan_new(client: &mut Client) {
    client.line(b"R");
    client.until(b"Conference Scan Selection:");
    client.line(b"N");
    loop {
        let (_, which) = client.any(&[READ, MENU], Duration::from_secs(15));
        if which == 1 {
            break;
        }
        client.line(b"N");
    }
}

#[test]
fn d3_native_existing_read_post_reply_scan_reconnect() {
    let board = fixture(5, false);
    let mut caller = board.client();
    caller.existing(b"returning", &board.password, false);
    enter(&mut caller);
    let list = select(&mut caller, b"2");
    assert!(has(&list, b"General"));
    assert!(!has(&list, b"Operators Only"));
    caller.line(b"B");
    let index = caller.until(MENU);
    assert!(has(&index, b"First conversation"));
    assert!(has(&index, b"UTC"));
    caller.line(b"R");
    caller.until(b"Conference Scan Selection:");
    caller.line(b"T");
    assert!(has(&caller.until(READ), b"First conversation"));
    caller.line(b"N");
    assert!(has(&caller.until(READ), b"Second conversation"));
    caller.line(b"-");
    assert!(has(&caller.until(READ), b"First conversation"));
    caller.line(b"9999");
    assert!(has(&caller.until(READ), b"unavailable"));
    caller.line(b"R");
    caller.until(b"Change message subject?");
    caller.line(b"Y");
    caller.until(b"New Subject:");
    caller.line(b"A changed reply subject");
    caller.until(b"Carbon copy #1:");
    caller.line(b"");
    caller.until(b"non-public/private?");
    caller.line(b"N");
    body(&mut caller, b"This is a durable reply.");
    caller.line(b"Y");
    assert!(has(&caller.until(READ), b"was saved"));
    caller.line(b"F");
    caller.until(b"Thread Selection:");
    caller.line(b"F");
    assert!(has(
        &caller.until(b"Thread Selection:"),
        b"A changed reply subject"
    ));
    caller.line(b"E");
    caller.until(READ);
    caller.line(b"Q");
    caller.until(MENU);
    compose(
        &mut caller,
        b"A local caller post",
        b"One confirmed caller action.",
    );
    caller.line(b"Y");
    caller.line(b"Y"); // second Enter/confirmation is merely another command
    caller.until(MENU);
    // Y may open Your Messages; leave it before the next menu action.
    caller.line(b"Q");
    caller.until(MENU);
    scan_new(&mut caller);
    scan_new(&mut caller);
    caller.line(b"Q");
    caller.until(b"MAIN MENU - Selection?");
    enter(&mut caller);
    caller.line(b"B");
    assert!(has(&caller.until(MENU), b"A local caller post")); // current conference survives Main
    caller.line(b"F");
    caller.until(b"FILE MENU - Selection?");
    caller.line(b"M");
    caller.until(MENU);
    caller.line(b"B");
    assert!(has(&caller.until(MENU), b"A local caller post"));
    caller.goodbye();
    board.claims(0);
    assert_eq!(board.count("SELECT count(*) FROM message_payloads WHERE subject=CAST('A local caller post' AS BLOB)"),1);
    assert_eq!(board.count("SELECT count(*) FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE p.subject=CAST('A changed reply subject' AS BLOB) AND m.parent_message_id IS NOT NULL"),1);
    let pointer=board.count("SELECT max(last_message_number) FROM caller_last_read r JOIN callers c USING(caller_id) WHERE c.login_identifier='returning'");
    assert!(pointer >= 4);
    let mut again = board.client();
    again.existing(b"returning", &board.password, false);
    enter(&mut again);
    select(&mut again, b"2");
    again.line(b"B");
    let persisted = again.until(MENU);
    assert!(has(&persisted, b"A local caller post"));
    assert!(has(&persisted, b"A changed reply subject"));
    again.line(b"R");
    again.until(b"Conference Scan Selection:");
    again.line(b"N");
    assert!(has(&again.until(MENU), b"No new messages"));
    again.goodbye();
    board.claims(0);
}

#[test]
fn d3_native_new_user_and_restricted_sysop() {
    let board = fixture(5, true);
    let mut new = board.client();
    new.until(b"(Y/N):");
    new.line(b"Y");
    new.until(b"Choose Login");
    new.line(b"newmessages");
    new.until(b"Handle:");
    new.line(b"New Message Caller");
    new.until(b"Choose Password:");
    new.line(&board.password);
    new.until(b"Confirm Password:");
    new.line(&board.password);
    new.until(b"First Name");
    new.line(b"Private");
    new.until(b"Last Name");
    new.line(b"Fixture");
    new.until(b"Email:");
    new.line(b"");
    new.until(MAIN);
    enter(&mut new);
    let list = select(&mut new, b"4");
    assert!(!has(&list, b"Operators Only"));
    new.line(b"R");
    new.until(b"Conference Scan Selection:");
    new.line(b"T");
    new.until(READ);
    new.line(b"Q");
    new.until(MENU);
    compose(
        &mut new,
        b"New caller introduction",
        b"Hello from a new caller.",
    );
    new.line(b"Y");
    new.until(MENU);
    assert!(!has(&new.transcript, b"Private Fixture"));
    new.goodbye();
    board.claims(0);
    let mut again = board.client();
    again.existing(b"newmessages", &board.password, true);
    enter(&mut again);
    again.line(b"B");
    assert!(has(&again.until(MENU), b"New caller introduction"));
    again.goodbye();
    board.claims(0);
    assert!(board.count("SELECT count(*) FROM caller_last_read r JOIN callers c USING(caller_id) WHERE c.login_identifier='newmessages' AND last_message_number>0")>0);
    let mut sysop = board.client();
    sysop.existing(b"sysop", &board.sysop_password, true);
    enter(&mut sysop);
    assert!(has(&select(&mut sysop, b"4"), b"Operators Only"));
    sysop.line(b"B");
    assert!(has(&sysop.until(MENU), b"Operator discussion"));
    sysop.line(b"R");
    sysop.until(b"Conference Scan Selection:");
    sysop.line(b"T");
    let (_, prompt) = sysop.any(
        &[b"Preview messages without", READ],
        Duration::from_secs(15),
    );
    if prompt == 0 {
        sysop.line(b"N");
        sysop.until(READ);
    }
    sysop.line(b"Q");
    sysop.until(MENU);
    compose(
        &mut sysop,
        b"Operator followup",
        b"Authorized local policy.",
    );
    sysop.line(b"Y");
    sysop.until(MENU);
    sysop.goodbye();
    board.claims(0);
}

#[test]
fn d3_native_two_nodes_post_and_access_revocation() {
    let board = fixture(5, false);
    let mut a = board.client();
    a.existing(b"returning", &board.password, false);
    enter(&mut a);
    let mut b = board.client();
    b.existing(b"other", &board.password, false);
    enter(&mut b);
    board.claims(2);
    for c in [&mut a, &mut b] {
        c.line(b"R");
        c.until(b"Conference Scan Selection:");
        c.line(b"T");
        c.until(READ);
        c.line(b"Q");
        c.until(MENU);
    }
    compose(&mut a, b"Concurrent A", b"Node A");
    compose(&mut b, b"Concurrent B", b"Node B");
    let gate = Arc::new(Barrier::new(2));
    let workers = [a, b]
        .into_iter()
        .map(|mut c| {
            let gate = gate.clone();
            thread::spawn(move || {
                gate.wait();
                c.line(b"Y");
                c.until(MENU);
                c
            })
        })
        .collect::<Vec<_>>();
    let mut clients = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    let mut b = clients.pop().unwrap();
    let a = clients.pop().unwrap();
    a.goodbye();
    board.claims(1);
    assert_eq!(board.count("SELECT count(DISTINCT message_id) FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE p.subject IN(CAST('Concurrent A' AS BLOB),CAST('Concurrent B' AS BLOB))"),2);
    assert_eq!(board.count("SELECT count(*) FROM caller_last_read r JOIN callers c USING(caller_id) WHERE c.login_identifier IN('returning','other') AND last_message_number=1"),2);
    select(&mut b, b"2");
    compose(&mut b, b"Withdrawn conference", b"Will not be published.");
    let operator = RuntimeDatabase::open(&board.database).unwrap();
    operator.set_conference_enabled(2, false).unwrap();
    let before_withdrawal = board.count("SELECT generation FROM network_preparation");
    b.line(b"Y");
    assert!(has(&b.until(MENU), b"Nothing was saved"));
    assert_eq!(
        board.count("SELECT generation FROM network_preparation"),
        before_withdrawal
    );
    assert_eq!(board.count("SELECT count(*) FROM message_payloads WHERE subject=CAST('Withdrawn conference' AS BLOB)"), 0);
    // Authoritative operator change after a live caller has completed composition.
    compose(&mut b, b"Must not commit", b"Access will be revoked.");
    let conn = rusqlite::Connection::open(&board.database).unwrap();
    conn.execute(
        "UPDATE message_conferences SET post_security=100 WHERE conference_number=1",
        [],
    )
    .unwrap();
    let generation = board.count("SELECT generation FROM network_preparation");
    b.line(b"Y");
    assert!(has(&b.until(MENU), b"Nothing was saved"));
    assert_eq!(
        board.count("SELECT generation FROM network_preparation"),
        generation
    );
    assert_eq!(
        board.count(
            "SELECT count(*) FROM message_payloads WHERE subject=CAST('Must not commit' AS BLOB)"
        ),
        0
    );
    conn.execute(
        "UPDATE message_conferences SET read_security=100 WHERE conference_number=1",
        [],
    )
    .unwrap();
    b.line(b"B");
    assert!(has(&b.until(b"MAIN MENU - Selection?"), b"unavailable"));
    b.goodbye();
    board.claims(0);
}

#[test]
fn d3_native_cancel_malformed_disconnect_and_composition_deadline() {
    let board = fixture(1, false);
    let original = board.count("SELECT count(*) FROM messages");
    let generation = board.count("SELECT generation FROM network_preparation");
    let mut c = board.client();
    c.existing(b"returning", &board.password, false);
    enter(&mut c);
    c.line(b"!");
    assert!(has(&c.until(MENU), b"Invalid"));
    select(&mut c, b"9999");
    select(&mut c, b"bad number");
    for input in [
        vec![b'x'; 31],
        vec![b'x'; 64],
        vec![0xfe],
        b"No such caller".to_vec(),
    ] {
        c.line(b"E");
        c.until(b"To (Enter");
        c.line(&input);
        assert!(has(&c.until(MENU), b"unavailable"));
    }
    select(&mut c, b"3");
    c.line(b"B");
    assert!(has(&c.until(MENU), b"No messages"));
    select(&mut c, b"1");
    c.line(b"E");
    c.until(b"To (Enter");
    c.line(&[b'x'; 65]);
    c.until(b"Please try again:");
    c.line(b"");
    c.until(b"Subject (/A");
    c.line(&[b'x'; 73]);
    c.until(b"Please try again:");
    c.line(b"bad\x1btext");
    c.until(b"Please try again:");
    c.line(b"Cancelled");
    c.until(b"1> ");
    c.line(&[b'x'; 1025]);
    c.until(b"Please try again:");
    c.line(b"/A");
    assert!(has(&c.until(MENU), b"nothing was saved"));
    c.line(b"E");
    c.until(b"To (Enter");
    c.line(b"");
    c.until(b"Subject (/A");
    c.line(b"");
    c.until(MENU);
    c.goodbye();
    board.claims(0);
    let mut reader = board.client();
    reader.existing(b"other", &board.password, false);
    enter(&mut reader);
    reader.line(b"R");
    reader.until(b"Conference Scan Selection:");
    reader.line(b"T");
    reader.until(READ);
    drop(reader);
    board.claims(0);
    for stage in 0..3 {
        let mut c = board.client();
        c.existing(b"other", &board.password, false);
        enter(&mut c);
        c.line(b"E");
        c.until(b"To (Enter");
        if stage > 0 {
            c.line(b"");
            c.until(b"Subject (/A");
        }
        if stage > 1 {
            c.line(b"Abandoned");
            c.until(b"1> ");
        }
        c.stream.write_all(b"unfinished").unwrap();
        drop(c);
        board.claims(0);
    }
    let mut c = board.client();
    c.existing(b"other", &board.password, false);
    enter(&mut c);
    c.line(b"E");
    c.until(b"To (Enter");
    c.line(b"");
    c.until(b"Subject (/A");
    c.line(b"Timed out");
    c.until(b"1> ");
    let end = c.eof(Duration::from_secs(70));
    assert!(has(&end, b"time") || has(&end, b"activity"));
    board.claims(0);
    assert_eq!(board.count("SELECT count(*) FROM messages"), original);
    assert_eq!(
        board.count("SELECT generation FROM network_preparation"),
        generation
    );
}
