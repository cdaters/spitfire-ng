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

//! Four real loopback daemons exercise file adapters through authenticated BinkP.
use super::*;
use sf_core::ftn::files::*;
fn configure(b: &HubBoard, subscriptions: &[(&str, u8)]) {
    let mut db = RuntimeDatabase::open(b.board.paths.database()).unwrap();
    let mut p = db.file_network_policy().unwrap();
    p.enabled = true;
    p.freq = true;
    db.configure_file_network("operator", &p, now()).unwrap();
    let native = db.all_file_areas().unwrap();
    for a in &native {
        db.update_file_area(
            a.number,
            &FileAreaDefinition {
                number: a.number,
                name: a.name.clone(),
                description: a.description.clone(),
                storage_key: a.storage_key.clone(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(0).unwrap(),
                upload_security: a.upload_security,
                preview: a.preview,
                no_charge: a.no_charge,
                maximum_upload_bytes: a.maximum_upload_bytes,
                privileged_security_levels: a.privileged_security_levels.clone(),
            },
        )
        .unwrap();
    }

    for n in [1, 2] {
        db.configure_fileecho_area(
            &b.board.policy,
            "operator",
            &FileEchoArea {
                domain: "isolated".parse().unwrap(),
                tag: format!("FILEAREA{n}"),
                native_area: native.iter().find(|a| a.number == n).unwrap().id.get(),
                enabled: true,
                inbound: true,
                outbound: true,
                description: "Isolated FileEcho".into(),
                version: 0,
            },
            now(),
        )
        .unwrap();
    }
    for (link, n) in subscriptions {
        db.configure_file_subscription(
            &b.board.policy,
            "operator",
            &FileSubscription {
                link: (*link).into(),
                domain: "isolated".parse().unwrap(),
                tag: format!("FILEAREA{n}"),
                inbound: true,
                subscribed: true,
                held: false,
                version: 0,
            },
            now(),
        )
        .unwrap();
    }
}
fn native_file(b: &HubBoard, area: u16, name: &str, bytes: &[u8]) -> i64 {
    let mut db = RuntimeDatabase::open(b.board.paths.database()).unwrap();
    let storage = FileStorage::open_existing(&b.board.paths).unwrap();
    let area = db
        .all_file_areas()
        .unwrap()
        .into_iter()
        .find(|a| a.number == area)
        .unwrap();
    db.add_managed_file(
        &storage,
        FileAdminActor::LocalOperator,
        area.id,
        area.state_version,
        name,
        "Synthetic file-network journey",
        bytes,
    )
    .unwrap()
    .file
    .id
    .get()
}
async fn hatch_file(c: &mut OperatorClient, id: i64, area: u8) {
    let request = sf_bbs::ftn::FileAction::Preview {
        file: id,
        domain: "isolated".parse().unwrap(),
        tag: format!("FILEAREA{area}"),
    };
    let result = c
        .qwk_network_action(
            format!("{:032x}", rand::random::<u128>()),
            NetworkAction::Ftn {
                request: sf_bbs::ftn::Action::Files { request },
            },
        )
        .await
        .unwrap();
    let NetworkResult::Ftn {
        response: sf_bbs::ftn::Result::HatchPreview { preview },
    } = result
    else {
        panic!("preview failed: {result:?}")
    };
    ftn_action(
        c,
        sf_bbs::ftn::Action::Files {
            request: sf_bbs::ftn::FileAction::Hatch { preview },
        },
    )
    .await;
}
async fn credentials(c: &mut OperatorClient, links: &[&str]) {
    for link in links {
        secret(c, link, false).await;
        let expected = c.binkp_status().await.unwrap().policy;
        action(
            c,
            binkp::Action::TicCredential {
                link: (*link).into(),
                expected,
                secret: "FILEPASS".into(),
            },
        )
        .await;
    }
}
fn status(b: &HubBoard) -> FileStatus {
    RuntimeDatabase::open(b.board.paths.database())
        .unwrap()
        .file_network_status()
        .unwrap()
}
fn copies(b: &HubBoard, name: &str) -> usize {
    RuntimeDatabase::open(b.board.paths.database())
        .unwrap()
        .all_cataloged_files()
        .unwrap()
        .iter()
        .filter(|(_, f)| f.filename == name)
        .count()
}
#[test]
fn four_daemon_fileecho_hatch_freq_and_cold_restore() {
    let _journey = DAEMON_JOURNEY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let temp = tempfile::tempdir().unwrap();
    let hp = port();
    let up = port();
    let ap = port();
    let bp = port();
    let hub = make_board(
        &temp.path().join("hub"),
        "10:100/1@isolated",
        hp,
        &[
            ("up", "10:100/2@isolated", up),
            ("a", "10:100/3@isolated", ap),
            ("b", "10:100/4@isolated", bp),
        ],
    );
    let upstream = make_board(
        &temp.path().join("up"),
        "10:100/2@isolated",
        up,
        &[("hub", "10:100/1@isolated", hp)],
    );
    let a = make_board(
        &temp.path().join("a"),
        "10:100/3@isolated",
        ap,
        &[("hub", "10:100/1@isolated", hp)],
    );
    let b = make_board(
        &temp.path().join("b"),
        "10:100/4@isolated",
        bp,
        &[("hub", "10:100/1@isolated", hp)],
    );
    configure(&hub, &[("up", 1), ("up", 2), ("a", 1), ("b", 1), ("b", 2)]);
    configure(&upstream, &[("hub", 1), ("hub", 2)]);
    configure(&a, &[("hub", 1)]);
    configure(&b, &[("hub", 1), ("hub", 2)]);
    let first = native_file(&hub, 1, "HATCH.ZIP", b"isolated hatch opaque zip");
    let incoming = native_file(&upstream, 1, "UPFILE.ZIP", b"isolated upstream");
    let second = native_file(&upstream, 2, "SECOND.ZIP", b"second FileEcho");
    let from_a = native_file(&a, 1, "AFILE.ZIP", b"downstream origin");
    let hd = start(&hub.board.config);
    let ud = start(&upstream.board.config);
    let ad = start(&a.board.config);
    let mut bd = Some(start(&b.board.config));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut hc = connect(&hub.board.config).await;
        let mut uc = connect(&upstream.board.config).await;
        let mut ac = connect(&a.board.config).await;
        let mut bc = connect(&b.board.config).await;
        credentials(&mut hc, &["up", "a", "b"]).await;
        credentials(&mut uc, &["hub"]).await;
        credentials(&mut ac, &["hub"]).await;
        credentials(&mut bc, &["hub"]).await;
        hatch_file(&mut hc, first, 1).await;
        for link in ["a", "b", "up"] {
            exchange(&mut hc, link, true).await;
        }
        assert_eq!(
            [
                copies(&hub, "HATCH.ZIP"),
                copies(&a, "HATCH.ZIP"),
                copies(&b, "HATCH.ZIP")
            ],
            [1, 1, 1]
        );
        assert!(status(&hub)
            .queue
            .iter()
            .all(|q| q.payload_accepted && q.tic_accepted));
        hatch_file(&mut uc, incoming, 1).await;
        exchange(&mut uc, "hub", true).await;
        exchange(&mut hc, "a", true).await;
        exchange(&mut hc, "b", true).await;
        assert_eq!(
            [
                copies(&hub, "UPFILE.ZIP"),
                copies(&a, "UPFILE.ZIP"),
                copies(&b, "UPFILE.ZIP")
            ],
            [1, 1, 1]
        );
        assert!(status(&hub)
            .queue
            .iter()
            .filter(|q| q.filename == "UPFILE.ZIP")
            .all(|q| q.link != "up"));
        hatch_file(&mut uc, second, 2).await;
        exchange(&mut uc, "hub", true).await;
        exchange(&mut hc, "b", true).await;
        exchange(&mut hc, "a", true).await;
        assert_eq!([copies(&a, "SECOND.ZIP"), copies(&b, "SECOND.ZIP")], [0, 1]);
        hatch_file(&mut ac, from_a, 1).await;
        exchange(&mut ac, "hub", true).await;
        exchange(&mut hc, "up", true).await;
        exchange(&mut hc, "b", true).await;
        assert_eq!(copies(&upstream, "AFILE.ZIP"), 1);
        assert!(status(&hub)
            .queue
            .iter()
            .filter(|q| q.filename == "AFILE.ZIP")
            .all(|q| q.link != "a"));
        exchange(&mut hc, "a", true).await;
        exchange(&mut hc, "b", true).await;
        assert_eq!(copies(&a, "UPFILE.ZIP"), 1);
        let requested = native_file(&hub, 1, "REQUEST.ZIP", b"explicitly approved FREQ response");
        ftn_action(
            &mut hc,
            sf_bbs::ftn::Action::Files {
                request: sf_bbs::ftn::FileAction::Grant {
                    value: FreqGrant {
                        link: "a".into(),
                        name: "REQUEST.ZIP".into(),
                        file: requested,
                        enabled: true,
                        version: 0,
                    },
                },
            },
        )
        .await;
        let area = status(&a).areas[0].native_area;
        ftn_action(
            &mut ac,
            sf_bbs::ftn::Action::Files {
                request: sf_bbs::ftn::FileAction::Request {
                    link: "hub".into(),
                    names: vec!["REQUEST.ZIP".into()],
                    native_area: area,
                },
            },
        )
        .await;
        exchange(&mut ac, "hub", true).await;
        exchange(&mut hc, "a", true).await;
        assert_eq!(copies(&a, "REQUEST.ZIP"), 1);
        assert_eq!(copies(&b, "REQUEST.ZIP"), 0);
        assert!(status(&a)
            .activity
            .iter()
            .any(|a| a.result == "freq-file-received"));
        // A held subscription retains future intent without offering any bytes.
        let mut subscription = status(&hub)
            .subscriptions
            .into_iter()
            .find(|s| s.link == "b" && s.tag == "FILEAREA1")
            .unwrap();
        subscription.held = true;
        ftn_action(
            &mut hc,
            sf_bbs::ftn::Action::Files {
                request: sf_bbs::ftn::FileAction::Subscription {
                    value: subscription.clone(),
                },
            },
        )
        .await;
        let held_file = native_file(&hub, 1, "HELD.ZIP", b"held subscription future intent");
        hatch_file(&mut hc, held_file, 1).await;
        exchange(&mut hc, "a", true).await;
        exchange(&mut hc, "b", true).await;
        assert_eq!([copies(&a, "HELD.ZIP"), copies(&b, "HELD.ZIP")], [1, 0]);
        assert!(status(&hub)
            .queue
            .iter()
            .any(|q| q.filename == "HELD.ZIP" && q.link == "b" && q.attempts == 0));
        subscription.version += 1;
        subscription.held = false;
        ftn_action(
            &mut hc,
            sf_bbs::ftn::Action::Files {
                request: sf_bbs::ftn::FileAction::Subscription {
                    value: subscription,
                },
            },
        )
        .await;
        exchange(&mut hc, "b", true).await;
        assert_eq!(copies(&b, "HELD.ZIP"), 1);
        // A succeeds while B is absent. Persist the split delivery truth.
        drop(bc);
        graceful(bd.take().unwrap());
        let partial = native_file(&hub, 1, "PARTIAL.ZIP", b"restore partial fanout");
        hatch_file(&mut hc, partial, 1).await;
        exchange(&mut hc, "a", true).await;
        exchange(&mut hc, "up", true).await;
        exchange(&mut hc, "b", false).await;
        let queues = status(&hub).queue;
        assert!(queues
            .iter()
            .filter(|q| q.filename == "PARTIAL.ZIP" && q.link == "a")
            .all(|q| q.payload_accepted && q.tic_accepted));
        assert!(queues
            .iter()
            .any(|q| q.filename == "PARTIAL.ZIP" && q.link == "b" && !q.payload_accepted));
        let snapshot = hc
            .networks(NetworkQuery {
                section: NetworkSection::Files,
                offset: 0,
            })
            .await
            .unwrap();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(snapshot.files.is_some());
        assert!(!json.contains("FILEPASS"));
        assert!(!json.contains(temp.path().to_str().unwrap()));
        drop(hc);
        drop(uc);
        drop(ac);
    });
    graceful(hd);
    graceful(ud);
    graceful(ad);
    let backup = temp.path().join("backup");
    sf_bbs::backup_board(&hub.board.config, &backup).unwrap();
    sf_bbs::restore_board(&backup, hub.board.paths.root(), true).unwrap();
    let restored = status(&hub);
    assert!(restored
        .queue
        .iter()
        .filter(|q| q.filename == "PARTIAL.ZIP" && q.link == "a")
        .all(|q| q.payload_accepted && q.tic_accepted));
    let hd = start(&hub.board.config);
    let ad = start(&a.board.config);
    let bd = start(&b.board.config);
    runtime.block_on(async {
        let mut hc = connect(&hub.board.config).await;
        let _ac = connect(&a.board.config).await;
        let _bc = connect(&b.board.config).await;
        for q in status(&hub)
            .queue
            .iter()
            .filter(|q| q.link == "b" && !q.payload_accepted)
        {
            ftn_action(
                &mut hc,
                sf_bbs::ftn::Action::Files {
                    request: sf_bbs::ftn::FileAction::Hold {
                        delivery: q.delivery.clone(),
                        expected: q.version,
                        held: false,
                    },
                },
            )
            .await;
        }
        exchange(&mut hc, "b", true).await;
        exchange(&mut hc, "a", true).await;
        assert_eq!(
            [copies(&a, "PARTIAL.ZIP"), copies(&b, "PARTIAL.ZIP")],
            [1, 1]
        );
    });
    graceful(hd);
    graceful(ad);
    graceful(bd);
}

#[test]
#[ignore = "explicit disposable N7 operator and independent peer acceptance"]
fn n7_operator_peer_driver() {
    let evidence = PathBuf::from(
        std::env::var_os("SPITFIRE_N7_EVIDENCE").expect("private evidence directory"),
    );
    fs::create_dir_all(&evidence).unwrap();
    let mode = std::env::var("SPITFIRE_N7_ACTION").unwrap_or_else(|_| "prepare".into());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    if mode == "prepare" {
        let root = tempfile::tempdir().unwrap().keep();
        let hub = make_board(
            &root.join("hub"),
            "10:100/1@isolated",
            34665,
            &[
                ("independent", "10:100/2@isolated", 34664),
                ("native", "10:100/3@isolated", 34666),
            ],
        );
        let native = make_board(
            &root.join("native"),
            "10:100/3@isolated",
            34666,
            &[("hub", "10:100/1@isolated", 34665)],
        );
        configure(
            &hub,
            &[
                ("independent", 1),
                ("independent", 2),
                ("native", 1),
                ("native", 2),
            ],
        );
        configure(&native, &[("hub", 1), ("hub", 2)]);
        let file = native_file(
            &hub,
            1,
            "NGFILE.ZIP",
            b"SPITFIRE independently authored opaque N7 payload\r\n",
        );
        let hd = start(&hub.board.config);
        let nd = start(&native.board.config);
        runtime.block_on(async {
            let mut hc = connect(&hub.board.config).await;
            let mut nc = connect(&native.board.config).await;
            credentials(&mut hc, &["independent", "native"]).await;
            credentials(&mut nc, &["hub"]).await;
            hatch_file(&mut hc, file, 1).await;
        });
        graceful(hd);
        graceful(nd);
        for (name, path) in [
            ("config-path", hub.board.config.as_path()),
            ("database-path", hub.board.paths.database()),
            ("native-config-path", native.board.config.as_path()),
            ("native-database-path", native.board.paths.database()),
        ] {
            fs::write(evidence.join(name), path.to_string_lossy().as_bytes()).unwrap();
        }
        return;
    }
    let config = PathBuf::from(fs::read_to_string(evidence.join("config-path")).unwrap());
    runtime.block_on(async {
        let mut client = connect(&config).await;
        if mode == "independent" || mode == "native" {
            exchange(&mut client, &mode, true).await;
        }
        if mode == "action" {
            let request: sf_bbs::ftn::FileAction =
                serde_json::from_slice(&fs::read(evidence.join("operator-action.json")).unwrap())
                    .unwrap();
            ftn_action(&mut client, sf_bbs::ftn::Action::Files { request }).await;
        }
        let snapshot = client
            .networks(NetworkQuery {
                section: NetworkSection::Files,
                offset: 0,
            })
            .await
            .unwrap();
        fs::write(
            evidence.join("status.json"),
            serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();
    });
}
