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
fn request(b: &mut Board, names: &[&str]) -> String {
    b.db.request_ftn_files(
        &b.policy,
        "operator",
        "up",
        &names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        b.area.native_area,
        NOW,
    )
    .unwrap()
}
fn state(b: &Board) -> FreqRequest {
    b.db.freq_requests().unwrap().remove(0)
}
fn ack(b: &mut Board, at: i64) -> (String, String) {
    let s = session(b, "up");
    let w = work(b, &s);
    assert_eq!(w.len(), 1);
    b.db.file_work_offered(&s, &w[0].0.key).unwrap();
    assert_eq!(state(b).state, FreqState::Sent);
    b.db.file_work_accepted(&s, &w[0].0.key, at).unwrap();
    let key = w[0].0.key.clone();
    close(b, &s);
    (s, key)
}
fn retry(b: &mut Board, id: &str, at: i64) -> Result<()> {
    b.db.retry_ftn_request(&b.policy, "operator", id, state(b).version, at)
}
#[test]
fn freq_pending_acknowledged_and_bounded_same_identity_reissue() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    let initial = state(&b);
    assert_eq!(initial.state, FreqState::Pending);
    assert!(retry(&mut b, &id, NOW + 10000).is_err());
    let (_, key) = ack(&mut b, NOW);
    let acknowledged = state(&b);
    assert_eq!(acknowledged.state, FreqState::Acknowledged);
    assert!(acknowledged.responses[0].file.is_none());
    assert_eq!(acknowledged.retry_after, Some(NOW + 900));
    assert!(retry(&mut b, &id, NOW + 899).is_err());
    retry(&mut b, &id, NOW + 900).unwrap();
    assert!(b
        .db
        .retry_ftn_request(&b.policy, "operator", &id, acknowledged.version, NOW + 901)
        .is_err());
    let after = state(&b);
    assert_eq!(after.request, id);
    assert_eq!(after.link, initial.link);
    assert_eq!(after.created_at, initial.created_at);
    assert_eq!(after.attempts.len(), 2);
    assert_eq!(after.attempts[0].acknowledged_at, Some(NOW));
    assert!(key.starts_with(&id));
    assert_ne!(after.attempts[1].delivery, id);
    assert_eq!(after.responses.len(), 1);
    assert_eq!(after.state, FreqState::Pending);
    assert!(retry(&mut b, &id, NOW + 2000).is_err());
    let count: i64 =
        b.db.connection
            .query_row("SELECT COUNT(*) FROM ftn_freq_inbound", [], |r| r.get(0))
            .unwrap();
    assert_eq!(count, 1);
}
#[test]
fn freq_attempt_limit_cannot_be_reset_and_late_response_can_complete() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    for n in 0..3 {
        ack(&mut b, NOW + n * 900);
        if n < 2 {
            retry(&mut b, &id, NOW + (n + 1) * 900).unwrap();
        }
    }
    assert_eq!(state(&b).state, FreqState::Exhausted);
    assert!(retry(&mut b, &id, NOW + 100000).is_err());
    assert_eq!(state(&b).attempts.len(), 3);
    let s = session(&mut b, "up");
    receive(&mut b, &s, "WANT.TXT", b"late valid answer");
    assert_eq!(state(&b).state, FreqState::Complete);
}
#[test]
fn freq_delayed_first_and_second_response_publish_only_once() {
    for offered in [false, true] {
        let mut b = board();
        let id = request(&mut b, &["WANT.TXT"]);
        ack(&mut b, NOW);
        retry(&mut b, &id, NOW + 900).unwrap();
        let s = session(&mut b, "up");
        if offered {
            let w = work(&mut b, &s);
            b.db.file_work_offered(&s, &w[0].0.key).unwrap();
            b.db.file_work_accepted(&s, &w[0].0.key, NOW + 900).unwrap();
        }
        receive(&mut b, &s, "WANT.TXT", b"only native result");
        let receipt = serde_json::to_string(&state(&b).responses).unwrap();
        assert_eq!(state(&b).state, FreqState::Complete);
        assert!(work(&mut b, &s).is_empty());
        receive(&mut b, &s, "WANT.TXT", b"only native result");
        assert_eq!(
            serde_json::to_string(&state(&b).responses).unwrap(),
            receipt
        );
        assert_eq!(
            b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
                .unwrap(),
            2
        );
        assert_eq!(
            b.db.file_network_status()
                .unwrap()
                .activity
                .iter()
                .filter(|a| a.result == "freq-file-received")
                .count(),
            1
        );
        close(&mut b, &s);
        assert!(retry(&mut b, &id, NOW + 2000).is_err());
    }
}
#[test]
fn freq_wrong_peer_filename_and_unoffered_response_cannot_satisfy_request() {
    let mut b = board();
    request(&mut b, &["WANT.TXT"]);
    let s = session(&mut b, "up");
    assert!(b
        .db
        .receive_file_artifact(
            &b.policy,
            &b.storage,
            &s,
            "WANT.TXT",
            b"before offering",
            NOW,
            &|_, _| false
        )
        .is_err());
    close(&mut b, &s);
    ack(&mut b, NOW);
    let s = session(&mut b, "a");
    receive(&mut b, &s, "WANT.TXT", b"wrong peer");
    close(&mut b, &s);
    let s = session(&mut b, "up");
    receive(&mut b, &s, "OTHER.TXT", b"wrong filename");
    assert!(state(&b).responses[0].file.is_none());
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
}
#[test]
fn freq_partial_reply_reissues_only_unanswered_original_names() {
    let mut b = board();
    let id = request(&mut b, &["ONE.TXT", "TWO.TXT"]);
    ack(&mut b, NOW);
    let s = session(&mut b, "up");
    receive(&mut b, &s, "ONE.TXT", b"one");
    close(&mut b, &s);
    retry(&mut b, &id, NOW + 900).unwrap();
    let s = session(&mut b, "up");
    let w = work(&mut b, &s);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].1, b"TWO.TXT\r\n");
    b.db.file_work_offered(&s, &w[0].0.key).unwrap();
    b.db.file_work_accepted(&s, &w[0].0.key, NOW + 900).unwrap();
    receive(&mut b, &s, "TWO.TXT", b"two");
    assert_eq!(state(&b).state, FreqState::Complete);
    assert_eq!(state(&b).responses.len(), 2);
}
#[test]
fn freq_uniqueness_receipts_and_ack_replay_remain_fail_closed() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    let (old, key) = ack(&mut b, NOW);
    assert!(b
        .db
        .request_ftn_files(
            &b.policy,
            "operator",
            "up",
            &["WANT.TXT".into()],
            b.area.native_area,
            NOW + 1
        )
        .is_err());
    assert!(b.db.connection.execute("INSERT INTO ftn_freq_inbound SELECT request_id||'extra',link_id,name,area_id,NULL,NULL,NULL,NULL FROM ftn_freq_inbound",[]).is_err());
    assert!(b.db.file_work_accepted(&old, &key, NOW + 1).is_err());
    retry(&mut b, &id, NOW + 900).unwrap();
    let s = session(&mut b, "up");
    assert!(b.db.file_work_accepted(&s, &key, NOW + 901).is_err());
    assert_eq!(state(&b).attempts[0].acknowledged_at, Some(NOW));
    assert!(b
        .db
        .connection
        .execute("DELETE FROM ftn_freq_inbound", [])
        .is_err());
    assert!(b
        .db
        .connection
        .execute("DELETE FROM ftn_freq_attempts", [])
        .is_err());
    assert!(b
        .db
        .connection
        .execute(
            "UPDATE ftn_file_deliveries SET payload_accepted=0 WHERE delivery_id=?1",
            [id]
        )
        .is_err());
}
#[test]
fn freq_restart_preserves_attempts_and_transport_budget() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    ack(&mut b, NOW);
    retry(&mut b, &id, NOW + 900).unwrap();
    let backup = b._temp.path().join("restart.sqlite");
    b.db.backup_to(&backup).unwrap();
    b.db = RuntimeDatabase::open(&backup).unwrap();
    b.db.recover_binkp(NOW + 901).unwrap();
    assert_eq!(state(&b).attempts.len(), 2);
    assert_eq!(state(&b).attempts[0].acknowledged_at, Some(NOW));
    let attempt = state(&b).attempts[1].delivery.clone();
    for _ in 0..12 {
        let s = session(&mut b, "up");
        let w = work(&mut b, &s);
        assert_eq!(w.len(), 1);
        b.db.file_work_offered(&s, &w[0].0.key).unwrap();
        close(&mut b, &s);
    }
    let s = session(&mut b, "up");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    assert_eq!(state(&b).state, FreqState::Exhausted);
    let q =
        b.db.file_network_status()
            .unwrap()
            .queue
            .into_iter()
            .find(|q| q.delivery == attempt)
            .unwrap();
    assert!(b
        .db
        .hold_file_delivery("operator", &attempt, q.version, false, NOW + 1000)
        .is_err());
}
#[test]
fn freq_restore_missing_later_attempts_stays_held_and_matching_history_recovers() {
    for later in [false, true] {
        let mut b = board();
        let id = request(&mut b, &["WANT.TXT"]);
        ack(&mut b, NOW);
        let snapshot = b._temp.path().join("snapshot.sqlite");
        b.db.backup_to(&snapshot).unwrap();
        if later {
            retry(&mut b, &id, NOW + 900).unwrap();
            ack(&mut b, NOW + 900);
        }
        let evidence = b.db.ftn_recovery_evidence().unwrap();
        b.db = RuntimeDatabase::open(&snapshot).unwrap();
        b.db.hold_restored_ftn().unwrap();
        assert_eq!(state(&b).state, FreqState::Held);
        assert!(retry(&mut b, &id, NOW + 2000).is_err());
        b.db.reconcile_ftn_recovery(&evidence, "operator", NOW + 2000)
            .unwrap();
        assert_eq!(
            state(&b).state,
            if later {
                FreqState::Held
            } else {
                FreqState::Acknowledged
            }
        );
        assert_eq!(retry(&mut b, &id, NOW + 2000).is_ok(), !later);
    }
}

#[test]
fn freq_schema_26_migration_preserves_acknowledged_request_and_is_atomic() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    ack(&mut b, NOW);
    b.db.connection.execute_batch("DROP TRIGGER ftn_freq_receipt_truth; DROP TRIGGER ftn_freq_receipt_delete; DROP TABLE ftn_freq_attempts; DROP TABLE ftn_freq_recovery; DELETE FROM schema_migrations WHERE version=27;").unwrap();
    assert_eq!(b.db.schema_version().unwrap(), 26);
    b.db.connection.execute_batch("CREATE TABLE ftn_freq_attempts(sentinel TEXT); INSERT INTO ftn_freq_attempts VALUES('retained');").unwrap();
    assert!(b.db.migrate().is_err());
    assert_eq!(b.db.schema_version().unwrap(), 26);
    let absent: bool =
        b.db.connection
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ftn_freq_recovery')",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert!(absent);
    b.db.connection
        .execute_batch("DROP TABLE ftn_freq_attempts;")
        .unwrap();
    let result = b.db.migrate().unwrap();
    assert_eq!(result.applied, 1);
    assert_eq!(result.ending_version, 27);
    let restored = state(&b);
    assert_eq!(restored.request, id);
    assert_eq!(restored.created_at, NOW);
    assert_eq!(restored.attempts.len(), 1);
    assert!(restored.attempts[0].offered);
    assert_eq!(restored.attempts[0].acknowledged_at, Some(NOW));
    assert_eq!(restored.state, FreqState::Acknowledged);
    retry(&mut b, &id, NOW + 900).unwrap();
}
#[test]
fn freq_policy_change_active_session_and_stale_version_cannot_reissue() {
    let mut b = board();
    let id = request(&mut b, &["WANT.TXT"]);
    ack(&mut b, NOW);
    let s = session(&mut b, "a");
    assert!(retry(&mut b, &id, NOW + 900).is_err());
    close(&mut b, &s);
    let mut wrong = b.policy.clone();
    wrong.links[0].remote = "10:100/9@synthetic".parse().unwrap();
    assert!(b
        .db
        .retry_ftn_request(&wrong, "operator", &id, state(&b).version, NOW + 900)
        .is_err());
    let mut disabled = b.policy.clone();
    disabled.links[0].outbound = false;
    assert!(b
        .db
        .retry_ftn_request(&disabled, "operator", &id, state(&b).version, NOW + 900)
        .is_err());
    assert_eq!(state(&b).attempts.len(), 1);
}
