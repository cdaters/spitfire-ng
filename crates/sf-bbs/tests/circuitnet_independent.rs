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

//! Actual SPITFIRE daemon versus independent Python TLS peer. Fixture setup may
//! use native APIs; Python protocol behavior never imports this test or Rust code.
#![cfg(unix)]
use serde_json::{json, Value};
use sf_core::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const NET: &str = "conformance";
struct Native {
    config: PathBuf,
    database: PathBuf,
    port: u16,
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn checked(mut cmd: Command) -> String {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn tool() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/circuitnet-conformance")
}
fn cn(b: &Native, action: &str, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_spitfire"));
    c.args(["circuitnet", b.config.to_str().unwrap(), action, NET])
        .args(args);
    checked(c)
}
fn database(b: &Native) -> RuntimeDatabase {
    let mut d = RuntimeDatabase::open(&b.database).unwrap();
    d.bind_posting_identity_configuration(&RuntimeConfig::load(&b.config).unwrap());
    d
}
fn actor(d: &RuntimeDatabase) -> MessageActor {
    MessageActor::new(
        d.caller_by_name(b"Sysop").unwrap().unwrap().id,
        SecurityLevel::new(9999).unwrap(),
    )
}
fn messages(b: &Native) -> Vec<Message> {
    let d = database(b);
    let a = actor(&d);
    let conf = d.conference(a, 42).unwrap();
    d.messages(a, conf.id)
        .unwrap()
        .into_iter()
        .map(|m| d.message(a, conf.id, m.number).unwrap())
        .collect()
}
fn post(b: &Native, subject: &str, parent: Option<MessageId>) {
    let mut d = database(b);
    let a = actor(&d);
    let c = d.conference(a, 42).unwrap();
    let preview = d.preview_posting_identity(a, c.id).unwrap();
    d.post(
        a,
        NewMessage {
            identity_preview: Some(preview),
            conference_id: c.id,
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: subject.as_bytes().to_vec(),
            body: b"Native synthetic reply\r\n".to_vec(),
            created_at: 1788800100,
            parent_message_id: parent,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        },
    )
    .unwrap();
}
fn setup(root: &Path, certs: &Path) -> Native {
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Conformance BBS", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    plan.config.transports.clear();
    let who = sf_bbs::current_operator_identity().unwrap();
    plan.config.operators.local_identities = vec![LocalOperatorIdentity::Unix {
        uid: who.strip_prefix("unix-uid:").unwrap().parse().unwrap(),
        label: Some("Synthetic operator".into()),
        capabilities: vec![
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
            LocalOperatorCapability::NetworkRun,
            LocalOperatorCapability::NetworkStatus,
            LocalOperatorCapability::NetworkTest,
            LocalOperatorCapability::NetworkQueue,
        ],
    }];
    plan.conferences.truncate(1);
    plan.conferences[0].number = 42;
    plan.conferences[0].public_only = true;
    plan.conferences[0].posting_identity = Some(PostingIdentityPolicy::HandleAllowed);
    sf_bbs::setup_board(root, &plan, b"synthetic conformance password").unwrap();
    let config = root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(root, &cfg.validate().unwrap()).unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let b = Native {
        config,
        database: paths.database().to_owned(),
        port,
    };
    cn(
        &b,
        "init",
        &[
            "USAZ000",
            "Synthetic conformance",
            "--live",
            "ROOT:ROOT:-",
            "USAZ000:HOST:ROOT",
            "USAZ001:END:USAZ000",
        ],
    );
    cn(
        &b,
        "identity",
        &[
            certs.join("USAZ000.der").to_str().unwrap(),
            certs.join("USAZ000.key.der").to_str().unwrap(),
        ],
    );
    cn(&b, "listener", &[&format!("127.0.0.1:{port}")]);
    cn(
        &b,
        "catalog-pin",
        &[certs.join("authority.json").to_str().unwrap()],
    );
    for file in ["catalog1.json", "catalog2.json"] {
        cn(&b, "catalog-import", &[certs.join(file).to_str().unwrap()]);
    }
    cn(&b, "catalog-map", &["RETRO", "42"]);
    cn(&b, "catalog-create-map", &["SUPPORT", "43"]);
    cn(&b, "subscribe", &["USAZ001", "SUPPORT"]);
    cn(&b, "subscribe", &["USAZ001", "RETRO"]);
    let mut d = database(&b);
    let area = d
        .create_file_area(&FileAreaDefinition {
            number: 77,
            name: "Harmless files".into(),
            description: "Conformance".into(),
            storage_key: "conformance".into(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            upload_security: SecurityLevel::new(0).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1048576,
            privileged_security_levels: vec![],
        })
        .unwrap();
    d.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        area.id,
        &sf_core::files::SafetyPolicy {
            scanning: sf_core::files::ScanPolicy::Disabled,
            approval_required: false,
            ..Default::default()
        },
    )
    .unwrap();
    drop(d);
    cn(&b, "file-map", &["KITDOCS", "77", "yes", "yes", "1048576"]);
    cn(&b, "file-subscribe", &["USAZ001", "KITDOCS", "yes"]);
    b
}
fn import_file(b: &Native, source: &Path, name: &str) {
    sf_bbs::files::run(
        &b.config,
        &[
            "import",
            "77",
            source.to_str().unwrap(),
            name,
            "Synthetic payload",
        ]
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>(),
    )
    .unwrap();
}
fn start(b: &Native) -> Running {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_spitfire"));
    cmd.args(["run", b.config.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(fs::File::create(b.config.with_extension("log")).unwrap())
        .stderr(fs::File::create(b.config.with_extension("errors")).unwrap());
    let mut running = Running(cmd.spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(running.0.try_wait().unwrap().is_none());
        let out = Command::new(env!("CARGO_BIN_EXE_spitfire"))
            .args(["circuitnet", b.config.to_str().unwrap(), "live-status", NET])
            .output()
            .unwrap();
        if out.status.success() {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(30));
    }
    running
}
fn peer_command(root: &Path, port: u16, work: Option<&Path>) -> Command {
    let mut c = Command::new("python3");
    c.arg(tool().join("peer.py")).args([
        "--config",
        root.join("peer.json").to_str().unwrap(),
        "--state",
        root.join("peer-state").to_str().unwrap(),
        "--port",
        &port.to_string(),
        "--report",
        root.join("peer-report.json").to_str().unwrap(),
    ]);
    if let Some(w) = work {
        c.arg("--work").arg(w);
    }
    c
}
fn peer_run(root: &Path, port: u16, work: Option<&Path>, expected: &str) -> Value {
    let out = peer_command(root, port, work).output().unwrap();
    let report: Value =
        serde_json::from_slice(&fs::read(root.join("peer-report.json")).unwrap()).unwrap();
    assert_eq!(
        report["result"],
        expected,
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.success(), expected == "ok");
    report
}
fn state(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("peer-state/state.json")).unwrap()).unwrap()
}
fn save(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn wait_peer(b: &Native) {
    let end = Instant::now() + Duration::from_secs(40);
    loop {
        let s: Value = serde_json::from_str(&cn(b, "live-status", &[])).unwrap();
        let peer = s["peers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["node"] == "USAZ001")
            .unwrap();
        if peer["active"] == false && !peer["health"].is_null() {
            assert_eq!(peer["health"]["result"], "ok", "{s}");
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(40));
    }
}
#[test]
fn real_macos_independent_python_peer_interoperability() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("interop");
    fs::create_dir(&root).unwrap();
    let mut fixture = Command::new("python3");
    fixture
        .arg(tool().join("fixtures.py"))
        .arg("--output")
        .arg(&root);
    checked(fixture);
    let b = setup(&root.join("native"), &root);
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let peer_port = socket.local_addr().unwrap().port();
    drop(socket);
    cn(
        &b,
        "peer",
        &[
            "USAZ001",
            "127.0.0.1",
            &peer_port.to_string(),
            "usaz001.circuitnet.invalid",
            root.join("USAZ001.der").to_str().unwrap(),
            "yes",
            "yes",
        ],
    );
    post(&b, "Native parent", None);
    let native_payload = root.join("native.txt");
    fs::write(
        &native_payload,
        b"Native independent transfer\n".repeat(4000),
    )
    .unwrap();
    import_file(&b, &native_payload, "native.txt");
    let daemon = start(&b);
    cn(&b, "control-policy", &["auto-approve"]);
    let workfile = root.join("work.json");
    let mut work: Value = serde_json::from_slice(&fs::read(&workfile).unwrap()).unwrap();
    let report = peer_run(&root, b.port, Some(&workfile), "ok");
    assert_eq!(report["catalogs_received"], 2);
    assert!(report["payload_received"].as_u64().unwrap() > 0);
    assert!(report["payload_sent"].as_u64().unwrap() > 0);
    assert_eq!(messages(&b).len(), 3);
    let s = state(&root);
    assert_eq!(s["messages"].as_object().unwrap().len(), 1);
    let native_id = s["messages"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    // A minor-1 peer must not smuggle a generation-aware message into a catalog mapping.
    let mut unsupported = work.clone();
    unsupported["batch"]["messages"] = json!([unsupported["batch"]["messages"][0].clone()]);
    unsupported["batch"]["messages"][0]["id"] = json!(format!("USAZ001:{:032x}", 99));
    let probe_work = root.join("probe-work.json");
    save(&probe_work, &unsupported);
    let mut probe = Command::new("python3");
    probe.arg(tool().join("probe.py")).args([
        "--config",
        root.join("peer.json").to_str().unwrap(),
        "--state",
        root.join("probe-state").to_str().unwrap(),
        "--work",
        probe_work.to_str().unwrap(),
        "--port",
        &b.port.to_string(),
        "--report",
        root.join("probe-report.json").to_str().unwrap(),
    ]);
    let outcome = probe.output().unwrap();
    let probe_report: Value =
        serde_json::from_slice(&fs::read(root.join("probe-report.json")).unwrap()).unwrap();
    assert!(
        outcome.status.success(),
        "unnegotiated generation accepted: {probe_report}"
    );
    unsupported["batch"]["messages"][0]["id"] = json!(format!("USAZ001:{:032x}", 100));
    unsupported["batch"]["messages"][0]["conference_identity"] = json!("2".repeat(32));
    unsupported["batch"]["messages"][0]["codename"] = json!("SUPPORT");
    save(&probe_work, &unsupported);
    probe.args([
        "--catalog-without-access",
        "--state",
        root.join("probe-access-state").to_str().unwrap(),
    ]);
    let outcome = probe.output().unwrap();
    let probe_report: Value =
        serde_json::from_slice(&fs::read(root.join("probe-report.json")).unwrap()).unwrap();
    assert!(
        outcome.status.success(),
        "unnegotiated Sysop access accepted: {probe_report}"
    );
    // Exact artifact replay recovers all duplicates without native reimport.
    peer_run(&root, b.port, Some(&workfile), "ok");
    assert_eq!(messages(&b).len(), 3);
    // Independent reply to native parent, directed only to this BBS.
    let mut directed = work["batch"]["messages"][0].clone();
    directed["id"] = json!(format!("USAZ001:{:032x}", 3));
    directed["reply"] = json!(native_id);
    directed["destination"] = json!("USAZ000");
    directed["subject"] = json!("Directed reply");
    work["batch"]["messages"] = json!([directed]);
    work["files"] = json!([]);
    work["controls"] = json!([]);
    save(&workfile, &work);
    peer_run(&root, b.port, Some(&workfile), "ok");
    let msgs = messages(&b);
    assert_eq!(msgs.len(), 4);
    let reply = msgs
        .iter()
        .find(|m| m.subject == b"Directed reply")
        .unwrap();
    assert!(reply.parent_message_id.is_some());
    post(&b, "Native reply", Some(reply.id));
    peer_run(&root, b.port, None, "ok");
    assert!(state(&root)["messages"]
        .as_object()
        .unwrap()
        .values()
        .any(|v| v["message"]["reply"] == format!("USAZ001:{:032x}", 3)));
    work["batch"]["messages"][0]["body"] = json!("Conflicting body");
    save(&workfile, &work);
    peer_run(&root, b.port, Some(&workfile), "conflicting-message");
    assert_eq!(messages(&b).len(), 5);
    // New publication of known content: no bytes, new receipt, both directions.
    work["batch"] = Value::Null;

    let mut pubobj: Value = serde_json::from_str::<Value>(include_str!(
        "../../../tools/circuitnet-conformance/vectors/valid.json"
    ))
    .unwrap()["publication"]
        .clone();
    let content = fs::read(root.join("payload.txt")).unwrap();
    pubobj["id"] = json!(format!("USAZ001:{:032x}", 2));
    pubobj["sha256"] = json!(sf_net::circuitnet::digest(&content));
    pubobj["size"] = json!(content.len());
    pubobj["filename"] = json!("other.txt");
    work["files"] = json!([{"publication":pubobj,"source":root.join("payload.txt")}]);
    save(&workfile, &work);
    let r = peer_run(&root, b.port, Some(&workfile), "ok");
    assert_eq!(r["payload_sent"], 0);
    assert_eq!(r["hash_have_sent"], 1);
    // Direct native import uses the same local Files pipeline while daemon is stopped later.
    // Malicious bytes remain harmless text and must produce a rejected receipt.
    pubobj["id"] = json!(format!("USAZ001:{:032x}", 4));
    pubobj["size"] = json!(4);
    pubobj["sha256"] = json!(sf_net::circuitnet::digest(b"good"));
    pubobj["filename"] = json!("bad.txt");
    fs::write(root.join("wrong.txt"), b"oops").unwrap();
    work["files"] = json!([{"publication":pubobj,"source":root.join("wrong.txt")}]);
    save(&workfile, &work);
    peer_run(&root, b.port, Some(&workfile), "ok");
    assert_eq!(
        state(&root)["sent"][format!("file:USAZ001:{:032x}", 4)]["reason"],
        "hash-mismatch"
    );
    // Dossier unsubscribe, subscribe, unauthorized requester and unknown codename.
    work["files"] = json!([]);
    let mut requests = Vec::new();
    for (n, op, who, code) in [
        (20, "unsubscribe", "USAZ001", "RETRO"),
        (21, "subscribe", "USAZ001", "RETRO"),
        (22, "subscribe", "ROOT", "RETRO"),
        (23, "subscribe", "USAZ001", "UNKNOWN"),
    ] {
        requests.push(json!({"network":NET,"id":format!("{who}:{n:032x}"),"requester":who,"target":"USAZ000","operation":op,"codename":code}));
    }
    work["controls"] = json!(requests);
    save(&workfile, &work);
    peer_run(&root, b.port, Some(&workfile), "ok");
    let s = state(&root);
    assert_eq!(
        s["sent"][format!("control:ROOT:{:032x}", 22)]["outcome"],
        "unauthorized"
    );
    assert_eq!(
        s["sent"][format!("control:USAZ001:{:032x}", 23)]["outcome"],
        "unknown-codename"
    );
    // Stop for a second approved native publication with identical content.
    let mut signal = Command::new("kill");
    signal.args(["-INT", &daemon.0.id().to_string()]);
    checked(signal);
    let mut daemon = daemon;
    assert!(daemon.0.wait().unwrap().success());
    drop(daemon);
    import_file(&b, &native_payload, "renamed.txt");
    post(&b, "Lost ACK native", None);
    let daemon = start(&b);
    let mut cfg: Value =
        serde_json::from_slice(&fs::read(root.join("peer.json")).unwrap()).unwrap();
    cfg["drop_ack_once"] = json!(true);
    save(&root.join("peer.json"), &cfg);
    let ready = root.join("ready");
    let mut listener = peer_command(&root, peer_port, None);
    listener.args([
        "--listen",
        "--sessions",
        "2",
        "--ready",
        ready.to_str().unwrap(),
    ]);
    let mut listener = Running(listener.spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
    cn(&b, "poll", &["USAZ001"]);
    wait_peer(&b);
    assert!(listener.0.wait().unwrap().success());
    let r: Value =
        serde_json::from_slice(&fs::read(root.join("peer-report.json")).unwrap()).unwrap();
    assert_eq!(r["result"], "ok");
    assert_eq!(r["duplicates"], 1);
    assert_eq!(r["hash_have_received"], 1);
    assert_eq!(r["payload_received"], 0);
    let mut signal = Command::new("kill");
    signal.args(["-INT", &daemon.0.id().to_string()]);
    checked(signal);
    let mut daemon = daemon;
    assert!(daemon.0.wait().unwrap().success());
}
