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

//! Six disposable native boards prove signed catalog propagation and local autonomy.
use super::*;
use sf_core::circuitnet::catalog::{Authority, Body, Entry, Intent, Lifecycle, Signed};
fn next(s: &Signed) -> Body {
    let mut b = s.body.clone();
    b.revision += 1;
    b.previous_revision = s.body.revision;
    b.previous_hash = Some(s.hash.clone());
    b.published_at += 1;
    b
}
fn publish(b: &Board, key: &[u8], body: Body) -> Signed {
    db(b)
        .circuitnet_catalog_publish(
            "acceptance-operator",
            body,
            key,
            chrono::Utc::now().timestamp(),
        )
        .unwrap()
}
fn revision(b: &Board) -> u64 {
    db(b)
        .circuitnet_catalog_status(&network())
        .unwrap()
        .revision
}
fn new_native(_b: &Board, number: u16) -> ConferenceDefinition {
    ConferenceDefinition {
        number,
        name: "Local chosen conference".into(),
        description: "Local authority preserved".into(),
        posting_identity: Some(PostingIdentityPolicy::HandleAllowed),
        access_mode: ConferenceAccessMode::AtLeast,
        read_security: SecurityLevel::new(0).unwrap(),
        post_security: SecurityLevel::new(0).unwrap(),
        public_only: true,
        caller_deletion_enabled: true,
        maximum_lines: 99,
        privileged_security_levels: vec![],
    }
}
#[test]
fn real_macos_c7_six_node_catalog_lifecycle_generation_restore_and_events() {
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C7_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("c7"));
    assert!(!root.exists());
    fs::create_dir_all(&root).unwrap();
    let r = board_tree(&root.join("root"), "ROOT1", 10, Some(11), true);
    let h1 = board_tree(&root.join("host1"), "HOST1", 20, Some(21), true);
    let h2 = board_tree(&root.join("host2"), "HOST2", 30, Some(31), true);
    let mut e1 = board_tree(&root.join("end1"), "END1", 40, Some(41), true);
    let mut e2 = board_tree(&root.join("end2"), "END2", 50, Some(51), true);
    let e3 = board_tree(&root.join("end3"), "END3", 60, Some(61), true);
    for (a, b) in [(&r, &h1), (&r, &h2), (&h1, &e1), (&h1, &e2), (&h2, &e3)] {
        enroll(a, b);
        enroll(b, a);
    }
    let key = Signed::generate_key().unwrap();
    let a = Authority {
        network: network(),
        catalog_id: "a".repeat(32),
        publisher: envelope::NodeId::new("ROOT1").unwrap(),
        public_key: Signed::public_key(&key).unwrap(),
    };
    let authority_file = root.join("authority.json");
    fs::write(&authority_file, serde_json::to_vec(&a).unwrap()).unwrap();
    for b in [&r, &h1, &h2, &e1, &e2, &e3] {
        run(b, "catalog-pin", &[authority_file.to_str().unwrap()]);
    }
    let seed = publish(
        &r,
        &key,
        Body {
            format: "circuitnet-ng-catalog".into(),
            schema: 1,
            network: network(),
            catalog_id: a.catalog_id.clone(),
            revision: 1,
            previous_revision: 0,
            previous_hash: None,
            published_at: 1788825600,
            publisher: a.publisher.clone(),
            governance_reference: "bootstrap-synthetic-1".into(),
            rationale: "Empty synthetic starting catalog".into(),
            intent: Intent::Ordinary,
            entries: vec![],
        },
    );
    let seed_file = root.join("seed.json");
    fs::write(&seed_file, seed.encode().unwrap()).unwrap();
    for b in [&h1, &h2, &e1, &e2, &e3] {
        run(b, "catalog-import", &[seed_file.to_str().unwrap()]);
        assert_eq!(revision(b), 1);
    }
    let local_existing = db(&e2).create_conference(&new_native(&e2, 78)).unwrap();
    let counts: [usize; 6] =
        [&r, &h1, &h2, &e1, &e2, &e3].map(|b| db(b).all_conferences().unwrap().len());
    let rd = start(&r);
    let mut hd1 = start(&h1);
    let hd2 = start(&h2);
    let mut ed1 = start(&e1);
    let mut ed2 = start(&e2);
    let ed3 = start(&e3);
    use sf_core::events::{Action, Definition, ExchangePolicy, MissedPolicy, Schedule};
    let mut immediate = Definition {
        id: "catalog-immediate".into(),
        name: "Catalog catch-up".into(),
        enabled: true,
        action: Action::Circuitnet {
            network: network(),
            node: Some(envelope::NodeId::new("HOST1").unwrap()),
        },
        schedule: Schedule::Interval { seconds: 60 },
        timezone: "America/Phoenix".into(),
        policy: ExchangePolicy::Immediate,
        missed: MissedPolicy::RunOnce,
        minimum_spacing_seconds: 5,
    };
    let mut scheduled = immediate.clone();
    scheduled.id = "catalog-scheduled".into();
    scheduled.action = Action::Circuitnet {
        network: network(),
        node: Some(envelope::NodeId::new("ROOT1").unwrap()),
    };
    scheduled.policy = ExchangePolicy::Scheduled;
    scheduled.schedule = Schedule::Interval { seconds: 6 };
    save_event(&r, &immediate);
    save_event(&h2, &scheduled);
    let mut body = next(&seed);
    body.entries.push(Entry {
        access: sf_net::circuitnet::catalog::Access::Public,
        id: "1".repeat(32),
        codename: envelope::Codename::new("RETROCOM").unwrap(),
        display_name: "Retro Computing".into(),
        description: "Synthetic cross-branch conference".into(),
        category: "computing".into(),
        required: false,
        status: Lifecycle::Active,
        effective_revision: 2,
        retired_revision: None,
        historical_reference: "Synthetic new conference".into(),
        moderator_role: "interim-moderator".into(),
    });
    let added = publish(&r, &key, body);
    eventually("C7 immediate catalog push", || revision(&h1) == 2);
    eventually("C7 scheduled catalog fetch", || revision(&h2) == 2);
    immediate.enabled = false;
    scheduled.enabled = false;
    save_event(&r, &immediate);
    save_event(&h2, &scheduled);
    wait_idle(&r, &h1);
    wait_idle(&h2, &r);
    assert!(db(&r)
        .event_history("catalog-immediate")
        .unwrap()
        .iter()
        .any(|h| h.result.as_deref() == Some("succeeded")));
    assert!(db(&h2)
        .event_history("catalog-scheduled")
        .unwrap()
        .iter()
        .any(|h| h.result.as_deref() == Some("succeeded")));
    poll(&e1, &h1, false);
    poll(&e2, &h1, false);
    poll(&e3, &h2, false);
    for (i, b) in [&r, &h1, &h2, &e1, &e2, &e3].into_iter().enumerate() {
        assert_eq!(revision(b), 2);
        assert_eq!(db(b).all_conferences().unwrap().len(), counts[i]);
        let s = db(b)
            .circuitnet_catalog_current(&network())
            .unwrap()
            .unwrap();
        s.verify(&a).unwrap();
        assert_eq!(s, added);
        assert_eq!(
            db(b).circuitnet_catalog_entries(&network()).unwrap()[0].decision,
            "available"
        );
    }
    stop(ed1, &e1);
    run(&e1, "catalog-create-map", &[&"1".repeat(32), "77"]);
    e1.test = 77;
    ed1 = start(&e1);
    stop(ed2, &e2);
    run(&e2, "catalog-map", &[&"1".repeat(32), "78"]);
    e2.test = 78;
    ed2 = start(&e2);
    for b in [&r, &h1, &h2] {
        db(b)
            .circuitnet_catalog_create_map(
                "acceptance-operator",
                &network(),
                &"1".repeat(32),
                &new_native(b, 80),
                1788825700,
            )
            .unwrap();
    }
    db(&e3)
        .circuitnet_catalog_choose(
            "acceptance-operator",
            &network(),
            &"1".repeat(32),
            None,
            1788825700,
        )
        .unwrap();
    for (b, peer) in [
        (&e1, &h1),
        (&h1, &e1),
        (&h1, &e2),
        (&e2, &h1),
        (&h1, &r),
        (&r, &h1),
        (&r, &h2),
        (&h2, &r),
    ] {
        run(b, "live-subscribe", &[&peer.id, "RETROCOM", "0"]);
    }
    assert!(db(&e3)
        .circuitnet_status(&network())
        .unwrap()
        .dossiers
        .iter()
        .all(|d| d.codename.as_str() != "RETROCOM"));
    let historical = post(&e1, false, "C7 catalog identity", None);
    poll(&e1, &h1, false);
    poll(&e2, &h1, false);
    poll(&h1, &r, false);
    poll(&h2, &r, false);
    poll(&e3, &h2, false);
    assert_eq!(count(&e2, false), 1);
    assert_eq!(count(&e3, false), 0);
    assert_eq!(
        db(&e2)
            .all_conferences()
            .unwrap()
            .into_iter()
            .find(|c| c.number == 78)
            .unwrap()
            .id,
        local_existing.id
    );
    let before = db(&e2).circuitnet_catalog_entries(&network()).unwrap()[0].conference;
    let mut body = next(&added);
    body.entries[0].description = "Updated official description".into();
    body.entries[0].effective_revision = 3;
    let updated = publish(&r, &key, body);
    // A parent can restart with a newer catalog still awaiting downstream sync.
    poll(&h1, &r, false);
    stop(hd1, &h1);
    hd1 = start(&h1);
    poll(&e2, &h1, false);
    assert_eq!(revision(&e2), 3);
    assert_eq!(count(&e2, false), 1);
    assert_eq!(
        db(&e2).circuitnet_catalog_entries(&network()).unwrap()[0].conference,
        before
    );
    let mut body = next(&updated);
    body.entries[0].status = Lifecycle::Deprecated;
    body.entries[0].effective_revision = 4;
    let deprecated = publish(&r, &key, body);
    poll(&h1, &r, false);
    poll(&e2, &h1, false);
    assert_eq!(
        db(&e2).circuitnet_catalog_entries(&network()).unwrap()[0].decision,
        "needs-attention"
    );
    let mut body = next(&deprecated);
    body.entries[0].status = Lifecycle::Retired;
    body.entries[0].retired_revision = Some(5);
    body.entries[0].effective_revision = 5;
    let retired = publish(&r, &key, body);
    poll(&h1, &r, false);
    poll(&e1, &h1, false);
    poll(&e2, &h1, false);
    poll(&h2, &r, false);
    poll(&e3, &h2, false);
    assert!(!command(&e1, "live-subscribe", &["HOST1", "RETROCOM", "1"])
        .status
        .success());
    post(&e1, false, "Local archive after retirement", None);
    poll(&e1, &h1, false);
    poll(&e2, &h1, false);
    assert_eq!(count(&e2, false), 1);
    assert!(retired
        .changes(Some(&deprecated))
        .contains("RETIRED: RETROCOM"));
    for bad in [
        added.clone(),
        {
            let mut body = retired.body.clone();
            body.rationale = "Alternate same-revision fork".into();
            Signed::sign(body, &key).unwrap()
        },
        {
            let mut s = retired.clone();
            s.signature = "0".repeat(128);
            s
        },
        {
            let mut body = next(&retired);
            body.publisher = envelope::NodeId::new("HOST2").unwrap();
            Signed::sign(body, &key).unwrap()
        },
        {
            let mut body = next(&retired);
            body.previous_hash = Some("f".repeat(64));
            Signed::sign(body, &key).unwrap()
        },
    ] {
        assert!(db(&e2)
            .circuitnet_catalog_receive("synthetic-host", &network(), &bad, 1788825800)
            .is_err());
    }
    assert_eq!(revision(&e2), 5);
    assert_eq!(
        db(&e2)
            .circuitnet_catalog_status(&network())
            .unwrap()
            .rejected,
        5
    );
    // An authenticated HOST cannot smuggle a bad signed object through live TLS.
    {
        let mut invalid = Signed::sign(next(&retired), &key).unwrap();
        invalid.signature = "0".repeat(128);
        let mut stream = raw(&h1, &e2);
        let hello = Hello::new(
            network(),
            envelope::NodeId::new("HOST1").unwrap(),
            envelope::Role::Host,
            Mode::Poll,
        );
        wire::write(&mut stream, &Frame::Hello { hello }).unwrap();
        assert!(matches!(
            wire::read(&mut stream, wire::CONTROL_FRAME),
            Ok(Frame::Hello { .. })
        ));
        wire::write(
            &mut stream,
            &Frame::CatalogHead {
                revision: 6,
                hash: Some(invalid.hash.clone()),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut stream, wire::CONTROL_FRAME),
            Ok(Frame::CatalogRequest { enabled: true, .. })
        ));
        wire::write(
            &mut stream,
            &Frame::CatalogObject {
                catalog: Some(invalid),
            },
        )
        .unwrap();
        assert!(wire::read(&mut stream, wire::CONTROL_FRAME).is_err());
    }
    wait_idle(&e2, &h1);
    assert_eq!(revision(&e2), 5);
    assert_eq!(
        db(&e2)
            .circuitnet_catalog_status(&network())
            .unwrap()
            .rejected,
        6
    );
    // Same identity reactivation, followed by explicit different-identity reuse.
    let mut body = next(&retired);
    body.intent = Intent::Reactivate;
    body.entries[0].status = Lifecycle::Active;
    body.entries[0].retired_revision = None;
    body.entries[0].effective_revision = 6;
    let revived = publish(&r, &key, body);
    poll(&h1, &r, false);
    poll(&e1, &h1, false);
    assert_eq!(revision(&e1), 6);
    let mut body = next(&revived);
    body.entries[0].status = Lifecycle::Retired;
    body.entries[0].retired_revision = Some(7);
    body.entries[0].effective_revision = 7;
    let retired = publish(&r, &key, body);
    poll(&h1, &r, false);
    poll(&e1, &h1, false);
    let mut body = next(&retired);
    let mut new = added.body.entries[0].clone();
    new.id = "2".repeat(32);
    new.effective_revision = 8;
    body.entries.push(new);
    assert!(db(&r)
        .circuitnet_catalog_publish("acceptance-operator", body.clone(), &key, 1788825900)
        .is_err());
    body.intent = Intent::Reuse;
    let reused = publish(&r, &key, body);
    poll(&h1, &r, false);
    poll(&e1, &h1, false);
    poll(&e2, &h1, false);
    let identity: String = rusqlite::Connection::open(&e1.db)
        .unwrap()
        .query_row(
            "SELECT conference_identity FROM circuitnet_messages WHERE message_id=?1",
            [historical.id.get()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(identity, "1".repeat(32));
    assert_eq!(
        db(&e1)
            .circuitnet_catalog_entries(&network())
            .unwrap()
            .iter()
            .find(|e| e.entry.id == "2".repeat(32))
            .unwrap()
            .decision,
        "available"
    );
    stop(ed2, &e2);
    let backup = root.join("end2-backup");
    sf_bbs::backup_board(&e2.config, &backup).unwrap();
    let mut body = next(&reused);
    body.rationale = "Later known revision".into();
    let newer = publish(&r, &key, body);
    db(&e2)
        .circuitnet_catalog_receive("synthetic-host", &network(), &newer, 1788825901)
        .unwrap();
    assert!(sf_bbs::restore_board(&backup, e2.config.parent().unwrap(), true).is_err());
    assert_eq!(revision(&e2), 9);
    // A fresh restore retains the signed backup state; a live parent then advances it.
    let restored = root.join("restored-end2");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let cfg = RuntimeConfig::load(&restored.join(sf_bbs::BOARD_CONFIG_FILE)).unwrap();
    let paths = LogicalPaths::resolve(&restored, &cfg.validate().unwrap()).unwrap();
    let restored_db = RuntimeDatabase::open(paths.database()).unwrap();
    assert_eq!(
        restored_db
            .circuitnet_catalog_status(&network())
            .unwrap()
            .revision,
        8
    );
    restored_db.validate_catalog_authority().unwrap();
    stop(ed1, &e1);
    stop(ed3, &e3);
    stop(hd1, &h1);
    stop(hd2, &h2);
    stop(rd, &r);
    fs::write(root.join("acceptance.txt"),"C7 six independent boards: signed propagation; explicit local choices; message identity; lifecycle; replay/fork/signature/publisher/chain rejection; reactivation/reuse; restart; rollback-protected restore; graceful shutdown. Synthetic loopback only.\n").unwrap();
}

#[test]
fn real_macos_c71_human_creation_access_and_local_key_recovery() {
    fn actor(database: &RuntimeDatabase) -> MessageActor {
        let caller = database.caller_by_name(b"Sysop").unwrap().unwrap();
        MessageActor::new(caller.id, caller.security_level)
    }

    use sf_net::circuitnet::catalog::keys::{Change, Mode, Transition};
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let root = tempfile::tempdir().unwrap();
    let r = board_tree(&root.path().join("root"), "ROOT1", 10, None, true);
    let h = board_tree(&root.path().join("host"), "HOST1", 20, None, true);
    let mut e = board_tree(&root.path().join("end"), "END1", 30, None, true);
    for (a, b) in [(&r, &h), (&h, &r), (&h, &e), (&e, &h)] {
        enroll(a, b);
    }
    let key = Signed::generate_key().unwrap();
    let mut authority = Authority {
        network: network(),
        catalog_id: "e".repeat(32),
        publisher: envelope::NodeId::new("ROOT1").unwrap(),
        public_key: Signed::public_key(&key).unwrap(),
    };
    let authority_path = root.path().join("authority.json");
    fs::write(&authority_path, serde_json::to_vec(&authority).unwrap()).unwrap();
    for b in [&r, &h, &e] {
        run(b, "catalog-pin", &[authority_path.to_str().unwrap()]);
    }
    let seed = publish(
        &r,
        &key,
        Body {
            format: "circuitnet-ng-catalog".into(),
            schema: 1,
            network: network(),
            catalog_id: authority.catalog_id.clone(),
            revision: 1,
            previous_revision: 0,
            previous_hash: None,
            published_at: 1788825600,
            publisher: authority.publisher.clone(),
            governance_reference: "synthetic-founding".into(),
            rationale: "Disposable authority".into(),
            intent: Intent::Ordinary,
            entries: vec![],
        },
    );
    for b in [&h, &e] {
        db(b)
            .circuitnet_catalog_receive("local-operator", &network(), &seed, 1788825600)
            .unwrap();
    }
    let draft = root.path().join("draft.json");
    run(
        &r,
        "catalog-add",
        &[
            "BASKETS",
            "Underwater Basket Weaving",
            "Synthetic operator discussion",
            "sysops",
            "optional",
            draft.to_str().unwrap(),
            "synthetic-approval",
            "Approved synthetic test",
        ],
    );
    let body: Body = serde_json::from_slice(&fs::read(&draft).unwrap()).unwrap();
    assert_eq!(body.entries[0].display_name, "Underwater Basket Weaving");
    let identity = body.entries[0].id.clone();
    assert_eq!(identity.len(), 32);
    let current = publish(&r, &key, body);
    assert_eq!(current.body.schema, 2);
    let rd = start(&r);
    let hd = start(&h);
    let ed = start(&e);
    poll(&h, &r, false);
    poll(&e, &h, false);
    for b in [&r, &h, &e] {
        assert_eq!(revision(b), 2);
    }
    stop(ed, &e);
    stop(hd, &h);
    stop(rd, &r);
    for b in [&r, &h, &e] {
        let listing = run(b, "catalog-list", &[]);
        assert!(listing.contains("Underwater Basket Weaving"));
        assert!(!listing.contains(&identity));
        run(b, "catalog-create-map", &["BASKETS", "77"]);
        let mut database = db(b);
        let hash = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
        .hash(b"synthetic ordinary caller")
        .unwrap();
        let caller = database
            .create_caller(
                b"Ordinary",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let ordinary = MessageActor::new(caller.id, SecurityLevel::new(9999).unwrap());
        assert!(database.conference(ordinary, 77).is_err());
        assert!(database.conference(actor(&database), 77).is_ok());
    }
    for (b, n) in [(&r, "HOST1"), (&h, "ROOT1"), (&h, "END1"), (&e, "HOST1")] {
        run(b, "subscribe", &[n, "BASKETS"]);
    }
    e.test = 77;
    let mut database = db(&e);
    let operator = actor(&database);
    let conference = database.conference(operator, 77).unwrap();
    let preview = database
        .preview_posting_identity(operator, conference.id)
        .unwrap();
    let message = database
        .post(
            operator,
            NewMessage {
                identity_preview: Some(preview),
                conference_id: conference.id,
                recipient_caller_id: None,
                recipient_name: "All Callers".into(),
                subject: b"Restricted original generation".to_vec(),
                body: b"Synthetic operator test".to_vec(),
                created_at: 1788825700,
                parent_message_id: None,
                visibility: MessageVisibility::Public,
                kind: MessageKind::Standard,
            },
        )
        .unwrap();
    drop(database);
    let rd = start(&r);
    let hd = start(&h);
    let ed = start(&e);
    poll(&e, &h, false);
    poll(&h, &r, false);
    let database = db(&r);
    let operator = actor(&database);
    let conference = database.conference(operator, 77).unwrap();
    assert_eq!(
        database
            .messages(operator, conference.id)
            .unwrap()
            .iter()
            .filter(|m| m.subject == b"Restricted original generation")
            .count(),
        1
    );
    drop(database);
    assert_eq!(
        db(&r)
            .circuitnet_catalog_current(&network())
            .unwrap()
            .unwrap(),
        current
    );
    stop(ed, &e);
    stop(hd, &h);
    stop(rd, &r);
    let mut old_key = key;
    for (index, mode) in [Mode::Planned, Mode::Emergency].into_iter().enumerate() {
        let old = db(&r)
            .circuitnet_catalog_current(&network())
            .unwrap()
            .unwrap();
        let new = Signed::generate_key().unwrap();
        let mut replacement = authority.clone();
        replacement.public_key = Signed::public_key(&new).unwrap();
        let transition = Transition::sign(
            Change {
                format: "circuitnet-ng-catalog-key".into(),
                previous: authority.clone(),
                replacement: replacement.clone(),
                revision: old.body.revision,
                catalog_hash: old.hash.clone(),
                mode,
                published_at: 1788825800 + index as i64,
                reference: "synthetic-key-approval".into(),
                rationale: "Synthetic key transition".into(),
            },
            if mode == Mode::Planned {
                Some(old_key.as_slice())
            } else {
                None
            },
            &new,
        )
        .unwrap();
        let path = root.path().join(format!("transition-{index}.json"));
        fs::write(&path, serde_json::to_vec(&transition).unwrap()).unwrap();
        let fingerprint = replacement.fingerprint().unwrap();
        assert!(!command(
            &e,
            "catalog-replace-key",
            &[
                path.to_str().unwrap(),
                &"0".repeat(64),
                "confirm-key-replacement"
            ]
        )
        .status
        .success());
        assert!(!command(
            &e,
            "catalog-replace-key",
            &[path.to_str().unwrap(), &fingerprint]
        )
        .status
        .success());
        for b in [&r, &h, &e] {
            run(
                b,
                "catalog-replace-key",
                &[
                    path.to_str().unwrap(),
                    &fingerprint,
                    "confirm-key-replacement",
                ],
            );
            assert!(run(
                b,
                "catalog-replace-key",
                &[
                    path.to_str().unwrap(),
                    &fingerprint,
                    "confirm-key-replacement"
                ]
            )
            .contains("already applied"));
        }
        let draft_path = root.path().join(format!("edit-{index}.json"));
        run(
            &r,
            "catalog-edit",
            &[
                "BASKETS",
                "Underwater Basket Weaving",
                &format!("Updated after key transition {index}"),
                "sysops",
                "optional",
                draft_path.to_str().unwrap(),
                "synthetic-update",
                "Approved metadata edit",
            ],
        );
        let body: Body = serde_json::from_slice(&fs::read(&draft_path).unwrap()).unwrap();
        let signed = publish(&r, &new, body);
        let rd = start(&r);
        let hd = start(&h);
        let ed = start(&e);
        poll(&h, &r, false);
        poll(&e, &h, false);
        for b in [&r, &h, &e] {
            assert_eq!(revision(b), signed.body.revision);
            db(b)
                .circuitnet_catalog_current(&network())
                .unwrap()
                .unwrap()
                .verify(&replacement)
                .unwrap();
        }
        stop(ed, &e);
        stop(hd, &h);
        stop(rd, &r);
        authority = replacement;
        old_key = new;
    }
    // Real traffic remains attached to the generated original conference identity.
    let database = db(&e);
    let c = database.conference(actor(&database), 77).unwrap();
    assert_eq!(
        database
            .message(actor(&database), c.id, message.number)
            .unwrap()
            .id,
        message.id
    );
}
