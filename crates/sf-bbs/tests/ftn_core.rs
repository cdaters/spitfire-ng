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

//! Isolated FTN configuration, manual artifacts, directory activation and restart.
use sf_bbs::ftn::{Action as FtnAction, Result as FtnResult};
use sf_bbs::ConfigurationResult;
use sf_bbs::{NetworkAction as Action, NetworkResult as ResultValue, OperatorClient};
use sf_core::configuration::ConfigurationCandidate;
use sf_core::ftn;
use sf_core::*;
use std::{
    fs,
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
fn start(config: &std::path::Path, log: &std::path::Path) -> Daemon {
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_spitfire"))
            .arg("run")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::create(log).unwrap()))
            .stderr(Stdio::from(
                fs::File::create(log.with_extension("errors")).unwrap(),
            ))
            .spawn()
            .unwrap(),
    )
}
async fn connect(config: &std::path::Path) -> OperatorClient {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(mut c) = OperatorClient::connect(config).await {
            c.describe_operator_controls().await.unwrap();
            return c;
        }
        assert!(Instant::now() < until, "daemon attach timed out");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}
async fn act(client: &mut OperatorClient, action: Action) -> ResultValue {
    client
        .qwk_network_action(format!("{:032x}", rand::random::<u128>()), action)
        .await
        .unwrap()
}
async fn ftn_act(client: &mut OperatorClient, request: FtnAction) -> FtnResult {
    match act(client, Action::Ftn { request }).await {
        ResultValue::Ftn { response } => response,
        other => panic!("FTN operation failed: {other:?}"),
    }
}
#[test]
fn native_ftn_real_daemon_journey() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("board");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Synthetic N3 Board", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    let principal = sf_bbs::current_operator_identity().unwrap();
    let capabilities = vec![
        LocalOperatorCapability::NetworkStatus,
        LocalOperatorCapability::NetworkRun,
        LocalOperatorCapability::NetworkQueue,
        LocalOperatorCapability::NetworkDirectoryActivate,
        LocalOperatorCapability::ChangeSensitiveConfiguration,
        LocalOperatorCapability::ChangeOnlineConfiguration,
        LocalOperatorCapability::ReadConfiguration,
        LocalOperatorCapability::BoardStatistics,
    ];
    plan.config.operators.local_identities =
        vec![if let Some(uid) = principal.strip_prefix("unix-uid:") {
            LocalOperatorIdentity::Unix {
                uid: uid.parse().unwrap(),
                label: Some("Synthetic operator".into()),
                capabilities,
            }
        } else {
            LocalOperatorIdentity::Windows {
                sid: principal.strip_prefix("windows-sid:").unwrap().into(),
                label: Some("Synthetic operator".into()),
                capabilities,
            }
        }];
    plan.config.transports = vec![TransportConfig {
        name: Some("isolated-n3".into()),
        enabled: true,
        adapter: TransportAdapterConfig::Raw {
            listen: address,
            terminal: NetworkTerminalDefaults {
                ansi: false,
                ..Default::default()
            },
        },
    }];
    let policy:ftn::Policy=serde_json::from_value(serde_json::json!({
      "enabled":true,
      "akas":[{"id":"node","endpoint":{"domain":"synthetic","address":"10:100/1"},"enabled":true,"primary":true},{"id":"point","endpoint":{"domain":"synthetic","address":"10:100/1.3"},"enabled":true,"primary":false},{"id":"other","endpoint":{"domain":"othernet","address":"10:100/1"},"enabled":true,"primary":true}],
      "links":[{"id":"peer","remote":{"domain":"synthetic","address":"10:100/2"},"aka":"point","enabled":true,"inbound":true,"outbound":true,"transit":true,"profile":"type2-plus","charset":"utf8"}],
      "routes":[{"domain":"synthetic","target":{"kind":"default"},"link":"peer"}],
      "sources":[{"id":"nodes","domain":"synthetic","enabled":true,"format":"nodelist","charset":"ascii","default_zone":10,"priority":10,"cadence_days":7,"require_crc":false},{"id":"points","domain":"synthetic","enabled":true,"format":"boss","charset":"cp866","default_zone":10,"priority":0,"cadence_days":7,"require_crc":false}]
    })).unwrap();
    let setup = sf_bbs::setup_board(&root, &plan, b"synthetic-password").unwrap();
    let paths = LogicalPaths::resolve(&root, &plan.config.validate().unwrap()).unwrap();
    let config = root.join("spitfire.toml");
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let hash = CredentialHasher::new(&plan.config.caller.password)
        .unwrap()
        .hash(b"synthetic-caller-password")
        .unwrap();
    let caller = db
        .create_caller(
            b"Recipient",
            &hash,
            SecurityLevel::new(50).unwrap(),
            CallerState::Active,
            false,
            1788609600,
        )
        .unwrap();
    let actor = MessageActor::new(caller.id, SecurityLevel::new(50).unwrap());
    let mut mappings = vec![];
    for n in [2, 3] {
        let c = db
            .ensure_conference(&ConferenceDefinition {
                posting_identity: None,
                number: n,
                name: format!("Synthetic {n}"),
                description: "Synthetic FTN area".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![],
            })
            .unwrap();
        mappings.push(ftn::Mapping {
            posting_identity: Default::default(),
            domain: "synthetic".parse().unwrap(),
            area: format!("TEST{}", n - 1),
            conference_id: c.id.get(),
            aka: "node".into(),
            receive: true,
            send: true,
            origin: "Isolated Board".into(),
            links: vec!["peer".into()],
            version: 1,
        });
    }
    drop(db);
    let mut daemon = start(&config, &temp.path().join("daemon.log"));
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut client = rt.block_on(connect(&config));
    let snapshot = rt.block_on(client.configuration_snapshot()).unwrap();
    let saved = rt
        .block_on(client.apply_configuration(
            format!("{:032x}", rand::random::<u128>()),
            ConfigurationCandidate {
                expected: snapshot.version,
                edits: vec![],
                operators: None,
                ftn: Some(policy.clone()),
                binkp: None,
            },
        ))
        .unwrap();
    assert!(
        matches!(saved, ConfigurationResult::Saved { .. }),
        "{saved:?}"
    );
    for mapping in &mappings {
        assert!(matches!(
            rt.block_on(ftn_act(
                &mut client,
                FtnAction::Mapping {
                    mapping: mapping.clone(),
                    expected: 0
                }
            )),
            FtnResult::Updated
        ));
    }
    rt.block_on(ftn_act(
        &mut client,
        FtnAction::Alias {
            alias: ftn::MailboxAlias {
                aka: "point".into(),
                alias: "Recipient".into(),
                caller_id: caller.id.get(),
            },
        },
    ));
    let mut native = RuntimeDatabase::open(paths.database()).unwrap();
    native.bind_posting_identity_configuration(&RuntimeConfig::load(&config).unwrap());
    for mapping in &mappings {
        native
            .post(
                actor,
                NewMessage {
                    identity_preview: None,
                    conference_id: sf_core::ConferenceId::new(mapping.conference_id).unwrap(),
                    recipient_caller_id: None,
                    recipient_name: "All Callers".into(),
                    subject: b"Native daemon EchoMail".to_vec(),
                    body: b"Daemon EchoMail body.\r\n".to_vec(),
                    created_at: 1788609600,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();
    }
    drop(native);
    assert!(matches!(
        rt.block_on(ftn_act(&mut client, FtnAction::Scan)),
        FtnResult::Scanned { messages: 2 }
    ));
    let mut native = RuntimeDatabase::open(paths.database()).unwrap();
    native
        .send_ftn_mail(
            actor,
            &policy,
            &ftn::NewNetMail {
                identity_preview: None,
                aka: "point".into(),
                destination: "10:100/2.9@synthetic".parse().unwrap(),
                recipient: "Sysop".into(),
                subject: "Daemon point NetMail".into(),
                body: "Private daemon sentinel.".into(),
                reply_to: None,
            },
            1788609600,
        )
        .unwrap();
    drop(native);
    let queue = rt.block_on(client.ftn_queue(None)).unwrap();
    assert_eq!(queue.len(), 3);
    for q in &queue {
        assert!(matches!(
            rt.block_on(ftn_act(
                &mut client,
                FtnAction::Build {
                    queue: q.id.clone(),
                    expected: q.version
                }
            )),
            FtnResult::Built { .. }
        ));
    }
    for (source,bytes) in [("nodes",b";A Synthetic\r\nZone,10,Zone,Here,Sysop,-Unpublished-,300\r\nHost,100,Net,Here,Sysop,-Unpublished-,300\r\n,2,Peer,Here,Sysop,-Unpublished-,300,IBN,INA:peer.invalid\r\n".as_slice()),("points",b";A Synthetic\r\nBoss,10:100/2\r\n,9,Point,Here,Sysop,-Unpublished-,300\r\n".as_slice())] {
      rt.block_on(ftn_act(&mut client,FtnAction::PrepareDirectory{source:source.into()}));
      fs::write(paths.get(LogicalPath::System).join("ftn-handoff").join(source).join("inbound.packet"),bytes).unwrap();
      let generation=match rt.block_on(ftn_act(&mut client,FtnAction::DirectoryIngest{source:source.into(),date:"2026-09-05".into()})){FtnResult::Directory{generation}=>generation,_=>panic!("directory result")};
      assert_eq!(generation.state,"validated");
      rt.block_on(ftn_act(&mut client,FtnAction::DirectoryActivate{generation:generation.id,expected:0}));
    }
    assert_eq!(
        rt.block_on(client.ftn_status()).unwrap().active_generations,
        2
    );
    rt.block_on(ftn_act(
        &mut client,
        FtnAction::Prepare {
            link: "peer".into(),
        },
    ));
    let handoff = paths
        .get(LogicalPath::System)
        .join("ftn-handoff/peer/inbound.packet");
    fs::write(&handoff, b"malformed-packet").unwrap();
    assert!(
        matches!(rt.block_on(ftn_act(&mut client,FtnAction::Toss{link:"peer".into()})),FtnResult::Tossed{summary} if summary.quarantined==1)
    );
    if let Some(peer) = std::env::var_os("SPITFIRE_FTN_PEER_DIR") {
        for name in ["peer-netmail.pkt", "peer-echo.pkt"] {
            fs::copy(std::path::PathBuf::from(&peer).join(name), &handoff).unwrap();
            assert!(
                matches!(rt.block_on(ftn_act(&mut client,FtnAction::Toss{link:"peer".into()})),FtnResult::Tossed{summary} if summary.imported==1 && summary.quarantined==0)
            );
            assert!(
                matches!(rt.block_on(ftn_act(&mut client,FtnAction::Toss{link:"peer".into()})),FtnResult::Tossed{summary} if summary.duplicates==1 && summary.imported==0)
            );
        }
    }
    let before = rt.block_on(client.ftn_queue(None)).unwrap();
    drop(client);
    daemon.0.kill().unwrap();
    daemon.0.wait().unwrap();
    daemon = start(&config, &temp.path().join("restarted.log"));
    client = rt.block_on(connect(&config));
    assert_eq!(rt.block_on(client.ftn_queue(None)).unwrap(), before);
    assert_eq!(
        rt.block_on(client.ftn_status()).unwrap().active_generations,
        2
    );
    assert!(matches!(
        rt.block_on(ftn_act(&mut client, FtnAction::Scan)),
        FtnResult::Scanned { messages: 0 }
    ));
    drop(client);
    drop(daemon);
    let backup = temp.path().join("snapshot");
    sf_bbs::backup_board(&setup.config_path, &backup).unwrap();
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let restored_config = RuntimeConfig::load(&restored.join("spitfire.toml")).unwrap();
    assert_eq!(restored_config.ftn, policy);
    let restored_paths =
        LogicalPaths::resolve(&restored, &restored_config.validate().unwrap()).unwrap();
    let restored_db = RuntimeDatabase::open(restored_paths.database()).unwrap();
    restored_db.validate_current_snapshot().unwrap();
    assert_eq!(restored_db.ftn_status().unwrap().active_generations, 2);
    assert!(restored_db
        .ftn_queue(None)
        .unwrap()
        .iter()
        .all(|q| q.state == "held"));
    assert_eq!(restored_db.ftn_queue(None).unwrap().len(), before.len());
    assert!(!restored_paths
        .get(LogicalPath::System)
        .join("ftn-handoff/peer/inbound.packet")
        .exists());
    let conn = rusqlite::Connection::open(paths.database()).unwrap();
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM operator_control_audit WHERE CAST(operation AS TEXT) LIKE '%Private daemon sentinel%'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    if let Some(directory) = std::env::var_os("SPITFIRE_FTN_OPERATOR_DIR") {
        let directory = std::path::PathBuf::from(directory);
        fs::write(
            directory.join("policy.json"),
            serde_json::to_vec_pretty(&policy).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("config-path"),
            config.to_string_lossy().as_bytes(),
        )
        .unwrap();
        drop(conn);
        let _retained = temp.keep();
    }
}
