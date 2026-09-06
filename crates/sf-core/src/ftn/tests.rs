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
use crate::*;
use std::sync::Mutex;
const NOW: i64 = 1_788_609_600;
#[derive(Default)]
struct Store(Mutex<BTreeMap<String, Vec<u8>>>, network::ImportCapacity);
impl NetworkArtifactStore for Store {
    fn admit_import(&self) -> Result<network::ImportPermit<'_>, network::NetworkError> {
        self.1.acquire()
    }
    fn preserve(&self, b: &[u8]) -> Result<String, network::NetworkError> {
        let id = sf_net::qwk::digest(b);
        self.0.lock().unwrap().insert(id.clone(), b.to_vec());
        Ok(id)
    }
    fn usage(&self) -> Result<(u64, usize), network::NetworkError> {
        let b = self.0.lock().unwrap();
        Ok((b.values().map(|b| b.len() as u64).sum(), b.len()))
    }
}
fn policy() -> Policy {
    let local: Endpoint = "10:100/1@synthetic".parse().unwrap();
    let point: Endpoint = "10:100/1.3@synthetic".parse().unwrap();
    Policy {
        enabled: true,
        akas: vec![
            Aka {
                id: "node".into(),
                endpoint: local,
                enabled: true,
                primary: true,
            },
            Aka {
                id: "point".into(),
                endpoint: point,
                enabled: true,
                primary: false,
            },
            Aka {
                id: "other".into(),
                endpoint: "10:100/1@othernet".parse().unwrap(),
                enabled: true,
                primary: true,
            },
        ],
        links: vec![
            Link {
                id: "peer".into(),
                remote: "10:100/2@synthetic".parse().unwrap(),
                aka: "point".into(),
                enabled: true,
                inbound: true,
                outbound: true,
                transit: true,
                profile: PacketProfile::Type2Plus,
                charset: Charset::Utf8,
            },
            Link {
                id: "next".into(),
                remote: "10:100/4@synthetic".parse().unwrap(),
                aka: "node".into(),
                enabled: true,
                inbound: true,
                outbound: true,
                transit: true,
                profile: PacketProfile::Type2Plus,
                charset: Charset::Utf8,
            },
        ],
        routes: vec![
            Route {
                domain: "synthetic".parse().unwrap(),
                target: RouteMatch::Default,
                link: "next".into(),
            },
            Route {
                domain: "synthetic".parse().unwrap(),
                target: RouteMatch::Boss {
                    address: "10:100/2".parse().unwrap(),
                },
                link: "peer".into(),
            },
        ],
        sources: vec![
            DirectorySource {
                id: "nodes".into(),
                domain: "synthetic".parse().unwrap(),
                enabled: true,
                format: wire::directory::DirectoryFormat::Nodelist,
                charset: Charset::Ascii,
                default_zone: 10,
                priority: 10,
                cadence_days: 7,
                require_crc: false,
            },
            DirectorySource {
                id: "points".into(),
                domain: "synthetic".parse().unwrap(),
                enabled: true,
                format: wire::directory::DirectoryFormat::Boss,
                charset: Charset::Cp866,
                default_zone: 10,
                priority: 0,
                cadence_days: 7,
                require_crc: false,
            },
        ],
    }
}
struct Fixture {
    temp: tempfile::TempDir,
    db: RuntimeDatabase,
    store: Store,
    policy: Policy,
    actor: MessageActor,
    other: MessageActor,
    areas: Vec<i64>,
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let mut db = RuntimeDatabase::open(&temp.path().join("board.db")).unwrap();
    db.migrate().unwrap();
    db.ensure_board_identity(&BoardIdentity::new("Synthetic FTN Board", "Sysop").unwrap())
        .unwrap();
    let hash = CredentialHasher::new(&PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    })
    .unwrap()
    .hash(b"synthetic-password")
    .unwrap();
    let mut actors = vec![];
    for name in [b"Recipient".as_slice(), b"Other"] {
        let c = db
            .create_caller(
                name,
                &hash,
                SecurityLevel::new(50).unwrap(),
                CallerState::Active,
                false,
                NOW,
            )
            .unwrap();
        actors.push(MessageActor::new(c.id, SecurityLevel::new(50).unwrap()));
    }
    let policy = policy();
    let mut areas = vec![];
    for n in [1, 2] {
        let c = db
            .ensure_conference(&ConferenceDefinition {
                number: n,
                name: format!("Synthetic {n}"),
                description: "FTN tests".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![],
            })
            .unwrap();
        areas.push(c.id.get());
        db.configure_ftn_mapping(
            &policy,
            "synthetic-operator",
            &Mapping {
                domain: "synthetic".parse().unwrap(),
                area: format!("TEST{n}"),
                conference_id: c.id.get(),
                aka: "node".into(),
                receive: true,
                send: true,
                origin: "Synthetic Board".into(),
                links: vec!["peer".into(), "next".into()],
                version: 1,
            },
            0,
            NOW,
        )
        .unwrap();
    }
    for aka in ["node", "point"] {
        db.configure_ftn_alias(
            &policy,
            "synthetic-operator",
            &MailboxAlias {
                aka: aka.into(),
                alias: "Recipient".into(),
                caller_id: actors[0].caller_id().get(),
            },
            NOW,
        )
        .unwrap();
    }
    Fixture {
        temp,
        db,
        store: Store::default(),
        policy,
        actor: actors[0],
        other: actors[1],
        areas,
    }
}
fn inbound(seq: u32, destination: &str, area: Option<&str>) -> Vec<u8> {
    let origin: Address = "10:100/2.9".parse().unwrap();
    let dest: Address = destination.parse().unwrap();
    let now = chrono::DateTime::from_timestamp(NOW, 0)
        .unwrap()
        .naive_utc();
    let text = if let Some(area) = area {
        format!("AREA:{area}\r\x01MSGID: {origin} {seq:08x}\r\x01CHRS: UTF-8 4\r\x01TZUTC: -0700\rHuman message café\r\x01EXPERIMENT: retain\r--- Synthetic Peer\r * Origin: Synthetic Peer ({origin})\rSEEN-BY: 100/2\r\x01PATH: 100/2\r")
    } else {
        format!("\x01MSGID: {origin} {seq:08x}\r\x01INTL {} {}\r\x01FMPT 9\r\x01TOPT {}\r\x01CHRS: UTF-8 4\r\x01TZUTC: -0700\rPrivate FTN body\r",dest.boss(),origin.boss(),dest.point())
    };
    let text = if dest.point() == 0 {
        text.replace("\x01TOPT 0\r", "")
    } else {
        text
    };
    Packet {
        header: PacketHeader {
            profile: PacketProfile::Type2Plus,
            origin: "10:100/2".parse().unwrap(),
            destination: "10:100/1.3".parse().unwrap(),
            created: now,
            password: [0; 8],
            product: [254, 0, 0, 1],
            product_data: [0; 4],
            baud: 0,
            spare: [0; 20],
        },
        messages: vec![PackedMessage {
            origin: origin.two_d(),
            destination: dest.two_d(),
            attributes: if area.is_some() { 0 } else { 1 },
            cost: 0,
            date: wire::format_date(now).unwrap(),
            from: b"Remote Author".to_vec(),
            to: b"Recipient".to_vec(),
            subject: b"FTN synthetic".to_vec(),
            text: text.into_bytes(),
        }],
    }
    .encode()
    .unwrap()
}
#[test]
fn native_point_netmail_queue_build_restart_and_mailbox_separation() {
    let mut f = fixture();
    let mid =
        f.db.send_ftn_mail(
            f.actor,
            &f.policy,
            &NewNetMail {
                aka: "point".into(),
                destination: "10:100/2.9@synthetic".parse().unwrap(),
                recipient: "Remote".into(),
                subject: "Point test".into(),
                body: "Unicode café".into(),
                reply_to: None,
            },
            NOW,
        )
        .unwrap();
    assert!(f.db.read_ftn_mail(f.other, mid).is_err());
    assert_eq!(
        f.db.read_ftn_mail(f.actor, mid).unwrap().body,
        "Unicode café"
    );
    assert!(f.db.qwk_mailbox(f.actor, None).unwrap().is_empty());
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    assert_eq!(q.final_destination.address.point(), 9);
    assert_eq!(q.next_hop.address.point(), 0);
    assert_eq!(q.link, "peer");
    let a =
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    let bytes = f.store.0.lock().unwrap()[&a].clone();
    let p = Packet::decode(&bytes, (10, 10)).unwrap();
    assert_eq!(p.header.origin.point(), 3);
    let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
    assert_eq!(t.fmpt, 3);
    assert_eq!(t.topt, 9);
    assert_eq!(t.body, "Unicode café");
    assert_eq!(f.db.scan_ftn(&f.policy, NOW).unwrap(), 0);
    drop(f.db);
    let db = RuntimeDatabase::open(&f.temp.path().join("board.db")).unwrap();
    assert_eq!(db.ftn_queue(None).unwrap().len(), 1);
}
#[test]
fn inbound_point_private_delivery_replay_collision_and_transit() {
    let mut f = fixture();
    let b = inbound(1, "10:100/1.3", None);
    let r = f.db.toss_ftn(&f.store, &f.policy, "peer", &b, NOW).unwrap();
    assert_eq!(r.imported, 1);
    let mid = MessageId::new(
        f.db.connection
            .query_row("SELECT message_id FROM ftn_messages", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        f.db.read_ftn_mail(f.actor, mid)
            .unwrap()
            .source
            .address
            .point(),
        9
    );
    assert!(f.db.read_ftn_mail(f.other, mid).is_err());
    f.db.toss_ftn(&f.store, &f.policy, "peer", &b, NOW).unwrap();
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM ftn_messages", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    let mut p = Packet::decode(&b, (10, 10)).unwrap();
    p.header.created += chrono::Duration::seconds(1);
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &p.encode().unwrap(), NOW)
            .unwrap()
            .duplicates,
        1
    );
    p.messages[0].subject = b"collision".to_vec();
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &p.encode().unwrap(), NOW)
            .unwrap()
            .quarantined,
        1
    );
    let transit = inbound(2, "10:200/9.8", None);
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &transit, NOW)
            .unwrap()
            .imported,
        1
    );
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    assert_eq!(q.final_destination.address, "10:200/9.8".parse().unwrap());
    assert_eq!(q.next_hop.address, "10:100/4".parse().unwrap());
    let a =
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    let p = Packet::decode(&f.store.0.lock().unwrap()[&a], (10, 10)).unwrap();
    let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
    assert_eq!(t.topt, 8);
    assert_eq!(
        t.addresses(&p.messages[0], &p.header).unwrap().1,
        "10:200/9.8".parse().unwrap()
    );
}
#[test]
fn echomail_native_scan_toss_fanout_and_loop() {
    let mut f = fixture();
    let posted =
        f.db.post(
            f.actor,
            NewMessage {
                conference_id: ConferenceId::new(f.areas[0]).unwrap(),
                recipient_caller_id: None,
                recipient_name: "All Callers".into(),
                subject: b"Local Echo".to_vec(),
                body: b"Body\r\n".to_vec(),
                created_at: NOW,
                parent_message_id: None,
                visibility: MessageVisibility::Public,
                kind: MessageKind::Standard,
            },
        )
        .unwrap();
    assert_eq!(f.db.scan_ftn(&f.policy, NOW).unwrap(), 1);
    assert_eq!(f.db.scan_ftn(&f.policy, NOW).unwrap(), 0);
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), 2);
    // Queue identifiers are random; select the node AKA explicitly. The other
    // configured link uses a real point AKA, which must not append its boss to PATH.
    let q =
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .find(|q| q.link == "next")
            .unwrap();
    let a =
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    let p = Packet::decode(&f.store.0.lock().unwrap()[&a], (10, 10)).unwrap();
    let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
    assert_eq!(t.area.as_deref(), Some("TEST1"));
    assert!(t.seen_by.contains(&(100, 1)));
    assert_eq!(t.path, vec![(100, 1)]);
    let q =
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .find(|q| q.link == "peer")
            .unwrap();
    let a =
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    let p = Packet::decode(&f.store.0.lock().unwrap()[&a], (10, 10)).unwrap();
    let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
    assert!(t.path.is_empty());
    assert_eq!(p.header.origin.point(), 3);
    let b = inbound(3, "10:100/1", Some("TEST2"));
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &b, NOW)
            .unwrap()
            .imported,
        1
    );
    let mid =
        f.db.connection
            .query_row(
                "SELECT message_id FROM ftn_messages WHERE ingress_link='peer'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
    let native =
        f.db.message(f.actor, ConferenceId::new(f.areas[1]).unwrap(), 1)
            .unwrap();
    assert!(!String::from_utf8(native.body).unwrap().contains("MSGID"));
    assert_ne!(mid, posted.id.get());
    let mut p = Packet::decode(&inbound(4, "10:100/1", Some("TEST2")), (10, 10)).unwrap();
    let text = String::from_utf8(p.messages[0].text.clone())
        .unwrap()
        .replace("PATH: 100/2", "PATH: 100/2 1 4");
    p.messages[0].text = text.into_bytes();
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &p.encode().unwrap(), NOW)
            .unwrap()
            .loops,
        1
    );
}
#[test]
fn admission_quarantine_resource_and_atomic_receipt_failure() {
    let mut f = fixture();
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", b"bad", NOW)
            .unwrap()
            .quarantined,
        1
    );
    assert!(f
        .db
        .toss_ftn(
            &f.store,
            &f.policy,
            "peer",
            &vec![0; wire::MAX_PACKET + 1],
            NOW
        )
        .is_err());
    assert!(f
        .db
        .toss_ftn(&f.store, &f.policy, "unknown", b"bad", NOW)
        .is_err());
    assert_eq!(
        f.db.toss_ftn(
            &f.store,
            &f.policy,
            "peer",
            &inbound(5, "10:100/1", Some("UNKNOWN")),
            NOW
        )
        .unwrap()
        .quarantined,
        1
    );
    f.db.connection.execute_batch("CREATE TRIGGER fail_ftn_receipt BEFORE INSERT ON ftn_import_receipts BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(f
        .db
        .toss_ftn(
            &f.store,
            &f.policy,
            "peer",
            &inbound(6, "10:100/1.3", None),
            NOW
        )
        .is_err());
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM ftn_messages", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    let dump =
        f.db.connection
            .prepare("SELECT operation FROM operator_control_audit")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join(" ");
    assert!(!dump.contains("Private FTN body"));
}
#[test]
fn directory_generation_conflict_staleness_backup_restore() {
    let mut f = fixture();
    let date = chrono::DateTime::from_timestamp(NOW, 0)
        .unwrap()
        .date_naive();
    let b = b";A Synthetic\r\nBoss,10:100/2\r\n,9,Point,Here,Person,-Unpublished-,300,IBN\r\n";
    let g =
        f.db.ingest_ftn_directory(&f.store, &f.policy, "points", date, b, NOW)
            .unwrap();
    assert_eq!(g.state, "validated");
    let ep = "10:100/2.9@synthetic".parse().unwrap();
    assert!(f
        .db
        .lookup_ftn_directory(&f.policy, &ep, NOW)
        .unwrap()
        .is_none());
    f.db.activate_ftn_directory(&f.policy, "synthetic-operator", &g.id, 0, NOW)
        .unwrap();
    assert_eq!(
        f.db.lookup_ftn_directory(&f.policy, &ep, NOW)
            .unwrap()
            .unwrap()
            .source,
        "points"
    );
    let bad = [
        b.as_slice(),
        b",9,Conflict,Here,Person,-Unpublished-,300\r\n",
    ]
    .concat();
    let rejected =
        f.db.ingest_ftn_directory(&f.store, &f.policy, "points", date, &bad, NOW)
            .unwrap();
    assert_eq!(rejected.state, "rejected");
    assert!(f
        .db
        .activate_ftn_directory(&f.policy, "synthetic-operator", &rejected.id, 1, NOW)
        .is_err());
    assert_eq!(
        f.db.lookup_ftn_directory(&f.policy, &ep, NOW)
            .unwrap()
            .unwrap()
            .generation,
        g.id
    );
    assert!(f
        .db
        .lookup_ftn_directory(&f.policy, &ep, NOW + 29 * 86400)
        .unwrap()
        .is_none());
    let backup = f.temp.path().join("backup.db");
    f.db.backup_to(&backup).unwrap();
    let mut restored = RuntimeDatabase::open(&backup).unwrap();
    restored.hold_restored_ftn().unwrap();
    assert_eq!(
        restored
            .lookup_ftn_directory(&f.policy, &ep, NOW)
            .unwrap()
            .unwrap()
            .generation,
        g.id
    );
    assert!(restored
        .send_ftn_mail(
            f.actor,
            &f.policy,
            &NewNetMail {
                aka: "point".into(),
                destination: "10:100/2.9@synthetic".parse().unwrap(),
                recipient: "Remote".into(),
                subject: "Held".into(),
                body: "Held".into(),
                reply_to: None
            },
            NOW
        )
        .is_err());
}
#[test]
fn route_precedence_and_domain_namespace() {
    let mut p = policy();
    let ep: Endpoint = "10:100/2.9@synthetic".parse().unwrap();
    assert_eq!(p.route(&ep).unwrap().reason, "boss");
    p.routes.push(Route {
        domain: ep.domain.clone(),
        target: RouteMatch::Exact {
            address: ep.address,
        },
        link: "next".into(),
    });
    assert_eq!(p.route(&ep).unwrap().reason, "exact");
    p.validate().unwrap();
    p.routes.push(p.routes.last().unwrap().clone());
    assert!(p.validate().is_err());
    assert!(policy()
        .route(&"10:100/2.9@othernet".parse().unwrap())
        .is_err());
}

/// Private acceptance artifacts are supplied by the operator, never checked into the repository.
#[test]
#[ignore = "requires independently generated packets in SPITFIRE_FTN_PEER_DIR"]
fn independent_peer_packet_exchange() {
    let root = std::path::PathBuf::from(std::env::var_os("SPITFIRE_FTN_PEER_DIR").unwrap());
    let mut f = fixture();
    let mut imported = 0;
    for name in ["peer-netmail.pkt", "peer-echo.pkt"] {
        let bytes = std::fs::read(root.join(name)).unwrap();
        let parsed = Packet::decode(&bytes, (10, 10)).unwrap();
        assert_eq!(parsed.header.destination, "10:100/1.3".parse().unwrap());
        let r =
            f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
                .unwrap();
        assert_eq!(r.quarantined, 0, "{name}: {r:?}");
        assert!(r.imported > 0);
        imported += r.imported;
        let retry =
            f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
                .unwrap();
        assert_eq!(retry.imported, 0);
        assert!(retry.duplicates > 0);
    }
    assert!(imported >= 2);
    let incoming: i64 =
        f.db.connection
            .query_row(
                "SELECT message_id FROM ftn_messages WHERE area LIKE '@netmail/%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let received =
        f.db.read_ftn_mail(f.actor, MessageId::new(incoming).unwrap())
            .unwrap();
    assert!(received.body.contains("Independent SBBSecho NetMail body."));
    assert!(f
        .db
        .read_ftn_mail(f.other, MessageId::new(incoming).unwrap())
        .is_err());
    let native =
        f.db.message(f.actor, ConferenceId::new(f.areas[0]).unwrap(), 1)
            .unwrap();
    assert!(String::from_utf8(native.body)
        .unwrap()
        .contains("Independent SBBSecho EchoMail body."));
    f.db.send_ftn_mail(
        f.actor,
        &f.policy,
        &NewNetMail {
            aka: "point".into(),
            destination: "10:100/2.9@synthetic".parse().unwrap(),
            recipient: "Sysop".into(),
            subject: "Native point NetMail".into(),
            body: "Native SPITFIRE point NetMail body.".into(),
            reply_to: None,
        },
        NOW,
    )
    .unwrap();
    f.db.post(
        f.actor,
        NewMessage {
            conference_id: ConferenceId::new(f.areas[1]).unwrap(),
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: b"Native EchoMail".to_vec(),
            body: b"Native SPITFIRE EchoMail body.\r\n".to_vec(),
            created_at: NOW,
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        },
    )
    .unwrap();
    f.db.scan_ftn(&f.policy, NOW).unwrap();
    let mut built = 0;
    for q in
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .filter(|q| q.link == "peer")
    {
        let artifact =
            f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
                .unwrap();
        let bytes = f.store.0.lock().unwrap()[&artifact].clone();
        let p = Packet::decode(&bytes, (10, 10)).unwrap();
        let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
        let name = if t.area.is_some() {
            "native-echo.pkt"
        } else {
            "native-netmail.pkt"
        };
        std::fs::write(root.join(name), bytes).unwrap();
        built += 1;
    }
    assert_eq!(built, 2);
    if root.join("peer-return.pkt").exists() {
        let returned = std::fs::read(root.join("peer-return.pkt")).unwrap();
        let result =
            f.db.toss_ftn(&f.store, &f.policy, "peer", &returned, NOW)
                .unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.quarantined, 0);
        assert_eq!(result.duplicates + result.loops, 1);
    }
    if root.join("peer-transit.pkt").exists() {
        let bytes = std::fs::read(root.join("peer-transit.pkt")).unwrap();
        let result =
            f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
                .unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.quarantined, 0);
        let q =
            f.db.ftn_queue(None)
                .unwrap()
                .into_iter()
                .find(|q| q.final_destination.address == "10:200/9.8".parse().unwrap())
                .unwrap();
        assert_eq!(q.next_hop.address, "10:100/4".parse().unwrap());
        let id =
            f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
                .unwrap();
        std::fs::write(
            root.join("native-transit.pkt"),
            &f.store.0.lock().unwrap()[&id],
        )
        .unwrap();
    }
}

#[test]
fn directory_source_priority_conflict_and_domain_isolation() {
    let mut f = fixture();
    let date = chrono::DateTime::from_timestamp(NOW, 0)
        .unwrap()
        .date_naive();
    let primary = b";A Synthetic\r\nBoss,10:100/2\r\n,9,Primary,Here,Person,-Unpublished-,300\r\n";
    let lower = b";A Synthetic\r\nBoss,10:100/2\r\n,9,Lower,There,Person,-Unpublished-,300\r\n";
    for (id, priority, domain) in [
        ("lower", 20, "synthetic"),
        ("conflict", 0, "synthetic"),
        ("separate", 0, "othernet"),
    ] {
        let mut source = f.policy.sources[1].clone();
        source.id = id.into();
        source.priority = priority;
        source.domain = domain.parse().unwrap();
        f.policy.sources.push(source);
    }
    let mut activate = |source: &str, bytes: &[u8]| {
        let g =
            f.db.ingest_ftn_directory(&f.store, &f.policy, source, date, bytes, NOW)
                .unwrap();
        assert_eq!(g.state, "validated");
        f.db.activate_ftn_directory(&f.policy, "synthetic-operator", &g.id, 0, NOW)
    };
    activate("points", primary).unwrap();
    activate("lower", lower).unwrap();
    assert!(matches!(
        activate("conflict", lower),
        Err(Error::DirectoryConflict)
    ));
    activate("separate", lower).unwrap();
    let entry =
        f.db.lookup_ftn_directory(&f.policy, &"10:100/2.9@synthetic".parse().unwrap(), NOW)
            .unwrap()
            .unwrap();
    assert_eq!(entry.system, "Primary");
    assert_eq!(entry.shadowed_sources, vec!["lower"]);
    assert_eq!(
        f.db.lookup_ftn_directory(&f.policy, &"10:100/2.9@othernet".parse().unwrap(), NOW)
            .unwrap()
            .unwrap()
            .system,
        "Lower"
    );
    assert_eq!(f.db.ftn_status().unwrap().quarantine, 1);
    // Disabled or replaced source policy is not effective and cannot veto activation.
    f.policy
        .sources
        .iter_mut()
        .find(|s| s.id == "points")
        .unwrap()
        .enabled = false;
    let candidate: String =
        f.db.connection
            .query_row(
                "SELECT generation_id FROM ftn_directory_generations WHERE source_id='conflict'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    f.db.activate_ftn_directory(&f.policy, "synthetic-operator", &candidate, 0, NOW)
        .unwrap();
    assert_eq!(
        f.db.lookup_ftn_directory(&f.policy, &"10:100/2.9@synthetic".parse().unwrap(), NOW)
            .unwrap()
            .unwrap()
            .source,
        "conflict"
    );
}
#[test]
fn local_text_cannot_supply_ftn_controls_and_stale_work_is_held() {
    let mut f = fixture();
    let mut mail = NewNetMail {
        aka: "point".into(),
        destination: "10:100/2.9@synthetic".parse().unwrap(),
        recipient: "Peer".into(),
        subject: "Native".into(),
        body: "\u{1}INTL 10:200/9 10:200/8".into(),
        reply_to: None,
    };
    assert!(f.db.send_ftn_mail(f.actor, &f.policy, &mail, NOW).is_err());
    mail.body = "AREA:FAKE\nVisible literal text".into();
    f.db.send_ftn_mail(f.actor, &f.policy, &mail, NOW).unwrap();
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    let id =
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    let p = Packet::decode(&f.store.0.lock().unwrap()[&id], (10, 10)).unwrap();
    let t = Text::parse(&p.messages[0].text, Charset::Ascii).unwrap();
    assert!(t.area.is_none());
    assert_eq!(t.body, mail.body);
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    f.policy.links[0].outbound = false;
    assert!(f
        .db
        .build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
        .is_err());
    assert_eq!(f.db.ftn_queue(None).unwrap()[0].state, "held");
}

#[test]
fn netmail_reply_binding_and_missing_identity_are_conservative() {
    let mut f = fixture();
    let bytes = inbound(90, "10:100/1.3", None);
    f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
        .unwrap();
    let parent: i64 =
        f.db.connection
            .query_row("SELECT message_id FROM ftn_messages", [], |r| r.get(0))
            .unwrap();
    let reply =
        f.db.send_ftn_mail(
            f.actor,
            &f.policy,
            &NewNetMail {
                aka: "point".into(),
                destination: "10:100/2.9@synthetic".parse().unwrap(),
                recipient: "Remote Author".into(),
                subject: "Reply".into(),
                body: "Native reply".into(),
                reply_to: Some(MessageId::new(parent).unwrap()),
            },
            NOW,
        )
        .unwrap();
    assert_eq!(
        f.db.connection
            .query_row(
                "SELECT reply FROM ftn_messages WHERE message_id=?1",
                [reply.get()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "10:100/2.9 0000005a"
    );
    let mut weak = Packet::decode(&inbound(91, "10:100/1", None), (10, 10)).unwrap();
    weak.messages[0].text = String::from_utf8(weak.messages[0].text.clone())
        .unwrap()
        .replace("\x01MSGID: 10:100/2.9 0000005b\r", "")
        .into_bytes();
    let bytes = weak.encode().unwrap();
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
            .unwrap()
            .imported,
        1
    );
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
            .unwrap()
            .duplicates,
        1
    );
    weak.header.product_data[0] = 1;
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &weak.encode().unwrap(), NOW)
            .unwrap()
            .quarantined,
        1
    );
}

fn binkp_policy() -> BinkpPolicy {
    BinkpPolicy {
        listener: Some(BinkpListener {
            enabled: true,
            bind: "127.0.0.1:24554".parse().unwrap(),
            akas: vec!["node".into(), "point".into()],
        }),
        links: ["peer", "next"]
            .into_iter()
            .map(|link| BinkpLink {
                link: link.into(),
                enabled: true,
                inbound: true,
                outbound: true,
                endpoint: Some("127.0.0.1".into()),
                port: 24554,
                directory: false,
                akas: vec!["node".into(), "point".into()],
                remote_akas: vec![],
                auth: BinkpAuth::RequireCram,
                allow_domainless: false,
            })
            .collect(),
    }
}
fn queued_binkp(f: &mut Fixture) {
    f.db.send_ftn_mail(
        f.actor,
        &f.policy,
        &NewNetMail {
            aka: "point".into(),
            destination: "10:100/2.9@synthetic".parse().unwrap(),
            recipient: "Remote".into(),
            subject: "Transport boundary".into(),
            body: "Private transport payload".into(),
            reply_to: None,
        },
        NOW,
    )
    .unwrap();
}
#[test]
fn binkp_claim_ack_and_restart_use_the_native_queue() {
    let mut f = fixture();
    let policy = binkp_policy();
    queued_binkp(&mut f);
    let session =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation-one",
            BinkpMode::Poll,
            NOW,
        )
        .unwrap();
    assert!(f
        .db
        .begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation-one",
            BinkpMode::Poll,
            NOW
        )
        .is_err());
    let work =
        f.db.claim_binkp(&f.policy, &policy, &f.store, &session, NOW)
            .unwrap();
    assert_eq!(work.len(), 1);
    assert!(f.db.accepted_binkp(&session, &work[0].queue, NOW).is_err());
    f.db.offered_binkp(&session, &work[0].queue).unwrap();
    assert_ne!(f.db.ftn_queue(None).unwrap()[0].state, "accepted");
    f.db.recover_binkp(NOW + 1).unwrap();
    assert_eq!(f.db.ftn_queue(None).unwrap()[0].state, "retry");
    assert!(!f.db.binkp_health().unwrap()[0].active);
    assert!(f
        .db
        .begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation-two",
            BinkpMode::Poll,
            NOW + 2
        )
        .is_err());
    let session =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation-two",
            BinkpMode::Poll,
            NOW + 1000,
        )
        .unwrap();
    let retry =
        f.db.claim_binkp(&f.policy, &policy, &f.store, &session, NOW + 1000)
            .unwrap();
    assert_eq!(retry[0].artifact, work[0].artifact);
    f.db.offered_binkp(&session, &retry[0].queue).unwrap();
    f.db.accepted_binkp(&session, &retry[0].queue, NOW + 1000)
        .unwrap();
    assert!(f
        .db
        .accepted_binkp(&session, &retry[0].queue, NOW + 1000)
        .is_err());
    f.db.finish_binkp(&session, None, NOW + 1000).unwrap();
    f.db.recover_binkp(NOW + 1001).unwrap();
    assert_eq!(f.db.ftn_queue(None).unwrap()[0].state, "accepted");
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM network_delivery_attempts", [], |r| {
                r.get::<_, u32>(0)
            })
            .unwrap(),
        2
    );
}
#[test]
fn binkp_restore_holds_work_and_preserves_receipts_and_identity() {
    let mut f = fixture();
    let policy = binkp_policy();
    queued_binkp(&mut f);
    let session =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "before-restore",
            BinkpMode::Poll,
            NOW,
        )
        .unwrap();
    let work =
        f.db.claim_binkp(&f.policy, &policy, &f.store, &session, NOW)
            .unwrap();
    f.db.recover_binkp(NOW + 1).unwrap();
    f.db.hold_restored_ftn().unwrap();
    assert_eq!(f.db.ftn_queue(None).unwrap()[0].state, "held");
    let queue = f.db.ftn_queue(None).unwrap().remove(0);
    assert!(f
        .db
        .release_binkp(&f.policy, "operator", &queue.id, queue.version - 1, NOW + 2)
        .is_err());
    f.db.release_binkp(&f.policy, "operator", &queue.id, queue.version, NOW + 2)
        .unwrap();
    let session =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "after-restore",
            BinkpMode::Poll,
            NOW + 3,
        )
        .unwrap();
    assert_eq!(
        f.db.claim_binkp(&f.policy, &policy, &f.store, &session, NOW + 3)
            .unwrap()[0]
            .artifact,
        work[0].artifact
    );
    assert_eq!(
        f.db.connection
            .query_row("SELECT MIN(restored_hold) FROM ftn_serials", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
}
#[test]
fn binkp_configuration_domain_limits_and_safe_observations() {
    let mut f = fixture();
    let mut policy = binkp_policy();
    policy.validate(&f.policy).unwrap();
    policy.links[0].akas.push("other".into());
    assert!(policy.validate(&f.policy).is_err());
    policy.links[0].akas.pop();
    policy.links[0].endpoint = Some("host/path".into());
    assert!(policy.validate(&f.policy).is_err());
    policy.links[0].endpoint = Some("127.0.0.1".into());
    let endpoint = f.db.resolve_binkp(&f.policy, &policy, "peer", NOW).unwrap();
    assert_eq!(endpoint.source, "override");
    let session =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "safe-generation",
            BinkpMode::Test,
            NOW,
        )
        .unwrap();
    f.db.observe_binkp(
        &session,
        &[f.policy.links[0].remote.clone()],
        &["CRAM-MD5".into()],
        42,
        NOW,
    )
    .unwrap();
    f.db.finish_binkp(&session, Some(sf_net::binkp::Error::Authentication), NOW)
        .unwrap();
    let health = f.db.binkp_health().unwrap();
    assert!(health[0].held);
    assert_eq!(health[0].last_error.as_deref(), Some("authentication"));
    assert_eq!(health[0].latency_ms, Some(42));
    assert!(f
        .db
        .begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "safe-generation",
            BinkpMode::Test,
            NOW + 1
        )
        .is_err());
    let json = serde_json::to_string(&health).unwrap();
    assert!(!json.contains("Private transport payload"));
}

#[test]
fn binkp_two_links_share_capacity_and_held_work_requires_review() {
    let mut f = fixture();
    let policy = binkp_policy();
    queued_binkp(&mut f);
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    f.db.hold_binkp(&f.policy, "operator", &q.id, q.version, NOW)
        .unwrap();
    let a =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation",
            BinkpMode::Poll,
            NOW,
        )
        .unwrap();
    let b =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "next",
            "generation",
            BinkpMode::Poll,
            NOW,
        )
        .unwrap();
    assert!(f
        .db
        .begin_binkp(&f.policy, &policy, "peer", "other", BinkpMode::Poll, NOW)
        .is_err());
    assert_eq!(
        f.db.binkp_health()
            .unwrap()
            .iter()
            .filter(|h| h.active)
            .count(),
        2
    );
    assert!(f
        .db
        .claim_binkp(&f.policy, &policy, &f.store, &a, NOW)
        .unwrap()
        .is_empty());
    f.db.finish_binkp(&a, None, NOW).unwrap();
    f.db.finish_binkp(&b, None, NOW).unwrap();
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    f.db.release_binkp(&f.policy, "operator", &q.id, q.version, NOW + 1)
        .unwrap();
    let a =
        f.db.begin_binkp(
            &f.policy,
            &policy,
            "peer",
            "generation",
            BinkpMode::Poll,
            NOW + 2,
        )
        .unwrap();
    let work =
        f.db.claim_binkp(&f.policy, &policy, &f.store, &a, NOW + 2)
            .unwrap();
    assert_eq!(work.len(), 1);
    let q = f.db.ftn_queue(None).unwrap().remove(0);
    assert!(f
        .db
        .hold_binkp(&f.policy, "operator", &q.id, q.version, NOW + 2)
        .is_err());
    assert!(f
        .db
        .release_binkp(&f.policy, "operator", &q.id, q.version, NOW + 2)
        .is_err());
    f.db.finish_binkp(&a, Some(sf_net::binkp::Error::Interrupted), NOW + 3)
        .unwrap();
    assert_eq!(f.db.ftn_queue(None).unwrap()[0].state, "retry");
}
#[test]
fn binkp_directory_service_resolution_preserves_override_and_generation() {
    let mut f = fixture();
    let mut policy = binkp_policy();
    f.policy.links[0].remote = "10:100/2.9@synthetic".parse().unwrap();
    let bytes=b";A Synthetic\r\nBoss,10:100/2\r\n,9,Point,Here,Person,-Unpublished-,300,INA:localhost,IBN:24555\r\n";
    let generation =
        f.db.ingest_ftn_directory(
            &f.store,
            &f.policy,
            "points",
            chrono::NaiveDate::from_ymd_opt(2026, 9, 5).unwrap(),
            bytes,
            NOW,
        )
        .unwrap();
    f.db.activate_ftn_directory(&f.policy, "operator", &generation.id, 0, NOW)
        .unwrap();
    let direct = f.db.resolve_binkp(&f.policy, &policy, "peer", NOW).unwrap();
    assert_eq!(direct.host, "127.0.0.1");
    policy.links[0].endpoint = None;
    policy.links[0].directory = true;
    let resolved = f.db.resolve_binkp(&f.policy, &policy, "peer", NOW).unwrap();
    assert_eq!(resolved.host, "localhost");
    assert_eq!(resolved.port, 24555);
    assert_eq!(resolved.generation, Some(generation.id));
    assert!(f
        .db
        .resolve_binkp(&f.policy, &policy, "peer", NOW + 400 * 86400)
        .is_err());
}

#[test]
fn recovery_transfers_verified_floor_and_acceptance_without_identity_reuse() {
    let mut f = fixture();
    let transport = binkp_policy();
    for _ in 0..5 {
        queued_binkp(&mut f);
    }
    let session =
        f.db.begin_binkp(
            &f.policy,
            &transport,
            "peer",
            "before-backup",
            BinkpMode::Poll,
            NOW,
        )
        .unwrap();
    let work =
        f.db.claim_binkp(&f.policy, &transport, &f.store, &session, NOW)
            .unwrap();
    f.db.finish_binkp(&session, None, NOW).unwrap();
    let snapshot = f.temp.path().join("older.db");
    f.db.backup_to(&snapshot).unwrap();
    // Later real acceptance and additional origination are absent from snapshot.
    let session =
        f.db.begin_binkp(
            &f.policy,
            &transport,
            "peer",
            "after-backup",
            BinkpMode::Poll,
            NOW + 1000,
        )
        .unwrap();
    let later =
        f.db.claim_binkp(&f.policy, &transport, &f.store, &session, NOW + 1000)
            .unwrap();
    for item in &later {
        f.db.offered_binkp(&session, &item.queue).unwrap();
        f.db.accepted_binkp(&session, &item.queue, NOW + 1000)
            .unwrap();
    }
    f.db.finish_binkp(&session, None, NOW + 1000).unwrap();
    for _ in 0..8 {
        queued_binkp(&mut f);
    }
    let evidence = f.db.ftn_recovery_evidence().unwrap();
    let old_ids: BTreeSet<String> =
        f.db.connection
            .prepare("SELECT msgid FROM ftn_messages")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
    f.db.hold_restored_ftn().unwrap();
    let mut restored = RuntimeDatabase::open(&snapshot).unwrap();
    restored.recover_binkp(NOW + 2000).unwrap();
    restored.hold_restored_ftn().unwrap();
    let result = restored
        .reconcile_ftn_recovery(&evidence, "test-operator", NOW + 2000)
        .unwrap();
    assert_eq!(result.accepted, work.len() as u32);
    assert!(restored
        .ftn_queue(None)
        .unwrap()
        .iter()
        .all(|q| q.state == "accepted"));
    f.db = restored;
    queued_binkp(&mut f);
    let new_id: String =
        f.db.connection
            .query_row(
                "SELECT msgid FROM ftn_messages ORDER BY rowid DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert!(!old_ids.contains(&new_id));
    assert_eq!(new_id.split_whitespace().last(), Some("0000000e"));
    // Idempotent evidence replay never rewinds the counter or revives accepted work.
    let floor = f.db.ftn_origin_states().unwrap();
    f.db.reconcile_ftn_recovery(&evidence, "test-operator", NOW + 2001)
        .unwrap();
    assert_eq!(f.db.ftn_origin_states().unwrap(), floor);
    assert_eq!(
        f.db.ftn_queue(None)
            .unwrap()
            .iter()
            .filter(|q| q.state == "accepted")
            .count(),
        5
    );
}

#[test]
fn uncertain_recovery_source_cannot_clear_an_origin_hold() {
    let mut f = fixture();
    queued_binkp(&mut f);
    f.db.hold_restored_ftn().unwrap();
    let evidence = f.db.ftn_recovery_evidence().unwrap();
    let result =
        f.db.reconcile_ftn_recovery(&evidence, "test-operator", NOW)
            .unwrap();
    assert!(result.held > 0);
    assert!(f.db.ftn_origin_states().unwrap().iter().all(|s| s.held));
}

#[test]
fn network_projection_is_bounded_and_excludes_private_payloads() {
    let mut f = fixture();
    queued_binkp(&mut f);
    for section in NetworkSection::ALL {
        let page =
            f.db.network_page(&NetworkQuery { section, offset: 0 })
                .unwrap();
        let text = serde_json::to_string(&page).unwrap();
        assert!(!text.contains("Private transport payload"));
        assert!(!text.contains("Transport boundary"));
        assert!(text.len() < 524288);
        if section == NetworkSection::Queues {
            assert_eq!(page.queues.len(), 1);
            let q = &page.queues[0];
            assert_eq!(q.final_destination.as_ref().unwrap().address.point(), 9);
            assert_eq!(q.next_hop.as_ref().unwrap().address.point(), 0);
            assert_eq!(q.kind, "netmail");
        }
    }
    assert!(f
        .db
        .network_page(&NetworkQuery {
            section: NetworkSection::Queues,
            offset: 2_000_001
        })
        .is_err());
}

#[test]
fn configuration_references_reject_orphans_and_allow_disabling() {
    let f = fixture();
    let mut changed = f.policy.clone();
    changed.akas.retain(|a| a.id != "node");
    assert!(f.db.validate_ftn_policy_references(&changed).is_err());
    let mut changed = f.policy.clone();
    changed.links[0].remote = "10:100/99@synthetic".parse().unwrap();
    assert!(f.db.validate_ftn_policy_references(&changed).is_err());
    let mut changed = f.policy.clone();
    changed.enabled = false;
    assert!(f.db.validate_ftn_policy_references(&changed).is_ok());
}

#[path = "hub_tests.rs"]
mod hub_tests;
