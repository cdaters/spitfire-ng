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

//! Real native four-daemon TLS journey; isolated synthetic certificates and traffic.
#![cfg(unix)]
use sf_core::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const NET: &str = "circuitnet-test";
// Process-heavy localhost campaigns share finite host socket/process resources.
// Keep independent board identities while avoiding competing campaign startups.
static CAMPAIGN: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[derive(Clone)]
struct Board {
    config: PathBuf,
    db: PathBuf,
    id: String,
    test: u16,
    tech: Option<u16>,
    port: u16,
    cert: PathBuf,
    key: PathBuf,
}
fn network() -> sf_core::circuitnet::NetworkId {
    sf_core::circuitnet::NetworkId::new(NET).unwrap()
}
fn command(b: &Board, action: &str, rest: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_spitfire"))
        .args(["circuitnet", b.config.to_str().unwrap(), action, NET])
        .args(rest)
        .output()
        .unwrap()
}
fn run(b: &Board, action: &str, rest: &[&str]) -> String {
    let o = command(b, action, rest);
    assert!(
        o.status.success(),
        "{action}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}
fn board(root: &Path, id: &str, test: u16, tech: Option<u16>) -> Board {
    board_tree(root, id, test, tech, false)
}
fn board_tree(root: &Path, id: &str, test: u16, tech: Option<u16>, c4: bool) -> Board {
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Synthetic CircuitNET", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    plan.config.transports.clear();
    let capabilities = vec![
        LocalOperatorCapability::ReadConfiguration,
        LocalOperatorCapability::ChangeSensitiveConfiguration,
        LocalOperatorCapability::NetworkRun,
        LocalOperatorCapability::NetworkStatus,
        LocalOperatorCapability::NetworkTest,
        LocalOperatorCapability::NetworkQueue,
    ];
    let principal = sf_bbs::current_operator_identity().unwrap();
    plan.config.operators.local_identities =
        vec![if let Some(uid) = principal.strip_prefix("unix-uid:") {
            LocalOperatorIdentity::Unix {
                uid: uid.parse().unwrap(),
                label: Some("Synthetic C2 operator".into()),
                capabilities,
            }
        } else {
            LocalOperatorIdentity::Windows {
                sid: principal.strip_prefix("windows-sid:").unwrap().into(),
                label: Some("Synthetic C2 operator".into()),
                capabilities,
            }
        }];
    let mut definition = plan.conferences[0].clone();
    definition.number = test;
    definition.name = "Synthetic CNTEST".into();
    definition.public_only = true;
    definition.posting_identity = Some(PostingIdentityPolicy::HandleAllowed);
    plan.conferences = vec![definition.clone()];
    if let Some(n) = tech {
        definition.number = n;
        definition.name = "Synthetic CNTECH".into();
        plan.conferences.push(definition);
    }
    sf_bbs::setup_board(root, &plan, b"synthetic circuitnet password").unwrap();
    let config = root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(root, &cfg.validate().unwrap()).unwrap();
    let (cert, key) = identity(root, id);
    let port = port();
    let b = Board {
        port,
        cert,
        key,
        config,
        db: paths.database().to_owned(),
        id: id.into(),
        test,
        tech,
    };
    let mut init = vec![id, "Synthetic CircuitNET", "--trusted-offline"];
    if c4 {
        init.extend([
            "ROOT1:ROOT:-",
            "HOST1:HOST:ROOT1",
            "HOST2:HOST:ROOT1",
            "END1:END:HOST1",
            "END2:END:HOST1",
            "END3:END:HOST2",
        ]);
    } else {
        init.extend([
            "ROOT0001:ROOT:-",
            "HOST0001:HOST:ROOT0001",
            "END00001:END:HOST0001",
            "END00002:END:HOST0001",
        ]);
    }
    run(&b, "init", &init);
    run(&b, "map", &[&test.to_string(), "CNTEST"]);
    if let Some(n) = tech {
        run(&b, "map", &[&n.to_string(), "CNTECH"]);
    }
    if c4 {
        let p = db(&b).circuitnet_status(&network()).unwrap().profile;
        for n in p.topology.neighbors(&p.local).unwrap() {
            run(&b, "subscribe", &[n.as_str(), "CNTEST"]);
            if !(id == "HOST2" && n.as_str() == "END3") {
                run(&b, "subscribe", &[n.as_str(), "CNTECH"]);
            }
        }
    } else if id == "ROOT0001" {
        run(&b, "subscribe", &["HOST0001", "CNTEST"]);
        run(&b, "subscribe", &["HOST0001", "CNTECH"]);
    } else if id == "HOST0001" {
        for (n, c) in [
            ("ROOT0001", "CNTEST"),
            ("ROOT0001", "CNTECH"),
            ("END00001", "CNTEST"),
            ("END00001", "CNTECH"),
            ("END00002", "CNTEST"),
        ] {
            run(&b, "subscribe", &[n, c]);
        }
    } else {
        run(&b, "subscribe", &["HOST0001", "CNTEST"]);
        if tech.is_some() {
            run(&b, "subscribe", &["HOST0001", "CNTECH"]);
        }
    }
    run(
        &b,
        "identity",
        &[b.cert.to_str().unwrap(), b.key.to_str().unwrap()],
    );
    run(&b, "listener", &[&format!("127.0.0.1:{}", b.port)]);
    b
}
fn db(b: &Board) -> RuntimeDatabase {
    let mut db = RuntimeDatabase::open(&b.db).unwrap();
    db.bind_posting_identity_configuration(&RuntimeConfig::load(&b.config).unwrap());
    db
}
fn actor(db: &RuntimeDatabase) -> MessageActor {
    let caller = db.caller_by_name(b"Sysop").unwrap().unwrap();
    MessageActor::new(caller.id, SecurityLevel::new(9999).unwrap())
}
fn post(b: &Board, tech: bool, subject: &str, parent: Option<u64>) -> Message {
    post_selected(b, tech, subject, parent, None)
}
fn post_selected(
    b: &Board,
    tech: bool,
    subject: &str,
    parent: Option<u64>,
    destination: Option<&str>,
) -> Message {
    let mut db = db(b);
    let actor = actor(&db);
    let c = db
        .conference(actor, if tech { b.tech.unwrap() } else { b.test })
        .unwrap();
    let parent = parent.map(|n| db.message(actor, c.id, n).unwrap().id);
    let preview = db.preview_posting_identity(actor, c.id).unwrap();
    let message = NewMessage {
        identity_preview: Some(preview),
        conference_id: c.id,
        recipient_caller_id: None,
        recipient_name: "All Callers".into(),
        subject: subject.as_bytes().to_vec(),
        body: b"Independently authored synthetic C2 traffic.\r\n".to_vec(),
        created_at: 1788800000,
        parent_message_id: parent,
        visibility: MessageVisibility::Public,
        kind: MessageKind::Standard,
    };
    if let Some(node) = destination {
        db.post_directed_circuitnet(
            actor,
            message,
            &network(),
            &envelope::NodeId::new(node).unwrap(),
        )
    } else {
        db.post(actor, message)
    }
    .unwrap()
}
fn count(b: &Board, tech: bool) -> usize {
    let db = db(b);
    let a = actor(&db);
    let c = db
        .conference(a, if tech { b.tech.unwrap() } else { b.test })
        .unwrap();
    db.messages(a, c.id).unwrap().len()
}

use sf_net::circuitnet::{
    self as envelope,
    transport::{self as wire, Error, Frame, Hello, Mode},
};
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    process::{Child, Stdio},
    time::{Duration, Instant},
};
fn port() -> u16 {
    // Keep fixture listeners outside the OS ephemeral source-port range.
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(36401);
    for _ in 0..1000 {
        let p = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    panic!("no isolated fixture port available")
}

fn identity(root: &Path, id: &str) -> (PathBuf, PathBuf) {
    let cert = root.join("identity.der");
    let key = root.join("identity-private.der");
    let pem = root.join("identity.key");
    let config = root.join("certificate.conf");
    fs::write(&config,format!("[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN={id}\n[ext]\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{}.circuitnet.invalid\n",id.to_ascii_lowercase())).unwrap();
    let output = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-days",
            "2",
            "-outform",
            "DER",
            "-config",
        ])
        .arg(config)
        .arg("-keyout")
        .arg(&pem)
        .arg("-out")
        .arg(&cert)
        .output()
        .unwrap();
    assert!(output.status.success(), "certificate generation failed");
    assert!(Command::new("openssl")
        .args(["pkcs8", "-topk8", "-nocrypt", "-outform", "DER", "-in"])
        .arg(&pem)
        .arg("-out")
        .arg(&key)
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new("openssl")
        .args(["x509", "-inform", "DER", "-in"])
        .arg(&cert)
        .arg("-out")
        .arg(cert.with_extension("pem"))
        .output()
        .unwrap()
        .status
        .success());
    (cert, key)
}
fn enroll(from: &Board, to: &Board) {
    run(
        from,
        "peer",
        &[
            &to.id,
            "127.0.0.1",
            &to.port.to_string(),
            &format!("{}.circuitnet.invalid", to.id.to_ascii_lowercase()),
            to.cert.to_str().unwrap(),
            "yes",
            "yes",
        ],
    );
}
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(b: &Board) -> Daemon {
    let child = Command::new(env!("CARGO_BIN_EXE_spitfire"))
        .arg("run")
        .arg(&b.config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            fs::File::create(b.config.with_extension("log")).unwrap(),
        ))
        .stderr(Stdio::from(
            fs::File::create(b.config.with_extension("errors")).unwrap(),
        ))
        .spawn()
        .unwrap();
    let mut d = Daemon(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(
            d.0.try_wait().unwrap().is_none(),
            "daemon failed: {}",
            fs::read_to_string(b.config.with_extension("errors")).unwrap()
        );
        if command(b, "live-status", &[]).status.success() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "listener/operator start deadline"
        );
        std::thread::sleep(Duration::from_millis(40));
    }
    d
}
fn stop(mut d: Daemon, b: &Board) {
    assert!(Command::new("kill")
        .args(["-INT", &d.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(s) = d.0.try_wait().unwrap() {
            assert!(s.success());
            break;
        }
        assert!(Instant::now() < deadline, "shutdown deadline");
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(TcpStream::connect(("127.0.0.1", b.port)).is_err());
}
fn status(b: &Board) -> sf_bbs::circuitnet_live::Status {
    serde_json::from_str(&run(b, "live-status", &[])).unwrap()
}
fn poll(from: &Board, to: &Board, test: bool) {
    // A restarted disposable macOS listener can accept an empty connection while
    // the client reports connect failure before TLS. Exercise bounded explicit
    // operator recovery; never retry an authentication/policy/protocol failure.
    // Production's three-attempt worker and its deadlines remain unchanged.
    for operator_attempt in 0..3 {
        run(from, if test { "test-link" } else { "poll" }, &[&to.id]);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let s = status(from);
            let p = s.peers.iter().find(|p| p.node.as_str() == to.id).unwrap();
            if !p.active {
                let result = p.health.as_ref().map(|h| h.result.as_str());
                if result == Some("ok") {
                    return;
                }
                if result == Some("connect") && operator_attempt < 2 {
                    break;
                }
                panic!("{} -> {} {:?}", from.id, to.id, p.health);
            }
            assert!(Instant::now() < deadline, "poll deadline");
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    unreachable!("finite attempts return success or fail with retained health")
}
// Independent OpenSSL-backed Python TLS probe. It relays bytes only; the Rust
// test supplies framing and checks native durable state. The child has finite
// socket deadlines and is reaped on Drop, including intentional ACK-loss cases.
struct Raw {
    child: Child,
    input: std::process::ChildStdin,
    output: std::process::ChildStdout,
}
impl std::io::Read for Raw {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.output.read(b)
    }
}
impl Write for Raw {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.input.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.input.flush()
    }
}
impl Drop for Raw {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn raw(from: &Board, to: &Board) -> Raw {
    let script = r#"
import os, socket, ssl, sys, threading
try:
    cert, key, trust, port, name = sys.argv[1:]
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    context.maximum_version = ssl.TLSVersion.TLSv1_3
    context.load_verify_locations(cadata=ssl.DER_cert_to_PEM_cert(open(trust,'rb').read()))
    context.load_cert_chain(cert,key)
    context.set_alpn_protocols(['circuitnet-ng/1'])
    connection = context.wrap_socket(socket.create_connection(('127.0.0.1',int(port)),5),server_hostname=name)
    connection.settimeout(12)
    assert connection.getpeercert(binary_form=True) == open(trust,'rb').read()
    assert connection.selected_alpn_protocol() == 'circuitnet-ng/1'
    def receive():
        try:
            while True:
                data=connection.recv(65536)
                if not data: break
                sys.stdout.buffer.write(data)
                sys.stdout.buffer.flush()
        except Exception: pass
        os._exit(0)
    threading.Thread(target=receive,daemon=True).start()
    while True:
        data=sys.stdin.buffer.read1(65536)
        if not data: break
        connection.sendall(data)
except Exception as error:
    print("TLS probe failed:", type(error).__name__, file=sys.stderr)
    sys.exit(1)
"#;
    let mut child = Command::new("python3")
        .args(["-u", "-c", script])
        .arg(from.cert.with_extension("pem"))
        .arg(from.key.with_file_name("identity.key"))
        .arg(&to.cert)
        .arg(to.port.to_string())
        .arg(format!("{}.circuitnet.invalid", to.id.to_ascii_lowercase()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    Raw {
        input: child.stdin.take().unwrap(),
        output: child.stdout.take().unwrap(),
        child,
    }
}

fn hello(b: &Board) -> Hello {
    let p = db(b).circuitnet_status(&network()).unwrap().profile;
    let mut h = Hello::new(
        network(),
        p.local.clone(),
        p.topology.node(&p.local).unwrap().role,
        Mode::Poll,
    );
    h.maximum_minor = 1;
    h.capabilities.truncate(2);
    h
}
fn negotiate(r: &mut Raw, b: &Board) {
    wire::write(r, &Frame::Hello { hello: hello(b) }).unwrap();
    assert!(matches!(
        wire::read(r, wire::CONTROL_FRAME),
        Ok(Frame::Hello { .. })
    ));
}
fn wait_idle(b: &Board, node: &Board) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while status(b)
        .peers
        .iter()
        .find(|p| p.node.as_str() == node.id)
        .unwrap()
        .active
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn prepare(from: &Board, to: &Board) -> sf_core::circuitnet::Prepared {
    let config = RuntimeConfig::load(&from.config).unwrap();
    let paths =
        LogicalPaths::resolve(from.config.parent().unwrap(), &config.validate().unwrap()).unwrap();
    let store = sf_bbs::DiskArtifactStore::new(paths.get(LogicalPath::System)).unwrap();
    let mut d = db(from);
    d.circuitnet_scan(&network(), 0, 1788800100).unwrap();
    d.circuitnet_prepare_neighbor(
        &store,
        &network(),
        &sf_core::circuitnet::NodeId::new(&to.id).unwrap(),
        1788800100,
    )
    .unwrap()
}

static JOURNEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn real_macos_four_node_live_journey_admission_routing_ack_loss_restore() {
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let _guard = JOURNEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C3_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("journey"));
    assert!(!root.exists());
    fs::create_dir_all(&root).unwrap();
    let r = board(&root.join("root"), "ROOT0001", 10, Some(11));
    let h = board(&root.join("host"), "HOST0001", 20, Some(21));
    let e1 = board(&root.join("end1"), "END00001", 30, Some(31));
    let e2 = board(&root.join("end2"), "END00002", 40, None);
    for (a, b) in [
        (&r, &h),
        (&h, &r),
        (&h, &e1),
        (&h, &e2),
        (&e1, &h),
        (&e2, &h),
    ] {
        enroll(a, b);
    }
    let rd = start(&r);
    let hd = start(&h);
    let d1 = start(&e1);
    let d2 = start(&e2);
    for (a, b) in [(&e1, &h), (&h, &r), (&e2, &h)] {
        poll(a, b, true);
    }
    assert_eq!(count(&h, false), 0);
    // Authenticated hostile hello bindings reject with bounded typed errors.
    for (change, expected) in [
        (0, Error::UnknownNode),
        (1, Error::WrongNetwork),
        (2, Error::UnsupportedVersion),
        (3, Error::TopologyMismatch),
    ] {
        let mut stream = raw(&e1, &h);
        let mut greeting = hello(&e1);
        match change {
            0 => greeting.node = envelope::NodeId::new("UNKNOWN").unwrap(),
            1 => greeting.network = envelope::NetworkId::new("wrong").unwrap(),
            2 => greeting.major = 2,
            _ => greeting.role = envelope::Role::Root,
        }
        wire::write(&mut stream, &Frame::Hello { hello: greeting }).unwrap();
        assert!(matches!(wire::read(&mut stream,wire::CONTROL_FRAME),Err(e) if e==expected));
        drop(stream);
        wait_idle(&h, &e1);
    }
    // A different configured credential cannot assert END1's Node ID.
    {
        let mut stream = raw(&e2, &h);
        wire::write(&mut stream, &Frame::Hello { hello: hello(&e1) }).unwrap();
        assert!(matches!(
            wire::read(&mut stream, wire::CONTROL_FRAME),
            Err(Error::UnknownNode)
        ));
    }
    wait_idle(&h, &e2);
    for (bytes, expected) in [
        (u32::MAX.to_be_bytes().to_vec(), Error::Oversized),
        (vec![0, 0, 0, 2, b'{', b'}'], Error::MalformedFrame),
    ] {
        let mut stream = raw(&e1, &h);
        negotiate(&mut stream, &e1);
        stream.write_all(&bytes).unwrap();
        stream.flush().unwrap();
        assert!(matches!(wire::read(&mut stream,wire::CONTROL_FRAME),Err(e) if e==expected));
        drop(stream);
        wait_idle(&h, &e1);
    }
    // Certificate name checks and a stalled authenticated frame fail closed.
    {
        let mut wrong = h.clone();
        wrong.id = "WRONG".into();
        let mut stream = raw(&e1, &wrong);
        let result = wire::write(&mut stream, &Frame::Hello { hello: hello(&e1) })
            .and_then(|_| wire::read(&mut stream, wire::CONTROL_FRAME).map(|_| 0));
        assert!(result.is_err());
    }
    wait_idle(&h, &e1);
    {
        let mut stream = raw(&e1, &h);
        negotiate(&mut stream, &e1);
        let started = Instant::now();
        stream.write_all(&[0, 0, 0, 100, b'{']).unwrap();
        stream.flush().unwrap();
        assert!(matches!(
            wire::read(&mut stream, wire::CONTROL_FRAME),
            Err(Error::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(15));
    }
    wait_idle(&h, &e1);
    // A non-neighbor's otherwise valid certificate has no TLS admission.
    {
        let mut stream = raw(&e2, &r);
        assert!(
            wire::write(&mut stream, &Frame::Hello { hello: hello(&e2) })
                .and_then(|_| wire::read(&mut stream, wire::CONTROL_FRAME).map(|_| 0))
                .is_err()
        );
    }
    post(&e1, false, "END1 start", None);
    poll(&e1, &h, false);
    assert_eq!(count(&h, false), 1);
    poll(&h, &r, false);
    poll(&e2, &h, false);
    for b in [&r, &h, &e1, &e2] {
        assert_eq!(count(b, false), 1);
    }
    for (b, ingress, path) in [
        (&h, "END00001", vec!["END00001", "HOST0001"]),
        (&r, "HOST0001", vec!["END00001", "HOST0001", "ROOT0001"]),
        (&e2, "HOST0001", vec!["END00001", "HOST0001", "END00002"]),
    ] {
        let connection = rusqlite::Connection::open(&b.db).unwrap();
        let (origin, actual_ingress, actual_path): (String, String, String) = connection
            .query_row(
                "SELECT origin,ingress,path FROM circuitnet_messages LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(origin, "END00001");
        assert_eq!(actual_ingress, ingress);
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&actual_path).unwrap(),
            path
        );
    }
    assert_eq!(
        db(&h)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&e1.id).unwrap())
            .unwrap(),
        0
    );
    post(&r, false, "ROOT down", None);
    poll(&h, &r, false);
    poll(&e1, &h, false);
    poll(&e2, &h, false);
    for b in [&r, &h, &e1, &e2] {
        assert_eq!(count(b, false), 2);
    }
    post(&r, true, "ROOT CNTECH", None);
    poll(&h, &r, false);
    poll(&e1, &h, false);
    poll(&e2, &h, false);
    assert_eq!(count(&e1, true), 1);
    assert_eq!(count(&e2, false), 2);
    post(&e2, false, "END2 parent", None);
    poll(&e2, &h, false);
    poll(&h, &r, false);
    poll(&e1, &h, false);
    post(&r, false, "ROOT reply", Some(3));
    poll(&h, &r, false);
    poll(&e1, &h, false);
    poll(&e2, &h, false);
    for b in [&r, &h, &e1, &e2] {
        assert_eq!(count(b, false), 4);
        let d = db(b);
        let a = actor(&d);
        let c = d.conference(a, b.test).unwrap();
        let messages = d.messages(a, c.id).unwrap();
        let reply = messages
            .iter()
            .find(|m| m.subject == b"ROOT reply")
            .unwrap();
        assert!(d
            .message(a, c.id, reply.number)
            .unwrap()
            .parent_message_id
            .is_some());
    }
    // Disconnect before any complete offer: no receiver mutation.
    let pending = post(&e1, false, "Interrupted", None);
    let prepared = prepare(&e1, &h);
    {
        let mut stream = raw(&e1, &h);
        negotiate(&mut stream, &e1);
        stream.write_all(&[0, 0, 0, 100, b'{']).unwrap();
        stream.flush().unwrap();
    }
    wait_idle(&h, &e1);
    assert_eq!(count(&h, false), 4);
    poll(&e1, &h, false);
    assert_eq!(count(&h, false), 5);
    // Replay the exact old offer; it must not import or fan out twice.
    {
        let mut stream = raw(&e1, &h);
        negotiate(&mut stream, &e1);
        wire::write(
            &mut stream,
            &Frame::Offer {
                batch: Some(envelope::Batch::decode(&prepared.bytes).unwrap()),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut stream, wire::CONTROL_FRAME),
            Ok(Frame::Ack {
                imported: 0,
                duplicates: 1,
                ..
            })
        ));
    }
    wait_idle(&h, &e1);
    assert_eq!(count(&h, false), 5);
    let _ = pending;
    // Durable receiver commit, lost ACK, cold sender backup/restore, retry.
    post(&e1, false, "Lost ACK restore", None);
    let prepared = prepare(&e1, &h);
    {
        let mut stream = raw(&e1, &h);
        negotiate(&mut stream, &e1);
        wire::write(
            &mut stream,
            &Frame::Offer {
                batch: Some(envelope::Batch::decode(&prepared.bytes).unwrap()),
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while count(&h, false) != 6 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        // Intentionally never consume ACK; sender queue remains uncertain.
    }
    wait_idle(&h, &e1);
    stop(d1, &e1);
    let backup = root.join("sender-backup");
    sf_bbs::backup_board(&e1.config, &backup).unwrap();
    // Advance the original sender after the snapshot, then restore older uncertainty.
    let advanced = start(&e1);
    poll(&e1, &h, false);
    assert_eq!(
        db(&e1)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&h.id).unwrap())
            .unwrap(),
        0
    );
    stop(advanced, &e1);
    let restored_root = root.join("restored-end1");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let config = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &cfg.validate().unwrap()).unwrap();
    let restored = Board {
        config,
        db: paths.database().to_owned(),
        id: e1.id.clone(),
        test: e1.test,
        tech: e1.tech,
        port: e1.port,
        cert: e1.cert.clone(),
        key: e1.key.clone(),
    };
    for q in db(&restored)
        .circuitnet_queue(&network(), "")
        .unwrap()
        .into_iter()
        .filter(|q| q.state == "held")
    {
        run(&restored, "retry", &[&q.id, &q.version.to_string()]);
    }
    let d1 = start(&restored);
    poll(&restored, &h, false);
    assert_eq!(count(&h, false), 6);
    assert_eq!(
        db(&restored)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&h.id).unwrap())
            .unwrap(),
        0
    );
    // Hold one branch; other branch and upstream continue, then exact catch-up.
    run(&h, "hold", &[&e2.id]);
    run(&e2, "poll", &[&h.id]);
    wait_idle(&e2, &h);
    assert_ne!(status(&e2).peers[0].health.as_ref().unwrap().result, "ok");
    post(&r, false, "Held branch", None);
    poll(&h, &r, false);
    poll(&restored, &h, false);
    assert_eq!(count(&e2, false), 4);
    run(&h, "release", &[&e2.id]);
    poll(&e2, &h, false);
    assert_eq!(count(&e2, false), 7);
    let dossier = db(&h)
        .circuitnet_status(&network())
        .unwrap()
        .dossiers
        .into_iter()
        .find(|d| d.neighbor.as_str() == e2.id && d.codename.as_str() == "CNTEST")
        .unwrap();
    run(
        &h,
        "live-unsubscribe",
        &[&e2.id, "CNTEST", &dossier.version.to_string()],
    );
    post(&r, false, "After unsubscribe", None);
    poll(&h, &r, false);
    poll(&restored, &h, false);
    poll(&e2, &h, false);
    assert_eq!(count(&e2, false), 7);
    post(&r, true, "Pending through HOST restart", None);
    poll(&h, &r, false);
    assert!(
        db(&h)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&restored.id).unwrap())
            .unwrap()
            > 0
    );
    stop(hd, &h);
    let hd = start(&h);
    poll(&restored, &h, true);
    poll(&restored, &h, false);
    assert_eq!(count(&restored, true), 2);
    assert!(status(&h).listening);
    stop(d1, &restored);
    stop(d2, &e2);
    stop(hd, &h);
    stop(rd, &r);
    for b in [&r, &h, &e1, &e2] {
        let log = fs::read_to_string(b.config.with_extension("log")).unwrap();
        assert!(!log.contains("BEGIN PRIVATE KEY"));
        assert!(!log.contains("Independently authored"));
    }
    fs::write(root.join("acceptance.txt"),"PASS: Darwin/native four-node TLS; admission; ROOT/HOST/END; two-hop threading; symmetric polling; Dossiers; no reflection; replay; before-commit disconnect; durable-commit lost ACK; sender backup/restore/retry; hold/release; restart; listeners removed. No production or external CircuitNET traffic.\n").unwrap();
}

#[test]
fn actual_sender_reconnects_after_unavailable_listener_and_lost_ack() {
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let _guard = JOURNEY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let host = board(&root.join("host"), "HOST0001", 20, Some(21));
    let end = board(&root.join("end"), "END00001", 30, Some(31));
    enroll(&end, &host);
    enroll(&host, &end);
    run(&end, "listener", &["off"]); // END works entirely by initiating polls.
    let daemon = start(&end);
    post(&end, false, "Automatic retry", None);
    run(&end, "poll", &[&host.id]);
    wait_idle(&end, &host);
    let s = status(&end);
    let health = s.peers[0].health.as_ref().unwrap();
    assert_eq!(health.result, "connect");
    assert_eq!(health.retry, 2);
    let listener = TcpListener::bind(("127.0.0.1", host.port)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let target = host.clone();
    let source = end.clone();
    let worker = std::thread::spawn(move || {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from(fs::read(&source.cert).unwrap()))
            .unwrap();
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            std::sync::Arc::new(roots),
            provider.clone(),
        )
        .build()
        .unwrap();
        let mut c = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                vec![CertificateDer::from(fs::read(&target.cert).unwrap())],
                PrivateKeyDer::try_from(fs::read(&target.key).unwrap()).unwrap(),
            )
            .unwrap();
        c.alpn_protocols = vec![wire::ALPN.to_vec()];
        let c = std::sync::Arc::new(c);
        let cfg = RuntimeConfig::load(&target.config).unwrap();
        let paths =
            LogicalPaths::resolve(target.config.parent().unwrap(), &cfg.validate().unwrap())
                .unwrap();
        let store = sf_bbs::DiskArtifactStore::new(paths.get(LogicalPath::System)).unwrap();
        let mut previous = None;
        for attempt in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(20);
            let socket = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            let mut stream =
                rustls::StreamOwned::new(rustls::ServerConnection::new(c.clone()).unwrap(), socket);
            assert!(matches!(
                wire::read(&mut stream, wire::CONTROL_FRAME),
                Ok(Frame::Hello { .. })
            ));
            wire::write(
                &mut stream,
                &Frame::Hello {
                    hello: hello(&target),
                },
            )
            .unwrap();
            let Frame::Offer { batch: Some(batch) } =
                wire::read(&mut stream, wire::MAX_FRAME).unwrap()
            else {
                panic!("missing offer")
            };
            let bytes = batch.encode().unwrap();
            if let Some(previous) = &previous {
                assert_eq!(previous, &bytes);
            } else {
                previous = Some(bytes.clone());
            }
            let result = db(&target)
                .circuitnet_import_neighbor(
                    &store,
                    &network(),
                    &envelope::NodeId::new(&source.id).unwrap(),
                    &bytes,
                    1788800200,
                )
                .unwrap();
            if attempt == 0 {
                assert_eq!(result.imported, 1);
                continue;
            } // Durable commit, force socket loss before ACK.
            assert_eq!(result.duplicates, 1);
            assert_eq!(result.imported, 0);
            wire::write(
                &mut stream,
                &Frame::Ack {
                    receipt: envelope::Receipt::decode(&result.receipt).unwrap(),
                    imported: 0,
                    duplicates: 1,
                },
            )
            .unwrap();
            wire::write(&mut stream, &Frame::Offer { batch: None }).unwrap();
            assert!(matches!(
                wire::read(&mut stream, wire::CONTROL_FRAME),
                Ok(Frame::Close {})
            ));
            wire::write(&mut stream, &Frame::Close {}).unwrap();
        }
    });
    poll(&end, &host, false);
    worker.join().unwrap();
    assert_eq!(count(&host, false), 1);
    let q = db(&end).circuitnet_queue(&network(), "").unwrap();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].state, "accepted");
    assert_eq!(q[0].attempts, 1);
    assert_eq!(status(&end).peers[0].health.as_ref().unwrap().retry, 1);
    stop(daemon, &end);
}

fn newest_request(b: &Board, action: &str, code: &str) -> String {
    let before = status(b)
        .controls
        .into_iter()
        .map(|e| e.request.id.clone())
        .collect::<Vec<_>>();
    run(b, action, &[code]);
    status(b)
        .controls
        .into_iter()
        .find(|e| e.direction == "outgoing" && !before.contains(&e.request.id))
        .unwrap()
        .request
        .id
        .to_string()
}
fn outcome(b: &Board, id: &str) -> sf_core::circuitnet::control::Outcome {
    status(b)
        .controls
        .into_iter()
        .find(|e| e.request.id.as_str() == id)
        .unwrap()
        .result
        .outcome
}
fn directed(from: &Board, to: &str, subject: &str) -> Message {
    post_selected(from, false, subject, None, Some(to))
}
fn assert_directed_path(boards: &[Board], subject: &str, path: &[&str]) {
    for b in boards {
        let c = rusqlite::Connection::open(&b.db).unwrap();
        let rows=c.prepare("SELECT c.origin,c.destination,c.path,c.identity FROM circuitnet_messages c JOIN messages m USING(message_id) JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE p.subject=?1").unwrap().query_map([subject.as_bytes()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        if let Some(index) = path.iter().position(|id| *id == b.id) {
            assert_eq!(rows.len(), 1);
            let (origin, destination, actual, id) = &rows[0];
            assert_eq!(origin, path[0]);
            assert_eq!(destination, *path.last().unwrap());
            assert_eq!(
                serde_json::from_str::<Vec<String>>(actual).unwrap(),
                path[..=index]
            );
            let neighbors = c
                .prepare("SELECT neighbor FROM circuitnet_deliveries WHERE identity=?1")
                .unwrap()
                .query_map([id], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(
                neighbors,
                path.get(index + 1)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(rows.is_empty(), "unrelated board {}", b.id);
        }
    }
}
fn c4_hello(raw: &mut Raw, b: &Board) {
    let mut h = hello(b);
    h.maximum_minor = 2;
    h.capabilities = wire::CAPABILITIES.iter().map(|s| s.to_string()).collect();
    wire::write(raw, &Frame::Hello { hello: h }).unwrap();
    assert!(matches!(
        wire::read(raw, wire::CONTROL_FRAME),
        Ok(Frame::Hello { .. })
    ));
}
fn c4_controls(
    raw: &mut Raw,
    requests: Vec<envelope::control::Request>,
) -> Vec<envelope::control::SubscriptionResult> {
    wire::write(raw, &Frame::Controls { requests }).unwrap();
    let Frame::ControlResults { results } = wire::read(raw, 1024 * 1024).unwrap() else {
        panic!("control result")
    };
    results
}
#[test]
fn six_node_c4_directed_controls_restart_restore_acceptance() {
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    use sf_core::circuitnet::control::{Operation, Outcome};
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C4_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("c4"));
    assert!(!root.exists());
    fs::create_dir(&root).unwrap();
    let boards = ["ROOT1", "HOST1", "HOST2", "END1", "END2", "END3"]
        .into_iter()
        .enumerate()
        .map(|(i, id)| board_tree(&root.join(id), id, 10 + i as u16, Some(20 + i as u16), true))
        .collect::<Vec<_>>();
    let [r, h1, h2, e1, e2, e3] = boards.as_slice() else {
        unreachable!()
    };
    for (a, b) in [(r, h1), (r, h2), (h1, e1), (h1, e2), (h2, e3)] {
        enroll(a, b);
        enroll(b, a);
    }
    let rd = start(r);
    let h1d = start(h1);
    let mut h2d = start(h2);
    let mut e1d = start(e1);
    let e2d = start(e2);
    let e3d = start(e3);
    for (a, b) in [(r, h1), (r, h2), (h1, e1), (h1, e2), (h2, e3)] {
        poll(a, b, true);
    }
    let route: serde_json::Value = serde_json::from_str(&run(e1, "route-test", &["END2"])).unwrap();
    assert_eq!(route["path"], serde_json::json!(["END1", "HOST1", "END2"]));
    directed(e1, "END2", "Same branch directed");
    poll(e1, h1, false);
    poll(e2, h1, false);
    assert_eq!(count(e2, false), 1);
    assert_eq!(count(h1, false), 0);
    assert_eq!(count(r, false), 0);
    assert_eq!(count(e3, false), 0);
    assert_directed_path(&boards, "Same branch directed", &["END1", "HOST1", "END2"]);
    directed(e1, "END3", "Cross branch directed");
    for (a, b) in [(e1, h1), (h1, r), (h2, r), (e3, h2)] {
        poll(a, b, false);
    }
    assert_eq!(count(e3, false), 1);
    assert_eq!(count(e2, false), 1);
    assert_eq!(count(r, false), 0);
    assert_eq!(count(h2, false), 0);
    let e3msg = db(e3).circuitnet_queue(&network(), "").unwrap();
    assert!(e3msg.is_empty());
    assert_directed_path(
        &boards,
        "Cross branch directed",
        &["END1", "HOST1", "ROOT1", "HOST2", "END3"],
    );
    directed(r, "END1", "Root directed");
    poll(h1, r, false);
    poll(e1, h1, false);
    assert_eq!(count(e1, false), 3);
    assert_directed_path(&boards, "Root directed", &["ROOT1", "HOST1", "END1"]);
    stop(e1d, e1);
    let unknown = post(e1, false, "Unknown destination held locally", None);
    assert!(!command(
        e1,
        "stage-direct",
        &[&unknown.id.get().to_string(), "UNKNOWN"]
    )
    .status
    .success());
    e1d = start(e1);
    poll(e1, h1, false);
    assert_eq!(status(e1).directed_failures, 1);
    // Independent TLS client loses ACK after intermediate durable acceptance.
    directed(e1, "END3", "Intermediate ACK loss");
    let offer = prepare(e1, h1);
    {
        let mut raw = raw(e1, h1);
        c4_hello(&mut raw, e1);
        assert!(c4_controls(&mut raw, vec![]).is_empty());
        wire::write(
            &mut raw,
            &Frame::Offer {
                batch: Some(envelope::Batch::decode(&offer.bytes).unwrap()),
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while db(h1)
            .circuitnet_queue(&network(), "")
            .unwrap()
            .iter()
            .filter(|q| q.state != "accepted")
            .count()
            != 1
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    wait_idle(h1, e1);
    poll(e1, h1, false);
    assert_eq!(
        db(h1)
            .circuitnet_queue(&network(), "")
            .unwrap()
            .iter()
            .filter(|q| q.state != "accepted")
            .count(),
        1
    );
    poll(h1, r, false);
    poll(h2, r, false);
    // Destination acceptance then loss before sender records receipt, plus replay.
    let last = prepare(h2, e3);
    {
        let mut raw = raw(h2, e3);
        c4_hello(&mut raw, h2);
        c4_controls(&mut raw, vec![]);
        wire::write(
            &mut raw,
            &Frame::Offer {
                batch: Some(envelope::Batch::decode(&last.bytes).unwrap()),
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while count(e3, false) != 2 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    wait_idle(e3, h2);
    poll(h2, e3, false);
    assert_eq!(count(e3, false), 2);
    {
        let mut raw = raw(h2, e3);
        c4_hello(&mut raw, h2);
        c4_controls(&mut raw, vec![]);
        wire::write(
            &mut raw,
            &Frame::Offer {
                batch: Some(envelope::Batch::decode(&last.bytes).unwrap()),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut raw, wire::CONTROL_FRAME),
            Ok(Frame::Ack { duplicates: 1, .. })
        ));
    }
    wait_idle(e3, h2);
    assert_eq!(count(e3, false), 2);
    let subscribe = newest_request(e3, "remote-subscribe", "CNTECH");
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &subscribe), Outcome::PendingApproval);
    assert_eq!(status(h2).pending_approvals, 1);
    stop(h2d, h2);
    h2d = start(h2);
    assert_eq!(status(h2).pending_approvals, 1);
    run(h2, "approve", &[&subscribe]);
    // Restart after apply, before requester receives the terminal result.
    stop(h2d, h2);
    h2d = start(h2);
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &subscribe), Outcome::Applied);
    post(h2, true, "CNTECH after approval", None);
    poll(e3, h2, false);
    assert_eq!(count(e3, true), 1);
    run(e3, "control-retry", &[&subscribe]);
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &subscribe), Outcome::Applied);
    assert_eq!(count(e3, true), 1);
    let unsub = newest_request(e3, "remote-unsubscribe", "CNTECH");
    poll(e3, h2, false);
    run(h2, "approve", &[&unsub]);
    poll(e3, h2, false);
    post(h2, true, "CNTECH after unsubscribe", None);
    poll(e3, h2, false);
    assert_eq!(count(e3, true), 1);
    // TLS-authenticated END1 cannot claim END3 or another branch.
    let mut forged = db(e1)
        .circuitnet_request(
            &network(),
            Operation::Subscribe,
            Some(envelope::Codename::new("CNTECH").unwrap()),
            1788800200,
        )
        .unwrap();
    forged.requester = envelope::NodeId::new("END3").unwrap();
    {
        let mut raw = raw(e1, h1);
        c4_hello(&mut raw, e1);
        assert_eq!(
            c4_controls(&mut raw, vec![forged])[0].outcome,
            Outcome::Unauthorized
        );
    }
    wait_idle(h1, e1);
    let deny = newest_request(e3, "remote-subscribe", "CNTECH");
    poll(e3, h2, false);
    run(h2, "deny", &[&deny]);
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &deny), Outcome::Denied);
    run(e3, "control-retry", &[&deny]);
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &deny), Outcome::Denied);
    run(h2, "control-policy", &["auto-approve"]);
    let automatic = newest_request(e3, "remote-subscribe", "CNTECH");
    let request = db(e3)
        .circuitnet_pending_controls(&network(), &envelope::NodeId::new("HOST2").unwrap())
        .unwrap();
    {
        let mut raw = raw(e3, h2);
        c4_hello(&mut raw, e3);
        wire::write(&mut raw, &Frame::Controls { requests: request }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !status(h2)
            .controls
            .iter()
            .any(|e| e.request.id.as_str() == automatic && e.result.outcome == Outcome::Applied)
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        // Drop without reading or durably recording the result.
    }
    wait_idle(h2, e3);
    stop(h2d, h2);
    h2d = start(h2);
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &automatic), Outcome::Applied);
    run(h2, "control-policy", &["deny"]);
    let disabled = newest_request(e3, "remote-unsubscribe", "CNTECH");
    poll(e3, h2, false);
    assert_eq!(outcome(e3, &disabled), Outcome::Denied);
    // Cold snapshot of uncertain directed sender, original subsequently advances.
    directed(e1, "END2", "Restored uncertain sender");
    prepare(e1, h1);
    stop(e1d, e1);
    let backup = root.join("sender-backup");
    sf_bbs::backup_board(&e1.config, &backup).unwrap();
    e1d = start(e1);
    poll(e1, h1, false);
    poll(e2, h1, false);
    let before = count(e2, false);
    stop(e1d, e1);
    let restored_root = root.join("restored-sender");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let config = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &cfg.validate().unwrap()).unwrap();
    let restored = Board {
        config,
        db: paths.database().to_owned(),
        ..e1.clone()
    };
    for q in db(&restored)
        .circuitnet_queue(&network(), "")
        .unwrap()
        .into_iter()
        .filter(|q| q.state == "held")
    {
        run(&restored, "retry", &[&q.id, &q.version.to_string()]);
    }
    let restored_daemon = start(&restored);
    poll(&restored, h1, false);
    poll(e2, h1, false);
    assert_eq!(count(e2, false), before);
    // Host backup retains applied/denied/pending control authority across mutation.
    run(h2, "control-policy", &["require-approval"]);
    let pending = newest_request(e3, "remote-unsubscribe", "CNTECH");
    poll(e3, h2, false);
    stop(h2d, h2);
    let backup = root.join("host-backup");
    sf_bbs::backup_board(&h2.config, &backup).unwrap();
    h2d = start(h2);
    run(h2, "deny", &[&pending]);
    stop(h2d, h2);
    let restored_root = root.join("restored-host");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let config = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &cfg.validate().unwrap()).unwrap();
    let restored_h = Board {
        config,
        db: paths.database().to_owned(),
        ..h2.clone()
    };
    let hd = start(&restored_h);
    assert_eq!(status(&restored_h).pending_approvals, 1);
    run(&restored_h, "approve", &[&pending]);
    poll(e3, &restored_h, false);
    assert_eq!(outcome(e3, &pending), Outcome::Applied);
    run(e3, "control-retry", &[&automatic]);
    poll(e3, &restored_h, false);
    assert_eq!(outcome(e3, &automatic), Outcome::Applied);
    assert_eq!(outcome(&restored_h, &deny), Outcome::Denied);
    assert!(
        !db(&restored_h)
            .circuitnet_status(&network())
            .unwrap()
            .dossiers
            .iter()
            .find(|d| d.neighbor.as_str() == "END3" && d.codename.as_str() == "CNTECH")
            .unwrap()
            .subscribed
    );
    for b in [r, h1, &restored_h, &restored, e2, e3] {
        let log = fs::read_to_string(b.config.with_extension("log")).unwrap();
        assert!(!log.contains("Independently authored"));
        assert!(!log.contains("PRIVATE KEY"));
    }
    stop(rd, r);
    stop(h1d, h1);
    stop(hd, &restored_h);
    stop(restored_daemon, &restored);
    stop(e2d, e2);
    stop(e3d, e3);
    fs::write(root.join("acceptance.txt"),"PASS: six independent native daemons; mutual TLS; three directed routes; no fanout; unknown fail closed; intermediate/destination ACK loss and replay; remote approval/denial/policies; forged child rejected; pending/apply restart; uncertain sender restore; host control restore. Only disposable local boards; no external CircuitNET or production changes.\n").unwrap();
}

fn event_command(b: &Board, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_spitfire"))
        .arg("events")
        .arg(&b.config)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "Event command: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}
fn eventually(label: &str, mut test: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(150);
    while !test() {
        assert!(Instant::now() < deadline, "{label}");
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn save_event(b: &Board, d: &sf_core::events::Definition) {
    eventually("Event idle for edit", || {
        db(b).events().unwrap().iter().all(|e| !e.running)
    });
    let path = b.config.with_extension("event.json");
    fs::write(&path, serde_json::to_vec(d).unwrap()).unwrap();
    event_command(b, &["save", path.to_str().unwrap()]);
}
#[test]
fn real_macos_c5_events_policy_burst_controls_restart_restore() {
    use sf_core::circuitnet::control::Outcome;
    use sf_core::events::{Action, Definition, ExchangePolicy as Policy, MissedPolicy, Schedule};
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C5_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("c5"));
    assert!(!root.exists());
    fs::create_dir(&root).unwrap();
    let host = board(&root.join("host"), "HOST0001", 20, Some(21));
    let end = board(&root.join("end"), "END00001", 30, Some(31));
    let other = board(&root.join("other"), "END00002", 40, None);
    for peer in [&end, &other] {
        enroll(peer, &host);
        enroll(&host, peer);
        run(peer, "listener", &["off"]);
    }
    let mut hd = start(&host);
    let mut ed = start(&end);
    let od = start(&other);
    let mut event = Definition {
        id: "event-a".into(),
        name: "CircuitNET Mail Run".into(),
        enabled: true,
        action: Action::Circuitnet {
            network: network(),
            node: Some(envelope::NodeId::new(&host.id).unwrap()),
        },
        schedule: Schedule::Interval { seconds: 20 },
        timezone: "America/Phoenix".into(),
        policy: Policy::Scheduled,
        missed: MissedPolicy::RunOnce,
        minimum_spacing_seconds: 5,
    };
    let mut quiet_event = event.clone();
    quiet_event.policy = Policy::Immediate;
    save_event(&other, &quiet_event);
    save_event(&end, &event);
    let before_generation = db(&end).network_preparation_state().unwrap().0;
    let m = post(&end, false, "C5 scheduled first", None);
    assert!(db(&end).network_preparation_state().unwrap().0 > before_generation);
    eventually("native queue prepared independently of due Event", || {
        db(&end)
            .circuitnet_queue(&network(), "")
            .unwrap()
            .iter()
            .any(|q| q.identity.origin().as_str() == end.id)
    });
    assert_eq!(count(&host, false), 0);
    eventually("scheduled delivery", || count(&host, false) == 1);
    assert_eq!(count(&end, false), 1);
    assert!(m.id.get() > 0);
    event.policy = Policy::Manual;
    save_event(&end, &event);
    post(&end, false, "C5 manual waits", None);
    std::thread::sleep(Duration::from_secs(7));
    assert_eq!(count(&host, false), 1);
    let operator = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    operator.block_on(async {
        let mut client = sf_bbs::OperatorClient::connect(&end.config).await.unwrap();
        client.describe_operator_controls().await.unwrap();
        let id = format!("{:032x}", rand::random::<u128>());
        let action = sf_bbs::NetworkAction::Event {
            request: sf_bbs::events::Command::Run {
                id: "event-a".into(),
                expected: db(&end).events().unwrap()[0].version,
            },
        };
        assert!(matches!(
            client
                .qwk_network_action(id.clone(), action.clone())
                .await
                .unwrap(),
            sf_bbs::NetworkResult::Updated
        ));
        assert!(matches!(
            client.qwk_network_action(id, action).await.unwrap(),
            sf_bbs::NetworkResult::Replayed { .. }
        ));
    });
    eventually("Run Now delivery", || count(&host, false) == 2);
    assert_eq!(
        db(&end)
            .event_history("event-a")
            .unwrap()
            .iter()
            .filter(|h| h.trigger == "manual")
            .count(),
        1
    );
    event.policy = Policy::Immediate;
    save_event(&end, &event);
    // Let the policy edit settle before measuring the burst's finite sessions.
    eventually("immediate idle", || !db(&end).events().unwrap()[0].running);
    let before = db(&end).event_history("event-a").unwrap().len();
    for i in 0..50 {
        post(&end, false, &format!("C5 burst {i}"), None);
    }
    eventually("burst drains multiple bounded batches", || {
        count(&host, false) == 52
    });
    eventually("burst completed", || !db(&end).events().unwrap()[0].running);
    let after = db(&end).event_history("event-a").unwrap().len();
    assert!(
        after - before <= 4,
        "burst created {} Event sessions",
        after - before
    );
    assert!(
        db(&other).event_history("event-a").unwrap().is_empty(),
        "an unrelated idle target must not poll"
    );
    event.policy = Policy::Hybrid;
    event.schedule = Schedule::Interval { seconds: 5 };
    save_event(&end, &event);
    run(&end, "hold", &[&host.id]);
    post(&end, false, "C5 held", None);
    eventually("held Event result", || {
        db(&end).events().unwrap()[0].last_result.as_deref() == Some("held")
    });
    assert_eq!(count(&host, false), 52);
    assert!(
        db(&end)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&host.id).unwrap())
            .unwrap()
            > 0
    );
    run(&end, "release", &[&host.id]);
    eventually("released catch-up", || count(&host, false) == 53);
    stop(hd, &host);
    post(&end, false, "C5 offline", None);
    eventually("offline failure retained", || {
        db(&end).events().unwrap()[0].last_result.as_deref() == Some("failed")
    });
    assert!(
        db(&end)
            .circuitnet_neighbor_pending(&network(), &envelope::NodeId::new(&host.id).unwrap())
            .unwrap()
            > 0
    );
    hd = start(&host);
    eventually("returned peer catch-up", || count(&host, false) == 54);
    // Directed native post and destination commit atomically while preparation runs.
    let mut receiver_event = event.clone();
    receiver_event.policy = Policy::Scheduled;
    save_event(&other, &receiver_event);
    eventually("existing broadcast fanout drained", || {
        count(&other, false) == 54
    });
    let previous = count(&other, false);
    directed(&end, &other.id, "C5 scheduled directed");
    eventually("scheduled HOST transit directed", || {
        count(&other, false) == previous + 1
    });
    assert_eq!(count(&host, false), 54);
    // The other END also receives preexisting public fanout; verify identity,
    // destination and unique import directly rather than treating it as private.
    let c = rusqlite::Connection::open(&other.db).unwrap();
    let directed_count: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM circuitnet_messages WHERE destination=?1",
            [&other.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(directed_count, 1);
    drop(c);
    let dossier = db(&host)
        .circuitnet_status(&network())
        .unwrap()
        .dossiers
        .into_iter()
        .find(|d| d.neighbor.as_str() == end.id && d.codename.as_str() == "CNTECH")
        .unwrap();
    run(
        &host,
        "live-unsubscribe",
        &[&end.id, "CNTECH", &dossier.version.to_string()],
    );
    let request = newest_request(&end, "remote-subscribe", "CNTECH");
    eventually("authenticated pending request via Event", || {
        db(&host)
            .circuitnet_controls(&network(), "")
            .unwrap()
            .iter()
            .any(|e| e.request.id.as_str() == request)
    });
    run(&host, "approve", &[&request]);
    eventually("approved result returned by Event", || {
        outcome(&end, &request) == Outcome::Applied
    });
    post(&host, true, "C5 CNTECH subscribed", None);
    eventually("new subscription receives", || count(&end, true) == 1);
    let request = newest_request(&end, "remote-unsubscribe", "CNTECH");
    eventually("unsubscribe pending", || {
        db(&host)
            .circuitnet_controls(&network(), "")
            .unwrap()
            .iter()
            .any(|e| e.request.id.as_str() == request)
    });
    run(&host, "approve", &[&request]);
    eventually("unsubscribe applied", || {
        outcome(&end, &request) == Outcome::Applied
    });
    post(&host, true, "C5 CNTECH stopped", None);
    std::thread::sleep(Duration::from_secs(7));
    assert_eq!(count(&end, true), 1);
    // Restart before due and once after missing a slot; no backlog replay.
    event.policy = Policy::Scheduled;
    event.schedule = Schedule::Interval { seconds: 20 };
    save_event(&end, &event);
    let due = db(&end).events().unwrap()[0].next_due;
    stop(ed, &end);
    ed = start(&end);
    assert_eq!(db(&end).events().unwrap()[0].next_due, due);
    stop(ed, &end);
    std::thread::sleep(Duration::from_secs(22));
    ed = start(&end);
    eventually("missed slot caught once", || {
        db(&end).events().unwrap()[0].next_due > due
    });
    stop(ed, &end);
    let backup = root.join("backup");
    sf_bbs::backup_board(&end.config, &backup).unwrap();
    let original = db(&end).events().unwrap()[0].definition.clone();
    let mut changed = original.clone();
    changed.enabled = false;
    let mut native = db(&end);
    let version = native.events().unwrap()[0].version;
    native
        .event_save(&changed, version, chrono::Utc::now().timestamp())
        .unwrap();
    drop(native);
    let restored_root = root.join("restored");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let config = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &cfg.validate().unwrap()).unwrap();
    let restored = Board {
        config,
        db: paths.database().to_owned(),
        ..end.clone()
    };
    assert_eq!(db(&restored).events().unwrap()[0].definition, original);
    let restored_daemon = start(&restored);
    assert!(!db(&restored).events().unwrap()[0].running);
    let history = event_command(&restored, &["history", "event-a"]);
    assert!(history.contains("scheduled"));
    assert!(!history.contains("synthetic circuitnet password"));
    stop(restored_daemon, &restored);
    stop(od, &other);
    stop(hd, &host);
    fs::write(root.join("acceptance.txt"),"PASS: native macOS daemon Events; immediate durable preparation; scheduled/manual/immediate/hybrid; burst coalescing and multiple batches; hold/release; offline/recovery; directed HOST transit; authenticated subscription approval/unsubscribe; restart/missed slot; Run Now; cold backup/restore; no running resurrection; graceful shutdown. Disposable loopback only.\n").unwrap();
}
