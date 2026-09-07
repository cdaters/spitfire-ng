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
use super::{MessageId, NodeId};
use crate::*;
use std::{collections::BTreeMap, sync::Mutex};
#[derive(Default)]
struct Store(Mutex<BTreeMap<String, Vec<u8>>>, network::ImportCapacity);
impl NetworkArtifactStore for Store {
    fn admit_import(&self) -> Result<network::ImportPermit<'_>, network::NetworkError> {
        self.1.acquire()
    }
    fn preserve(&self, b: &[u8]) -> Result<String, network::NetworkError> {
        let id = wire::digest(b);
        self.0.lock().unwrap().insert(id.clone(), b.to_vec());
        Ok(id)
    }
    fn usage(&self) -> Result<(u64, usize), network::NetworkError> {
        let s = self.0.lock().unwrap();
        Ok((s.values().map(|b| b.len() as u64).sum(), s.len()))
    }
}
fn net() -> NetworkId {
    NetworkId::new("circuitnet-test").unwrap()
}
fn node(s: &str) -> NodeId {
    NodeId::new(s).unwrap()
}
fn code(s: &str) -> Codename {
    Codename::new(s).unwrap()
}
struct Board {
    _temp: tempfile::TempDir,
    db: RuntimeDatabase,
    store: Store,
    actor: MessageActor,
    conference: ConferenceId,
}
fn board(local: &str) -> Board {
    let temp = tempfile::tempdir().unwrap();
    let mut db = RuntimeDatabase::open(&temp.path().join("native.sqlite3")).unwrap();
    db.migrate().unwrap();
    db.ensure_board_identity(&BoardIdentity::new("CircuitNET Test", "Sysop").unwrap())
        .unwrap();
    let hash = CredentialHasher::new(&PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    })
    .unwrap()
    .hash(b"synthetic test password")
    .unwrap();
    let caller = db
        .create_caller(
            b"Handle",
            &hash,
            SecurityLevel::new(100).unwrap(),
            CallerState::Active,
            false,
            1,
        )
        .unwrap();
    let conference = db
        .ensure_conference(&ConferenceDefinition {
            posting_identity: None,
            number: 17,
            name: "Synthetic".into(),
            description: "C2".into(),
            access_mode: ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            post_security: SecurityLevel::new(0).unwrap(),
            public_only: true,
            caller_deletion_enabled: true,
            maximum_lines: 99,
            privileged_security_levels: vec![],
        })
        .unwrap()
        .id;
    let topology = Topology {
        nodes: vec![
            Node {
                id: node("HOST"),
                role: Role::Host,
                parent: None,
            },
            Node {
                id: node("END1"),
                role: Role::End,
                parent: Some(node("HOST")),
            },
            Node {
                id: node("END2"),
                role: Role::End,
                parent: Some(node("HOST")),
            },
        ],
    };
    let p = Profile {
        network: net(),
        display_name: "Synthetic CircuitNET".into(),
        local: node(local),
        enabled: true,
        trusted_offline: true,
        topology,
    };
    db.circuitnet_configure("operator", &p, 0, 1).unwrap();
    db.circuitnet_map(
        "operator",
        &net(),
        &Mapping {
            codename: code("CNTEST"),
            conference: conference.get(),
            send: true,
            receive: true,
        },
        0,
        1,
    )
    .unwrap();
    for n in p.topology.neighbors(&p.local).unwrap() {
        db.circuitnet_subscribe(
            "operator",
            &net(),
            &Dossier {
                neighbor: n,
                codename: code("CNTEST"),
                subscribed: true,
                version: 0,
            },
            1,
        )
        .unwrap();
    }
    Board {
        _temp: temp,
        db,
        store: Store::default(),
        actor: MessageActor::new(caller.id, SecurityLevel::new(100).unwrap()),
        conference,
    }
}
fn post(b: &mut Board, subject: &str, parent: Option<crate::MessageId>) -> crate::Message {
    let preview =
        b.db.preview_posting_identity(b.actor, b.conference)
            .unwrap();
    b.db.post(
        b.actor,
        NewMessage {
            identity_preview: Some(preview),
            conference_id: b.conference,
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: subject.as_bytes().to_vec(),
            body: b"Synthetic body\r\n".to_vec(),
            created_at: 10,
            parent_message_id: parent,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        },
    )
    .unwrap()
}
fn scan(b: &mut Board) {
    b.db.circuitnet_scan(&net(), 0, 11).unwrap();
}
fn prepare(b: &mut Board, n: &str) -> Prepared {
    b.db.circuitnet_prepare(&b.store, &net(), &node(n), 12)
        .unwrap()
}
fn transfer(from: &mut Board, to: &mut Board) -> Prepared {
    let src = from.db.circuitnet_status(&net()).unwrap().profile.local;
    let dest = to.db.circuitnet_status(&net()).unwrap().profile.local;
    let p = from
        .db
        .circuitnet_prepare(&from.store, &net(), &dest, 12)
        .unwrap();
    let r = to
        .db
        .circuitnet_import(&to.store, &net(), &src, &p.bytes, 13)
        .unwrap();
    from.db
        .circuitnet_acknowledge(&from.store, &net(), &dest, &r.receipt, 14)
        .unwrap();
    p
}
fn count(b: &Board) -> usize {
    b.db.messages(b.actor, b.conference).unwrap().len()
}
#[test]
fn child_host_sibling_no_reflection_replay_and_threading() {
    let (mut host, mut e1, mut e2) = (board("HOST"), board("END1"), board("END2"));
    post(&mut e2, "Parent", None);
    scan(&mut e2);
    let original = transfer(&mut e2, &mut host);
    transfer(&mut host, &mut e1);
    assert_eq!((count(&host), count(&e1), count(&e2)), (1, 1, 1));
    assert!(matches!(
        host.db
            .circuitnet_prepare(&host.store, &net(), &node("END2"), 15),
        Err(Error::Empty)
    ));
    let parent = e1.db.message(e1.actor, e1.conference, 1).unwrap().id;
    post(&mut e1, "Reply", Some(parent));
    scan(&mut e1);
    transfer(&mut e1, &mut host);
    transfer(&mut host, &mut e2);
    for b in [&host, &e1, &e2] {
        let reply = b.db.message(b.actor, b.conference, 2).unwrap();
        assert!(reply.parent_message_id.is_some());
        assert_eq!(count(b), 2);
    }
    let before = host.db.circuitnet_status(&net()).unwrap();
    let r = host
        .db
        .circuitnet_import(&host.store, &net(), &node("END2"), &original.bytes, 20)
        .unwrap();
    assert!(r.replay);
    assert_eq!(r.imported, 0);
    assert_eq!(count(&host), 2);
    assert_eq!(
        host.db.circuitnet_status(&net()).unwrap().accepted,
        before.accepted
    );
    let origins: Vec<String> = host
        .db
        .connection
        .prepare("SELECT origin FROM circuitnet_messages ORDER BY message_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(origins, vec!["END2", "END1"]);
}
#[test]
fn partial_delivery_retry_and_unsubscribe_preserve_history() {
    let (mut host, mut e1, mut e2) = (board("HOST"), board("END1"), board("END2"));
    post(&mut host, "Fanout", None);
    scan(&mut host);
    transfer(&mut host, &mut e1);
    let failed = prepare(&mut host, "END2");
    host.db
        .circuitnet_failed(&net(), &failed.artifact, 20)
        .unwrap();
    let again = prepare(&mut host, "END2");
    assert_eq!(again.bytes, failed.bytes);
    assert_eq!(host.db.circuitnet_status(&net()).unwrap().accepted, 1);
    transfer(&mut host, &mut e2);
    assert_eq!(host.db.circuitnet_status(&net()).unwrap().accepted, 2);
    post(&mut host, "Pending removal", None);
    scan(&mut host);
    host.db
        .circuitnet_subscribe(
            "operator",
            &net(),
            &Dossier {
                neighbor: node("END2"),
                codename: code("CNTEST"),
                subscribed: false,
                version: 1,
            },
            21,
        )
        .unwrap();
    assert!(matches!(
        host.db
            .circuitnet_prepare(&host.store, &net(), &node("END2"), 22),
        Err(Error::Empty)
    ));
    post(&mut host, "Future filtered", None);
    scan(&mut host);
    transfer(&mut host, &mut e1);
    assert_eq!(count(&e2), 1);
    assert_eq!(count(&e1), 3);
    let state = host.db.circuitnet_status(&net()).unwrap();
    assert_eq!(state.accepted, 4);
    assert_eq!(state.pending, 1);
}
#[test]
fn conflicts_wrong_neighbor_network_and_atomic_rejection() {
    let (mut host, mut e1) = (board("HOST"), board("END1"));
    post(&mut e1, "Original", None);
    scan(&mut e1);
    let p = transfer(&mut e1, &mut host);
    let mut b = Batch::decode(&p.bytes).unwrap();
    b.messages[0].body = "Forged".into();
    assert!(matches!(
        host.db
            .circuitnet_import(&host.store, &net(), &node("END1"), &b.encode().unwrap(), 21),
        Err(Error::Conflict)
    ));
    assert!(host
        .db
        .circuitnet_import(&host.store, &net(), &node("END2"), &p.bytes, 21)
        .is_err());
    let mut b = Batch::decode(&p.bytes).unwrap();
    b.network = NetworkId::new("wrong-network").unwrap();
    assert!(host
        .db
        .circuitnet_import(&host.store, &net(), &node("END1"), &b.encode().unwrap(), 21)
        .is_err());
    let mut b = Batch::decode(&p.bytes).unwrap();
    b.messages[0].id = MessageId::new("END2:00000000000000000000000000000003").unwrap();
    b.messages[0].origin = node("END2");
    b.messages[0].path = vec![node("END2"), node("END1")];
    assert!(host
        .db
        .circuitnet_import(&host.store, &net(), &node("END1"), &b.encode().unwrap(), 21)
        .is_err());
    assert_eq!(count(&host), 1);
}
#[test]
fn restart_snapshot_restore_receipts_and_identity_freeze() {
    let (mut host, mut e1) = (board("HOST"), board("END1"));
    let post = post(&mut host, "Frozen", None);
    scan(&mut host);
    let before = prepare(&mut host, "END1");
    let caller = host
        .db
        .caller_by_id(host.actor.caller_id())
        .unwrap()
        .unwrap();
    let mut profile = caller.profile.clone();
    profile.identity =
        PrivateIdentity::new(Some("PrivateCanary".into()), Some("NeverPublish".into())).unwrap();
    host.db
        .update_caller_profile_versioned(
            caller.id,
            caller.state_version,
            profile,
            &CallerProfilePolicy::default(),
            identity::IdentityEditActor::LocalOperator,
            20,
        )
        .unwrap();
    let caller = host.db.caller_by_id(caller.id).unwrap().unwrap();
    host.db
        .update_caller_login_handle(
            caller.id,
            caller.state_version,
            b"renamed",
            b"RenamedHandle",
            &CallerConfig::default(),
            21,
        )
        .unwrap();
    let path = host._temp.path().join("native.sqlite3");
    host.db = RuntimeDatabase::open(&path).unwrap();
    assert_eq!(prepare(&mut host, "END1").bytes, before.bytes);
    let r = e1
        .db
        .circuitnet_import(&e1.store, &net(), &node("HOST"), &before.bytes, 22)
        .unwrap();
    host.db
        .circuitnet_acknowledge(&host.store, &net(), &node("END1"), &r.receipt, 23)
        .unwrap();
    let backup = host._temp.path().join("backup.sqlite3");
    host.db.backup_to(&backup).unwrap();
    let mut restored = RuntimeDatabase::open(&backup).unwrap();
    restored.hold_restored_circuitnet().unwrap();
    assert_eq!(restored.circuitnet_status(&net()).unwrap().accepted, 1);
    assert!(matches!(
        restored.circuitnet_prepare(&host.store, &net(), &node("END1"), 24),
        Err(Error::Empty)
    ));
    assert_eq!(
        restored
            .message(host.actor, host.conference, post.number)
            .unwrap()
            .author_name,
        "Handle"
    );
    assert!(!String::from_utf8(before.bytes)
        .unwrap()
        .contains("PrivateCanary"));
}
#[test]
fn mapping_after_post_cannot_grant_retroactive_publication() {
    let mut b = board("HOST");
    b.db.connection.execute("UPDATE circuitnet_profiles SET configuration=json_set(configuration,'$.enabled',json('false'))",[]).unwrap();
    post(&mut b, "Before enable", None);
    b.db.connection.execute("UPDATE circuitnet_profiles SET configuration=json_set(configuration,'$.enabled',json('true'))",[]).unwrap();
    scan(&mut b);
    assert_eq!(b.db.circuitnet_status(&net()).unwrap().pending, 0);
}
#[test]
fn stale_config_mapping_and_dossier_revisions_reject() {
    let mut b = board("HOST");
    let s = b.db.circuitnet_status(&net()).unwrap();
    assert!(b
        .db
        .circuitnet_configure("operator", &s.profile, 0, 2)
        .is_err());
    assert!(b
        .db
        .circuitnet_subscribe(
            "operator",
            &net(),
            &Dossier {
                neighbor: node("END1"),
                codename: code("CNTEST"),
                subscribed: false,
                version: 0
            },
            2
        )
        .is_err());
    let mut p = s.profile;
    post(&mut b, "History", None);
    scan(&mut b);
    p.local = node("END1");
    assert!(b.db.circuitnet_configure("operator", &p, 1, 2).is_err());
}

#[test]
fn retry_offer_stays_stable_when_new_traffic_arrives() {
    let mut host = board("HOST");
    post(&mut host, "First", None);
    scan(&mut host);
    let first = prepare(&mut host, "END1");
    post(&mut host, "Second", None);
    scan(&mut host);
    let retry = prepare(&mut host, "END1");
    assert_eq!(first.bytes, retry.bytes);
    assert_eq!(retry.messages, 1);
}
#[test]
fn uncarried_host_transit_uses_native_restricted_container() {
    let (mut host, mut e1, mut e2) = (board("HOST"), board("END1"), board("END2"));
    host.db
        .connection
        .execute("DELETE FROM circuitnet_mappings", [])
        .unwrap();
    post(&mut e1, "Transit", None);
    scan(&mut e1);
    transfer(&mut e1, &mut host);
    assert_eq!(count(&host), 0);
    assert_eq!(
        host.db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE container_kind='network-transit'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    transfer(&mut host, &mut e2);
    assert_eq!(count(&e2), 1);
}
#[test]
fn unresolved_parent_later_links_and_cyclic_batch_rolls_back() {
    let (mut host, mut e1) = (board("HOST"), board("END1"));
    post(&mut e1, "Template", None);
    scan(&mut e1);
    let p = prepare(&mut e1, "HOST");
    let mut parent = Batch::decode(&p.bytes).unwrap();
    let mut child = parent.clone();
    child.messages[0].id = MessageId::new("END1:00000000000000000000000000000099").unwrap();
    child.messages[0].reply = Some(parent.messages[0].id.clone());
    child.messages[0].subject = "Early reply".into();
    host.db
        .circuitnet_import(
            &host.store,
            &net(),
            &node("END1"),
            &child.encode().unwrap(),
            20,
        )
        .unwrap();
    assert!(host
        .db
        .message(host.actor, host.conference, 1)
        .unwrap()
        .parent_message_id
        .is_none());
    host.db
        .circuitnet_import(
            &host.store,
            &net(),
            &node("END1"),
            &parent.encode().unwrap(),
            21,
        )
        .unwrap();
    assert_eq!(
        host.db
            .message(host.actor, host.conference, 1)
            .unwrap()
            .parent_message_id,
        Some(host.db.message(host.actor, host.conference, 2).unwrap().id)
    );
    parent.messages[0].id = MessageId::new("END1:00000000000000000000000000000097").unwrap();
    child.messages[0].id = MessageId::new("END1:00000000000000000000000000000098").unwrap();
    parent.messages[0].reply = Some(child.messages[0].id.clone());
    child.messages[0].reply = Some(parent.messages[0].id.clone());
    parent.messages.push(child.messages.remove(0));
    assert!(matches!(
        host.db.circuitnet_import(
            &host.store,
            &net(),
            &node("END1"),
            &parent.encode().unwrap(),
            22
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(count(&host), 2);
}
#[test]
fn receipt_is_bound_to_exact_neighbor_artifact_and_members() {
    let (mut host, mut e1) = (board("HOST"), board("END1"));
    post(&mut host, "One", None);
    scan(&mut host);
    let p = prepare(&mut host, "END1");
    let r = e1
        .db
        .circuitnet_import(&e1.store, &net(), &node("HOST"), &p.bytes, 15)
        .unwrap();
    assert!(host
        .db
        .circuitnet_acknowledge(&host.store, &net(), &node("END2"), &r.receipt, 16)
        .is_err());
    let mut bad = Receipt::decode(&r.receipt).unwrap();
    bad.artifact = "0".repeat(64);
    assert!(host
        .db
        .circuitnet_acknowledge(
            &host.store,
            &net(),
            &node("END1"),
            &bad.encode().unwrap(),
            16
        )
        .is_err());
    assert_eq!(host.db.circuitnet_status(&net()).unwrap().accepted, 0);
    assert_eq!(
        host.db
            .circuitnet_acknowledge(&host.store, &net(), &node("END1"), &r.receipt, 17)
            .unwrap(),
        1
    );
    assert_eq!(
        host.db
            .circuitnet_acknowledge(&host.store, &net(), &node("END1"), &r.receipt, 18)
            .unwrap(),
        0
    );
}
#[test]
fn admission_failure_after_first_message_rolls_back_entire_batch() {
    let (mut host, mut e1) = (board("HOST"), board("END1"));
    post(&mut e1, "One", None);
    scan(&mut e1);
    let p = prepare(&mut e1, "HOST");
    let mut b = Batch::decode(&p.bytes).unwrap();
    let mut bad = b.messages[0].clone();
    bad.id = MessageId::new("END1:00000000000000000000000000000099").unwrap();
    bad.codename = code("DENIED");
    b.messages.push(bad);
    assert!(host
        .db
        .circuitnet_import(&host.store, &net(), &node("END1"), &b.encode().unwrap(), 20)
        .is_err());
    assert_eq!(count(&host), 0);
    assert_eq!(host.db.circuitnet_status(&net()).unwrap().pending, 0);
}
