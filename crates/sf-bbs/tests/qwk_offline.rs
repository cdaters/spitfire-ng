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

//! Disposable subprocess daemon acceptance through real RAW TCP / YMODEM.
use sf_core::*;
use sf_net::qwk;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct AcceptanceLog(std::path::PathBuf);
impl Drop for AcceptanceLog {
    fn drop(&mut self) {
        if thread::panicking() {
            eprintln!("{}", fs::read_to_string(&self.0).unwrap_or_default());
        }
    }
}
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Peer(TcpStream);
impl Peer {
    fn until(&mut self, needle: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        while !data.ends_with(needle) {
            let mut b = [0];
            let n = self.0.read(&mut b).unwrap_or_else(|e| {
                panic!(
                    "waiting for {:?}: {e}; transcript {:?}",
                    String::from_utf8_lossy(needle),
                    String::from_utf8_lossy(&data)
                )
            });
            assert_eq!(n, 1, "peer closed: {:?}", String::from_utf8_lossy(&data));
            data.push(b[0]);
            if data.ends_with(b"to continue?  ") {
                self.0.write_all(b"N\r").unwrap();
            }
            assert!(data.len() < 128 * 1024);
        }
        data
    }
    fn send(&mut self, b: &[u8]) {
        self.0.write_all(b).unwrap();
    }
    fn login(&mut self, name: &str) {
        self.send(format!("N\r{name}\rsynthetic-qwk-password\r").as_bytes());
        self.until(b"MAIN MENU - Selection? ");
        self.send(b"M\r");
        self.until(b"MESSAGE MENU - Selection? ");
        self.send(b"L\r");
        self.until(b"Help: ");
    }
    fn download(&mut self) -> Vec<u8> {
        self.send(b"D\rN\rA\r");
        self.until(b"Protocol [1-9,T; Q cancels]: ");
        self.send(b"5\r");
        let files = receive_binary_files(
            self,
            TransferProtocol::YmodemBatch,
            "TEST.QWK",
            qwk::MAX_ARCHIVE as u64,
            1,
        )
        .unwrap();
        self.until(b"[y/N]: ");
        self.send(b"Y\r");
        self.until(b"Help: ");
        assert_eq!(files.len(), 1);
        files.into_iter().next().unwrap().bytes
    }
    fn upload(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.send(b"U\rR\r");
        self.until(b"Protocol [1-9,T; Q cancels]: ");
        self.send(b"5\r");
        send_binary_files(
            self,
            TransferProtocol::YmodemBatch,
            &[ProtocolFile {
                name: "TEST.REP".into(),
                bytes: bytes.to_vec(),
                modified_unix: None,
            }],
        )
        .unwrap();
        self.until(b"Help: ")
    }
}
impl Terminal for Peer {
    fn info(&self) -> TerminalInfo {
        TerminalInfo::in_memory()
    }
    fn write_all(&mut self, b: &[u8]) -> Result<(), TerminalError> {
        Ok(self.0.write_all(b)?)
    }
    fn read_line(&mut self, _: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        unreachable!()
    }
    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        Ok(())
    }
    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        Ok(())
    }
    fn read_binary(&mut self, b: &mut [u8], timeout: Duration) -> Result<usize, TerminalError> {
        self.0.set_read_timeout(Some(timeout))?;
        Ok(self.0.read(b)?)
    }
    fn write_binary(&mut self, b: &[u8]) -> Result<(), TerminalError> {
        Ok(self.0.write_all(b)?)
    }
}
fn connect(address: std::net::SocketAddr) -> Peer {
    let stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    Peer(stream)
}
fn await_failed_transfer(database: &std::path::Path) {
    let conn = rusqlite::Connection::open(database).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let active:i64=conn.query_row("SELECT COUNT(*) FROM transfer_records WHERE purpose='message-packet' AND state='transferring'",[],|r|r.get(0)).unwrap();
        if active == 0 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "incomplete transfer was not finalized"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn start(
    config: &std::path::Path,
    addr: std::net::SocketAddr,
    log: &std::path::Path,
) -> (Daemon, Peer) {
    let child = Command::new(env!("CARGO_BIN_EXE_spitfire"))
        .arg("run")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(log).unwrap()))
        .stderr(Stdio::from(
            fs::File::create(log.with_extension("errors")).unwrap(),
        ))
        .spawn()
        .unwrap();
    let mut daemon = Daemon(child);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(s) = TcpStream::connect(addr) {
            s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
            return (daemon, Peer(s));
        }
        assert!(
            daemon.0.try_wait().unwrap().is_none(),
            "daemon startup failed: {}",
            fs::read_to_string(log.with_extension("errors")).unwrap()
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn caller_qwk_journey_through_independent_daemon() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("board");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut plan =
        sf_bbs::SetupPlan::stock_defaults("Synthetic QWK Acceptance", "Sysop", "SYSOP", 4);
    plan.config.caller.qwk_board_id = Some("TEST".into());
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    plan.config.caller.post_login_journey = PostLoginJourney::None;
    plan.config.transports = vec![TransportConfig {
        name: Some("qwk-loopback".into()),
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
    let config = RuntimeConfig::load(&setup.config_path).unwrap();
    let paths = LogicalPaths::resolve(&root, &config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let hash = CredentialHasher::new(&plan.config.caller.password)
        .unwrap()
        .hash(b"synthetic-qwk-password")
        .unwrap();
    let mut actors = Vec::new();
    for name in [b"ALICE".as_slice(), b"BOB", b"OTHER"] {
        let caller = db
            .create_caller(
                name,
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1_788_627_600,
            )
            .unwrap();
        actors.push(MessageActor::new(
            caller.id,
            SecurityLevel::new(50).unwrap(),
        ));
    }
    let c1 = db.conference(actors[0], 1).unwrap();
    let c2 = db
        .ensure_conference(&ConferenceDefinition {
            number: 2,
            name: "Second".into(),
            description: "Synthetic conference".into(),
            access_mode: ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(5).unwrap(),
            post_security: SecurityLevel::new(5).unwrap(),
            public_only: false,
            caller_deletion_enabled: true,
            maximum_lines: 99,
            privileged_security_levels: vec![],
        })
        .unwrap();
    for actor in &actors {
        db.replace_queue(*actor, &[1, 2]).unwrap();
    }
    let mut base = NewMessage {
        conference_id: c1.id,
        recipient_caller_id: None,
        recipient_name: "All Callers".into(),
        subject: b"Public CP437".to_vec(),
        body: b"Synthetic caf\x82 \xb3 \xdb\r\n".to_vec(),
        created_at: 1_788_627_600,
        parent_message_id: None,
        visibility: MessageVisibility::Public,
        kind: MessageKind::Standard,
    };
    let first = db.post(actors[1], base.clone()).unwrap();
    base.parent_message_id = Some(first.id);
    base.subject = b"A native reply with a long QWKE subject field".to_vec();
    db.post(actors[1], base.clone()).unwrap();
    base.parent_message_id = None;
    base.conference_id = c2.id;
    base.subject = b"Private permitted".to_vec();
    base.visibility = MessageVisibility::Private;
    base.recipient_caller_id = Some(actors[0].caller_id());
    base.recipient_name = "ALICE".into();
    db.post(actors[1], base.clone()).unwrap();
    base.recipient_caller_id = Some(actors[2].caller_id());
    base.recipient_name = "OTHER".into();
    base.subject = b"PRIVATE EXCLUSION SENTINEL".to_vec();
    db.post(actors[1], base).unwrap();
    drop(db);
    let (daemon, mut peer) = start(&setup.config_path, address, &temp.path().join("daemon.log"));
    let _log = AcceptanceLog(temp.path().join("daemon.log"));
    peer.login("ALICE");
    // Lose the transport after the server starts sending a real YMODEM header.
    peer.send(b"D\rN\rA\r");
    peer.until(b"Protocol [1-9,T; Q cancels]: ");
    peer.send(b"5\rC");
    peer.until(&[1]);
    peer.0.shutdown(std::net::Shutdown::Both).unwrap();
    drop(peer);
    await_failed_transfer(paths.database());
    let before = RuntimeDatabase::open(paths.database()).unwrap();
    assert_eq!(before.last_read(actors[0], c1.id).unwrap(), 0);
    drop(before);
    let mut bob = connect(address);
    bob.login("BOB");
    let mut peer = connect(address);
    peer.login("ALICE");
    let (packet, bob_packet) = thread::scope(|scope| {
        let b = scope.spawn(|| bob.download());
        let a = peer.download();
        (a, b.join().unwrap())
    });
    assert_ne!(packet, bob_packet);
    bob.send(b"Q\rQ\rG\r");
    drop(bob);
    peer.send(b"D\rN\rA\r");
    assert!(String::from_utf8_lossy(&peer.until(b"Help: "))
        .contains("No messages match this selection"));
    let archive = qwk::inspect(&packet).unwrap();
    let messages = qwk::decode_records(
        &archive.members["MESSAGES.DAT"],
        None,
        qwk::Profile::ExtendedCp437,
    )
    .unwrap();
    assert!(messages.iter().any(|m| m.message.private));
    assert!(!messages
        .iter()
        .any(|m| m.message.subject == b"PRIVATE EXCLUSION SENTINEL"));
    assert!(messages.iter().any(|m| m.message.body.contains(&0x82)));
    if let Some(dir) = std::env::var_os("SFNG_N1_PEER_DIR") {
        fs::create_dir_all(&dir).unwrap();
        fs::write(std::path::Path::new(&dir).join("TEST.QWK"), &packet).unwrap();
    }
    let mut reply = messages[0].message.clone();
    reply.number = 1;
    reply.reference = first.number as u32;
    reply.private = false;
    reply.to = b"ALL".to_vec();
    reply.from = b"SYSOP SPOOF".to_vec();
    reply.subject = b"Offline native reply".to_vec();
    reply.body = b"Synthetic reply from an offline packet\n".to_vec();
    let mut denied = reply.clone();
    denied.conference = 65000;
    denied.subject = b"Forbidden area".to_vec();
    let (records, _) =
        qwk::encode_records(&[reply, denied], Some("TEST"), qwk::Profile::ExtendedCp437).unwrap();
    let rep = qwk::archive(&BTreeMap::from([("TEST.MSG".into(), records)])).unwrap();
    let summary = peer.upload(&rep);
    let text = String::from_utf8_lossy(&summary);
    assert!(text.contains("Replies imported: 1"), "{text}");
    assert!(text.contains("Rejected: 1"), "{text}");
    let summary = peer.upload(&rep);
    assert!(String::from_utf8_lossy(&summary).contains("Duplicates skipped: 1"));
    if let Some(file) = std::env::var_os("SFNG_N1_PEER_REPLY") {
        let external = fs::read(file).unwrap();
        let response = peer.upload(&external);
        assert!(
            String::from_utf8_lossy(&response).contains("Replies imported: 1"),
            "{:?}",
            String::from_utf8_lossy(&response)
        );
        assert!(String::from_utf8_lossy(&peer.upload(&external)).contains("Duplicates skipped: 1"));
    }
    let mut traversal = rep.clone();
    for start in 0..traversal.len().saturating_sub(7) {
        if &traversal[start..start + 8] == b"TEST.MSG" {
            traversal[start..start + 8].copy_from_slice(b"../X.MSG");
        }
    }
    assert!(String::from_utf8_lossy(&peer.upload(&traversal)).contains("malformed"));
    assert!(String::from_utf8_lossy(&peer.upload(b"not a packet")).contains("malformed"));
    let mut oversized = rep.clone();
    let central = oversized
        .windows(4)
        .position(|b| b == b"PK\x01\x02")
        .unwrap();
    oversized[central + 24..central + 28]
        .copy_from_slice(&((qwk::MAX_EXPANDED + 1) as u32).to_le_bytes());
    oversized[22..26].copy_from_slice(&((qwk::MAX_EXPANDED + 1) as u32).to_le_bytes());
    assert!(String::from_utf8_lossy(&peer.upload(&oversized)).contains("malformed"));
    let count_before = RuntimeDatabase::open(paths.database())
        .unwrap()
        .messages(actors[0], c1.id)
        .unwrap()
        .len();
    peer.send(b"U\rR\r");
    peer.until(b"Protocol [1-9,T; Q cancels]: ");
    peer.send(b"5\r");
    peer.until(b"C");
    peer.send(b"\x01\x00\xffTEST.REP\0incomplete");
    peer.0.shutdown(std::net::Shutdown::Both).unwrap();
    drop(peer);
    await_failed_transfer(paths.database());
    assert_eq!(
        RuntimeDatabase::open(paths.database())
            .unwrap()
            .messages(actors[0], c1.id)
            .unwrap()
            .len(),
        count_before
    );
    let mut peer = connect(address);
    peer.login("ALICE");
    let db = RuntimeDatabase::open(paths.database()).unwrap();
    let native = db.messages(actors[0], c1.id).unwrap();
    let imported = native
        .iter()
        .find(|m| m.subject == b"Offline native reply")
        .unwrap();
    let imported = db.message(actors[0], c1.id, imported.number).unwrap();
    assert_eq!(imported.author_caller_id, Some(actors[0].caller_id()));
    assert_eq!(imported.parent_message_id, Some(first.id));
    drop(db);
    // Hard-stop the independent daemon while an upload owns a transfer record.
    // This differs from socket loss: no worker finalizer can complete the row.
    peer.send(b"U\rR\r");
    peer.until(b"Protocol [1-9,T; Q cancels]: ");
    peer.send(b"5\r");
    peer.until(b"C");
    peer.send(b"\x01\x00\xffTEST.REP\0interrupted by daemon kill");
    drop(daemon);
    drop(peer);

    let (daemon, mut peer) = start(
        &setup.config_path,
        address,
        &temp.path().join("restart.log"),
    );
    peer.login("ALICE");
    assert!(String::from_utf8_lossy(&peer.upload(&rep)).contains("Duplicates skipped: 1"));
    peer.send(b"Q\rQ\rG\r");
    drop(peer);
    drop(daemon);
    let custody = RuntimeDatabase::open(paths.database()).unwrap();
    let store = sf_bbs::DiskArtifactStore::new(paths.get(LogicalPath::System)).unwrap();
    store.validate(&custody).unwrap();
    assert!(custody
        .network_artifact_inventory()
        .unwrap()
        .iter()
        .all(|(_, _, complete)| *complete));
    assert!(!root.join("X.MSG").exists());
    drop(custody);
    let backup = temp.path().join("backup");
    sf_bbs::backup_board(&setup.config_path, &backup).unwrap();
    let restored = temp.path().join("restored");
    sf_bbs::restore_board(&backup, &restored, false).unwrap();
    let restored_config = RuntimeConfig::load(&restored.join(sf_bbs::BOARD_CONFIG_FILE)).unwrap();
    let restored_paths =
        LogicalPaths::resolve(&restored, &restored_config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(restored_paths.database()).unwrap();
    let store = sf_bbs::DiskArtifactStore::new(restored_paths.get(LogicalPath::System)).unwrap();
    assert_eq!(
        db.import_offline_replies(
            actors[0],
            "TEST",
            &rep,
            &store,
            &sf_core::network::SubmissionIntent::Retry,
            1_788_627_601
        )
        .unwrap()
        .duplicates,
        1
    );
}
