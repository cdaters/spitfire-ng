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

//! Five actual isolated native daemons: hub, upstream, two downstreams and point.
use super::*;
struct HubBoard {
    board: Board,
    second: ConferenceId,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
fn make_board(root: &Path, address: &str, listen: u16, links: &[(&str, &str, u16)]) -> HubBoard {
    let mut plan = sf_bbs::SetupPlan::stock_defaults("Synthetic N6 Board", "Sysop", "SYSOP", 2);
    plan.config.caller.password = PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    let principal = sf_bbs::current_operator_identity().unwrap();
    let mut capabilities = vec![
        LocalOperatorCapability::NetworkStatus,
        LocalOperatorCapability::NetworkRun,
        LocalOperatorCapability::NetworkTest,
        LocalOperatorCapability::NetworkQueue,
        LocalOperatorCapability::ReadConfiguration,
        LocalOperatorCapability::ChangeSensitiveConfiguration,
        LocalOperatorCapability::ChangeOnlineConfiguration,
    ];
    for cap in LocalOperatorCapability::READ_ONLY {
        if !capabilities.contains(&cap) {
            capabilities.push(cap)
        }
    }
    plan.config.operators.local_identities =
        vec![if let Some(uid) = principal.strip_prefix("unix-uid:") {
            LocalOperatorIdentity::Unix {
                uid: uid.parse().unwrap(),
                label: None,
                capabilities,
            }
        } else {
            LocalOperatorIdentity::Windows {
                sid: principal.strip_prefix("windows-sid:").unwrap().into(),
                label: None,
                capabilities,
            }
        }];
    plan.config.transports = vec![TransportConfig {
        name: Some("isolated-hub".into()),
        enabled: true,
        adapter: TransportAdapterConfig::Raw {
            listen: format!("127.0.0.1:{}", port()).parse().unwrap(),
            terminal: NetworkTerminalDefaults::default(),
        },
    }];
    let local: sf_net::ftn::Endpoint = address.parse().unwrap();
    let policy = Policy {
        enabled: true,
        akas: vec![Aka {
            id: "local".into(),
            endpoint: local.clone(),
            enabled: true,
            primary: true,
        }],
        links: links
            .iter()
            .map(|(id, address, _)| Link {
                posting_identity: Default::default(),
                id: (*id).into(),
                remote: address.parse().unwrap(),
                aka: "local".into(),
                enabled: true,
                inbound: true,
                outbound: true,
                transit: true,
                profile: sf_net::ftn::PacketProfile::Type2Plus,
                charset: sf_net::ftn::Charset::Utf8,
            })
            .collect(),
        routes: vec![Route {
            domain: local.domain.clone(),
            target: RouteMatch::Default,
            link: links[0].0.into(),
        }],
        sources: vec![],
    };
    plan.config.ftn = policy.clone();
    plan.config.binkp = BinkpPolicy {
        listener: Some(BinkpListener {
            enabled: true,
            bind: format!("127.0.0.1:{listen}").parse().unwrap(),
            akas: vec!["local".into()],
        }),
        links: links
            .iter()
            .map(|(id, _, port)| BinkpLink {
                link: (*id).into(),
                enabled: true,
                inbound: true,
                outbound: true,
                endpoint: Some("127.0.0.1".into()),
                port: *port,
                directory: false,
                akas: vec!["local".into()],
                remote_akas: vec![],
                auth: BinkpAuth::RequireCram,
                allow_domainless: false,
            })
            .collect(),
    };
    let setup = sf_bbs::setup_board(root, &plan, b"synthetic-setup-password").unwrap();
    let paths = LogicalPaths::resolve(root, &plan.config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
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
                now(),
            )
            .unwrap();
        actors.push(MessageActor::new(c.id, SecurityLevel::new(50).unwrap()));
    }
    db.configure_ftn_alias(
        &policy,
        "operator",
        &MailboxAlias {
            aka: "local".into(),
            alias: "Recipient".into(),
            caller_id: actors[0].caller_id().get(),
        },
        now(),
    )
    .unwrap();
    let mut areas = vec![];
    for n in [2, 3] {
        let c = db
            .ensure_conference(&ConferenceDefinition {
                posting_identity: None,
                number: n,
                name: format!("N6 Echo {n}"),
                description: "Isolated hub acceptance".into(),
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
                posting_identity: Default::default(),
                domain: local.domain.clone(),
                area: format!("AREA{}", n - 1),
                conference_id: c.id.get(),
                aka: "local".into(),
                receive: true,
                send: true,
                origin: "Synthetic hub journey".into(),
                links: vec![links[0].0.into()],
                version: 1,
            },
            0,
            now(),
        )
        .unwrap();
        areas.push(c.id);
    }
    HubBoard {
        board: Board {
            config: setup.config_path,
            paths,
            policy,
            actor: actors[0],
            other: actors[1],
            conference: areas[0],
        },
        second: areas[1],
    }
}
fn echo(b: &HubBoard, area: u8, seq: u32) {
    let b0 = &b.board;
    let mut db = RuntimeDatabase::open(b0.paths.database()).unwrap();
    db.bind_posting_identity_configuration(&RuntimeConfig::load(&b0.config).unwrap());
    db.post(
        b0.actor,
        NewMessage {
            identity_preview: None,
            conference_id: if area == 1 { b0.conference } else { b.second },
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: format!("N6 Echo {seq}").into_bytes(),
            body: format!("Native isolated hub message {seq}").into_bytes(),
            created_at: now(),
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        },
    )
    .unwrap();
    assert_eq!(db.scan_ftn(&b0.policy, now()).unwrap(), 1);
}
fn mail(b: &HubBoard, destination: &str, recipient: &str, subject: &str, body: &str) -> MessageId {
    let b = &b.board;
    RuntimeDatabase::open(b.paths.database())
        .unwrap()
        .send_ftn_mail(
            b.actor,
            &b.policy,
            &NewNetMail {
                identity_preview: None,
                aka: "local".into(),
                destination: destination.parse().unwrap(),
                recipient: recipient.into(),
                subject: subject.into(),
                body: body.into(),
                reply_to: None,
            },
            now(),
        )
        .unwrap()
}
fn count(b: &HubBoard, area: u8) -> u32 {
    rusqlite::Connection::open(b.board.paths.database())
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conference_id=?1",
            [if area == 1 {
                b.board.conference.get()
            } else {
                b.second.get()
            }],
            |r| r.get(0),
        )
        .unwrap()
}
async fn ftn_action(c: &mut OperatorClient, request: sf_bbs::ftn::Action) {
    assert!(matches!(
        c.qwk_network_action(
            format!("{:032x}", rand::random::<u128>()),
            NetworkAction::Ftn { request }
        )
        .await
        .unwrap(),
        NetworkResult::Ftn {
            response: sf_bbs::ftn::Result::Updated
        }
    ));
}
async fn secret(c: &mut OperatorClient, link: &str, area: bool) {
    let expected = c.binkp_status().await.unwrap().policy;
    action(
        c,
        if area {
            binkp::Action::AreaFixCredential {
                link: link.into(),
                expected,
                secret: "synthetic-areafix-password".into(),
            }
        } else {
            binkp::Action::Credential {
                link: link.into(),
                expected,
                secret: "synthetic-hub-password".into(),
            }
        },
    )
    .await;
}
async fn exchange(c: &mut OperatorClient, link: &str, success: bool) {
    let expected = c.binkp_status().await.unwrap().policy;
    action(
        c,
        binkp::Action::Poll {
            link: link.into(),
            expected,
        },
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = c.binkp_status().await.unwrap();
        let h = s
            .links
            .iter()
            .find(|l| l.link == link)
            .and_then(|l| l.health.as_ref())
            .unwrap();
        if !h.active {
            assert_eq!(h.last_error.is_none(), success, "{h:?}");
            return;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
async fn subscribe(c: &mut OperatorClient, link: &str, area: &str, state: bool, expected: i64) {
    ftn_action(
        c,
        sf_bbs::ftn::Action::Subscription {
            subscription: Subscription {
                link: link.into(),
                domain: "isolated".parse().unwrap(),
                area: area.into(),
                subscribed: state,
                source: SubscriptionSource::Manual,
                version: expected + 1,
                changed_at: 0,
            },
            expected,
        },
    )
    .await;
}
#[test]
fn five_daemon_hub_areafix_rescan_point_and_partial_delivery() {
    let _journey = DAEMON_JOURNEY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let temp = tempfile::tempdir().unwrap();
    let ports = [port(), port(), port(), port(), port()];
    let hub = make_board(
        &temp.path().join("hub"),
        "10:100/1@isolated",
        ports[0],
        &[
            ("upstream", "10:100/2@isolated", ports[1]),
            ("a", "10:100/3@isolated", ports[2]),
            ("b", "10:100/4@isolated", ports[3]),
            ("point", "10:100/1.1@isolated", ports[4]),
        ],
    );
    let up = make_board(
        &temp.path().join("upstream"),
        "10:100/2@isolated",
        ports[1],
        &[("hub", "10:100/1@isolated", ports[0])],
    );
    let a = make_board(
        &temp.path().join("a"),
        "10:100/3@isolated",
        ports[2],
        &[("hub", "10:100/1@isolated", ports[0])],
    );
    let b = make_board(
        &temp.path().join("b"),
        "10:100/4@isolated",
        ports[3],
        &[("hub", "10:100/1@isolated", ports[0])],
    );
    let point = make_board(
        &temp.path().join("point"),
        "10:100/1.1@isolated",
        ports[4],
        &[("hub", "10:100/1@isolated", ports[0])],
    );
    let mut hd = Some(start(&hub.board.config));
    let ud = start(&up.board.config);
    let ad = start(&a.board.config);
    let mut bd = Some(start(&b.board.config));
    let pd = start(&point.board.config);
    tokio::runtime::Runtime::new().unwrap().block_on(async{
        let mut hc=connect(&hub.board.config).await;let mut uc=connect(&up.board.config).await;let mut ac=connect(&a.board.config).await;let mut bc=connect(&b.board.config).await;let mut pc=connect(&point.board.config).await;
        for c in [&mut uc,&mut ac,&mut bc,&mut pc]{secret(c,"hub",false).await;}
        for link in ["upstream","a","b","point"]{secret(&mut hc,link,false).await;}
        for link in ["a","b","point"]{ftn_action(&mut hc,sf_bbs::ftn::Action::Downstream{downstream:Downstream{link:link.into(),enabled:true,held:false,boss_aka:(link=="point").then(||"local".into()),areafix:true,rescan:true,max_area:3,max_total:5,cooldown:60,version:1},expected:0}).await;secret(&mut hc,link,true).await;}
        for area in ["AREA1","AREA2"]{ftn_action(&mut hc,sf_bbs::ftn::Action::AreaAccess{access:AreaAccess{domain:"isolated".parse().unwrap(),area:area.into(),remote_subscribe:true,rescan:true,version:1},expected:0}).await;}
        for (link,area) in [("a","AREA1"),("b","AREA1"),("b","AREA2"),("point","AREA1")]{subscribe(&mut hc,link,area,true,0).await;}
        echo(&up,1,1);exchange(&mut uc,"hub",true).await;assert_eq!(count(&hub,1),1);
        for link in ["a","b","point"]{exchange(&mut hc,link,true).await;}
        assert_eq!([count(&a,1),count(&b,1),count(&point,1),count(&up,1)],[1,1,1,1]);
        echo(&up,2,2);exchange(&mut uc,"hub",true).await;for link in ["a","b","point"]{exchange(&mut hc,link,true).await;}assert_eq!([count(&a,2),count(&b,2),count(&point,2)],[0,1,0]);
        echo(&a,1,3);exchange(&mut ac,"hub",true).await;for link in ["upstream","b","point"]{exchange(&mut hc,link,true).await;}assert_eq!([count(&hub,1),count(&a,1),count(&b,1),count(&point,1),count(&up,1)],[2,2,2,2,2]);
        echo(&point,1,4);exchange(&mut pc,"hub",true).await;for link in ["upstream","a","b"]{exchange(&mut hc,link,true).await;}assert_eq!([count(&hub,1),count(&a,1),count(&b,1),count(&point,1),count(&up,1)],[3,3,3,3,3]);
        let conn=rusqlite::Connection::open(hub.board.paths.database()).unwrap();assert_eq!(conn.query_row("SELECT COUNT(*) FROM ftn_messages m JOIN ftn_addresses a ON a.address_id=m.origin_address WHERE a.point=1",[],|r|r.get::<_,u32>(0)).unwrap(),1);
        mail(&point,"10:100/2@isolated","Recipient","Point transit","Private hub sentinel");exchange(&mut pc,"hub",true).await;exchange(&mut hc,"upstream",true).await;
        mail(&up,"10:100/1.1@isolated","Recipient","Point delivery","Private point sentinel");exchange(&mut uc,"hub",true).await;exchange(&mut hc,"point",true).await;
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM messages WHERE container_kind='network-transit' AND visibility='private'",[],|r|r.get::<_,u32>(0)).unwrap(),2);
        for (peer,sentinel) in [(&up,"Private hub sentinel"),(&point,"Private point sentinel")]{let c=rusqlite::Connection::open(peer.board.paths.database()).unwrap();let id=c.query_row("SELECT m.message_id FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE CAST(p.body AS TEXT)=?1",[sentinel],|r|r.get::<_,i64>(0)).unwrap();let db=RuntimeDatabase::open(peer.board.paths.database()).unwrap();assert_eq!(db.read_ftn_mail(peer.board.actor,MessageId::new(id).unwrap()).unwrap().body,sentinel);assert!(db.read_ftn_mail(peer.board.other,MessageId::new(id).unwrap()).is_err());}
        mail(&a,"10:100/1@isolated","AreaFix","synthetic-areafix-password","%LIST\n%HELP\n+AREA2");exchange(&mut ac,"hub",true).await;exchange(&mut hc,"a",true).await;
        let page=hc.networks(NetworkQuery{section:NetworkSection::Hub,offset:0}).await.unwrap().page;assert!(page.subscriptions.iter().any(|s|s.link=="a"&&s.area=="AREA2"&&s.subscribed));assert_eq!(page.areafix[0].result,"accepted");
        echo(&up,2,5);exchange(&mut uc,"hub",true).await;exchange(&mut hc,"a",true).await;exchange(&mut hc,"b",true).await;assert_eq!(count(&a,2),1);
        mail(&a,"10:100/1@isolated","AreaFix","wrong-password","-AREA2");exchange(&mut ac,"hub",true).await;exchange(&mut hc,"a",true).await;assert!(RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_subscriptions(0).unwrap().iter().any(|s|s.link=="a"&&s.area=="AREA2"&&s.subscribed));
        mail(&a,"10:100/1@isolated","AreaFix","synthetic-areafix-password","-AREA2");exchange(&mut ac,"hub",true).await;exchange(&mut hc,"a",true).await;echo(&up,2,6);exchange(&mut uc,"hub",true).await;exchange(&mut hc,"b",true).await;exchange(&mut hc,"a",true).await;assert_eq!(count(&a,2),1);
        mail(&point,"10:100/1@isolated","AreaFix","synthetic-areafix-password","%RESCAN AREA1 R=3");exchange(&mut pc,"hub",true).await;exchange(&mut hc,"point",true).await;
        let activity=RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_rescan_activity(0).unwrap();assert_eq!((activity[0].queued,activity[0].accepted),(3,3));assert_eq!(count(&point,1),3);
        // Partial fanout: stop B; other targets accept and never return to pending.
        drop(bc);graceful(bd.take().unwrap());echo(&up,1,7);exchange(&mut uc,"hub",true).await;exchange(&mut hc,"a",true).await;exchange(&mut hc,"point",true).await;exchange(&mut hc,"b",false).await;
        let queues=RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_queue(None).unwrap();assert!(queues.iter().filter(|q|q.state!="accepted").all(|q|q.link=="b"));
        bd=Some(start(&b.board.config));bc=connect(&b.board.config).await;
        for q in queues.iter().filter(|q|q.state!="accepted"){action(&mut hc,binkp::Action::Release{queue:q.id.clone(),expected:q.version}).await;}
        exchange(&mut hc,"b",true).await;assert_eq!([count(&a,1),count(&b,1),count(&point,1)],[4,4,4]);
        let mut d=RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_downstreams().unwrap().into_iter().find(|d|d.link=="b").unwrap();d.held=true;d.version+=1;ftn_action(&mut hc,sf_bbs::ftn::Action::Downstream{downstream:d.clone(),expected:1}).await;
        echo(&up,1,8);exchange(&mut uc,"hub",true).await;exchange(&mut hc,"a",true).await;exchange(&mut hc,"point",true).await;assert_eq!(count(&b,1),4);assert!(RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_queue(None).unwrap().iter().any(|q|q.link=="b"&&q.state=="pending"));
        d.held=false;d.version+=1;ftn_action(&mut hc,sf_bbs::ftn::Action::Downstream{downstream:d,expected:2}).await;exchange(&mut hc,"b",true).await;assert_eq!(count(&b,1),5);
        let before=RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_subscriptions(0).unwrap();let points=RuntimeDatabase::open(hub.board.paths.database()).unwrap().ftn_downstreams().unwrap();
        for section in NetworkSection::ALL{let s=serde_json::to_string(&hc.networks(NetworkQuery{section,offset:0}).await.unwrap()).unwrap();for secret in ["synthetic-areafix-password","Private hub sentinel","Private point sentinel"]{assert!(!s.contains(secret));}}
        drop(hc);graceful(hd.take().unwrap());hd=Some(start(&hub.board.config));hc=connect(&hub.board.config).await;
        let db=RuntimeDatabase::open(hub.board.paths.database()).unwrap();assert_eq!(db.ftn_subscriptions(0).unwrap(),before);assert_eq!(db.ftn_downstreams().unwrap(),points);assert!(db.binkp_health().unwrap().iter().all(|h|!h.active));
        for link in ["upstream","a","b","point"]{exchange(&mut hc,link,true).await;}
        assert_eq!([count(&hub,1),count(&a,1),count(&b,1),count(&point,1),count(&up,1)],[5,5,5,5,5]);
        // Cold restore with A and point accepted, B frozen but not yet offered.
        drop(db); drop(conn);
        echo(&up,1,9);exchange(&mut uc,"hub",true).await;exchange(&mut hc,"a",true).await;exchange(&mut hc,"point",true).await;
        drop(hc);graceful(hd.take().unwrap());
        let mut db=RuntimeDatabase::open(hub.board.paths.database()).unwrap();
        let store=sf_bbs::DiskArtifactStore::new(hub.board.paths.get(LogicalPath::System)).unwrap();
        let pending:Vec<_>=db.ftn_queue(None).unwrap().into_iter().filter(|q|q.state!="accepted").collect();assert_eq!(pending.len(),1);assert_eq!(pending[0].link,"b");
        for q in pending {db.build_ftn(&store,&hub.board.policy,&q.id,q.version,now()).unwrap();}
        let subscriptions=db.ftn_subscriptions(0).unwrap();let downstreams=db.ftn_downstreams().unwrap();let rescans=db.ftn_rescan_activity(0).unwrap();
        let accepted=db.ftn_queue(None).unwrap().into_iter().filter(|q|q.state=="accepted").count();drop(db);
        let backup=temp.path().join("hub-backup");sf_bbs::backup_board(&hub.board.config,&backup).unwrap();sf_bbs::restore_board(&backup,&temp.path().join("hub"),true).unwrap();
        let db=RuntimeDatabase::open(hub.board.paths.database()).unwrap();assert_eq!(db.ftn_subscriptions(0).unwrap(),subscriptions);assert_eq!(db.ftn_downstreams().unwrap(),downstreams);assert_eq!(db.ftn_rescan_activity(0).unwrap(),rescans);assert_eq!(db.ftn_queue(None).unwrap().iter().filter(|q|q.state=="accepted").count(),accepted);assert!(db.binkp_health().unwrap().iter().all(|h|!h.active));
        let held:Vec<_>=db.ftn_queue(None).unwrap().into_iter().filter(|q|q.state=="held").collect();assert_eq!(held.len(),1);assert_eq!(held[0].link,"b");drop(db);
        hd=Some(start(&hub.board.config));hc=connect(&hub.board.config).await;
        let status=hc.binkp_status().await.unwrap();assert!(status.links.iter().filter(|l|l.link!="upstream").all(|l|l.areafix_credential==sf_bbs::SecretStatus::Configured));
        for q in held{action(&mut hc,binkp::Action::Release{queue:q.id,expected:q.version}).await;}
        for link in ["upstream","a","b","point"]{exchange(&mut hc,link,true).await;}
        assert_eq!([count(&hub,1),count(&a,1),count(&b,1),count(&point,1),count(&up,1)],[6,6,6,6,6]);
        drop((hc,uc,ac,bc,pc));
    });
    graceful(hd.take().unwrap());
    graceful(ud);
    graceful(ad);
    graceful(bd.take().unwrap());
    graceful(pd);
}

#[test]
#[ignore = "explicit isolated N6 terminal and independent peer acceptance"]
fn prepare_n6_operator_acceptance() {
    let evidence = PathBuf::from(
        std::env::var_os("SPITFIRE_N6_EVIDENCE").expect("private evidence directory"),
    );
    fs::create_dir_all(&evidence).unwrap();
    let root = tempfile::tempdir().unwrap().keep();
    let hub = make_board(
        &root.join("hub"),
        "10:100/1@isolated",
        34565,
        &[
            ("independent", "10:100/2@isolated", 34564),
            ("native", "10:100/3@isolated", 34566),
            ("point", "10:100/1.1@isolated", 34567),
        ],
    );
    let native = make_board(
        &root.join("native"),
        "10:100/3@isolated",
        34566,
        &[("hub", "10:100/1@isolated", 34565)],
    );
    let mut db = RuntimeDatabase::open(hub.board.paths.database()).unwrap();
    for link in ["independent", "point"] {
        db.configure_ftn_downstream(
            &hub.board.policy,
            "operator",
            &Downstream {
                link: link.into(),
                enabled: true,
                held: false,
                boss_aka: (link == "point").then(|| "local".into()),
                areafix: true,
                rescan: true,
                max_area: 3,
                max_total: 5,
                cooldown: 60,
                version: 1,
            },
            0,
            now(),
        )
        .unwrap();
    }
    // The native peer is an ordinary upstream; each area maps once locally.
    for (area, conference) in [("AREA1", hub.board.conference), ("AREA2", hub.second)] {
        db.configure_ftn_mapping(
            &hub.board.policy,
            "operator",
            &Mapping {
                posting_identity: Default::default(),
                domain: "isolated".parse().unwrap(),
                area: area.into(),
                conference_id: conference.get(),
                aka: "local".into(),
                receive: true,
                send: true,
                origin: "Synthetic N6 acceptance".into(),
                links: vec!["native".into()],
                version: 2,
            },
            1,
            now(),
        )
        .unwrap();
        db.configure_ftn_area_access(
            &hub.board.policy,
            "operator",
            &AreaAccess {
                domain: "isolated".parse().unwrap(),
                area: area.into(),
                remote_subscribe: true,
                rescan: true,
                version: 1,
            },
            0,
            now(),
        )
        .unwrap();
    }
    echo(&hub, 1, 100);
    echo(&native, 1, 101);
    for (key, path) in [
        ("config-path", hub.board.config.as_path()),
        ("database-path", hub.board.paths.database()),
        ("native-config-path", native.board.config.as_path()),
        ("native-database-path", native.board.paths.database()),
    ] {
        fs::write(evidence.join(key), path.to_string_lossy().as_bytes()).unwrap();
    }
    println!("Prepared private N6 acceptance boards; no network contact.");
}

#[cfg(unix)]
#[path = "files_journey.rs"]
mod files_journey;
