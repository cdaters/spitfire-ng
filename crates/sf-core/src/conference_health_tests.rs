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

//! Disposable synthetic observations, never historical readership reconstruction.
use super::*;
use crate::*;
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    db: RuntimeDatabase,
    user: MessageActor,
    sysop: MessageActor,
    now: i64,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let mut db = RuntimeDatabase::open(&temp.path().join("board.sqlite")).unwrap();
        db.migrate().unwrap();
        db.ensure_board_identity(
            &BoardIdentity::new("Synthetic Health Board", "Test Operator").unwrap(),
        )
        .unwrap();
        let hash = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
        .hash(b"synthetic password")
        .unwrap();
        let user = db
            .create_caller(
                b"Synthetic Reader",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let sysop = db
            .create_caller(
                b"Synthetic Operator",
                &hash,
                SecurityLevel::new(100).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        Self {
            temp,
            db,
            user: MessageActor::new(user.id, SecurityLevel::new(100).unwrap()),
            sysop: MessageActor::new(sysop.id, SecurityLevel::new(100).unwrap()),
            now: chrono::Utc::now().timestamp(),
        }
    }
    fn area(&mut self, number: u16, security: u16) -> ConferenceId {
        self.db
            .ensure_conference(&ConferenceDefinition {
                posting_identity: None,
                number,
                name: format!("Synthetic {number}"),
                description: "Synthetic health fixture".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(security).unwrap(),
                post_security: SecurityLevel::new(security).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 50,
                privileged_security_levels: vec![],
            })
            .unwrap()
            .id
    }
    fn post(
        &mut self,
        c: ConferenceId,
        days: i64,
        inbound: bool,
        parent: Option<MessageId>,
    ) -> Message {
        if inbound {
            let tx = self.db.connection.transaction().unwrap();
            let number = crate::message::next_message_number(&tx, c).unwrap();
            let id = crate::message::next_message_id(&tx).unwrap();
            let at = self.now - days * DAY;
            tx.execute("INSERT INTO message_payloads(subject,body,content_kind,encoding) VALUES(?1,?2,'standard','utf8')",params![b"Synthetic".as_slice(),b"Synthetic inbound".as_slice()]).unwrap();
            let payload = tx.last_insert_rowid();
            tx.execute("INSERT INTO message_fanouts(payload_id,created_by_caller_id,created_at) VALUES(?1,NULL,?2)",params![payload,at]).unwrap();
            let fanout = tx.last_insert_rowid();
            tx.execute("INSERT INTO messages(message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal,origin_kind,identity_mode) VALUES(?1,?2,?3,?4,NULL,'Synthetic External',?5,?5,?6,'all-callers','public','active','single',0,'external-network','external-asserted')",params![id.get(),fanout,c.get(),i64::try_from(number).unwrap(),at,parent.map(|p|p.get())]).unwrap();
            tx.commit().unwrap();
            return self.db.message(self.sysop, c, number).unwrap();
        }
        let m = self
            .db
            .post(
                self.sysop,
                NewMessage {
                    identity_preview: None,
                    conference_id: c,
                    recipient_caller_id: None,
                    recipient_name: "All Callers".into(),
                    subject: b"Synthetic".to_vec(),
                    body: b"Synthetic public message".to_vec(),
                    created_at: self.now - days * DAY,
                    parent_message_id: parent,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();
        self.db
            .connection
            .execute(
                "UPDATE messages SET placed_at=?1 WHERE message_id=?2",
                params![self.now - days * DAY, m.id.get()],
            )
            .unwrap();
        m
    }
    fn mature(&self) {
        self.db
            .connection
            .execute(
                "UPDATE conference_health_config SET monitoring_since=?1",
                [self.now - 100 * DAY],
            )
            .unwrap();
        self.db
            .connection
            .execute(
                "UPDATE message_conferences SET created_at=datetime(?1,'unixepoch')",
                [self.now - 100 * DAY],
            )
            .unwrap();
    }
    fn reads(&self, c: ConferenceId, days: i64, n: usize) {
        // Explicitly synthetic aggregate history for deterministic window/status tests.
        for i in 0..n {
            self.db.connection.execute("INSERT INTO conference_health_reads VALUES(?1,?2,?3,1,?4) ON CONFLICT DO NOTHING",params![c.get(),(self.now-days*DAY)/DAY,format!("{i:032x}"),self.now-days*DAY]).unwrap();
        }
        self.db
            .connection
            .execute(
                "INSERT OR IGNORE INTO conference_health_dirty VALUES(?1)",
                [c.get()],
            )
            .unwrap();
    }
    fn settle(&mut self) {
        for _ in 0..1000 {
            let r = self.db.conference_health_rollup(self.now).unwrap();
            assert!(r.messages <= MESSAGE_BATCH && r.conferences <= SNAPSHOT_BATCH);
            if !r.pending {
                return;
            }
        }
        panic!("bounded fixture failed to settle")
    }
    fn detail(&self, c: ConferenceId) -> Detail {
        self.db
            .conference_health_snapshot(self.now)
            .unwrap()
            .rows
            .into_iter()
            .find(|r| r.conference_id == c.get())
            .unwrap()
            .detail
            .unwrap()
    }
}
#[test]
fn progress_is_not_exact_reads_and_reset_is_not_readership() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let a = f.post(c, 0, false, None);
    let _b = f.post(c, 0, false, None);
    let last = f.post(c, 0, false, None);
    f.db.mark_read(f.user, c, a.number).unwrap();
    f.db.mark_read(f.user, c, a.number).unwrap();
    f.db.mark_read(f.user, c, last.number).unwrap();
    f.db.mark_read(f.sysop, c, last.number).unwrap();
    f.settle();
    let m = &f.detail(c).windows[1];
    assert_eq!((m.readers, m.progress, m.local_posts), (2, 3, 3));
    f.db.connection
        .execute(
            "UPDATE caller_last_read SET last_message_number=0,reset_version=reset_version+1",
            [],
        )
        .unwrap();
    f.db.connection
        .execute(
            "UPDATE caller_last_read SET last_message_number=?1,reset_version=reset_version+1",
            [i64::try_from(last.number).unwrap()],
        )
        .unwrap();
    f.settle();
    assert_eq!(f.detail(c).windows[1].progress, 3);
}
#[test]
fn windows_provenance_threads_and_idempotent_corrections() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let root = f.post(c, 89, false, None);
    f.post(c, 60, true, None);
    f.post(c, 29, true, Some(root.id));
    let reply = f.post(c, 6, false, Some(root.id));
    f.post(c, 0, true, None);
    f.post(c, 91, true, None);
    f.reads(c, 0, 4);
    f.reads(c, 6, 4);
    f.reads(c, 29, 5);
    f.reads(c, 89, 6);
    f.mature();
    f.settle();
    let d = f.detail(c);
    assert_eq!(
        d.windows.iter().map(Metrics::posts).collect::<Vec<_>>(),
        [2, 3, 5]
    );
    assert_eq!(
        d.windows.iter().map(|m| m.readers).collect::<Vec<_>>(),
        [4, 5, 6]
    );
    assert_eq!(
        (
            d.windows[1].local_posts,
            d.windows[1].inbound_posts,
            d.windows[1].replies,
            d.windows[1].active_threads
        ),
        (1, 2, 2, 2)
    );
    assert_eq!(d.windows[1].reads_per_post(), Some(13.0 / 3.0));
    assert_eq!(d.windows[1].replies_per_active_thread(), Some(1.0));
    let before = serde_json::to_value(&d).unwrap();
    f.settle();
    assert_eq!(before, serde_json::to_value(f.detail(c)).unwrap());
    f.db.connection
        .execute(
            "UPDATE messages SET lifecycle_state='deleted' WHERE message_id=?1",
            [reply.id.get()],
        )
        .unwrap();
    f.settle();
    assert_eq!(f.detail(c).windows[1].local_posts, 0);
}
#[test]
fn explainable_statuses_age_and_trends() {
    let mut f = Fixture::new();
    let healthy = f.area(1, 5);
    let unread = f.area(2, 5);
    let rising = f.area(3, 5);
    let quiet = f.area(4, 5);
    let dormant = f.area(5, 5);
    f.post(healthy, 0, false, None);
    f.reads(healthy, 0, 6);
    f.reads(healthy, 40, 6);
    f.post(unread, 0, true, None);
    f.post(rising, 0, true, None);
    f.reads(rising, 0, 10);
    f.reads(rising, 40, 5);
    f.reads(quiet, 0, 2);
    f.reads(quiet, 40, 10);
    f.mature();
    let new = f.area(6, 5);
    f.settle();
    assert_eq!(f.detail(healthy).status, Status::Healthy);
    assert_eq!(f.detail(healthy).trend, Trend::Stable);
    assert_eq!(f.detail(unread).status, Status::LocallyUnread);
    assert_eq!(f.detail(rising).trend, Trend::Rising);
    assert_eq!(f.detail(quiet).status, Status::Quiet);
    assert_eq!(f.detail(quiet).trend, Trend::Falling);
    assert_eq!(f.detail(dormant).status, Status::Dormant);
    assert_eq!(f.detail(new).status, Status::New);
    assert!(f
        .detail(unread)
        .reasons
        .join(" ")
        .contains("1 inbound; 0 reader-progress"));
}
#[test]
fn caller_access_bulletin_retirement_and_no_personal_projection() {
    let mut f = Fixture::new();
    let public = f.area(1, 5);
    let secret = f.area(2, 100);
    let retired = f.area(3, 5);
    let a = f.post(public, 0, false, None);
    let b = f.post(secret, 0, false, None);
    assert!(f.db.mark_read(f.user, secret, b.number).is_err());
    f.db.mark_read(f.user, public, a.number).unwrap();
    f.db.mark_read(f.sysop, secret, b.number).unwrap();
    let mut settings = f.db.conference_health_settings().unwrap();
    settings.bulletin = true;
    f.db.conference_health_configure(&settings, f.now).unwrap();
    f.settle();
    f.db.connection
        .execute(
            "UPDATE message_conferences SET active=0 WHERE conference_id=?1",
            [retired.get()],
        )
        .unwrap();
    let caller = f.db.conference_health_hot(f.user, f.now).unwrap();
    assert_eq!(caller.len(), 1);
    assert_eq!(caller[0].conference_id, public.get());
    assert_eq!(f.db.conference_health_hot(f.sysop, f.now).unwrap().len(), 2);
    // Restrict after aggregation: current access applies without waiting for rollup.
    f.db.connection
        .execute(
            "UPDATE message_conferences SET read_security=100 WHERE conference_id=?1",
            [public.get()],
        )
        .unwrap();
    assert!(f
        .db
        .conference_health_hot(f.user, f.now)
        .unwrap()
        .is_empty());
    let json = serde_json::to_string(&f.db.conference_health_snapshot(f.now).unwrap()).unwrap();
    for forbidden in [
        "Synthetic Reader",
        "Synthetic Operator",
        "Synthetic public message",
        "caller_id",
        "token",
    ] {
        assert!(!json.contains(forbidden));
    }
    assert_eq!(
        f.db.conference_health_page(&Query::default(), f.now)
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        f.db.conference_health_page(
            &Query {
                include_retired: true,
                ..Query::default()
            },
            f.now
        )
        .unwrap()
        .total,
        3
    );
}
#[test]
fn deleted_reader_counts_survive_without_identity_link() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let m = f.post(c, 0, false, None);
    f.db.mark_read(f.user, c, m.number).unwrap();
    f.settle();
    f.db.connection
        .execute(
            "UPDATE callers SET account_state='deleted' WHERE caller_id=?1",
            [f.user.caller_id().get()],
        )
        .unwrap();
    let count: i64 =
        f.db.connection
            .query_row(
                "SELECT COUNT(*) FROM conference_health_tokens WHERE caller_id=?1",
                [f.user.caller_id().get()],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(count, 0);
    f.settle();
    assert_eq!(f.detail(c).windows[1].readers, 1);
}
#[test]
fn restart_backup_restore_pending_and_clock_rollback() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let m = f.post(c, 0, false, None);
    f.db.mark_read(f.user, c, m.number).unwrap();
    let backup = f.temp.path().join("backup.sqlite");
    f.db.backup_to(&backup).unwrap();
    f.settle();
    let expected = serde_json::to_value(f.detail(c)).unwrap();
    f.post(c, 0, false, None);
    f.settle();
    assert_eq!(f.detail(c).windows[0].posts(), 2);
    f.db = RuntimeDatabase::open(&backup).unwrap();
    f.db.validate_conference_health().unwrap();
    f.settle();
    assert_eq!(expected, serde_json::to_value(f.detail(c)).unwrap());
    f.now -= DAY;
    f.settle();
    let d = f.detail(c);
    assert_eq!(d.windows[0].posts(), 0);
    assert_eq!(d.windows[0].readers, 0);
    assert!(d.last_read.is_none());
    f.now += DAY;
    f.settle();
    assert_eq!(f.detail(c).windows[0].posts(), 1);
}
#[test]
fn disabled_tracking_retention_and_validation() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let m = f.post(c, 0, false, None);
    let mut s = f.db.conference_health_settings().unwrap();
    s.enabled = false;
    f.db.conference_health_configure(&s, f.now).unwrap();
    f.db.mark_read(f.user, c, m.number).unwrap();
    f.settle();
    s.enabled = true;
    s.retention_days = 180;
    f.db.conference_health_configure(&s, f.now).unwrap();
    f.reads(c, 181, 2);
    f.post(c, 181, false, None);
    f.settle();
    assert_eq!(f.detail(c).windows[2].readers, 0);
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM conference_health_reads", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    // With no rollup Event, each new observation removes at most one old row.
    f.reads(c, 181, 2);
    let latest = f.post(c, 0, false, None);
    f.db.mark_read(f.user, c, latest.number).unwrap();
    let expired = |f: &Fixture| {
        f.db.connection
            .query_row(
                "SELECT COUNT(*) FROM conference_health_reads WHERE day < ?1",
                [f.now / DAY - 180],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert_eq!(expired(&f), 1); // Existing high-water update.
    f.db.mark_read(f.sysop, c, latest.number).unwrap();
    assert_eq!(expired(&f), 0); // First high-water insertion.
    assert_eq!(
        f.db.connection
            .query_row("SELECT COUNT(*) FROM conference_health_reads", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    s.retention_days = 90;
    assert!(f.db.conference_health_configure(&s, f.now).is_err());
    assert!(f.db.conference_health_rollup(-1).is_err());
    f.db.validate_conference_health().unwrap();
}
#[test]
fn bounded_large_fixture_cached_page_is_responsive() {
    let mut f = Fixture::new();
    let started = std::time::Instant::now();
    for n in 1..=200 {
        let c = f.area(n, 5);
        for _ in 0..100 {
            f.post(c, 0, n % 2 == 0, None);
        }
        f.reads(c, 0, 4);
    }
    let seeded = started.elapsed();
    let rollup = std::time::Instant::now();
    f.settle();
    let elapsed = rollup.elapsed();
    let view = std::time::Instant::now();
    for _ in 0..10 {
        let page =
            f.db.conference_health_page(&Query::default(), f.now)
                .unwrap();
        assert_eq!(page.total, 200);
        assert_eq!(page.snapshot.rows.len(), 32);
        assert!(serde_json::to_vec(&page).unwrap().len() < 1_000_000);
    }
    eprintln!("C8 performance 200 conferences / 20000 messages: seed {seeded:?}, rollup {elapsed:?}, 10 cached pages {:?}",view.elapsed());
    assert!(elapsed.as_secs() < 60);
    assert!(view.elapsed().as_secs() < 10);
}

#[test]
fn multiple_adapter_contexts_never_multiply_native_posts() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    f.post(c, 0, true, None);
    f.db.connection.execute("INSERT INTO ftn_area_mappings(domain,area,conference_id,aka,receive,send,origin,version) VALUES('synthetic','SYNTH',?1,'1:2/3',1,1,'Synthetic',1)",[c.get()]).unwrap();
    f.db.connection.execute("INSERT INTO qwk_links(link_id,network,local_id,remote_id,name,profile,role,enabled,inbound,outbound,version) VALUES('synthetic','synthetic','LOCAL','REMOTE','Synthetic','qwk-headers','node',1,1,1,1)",[]).unwrap();
    f.db.connection.execute("INSERT INTO qwk_link_mappings(link_id,wire_conference,area,conference_id,enabled,inbound,outbound,version) VALUES('synthetic',1,'SYNTH',?1,1,1,1,1)",[c.get()]).unwrap();
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_profiles VALUES('synthetic','{}',1)",
            [],
        )
        .unwrap();
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_mappings VALUES('synthetic','SYNTH',?1,1,1,1)",
            [c.get()],
        )
        .unwrap();
    f.settle();
    let p =
        f.db.conference_health_page(&Query::default(), f.now)
            .unwrap();
    assert_eq!(p.total, 1);
    assert_eq!(p.snapshot.rows[0].contexts.len(), 3);
    assert_eq!(
        p.snapshot.rows[0].detail.as_ref().unwrap().windows[1].inbound_posts,
        1
    );
    for filter in [Filter::Ftn, Filter::Qwk, Filter::Circuitnet] {
        assert_eq!(
            f.db.conference_health_page(
                &Query {
                    filter,
                    ..Query::default()
                },
                f.now
            )
            .unwrap()
            .total,
            1
        );
    }
    assert_eq!(
        f.db.conference_health_page(
            &Query {
                filter: Filter::Local,
                ..Query::default()
            },
            f.now
        )
        .unwrap()
        .total,
        0
    );
}
#[test]
fn native_thread_correction_and_private_exclusion() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    let a = f.post(c, 0, false, None);
    let b = f.post(c, 0, false, None);
    let child = f.post(c, 0, false, Some(b.id));
    f.post(c, 0, false, Some(child.id));
    f.settle();
    assert_eq!(f.detail(c).windows[1].active_threads, 2);
    f.db.connection
        .execute(
            "UPDATE messages SET parent_message_id=?1 WHERE message_id=?2",
            params![a.id.get(), b.id.get()],
        )
        .unwrap();
    f.settle();
    assert_eq!(f.detail(c).windows[1].active_threads, 1);
    assert_eq!(f.detail(c).windows[1].replies, 3);
    // Native local-recipient delivery is not public readership, even in a mixed area.
    f.db.connection
        .execute(
            "UPDATE message_conferences SET public_only=0 WHERE conference_id=?1",
            [c.get()],
        )
        .unwrap();
    let m =
        f.db.post(
            f.sysop,
            NewMessage {
                identity_preview: None,
                conference_id: c,
                recipient_caller_id: Some(f.user.caller_id()),
                recipient_name: "Synthetic Reader".into(),
                subject: b"Synthetic restricted".to_vec(),
                body: b"Restricted synthetic text".to_vec(),
                created_at: f.now,
                parent_message_id: None,
                visibility: MessageVisibility::Private,
                kind: MessageKind::Standard,
            },
        )
        .unwrap();
    f.db.mark_read(f.user, c, m.number).unwrap();
    f.settle();
    assert_eq!(f.detail(c).windows[1].posts(), 4);
    assert_eq!(f.detail(c).windows[1].readers, 0);
}

#[test]
fn retired_catalog_context_preserves_native_history() {
    let mut f = Fixture::new();
    let c = f.area(1, 5);
    f.post(c, 0, false, None);
    f.settle();
    // Projection fixture: signature admission itself is covered by catalog tests.
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_profiles VALUES('synthetic','{}',1)",
            [],
        )
        .unwrap();
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_mappings VALUES('synthetic','SYNTH',?1,0,0,1)",
            [c.get()],
        )
        .unwrap();
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_catalog_authority VALUES('synthetic','{}')",
            [],
        )
        .unwrap();
    f.db.connection.execute("INSERT INTO circuitnet_catalog_choices VALUES('synthetic','generation-one','mapped',?1,0)",[c.get()]).unwrap();
    let object=serde_json::json!({"body":{"entries":[{"id":"generation-one","codename":"SYNTH","status":"retired"}]}}).to_string();
    f.db.connection
        .execute(
            "INSERT INTO circuitnet_catalog_revisions VALUES('synthetic',1,'synthetic-hash',?1,?2)",
            params![object, f.now],
        )
        .unwrap();
    assert_eq!(
        f.db.conference_health_page(&Query::default(), f.now)
            .unwrap()
            .total,
        0
    );
    let p =
        f.db.conference_health_page(
            &Query {
                include_retired: true,
                ..Query::default()
            },
            f.now,
        )
        .unwrap();
    assert!(p.snapshot.rows[0].retired);
    assert_eq!(
        p.snapshot.rows[0].detail.as_ref().unwrap().windows[0].posts(),
        1
    );
    assert_eq!(f.db.conferences(f.sysop).unwrap().len(), 1);
}
