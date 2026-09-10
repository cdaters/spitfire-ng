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

//! D2 acceptance through the shared disposable native caller fixture.
#[path = "support/caller_board.rs"]
mod caller_board;
use caller_board::{has, Board, MAIN};
use sf_core::RuntimeDatabase;
use std::{
    io::Write,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

#[test]
fn d2_native_existing_new_user_validation_and_bounded_failure() {
    let board = Board::new(2, true);
    let mut existing = board.client();
    existing.existing(b"RETURNING", &board.password, true);
    assert!(!has(&existing.transcript, &board.password));
    existing.goodbye();
    board.claims(0);
    let mut sysop = board.client();
    sysop.existing(b"sysop", &board.sysop_password, true);
    sysop.goodbye();
    board.claims(0);
    let mut new = board.client();
    new.until(b"(Y/N):");
    new.line(b"Y");
    new.until(b"Choose Login");
    for invalid in [
        b"".as_slice(),
        b"bad login",
        b"sysop",
        b"returning",
        &[b'x'; 33],
        &[0xfe],
        b"bad\0id",
    ] {
        new.line(invalid);
        new.until(b"Choose Login");
    }
    let login = &[b'n'; 32];
    assert_eq!(login.len(), 32);
    new.line(login);
    new.until(b"Handle:");
    for invalid in [b"".as_slice(), b"Public Handle", &[b'x'; 31]] {
        new.line(invalid);
        new.until(b"Handle:");
    }
    new.line(b"New Public Caller");
    new.until(b"Choose Password:");
    new.line(&[b'x'; 129]);
    new.until(b"Choose Password:");
    new.line(b"short");
    new.until(b"Confirm Password:");
    new.line(b"short");
    new.until(b"Choose Password:");
    new.line(&board.password);
    new.until(b"Confirm Password:");
    new.line(b"mismatch");
    new.until(b"Choose Password:");
    new.line(&board.password);
    new.until(b"Confirm Password:");
    new.line(&board.password);
    new.until(b"First Name");
    new.line(b"");
    new.until(b"First Name");
    new.line(&[0xfe]);
    new.until(b"First Name");
    new.line(b"Private");
    new.until(b"Last Name");
    new.line(b"Fixture");
    new.until(b"Email:");
    new.line(b"invalid-email");
    new.until(b"Email:");
    new.line(b"");
    let entry = new.until(MAIN);
    assert!(!has(&entry, b"Private Fixture"));
    assert!(!has(&new.transcript, &board.password));
    new.goodbye();
    board.claims(0);
    let db = RuntimeDatabase::open(&board.database).unwrap();
    let caller = db.caller_by_login_identifier(login).unwrap().unwrap();
    assert_eq!(caller.display_name, "New Public Caller");
    assert_eq!(caller.security_level.get(), 10);
    assert_eq!(
        caller.profile.identity.real_name().as_deref(),
        Some("Private Fixture")
    );
    let mut again = board.client();
    again.existing(login, &board.password, true);
    again.goodbye();
    board.claims(0);
    let successes = board.count("SELECT sum(call_count) FROM callers");
    let mut failed = board.client();
    failed.until(b"(Y/N):");
    failed.line(b"N");
    for (index, login) in [b"missing".as_slice(), b"returning", b"disabled"]
        .into_iter()
        .enumerate()
    {
        failed.until(b"Login:");
        failed.line(login);
        failed.until(b"Password:");
        failed.line(b"wrong-attempt");
        if index == 2 {
            let tail = failed.eof(Duration::from_secs(10));
            assert!(!has(&tail, b"MAIN MENU"));
        }
    }
    board.claims(0);
    assert_eq!(
        board.count("SELECT sum(call_count) FROM callers"),
        successes
    );
    let mut disabled = board.client();
    disabled.until(b"(Y/N):");
    disabled.line(b"N");
    disabled.until(b"Login:");
    disabled.line(b"disabled");
    disabled.until(b"Password:");
    disabled.line(&board.password);
    let output = disabled.eof(Duration::from_secs(10));
    assert!(!has(&output, b"MAIN MENU"));
    board.claims(0);
    assert_eq!(
        board.count("SELECT sum(call_count) FROM callers"),
        successes
    );
}

#[test]
fn d2_native_two_nodes_exactly_one_account_winner_and_restart() {
    let mut board = Board::new(2, false);
    let mut clients = [board.client(), board.client()];
    for c in &mut clients {
        c.until(b"Login:");
        c.line(b"returning");
        c.until(b"Password:");
    }
    let barrier = Arc::new(Barrier::new(2));
    let handles = clients
        .into_iter()
        .map(|mut c| {
            let barrier = Arc::clone(&barrier);
            let password = board.password.clone();
            thread::spawn(move || {
                barrier.wait();
                c.line(&password);
                let (_, result) = c.any(&[MAIN, b"already logged in"], Duration::from_secs(15));
                (c, result)
            })
        })
        .collect::<Vec<_>>();
    let mut winner = None;
    let mut denied = 0;
    for h in handles {
        let (mut c, result) = h.join().unwrap();
        if result == 0 {
            assert!(winner.is_none());
            winner = Some(c);
        } else {
            c.eof(Duration::from_secs(10));
            denied += 1;
        }
    }
    assert_eq!(denied, 1);
    board.claims(1);
    assert_eq!(
        board.count("SELECT call_count FROM callers WHERE login_identifier='returning'"),
        1
    );
    assert!(board.count("SELECT sum(reserved_seconds) FROM caller_sessions") <= 120);
    let mut other = board.client();
    other.existing(b"other", &board.password, false);
    board.claims(2);
    assert_eq!(
        board.count("SELECT count(DISTINCT node_id) FROM caller_sessions"),
        2
    );
    assert_eq!(
        board.count("SELECT count(DISTINCT caller_id) FROM caller_sessions"),
        2
    );
    drop(other);
    board.claims(1);
    assert_eq!(board.count("SELECT count(*) FROM caller_sessions s JOIN callers c USING(caller_id) WHERE c.login_identifier='returning'"), 1);
    winner.unwrap().goodbye();
    board.claims(0);
    let mut interrupted = board.client();
    interrupted.existing(b"returning", &board.password, false);
    board.claims(1);
    board.restart();
    drop(interrupted);
    let mut reconnect = board.client();
    reconnect.existing(b"returning", &board.password, false);
    board.claims(1);
    assert_eq!(board.count("SELECT count(*) FROM operational_events WHERE event_code='session.interrupted-recovered'"),1);
    reconnect.goodbye();
    board.claims(0);
}

#[test]
fn d2_native_abandonment_phase_deadlines_idle_and_call_deadline() {
    let board = Board::new(2, true);
    let original = board.count("SELECT count(*) FROM callers");
    let mut abandoned = board.client();
    abandoned.until(b"(Y/N):");
    abandoned.line(b"Y");
    abandoned.until(b"Choose Login");
    abandoned.line(b"abandoned");
    abandoned.until(b"Handle:");
    abandoned.line(b"Abandoned Caller");
    abandoned.until(b"Choose Password:");
    abandoned.stream.write_all(&board.password[..9]).unwrap();
    drop(abandoned);
    board.claims(0);
    {
        // A short login budget belongs only to this no-input timeout case.
        // Interactive registration checks use the normal login budget above.
        let login_board = Board::with_login_timeout(2, true, 10);
        let mut login = login_board.client();
        login.until(b"(Y/N):");
        let timeout = login.eof(Duration::from_secs(15));
        assert!(has(&timeout, b"Login time expired"));
        login_board.claims(0);
        assert_eq!(login_board.count("SELECT sum(call_count) FROM callers"), 0);
    }
    let mut registration = board.client();
    registration.until(b"(Y/N):");
    registration.line(b"Y");
    registration.until(b"Choose Login");
    let timeout = registration.eof(Duration::from_secs(35));
    assert!(has(&timeout, b"Registration time expired"));
    assert_eq!(board.count("SELECT count(*) FROM callers"), original);
    let mut idle = board.client();
    idle.existing(b"other", &board.password, true);
    let output = idle.eof(Duration::from_secs(70));
    assert!(has(&output, b"activity") || has(&output, b"asleep"));
    board.claims(0);
    // Model an account with one second of eligible time remaining, using only
    // disposable fixture data. Native admission and timeout run unchanged.
    let day = sf_core::board_local_day(chrono::Utc::now().timestamp(), chrono_tz::UTC).unwrap();
    rusqlite::Connection::open(&board.database).unwrap().execute(
        "UPDATE callers SET daily_usage_day=?1,daily_time_seconds=119 WHERE login_identifier='returning'",[day]).unwrap();
    let mut limited = board.client();
    limited.existing(b"returning", &board.password, true);
    let output = limited.eof(Duration::from_secs(8));
    assert!(has(&output, b"call time has expired"));
    board.claims(0);
}

#[test]
fn d2_native_registration_cancel_and_profile_eof_are_atomic() {
    let board = Board::new(2, true);
    let original = board.count("SELECT count(*) FROM callers");
    let mut cancelled = board.client();
    cancelled.until(b"(Y/N):");
    cancelled.line(b"Y");
    cancelled.until(b"Choose Login");
    cancelled.line(b"/Q");
    cancelled.eof(Duration::from_secs(10));
    for cancel_profile in [false, true] {
        let mut c = board.client();
        c.until(b"(Y/N):");
        c.line(b"Y");
        c.until(b"Choose Login");
        c.line(b"unfinished");
        c.until(b"Handle:");
        c.line(b"Unfinished Caller");
        c.until(b"Choose Password:");
        c.line(&board.password);
        c.until(b"Confirm Password:");
        c.line(&board.password);
        c.until(b"First Name");
        if cancel_profile {
            c.line(b"/Q");
            c.until(b"Login:");
        } else {
            c.line(b"Private");
            c.until(b"Last Name");
            c.stream.write_all(b"Incomplete").unwrap();
        }
        drop(c);
        board.claims(0);
        assert_eq!(board.count("SELECT count(*) FROM callers"), original);
        assert_eq!(board.count("SELECT sum(call_count) FROM callers"), 0);
    }
}
