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

//! Real isolated BinkP daemons, native mail custody and restart/restore acceptance.
use sf_bbs::{binkp, NetworkAction, NetworkResult, OperatorClient};
use sf_core::{ftn::*, *};
use std::{
    fs,
    path::{Path, PathBuf},
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
#[cfg(unix)]
fn graceful(mut daemon: Daemon) {
    assert!(Command::new("kill")
        .args(["-INT", &daemon.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = daemon.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < until, "graceful BinkP shutdown deadline");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn start(config: &Path) -> Daemon {
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_spitfire"))
            .arg("run")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                fs::File::create(config.with_extension("log")).unwrap(),
            ))
            .stderr(Stdio::from(
                fs::File::create(config.with_extension("errors")).unwrap(),
            ))
            .spawn()
            .unwrap(),
    )
}
fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
struct Board {
    config: PathBuf,
    paths: LogicalPaths,
    policy: Policy,
    actor: MessageActor,
    other: MessageActor,
    conference: ConferenceId,
}
fn board(root: &Path, node: u16, listen: u16, remote_port: u16) -> Board {
    board_in_domain(root, node, listen, remote_port, "synthetic")
}
fn board_in_domain(root: &Path, node: u16, listen: u16, remote_port: u16, domain: &str) -> Board {
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Synthetic N4 Board", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    let principal = sf_bbs::current_operator_identity().unwrap();
    let caps = vec![
        LocalOperatorCapability::NetworkStatus,
        LocalOperatorCapability::NetworkRun,
        LocalOperatorCapability::NetworkTest,
        LocalOperatorCapability::NetworkQueue,
        LocalOperatorCapability::ChangeSensitiveConfiguration,
        LocalOperatorCapability::ChangeOnlineConfiguration,
        LocalOperatorCapability::ReadConfiguration,
        LocalOperatorCapability::BoardStatistics,
    ];
    plan.config.operators.local_identities =
        vec![if let Some(uid) = principal.strip_prefix("unix-uid:") {
            LocalOperatorIdentity::Unix {
                uid: uid.parse().unwrap(),
                label: None,
                capabilities: caps,
            }
        } else {
            LocalOperatorIdentity::Windows {
                sid: principal.strip_prefix("windows-sid:").unwrap().into(),
                label: None,
                capabilities: caps,
            }
        }];
    plan.config.transports = vec![TransportConfig {
        name: Some("isolated-n4".into()),
        enabled: true,
        adapter: TransportAdapterConfig::Raw {
            listen: format!("127.0.0.1:{}", port()).parse().unwrap(),
            terminal: NetworkTerminalDefaults::default(),
        },
    }];
    let point = if node == 1 { 3 } else { 9 };
    let remote = if node == 1 {
        format!("10:100/2@{domain}")
    } else {
        format!("10:100/1.3@{domain}")
    };
    let primary = if node == 1 { "point" } else { "node" };
    let akas = vec![
        Aka {
            id: "node".into(),
            endpoint: format!("10:100/{node}@{domain}").parse().unwrap(),
            enabled: true,
            primary: true,
        },
        Aka {
            id: "point".into(),
            endpoint: format!("10:100/{node}.{point}@{domain}").parse().unwrap(),
            enabled: true,
            primary: false,
        },
        Aka {
            id: "other".into(),
            endpoint: format!("10:100/{node}@othernet").parse().unwrap(),
            enabled: true,
            primary: true,
        },
    ];
    let policy = Policy {
        enabled: true,
        akas,
        links: vec![Link {
            id: "peer".into(),
            remote: remote.parse().unwrap(),
            aka: primary.into(),
            enabled: true,
            inbound: true,
            outbound: true,
            transit: true,
            profile: sf_net::ftn::PacketProfile::Type2Plus,
            charset: sf_net::ftn::Charset::Utf8,
        }],
        routes: vec![Route {
            domain: domain.parse().unwrap(),
            target: RouteMatch::Default,
            link: "peer".into(),
        }],
        sources: vec![DirectorySource {
            id: "points".into(),
            domain: domain.parse().unwrap(),
            enabled: true,
            format: sf_net::ftn::directory::DirectoryFormat::Boss,
            charset: sf_net::ftn::Charset::Ascii,
            default_zone: 10,
            priority: 0,
            cadence_days: 7,
            require_crc: false,
        }],
    };
    plan.config.ftn = policy.clone();
    plan.config.binkp = BinkpPolicy {
        listener: Some(BinkpListener {
            enabled: true,
            bind: format!("127.0.0.1:{listen}").parse().unwrap(),
            akas: vec!["node".into(), "point".into()],
        }),
        links: vec![BinkpLink {
            link: "peer".into(),
            enabled: true,
            inbound: true,
            outbound: true,
            endpoint: Some("localhost".into()),
            port: remote_port,
            directory: false,
            akas: vec![
                primary.into(),
                if primary == "point" {
                    "node".into()
                } else {
                    "point".into()
                },
            ],
            remote_akas: vec![if node == 1 {
                format!("10:100/2.9@{domain}")
            } else {
                format!("10:100/1@{domain}")
            }
            .parse()
            .unwrap()],
            auth: BinkpAuth::RequireCram,
            allow_domainless: false,
        }],
    };
    let setup = sf_bbs::setup_board(root, &plan, b"synthetic-setup-password").unwrap();
    let paths = LogicalPaths::resolve(root, &plan.config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let store = sf_bbs::DiskArtifactStore::new(paths.get(LogicalPath::System)).unwrap();
    let source=format!(";A Synthetic\r\nBoss,10:100/{node}\r\n,{point},Point,Here,Person,-Unpublished-,300,INA:localhost,IBN\r\n");
    let generation = db
        .ingest_ftn_directory(
            &store,
            &policy,
            "points",
            chrono::Utc::now().date_naive(),
            source.as_bytes(),
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    db.activate_ftn_directory(
        &policy,
        "operator",
        &generation.id,
        0,
        chrono::Utc::now().timestamp(),
    )
    .unwrap();
    let hash = CredentialHasher::new(&plan.config.caller.password)
        .unwrap()
        .hash(b"synthetic-caller-password")
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
                chrono::Utc::now().timestamp(),
            )
            .unwrap();
        actors.push(MessageActor::new(c.id, SecurityLevel::new(50).unwrap()));
    }
    for aka in ["node", "point"] {
        db.configure_ftn_alias(
            &policy,
            "operator",
            &MailboxAlias {
                aka: aka.into(),
                alias: "Recipient".into(),
                caller_id: actors[0].caller_id().get(),
            },
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    }
    let mut conference = None;
    for n in [2, 3] {
        let c = db
            .ensure_conference(&ConferenceDefinition {
                number: n,
                name: format!("N4 Echo {n}"),
                description: "Synthetic echo".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![],
            })
            .unwrap();
        db.configure_ftn_mapping(
            &policy,
            "operator",
            &Mapping {
                domain: domain.parse().unwrap(),
                area: format!("TEST{}", n - 1),
                conference_id: c.id.get(),
                aka: "node".into(),
                receive: true,
                send: true,
                origin: "Synthetic N4".into(),
                links: vec!["peer".into()],
                version: 1,
            },
            0,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
        if n == 2 {
            conference = Some(c.id);
        }
    }
    Board {
        config: setup.config_path,
        paths,
        policy,
        actor: actors[0],
        other: actors[1],
        conference: conference.unwrap(),
    }
}
fn author(board: &Board, destination: &str) {
    let mut db = RuntimeDatabase::open(board.paths.database()).unwrap();
    let now = chrono::Utc::now().timestamp();
    db.send_ftn_mail(
        board.actor,
        &board.policy,
        &NewNetMail {
            aka: "point".into(),
            destination: destination.parse().unwrap(),
            recipient: "Recipient".into(),
            subject: "BinkP private journey".into(),
            body: "Private BinkP sentinel".into(),
            reply_to: None,
        },
        now,
    )
    .unwrap();
    db.post(
        board.actor,
        NewMessage {
            conference_id: board.conference,
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: b"BinkP Echo journey".to_vec(),
            body: b"Native BinkP Echo body".to_vec(),
            created_at: now,
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        },
    )
    .unwrap();
    assert_eq!(db.scan_ftn(&board.policy, now).unwrap(), 1);
}
async fn connect(config: &Path) -> OperatorClient {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(mut client) = OperatorClient::connect(config).await {
            client.describe_operator_controls().await.unwrap();
            return client;
        }
        assert!(Instant::now() < until, "daemon attach timed out");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn action(client: &mut OperatorClient, request: binkp::Action) -> binkp::Result {
    match client
        .qwk_network_action(
            format!("{:032x}", rand::random::<u128>()),
            NetworkAction::Binkp { request },
        )
        .await
        .unwrap()
    {
        NetworkResult::Binkp { response } => response,
        other => panic!("BinkP action failed: {other:?}"),
    }
}
async fn credential(client: &mut OperatorClient, secret: &str) {
    let status = client.binkp_status().await.unwrap();
    action(
        client,
        binkp::Action::Credential {
            link: "peer".into(),
            expected: status.policy,
            secret: secret.into(),
        },
    )
    .await;
    let status = client.binkp_status().await.unwrap();
    assert_eq!(status.links[0].credential, sf_bbs::SecretStatus::Configured);
    assert!(!serde_json::to_string(&status).unwrap().contains(secret));
}
async fn poll(client: &mut OperatorClient, test: bool) {
    let status = client.binkp_status().await.unwrap();
    let request = if test {
        binkp::Action::Test {
            link: "peer".into(),
            expected: status.policy,
        }
    } else {
        binkp::Action::Poll {
            link: "peer".into(),
            expected: status.policy,
        }
    };
    assert!(matches!(
        action(client, request).await,
        binkp::Result::Started { .. }
    ));
    let until = Instant::now() + Duration::from_secs(80);
    loop {
        let status = client.binkp_status().await.unwrap();
        if let Some(health) = &status.links[0].health {
            if !health.active {
                assert_eq!(health.last_error, None, "{health:?}");
                break;
            }
        }
        assert!(Instant::now() < until, "BinkP session did not finish");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
fn delivered(board: &Board) {
    let db = RuntimeDatabase::open(board.paths.database()).unwrap();
    let conn = rusqlite::Connection::open(board.paths.database()).unwrap();
    let mid:i64=conn.query_row("SELECT m.message_id FROM ftn_messages f JOIN messages m USING(message_id) WHERE f.ingress_link='peer' AND m.container_kind<>'conference'",[],|r|r.get(0)).unwrap();
    let mid = MessageId::new(mid).unwrap();
    assert_eq!(
        db.read_ftn_mail(board.actor, mid).unwrap().body,
        "Private BinkP sentinel"
    );
    assert!(db.read_ftn_mail(board.other, mid).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM ftn_messages WHERE ingress_link='peer'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    assert!(db
        .ftn_queue(None)
        .unwrap()
        .iter()
        .all(|q| q.state == "accepted"));
}
#[test]
fn native_mailer_daemons_exchange_and_restore() {
    let temp = tempfile::tempdir().unwrap();
    let one = port();
    let two = port();
    let a = board(&temp.path().join("a"), 1, one, two);
    let b = board(&temp.path().join("b"), 2, two, one);
    author(&a, "10:100/2.9@synthetic");
    author(&b, "10:100/1.3@synthetic");
    let da = start(&a.config);
    let db = start(&b.config);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut ca = rt.block_on(connect(&a.config));
    let mut cb = rt.block_on(connect(&b.config));
    let secret = format!("{:032x}", rand::random::<u128>());
    rt.block_on(credential(&mut ca, &secret));
    rt.block_on(credential(&mut cb, &secret));
    rt.block_on(poll(&mut ca, false));
    delivered(&a);
    delivered(&b);
    rt.block_on(poll(&mut cb, false));
    delivered(&a);
    delivered(&b);
    rt.block_on(poll(&mut ca, true));
    delivered(&a);
    delivered(&b);
    // A live unauthenticated listener worker must participate in graceful drain.
    let mut stalled = std::net::TcpStream::connect(format!("127.0.0.1:{one}")).unwrap();
    stalled
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut greeting = [0u8; 3];
    std::io::Read::read_exact(&mut stalled, &mut greeting).unwrap();
    drop(ca);
    drop(cb);
    #[cfg(unix)]
    {
        graceful(da);
        graceful(db);
    }
    #[cfg(not(unix))]
    {
        drop(da);
        drop(db);
    }
    drop(stalled);
    // A cold backup must retain both acknowledged history and frozen unsent work.
    author(&a, "10:100/2.9@synthetic");
    let mut pending = RuntimeDatabase::open(a.paths.database()).unwrap();
    let store = sf_bbs::DiskArtifactStore::new(a.paths.get(LogicalPath::System)).unwrap();
    for queue in pending
        .ftn_queue(None)
        .unwrap()
        .iter()
        .filter(|q| q.state != "accepted")
    {
        pending
            .build_ftn(
                &store,
                &a.policy,
                &queue.id,
                queue.version,
                chrono::Utc::now().timestamp(),
            )
            .unwrap();
    }
    drop(pending);
    let backup = temp.path().join("backup");
    sf_bbs::backup_board(&a.config, &backup).unwrap();
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let config = RuntimeConfig::load(&restored.join("spitfire.toml")).unwrap();
    let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
    let db = RuntimeDatabase::open(paths.database()).unwrap();
    let queues = db.ftn_queue(None).unwrap();
    assert_eq!(queues.iter().filter(|q| q.state == "accepted").count(), 2);
    assert_eq!(queues.iter().filter(|q| q.state == "held").count(), 2);
    assert_eq!(db.ftn_status().unwrap().active_generations, 1);
    assert!(db.binkp_health().unwrap().iter().all(|h| !h.active));
    let restored_daemon = start(&restored.join("spitfire.toml"));
    let mut restored_client = rt.block_on(connect(&restored.join("spitfire.toml")));
    assert_eq!(
        rt.block_on(restored_client.binkp_status()).unwrap().links[0].credential,
        sf_bbs::SecretStatus::Configured
    );
    let peer_again = start(&b.config);
    let peer_client = rt.block_on(connect(&b.config));
    for queue in queues.iter().filter(|q| q.state == "held") {
        rt.block_on(action(
            &mut restored_client,
            binkp::Action::Release {
                queue: queue.id.clone(),
                expected: queue.version,
            },
        ));
    }
    rt.block_on(poll(&mut restored_client, false));
    assert!(db
        .ftn_queue(None)
        .unwrap()
        .iter()
        .all(|q| q.state == "accepted"));
    let peer_db = rusqlite::Connection::open(b.paths.database()).unwrap();
    assert_eq!(
        peer_db
            .query_row(
                "SELECT COUNT(*) FROM ftn_messages WHERE ingress_link='peer'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        4
    );
    drop(peer_client);
    drop(peer_again);
    drop(restored_client);
    drop(restored_daemon);
}
#[test]
#[ignore = "requires an independently configured isolated BinkP peer"]
fn independent_peer_outbound_and_retained_listener() {
    if std::env::var("SPITFIRE_BINKP_PHASE").as_deref() == Ok("operator") {
        let root = PathBuf::from(std::env::var_os("SPITFIRE_BINKP_PEER_DIR").unwrap());
        let config = PathBuf::from(fs::read_to_string(root.join("config-path")).unwrap());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut client = connect(&config).await;
            let snapshot = client.configuration_snapshot().await.unwrap();
            let mut operators = snapshot.config.operators.clone();
            for identity in &mut operators.local_identities {
                let (LocalOperatorIdentity::Unix { capabilities, .. }
                | LocalOperatorIdentity::Windows { capabilities, .. }) = identity;
                for cap in LocalOperatorCapability::READ_ONLY {
                    if !capabilities.contains(&cap) {
                        capabilities.push(cap);
                    }
                }
            }
            let result = client
                .apply_configuration(
                    format!("{:032x}", rand::random::<u128>()),
                    configuration::ConfigurationCandidate {
                        expected: snapshot.version,
                        edits: vec![],
                        operators: Some(operators),
                        ftn: None,
                        binkp: None,
                    },
                )
                .await
                .unwrap();
            assert!(matches!(result, sf_bbs::ConfigurationResult::Saved { .. }));
        });
        return;
    }

    if std::env::var("SPITFIRE_BINKP_PHASE").as_deref() == Ok("inbound") {
        let root = PathBuf::from(std::env::var_os("SPITFIRE_BINKP_PEER_DIR").unwrap());
        let path = PathBuf::from(fs::read_to_string(root.join("database-path")).unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        let db = RuntimeDatabase::open(&path).unwrap();
        let mut ids = vec![];
        for alias in ["Recipient", "Other"] {
            let id: i64 = conn
                .query_row(
                    "SELECT caller_id FROM callers WHERE display_name=?1",
                    [alias],
                    |r| r.get(0),
                )
                .unwrap();
            ids.push(MessageActor::new(
                CallerId::new(id).unwrap(),
                SecurityLevel::new(50).unwrap(),
            ));
        }
        let mail:i64=conn.query_row("SELECT m.message_id FROM ftn_messages f JOIN messages m USING(message_id) WHERE ingress_link='peer' AND m.container_kind<>'conference'",[],|r|r.get(0)).unwrap();
        let mail = MessageId::new(mail).unwrap();
        let private = db.read_ftn_mail(ids[0], mail).unwrap();
        assert!(private.body.contains("Independent SBBSecho NetMail body."));
        assert!(!private.body.contains('\u{1}'));
        assert!(db.read_ftn_mail(ids[1], mail).is_err());
        let (conference,number):(i64,u32)=conn.query_row("SELECT m.conference_id,m.message_number FROM ftn_messages f JOIN messages m USING(message_id) WHERE ingress_link='peer' AND m.container_kind='conference'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let echo = db
            .message(
                ids[0],
                ConferenceId::new(conference).unwrap(),
                u64::from(number),
            )
            .unwrap();
        assert!(String::from_utf8(echo.body)
            .unwrap()
            .contains("Independent SBBSecho EchoMail body."));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM ftn_messages WHERE ingress_link='peer'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            2
        );
        assert!(db
            .binkp_health()
            .unwrap()
            .iter()
            .all(|h| !h.active && h.last_error.is_none()));
        let secret = fs::read_to_string(root.join("credential")).unwrap();
        for table in [
            "operator_control_audit",
            "operational_events",
            "binkp_link_health",
            "network_quarantine",
        ] {
            let mut statement = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let columns = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                for n in 0..columns {
                    if let Ok(value) = row.get::<_, String>(n) {
                        assert!(!value.contains(&secret));
                        assert!(!value.contains("Independent SBBSecho NetMail body."));
                    }
                }
            }
        }
        return;
    }

    let evidence = PathBuf::from(
        std::env::var_os("SPITFIRE_BINKP_PEER_DIR").expect("private acceptance directory"),
    );
    let temp = tempfile::tempdir().unwrap();
    let a = board_in_domain(&temp.path().join("board"), 1, 34555, 34554, "n4test");
    author(&a, "10:100/2.9@n4test");
    let daemon = start(&a.config);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut client = rt.block_on(connect(&a.config));
    let secret = fs::read_to_string(evidence.join("credential")).unwrap();
    rt.block_on(credential(&mut client, &secret));
    rt.block_on(poll(&mut client, false));
    let queue = rt.block_on(client.ftn_queue(None)).unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|q| q.state == "accepted"));
    fs::write(
        evidence.join("config-path"),
        a.config.to_string_lossy().as_bytes(),
    )
    .unwrap();
    fs::write(
        evidence.join("database-path"),
        a.paths.database().to_string_lossy().as_bytes(),
    )
    .unwrap();
    drop(client);
    drop(daemon);
    let _ = temp.keep();
}
