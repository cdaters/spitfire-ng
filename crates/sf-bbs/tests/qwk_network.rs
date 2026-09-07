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

//! Disposable real daemon/IPC/manual-artifact acceptance. Optional independent
//! peer packets stay outside the repository and never become synthetic fixtures.
use sf_bbs::{NetworkAction as Action, NetworkResult as ResultValue, OperatorClient};
use sf_core::qwk_network::{Link, Mapping, PartnerRole, Profile};
use sf_core::*;
use std::{
    fs,
    process::{Child, Command, Stdio},
    thread,
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
#[test]
fn manual_network_journey_through_real_daemon() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("board");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Synthetic N2 Board", "Sysop", "SYSOP", 2);
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
        LocalOperatorCapability::ChangeSensitiveConfiguration,
        LocalOperatorCapability::ReadConfiguration,
        LocalOperatorCapability::BoardStatistics,
    ];
    plan.config.operators.local_identities =
        vec![if let Some(uid) = principal.strip_prefix("unix-uid:") {
            LocalOperatorIdentity::Unix {
                uid: uid.parse().unwrap(),
                label: Some("Synthetic N2 Operator".into()),
                capabilities,
            }
        } else {
            LocalOperatorIdentity::Windows {
                sid: principal.strip_prefix("windows-sid:").unwrap().into(),
                label: Some("Synthetic N2 Operator".into()),
                capabilities,
            }
        }];
    plan.config.transports = vec![TransportConfig {
        name: Some("isolated-n2".into()),
        enabled: true,
        adapter: TransportAdapterConfig::Raw {
            listen: address,
            terminal: NetworkTerminalDefaults {
                ansi: false,
                ..Default::default()
            },
        },
    }];
    let setup = sf_bbs::setup_board(&root, &plan, b"synthetic-sysop-password").unwrap();
    let paths = LogicalPaths::resolve(&root, &plan.config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let hash = CredentialHasher::new(&plan.config.caller.password)
        .unwrap()
        .hash(b"synthetic-caller-password")
        .unwrap();
    let caller = db
        .create_caller(
            b"Native Author",
            &hash,
            SecurityLevel::new(10).unwrap(),
            CallerState::Active,
            false,
            1_788_627_600,
        )
        .unwrap();
    let actor = MessageActor::new(caller.id, SecurityLevel::new(50).unwrap());
    let mut mappings = Vec::new();
    for (number, wire, area) in [(2, 2001, "general"), (3, 2006, "programming")] {
        let c = db
            .ensure_conference(&ConferenceDefinition {
                posting_identity: None,
                number,
                name: area.into(),
                description: "Isolated synthetic N2 area".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![],
            })
            .unwrap();
        mappings.push(Mapping {
            posting_identity: Default::default(),
            wire_conference: wire,
            area: area.into(),
            conference_id: c.id.get(),
            enabled: true,
            inbound: true,
            outbound: true,
            version: 1,
        });
    }
    drop(db);
    let mut daemon = start(&setup.config_path, &temp.path().join("daemon.log"));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut client = rt.block_on(connect(&setup.config_path));
    let mut link = Link {
        posting_identity: Default::default(),
        id: "peer".into(),
        network: "controlled".into(),
        local_id: "LOCAL".into(),
        remote_id: "PEER".into(),
        name: "Controlled Peer".into(),
        profile: Profile::DoveHeaders,
        role: PartnerRole::Hub,
        enabled: true,
        inbound: true,
        outbound: true,
        version: 1,
    };
    let configured = rt.block_on(act(
        &mut client,
        Action::Configure {
            link: link.clone(),
            mappings: mappings.clone(),
            expected: 0,
        },
    ));
    assert!(
        matches!(configured, ResultValue::Configured),
        "configuration: {configured:?}"
    );
    let mut invalid_mappings = mappings.clone();
    invalid_mappings.push(invalid_mappings[0].clone());
    let mut invalid_link = link.clone();
    invalid_link.version = 2;
    for m in &mut invalid_mappings {
        m.version = 2;
    }
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Configure {
                link: invalid_link,
                mappings: invalid_mappings,
                expected: 1
            }
        )),
        ResultValue::Rejected { .. }
    ));
    let mail_policy = sf_core::qwk_network::MailPolicy {
        link: "peer".into(),
        enabled: true,
        inbound: true,
        outbound: true,
        transit: true,
        version: 1,
        aliases: vec![sf_core::qwk_network::MailboxAlias {
            alias: "Native Author".into(),
            caller_id: caller.id.get(),
        }],
        destinations: vec!["PEER".into()],
    };
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::ConfigureMail {
                policy: mail_policy.clone(),
                expected: 0
            }
        )),
        ResultValue::Configured
    ));
    let mut second = link.clone();
    second.id = "second".into();
    second.remote_id = "SECOND".into();
    second.profile = Profile::QwkHeaders;
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Configure {
                link: second,
                mappings: mappings.clone(),
                expected: 0
            }
        )),
        ResultValue::Configured
    ));
    let mut second_policy = mail_policy.clone();
    second_policy.link = "second".into();
    second_policy.destinations = vec!["SECOND".into()];
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::ConfigureMail {
                policy: second_policy,
                expected: 0
            }
        )),
        ResultValue::Configured
    ));
    // Bounded authenticated native service hook; the operator cannot supply an author.
    let mut native = RuntimeDatabase::open(paths.database()).unwrap();
    native.bind_posting_identity_configuration(&RuntimeConfig::load(&setup.config_path).unwrap());
    for mapping in &mappings {
        native
            .post(
                actor,
                NewMessage {
                    identity_preview: None,
                    conference_id: ConferenceId::new(mapping.conference_id).unwrap(),
                    recipient_caller_id: None,
                    recipient_name: "All Callers".into(),
                    subject: format!("SPITFIRE N2 {}", mapping.area).into_bytes(),
                    body: [
                        b"Independent QWK networking acceptance: caf\x82 \xb3 \xdb\r\n".to_vec(),
                        format!("Synthetic run {:032x}\r\n", rand::random::<u128>()).into_bytes(),
                    ]
                    .concat(),
                    created_at: 1_788_627_600,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();
    }
    let private_sent = native
        .send_qwk_mail(
            actor,
            &sf_core::qwk_network::NewNetworkMail {
                identity_preview: None,
                network: "controlled".into(),
                destination: "PEER".into(),
                recipient: "Peer Recipient".into(),
                subject: b"Native private acceptance".to_vec(),
                body: format!(
                    "Private native sentinel {:032x}\r\n",
                    rand::random::<u128>()
                )
                .into_bytes(),
                reply_to: None,
            },
            1788627600,
        )
        .unwrap();
    assert_eq!(native.qwk_mailbox(actor, None).unwrap(), vec![private_sent]);
    drop(native);
    let replay_id = format!("{:032x}", rand::random::<u128>());
    let replay_action = Action::Build {
        link: link.id.clone(),
        expected: 1,
    };
    assert!(matches!(
        rt.block_on(client.qwk_network_action(replay_id.clone(), replay_action.clone()))
            .unwrap(),
        ResultValue::Built { .. }
    ));
    assert!(matches!(
        rt.block_on(client.qwk_network_action(replay_id, replay_action))
            .unwrap(),
        ResultValue::Replayed { .. }
    ));
    let artifact = match rt.block_on(act(
        &mut client,
        Action::Build {
            link: link.id.clone(),
            expected: 1,
        },
    )) {
        ResultValue::Built { artifact: Some(a) } => a,
        v => panic!("build: {v:?}"),
    };
    let status = rt.block_on(client.qwk_network_status()).unwrap();
    assert_eq!(status[0].queue.len(), 3);
    assert!(status[0].queue.iter().all(|q| q.state == "ready"));
    let packet = fs::read(
        paths
            .get(LogicalPath::System)
            .join("network-artifacts")
            .join(&artifact),
    )
    .unwrap();
    let decoded =
        sf_net::qwk_network::decode(&sf_net::qwk::inspect(&packet).unwrap(), Some("PEER")).unwrap();
    assert_eq!(decoded.len(), 3);
    assert_eq!(
        decoded
            .iter()
            .filter(|m| m.message.private && m.message.conference == 0)
            .count(),
        1
    );
    if let Some(directory) = std::env::var_os("SFNG_N2_PEER_DIR") {
        let directory = std::path::PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("PEER.REP"), &packet).unwrap();
        fs::write(directory.join("outbound.ready"), b"ready").unwrap();
        if std::env::var_os("SFNG_N2_WAIT_FOR_PEER").is_some() {
            let until = Instant::now() + Duration::from_secs(300);
            while !directory.join("return.ready").exists() {
                assert!(Instant::now() < until, "controlled peer response timed out");
                thread::sleep(Duration::from_millis(250));
            }
        }
        drop(client);
        client = rt.block_on(connect(&setup.config_path));
        if directory.join("PEER.QWK").exists() {
            let inbound = fs::read(directory.join("PEER.QWK")).unwrap();
            let inspected = sf_net::qwk::inspect(&inbound).expect("independent ZIP inspect");
            let peer_messages =
                sf_net::qwk_network::decode(&inspected, None).expect("independent network decode");
            fs::write(
                paths
                    .get(LogicalPath::System)
                    .join("qwk-handoff/peer/inbound.packet"),
                inbound,
            )
            .unwrap();
            let first = rt.block_on(act(
                &mut client,
                Action::Ingest {
                    link: link.id.clone(),
                    expected: 1,
                },
            ));
            assert!(
                matches!(first,ResultValue::Imported{ref summary} if summary.imported>=2&&summary.quarantined==0),
                "independent import: {first:?}"
            );
            let second = rt.block_on(act(
                &mut client,
                Action::Ingest {
                    link: link.id.clone(),
                    expected: 1,
                },
            ));
            assert!(
                matches!(second,ResultValue::Imported{summary} if summary.imported==0&&summary.duplicates>=2)
            );
            let db = RuntimeDatabase::open_read_only(paths.database()).unwrap();
            let mailbox = db.qwk_mailbox(actor, None).unwrap();
            assert_eq!(mailbox.len(), 2);
            let private_reply = db.read_qwk_mail(actor, *mailbox.last().unwrap()).unwrap();
            assert_eq!(private_reply.author, "Peer Recipient");
            assert_eq!(private_reply.origin, "PEER");
            assert_eq!(private_reply.parent, Some(private_sent));
            assert!(private_reply
                .body
                .starts_with(b"Independent private caf\x82"));
            let unauthorized =
                MessageActor::new(CallerId::new(1).unwrap(), SecurityLevel::new(999).unwrap());
            assert!(db
                .read_qwk_mail(unauthorized, *mailbox.last().unwrap())
                .is_err());
            let imported = db
                .messages(actor, ConferenceId::new(mappings[0].conference_id).unwrap())
                .unwrap();
            assert!(imported.iter().any(|m| m.author_caller_id.is_none()));
            for mapping in &mappings {
                let conference = ConferenceId::new(mapping.conference_id).unwrap();
                let summaries = db.messages(actor, conference).unwrap();
                let external = summaries
                    .iter()
                    .filter(|m| m.origin == sf_core::message::MessageOrigin::ExternalNetwork)
                    .collect::<Vec<_>>();
                assert!(!external.is_empty());
                for summary in &external {
                    let message = db.message(actor, conference, summary.number).unwrap();
                    assert_eq!(message.author_name, "Peer Author");
                    assert!(message.body.starts_with(b"Peer CP437: caf\x82 \xb3 \xdb\n"));
                    assert!(peer_messages
                        .iter()
                        .any(|m| m.message.body == message.body
                            && m.message.subject == message.subject));
                    assert_eq!(message.created_at, 1788627900);
                }
                if std::env::var_os("SFNG_N2_WAIT_FOR_PEER").is_some() {
                    assert!(
                        external.iter().any(|s| db
                            .message(actor, conference, s.number)
                            .unwrap()
                            .parent_message_id
                            .is_some()),
                        "independent reply must bind this board's exported parent"
                    );
                }
            }
        }
    }
    let inbound_path = paths
        .get(LogicalPath::System)
        .join("qwk-handoff/peer/inbound.packet");
    fs::write(&inbound_path, b"malformed packet").unwrap();
    assert!(
        matches!(rt.block_on(act(&mut client,Action::Ingest{link:"peer".into(),expected:1})),ResultValue::Imported{summary} if summary.quarantined==1)
    );
    fs::write(&inbound_path, vec![0; sf_net::qwk::MAX_ARCHIVE + 1]).unwrap();
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Ingest {
                link: "peer".into(),
                expected: 1
            }
        )),
        ResultValue::Rejected { .. }
    ));
    assert!(fs::metadata(&inbound_path).unwrap().len() > sf_net::qwk::MAX_ARCHIVE as u64);
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Ingest {
                link: "unknown".into(),
                expected: 1
            }
        )),
        ResultValue::Rejected { .. }
    ));
    assert!(!paths
        .get(LogicalPath::System)
        .join("qwk-handoff/unknown")
        .exists());
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Handoff {
                link: link.id.clone(),
                expected: 1,
                artifact: artifact.clone(),
                accepted: false
            }
        )),
        ResultValue::Updated
    ));
    drop(client);
    drop(daemon);
    thread::sleep(Duration::from_millis(40));
    daemon = start(&setup.config_path, &temp.path().join("restart.log"));
    client = rt.block_on(connect(&setup.config_path));
    let status = rt.block_on(client.qwk_network_status()).unwrap();
    assert!(status[0].queue.iter().all(|q| q.state == "retry"));
    for q in &status[0].queue {
        assert!(matches!(
            rt.block_on(act(
                &mut client,
                Action::Retry {
                    queue: q.id.clone(),
                    expected: q.version
                }
            )),
            ResultValue::Updated
        ));
    }
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Handoff {
                link: link.id.clone(),
                expected: 1,
                artifact,
                accepted: true
            }
        )),
        ResultValue::Updated
    ));
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Build {
                link: link.id.clone(),
                expected: 1
            }
        )),
        ResultValue::Built { artifact: None }
    ));
    let second_artifact = match rt.block_on(act(
        &mut client,
        Action::Build {
            link: "second".into(),
            expected: 1,
        },
    )) {
        ResultValue::Built { artifact: Some(a) } => a,
        other => panic!("second build: {other:?}"),
    };
    let second_bytes = fs::read(
        paths
            .get(LogicalPath::System)
            .join("network-artifacts")
            .join(second_artifact),
    )
    .unwrap();
    let second_members = sf_net::qwk_network::decode(
        &sf_net::qwk::inspect(&second_bytes).unwrap(),
        Some("SECOND"),
    )
    .unwrap();
    if std::env::var_os("SFNG_N2_WAIT_FOR_PEER").is_some() {
        let transit = second_members
            .iter()
            .filter(|m| m.message.private)
            .collect::<Vec<_>>();
        assert_eq!(transit.len(), 1);
        assert_eq!(transit[0].message.to, b"Transit Recipient");
        assert_eq!(transit[0].metadata.path, vec!["PEER"]);
        assert!(transit[0]
            .message
            .body
            .starts_with(b"Independent transit sentinel"));
    }

    let mut native = RuntimeDatabase::open(paths.database()).unwrap();
    let queued_private = native
        .send_qwk_mail(
            actor,
            &sf_core::qwk_network::NewNetworkMail {
                identity_preview: None,
                network: "controlled".into(),
                destination: "SECOND".into(),
                recipient: "Other Recipient".into(),
                subject: b"Queued private restore".to_vec(),
                body: b"Private restore sentinel\r\n".to_vec(),
                reply_to: None,
            },
            1788628000,
        )
        .unwrap();
    drop(native);
    link.enabled = false;
    link.version = 2;
    for m in &mut mappings {
        m.version = 2;
    }
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Configure {
                link: link.clone(),
                mappings,
                expected: 1
            }
        )),
        ResultValue::Configured
    ));
    assert!(matches!(
        rt.block_on(act(
            &mut client,
            Action::Build {
                link: link.id,
                expected: 2
            }
        )),
        ResultValue::Rejected { .. }
    ));
    assert!(rt.block_on(client.board_status()).is_ok());
    assert!(rt.block_on(client.configuration_snapshot()).is_ok());
    drop(client);
    drop(daemon);
    thread::sleep(Duration::from_millis(40));
    let backup = temp.path().join("snapshot");
    sf_bbs::backup_board(&setup.config_path, &backup).unwrap();
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let config = RuntimeConfig::load(&restored.join("spitfire.toml")).unwrap();
    let restored_paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(restored_paths.database()).unwrap();
    let states = db.qwk_network_status().unwrap();
    assert!(states
        .iter()
        .find(|s| s.link.id == "second")
        .unwrap()
        .queue
        .iter()
        .all(|q| q.state == "held"));
    db.validate_current_snapshot().unwrap();
    assert_eq!(
        db.read_qwk_mail(actor, queued_private).unwrap().body,
        b"Private restore sentinel\r\n"
    );
    assert_eq!(db.qwk_mail_policy("peer").unwrap(), mail_policy);
    assert!(!restored_paths
        .get(LogicalPath::System)
        .join("qwk-handoff/peer/inbound.packet")
        .exists());
    if let Some(directory) = std::env::var_os("SFNG_N2_PEER_DIR") {
        let packet = std::path::PathBuf::from(directory).join("PEER.QWK");
        if packet.exists() {
            let state = states.iter().find(|s| s.link.id == "peer").unwrap();
            let mut link = state.link.clone();
            link.enabled = true;
            link.version += 1;
            let mut maps = state.mappings.clone();
            for m in &mut maps {
                m.version = link.version;
            }
            db.configure_qwk_link(
                "synthetic-restore-operator",
                &link,
                &maps,
                state.link.version,
                1788629000,
            )
            .unwrap();
            let store =
                sf_bbs::DiskArtifactStore::new(restored_paths.get(LogicalPath::System)).unwrap();
            let result = db
                .ingest_qwk_network(
                    &store,
                    "peer",
                    link.version,
                    &fs::read(packet).unwrap(),
                    1788629000,
                )
                .unwrap();
            assert_eq!(result.imported, 0);
            assert!(result.duplicates >= 2);
        }
    }
    for log in [
        "daemon.log",
        "daemon.errors",
        "restart.log",
        "restart.errors",
    ] {
        let bytes = fs::read(temp.path().join(log)).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        for private in [
            "Private native sentinel",
            "Private peer sentinel",
            "Private restore sentinel",
            "Independent transit sentinel",
            "Independent private reply",
        ] {
            assert!(
                !text.contains(private),
                "private content in daemon diagnostics"
            );
        }
    }
}
