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

//! Native daemon/operator/Event/restart/backup journey on disposable local boards.
#![cfg(unix)]
use sf_core::conference_health::{Page, Query};
use sf_core::*;
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(config: &Path) -> Daemon {
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_spitfire"))
            .arg("run")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
fn stop(mut d: Daemon) {
    assert!(Command::new("kill")
        .args(["-INT", &d.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(s) = d.0.try_wait().unwrap() {
            assert!(s.success());
            return;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(30));
    }
}
async fn client(config: &Path) -> sf_bbs::OperatorClient {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match sf_bbs::OperatorClient::connect(config).await {
            Ok(mut c) => {
                c.describe_operator_controls().await.unwrap();
                return c;
            }
            Err(error) => assert!(
                Instant::now() < until,
                "local operator start deadline: {error}"
            ),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn action(
    c: &mut sf_bbs::OperatorClient,
    request: sf_bbs::conference_health::Command,
) -> sf_bbs::NetworkResult {
    c.qwk_network_action(
        format!("{:032x}", rand::random::<u128>()),
        sf_bbs::NetworkAction::ConferenceHealth { request },
    )
    .await
    .unwrap()
}
async fn settled(c: &mut sf_bbs::OperatorClient) -> Page {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let p = c.conference_health(Query::default()).await.unwrap();
        if p.snapshot.pending_messages == 0
            && p.snapshot.pending_conferences == 0
            && !p.snapshot.historical_scan_pending
            && p.snapshot.rows.iter().all(|r| r.detail.is_some())
        {
            return p;
        }
        assert!(Instant::now() < until, "Event rollup deadline");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
#[tokio::test]
async fn native_health_daemon_event_restart_and_cold_restore() {
    use sf_bbs::conference_health::Command as Health;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("board");
    let mut plan =
        sf_bbs::SetupPlan::stock_defaults("Synthetic Health", "Synthetic Sysop", "SYSOP", 2);
    plan.config.transports.clear();
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    let principal = sf_bbs::current_operator_identity().unwrap();
    plan.config.operators.local_identities = vec![LocalOperatorIdentity::Unix {
        uid: principal
            .strip_prefix("unix-uid:")
            .unwrap()
            .parse()
            .unwrap(),
        label: Some("Synthetic Health Operator".into()),
        capabilities: vec![
            LocalOperatorCapability::BoardStatistics,
            LocalOperatorCapability::ChangeOnlineConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::NetworkRun,
            LocalOperatorCapability::NetworkStatus,
        ],
    }];
    plan.conferences.truncate(1);
    plan.conferences[0].public_only = true;
    plan.conferences[0].posting_identity = Some(PostingIdentityPolicy::HandleAllowed);
    sf_bbs::setup_board(&root, &plan, b"synthetic password").unwrap();
    let config = root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&root, &cfg.validate().unwrap()).unwrap();
    let dbpath = paths.database().to_owned();
    let mut db = RuntimeDatabase::open(&dbpath).unwrap();
    let caller = db
        .create_caller(
            b"Synthetic Reader",
            &CredentialHasher::new(&plan.config.caller.password)
                .unwrap()
                .hash(b"synthetic reader password")
                .unwrap(),
            SecurityLevel::new(100).unwrap(),
            CallerState::Active,
            false,
            1,
        )
        .unwrap();
    let actor = MessageActor::new(caller.id, SecurityLevel::new(100).unwrap());
    let conf = db.conferences(actor).unwrap()[0].id;
    let post = |db: &mut RuntimeDatabase| {
        db.post(
            actor,
            NewMessage {
                identity_preview: None,
                conference_id: conf,
                recipient_caller_id: None,
                recipient_name: "All Callers".into(),
                subject: b"Synthetic Health".to_vec(),
                body: b"No private analytics".to_vec(),
                created_at: chrono::Utc::now().timestamp(),
                parent_message_id: None,
                visibility: MessageVisibility::Public,
                kind: MessageKind::Standard,
            },
        )
        .unwrap()
    };
    db.event_save(
        &sf_core::events::Definition {
            id: "health-acceptance".into(),
            name: "Synthetic disabled Event".into(),
            enabled: false,
            action: sf_core::events::Action::ConferenceHealth,
            schedule: sf_core::events::Schedule::Manual,
            timezone: "UTC".into(),
            policy: sf_core::events::ExchangePolicy::Manual,
            missed: sf_core::events::MissedPolicy::RunOnce,
            minimum_spacing_seconds: 5,
        },
        0,
        chrono::Utc::now().timestamp(),
    )
    .unwrap();
    let m = post(&mut db);
    db.mark_read(actor, conf, m.number).unwrap();
    drop(db);
    let daemon = start(&config);
    let mut c = client(&config).await;
    let command_config = config.clone();
    let output = tokio::task::spawn_blocking(move || {
        sf_bbs::conference_health::run(&command_config, &["status".into()])
    })
    .await
    .unwrap()
    .unwrap();
    assert!(serde_json::from_str::<Page>(&output).is_ok());
    let initial = c.conference_health(Query::default()).await.unwrap();
    assert!(initial.snapshot.pending_messages > 0);
    assert!(matches!(
        action(&mut c, Health::Schedule).await,
        sf_bbs::NetworkResult::Updated
    ));
    // Short recurrence is a disposable acceptance policy, saved through existing Events.
    let db = RuntimeDatabase::open(&dbpath).unwrap();
    let event = db
        .events()
        .unwrap()
        .into_iter()
        .find(|e| e.definition.id == "conference-health")
        .unwrap();
    let mut definition = event.definition.clone();
    definition.schedule = sf_core::events::Schedule::Interval { seconds: 5 };
    let result = c
        .qwk_network_action(
            format!("{:032x}", rand::random::<u128>()),
            sf_bbs::NetworkAction::Event {
                request: sf_bbs::events::Command::Save {
                    definition,
                    expected: event.version,
                },
            },
        )
        .await
        .unwrap();
    assert!(matches!(result, sf_bbs::NetworkResult::Updated));
    let p = settled(&mut c).await;
    let metrics = &p.snapshot.rows[0].detail.as_ref().unwrap().windows[1];
    assert_eq!(
        (metrics.local_posts, metrics.readers, metrics.progress),
        (1, 1, 1)
    );
    let request_id = format!("{:032x}", rand::random::<u128>());
    let request = sf_bbs::NetworkAction::ConferenceHealth {
        request: Health::Rollup,
    };
    assert!(matches!(
        c.qwk_network_action(request_id.clone(), request.clone())
            .await
            .unwrap(),
        sf_bbs::NetworkResult::Updated
    ));
    assert!(matches!(
        c.qwk_network_action(request_id, request).await.unwrap(),
        sf_bbs::NetworkResult::Replayed { .. }
    ));
    assert_eq!(
        c.conference_health(Query::default())
            .await
            .unwrap()
            .snapshot
            .rows[0]
            .detail
            .as_ref()
            .unwrap()
            .windows[1]
            .progress,
        1
    );
    let mut settings = p.snapshot.settings;
    settings.bulletin = true;
    assert!(matches!(
        action(&mut c, Health::Configure { settings }).await,
        sf_bbs::NetworkResult::Updated
    ));
    action(&mut c, Health::Rollup).await;
    assert_eq!(
        db.conference_health_hot(actor, chrono::Utc::now().timestamp())
            .unwrap()
            .len(),
        1
    );
    drop(c);
    stop(daemon);
    drop(db);
    let backup = temp.path().join("backup");
    sf_bbs::backup_board(&config, &backup).unwrap();
    let mut db = RuntimeDatabase::open(&dbpath).unwrap();
    post(&mut db);
    drop(db);
    let daemon = start(&config);
    let mut c = client(&config).await;
    action(&mut c, Health::Rollup).await;
    assert_eq!(
        c.conference_health(Query::default())
            .await
            .unwrap()
            .snapshot
            .rows[0]
            .detail
            .as_ref()
            .unwrap()
            .windows[1]
            .local_posts,
        2
    );
    drop(c);
    stop(daemon);
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let config = restored.join(sf_bbs::BOARD_CONFIG_FILE);
    let daemon = start(&config);
    let mut c = client(&config).await;
    action(&mut c, Health::Rollup).await;
    let p = c.conference_health(Query::default()).await.unwrap();
    assert_eq!(
        p.snapshot.rows[0].detail.as_ref().unwrap().windows[1].local_posts,
        1
    );
    assert!(p.snapshot.settings.bulletin);
    drop(c);
    stop(daemon);
}
