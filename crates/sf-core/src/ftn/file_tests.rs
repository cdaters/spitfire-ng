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
use crate::{FileAccessMode, FileAreaDefinition, LogicalPaths, RuntimeConfig, SecurityLevel};
const NOW: i64 = 1_788_609_600;
struct Board {
    _temp: tempfile::TempDir,
    db: RuntimeDatabase,
    storage: FileStorage,
    policy: Policy,
    area: FileEchoArea,
    file: i64,
}
fn board() -> Board {
    let temp = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
    let paths = LogicalPaths::resolve(temp.path(), &config).unwrap();
    paths.create_directories().unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    db.migrate().unwrap();
    let storage = FileStorage::new(&paths).unwrap();
    let area = db
        .create_file_area(&FileAreaDefinition {
            number: 1,
            name: "Public".into(),
            description: "Synthetic".into(),
            storage_key: "public".into(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            upload_security: SecurityLevel::new(10).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: vec![],
        })
        .unwrap();
    let file = storage
        .write_seed_file(
            &mut db,
            &area,
            "LOCAL.ZIP",
            "Synthetic archive; opaque",
            b"synthetic opaque bytes",
            NOW,
        )
        .unwrap()
        .id
        .get();
    let local: Endpoint = "10:100/1@synthetic".parse().unwrap();
    let policy = Policy {
        enabled: true,
        akas: vec![Aka {
            id: "local".into(),
            endpoint: local,
            enabled: true,
            primary: true,
        }],
        links: [("up", 2), ("a", 3), ("b", 4)]
            .into_iter()
            .map(|(id, node)| Link {
                posting_identity: Default::default(),
                id: id.into(),
                remote: format!("10:100/{node}@synthetic").parse().unwrap(),
                aka: "local".into(),
                enabled: true,
                inbound: true,
                outbound: true,
                transit: true,
                profile: PacketProfile::Type2Plus,
                charset: Charset::Cp437,
            })
            .collect(),
        routes: vec![],
        sources: vec![],
    };
    let map = FileEchoArea {
        domain: "synthetic".parse().unwrap(),
        tag: "FILES".into(),
        native_area: area.id.get(),
        enabled: true,
        inbound: true,
        outbound: true,
        description: "Synthetic files".into(),
        version: 0,
    };
    db.configure_fileecho_area(&policy, "operator", &map, NOW)
        .unwrap();
    for link in ["up", "a", "b"] {
        db.configure_file_subscription(
            &policy,
            "operator",
            &FileSubscription {
                link: link.into(),
                domain: map.domain.clone(),
                tag: map.tag.clone(),
                inbound: true,
                subscribed: true,
                held: false,
                version: 0,
            },
            NOW,
        )
        .unwrap();
    }
    let mut limits = db.file_network_policy().unwrap();
    limits.enabled = true;
    limits.freq = true;
    db.configure_file_network("operator", &limits, NOW).unwrap();
    let mut map = map;
    map.version = 1;
    Board {
        _temp: temp,
        db,
        storage,
        policy,
        area: map,
        file,
    }
}
fn session(b: &mut Board, link: &str) -> String {
    let session = super::super::id();
    b.db.connection.execute("INSERT INTO binkp_link_health(link_id,policy_digest,session_id,daemon_generation,authenticated) VALUES(?1,'synthetic',?2,'test',1) ON CONFLICT(link_id) DO UPDATE SET session_id=excluded.session_id,daemon_generation='test',authenticated=1",params![link,session]).unwrap();
    session
}
fn close(b: &mut Board, s: &str) {
    b.db.finish_binkp(s, None, NOW).unwrap();
}
fn inbound(b: &Board, link: &str, name: &str, bytes: &[u8]) -> Vec<u8> {
    let remote = b.policy.link(link).unwrap().remote.clone();
    tic::Metadata {
        area: b.area.tag.clone(),
        file: name.into(),
        long_name: None,
        origin: remote.clone(),
        from: remote.clone(),
        to: Some(b.policy.akas[0].endpoint.clone()),
        size: bytes.len() as u64,
        crc: tic::crc32(bytes),
        descriptions: vec!["Inbound synthetic".into()],
        long_descriptions: vec![],
        path: vec![tic::Hop {
            address: remote.clone(),
            time: NOW as u64,
            detail: String::new(),
        }],
        seen: vec![remote],
        opaque: vec![],
    }
    .encode("TESTPASS")
    .unwrap()
}
fn receive(b: &mut Board, s: &str, name: &str, bytes: &[u8]) {
    b.db.receive_file_artifact(&b.policy, &b.storage, s, name, bytes, NOW, &|_, p| {
        p == "TESTPASS"
    })
    .unwrap();
}
fn hatch(b: &mut Board) {
    let p =
        b.db.preview_file_hatch(
            &b.policy,
            &b.storage,
            b.file,
            &b.area.domain,
            &b.area.tag,
            NOW,
        )
        .unwrap();
    b.db.hatch_native_file(&b.policy, &b.storage, "operator", &p, NOW)
        .unwrap();
}
fn work(b: &mut Board, s: &str) -> Vec<(FileWork, Vec<u8>)> {
    b.db.claim_file_work(
        &b.policy,
        &b.storage,
        s,
        &|_| Some("TESTPASS".into()),
        64,
        64 * 1024 * 1024,
    )
    .unwrap()
}
#[test]
fn native_hatch_partial_ack_retry_and_restart_truth() {
    let mut b = board();
    hatch(&mut b);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
    assert_eq!(b.db.file_network_status().unwrap().queue.len(), 3);
    let s = session(&mut b, "a");
    let items = work(&mut b, &s);
    assert_eq!(items.len(), 2);
    assert!(b.db.file_work_accepted(&s, &items[0].0.key, NOW).is_err());
    for (item, bytes) in &items {
        if item.name.ends_with(".TIC") {
            tic::parse(bytes, &b.area.domain)
                .unwrap()
                .metadata
                .validate_payload(&items[0].1)
                .unwrap();
        }
        b.db.file_work_offered(&s, &item.key).unwrap();
        b.db.file_work_accepted(&s, &item.key, NOW).unwrap();
    }
    close(&mut b, &s);
    let s = session(&mut b, "b");
    let items = work(&mut b, &s);
    b.db.file_work_offered(&s, &items[0].0.key).unwrap();
    b.db.file_work_accepted(&s, &items[0].0.key, NOW).unwrap();
    b.db.recover_binkp(NOW + 1).unwrap();
    let s = session(&mut b, "b");
    let retry = work(&mut b, &s);
    assert_eq!(retry.len(), 1);
    assert!(retry[0].0.name.ends_with(".TIC"));
    close(&mut b, &s);
    let s = session(&mut b, "a");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
}
#[test]
fn inbound_both_orders_replay_and_reflection() {
    let mut b = board();
    let s = session(&mut b, "up");
    let bytes = b"first inbound";
    let tic = inbound(&b, "up", "FIRST.ZIP", bytes);
    receive(&mut b, &s, "ABC.TIC", &tic);
    assert_eq!(b.db.file_network_status().unwrap().staged, 1);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
    receive(&mut b, &s, "first.zip", bytes);
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    let status = b.db.file_network_status().unwrap();
    assert_eq!(status.queue.len(), 2);
    assert!(status.queue.iter().all(|q| q.link != "up"));
    receive(&mut b, &s, "FIRST.ZIP", bytes);
    receive(&mut b, &s, "DEF.TIC", &tic);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        2
    );
    assert_eq!(b.db.file_network_status().unwrap().queue.len(), 2);
    let bytes = b"second inbound";
    let tic = inbound(&b, "up", "SECOND.ZIP", bytes);
    receive(&mut b, &s, "SECOND.ZIP", bytes);
    receive(&mut b, &s, "NEW.TIC", &tic);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        3
    );
    close(&mut b, &s);
    let s = session(&mut b, "a");
    let bytes = b"from downstream a";
    let tic = inbound(&b, "a", "THIRD.ZIP", bytes);
    receive(&mut b, &s, "THIRD.ZIP", bytes);
    receive(&mut b, &s, "THIRD.TIC", &tic);
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .queue
        .iter()
        .filter(|q| q.filename == "THIRD.ZIP")
        .all(|q| q.link != "a"));
}
#[test]
fn trust_mismatch_conflict_capacity_and_private_projection() {
    let mut b = board();
    let s = session(&mut b, "up");
    let bytes = b"synthetic inbound";
    let original = inbound(&b, "up", "INBOUND.ZIP", bytes);
    let bad = String::from_utf8(original.clone())
        .unwrap()
        .replace("TESTPASS", "WRONGSECRET");
    receive(&mut b, &s, "BAD.TIC", bad.as_bytes());
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    let status = serde_json::to_string(&b.db.file_network_status().unwrap()).unwrap();
    assert!(!status.contains("WRONGSECRET"));
    assert!(!status.contains("TESTPASS"));
    assert!(b
        .db
        .receive_file_artifact(
            &b.policy,
            &b.storage,
            "fake",
            "X.TIC",
            &original,
            NOW,
            &|_, _| true
        )
        .is_err());
    assert!(b
        .db
        .receive_file_artifact(
            &b.policy,
            &b.storage,
            &s,
            "../SECRET",
            bytes,
            NOW,
            &|_, _| true
        )
        .is_err());
    receive(&mut b, &s, "GOOD.TIC", &original);
    receive(&mut b, &s, "INBOUND.ZIP", b"wrong");
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .activity
        .iter()
        .any(|a| a.result == "file-size-mismatch"));
    receive(&mut b, &s, "INBOUND.ZIP", bytes);
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .activity
        .iter()
        .any(|a| a.result == "file-pair-conflict"));
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
}
#[test]
fn freq_exact_grant_denial_bounds_and_revocation() {
    let mut b = board();
    let grant = FreqGrant {
        link: "a".into(),
        name: "LOCAL.ZIP".into(),
        file: b.file,
        enabled: true,
        version: 0,
    };
    b.db.configure_freq_grant(&b.policy, &b.storage, "operator", &grant, NOW)
        .unwrap();
    let s = session(&mut b, "b");
    receive(
        &mut b,
        &s,
        "ONE.REQ",
        b"LOCAL.ZIP
",
    );
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    let s = session(&mut b, "a");
    receive(
        &mut b,
        &s,
        "TWO.REQ",
        b"../secret
",
    );
    receive(
        &mut b,
        &s,
        "THREE.REQ",
        b"PRIVATE.ZIP
",
    );
    assert!(work(&mut b, &s).is_empty());
    receive(
        &mut b,
        &s,
        "FOUR.REQ",
        b"LOCAL.ZIP
",
    );
    let items = work(&mut b, &s);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0.name, "LOCAL.ZIP");
    close(&mut b, &s);
    let mut grant = grant;
    grant.version = 1;
    grant.enabled = false;
    b.db.configure_freq_grant(&b.policy, &b.storage, "operator", &grant, NOW)
        .unwrap();
    let s = session(&mut b, "a");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    let mut limits = b.db.file_network_policy().unwrap();
    limits.freq_bytes = 1;
    b.db.configure_file_network("operator", &limits, NOW)
        .unwrap();
    grant.version = 2;
    grant.enabled = true;
    b.db.configure_freq_grant(&b.policy, &b.storage, "operator", &grant, NOW)
        .unwrap();
    let s = session(&mut b, "a");
    receive(
        &mut b,
        &s,
        "FIVE.REQ",
        b"LOCAL.ZIP
",
    );
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .activity
        .iter()
        .any(|a| a.result == "freq-bound-exceeded"));
}
#[test]
fn cas_hold_and_transaction_failure_preserve_native_authority() {
    let mut b = board();
    assert!(b
        .db
        .configure_fileecho_area(
            &b.policy,
            "operator",
            &FileEchoArea {
                version: 0,
                ..b.area.clone()
            },
            NOW
        )
        .is_err());
    let mut sub =
        b.db.file_network_status()
            .unwrap()
            .subscriptions
            .into_iter()
            .find(|s| s.link == "b")
            .unwrap();
    sub.held = true;
    b.db.configure_file_subscription(&b.policy, "operator", &sub, NOW)
        .unwrap();
    hatch(&mut b);
    let s = session(&mut b, "b");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    sub.version += 1;
    sub.held = false;
    b.db.configure_file_subscription(&b.policy, "operator", &sub, NOW)
        .unwrap();
    let s = session(&mut b, "b");
    assert_eq!(work(&mut b, &s).len(), 2);
    close(&mut b, &s);
    b.db.connection.execute_batch("CREATE TRIGGER reject_test_publication BEFORE INSERT ON ftn_file_publications BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    let s = session(&mut b, "up");
    let bytes = b"transaction failure";
    let tic = inbound(&b, "up", "ATOMIC.ZIP", bytes);
    receive(&mut b, &s, "ATOMIC.TIC", &tic);
    assert!(b
        .db
        .receive_file_artifact(
            &b.policy,
            &b.storage,
            &s,
            "ATOMIC.ZIP",
            bytes,
            NOW,
            &|_, _| true
        )
        .is_err());
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
}

#[test]
fn snapshot_staging_and_later_partial_acceptance_reconcile() {
    let mut b = board();
    hatch(&mut b);
    let s = session(&mut b, "up");
    let content = b"staged across backup";
    let tic = inbound(&b, "up", "STAGED.ZIP", content);
    receive(&mut b, &s, "STAGED.TIC", &tic);
    close(&mut b, &s);
    let backup = b._temp.path().join("snapshot.sqlite");
    b.db.backup_to(&backup).unwrap();
    let s = session(&mut b, "a");
    let items = work(&mut b, &s);
    for (item, _) in items {
        b.db.file_work_offered(&s, &item.key).unwrap();
        b.db.file_work_accepted(&s, &item.key, NOW).unwrap();
    }
    close(&mut b, &s);
    let s = session(&mut b, "b");
    let items = work(&mut b, &s);
    b.db.file_work_offered(&s, &items[0].0.key).unwrap();
    b.db.file_work_accepted(&s, &items[0].0.key, NOW).unwrap();
    close(&mut b, &s);
    let evidence = b.db.ftn_recovery_evidence().unwrap();
    b.db = RuntimeDatabase::open(&backup).unwrap();
    b.db.recover_binkp(NOW).unwrap();
    b.db.hold_restored_ftn().unwrap();
    b.db.reconcile_ftn_recovery(&evidence, "operator", NOW)
        .unwrap();
    let s = session(&mut b, "a");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    let s = session(&mut b, "b");
    let items = work(&mut b, &s);
    assert_eq!(items.len(), 1);
    assert!(items[0].0.name.ends_with(".TIC"));
    close(&mut b, &s);
    assert_eq!(b.db.file_network_status().unwrap().staged, 1);
    let s = session(&mut b, "up");
    receive(&mut b, &s, "STAGED.ZIP", content);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        2
    );
}
#[test]
fn freq_private_area_symlink_and_per_session_limits() {
    let mut b = board();
    b.db.connection
        .execute(
            "UPDATE file_areas SET read_security=9999 WHERE area_id=?1",
            [b.area.native_area],
        )
        .unwrap();
    let grant = FreqGrant {
        link: "a".into(),
        name: "LOCAL.ZIP".into(),
        file: b.file,
        enabled: true,
        version: 0,
    };
    assert!(b
        .db
        .configure_freq_grant(&b.policy, &b.storage, "operator", &grant, NOW)
        .is_err());
    b.db.connection
        .execute(
            "UPDATE file_areas SET read_security=0 WHERE area_id=?1",
            [b.area.native_area],
        )
        .unwrap();
    b.db.configure_freq_grant(&b.policy, &b.storage, "operator", &grant, NOW)
        .unwrap();
    let mut limits = b.db.file_network_policy().unwrap();
    limits.freq_files = 1;
    b.db.configure_file_network("operator", &limits, NOW)
        .unwrap();
    let s = session(&mut b, "a");
    receive(&mut b, &s, "ONE.REQ", b"LOCAL.ZIP\r\n");
    receive(&mut b, &s, "TWO.REQ", b"LOCAL.ZIP\r\n");
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .activity
        .iter()
        .any(|a| a.result == "freq-session-bound-exceeded"));
    close(&mut b, &s);
    #[cfg(unix)]
    {
        let area =
            b.db.load_area_by_id(FileAreaId::new(b.area.native_area).unwrap())
                .unwrap()
                .unwrap();
        let path = b.storage.ensure_area(&area).unwrap().join("LOCAL.ZIP");
        let outside = b._temp.path().join("private.txt");
        std::fs::write(&outside, b"not authorized").unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        let s = session(&mut b, "a");
        assert!(work(&mut b, &s).is_empty());
    }
}
#[test]
fn crc_conflicts_and_staging_quota_fail_closed() {
    let mut b = board();
    let mut limits = b.db.file_network_policy().unwrap();
    limits.staging_count = 1;
    b.db.configure_file_network("operator", &limits, NOW)
        .unwrap();
    let s = session(&mut b, "up");
    let bytes = b"123456789";
    let tic = inbound(&b, "up", "CRC.ZIP", bytes);
    receive(&mut b, &s, "CRC.TIC", &tic);
    assert!(b
        .db
        .receive_file_artifact(
            &b.policy,
            &b.storage,
            &s,
            "CRC.ZIP",
            b"123456780",
            NOW,
            &|_, _| true
        )
        .is_err());
    close(&mut b, &s);
    limits = b.db.file_network_policy().unwrap();
    limits.staging_count = 2;
    b.db.configure_file_network("operator", &limits, NOW)
        .unwrap();
    let s = session(&mut b, "up");
    receive(&mut b, &s, "CRC.ZIP", b"123456780");
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .activity
        .iter()
        .any(|a| a.result == "file-checksum-mismatch"));
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
}

#[test]
fn credential_rotation_revokes_incomplete_control_and_freq_offer_replay_is_stable() {
    let mut b = board();
    let s = session(&mut b, "up");
    let control = inbound(&b, "up", "WAIT.ZIP", b"waiting");
    receive(&mut b, &s, "WAIT.TIC", &control);
    assert_eq!(b.db.file_network_status().unwrap().staged, 1);
    assert!(b.db.invalidate_staged_tics("up", NOW).is_err());
    close(&mut b, &s);
    b.db.invalidate_staged_tics("up", NOW).unwrap();
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    let s = session(&mut b, "up");
    receive(&mut b, &s, "WAIT.ZIP", b"waiting");
    assert_eq!(b.db.file_network_status().unwrap().staged, 1);
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        1
    );
    close(&mut b, &s);
    b.db.configure_freq_grant(
        &b.policy,
        &b.storage,
        "operator",
        &FreqGrant {
            link: "a".into(),
            name: "LOCAL.ZIP".into(),
            file: b.file,
            enabled: true,
            version: 0,
        },
        NOW,
    )
    .unwrap();
    let s = session(&mut b, "a");
    let request = b"LOCAL.ZIP\r\n";
    for time in [100, 100, 101] {
        b.db.receive_file_offer(
            &b.policy,
            &b.storage,
            &s,
            &sf_net::binkp::Offer {
                name: "00640001.REQ".into(),
                size: request.len() as u64,
                time,
                offset: 0,
            },
            request,
            NOW,
            &|_, _| false,
        )
        .unwrap();
    }
    assert_eq!(
        b.db.file_network_status()
            .unwrap()
            .queue
            .iter()
            .filter(|q| q.kind == "freq-response")
            .count(),
        2
    );
}

#[test]
fn forced_retry_renews_attempt_budget_without_losing_partial_acceptance() {
    let mut b = board();
    hatch(&mut b);
    let s = session(&mut b, "a");
    let items = work(&mut b, &s);
    b.db.file_work_offered(&s, &items[0].0.key).unwrap();
    b.db.file_work_accepted(&s, &items[0].0.key, NOW).unwrap();
    close(&mut b, &s);
    b.db.connection
        .execute(
            "UPDATE ftn_file_deliveries SET attempts=12 WHERE link_id='a'",
            [],
        )
        .unwrap();
    let s = session(&mut b, "a");
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    let q =
        b.db.file_network_status()
            .unwrap()
            .queue
            .into_iter()
            .find(|q| q.link == "a")
            .unwrap();
    assert!(q.held && q.payload_accepted && !q.tic_accepted);
    b.db.hold_file_delivery("operator", &q.delivery, q.version, false, NOW)
        .unwrap();
    let s = session(&mut b, "a");
    let retry = work(&mut b, &s);
    assert_eq!(retry.len(), 1);
    assert!(retry[0].0.name.ends_with(".TIC"));
    assert!(b
        .db
        .file_network_status()
        .unwrap()
        .queue
        .iter()
        .any(|q| q.link == "a" && q.payload_accepted && q.attempts == 1));
}

#[test]
fn native_name_collision_and_long_hatch_preserve_content_identity() {
    let mut b = board();
    let s = session(&mut b, "up");
    let bytes = b"different content under an existing native filename";
    let control = inbound(&b, "up", "LOCAL.ZIP", bytes);
    receive(&mut b, &s, "LOCAL.ZIP", bytes);
    receive(&mut b, &s, "COLLIDE.TIC", &control);
    receive(&mut b, &s, "LOCAL.ZIP", bytes);
    receive(&mut b, &s, "COLLIDE.TIC", &control);
    close(&mut b, &s);
    let catalog = b.db.all_cataloged_files().unwrap();
    assert_eq!(catalog.len(), 2);
    let collision = catalog
        .iter()
        .find(|(_, f)| f.sha256 == qwk::digest(bytes))
        .unwrap();
    assert_ne!(collision.1.filename, "LOCAL.ZIP");
    assert!(collision.1.filename.starts_with(&qwk::digest(bytes)[..16]));
    let area =
        b.db.load_area_by_id(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap()
            .unwrap();
    // Native C6 admission validates actual archives; this filename fixture must
    // contain a real ZIP, rather than text carrying a .ZIP extension.
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("README.TXT", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, b"new hatch content").unwrap();
    let native_zip = archive.finish().unwrap().into_inner();
    let file =
        b.db.add_managed_file(
            &b.storage,
            FileAdminActor::LocalOperator,
            area.id,
            area.state_version,
            "Long-Native-File-Name.ZIP",
            "Preserved long native metadata",
            &native_zip,
        )
        .unwrap()
        .file;
    let preview =
        b.db.preview_file_hatch(
            &b.policy,
            &b.storage,
            file.id.get(),
            &b.area.domain,
            &b.area.tag,
            NOW,
        )
        .unwrap();
    assert_eq!(preview.filename, "Long-Native-File-Name.ZIP");
    assert_ne!(preview.filename, preview.transfer_name);
    assert!(tic::filename(&preview.transfer_name).is_ok());
    b.db.hatch_native_file(&b.policy, &b.storage, "operator", &preview, NOW)
        .unwrap();
    let s = session(&mut b, "up");
    let items = work(&mut b, &s);
    assert_eq!(items.len(), 2);
    let envelope = tic::parse(
        &items
            .iter()
            .find(|(w, _)| w.name.ends_with(".TIC"))
            .unwrap()
            .1,
        &b.area.domain,
    )
    .unwrap();
    assert_eq!(
        envelope.metadata.long_name.as_deref(),
        Some("Long-Native-File-Name.ZIP")
    );
    assert_eq!(b.db.file_count(area.id).unwrap(), 3);
}

#[path = "file_hatch_tests.rs"]
mod direct_hatch;

#[path = "freq_tests.rs"]
mod freq_recovery;
