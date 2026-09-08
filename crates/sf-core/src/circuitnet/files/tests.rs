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
use crate::circuitnet::{Node, Topology};
use crate::files::tests::board;
use std::io::Cursor;
fn node(s: &str) -> NodeId {
    NodeId::new(s).unwrap()
}
fn net() -> NetworkId {
    NetworkId::new("synthetic-files").unwrap()
}
fn code() -> Codename {
    Codename::new("CNFILES").unwrap()
}
fn topology() -> Topology {
    Topology {
        nodes: [
            ("ROOT1", Role::Root, None),
            ("HOST1", Role::Host, Some("ROOT1")),
            ("HOST2", Role::Host, Some("ROOT1")),
            ("END1", Role::End, Some("HOST1")),
            ("END2", Role::End, Some("HOST1")),
            ("END3", Role::End, Some("HOST2")),
        ]
        .into_iter()
        .map(|(id, role, parent)| Node {
            id: node(id),
            role,
            parent: parent.map(node),
        })
        .collect(),
    }
}
struct Board {
    _tmp: tempfile::TempDir,
    db: RuntimeDatabase,
    store: FileStorage,
    area: crate::FileArea,
    local: NodeId,
}
fn configured(local: &str) -> Board {
    let (_tmp, mut db, store, area) = board();
    let local = node(local);
    let topology = topology();
    db.circuitnet_configure(
        "operator",
        &Profile {
            network: net(),
            display_name: "Synthetic files".into(),
            local: local.clone(),
            enabled: true,
            trusted_offline: false,
            topology: topology.clone(),
        },
        0,
        1,
    )
    .unwrap();
    db.circuitnet_file_map(
        "operator",
        &net(),
        &Mapping {
            codename: code(),
            area: area.id.get(),
            send: true,
            receive: true,
            maximum_bytes: 1024 * 1024,
            version: 0,
        },
        1,
    )
    .unwrap();
    for neighbor in topology.neighbors(&local).unwrap() {
        db.circuitnet_file_subscribe(
            "operator",
            &net(),
            &FileDossier {
                neighbor,
                codename: code(),
                subscribed: true,
                version: 0,
            },
            1,
        )
        .unwrap();
    }
    Board {
        _tmp,
        db,
        store,
        area,
        local,
    }
}
fn originate(b: &mut Board, name: &str, bytes: &[u8]) {
    b.store
        .import_file(
            &mut b.db,
            FileAdminActor::LocalOperator,
            &b.area,
            name,
            "Synthetic file",
            &mut Cursor::new(bytes),
            "operator",
            None,
        )
        .unwrap();
    b.db.circuitnet_files_prepare(&net(), 2).unwrap();
}
fn exchange(from: &mut Board, to: &mut Board, ack: bool) -> Vec<Receipt> {
    let offers = from
        .db
        .circuitnet_file_offers(&net(), &to.local, 3)
        .unwrap();
    let mut receipts = Vec::new();
    for p in offers {
        from.db
            .circuitnet_file_attempt(&net(), &to.local, &p.id, None, 3)
            .unwrap();
        let result = match to
            .db
            .circuitnet_file_offer(&to.store, &net(), &from.local, &p)
            .unwrap()
        {
            Want::Complete { receipt } => receipt,
            _ => {
                let mut file = from.store.open_content(&p.sha256, p.size).unwrap();
                to.db
                    .circuitnet_file_receive(&to.store, &net(), &from.local, &p, &mut file, 3)
                    .unwrap()
            }
        };
        if ack {
            from.db
                .circuitnet_file_attempt(&net(), &to.local, &p.id, Some(&result), 3)
                .unwrap();
        }
        receipts.push(result);
    }
    receipts
}
#[test]
fn six_native_boards_sibling_cross_root_origin_no_reflection_and_partial_ack() {
    let (mut root, mut h1, mut h2, mut e1, mut e2, mut e3) = (
        configured("ROOT1"),
        configured("HOST1"),
        configured("HOST2"),
        configured("END1"),
        configured("END2"),
        configured("END3"),
    );
    originate(&mut e1, "FIRST.TXT", b"synthetic original");
    let first = exchange(&mut e1, &mut h1, false);
    assert_eq!(first.len(), 1);
    assert_eq!(exchange(&mut e1, &mut h1, true), first);
    assert_eq!(h1.db.file_count(h1.area.id).unwrap(), 1);
    assert!(h1
        .db
        .circuitnet_file_offers(&net(), &e1.local, 4)
        .unwrap()
        .is_empty());
    exchange(&mut h1, &mut e2, true);
    exchange(&mut h1, &mut root, true);
    exchange(&mut root, &mut h2, true);
    // One branch's unacknowledged result never rolls back the completed sibling.
    let lost = exchange(&mut h2, &mut e3, false);
    assert_eq!(exchange(&mut h2, &mut e3, true), lost);
    for b in [&root, &h1, &h2, &e2, &e3] {
        assert_eq!(b.db.file_count(b.area.id).unwrap(), 1);
        let json: String =
            b.db.connection
                .query_row(
                    "SELECT envelope FROM circuitnet_file_publications WHERE network=?1",
                    [net().as_str()],
                    |r| r.get(0),
                )
                .unwrap();
        let p: Publication = serde_json::from_str(&json).unwrap();
        assert_eq!(p.origin, node("END1"));
    }
    assert!(e2
        .db
        .circuitnet_file_offers(&net(), &h1.local, 4)
        .unwrap()
        .is_empty());
    h1.db
        .circuitnet_file_subscribe(
            "operator",
            &net(),
            &FileDossier {
                neighbor: e2.local.clone(),
                codename: code(),
                subscribed: false,
                version: 1,
            },
            5,
        )
        .unwrap();
    originate(&mut root, "ROOT.TXT", b"root publication");
    exchange(&mut root, &mut h1, true);
    exchange(&mut h1, &mut e1, true);
    assert!(h1
        .db
        .circuitnet_file_offers(&net(), &e2.local, 6)
        .unwrap()
        .is_empty());
    assert_eq!(e2.db.file_count(e2.area.id).unwrap(), 1);
    assert_eq!(e1.db.file_count(e1.area.id).unwrap(), 2);
}
#[test]
fn have_hash_replay_conflict_local_scan_and_bad_payload_fail_closed() {
    let (mut end, mut host) = (configured("END1"), configured("HOST1"));
    originate(&mut end, "NEW.TXT", b"same bytes");
    originate(&mut host, "ALREADY.TXT", b"same bytes");
    let p = end
        .db
        .circuitnet_file_offers(&net(), &host.local, 2)
        .unwrap()
        .remove(0);
    assert!(matches!(
        host.db
            .circuitnet_file_offer(&host.store, &net(), &end.local, &p)
            .unwrap(),
        Want::Have
    ));
    let mut corrupt = Cursor::new(b"different");
    assert!(host
        .db
        .circuitnet_file_receive(&host.store, &net(), &end.local, &p, &mut corrupt, 3)
        .is_err());
    assert_eq!(host.db.file_count(host.area.id).unwrap(), 1);
    host.db
        .set_file_safety_policy(
            FileAdminActor::LocalOperator,
            host.area.id,
            &crate::files::SafetyPolicy::default(),
        )
        .unwrap();
    let receipts = exchange(&mut end, &mut host, true);
    assert_eq!(receipts[0].outcome, Outcome::Quarantined);
    assert_eq!(host.db.file_count(host.area.id).unwrap(), 1);
    assert!(matches!(
        host.db
            .circuitnet_file_offer(&host.store, &net(), &end.local, &p)
            .unwrap(),
        Want::Complete { .. }
    ));
    let mut forged = p.clone();
    forged.description = "forged metadata".into();
    assert!(host
        .db
        .circuitnet_file_offer(&host.store, &net(), &end.local, &forged)
        .is_err());
    assert!(host
        .db
        .circuitnet_file_offer(&host.store, &net(), &node("END3"), &p)
        .is_err());
    let mut wrong = p.clone();
    wrong.network = NetworkId::new("wrong").unwrap();
    assert!(host
        .db
        .circuitnet_file_offer(&host.store, &net(), &end.local, &wrong)
        .is_err());
    assert!(host
        .db
        .circuitnet_file_offers(&net(), &node("ROOT1"), 4)
        .unwrap()
        .iter()
        .all(|f| f.id != p.id));
}

#[test]
fn attempt_budget_explicit_retry_and_unsubscribed_ingress_fail_closed() {
    let (mut end, mut host) = (configured("END1"), configured("HOST1"));
    originate(&mut end, "RETRY.TXT", b"retry");
    let p = end
        .db
        .circuitnet_file_offers(&net(), &host.local, 1)
        .unwrap()
        .remove(0);
    for _ in 0..12 {
        end.db
            .circuitnet_file_attempt(&net(), &host.local, &p.id, None, 2)
            .unwrap();
    }
    assert!(end
        .db
        .circuitnet_file_offers(&net(), &host.local, 3)
        .unwrap()
        .is_empty());
    end.db
        .circuitnet_file_retry("operator", &net(), &host.local, &p.id, 4)
        .unwrap();
    assert_eq!(
        end.db
            .circuitnet_file_offers(&net(), &host.local, 4)
            .unwrap()
            .len(),
        1
    );
    host.db
        .circuitnet_file_subscribe(
            "operator",
            &net(),
            &FileDossier {
                neighbor: end.local.clone(),
                codename: code(),
                subscribed: false,
                version: 1,
            },
            5,
        )
        .unwrap();
    assert!(host
        .db
        .circuitnet_file_offer(&host.store, &net(), &end.local, &p)
        .is_err());
}

#[test]
fn file_preparation_waits_for_a_concurrent_native_writer() {
    let mut end = configured("END1");
    originate(&mut end, "QUEUED.TXT", b"harmless concurrent work");
    let path = end.db.path().to_owned();
    let (ready, wait) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        let mut db = RuntimeDatabase::open(&path).unwrap();
        let transaction = db
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        transaction
            .execute(
                "UPDATE network_preparation SET generation=generation+1 WHERE singleton=1",
                [],
            )
            .unwrap();
        ready.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        transaction.commit().unwrap();
    });
    wait.recv().unwrap();
    end.db.circuitnet_files_prepare(&net(), 5).unwrap();
    writer.join().unwrap();
    assert_eq!(
        end.db
            .circuitnet_file_offers(&net(), &node("HOST1"), 6)
            .unwrap()
            .len(),
        1
    );
}
