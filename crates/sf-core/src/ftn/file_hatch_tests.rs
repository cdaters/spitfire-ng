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

use super::*;
use crate::network::{ImportCapacity, ImportPermit, NetworkError};
use std::sync::Mutex;

#[derive(Default)]
struct Store(Mutex<BTreeMap<String, Vec<u8>>>, ImportCapacity);
impl NetworkArtifactStore for Store {
    fn admit_import(&self) -> std::result::Result<ImportPermit<'_>, NetworkError> {
        self.1.acquire()
    }
    fn preserve(&self, bytes: &[u8]) -> std::result::Result<String, NetworkError> {
        let id = qwk::digest(bytes);
        self.0.lock().unwrap().insert(id.clone(), bytes.to_vec());
        Ok(id)
    }
    fn usage(&self) -> std::result::Result<(u64, usize), NetworkError> {
        let files = self.0.lock().unwrap();
        Ok((files.values().map(|b| b.len() as u64).sum(), files.len()))
    }
}
fn transport(b: &Board) -> BinkpPolicy {
    BinkpPolicy {
        listener: None,
        links: b
            .policy
            .links
            .iter()
            .map(|l| BinkpLink {
                link: l.id.clone(),
                enabled: true,
                inbound: true,
                outbound: true,
                endpoint: Some("127.0.0.1".into()),
                port: 24556,
                directory: false,
                akas: vec![l.aka.clone()],
                remote_akas: vec![],
                auth: BinkpAuth::RequireCram,
                allow_domainless: false,
            })
            .collect(),
    }
}
fn authenticated(b: &mut Board, t: &BinkpPolicy, link: &str) -> String {
    let s =
        b.db.begin_binkp(&b.policy, t, link, "hatch-test", BinkpMode::Poll, NOW)
            .unwrap();
    b.db.observe_binkp(
        &s,
        &[b.policy.link(link).unwrap().remote.clone()],
        &["CRAM-MD5".into()],
        1,
        NOW,
    )
    .unwrap();
    s
}
fn contextual(
    b: &mut Board,
    t: &BinkpPolicy,
    store: &Store,
    s: &str,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    b.db.receive_file_offer_with_context(
        &b.policy,
        &b.storage,
        &FileReceiveContext {
            transport: t,
            artifacts: store,
        },
        s,
        &sf_net::binkp::Offer {
            name: name.into(),
            size: bytes.len() as u64,
            time: NOW as u64,
            offset: 0,
        },
        bytes,
        NOW,
        &|_, p| p == "TESTPASS",
    )
}
fn hatch_bytes(b: &Board, untimed: bool, empty: bool) -> Vec<u8> {
    let text = String::from_utf8(inbound(b, "up", "HATCH.TXT", b"hatch payload")).unwrap();
    let peer = b.policy.link("up").unwrap().remote.to_string();
    text.lines()
        .filter_map(|line| {
            if empty && line.starts_with("Seenby ") {
                None
            } else if untimed && line.starts_with("Path ") {
                Some(format!("Path {}", peer.split('@').next().unwrap()))
            } else {
                Some(line.to_owned())
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        .into_bytes()
}
fn publication(b: &Board) -> (String, StoredTic) {
    let json: String =
        b.db.connection
            .query_row("SELECT metadata FROM ftn_file_publications", [], |r| {
                r.get(0)
            })
            .unwrap();
    let parsed = serde_json::from_str(&json).unwrap();
    (json, parsed)
}
#[test]
fn authenticated_hatch_each_omission_retains_exact_raw_and_typed_provenance() {
    for untimed in [false, true] {
        for empty in [false, true] {
            let mut b = board();
            let t = transport(&b);
            let store = Store::default();
            let s = authenticated(&mut b, &t, "up");
            let bytes = hatch_bytes(&b, untimed, empty);
            contextual(&mut b, &t, &store, &s, "HATCH.TXT", b"hatch payload").unwrap();
            contextual(&mut b, &t, &store, &s, "CONTROL.TIC", &bytes).unwrap();
            let (json, stored) = publication(&b);
            assert!(!json.contains("TESTPASS"));
            let receipt = stored.received_tic.unwrap();
            assert_eq!(receipt.peer, b.policy.link("up").unwrap().remote);
            assert_eq!(receipt.session, s);
            assert_eq!(receipt.received_at, NOW);
            assert_eq!(
                store.0.lock().unwrap().get(&receipt.artifact).unwrap(),
                &bytes
            );
            assert_eq!(stored.metadata.path.len(), usize::from(!untimed));
            assert_eq!(stored.metadata.seen.len(), usize::from(!empty));
            assert_eq!(receipt.direct_hatch.is_some(), untimed || empty);
            if let Some(history) = receipt.direct_hatch {
                assert_eq!(history.untimed_path.is_some(), untimed);
                assert_eq!(history.empty_seenby, empty);
            }
            let status = b.db.file_network_status().unwrap();
            assert_eq!(status.queue.len(), 2);
            assert!(status.queue.iter().all(|q| q.link != "up"));
            assert!(!serde_json::to_string(&status).unwrap().contains("TESTPASS"));
            assert_eq!(
                b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
                    .unwrap(),
                2
            );
        }
    }
}
#[test]
fn hatch_downstream_history_duplicate_replay_and_wrong_peer_remain_safe() {
    let mut b = board();
    let t = transport(&b);
    let store = Store::default();
    let s = authenticated(&mut b, &t, "up");
    let bytes = hatch_bytes(&b, true, true);
    contextual(&mut b, &t, &store, &s, "CONTROL.TIC", &bytes).unwrap();
    contextual(&mut b, &t, &store, &s, "HATCH.TXT", b"hatch payload").unwrap();
    let original = publication(&b).0;
    assert!(work(&mut b, &s).is_empty());
    close(&mut b, &s);
    let s = authenticated(&mut b, &t, "up");
    contextual(&mut b, &t, &store, &s, "HATCH.TXT", b"hatch payload").unwrap();
    contextual(&mut b, &t, &store, &s, "REPLAY.TIC", &bytes).unwrap();
    let status = b.db.file_network_status().unwrap();
    assert!(status
        .activity
        .iter()
        .any(|a| a.result == "file-duplicate-suppressed"));
    assert_eq!(status.queue.len(), 2);
    assert_eq!(status.staged, 0);
    assert_eq!(publication(&b).0, original);
    close(&mut b, &s);
    let s = authenticated(&mut b, &t, "a");
    contextual(&mut b, &t, &store, &s, "REPLAY.TIC", &bytes).unwrap();
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    let items = work(&mut b, &s);
    assert_eq!(items.len(), 2);
    let outbound = tic::parse(
        &items
            .iter()
            .find(|(w, _)| w.name.ends_with(".TIC"))
            .unwrap()
            .1,
        &b.area.domain,
    )
    .unwrap()
    .metadata;
    assert_eq!(outbound.from, b.policy.akas[0].endpoint);
    assert_eq!(outbound.origin, b.policy.link("up").unwrap().remote);
    assert_eq!(outbound.path.len(), 1);
    assert_eq!(outbound.path[0].address, b.policy.akas[0].endpoint);
    assert_eq!(outbound.path[0].time, NOW as u64);
    assert_eq!(outbound.seen.len(), 4);
    for id in ["up", "a", "b"] {
        assert!(outbound.seen.contains(&b.policy.link(id).unwrap().remote));
    }
    assert_eq!(
        b.db.file_count(FileAreaId::new(b.area.native_area).unwrap())
            .unwrap(),
        2
    );
}
#[test]
fn hatch_missing_context_unauthenticated_and_stale_policy_reject() {
    let mut b = board();
    let t = transport(&b);
    let store = Store::default();
    let bytes = hatch_bytes(&b, true, true);
    let s =
        b.db.begin_binkp(&b.policy, &t, "up", "hatch-test", BinkpMode::Poll, NOW)
            .unwrap();
    assert!(contextual(&mut b, &t, &store, &s, "CONTROL.TIC", &bytes).is_err());
    b.db.observe_binkp(
        &s,
        &[b.policy.link("up").unwrap().remote.clone()],
        &[],
        1,
        NOW,
    )
    .unwrap();
    receive(&mut b, &s, "CONTROL.TIC", &bytes);
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    let mut changed = t.clone();
    changed.links[0].port += 1;
    assert!(contextual(&mut b, &changed, &store, &s, "CONTROL.TIC", &bytes).is_err());
    assert!(store.0.lock().unwrap().is_empty());
}
#[test]
fn hatch_malformed_wrong_domain_wrong_peer_and_loop_evidence_reject() {
    let mut b = board();
    let t = transport(&b);
    let store = Store::default();
    let s = authenticated(&mut b, &t, "up");
    let text = String::from_utf8(hatch_bytes(&b, true, true)).unwrap();
    for (before, after) in [
        ("Path 10:100/2", "Path bad"),
        ("Path 10:100/2", "Path 10:100/9"),
        ("Path 10:100/2", "Path 10:100/2@other"),
        ("Path 10:100/2", "Path 1:2/3@fidonet"),
        ("Path 10:100/2", "Path 20:200/7@othernet"),
        ("Path 10:100/2", "Path 10:100/2@unknown"),
        ("Path 10:100/2", "Path 10:100/2\r\nPath 10:100/2 1"),
        ("Origin 10:100/2@synthetic", "Origin 10:100/3@synthetic"),
        ("From 10:100/2@synthetic", "From 10:100/3@synthetic"),
        ("To 10:100/1@synthetic", "To 10:100/4@synthetic"),
        ("TESTPASS", "WRONGPASS"),
        ("Area FILES", "Area UNKNOWN"),
        (
            "Path 10:100/2",
            "Path 10:100/2\r\nSeenby 10:100/1@synthetic",
        ),
    ] {
        assert!(text.contains(before), "{before}");
        contextual(
            &mut b,
            &t,
            &store,
            &s,
            "CONTROL.TIC",
            text.replace(before, after).as_bytes(),
        )
        .unwrap();
        assert_eq!(b.db.file_network_status().unwrap().staged, 0, "{after}");
    }
    assert!(store.0.lock().unwrap().is_empty());
}
#[test]
fn hatch_cross_domain_configured_ambiguity_rejects() {
    for remote in [false, true] {
        let mut b = board();
        let numeric = if remote { "10:100/2" } else { "10:100/1" };
        b.policy.akas.push(Aka {
            id: "other".into(),
            endpoint: format!("{numeric}@other").parse().unwrap(),
            enabled: true,
            primary: true,
        });
        let t = transport(&b);
        let store = Store::default();
        let s = authenticated(&mut b, &t, "up");
        let bytes = hatch_bytes(&b, true, true);
        contextual(&mut b, &t, &store, &s, "CONTROL.TIC", &bytes).unwrap();
        assert_eq!(b.db.file_network_status().unwrap().staged, 0);
    }
}
#[test]
fn staged_hatch_restart_retains_first_receipt_and_requires_fresh_authority() {
    let mut b = board();
    let t = transport(&b);
    let store = Store::default();
    let s = authenticated(&mut b, &t, "up");
    let bytes = hatch_bytes(&b, true, true);
    contextual(&mut b, &t, &store, &s, "CONTROL.TIC", &bytes).unwrap();
    let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
    let paths = LogicalPaths::resolve(b._temp.path(), &config).unwrap();
    b.db = RuntimeDatabase::open(paths.database()).unwrap();
    b.db.recover_binkp(NOW + 1).unwrap();
    assert!(contextual(&mut b, &t, &store, &s, "HATCH.TXT", b"hatch payload").is_err());
    let fresh =
        b.db.begin_binkp(
            &b.policy,
            &t,
            "up",
            "restarted-hatch",
            BinkpMode::Poll,
            NOW + 600,
        )
        .unwrap();
    b.db.observe_binkp(
        &fresh,
        &[b.policy.link("up").unwrap().remote.clone()],
        &[],
        1,
        NOW + 600,
    )
    .unwrap();
    contextual(&mut b, &t, &store, &fresh, "RETRY.TIC", &bytes).unwrap();
    contextual(&mut b, &t, &store, &fresh, "HATCH.TXT", b"hatch payload").unwrap();
    assert_eq!(publication(&b).1.received_tic.unwrap().session, s);
    assert_eq!(b.db.file_network_status().unwrap().staged, 0);
}
