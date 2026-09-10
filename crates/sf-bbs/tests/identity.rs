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

//! Disposable native daemon acceptance. Only a loopback caller listener is opened;
//! the FTN peer is an isolated artifact destination with no configured transport.
use sf_core::*;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
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
fn start(config: &Path, log: &Path) -> Daemon {
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
fn caller(address: std::net::SocketAddr, commands: &[u8]) -> String {
    let until = Instant::now() + Duration::from_secs(20);
    let mut stream = loop {
        if let Ok(s) = TcpStream::connect(address) {
            break s;
        }
        assert!(Instant::now() < until, "isolated daemon did not start");
        thread::sleep(Duration::from_millis(30));
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    stream.write_all(commands).unwrap();
    let mut output = Vec::new();
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let mut buffer = [0u8; 4096];
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => output.extend_from_slice(&buffer[..n]),
            Err(error) => panic!(
                "caller read failed: {error}: {}",
                String::from_utf8_lossy(&output)
            ),
        };
        assert!(
            Instant::now() < end,
            "caller journey exceeded deadline: {}",
            String::from_utf8_lossy(&output)
        );
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn disposable_daemon_identity_post_profile_history_queue_restart_and_restore() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("board");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut plan =
        sf_bbs::SetupPlan::stock_defaults("Synthetic Identity Board", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    plan.config.caller.post_login_journey = PostLoginJourney::None;
    plan.config.transports = vec![TransportConfig {
        name: Some("identity-loopback".into()),
        enabled: true,
        adapter: TransportAdapterConfig::Raw {
            listen: address,
            terminal: NetworkTerminalDefaults {
                ansi: false,
                cp437: false,
                width: 100,
                height: 100,
            },
        },
    }];
    let policy:ftn::Policy=serde_json::from_value(serde_json::json!({"enabled":true,"akas":[{"id":"local","endpoint":{"domain":"identity-test","address":"10:100/1"},"enabled":true,"primary":true}],"links":[{"id":"peer","remote":{"domain":"identity-test","address":"10:100/2"},"aka":"local","enabled":true,"inbound":true,"outbound":true,"transit":false,"profile":"type2-plus","charset":"cp437"}],"routes":[],"sources":[]})).unwrap();
    plan.config.ftn = policy.clone();
    plan.conferences[0].posting_identity = Some(PostingIdentityPolicy::HandleAllowed);
    plan.conferences[0].public_only = true;
    plan.conferences[1].posting_identity = Some(PostingIdentityPolicy::RealNameRequired);
    plan.conferences[1].public_only = true;
    let mut mapped = plan.conferences[0].clone();
    mapped.number = 3;
    mapped.name = "Isolated FTN".into();
    plan.conferences.push(mapped);
    sf_bbs::setup_board(&root, &plan, b"synthetic sysop password").unwrap();
    let config_path = root.join(sf_bbs::BOARD_CONFIG_FILE);
    let config = RuntimeConfig::load(&config_path).unwrap();
    let paths = LogicalPaths::resolve(&root, &config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    db.bind_posting_identity_configuration(&config);
    let conference = db
        .all_conferences()
        .unwrap()
        .into_iter()
        .find(|c| c.number == 3)
        .unwrap();
    db.configure_ftn_mapping(
        &policy,
        "synthetic-operator",
        &ftn::Mapping {
            posting_identity: PostingIdentityPolicy::RealNameRequired,
            domain: "identity-test".parse().unwrap(),
            area: "IDENTITY".into(),
            conference_id: conference.id.get(),
            aka: "local".into(),
            receive: true,
            send: true,
            origin: "Synthetic Identity Board".into(),
            links: vec!["peer".into()],
            version: 1,
        },
        0,
        1,
    )
    .unwrap();
    drop(db);
    let daemon = start(&config_path, &temp.path().join("first.log"));
    let output=caller(address,b"Y\r\npixelwizard\r\nPixelWizard\r\nsynthetic caller password\r\nsynthetic caller password\r\nR\r\nCraig\r\nDaters\r\nM\r\nE\r\n\r\nHandle post\r\nSynthetic local body\r\n/S\r\nY\r\nC\r\n2\r\nE\r\n\r\nReal post\r\nSynthetic real-name body\r\n/S\r\nY\r\nC\r\n3\r\nE\r\n\r\nFTN post\r\nSynthetic isolated FTN body\r\n/S\r\nY\r\nG\r\n");
    assert!(output.contains("Posting as: PixelWizard"), "{output}");
    assert_eq!(
        output.matches("Posting as: Craig Daters").count(),
        4,
        "{output}"
    );
    assert_eq!(
        output.matches("was saved in Conference").count(),
        3,
        "{output}"
    );
    drop(daemon);
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    db.bind_posting_identity_configuration(&config);
    let account = db.caller_by_name(b"PixelWizard").unwrap().unwrap();
    assert_eq!(account.real_name, None);
    assert_eq!(
        account.profile.identity.real_name().as_deref(),
        Some("Craig Daters")
    );
    assert_eq!(db.scan_ftn(&policy, 3).unwrap(), 1);
    let queue = db.ftn_queue(None).unwrap().remove(0);
    drop(db);
    let daemon = start(&config_path, &temp.path().join("profile.log"));
    let edited = caller(
        address,
        b"N\r\npixelwizard\r\nsynthetic caller password\r\nR\r\nCraig\r\nD.\r\nG\r\n",
    );
    assert!(edited.contains("profile"));
    drop(daemon);
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    db.bind_posting_identity_configuration(&config);
    let account = db.caller_by_id(account.id).unwrap().unwrap();
    assert_eq!(
        account.profile.identity.real_name().as_deref(),
        Some("Craig D.")
    );
    db.update_caller_login_handle(
        account.id,
        account.state_version,
        account.login_identifier.as_bytes(),
        b"Darkmage",
        &config.caller,
        5,
    )
    .unwrap();
    let store = sf_bbs::DiskArtifactStore::new(paths.get(LogicalPath::System)).unwrap();
    let artifact = db
        .build_ftn(&store, &policy, &queue.id, queue.version, 6)
        .unwrap();
    let packet = fs::read(
        paths
            .get(LogicalPath::System)
            .join("network-artifacts")
            .join(&artifact),
    )
    .unwrap();
    assert_eq!(
        sf_net::ftn::Packet::decode(&packet, (10, 10))
            .unwrap()
            .messages[0]
            .from,
        b"Craig Daters"
    );
    drop(db);
    let daemon = start(&config_path, &temp.path().join("history.log"));
    let history=caller(address,b"N\r\npixelwizard\r\nsynthetic caller password\r\nM\r\nB\r\nC\r\n2\r\nB\r\nC\r\n3\r\nB\r\nG\r\n");
    assert!(history.contains("PixelWizard"), "{history}");
    assert!(history.contains("Craig Daters"), "{history}");
    assert!(!history.contains("Craig D."), "{history}");
    drop(daemon);
    let backup = temp.path().join("backup");
    sf_bbs::backup_board(&config_path, &backup).unwrap();
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let restored_config = RuntimeConfig::load(&restored.join(sf_bbs::BOARD_CONFIG_FILE)).unwrap();
    let restored_paths =
        LogicalPaths::resolve(&restored, &restored_config.validate().unwrap()).unwrap();
    let restored_db = RuntimeDatabase::open_read_only(restored_paths.database()).unwrap();
    let restored_account = restored_db.caller_by_id(account.id).unwrap().unwrap();
    assert_eq!(restored_account.display_name, "Darkmage");
    assert_eq!(restored_account.login_identifier, account.login_identifier);
    assert_eq!(
        restored_account.profile.identity.real_name().as_deref(),
        Some("Craig D.")
    );
    assert_eq!(
        fs::read(
            restored_paths
                .get(LogicalPath::System)
                .join("network-artifacts")
                .join(artifact)
        )
        .unwrap(),
        packet
    );
    let conn = rusqlite::Connection::open_with_flags(
        restored_paths.database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT sender FROM network_sender_snapshots WHERE queue_id=?1",
            [queue.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Craig Daters"
    );
    assert_eq!(
        restored_db.schema_version().unwrap(),
        sf_core::SCHEMA_VERSION
    );
}
