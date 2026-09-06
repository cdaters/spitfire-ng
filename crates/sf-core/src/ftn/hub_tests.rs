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
fn hub() -> Fixture {
    let mut f = fixture();
    for (id, address) in [
        ("downb", "10:100/5@synthetic"),
        ("point1", "10:100/1.1@synthetic"),
    ] {
        let mut l = f.policy.links[1].clone();
        l.id = id.into();
        l.remote = address.parse().unwrap();
        f.policy.links.push(l);
    }
    for link in ["next", "downb", "point1"] {
        f.db.configure_ftn_downstream(
            &f.policy,
            "operator",
            &Downstream {
                link: link.into(),
                enabled: true,
                held: false,
                boss_aka: (link == "point1").then(|| "node".into()),
                areafix: true,
                rescan: true,
                max_area: 5,
                max_total: 8,
                cooldown: 60,
                version: 1,
            },
            0,
            NOW,
        )
        .unwrap();
    }
    for area in ["TEST1", "TEST2"] {
        f.db.configure_ftn_area_access(
            &f.policy,
            "operator",
            &AreaAccess {
                domain: "synthetic".parse().unwrap(),
                area: area.into(),
                remote_subscribe: true,
                rescan: true,
                version: 1,
            },
            0,
            NOW,
        )
        .unwrap();
        subscribe(&mut f, "downb", area, true, 0);
    }
    subscribe(&mut f, "point1", "TEST1", true, 0);
    subscribe(&mut f, "next", "TEST2", false, 1);
    f
}
fn subscribe(f: &mut Fixture, link: &str, area: &str, state: bool, expected: i64) {
    f.db.configure_ftn_subscription(
        &f.policy,
        "operator",
        &Subscription {
            link: link.into(),
            domain: "synthetic".parse().unwrap(),
            area: area.into(),
            subscribed: state,
            source: SubscriptionSource::Manual,
            version: expected + 1,
            changed_at: NOW,
        },
        expected,
        NOW,
    )
    .unwrap();
}
fn toss(f: &mut Fixture, seq: u32, area: &str) -> TossResult {
    f.db.toss_ftn(
        &f.store,
        &f.policy,
        "peer",
        &inbound(seq, "10:100/1", Some(area)),
        NOW,
    )
    .unwrap()
}
fn transport(f: &Fixture) -> BinkpPolicy {
    let mut p = binkp_policy();
    let template = p.links[0].clone();
    p.links.clear();
    for l in &f.policy.links {
        let mut t = template.clone();
        t.link = l.id.clone();
        t.akas = vec![l.aka.clone()];
        p.links.push(t);
    }
    p
}
fn accept_link(f: &mut Fixture, link: &str, now: i64) -> Vec<String> {
    let p = transport(f);
    let session =
        f.db.begin_binkp(&f.policy, &p, link, "hub-test", BinkpMode::Poll, now)
            .unwrap();
    let work =
        f.db.claim_binkp(&f.policy, &p, &f.store, &session, now)
            .unwrap();
    for w in &work {
        f.db.offered_binkp(&session, &w.queue).unwrap();
        f.db.accepted_binkp(&session, &w.queue, now).unwrap();
    }
    f.db.finish_binkp(&session, None, now).unwrap();
    work.into_iter().map(|w| w.queue).collect()
}
fn request(
    f: &Fixture,
    link: &str,
    seq: u32,
    body: &str,
    secret: &str,
    source: Option<Address>,
) -> Vec<u8> {
    let l = f.policy.link(link).unwrap();
    let from = source.unwrap_or(l.remote.address);
    let dest = f.policy.aka(&l.aka).unwrap().endpoint.address;
    let mut p = Packet::decode(&inbound(seq, &dest.to_string(), None), (10, 10)).unwrap();
    p.header.origin = l.remote.address;
    p.header.destination = dest;
    let m = &mut p.messages[0];
    m.origin = from.two_d();
    m.to = b"AreaFix".to_vec();
    m.subject = secret.as_bytes().to_vec();
    m.text = format!(
        "\x01MSGID: {from} {seq:08x}\r\x01INTL {} {}\r{}{}\x01CHRS: UTF-8 4\r{body}\r",
        dest.boss(),
        from.boss(),
        if from.point() > 0 {
            format!("\x01FMPT {}\r", from.point())
        } else {
            String::new()
        },
        if dest.point() > 0 {
            format!("\x01TOPT {}\r", dest.point())
        } else {
            String::new()
        }
    )
    .into_bytes();
    p.encode().unwrap()
}
fn areafix(
    f: &mut Fixture,
    link: &str,
    seq: u32,
    body: &str,
    secret: &str,
    source: Option<Address>,
    authenticated: bool,
) -> TossResult {
    let bytes = request(f, link, seq, body, secret, source);
    let p = transport(f);
    let session =
        f.db.begin_binkp(&f.policy, &p, link, "hub-test", BinkpMode::Poll, NOW)
            .unwrap();
    if authenticated {
        f.db.observe_binkp(
            &session,
            &[f.policy.link(link).unwrap().remote.clone()],
            &[],
            0,
            NOW,
        )
        .unwrap();
    }
    let result =
        f.db.receive_binkp_with_areafix(
            &f.store,
            &f.policy,
            &p,
            &session,
            &bytes,
            NOW,
            Some(&|_, s| s == "synthetic-area-secret"),
        )
        .unwrap();
    f.db.finish_binkp(&session, None, NOW).unwrap();
    result
}
#[test]
fn subscriptions_fanout_filter_replay_point_and_no_reflection() {
    let mut f = hub();
    assert_eq!(toss(&mut f, 100, "TEST1").imported, 1);
    let links: Vec<_> =
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .map(|q| q.link)
            .collect();
    assert_eq!(links.len(), 3);
    for l in ["next", "downb", "point1"] {
        assert!(links.contains(&l.into()))
    }
    assert_eq!(toss(&mut f, 100, "TEST1").duplicates, 1);
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), 3);
    assert_eq!(toss(&mut f, 101, "TEST2").imported, 1);
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), 4);
    for link in ["next", "point1"] {
        let l = f.policy.link(link).unwrap();
        let mut p = Packet::decode(
            &inbound(
                if link == "next" { 102 } else { 103 },
                "10:100/1",
                Some("TEST1"),
            ),
            (10, 10),
        )
        .unwrap();
        p.header.origin = l.remote.address;
        p.header.destination = "10:100/1".parse().unwrap();
        let source = l.remote.address;
        let m = &mut p.messages[0];
        m.origin = source.two_d();
        m.text=format!("AREA:TEST1\r\x01MSGID: {source} {:08x}\r\x01CHRS: UTF-8 4\rDownstream post\r--- Synthetic\r * Origin: Synthetic ({source})\r",if link=="next"{102}else{103}).into_bytes();
        let bytes = p.encode().unwrap();
        assert_eq!(
            f.db.toss_ftn(&f.store, &f.policy, link, &bytes, NOW)
                .unwrap()
                .imported,
            1
        );
        let pid: String =
            f.db.connection
                .query_row(
                    "SELECT publication_id FROM ftn_messages WHERE msgid=?1",
                    [format!(
                        "{source} {:08x}",
                        if link == "next" { 102 } else { 103 }
                    )],
                    |r| r.get(0),
                )
                .unwrap();
        let targets: Vec<String> =
            f.db.connection
                .prepare("SELECT link_id FROM ftn_routing_decisions WHERE publication_id=?1")
                .unwrap()
                .query_map([&pid], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
        assert_eq!(targets.len(), 3);
        assert!(!targets.contains(&link.into()));
        assert!(targets.contains(&"peer".into()));
        let rows = f.db.ftn_queue(None).unwrap();
        for q in rows.into_iter().filter(|q| q.state == "pending") {
            let a =
                f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
                    .unwrap();
            let p = Packet::decode(&f.store.0.lock().unwrap()[&a], (10, 10)).unwrap();
            let t = Text::parse(&p.messages[0].text, Charset::Utf8).unwrap();
            if q.aka == "node" {
                assert!(t.path.contains(&(100, 1)));
            }
        }
    }
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
}
#[test]
fn areafix_authentication_atomicity_replay_and_manual_cas() {
    let mut f = hub();
    let initial = f.db.ftn_subscriptions(0).unwrap();
    for (seq, secret, source, authenticated, body) in [
        (200, "wrong", None, true, "+TEST2"),
        (
            201,
            "synthetic-area-secret",
            Some("10:100/5".parse().unwrap()),
            true,
            "+TEST2",
        ),
        (202, "synthetic-area-secret", None, false, "+TEST2"),
        (203, "synthetic-area-secret", None, true, "+TEST2\r+UNKNOWN"),
        (
            204,
            "synthetic-area-secret",
            None,
            true,
            "+TEST2\r$(touch /tmp/forbidden)",
        ),
        (
            205,
            "synthetic-area-secret",
            None,
            true,
            "+TEST2\r%QUERY 10:100/5",
        ),
    ] {
        areafix(&mut f, "next", seq, body, secret, source, authenticated);
        assert_eq!(f.db.ftn_subscriptions(0).unwrap(), initial);
    }
    assert_eq!(
        areafix(
            &mut f,
            "next",
            206,
            "%LIST\r%HELP\r+TEST2",
            "synthetic-area-secret",
            None,
            true
        )
        .imported,
        1
    );
    let updated = f.db.ftn_subscriptions(0).unwrap();
    assert!(updated.iter().any(|s| s.link == "next"
        && s.area == "TEST2"
        && s.subscribed
        && s.source == SubscriptionSource::Areafix));
    let count = f.db.ftn_queue(None).unwrap().len();
    assert_eq!(
        areafix(
            &mut f,
            "next",
            206,
            "%LIST\r%HELP\r+TEST2",
            "synthetic-area-secret",
            None,
            true
        )
        .duplicates,
        1
    );
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), count);
    let mut stale = initial
        .into_iter()
        .find(|s| s.link == "next" && s.area == "TEST2")
        .unwrap();
    let expected = stale.version;
    stale.version += 1;
    assert!(matches!(
        f.db.configure_ftn_subscription(&f.policy, "operator", &stale, expected, NOW),
        Err(Error::Conflict)
    ));
    assert_eq!(f.db.ftn_subscriptions(0).unwrap(), updated);
    let safe = serde_json::to_string(&f.db.ftn_areafix_activity(0).unwrap()).unwrap();
    assert!(!safe.contains("secret"));
    assert!(!safe.contains("touch"));
    let leaked:u32=f.db.connection.query_row("SELECT COUNT(*) FROM message_payloads WHERE CAST(subject AS TEXT) LIKE '%secret%' OR CAST(body AS TEXT) LIKE '%secret%'",[],|r|r.get(0)).unwrap();
    assert_eq!(leaked, 0);
    let b = request(&f, "point1", 207, "-TEST1", "synthetic-area-secret", None);
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "point1", &b, NOW)
            .unwrap()
            .imported,
        1
    );
    assert!(f
        .db
        .ftn_subscriptions(0)
        .unwrap()
        .iter()
        .any(|s| s.link == "point1" && s.subscribed));
    areafix(
        &mut f,
        "point1",
        208,
        "-TEST1",
        "synthetic-area-secret",
        None,
        true,
    );
    assert!(
        !f.db
            .ftn_subscriptions(0)
            .unwrap()
            .iter()
            .find(|s| s.link == "point1")
            .unwrap()
            .subscribed
    );
    assert!(f
        .db
        .ftn_subscriptions(0)
        .unwrap()
        .iter()
        .any(|s| s.link == "downb" && s.subscribed));
}
#[test]
fn rescan_bounds_identity_privacy_replay_and_partial_fanout_recovery() {
    let mut f = hub();
    for seq in 300..307 {
        assert_eq!(toss(&mut f, seq, "TEST1").imported, 1)
    }
    let a = accept_link(&mut f, "next", NOW);
    let point = accept_link(&mut f, "point1", NOW);
    assert_eq!(a.len(), 7);
    assert_eq!(point.len(), 7);
    // Native snapshot taken with A/point accepted and B pending.
    for q in
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .filter(|q| q.link == "downb")
    {
        f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
            .unwrap();
    }
    let snapshot = f.temp.path().join("hub-snapshot.db");
    f.db.connection
        .execute("VACUUM INTO ?1", [snapshot.to_str().unwrap()])
        .unwrap();
    let before = f.db.ftn_subscriptions(0).unwrap();
    let accepted_b = accept_link(&mut f, "downb", NOW);
    assert_eq!(accepted_b.len(), 7);
    let evidence = f.db.ftn_recovery_evidence().unwrap();
    let mut restored = RuntimeDatabase::open(&snapshot).unwrap();
    restored.hold_restored_ftn().unwrap();
    restored.recover_binkp(NOW).unwrap();
    let result = restored
        .reconcile_ftn_recovery(&evidence, "operator", NOW)
        .unwrap();
    assert_eq!(result.accepted, 7);
    assert_eq!(restored.ftn_subscriptions(0).unwrap(), before);
    assert_eq!(
        restored.ftn_downstreams().unwrap(),
        f.db.ftn_downstreams().unwrap()
    );
    assert!(restored
        .ftn_queue(None)
        .unwrap()
        .iter()
        .all(|q| q.state == "accepted"));
    assert_eq!(
        areafix(
            &mut f,
            "next",
            310,
            "%RESCAN TEST1 R=6",
            "synthetic-area-secret",
            None,
            true
        )
        .imported,
        1
    );
    assert!(f.db.ftn_rescan_activity(0).unwrap().is_empty());
    areafix(
        &mut f,
        "next",
        311,
        "%RESCAN TEST1 R=5",
        "synthetic-area-secret",
        None,
        true,
    );
    let activity = f.db.ftn_rescan_activity(0).unwrap();
    assert_eq!(activity.len(), 1);
    assert_eq!(activity[0].queued, 5);
    let count = f.db.ftn_queue(None).unwrap().len();
    areafix(
        &mut f,
        "next",
        311,
        "%RESCAN TEST1 R=5",
        "synthetic-area-secret",
        None,
        true,
    );
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), count);
    areafix(
        &mut f,
        "next",
        312,
        "%RESCAN TEST1 R=5",
        "synthetic-area-secret",
        None,
        true,
    );
    assert_eq!(f.db.ftn_rescan_activity(0).unwrap(), activity);
    assert_eq!(toss(&mut f, 300, "TEST1").duplicates, 1);
    let rescan_ids: Vec<String> =
        f.db.connection
            .prepare("SELECT queue_id FROM ftn_routing_decisions WHERE reason='authorized-rescan'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
    for q in
        f.db.ftn_queue(None)
            .unwrap()
            .into_iter()
            .filter(|q| rescan_ids.contains(&q.id))
    {
        assert_eq!(q.link, "next");
        let artifact =
            f.db.build_ftn(&f.store, &f.policy, &q.id, q.version, NOW)
                .unwrap();
        let bytes = &f.store.0.lock().unwrap()[&artifact];
        let p = Packet::decode(bytes, (10, 10)).unwrap();
        let t = Text::parse(&p.messages[0].text, Charset::Utf8).unwrap();
        assert_eq!(t.area.as_deref(), Some("TEST1"));
        assert!(t.msgid.as_deref().unwrap().starts_with("10:100/2.9 "));
    }
    assert_eq!(
        f.db.connection
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE container_kind='conference'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        7
    );
}
#[test]
fn area_access_disable_point_validation_and_subscription_hold() {
    let mut f = hub();
    toss(&mut f, 400, "TEST1");
    subscribe(&mut f, "next", "TEST1", false, 1);
    assert!(f
        .db
        .ftn_queue(None)
        .unwrap()
        .iter()
        .any(|q| q.link == "next" && q.state == "held"));
    f.db.configure_ftn_area_access(
        &f.policy,
        "operator",
        &AreaAccess {
            domain: "synthetic".parse().unwrap(),
            area: "TEST2".into(),
            remote_subscribe: false,
            rescan: false,
            version: 2,
        },
        1,
        NOW,
    )
    .unwrap();
    areafix(
        &mut f,
        "next",
        401,
        "+TEST1\r+TEST2",
        "synthetic-area-secret",
        None,
        true,
    );
    assert!(
        !f.db
            .ftn_subscriptions(0)
            .unwrap()
            .iter()
            .find(|s| s.link == "next" && s.area == "TEST1")
            .unwrap()
            .subscribed
    );
    let mut d =
        f.db.ftn_downstreams()
            .unwrap()
            .into_iter()
            .find(|d| d.link == "point1")
            .unwrap();
    d.boss_aka = Some("point".into());
    d.version += 1;
    assert!(f
        .db
        .configure_ftn_downstream(&f.policy, "operator", &d, 1, NOW)
        .is_err());
    let mut removed = f.policy.clone();
    removed.links.retain(|l| l.id != "downb");
    assert!(f.db.validate_ftn_policy_references(&removed).is_err());
    let mut d =
        f.db.ftn_downstreams()
            .unwrap()
            .into_iter()
            .find(|d| d.link == "downb")
            .unwrap();
    d.enabled = false;
    d.version += 1;
    f.db.configure_ftn_downstream(&f.policy, "operator", &d, 1, NOW)
        .unwrap();
    assert!(matches!(
        f.db.begin_binkp(
            &f.policy,
            &transport(&f),
            "downb",
            "hub-test",
            BinkpMode::Poll,
            NOW
        ),
        Err(Error::Held)
    ));
    let before = f.db.ftn_queue(None).unwrap().len();
    toss(&mut f, 402, "TEST1");
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), before + 1);
}
#[test]
fn areafix_parser_bounds_and_strict_complete_grammar() {
    assert_eq!(
        parse_areafix(
            "%help\n%list\n%query\n+test1\n-test2\n%rescan test1 R=3\n--- Synthetic",
            5
        )
        .unwrap()
        .len(),
        6
    );
    for b in [
        "%RESCAN TEST1 0",
        "%RESCAN TEST1 -1",
        "%RESCAN TEST1 R=3 trailing",
        "+TEST1\n%EXEC",
        "+TEST1\n%QUERY node",
        "+*",
        "",
        "%RESCAN",
    ] {
        assert!(parse_areafix(b, 5).is_err(), "{b}");
    }
    assert!(parse_areafix(&"%HELP\n".repeat(33), 5).is_err());
    assert!(parse_areafix(&"A".repeat(4097), 5).is_err());
}

#[test]
fn inbound_rescan_marker_does_not_refanout_and_preserves_normal_duplicates() {
    let mut f = hub();
    let mut p = Packet::decode(&inbound(501, "10:100/1", Some("TEST1")), (10, 10)).unwrap();
    let mut t = Text::parse(&p.messages[0].text, Charset::Utf8).unwrap();
    t.add_control("RESCANNED 10:100/2@synthetic");
    p.messages[0].text = t.encode().unwrap();
    let bytes = p.encode().unwrap();
    assert_eq!(
        f.db.toss_ftn(&f.store, &f.policy, "peer", &bytes, NOW)
            .unwrap()
            .imported,
        1
    );
    assert!(f.db.ftn_queue(None).unwrap().is_empty());
    assert_eq!(toss(&mut f, 501, "TEST1").duplicates, 1);
    assert!(f.db.ftn_queue(None).unwrap().is_empty());
    assert_eq!(toss(&mut f, 502, "TEST1").imported, 1);
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), 3);
}

#[test]
fn areafix_transaction_failure_rolls_back_changes_response_and_replay_receipt() {
    let mut f = hub();
    let before = f.db.ftn_subscriptions(0).unwrap();
    let bytes = request(&f, "next", 601, "+TEST2", "synthetic-area-secret", None);
    let p = transport(&f);
    let session =
        f.db.begin_binkp(&f.policy, &p, "next", "hub-test", BinkpMode::Poll, NOW)
            .unwrap();
    f.db.observe_binkp(
        &session,
        &[f.policy.link("next").unwrap().remote.clone()],
        &[],
        0,
        NOW,
    )
    .unwrap();
    f.db.connection.execute_batch("CREATE TRIGGER reject_areafix BEFORE INSERT ON ftn_areafix_requests BEGIN SELECT RAISE(ABORT,'injected transaction failure'); END;").unwrap();
    assert!(f
        .db
        .receive_binkp_with_areafix(
            &f.store,
            &f.policy,
            &p,
            &session,
            &bytes,
            NOW,
            Some(&|_, s| s == "synthetic-area-secret")
        )
        .is_err());
    assert_eq!(f.db.ftn_subscriptions(0).unwrap(), before);
    assert!(f.db.ftn_queue(None).unwrap().is_empty());
    assert!(f.db.ftn_areafix_activity(0).unwrap().is_empty());
    f.db.connection
        .execute_batch("DROP TRIGGER reject_areafix")
        .unwrap();
    assert_eq!(
        f.db.receive_binkp_with_areafix(
            &f.store,
            &f.policy,
            &p,
            &session,
            &bytes,
            NOW,
            Some(&|_, s| s == "synthetic-area-secret")
        )
        .unwrap()
        .imported,
        1
    );
    assert_eq!(f.db.ftn_areafix_activity(0).unwrap().len(), 1);
    assert_eq!(f.db.ftn_queue(None).unwrap().len(), 1);
    f.db.finish_binkp(&session, None, NOW).unwrap();
    let mut removed = p;
    removed.links.retain(|l| l.link != "next");
    assert!(f.db.validate_ftn_hub_transport(&removed).is_err());
}
#[test]
fn private_deleted_and_unsubscribed_content_never_enters_rescan() {
    let mut f = hub();
    for seq in 701..705 {
        toss(&mut f, seq, "TEST1");
    }
    f.db.connection.execute("UPDATE messages SET lifecycle_state='deleted',state_version=state_version+1 WHERE message_id=(SELECT MIN(message_id) FROM messages)",[]).unwrap();
    f.db.toss_ftn(
        &f.store,
        &f.policy,
        "peer",
        &inbound(705, "10:100/1", None),
        NOW,
    )
    .unwrap();
    let request =
        f.db.rescan_ftn(
            &f.policy,
            "operator",
            "downb",
            &[RescanArea {
                area: "TEST1".into(),
                count: 5,
            }],
            NOW,
        )
        .unwrap();
    let invalid:u32=f.db.connection.query_row("SELECT COUNT(*) FROM ftn_routing_decisions d JOIN ftn_messages f USING(publication_id) JOIN messages m USING(message_id) WHERE d.delivery_key=?1 AND (m.visibility<>'public' OR m.lifecycle_state<>'active' OR m.container_kind<>'conference')",[&request],|r|r.get(0)).unwrap();
    assert_eq!(invalid, 0);
    assert_eq!(f.db.ftn_rescan_activity(0).unwrap()[0].queued, 3);
    assert!(f
        .db
        .rescan_ftn(
            &f.policy,
            "operator",
            "point1",
            &[RescanArea {
                area: "TEST2".into(),
                count: 1
            }],
            NOW
        )
        .is_err());
    assert!(f
        .db
        .rescan_ftn(
            &f.policy,
            "operator",
            "point1",
            &[RescanArea {
                area: "@netmail/10:100/1/recipient".into(),
                count: 1
            }],
            NOW
        )
        .is_err());
}

#[test]
fn queued_rescan_restore_never_reoffers_its_accepted_member() {
    let mut f = hub();
    for seq in 801..804 {
        toss(&mut f, seq, "TEST1");
    }
    for link in ["next", "downb", "point1"] {
        accept_link(&mut f, link, NOW);
    }
    f.db.rescan_ftn(
        &f.policy,
        "operator",
        "downb",
        &[RescanArea {
            area: "TEST1".into(),
            count: 3,
        }],
        NOW,
    )
    .unwrap();
    let p = transport(&f);
    let session =
        f.db.begin_binkp(&f.policy, &p, "downb", "hub-test", BinkpMode::Poll, NOW)
            .unwrap();
    let work =
        f.db.claim_binkp(&f.policy, &p, &f.store, &session, NOW)
            .unwrap();
    assert_eq!(work.len(), 3);
    f.db.offered_binkp(&session, &work[0].queue).unwrap();
    f.db.accepted_binkp(&session, &work[0].queue, NOW).unwrap();
    f.db.finish_binkp(&session, Some(sf_net::binkp::Error::Interrupted), NOW)
        .unwrap();
    let snapshot = f.temp.path().join("rescan-backup.db");
    f.db.connection
        .execute("VACUUM INTO ?1", [snapshot.to_str().unwrap()])
        .unwrap();
    let mut restored = RuntimeDatabase::open(&snapshot).unwrap();
    restored.recover_binkp(NOW).unwrap();
    restored.hold_restored_ftn().unwrap();
    assert_eq!(restored.ftn_rescan_activity(0).unwrap()[0].accepted, 1);
    for q in restored
        .ftn_queue(None)
        .unwrap()
        .into_iter()
        .filter(|q| q.state == "held")
    {
        restored
            .release_binkp(&f.policy, "operator", &q.id, q.version, NOW)
            .unwrap();
    }
    let session = restored
        .begin_binkp(&f.policy, &p, "downb", "restored", BinkpMode::Poll, NOW)
        .unwrap();
    let pending = restored
        .claim_binkp(&f.policy, &p, &f.store, &session, NOW)
        .unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().all(|q| q.queue != work[0].queue));
    for q in pending {
        restored.offered_binkp(&session, &q.queue).unwrap();
        restored.accepted_binkp(&session, &q.queue, NOW).unwrap();
    }
    restored.finish_binkp(&session, None, NOW).unwrap();
    assert_eq!(restored.ftn_rescan_activity(0).unwrap()[0].accepted, 3);
}
