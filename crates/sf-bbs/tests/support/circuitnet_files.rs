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

//! Independent disposable macOS boards exercise the real C6 TLS and Event paths.
use super::*;
use sf_core::files::{AdmissionStatus, SafetyPolicy, ScanPolicy};
use std::io::Cursor;
fn storage(b: &Board) -> FileStorage {
    let c = RuntimeConfig::load(&b.config).unwrap();
    FileStorage::new(
        &LogicalPaths::resolve(b.config.parent().unwrap(), &c.validate().unwrap()).unwrap(),
    )
    .unwrap()
}
fn file_area(b: &Board) -> FileArea {
    db(b)
        .all_file_areas()
        .unwrap()
        .into_iter()
        .find(|a| a.number == 77)
        .unwrap()
}
fn configure_files(b: &Board) {
    let mut d = db(b);
    let a = d
        .create_file_area(&FileAreaDefinition {
            number: 77,
            name: "Synthetic Files".into(),
            description: "Harmless fixture publications".into(),
            storage_key: "c6-files".into(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            upload_security: SecurityLevel::new(0).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: vec![],
        })
        .unwrap();
    d.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        a.id,
        &SafetyPolicy {
            scanning: ScanPolicy::Disabled,
            approval_required: false,
            ..SafetyPolicy::default()
        },
    )
    .unwrap();
    drop(d);
    run(b, "file-map", &["CNFILES", "77", "yes", "yes", "1048576"]);
    let p = db(b).circuitnet_status(&network()).unwrap().profile;
    for n in p.topology.neighbors(&p.local).unwrap() {
        run(b, "file-subscribe", &[n.as_str(), "CNFILES", "yes"]);
    }
}
fn import_file(b: &Board, name: &str, bytes: &[u8]) -> FileEntry {
    storage(b)
        .import_file(
            &mut db(b),
            FileAdminActor::LocalOperator,
            &file_area(b),
            name,
            "Synthetic original",
            &mut Cursor::new(bytes),
            "operator",
            None,
        )
        .unwrap()
        .file
}
fn file_count(b: &Board) -> usize {
    db(b).file_count(file_area(b).id).unwrap() as usize
}
fn files_state(b: &Board) -> sf_core::circuitnet::files::Status {
    db(b).circuitnet_file_status(&network()).unwrap()
}
fn native_files_journey(b: &Board, root: &Path) {
    use sf_core::files::scanner::{ScanReport, ScanResult, Scanner};
    struct Mock(ScanResult);
    impl Scanner for Mock {
        fn scan(&self, _: &mut dyn std::io::Read) -> ScanReport {
            ScanReport::new("harmless-synthetic", self.0.clone())
        }
    }
    let command = |args: &[&str]| {
        sf_bbs::files::run(
            &b.config,
            &args
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
        )
        .unwrap()
    };
    command(&["area", "78", "local-c6", "Local fixture files"]);
    command(&["policy", "78", "disabled", "automatic"]);
    let area = db(b)
        .all_file_areas()
        .unwrap()
        .into_iter()
        .find(|a| a.number == 78)
        .unwrap();
    let source = root.join("safe-fixture.txt");
    fs::write(&source, b"immutable original").unwrap();
    let result = command(&[
        "import",
        "78",
        source.to_str().unwrap(),
        "FIRST.TXT",
        "Operator description",
    ]);
    assert!(result.contains("duplicate=false"));
    assert!(command(&[
        "import",
        "78",
        source.to_str().unwrap(),
        "OTHER.TXT",
        "Other label"
    ])
    .contains("duplicate=true"));
    fs::write(&source, b"changed source bytes").unwrap();
    assert!(sf_bbs::files::run(
        &b.config,
        &[
            "import",
            "78",
            source.to_str().unwrap(),
            "FIRST.TXT",
            "collision"
        ]
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>()
    )
    .is_err());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("file_id.diz", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"Suggested description\r\nSafe line\x1b[31m")
        .unwrap();
    let original = zip.finish().unwrap().into_inner();
    let store = storage(b);
    let diz = store
        .import_file(
            &mut db(b),
            FileAdminActor::LocalOperator,
            &area,
            "DIZ.ZIP",
            "Original description",
            &mut Cursor::new(&original),
            "operator",
            None,
        )
        .unwrap()
        .file;
    let record = db(b).file_admission(diz.id).unwrap().unwrap();
    let suggestion = record.report.unwrap().diz.unwrap();
    assert!(!suggestion.suggestion.contains('\u{1b}'));
    assert!(suggestion.original.contains(&27));
    command(&["description", &diz.id.get().to_string(), "use"]);
    command(&[
        "description",
        &diz.id.get().to_string(),
        "edit",
        "Reviewed description",
    ]);
    let mut copy = Vec::new();
    std::io::Read::read_to_end(
        &mut store.open_content(&diz.sha256, diz.size_bytes).unwrap(),
        &mut copy,
    )
    .unwrap();
    assert_eq!(copy, original);
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(4);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "SAFE.TXT", Cursor::new(b"safe"))
        .unwrap();
    let tar = tar.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar).unwrap();
    for (name, bytes) in [("SAFE.TAR", tar), ("SAFE.TGZ", gz.finish().unwrap())] {
        let f = store
            .import_file(
                &mut db(b),
                FileAdminActor::LocalOperator,
                &area,
                name,
                "safe",
                &mut Cursor::new(bytes),
                "operator",
                None,
            )
            .unwrap()
            .file;
        assert_eq!(f.lifecycle, FileLifecycle::Active);
    }
    let mut bad = zip::ZipWriter::new(Cursor::new(Vec::new()));
    bad.start_file("../escape", zip::write::SimpleFileOptions::default())
        .unwrap();
    bad.write_all(b"harmless traversal fixture").unwrap();
    let bad = store
        .import_file(
            &mut db(b),
            FileAdminActor::LocalOperator,
            &area,
            "BAD.ZIP",
            "quarantine fixture",
            &mut Cursor::new(bad.finish().unwrap().into_inner()),
            "operator",
            None,
        )
        .unwrap()
        .file;
    assert_eq!(
        db(b).file_admission(bad.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
    command(&["policy", "78", "required", "approval"]);
    let detected = store
        .import_file(
            &mut db(b),
            FileAdminActor::LocalOperator,
            &area,
            "MOCK.TXT",
            "harmless",
            &mut Cursor::new(b"no malware"),
            "operator",
            Some(&Mock(ScanResult::MalwareDetected)),
        )
        .unwrap()
        .file;
    assert_eq!(
        db(b).file_admission(detected.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
    assert_eq!(
        store
            .rescan_file(
                &mut db(b),
                FileAdminActor::LocalOperator,
                detected.id,
                Some(&Mock(ScanResult::Clean))
            )
            .unwrap()
            .status,
        AdmissionStatus::PendingApproval
    );
    command(&["approve", &detected.id.get().to_string()]);
    assert!(command(&["integrity"]).contains("verified"));
    // Caller FileBackend sees published entries only, through existing area access rules.
    let d = db(b);
    let c = d.caller_by_name(b"Sysop").unwrap().unwrap();
    let caller = FileActor::new(c.id, SecurityLevel::new(9999).unwrap());
    assert!(d
        .files(caller, area.id)
        .unwrap()
        .iter()
        .all(|f| f.lifecycle == FileLifecycle::Active));
}
fn c6_raw_begin(from: &Board, to: &Board) -> Raw {
    let mut raw = raw(from, to);
    let p = db(from).circuitnet_status(&network()).unwrap().profile;
    let h = Hello::new(
        network(),
        p.local.clone(),
        p.topology.node(&p.local).unwrap().role,
        Mode::Poll,
    );
    wire::write(&mut raw, &Frame::Hello { hello: h }).unwrap();
    assert!(matches!(
        wire::read(&mut raw, wire::CONTROL_FRAME),
        Ok(Frame::Hello { .. })
    ));
    c4_controls(&mut raw, vec![]);
    wire::write(&mut raw, &Frame::Offer { batch: None }).unwrap();
    raw
}
fn offer(b: &Board, to: &Board, name: &str) -> envelope::files::Publication {
    db(b)
        .circuitnet_file_offers(
            &network(),
            &envelope::NodeId::new(&to.id).unwrap(),
            chrono::Utc::now().timestamp(),
        )
        .unwrap()
        .into_iter()
        .find(|p| p.filename == name)
        .unwrap()
}
#[test]
fn real_macos_c6_files_six_nodes_events_recovery_and_backup() {
    use sf_core::events::{Action, Definition, ExchangePolicy, MissedPolicy, Schedule};
    let _campaign = CAMPAIGN.lock().unwrap_or_else(|p| p.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("SPITFIRE_C6_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("c6"));
    assert!(!root.exists());
    fs::create_dir(&root).unwrap();
    let r = board_tree(&root.join("root"), "ROOT1", 10, Some(11), true);
    let h1 = board_tree(&root.join("host1"), "HOST1", 20, Some(21), true);
    let h2 = board_tree(&root.join("host2"), "HOST2", 30, Some(31), true);
    let e1 = board_tree(&root.join("end1"), "END1", 40, Some(41), true);
    let e2 = board_tree(&root.join("end2"), "END2", 50, Some(51), true);
    let e3 = board_tree(&root.join("end3"), "END3", 60, Some(61), true);
    for b in [&r, &h1, &h2, &e1, &e2, &e3] {
        configure_files(b);
    }
    native_files_journey(&e1, &root);
    for (a, b) in [(&r, &h1), (&r, &h2), (&h1, &e1), (&h1, &e2), (&h2, &e3)] {
        enroll(a, b);
        enroll(b, a);
    }
    let rd = start(&r);
    let mut hd1 = start(&h1);
    let hd2 = start(&h2);
    let mut ed1 = start(&e1);
    let ed2 = start(&e2);
    let mut ed3 = start(&e3);
    for (a, b) in [(&e1, &h1), (&e2, &h1), (&h1, &r), (&h2, &r), (&e3, &h2)] {
        poll(a, b, true);
    }
    let first = import_file(&e1, "FIRST.TXT", b"harmless native file publication");
    eventually("immediate durable preparation", || {
        !files_state(&e1).deliveries.is_empty()
    });
    assert_eq!(file_count(&h1), 0);
    // Intermediate durable acceptance, then loss of the receipt.
    let publication = offer(&e1, &h1, "FIRST.TXT");
    {
        let mut raw = c6_raw_begin(&e1, &h1);
        wire::write(
            &mut raw,
            &Frame::FileOffer {
                publication: Some(publication.clone()),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut raw, envelope::files::METADATA_FRAME),
            Ok(Frame::FileWant {
                want: envelope::files::Want::Send
            })
        ));
        let mut bytes = storage(&e1)
            .open_content(&first.sha256, first.size_bytes)
            .unwrap();
        envelope::files::send(&mut bytes, &mut raw, first.size_bytes).unwrap();
        eventually("intermediate accepted before lost receipt", || {
            file_count(&h1) == 1
        });
    }
    wait_idle(&h1, &e1);
    poll(&e1, &h1, false);
    assert_eq!(file_count(&h1), 1);
    assert_eq!(
        status(&e1)
            .peers
            .iter()
            .find(|p| p.node.as_str() == h1.id)
            .unwrap()
            .health
            .as_ref()
            .unwrap()
            .file_bytes,
        0
    );
    poll(&h1, &e2, false);
    poll(&h1, &r, false);
    poll(&r, &h2, false);
    poll(&h2, &e3, false);
    for b in [&h1, &e2, &r, &h2, &e3] {
        assert_eq!(file_count(b), 1);
        assert_eq!(
            db(b)
                .file_admissions(FileAdminActor::LocalOperator)
                .unwrap()[0]
                .sha256,
            first.sha256
        );
    }
    assert!(files_state(&h1)
        .deliveries
        .iter()
        .all(|d| d.neighbor.as_str() != "END1"));
    // Same content under a new publication filename uses hash-have.
    import_file(&e1, "OTHER.TXT", b"harmless native file publication");
    poll(&e1, &h1, false);
    assert_eq!(file_count(&h1), 2);
    assert_eq!(
        status(&e1)
            .peers
            .iter()
            .find(|p| p.node.as_str() == h1.id)
            .unwrap()
            .health
            .as_ref()
            .unwrap()
            .file_bytes,
        0
    );
    // A truncated binary payload leaves no native publication; the real daemon retries from zero.
    let interrupted = import_file(&e1, "PART.TXT", &vec![b'P'; 200000]);
    let part = offer(&e1, &h1, "PART.TXT");
    {
        let mut raw = c6_raw_begin(&e1, &h1);
        wire::write(
            &mut raw,
            &Frame::FileOffer {
                publication: Some(part),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut raw, envelope::files::METADATA_FRAME),
            Ok(Frame::FileWant {
                want: envelope::files::Want::Send
            })
        ));
        raw.write_all(&65536u32.to_be_bytes()).unwrap();
        raw.write_all(b"incomplete chunk").unwrap();
    }
    wait_idle(&h1, &e1);
    assert_eq!(file_count(&h1), 2);
    poll(&e1, &h1, false);
    assert_eq!(file_count(&h1), 3);
    assert!(storage(&h1)
        .open_content(&interrupted.sha256, interrupted.size_bytes)
        .is_ok());
    // Corrupt bytes are rejected durably without native import.
    import_file(&e1, "CORRUPT.TXT", b"good");
    let corrupt = offer(&e1, &h1, "CORRUPT.TXT");
    {
        let mut raw = c6_raw_begin(&e1, &h1);
        wire::write(
            &mut raw,
            &Frame::FileOffer {
                publication: Some(corrupt),
            },
        )
        .unwrap();
        assert!(matches!(
            wire::read(&mut raw, envelope::files::METADATA_FRAME),
            Ok(Frame::FileWant {
                want: envelope::files::Want::Send
            })
        ));
        envelope::files::send(&mut Cursor::new(b"evil"), &mut raw, 4).unwrap();
        let Frame::FileReceipt { receipt } =
            wire::read(&mut raw, envelope::files::METADATA_FRAME).unwrap()
        else {
            panic!("file rejection receipt")
        };
        assert_eq!(receipt.reason, "hash-mismatch");
    }
    wait_idle(&h1, &e1);
    poll(&e1, &h1, false);
    assert_eq!(file_count(&h1), 3);
    // C5 Events own all file exchange timing.
    let mut event = Definition {
        id: "c6-exchange".into(),
        name: "CircuitNET Files".into(),
        enabled: true,
        action: Action::Circuitnet {
            network: network(),
            node: Some(envelope::NodeId::new("HOST1").unwrap()),
        },
        schedule: Schedule::Interval { seconds: 15 },
        timezone: "America/Phoenix".into(),
        policy: ExchangePolicy::Scheduled,
        missed: MissedPolicy::RunOnce,
        minimum_spacing_seconds: 5,
    };
    save_event(&e1, &event);
    import_file(&e1, "SCHEDULE.TXT", b"scheduled payload");
    eventually("scheduled file distribution", || file_count(&h1) == 4);
    event.policy = ExchangePolicy::Manual;
    save_event(&e1, &event);
    import_file(&e1, "MANUAL.TXT", b"manual payload");
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(file_count(&h1), 4);
    event_command(&e1, &["run", "c6-exchange"]);
    eventually("manual Run Now", || file_count(&h1) == 5);
    event.policy = ExchangePolicy::Immediate;
    save_event(&e1, &event);
    for n in 0..12 {
        import_file(
            &e1,
            &format!("BURST{n}.TXT"),
            format!("synthetic burst {n}").as_bytes(),
        );
    }
    eventually("bounded file batches drain", || file_count(&h1) == 17);
    event.policy = ExchangePolicy::Hybrid;
    save_event(&e1, &event);
    run(&e1, "hold", &["HOST1"]);
    import_file(&e1, "HELD.TXT", b"held native file");
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(file_count(&h1), 17);
    run(&e1, "release", &["HOST1"]);
    eventually("release catches up", || file_count(&h1) == 18);
    stop(hd1, &h1);
    import_file(&e1, "OFFLINE.TXT", b"queue survives offline peer");
    event_command(&e1, &["run", "c6-exchange"]);
    eventually("file event failure preserves work", || {
        db(&e1).events().unwrap()[0].last_result.as_deref() == Some("failed")
    });
    assert!(files_state(&e1)
        .deliveries
        .iter()
        .any(|d| d.receipt.is_none()));
    hd1 = start(&h1);
    eventually("file peer returns", || file_count(&h1) == 19);

    event.policy = ExchangePolicy::Manual;
    save_event(&e1, &event);
    import_file(&e1, "RESTART.TXT", b"restart pending");
    stop(ed1, &e1);
    ed1 = start(&e1);
    assert_eq!(file_count(&h1), 19);
    poll(&e1, &h1, false);
    assert_eq!(file_count(&h1), 20);
    // Receiver-required scanning quarantines incoming files instead of trusting the sender.
    stop(ed3, &e3);
    let policy = SafetyPolicy::default();
    db(&e3)
        .set_file_safety_policy(FileAdminActor::LocalOperator, file_area(&e3).id, &policy)
        .unwrap();
    ed3 = start(&e3);
    poll(&h1, &r, false);
    poll(&r, &h2, false);
    poll(&h2, &e3, false);
    assert!(db(&e3)
        .file_admissions(FileAdminActor::LocalOperator)
        .unwrap()
        .iter()
        .any(|a| a.status == AdmissionStatus::Quarantined));
    // Same live session still carries C4 directed messages and remote Dossier controls.
    directed(&e1, "END3", "C6 directed regression");
    poll(&e1, &h1, false);
    poll(&h1, &r, false);
    poll(&r, &h2, false);
    poll(&h2, &e3, false);
    assert_eq!(count(&e3, false), 1);
    assert_eq!(count(&e2, false), 0);
    run(&e3, "remote-subscribe", &["CNTECH"]);
    poll(&e3, &h2, false);
    assert!(status(&h2).pending_approvals > 0);
    stop(ed1, &e1);
    let backup = root.join("backup");
    sf_bbs::backup_board(&e1.config, &backup).unwrap();
    let original_count = file_count(&e1);
    db(&e1)
        .review_file_admission(FileAdminActor::LocalOperator, first.id, false)
        .unwrap();
    assert_eq!(file_count(&e1), original_count - 1);
    let restored_root = root.join("restored");
    sf_bbs::restore_board(&backup, &restored_root, false).unwrap();
    let cfg = restored_root.join(sf_bbs::BOARD_CONFIG_FILE);
    let config = RuntimeConfig::load(&cfg).unwrap();
    let paths = LogicalPaths::resolve(&restored_root, &config.validate().unwrap()).unwrap();
    let restored = Board {
        config: cfg,
        db: paths.database().to_owned(),
        ..e1.clone()
    };
    assert_eq!(file_count(&restored), original_count);
    assert!(storage(&restored)
        .check_content(&db(&restored))
        .unwrap()
        .iter()
        .all(|c| c.status == "verified"));
    assert_eq!(
        files_state(&restored).deliveries.len(),
        files_state(&e1).deliveries.len()
    );
    let restored_daemon = start(&restored);
    poll(&restored, &h1, false);
    assert_eq!(file_count(&h1), 20);
    db(&h1)
        .circuitnet_file_subscribe(
            "synthetic-operator",
            &network(),
            &sf_core::circuitnet::files::FileDossier {
                neighbor: envelope::NodeId::new("END2").unwrap(),
                codename: envelope::Codename::new("CNFILES").unwrap(),
                subscribed: false,
                version: 1,
            },
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    let root_file = import_file(&r, "ROOTFILE.TXT", b"root native origin");
    poll(&r, &h1, false);
    poll(&h1, &restored, false);
    poll(&e2, &h1, false);
    assert_eq!(file_count(&restored), original_count + 1);
    assert_eq!(file_count(&e2), 1);
    assert!(storage(&restored)
        .open_content(&root_file.sha256, root_file.size_bytes)
        .is_ok());
    stop(restored_daemon, &restored);
    stop(ed3, &e3);
    stop(ed2, &e2);
    stop(hd2, &h2);
    stop(hd1, &h1);
    stop(rd, &r);
    fs::write(root.join("acceptance.txt"),"PASS: six independent native daemons; TLS; native content authority; sibling/cross-ROOT file fanout; preserved content; hash-have; intermediate receipt loss; truncated transfer retry; corrupt payload durable rejection; Scheduled/Manual/Immediate/Hybrid Events; burst drain; hold/release; restart; receiver quarantine; C4 directed/control regression; payload-aware backup/restore; graceful shutdown. Disposable loopback only.\n").unwrap();
}
