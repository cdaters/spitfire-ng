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

//! C2 real native boards, fresh operator processes, file transport and cold restore.
use sf_core::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const NET: &str = "circuitnet-test";
struct Board {
    config: PathBuf,
    db: PathBuf,
    id: String,
    test: u16,
    tech: Option<u16>,
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
    let b = Board {
        config,
        db: paths.database().to_owned(),
        id: id.into(),
        test,
        tech,
    };
    run(
        &b,
        "init",
        &[
            id,
            "Synthetic CircuitNET",
            "--trusted-offline",
            "HOST0001:HOST:-",
            "END00001:END:HOST0001",
            "END00002:END:HOST0001",
        ],
    );
    run(&b, "map", &[&test.to_string(), "CNTEST"]);
    if let Some(n) = tech {
        run(&b, "map", &[&n.to_string(), "CNTECH"]);
    }
    if id == "HOST0001" {
        for (n, c) in [
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
    let mut db = db(b);
    let actor = actor(&db);
    let c = db
        .conference(actor, if tech { b.tech.unwrap() } else { b.test })
        .unwrap();
    let parent = parent.map(|n| db.message(actor, c.id, n).unwrap().id);
    let preview = db.preview_posting_identity(actor, c.id).unwrap();
    db.post(
        actor,
        NewMessage {
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
        },
    )
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
fn exchange(from: &Board, to: &Board, root: &Path, name: &str) -> PathBuf {
    let packet = root.join(format!("{name}.json"));
    let receipt = root.join(format!("{name}.receipt.json"));
    run(from, "export", &[&to.id, packet.to_str().unwrap()]);
    run(
        to,
        "import",
        &[
            &from.id,
            packet.to_str().unwrap(),
            receipt.to_str().unwrap(),
        ],
    );
    run(from, "ack", &[&to.id, receipt.to_str().unwrap()]);
    packet
}
#[test]
fn macos_native_offline_three_board_journey_restart_and_cold_restore() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C2_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("evidence"));
    fs::create_dir(&root).unwrap();
    let host = board(&root.join("host"), "HOST0001", 17, Some(18));
    let e1 = board(&root.join("end1"), "END00001", 4, Some(5));
    let e2 = board(&root.join("end2"), "END00002", 9, None);
    post(&e1, false, "Journey one", None);
    run(&e1, "scan", &[]);
    let original = exchange(&e1, &host, &root, "e1-host");
    exchange(&host, &e2, &root, "host-e2");
    assert_eq!(
        (count(&e1, false), count(&host, false), count(&e2, false)),
        (1, 1, 1)
    );
    assert!(!command(
        &host,
        "export",
        &[&e1.id, root.join("reflection.json").to_str().unwrap()]
    )
    .status
    .success());
    post(&host, true, "CNTECH subscribers", None);
    run(&host, "scan", &[]);
    exchange(&host, &e1, &root, "tech-e1");
    assert_eq!(count(&e1, true), 1);
    assert!(!command(
        &host,
        "export",
        &[&e2.id, root.join("not-subscribed.json").to_str().unwrap()]
    )
    .status
    .success());
    post(&e2, false, "Journey three", None);
    run(&e2, "scan", &[]);
    exchange(&e2, &host, &root, "e2-host");
    exchange(&host, &e1, &root, "host-e1");
    post(&e1, false, "Thread reply", Some(2));
    run(&e1, "scan", &[]);
    exchange(&e1, &host, &root, "reply-host");
    exchange(&host, &e2, &root, "reply-e2");
    for b in [&host, &e1, &e2] {
        let db = db(b);
        let a = actor(&db);
        let c = db.conference(a, b.test).unwrap();
        let reply = db.message(a, c.id, 3).unwrap();
        assert_eq!(
            reply.parent_message_id,
            Some(db.message(a, c.id, 2).unwrap().id)
        );
        assert_eq!(count(b, false), 3);
    }
    let replay_receipt = root.join("replay.receipt.json");
    run(
        &host,
        "import",
        &[
            &e1.id,
            original.to_str().unwrap(),
            replay_receipt.to_str().unwrap(),
        ],
    );
    run(&e1, "ack", &[&host.id, replay_receipt.to_str().unwrap()]);
    assert_eq!(count(&host, false), 3);
    let before = db(&host).circuitnet_status(&network()).unwrap().accepted;
    post(&host, false, "Partial delivery", None);
    run(&host, "scan", &[]);
    exchange(&host, &e1, &root, "partial-e1");
    let existing = root.join("exists.json");
    fs::write(&existing, b"do not overwrite").unwrap();
    assert!(
        !command(&host, "export", &[&e2.id, existing.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(fs::read(&existing).unwrap(), b"do not overwrite");
    let status = db(&host).circuitnet_status(&network()).unwrap();
    assert_eq!(status.accepted, before + 1);
    assert_eq!(status.pending, 1);
    exchange(&host, &e2, &root, "partial-retry-e2");
    assert_eq!(
        db(&host).circuitnet_status(&network()).unwrap().accepted,
        before + 2
    );
    run(&host, "unsubscribe", &[&e2.id, "CNTEST"]);
    post(&host, false, "After Dossier removal", None);
    run(&host, "scan", &[]);
    exchange(&host, &e1, &root, "after-removal");
    assert_eq!(count(&e2, false), 4);
    assert_eq!(count(&e1, false), 5);
    post(&host, false, "Frozen pending sender", None);
    run(&host, "scan", &[]);
    let frozen = root.join("frozen.json");
    run(&host, "export", &[&e1.id, frozen.to_str().unwrap()]);
    let database = db(&host);
    let caller = database.caller_by_name(b"Sysop").unwrap().unwrap();
    let mut p = caller.profile.clone();
    p.identity = PrivateIdentity::new(
        Some("PrivateCanaryFirst".into()),
        Some("PrivateCanaryLast".into()),
    )
    .unwrap();
    database
        .update_caller_profile_versioned(
            caller.id,
            caller.state_version,
            p,
            &CallerProfilePolicy::default(),
            identity::IdentityEditActor::LocalOperator,
            1788800001,
        )
        .unwrap();
    drop(database);
    let retried = root.join("frozen-retry.json");
    run(&host, "export", &[&e1.id, retried.to_str().unwrap()]);
    assert_eq!(fs::read(&frozen).unwrap(), fs::read(&retried).unwrap());
    assert!(!String::from_utf8(fs::read(&frozen).unwrap())
        .unwrap()
        .contains("PrivateCanary"));
    let backup = root.join("backup");
    sf_bbs::backup_board(&host.config, &backup).unwrap();
    let restored_root = root.join("restored");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let config = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let cfg = RuntimeConfig::load(&config).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &cfg.validate().unwrap()).unwrap();
    let restored = Board {
        config,
        db: paths.database().to_owned(),
        id: host.id.clone(),
        test: host.test,
        tech: host.tech,
    };
    let s = db(&restored).circuitnet_status(&network()).unwrap();
    assert_eq!(
        s.accepted,
        db(&host).circuitnet_status(&network()).unwrap().accepted
    );
    assert_eq!(s.pending, 1);
    assert_eq!(s.profile.local.as_str(), "HOST0001");
    assert!(
        !s.dossiers
            .iter()
            .find(|d| d.neighbor.as_str() == "END00002")
            .unwrap()
            .subscribed
    );
    assert!(!command(
        &restored,
        "export",
        &[&e1.id, root.join("held.json").to_str().unwrap()]
    )
    .status
    .success());
    let q = db(&restored)
        .circuitnet_queue(&network(), "")
        .unwrap()
        .into_iter()
        .find(|q| q.state == "held")
        .unwrap();
    run(&restored, "retry", &[&q.id, &q.version.to_string()]);
    exchange(&restored, &e1, &root, "restored-delivery");
    assert_eq!(count(&e1, false), 6);
    assert!(!command(
        &restored,
        "export",
        &[&e1.id, root.join("false-resend.json").to_str().unwrap()]
    )
    .status
    .success());
    let conn = rusqlite::Connection::open_with_flags(
        &restored.db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let origins: Vec<String> = conn
        .prepare("SELECT origin FROM circuitnet_messages ORDER BY message_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        &origins[..4],
        &["END00001", "HOST0001", "END00002", "END00001"]
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    fs::write(root.join("acceptance.txt"),"PASS: independent native HOST/END1/END2; distinct conference numbers; CNTEST/CNTECH; origin; sibling fanout; no reflection; thread; replay; Dossier removal; partial failure/retry; fresh processes; native cold backup/restore; per-neighbor acknowledgement; immutable sender; no live CircuitNET transport or production traffic.\n").unwrap();
}
