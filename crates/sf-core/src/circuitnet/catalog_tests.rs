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
use crate::circuitnet::catalog::{Authority, Body, Entry, Intent, Lifecycle, Signed};
fn setup(b: &mut Board, key: &[u8]) -> Authority {
    let mut p = b.db.circuitnet_status(&net()).unwrap().profile;
    p.topology.nodes[0].role = Role::Root;
    b.db.circuitnet_configure("operator", &p, 1, 2).unwrap();
    let a = Authority {
        network: net(),
        catalog_id: "a".repeat(32),
        publisher: node("HOST"),
        public_key: Signed::public_key(key).unwrap(),
    };
    b.db.circuitnet_catalog_pin("operator", &a, 3).unwrap();
    a
}
fn initial(a: &Authority) -> Body {
    Body {
        format: "circuitnet-ng-catalog".into(),
        schema: 1,
        network: a.network.clone(),
        catalog_id: a.catalog_id.clone(),
        revision: 1,
        previous_revision: 0,
        previous_hash: None,
        published_at: 4,
        publisher: a.publisher.clone(),
        governance_reference: "bootstrap-test".into(),
        rationale: "Synthetic initial scope".into(),
        intent: Intent::Ordinary,
        entries: vec![Entry {
            access: sf_net::circuitnet::catalog::Access::Public,
            id: "1".repeat(32),
            codename: code("CNTEST"),
            display_name: "Synthetic catalog".into(),
            description: "Synthetic accepted scope".into(),
            category: "test".into(),
            required: true,
            status: Lifecycle::Active,
            effective_revision: 1,
            retired_revision: None,
            historical_reference: "synthetic".into(),
            moderator_role: "moderator".into(),
        }],
    }
}
fn next(s: &Signed) -> Body {
    let mut b = s.body.clone();
    b.revision += 1;
    b.previous_revision = s.body.revision;
    b.previous_hash = Some(s.hash.clone());
    b.published_at += 1;
    b
}
fn choose(b: &mut Board) {
    b.db.circuitnet_catalog_choose(
        "operator",
        &net(),
        &"1".repeat(32),
        Some(b.conference.get()),
        5,
    )
    .unwrap();
}
fn subscribe(b: &mut Board, peer: &str) {
    let version =
        b.db.circuitnet_status(&net())
            .unwrap()
            .dossiers
            .into_iter()
            .find(|d| d.neighbor == node(peer) && d.codename == code("CNTEST"))
            .unwrap()
            .version;
    b.db.circuitnet_subscribe(
        "operator",
        &net(),
        &Dossier {
            neighbor: node(peer),
            codename: code("CNTEST"),
            subscribed: true,
            version,
        },
        6,
    )
    .unwrap();
}
#[test]
fn catalog_pin_mapping_dossier_no_auto_create_or_subscribe() {
    let mut b = board("HOST");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut b, &key);
    let count = b.db.all_conferences().unwrap().len();
    let s =
        b.db.circuitnet_catalog_publish("operator", initial(&a), &key, 4)
            .unwrap();
    assert_eq!(b.db.all_conferences().unwrap().len(), count);
    assert_eq!(
        b.db.circuitnet_catalog_status(&net())
            .unwrap()
            .required_unmapped,
        1
    );
    assert!(
        !catalog::dossier_allowed(&b.db.connection, &net(), &node("END1"), &code("CNTEST"))
            .unwrap()
    );
    choose(&mut b);
    subscribe(&mut b, "END1");
    assert_eq!(
        b.db.circuitnet_catalog_status(&net())
            .unwrap()
            .required_unmapped,
        0
    );
    let m = post(&mut b, "Catalog bound post", None);
    assert_eq!(b.db.circuitnet_scan(&net(), 0, 10).unwrap().0, 1);
    let prepared =
        b.db.circuitnet_prepare_neighbor(&b.store, &net(), &node("END1"), 10)
            .unwrap();
    let batch = wire::Batch::decode(&prepared.bytes).unwrap();
    assert_eq!(
        batch.messages[0].conference_identity,
        Some(s.body.entries[0].id.clone())
    );
    assert!(matches!(
        b.db.circuitnet_prepare_catalog_neighbor(
            &b.store,
            &net(),
            &node("END1"),
            MessageCapabilities {
                catalog_access: true,
                directed: true,
                catalog: false
            },
            10
        ),
        Err(Error::Empty)
    ));
    let id =
        b.db.connection
            .query_row(
                "SELECT conference_identity FROM circuitnet_messages WHERE message_id=?1",
                [m.id.get()],
                |r| r.get::<_, String>(0),
            )
            .unwrap();
    assert_eq!(id, s.body.entries[0].id);
    b.db.circuitnet_catalog_choose("operator", &net(), &id, None, 11)
        .unwrap();
    assert_eq!(
        b.db.circuitnet_catalog_entries(&net()).unwrap()[0].decision,
        "ignored"
    );
    assert_eq!(b.db.all_conferences().unwrap().len(), count);
}
#[test]
fn catalog_retirement_reactivation_reuse_and_history_fences() {
    let mut b = board("HOST");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut b, &key);
    let s =
        b.db.circuitnet_catalog_publish("operator", initial(&a), &key, 4)
            .unwrap();
    choose(&mut b);
    subscribe(&mut b, "END1");
    let m = post(&mut b, "Historical generation", None);
    b.db.circuitnet_scan(&net(), 0, 10).unwrap();
    let mut body = next(&s);
    body.entries[0].status = Lifecycle::Retired;
    body.entries[0].retired_revision = Some(2);
    body.entries[0].effective_revision = 2;
    let retired =
        b.db.circuitnet_catalog_publish("operator", body, &key, 11)
            .unwrap();
    assert!(catalog::active(&b.db.connection, &net(), &code("CNTEST"), true).is_err());
    assert!(matches!(
        b.db.circuitnet_prepare_neighbor(&b.store, &net(), &node("END1"), 12),
        Err(Error::Empty)
    ));
    assert!(b
        .db
        .all_conferences()
        .unwrap()
        .iter()
        .any(|c| c.id == b.conference));
    post(&mut b, "Local archive during retirement", None);
    let mut body = next(&retired);
    body.intent = Intent::Reactivate;
    body.entries[0].status = Lifecycle::Active;
    body.entries[0].retired_revision = None;
    body.entries[0].effective_revision = 3;
    let revived =
        b.db.circuitnet_catalog_publish("operator", body, &key, 13)
            .unwrap();
    choose(&mut b);
    assert_eq!(b.db.circuitnet_scan(&net(), 0, 14).unwrap().0, 0);
    let mut body = next(&revived);
    body.entries[0].status = Lifecycle::Retired;
    body.entries[0].retired_revision = Some(4);
    body.entries[0].effective_revision = 4;
    let retired =
        b.db.circuitnet_catalog_publish("operator", body, &key, 15)
            .unwrap();
    let mut body = next(&retired);
    let mut fresh = s.body.entries[0].clone();
    fresh.id = "2".repeat(32);
    fresh.effective_revision = 5;
    body.entries.push(fresh);
    assert!(b
        .db
        .circuitnet_catalog_publish("operator", body.clone(), &key, 16)
        .is_err());
    body.intent = Intent::Reuse;
    b.db.circuitnet_catalog_publish("operator", body, &key, 16)
        .unwrap();
    assert!(!catalog::mapped(
        &b.db.connection,
        &net(),
        &code("CNTEST"),
        b.conference.get()
    )
    .unwrap());
    assert!(
        !catalog::dossier_allowed(&b.db.connection, &net(), &node("END1"), &code("CNTEST"))
            .unwrap()
    );
    let original: String =
        b.db.connection
            .query_row(
                "SELECT conference_identity FROM circuitnet_messages WHERE message_id=?1",
                [m.id.get()],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(original, "1".repeat(32));
    assert_eq!(
        b.db.circuitnet_catalog_current(&net())
            .unwrap()
            .unwrap()
            .body
            .entries
            .len(),
        2
    );
    assert_eq!(b.db.circuitnet_catalog_status(&net()).unwrap().rejected, 1);
}
#[test]
fn catalog_restart_backup_and_restore_cannot_discard_known_revision() {
    let mut b = board("HOST");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut b, &key);
    let s =
        b.db.circuitnet_catalog_publish("operator", initial(&a), &key, 4)
            .unwrap();
    choose(&mut b);
    let backup = b._temp.path().join("snapshot.sqlite3");
    b.db.backup_to(&backup).unwrap();
    let old = RuntimeDatabase::open(&backup).unwrap();
    old.validate_catalog_authority().unwrap();
    b.db.circuitnet_catalog_check_restore(&old).unwrap();
    let mut body = next(&s);
    body.entries[0].description = "Metadata updated".into();
    body.entries[0].effective_revision = 2;
    b.db.circuitnet_catalog_publish("operator", body, &key, 8)
        .unwrap();
    assert!(matches!(
        b.db.circuitnet_catalog_check_restore(&old),
        Err(Error::Catalog(sf_net::circuitnet::catalog::Error::Rollback))
    ));
    assert!(b
        .db
        .circuitnet_catalog_receive("HOST", &net(), &s, 9)
        .is_err());
    let reopened = RuntimeDatabase::open(&b._temp.path().join("native.sqlite3")).unwrap();
    assert_eq!(
        reopened.circuitnet_catalog_status(&net()).unwrap().revision,
        2
    );
    reopened.validate_catalog_authority().unwrap();
    assert_eq!(
        reopened.circuitnet_catalog_entries(&net()).unwrap()[0].conference,
        Some(b.conference.get())
    );
}
#[test]
fn catalog_incoming_generation_policy_and_create_map_are_explicit() {
    let key = Signed::generate_key().unwrap();
    let mut host = board("HOST");
    let a = setup(&mut host, &key);
    let mut end = board("END1");
    setup(&mut end, &key);
    let s = host
        .db
        .circuitnet_catalog_publish("operator", initial(&a), &key, 4)
        .unwrap();
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &s, 4)
        .unwrap();
    assert!(end
        .db
        .circuitnet_catalog_publish("operator", initial(&a), &key, 4)
        .is_err());
    choose(&mut host);
    subscribe(&mut host, "END1");
    // Ignore prevents END import even with old codename Dossier flags retained.
    end.db
        .circuitnet_catalog_choose("operator", &net(), &"1".repeat(32), None, 5)
        .unwrap();
    post(&mut host, "Scoped new identity", None);
    host.db.circuitnet_scan(&net(), 0, 10).unwrap();
    let prepared = host
        .db
        .circuitnet_prepare_neighbor(&host.store, &net(), &node("END1"), 11)
        .unwrap();
    assert!(end
        .db
        .circuitnet_import(&end.store, &net(), &node("HOST"), &prepared.bytes, 12)
        .is_err());
    choose(&mut end);
    subscribe(&mut end, "HOST");
    let mut wrong = wire::Batch::decode(&prepared.bytes).unwrap();
    wrong.messages[0].conference_identity = Some("f".repeat(32));
    assert!(end
        .db
        .circuitnet_import(
            &end.store,
            &net(),
            &node("HOST"),
            &wrong.encode().unwrap(),
            13
        )
        .is_err());
    end.db
        .circuitnet_import(&end.store, &net(), &node("HOST"), &prepared.bytes, 14)
        .unwrap();
    assert_eq!(
        end.db
            .circuitnet_import(&end.store, &net(), &node("HOST"), &prepared.bytes, 15)
            .unwrap()
            .duplicates,
        1
    );
    let mut body = next(&s);
    let mut entry = body.entries[0].clone();
    entry.id = "2".repeat(32);
    entry.codename = code("NEWAREA");
    entry.effective_revision = 2;
    body.entries.push(entry);
    let next = host
        .db
        .circuitnet_catalog_publish("operator", body, &key, 16)
        .unwrap();
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &next, 16)
        .unwrap();
    let definition = ConferenceDefinition {
        posting_identity: None,
        number: 42,
        name: "Locally selected".into(),
        description: "Local policy".into(),
        access_mode: ConferenceAccessMode::AtLeast,
        read_security: SecurityLevel::new(10).unwrap(),
        post_security: SecurityLevel::new(10).unwrap(),
        public_only: true,
        caller_deletion_enabled: false,
        maximum_lines: 99,
        privileged_security_levels: vec![],
    };
    end.db
        .circuitnet_catalog_create_map("operator", &net(), &"2".repeat(32), &definition, 17)
        .unwrap();
    assert!(end
        .db
        .all_conferences()
        .unwrap()
        .iter()
        .any(|c| c.number == 42));
    assert!(!end
        .db
        .circuitnet_status(&net())
        .unwrap()
        .dossiers
        .iter()
        .any(|d| d.codename == code("NEWAREA")));
}

#[test]
fn catalog_partial_catchup_remains_generic_event_work_and_head_forks_are_audited() {
    let key = Signed::generate_key().unwrap();
    let mut host = board("HOST");
    let a = setup(&mut host, &key);
    let mut end = board("END1");
    setup(&mut end, &key);
    let first = host
        .db
        .circuitnet_catalog_publish("operator", initial(&a), &key, 4)
        .unwrap();
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &first, 4)
        .unwrap();
    let second = host
        .db
        .circuitnet_catalog_publish("operator", next(&first), &key, 5)
        .unwrap();
    end.db
        .circuitnet_catalog_observe_parent(&net(), &node("HOST"), 2, Some(&second.hash), 6)
        .unwrap();
    assert!(
        end.db
            .circuitnet_catalog_status(&net())
            .unwrap()
            .pending_sync
    );
    assert!(end
        .db
        .event_has_outbound_work(
            &events::Action::Circuitnet {
                network: net(),
                node: Some(node("HOST"))
            },
            6
        )
        .unwrap());
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &second, 7)
        .unwrap();
    assert!(
        !end.db
            .circuitnet_catalog_status(&net())
            .unwrap()
            .pending_sync
    );
    assert!(!end
        .db
        .circuitnet_catalog_pending(&net(), Some(&node("HOST")))
        .unwrap());
    assert!(end
        .db
        .circuitnet_catalog_observe_parent(&net(), &node("HOST"), 2, Some(&"f".repeat(64)), 8)
        .is_err());
    assert_eq!(
        end.db.circuitnet_catalog_status(&net()).unwrap().rejected,
        1
    );
    assert_eq!(
        end.db.circuitnet_catalog_status(&net()).unwrap().revision,
        2
    );
}

#[test]
fn catalog_human_creation_generates_identity_and_reuse_preserves_old_traffic() {
    let mut b = board("HOST");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut b, &key);
    let mut body = initial(&a);
    body.entries.clear();
    let input = crate::circuitnet::catalog::ConferenceInput {
        codename: code("BASKETS"),
        display_name: "Underwater Basket Weaving".into(),
        description: "A synthetic discussion area".into(),
        category: "hobbies".into(),
        required: false,
        access: catalog::Access::Public,
    };
    body.add_conference(input.clone(), false).unwrap();
    let id = body.select("BASKETS").unwrap().id.clone();
    b.db.connection
        .execute(
            "DELETE FROM circuitnet_mappings WHERE network=?1",
            [net().as_str()],
        )
        .unwrap();
    assert_eq!(id.len(), 32);
    let first =
        b.db.circuitnet_catalog_publish("operator", body, &key, 4)
            .unwrap();
    b.db.circuitnet_catalog_choose("operator", &net(), &id, Some(b.conference.get()), 5)
        .unwrap();
    let m = post(&mut b, "Original generation", None);
    b.db.circuitnet_scan(&net(), 0, 11).unwrap();
    let mut body = next(&first);
    body.set_lifecycle("BASKETS", Lifecycle::Retired).unwrap();
    let retired =
        b.db.circuitnet_catalog_publish("operator", body, &key, 12)
            .unwrap();
    let mut body = next(&retired);
    body.set_lifecycle("BASKETS", Lifecycle::Active).unwrap();
    assert_eq!(body.select("BASKETS").unwrap().id, id);
    let resumed =
        b.db.circuitnet_catalog_publish("operator", body, &key, 13)
            .unwrap();
    let mut body = next(&resumed);
    body.set_lifecycle("BASKETS", Lifecycle::Retired).unwrap();
    let retired =
        b.db.circuitnet_catalog_publish("operator", body, &key, 14)
            .unwrap();
    let mut body = next(&retired);
    assert!(body.add_conference(input.clone(), false).is_err());
    body.add_conference(input, true).unwrap();
    assert_ne!(body.select("BASKETS").unwrap().id, id);
    b.db.circuitnet_catalog_publish("operator", body, &key, 15)
        .unwrap();
    let historical: String =
        b.db.connection
            .query_row(
                "SELECT conference_identity FROM circuitnet_messages WHERE message_id=?1",
                [m.id.get()],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(historical, id);
}

#[test]
fn catalog_sysop_areas_do_not_grant_callers_or_old_peers_access() {
    let mut host = board("HOST");
    let mut end = board("END1");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut host, &key);
    setup(&mut end, &key);
    let mut body = initial(&a);
    body.schema = 2;
    body.entries[0].access = catalog::Access::Sysops;
    let s = host
        .db
        .circuitnet_catalog_publish("operator", body, &key, 4)
        .unwrap();
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &s, 4)
        .unwrap();
    for b in [&mut host, &mut end] {
        assert!(b
            .db
            .circuitnet_catalog_choose(
                "operator",
                &net(),
                &s.body.entries[0].id,
                Some(b.conference.get()),
                5
            )
            .is_err());
        b.db.connection.execute("UPDATE message_conferences SET read_security=9999,post_security=9999 WHERE conference_id=?1",[b.conference.get()]).unwrap();
        choose(b);
    }
    subscribe(&mut host, "END1");
    subscribe(&mut end, "HOST");
    post(&mut host, "Operator conference", None);
    host.db.circuitnet_scan(&net(), 0, 11).unwrap();
    assert!(matches!(
        host.db.circuitnet_prepare_catalog_neighbor(
            &host.store,
            &net(),
            &node("END1"),
            MessageCapabilities {
                directed: true,
                catalog: true,
                catalog_access: false
            },
            12
        ),
        Err(Error::Empty)
    ));
    let prepared = host
        .db
        .circuitnet_prepare_neighbor(&host.store, &net(), &node("END1"), 12)
        .unwrap();
    end.db
        .circuitnet_import(&end.store, &net(), &node("HOST"), &prepared.bytes, 13)
        .unwrap();
    let hash = CredentialHasher::new(&PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    })
    .unwrap()
    .hash(b"synthetic password")
    .unwrap();
    let ordinary = end
        .db
        .create_caller(
            b"Ordinary",
            &hash,
            SecurityLevel::new(10).unwrap(),
            CallerState::Active,
            false,
            1,
        )
        .unwrap();
    let visitor = end
        .db
        .create_caller(
            b"Verified visitor",
            &hash,
            SecurityLevel::new(40).unwrap(),
            CallerState::Active,
            false,
            1,
        )
        .unwrap();
    let actor = MessageActor::new(ordinary.id, SecurityLevel::new(100).unwrap());
    assert!(!end
        .db
        .conferences(actor)
        .unwrap()
        .iter()
        .any(|c| c.id == end.conference));
    assert!(end.db.conference(actor, 17).is_err());
    assert!(end.db.messages(actor, end.conference).is_err());
    assert!(end.db.replace_queue(actor, &[17]).is_err());
    let visiting = MessageActor::new(visitor.id, SecurityLevel::new(100).unwrap());
    assert!(end.db.conference(visiting, 17).is_err());
    end.db
        .circuitnet_catalog_access_levels(
            "operator",
            &net(),
            "CNTEST",
            &[SecurityLevel::new(40).unwrap()],
            14,
        )
        .unwrap();
    assert!(end.db.conference(visiting, 17).is_ok());
    assert!(end.db.conference(actor, 17).is_err());
    assert!(end
        .db
        .circuitnet_catalog_access_levels(
            "operator",
            &net(),
            "CNTEST",
            &[
                SecurityLevel::new(40).unwrap(),
                SecurityLevel::new(40).unwrap()
            ],
            15
        )
        .is_err());
    assert!(end.db.conference(visiting, 17).is_ok());
    end.db
        .circuitnet_catalog_access_levels("operator", &net(), "CNTEST", &[], 16)
        .unwrap();
    assert!(end.db.conference(visiting, 17).is_err());
    assert!(end.db.conference(actor, 17).is_err());
    let audits: i64 = end
        .db
        .connection
        .query_row(
            "SELECT COUNT(*) FROM circuitnet_changes WHERE operation='catalog-local-access-levels'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audits, 2);
    end.db
        .connection
        .execute(
            "UPDATE message_conferences SET read_security=10 WHERE conference_id=?1",
            [end.conference.get()],
        )
        .unwrap();
    assert!(!catalog::mapped(
        &end.db.connection,
        &net(),
        &code("CNTEST"),
        end.conference.get()
    )
    .unwrap());
}

#[test]
fn catalog_key_rotation_emergency_recovery_restart_and_backup_preserve_epochs() {
    use sf_net::circuitnet::catalog::keys::{Change, Mode, Transition};
    let mut b = board("HOST");
    let old = Signed::generate_key().unwrap();
    let a = setup(&mut b, &old);
    let first =
        b.db.circuitnet_catalog_publish("operator", initial(&a), &old, 4)
            .unwrap();
    let before = b._temp.path().join("before-key.sqlite3");
    b.db.backup_to(&before).unwrap();
    let mut previous = a;
    let mut checkpoint = first;
    let mut oldkey = old;
    for mode in [Mode::Planned, Mode::Emergency] {
        let new = Signed::generate_key().unwrap();
        let mut replacement = previous.clone();
        replacement.public_key = Signed::public_key(&new).unwrap();
        let t = Transition::sign(
            Change {
                format: "circuitnet-ng-catalog-key".into(),
                previous: previous.clone(),
                replacement: replacement.clone(),
                revision: checkpoint.body.revision,
                catalog_hash: checkpoint.hash.clone(),
                mode,
                published_at: 20,
                reference: "synthetic reviewed replacement".into(),
                rationale: "Synthetic recovery test".into(),
            },
            if mode == Mode::Planned {
                Some(&oldkey)
            } else {
                None
            },
            &new,
        )
        .unwrap();
        assert!(b
            .db
            .circuitnet_catalog_replace_key("operator", &net(), &t, &"0".repeat(64), 21)
            .is_err());
        let mut wrong = t.clone();
        wrong.new_signature = "0".repeat(128);
        assert!(b
            .db
            .circuitnet_catalog_replace_key(
                "operator",
                &net(),
                &wrong,
                &replacement.fingerprint().unwrap(),
                21
            )
            .is_err());
        let fingerprint = replacement.fingerprint().unwrap();
        assert!(b
            .db
            .circuitnet_catalog_replace_key("operator", &net(), &t, &fingerprint, 22)
            .unwrap());
        assert!(!b
            .db
            .circuitnet_catalog_replace_key("operator", &net(), &t, &fingerprint, 22)
            .unwrap());
        b.db.validate_catalog_authority().unwrap();
        assert!(b
            .db
            .circuitnet_catalog_publish("operator", next(&checkpoint), &oldkey, 23)
            .is_err());
        checkpoint =
            b.db.circuitnet_catalog_publish("operator", next(&checkpoint), &new, 24)
                .unwrap();
        b.db.validate_catalog_authority().unwrap();
        previous = replacement;
        oldkey = new;
    }
    let reopened = RuntimeDatabase::open(&b._temp.path().join("native.sqlite3")).unwrap();
    reopened.validate_catalog_authority().unwrap();
    let saved = b._temp.path().join("after-key.sqlite3");
    b.db.backup_to(&saved).unwrap();
    let restored = RuntimeDatabase::open(&saved).unwrap();
    restored.validate_catalog_authority().unwrap();
    b.db.circuitnet_catalog_check_restore(&restored).unwrap();
    assert!(b
        .db
        .circuitnet_catalog_check_restore(&RuntimeDatabase::open(&before).unwrap())
        .is_err());
    assert_eq!(
        b.db.circuitnet_catalog_status(&net())
            .unwrap()
            .authority
            .unwrap(),
        previous
    );
    assert!(b
        .db
        .connection
        .execute("DELETE FROM circuitnet_catalog_keys", [])
        .is_err());
}

#[test]
fn catalog_schema_one_peer_skips_new_public_generations_without_blocking_old_work() {
    let mut host = board("HOST");
    let mut end = board("END1");
    let key = Signed::generate_key().unwrap();
    let a = setup(&mut host, &key);
    setup(&mut end, &key);
    let first = host
        .db
        .circuitnet_catalog_publish("operator", initial(&a), &key, 4)
        .unwrap();
    end.db
        .circuitnet_catalog_receive("HOST", &net(), &first, 4)
        .unwrap();
    choose(&mut host);
    choose(&mut end);
    subscribe(&mut host, "END1");
    subscribe(&mut end, "HOST");
    let original = host.conference;
    let mut body = next(&first);
    body.add_conference(
        catalog::ConferenceInput {
            codename: code("NEWAREA"),
            display_name: "New area".into(),
            description: "New public scope".into(),
            category: "test".into(),
            required: false,
            access: catalog::Access::Public,
        },
        false,
    )
    .unwrap();
    let new_id = body.select("NEWAREA").unwrap().id.clone();
    host.db
        .circuitnet_catalog_publish("operator", body, &key, 5)
        .unwrap();
    let definition = ConferenceDefinition {
        posting_identity: None,
        number: 42,
        name: "New local".into(),
        description: "Synthetic".into(),
        access_mode: ConferenceAccessMode::AtLeast,
        read_security: SecurityLevel::new(10).unwrap(),
        post_security: SecurityLevel::new(10).unwrap(),
        public_only: true,
        caller_deletion_enabled: false,
        maximum_lines: 99,
        privileged_security_levels: vec![],
    };
    host.db
        .circuitnet_catalog_create_map("operator", &net(), &new_id, &definition, 6)
        .unwrap();
    host.conference = host.db.conference(host.actor, 42).unwrap().id;
    host.db
        .circuitnet_subscribe(
            "operator",
            &net(),
            &Dossier {
                neighbor: node("END1"),
                codename: code("NEWAREA"),
                subscribed: true,
                version: 0,
            },
            7,
        )
        .unwrap();
    post(&mut host, "New identity waits", None);
    host.db.circuitnet_scan(&net(), 0, 10).unwrap();
    // Freeze an offer to a capable peer, then retry against the older capability.
    host.db
        .circuitnet_prepare_neighbor(&host.store, &net(), &node("END1"), 11)
        .unwrap();
    host.conference = original;
    post(&mut host, "Known identity still moves", None);
    host.db.circuitnet_scan(&net(), 0, 12).unwrap();
    let prepared = host
        .db
        .circuitnet_prepare_catalog_neighbor(
            &host.store,
            &net(),
            &node("END1"),
            MessageCapabilities {
                directed: true,
                catalog: true,
                catalog_access: false,
            },
            13,
        )
        .unwrap();
    let batch = Batch::decode(&prepared.bytes).unwrap();
    assert_eq!(batch.messages.len(), 1);
    assert_eq!(batch.messages[0].codename, code("CNTEST"));
    assert_eq!(
        end.db
            .circuitnet_import(&end.store, &net(), &node("HOST"), &prepared.bytes, 14)
            .unwrap()
            .imported,
        1
    );
}

#[test]
fn catalog_key_lost_before_first_publication_can_be_replaced_at_same_checkpoint() {
    use sf_net::circuitnet::catalog::keys::{Change, Mode, Transition};
    let mut b = board("HOST");
    let old = Signed::generate_key().unwrap();
    let a = setup(&mut b, &old);
    let first =
        b.db.circuitnet_catalog_publish("operator", initial(&a), &old, 4)
            .unwrap();
    let lost = Signed::generate_key().unwrap();
    let replacement = Signed::generate_key().unwrap();
    let mut previous = a;
    for key in [&lost, &replacement] {
        let mut authority = previous.clone();
        authority.public_key = Signed::public_key(key).unwrap();
        let transition = Transition::sign(
            Change {
                format: "circuitnet-ng-catalog-key".into(),
                previous: previous.clone(),
                replacement: authority.clone(),
                revision: 1,
                catalog_hash: first.hash.clone(),
                mode: Mode::Emergency,
                published_at: 5,
                reference: "synthetic-emergency".into(),
                rationale: "Replace unavailable key without requiring it to publish".into(),
            },
            None,
            key,
        )
        .unwrap();
        b.db.circuitnet_catalog_replace_key(
            "local-operator",
            &net(),
            &transition,
            &authority.fingerprint().unwrap(),
            5,
        )
        .unwrap();
        previous = authority;
    }
    b.db.validate_catalog_authority().unwrap();
    let signed =
        b.db.circuitnet_catalog_publish("operator", next(&first), &replacement, 6)
            .unwrap();
    assert_eq!(signed.body.revision, 2);
    signed.verify(&previous).unwrap();
    b.db.validate_catalog_authority().unwrap();
    let backup = b._temp.path().join("same-checkpoint.sqlite3");
    b.db.backup_to(&backup).unwrap();
    RuntimeDatabase::open(&backup)
        .unwrap()
        .validate_catalog_authority()
        .unwrap();
}
