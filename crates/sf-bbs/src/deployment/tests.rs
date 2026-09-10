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

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ring::signature::{Ed25519KeyPair, KeyPair};
use sf_core::{LogicalPath, LogicalPaths, RuntimeConfig, RuntimeDatabase, SecurityLevel};

use super::model::*;
use super::worker::Runner;
use super::*;

struct FixedCapacity(u64);
impl io::Capacity for FixedCapacity {
    fn available(&self, _: &Path) -> Result<u64> {
        Ok(self.0)
    }
}
const AMPLE: FixedCapacity = FixedCapacity(1024 * 1024 * 1024 * 1024);

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkerFailure {
    None,
    Migration,
    Validation,
    Descriptor,
}
struct ModelRunner {
    failure: Cell<WorkerFailure>,
}
impl ModelRunner {
    fn new() -> Self {
        Self {
            failure: Cell::new(WorkerFailure::None),
        }
    }
}
impl Runner for ModelRunner {
    fn describe(&self, executable: &Path) -> Result<RuntimeDescriptor> {
        let mut d: RuntimeDescriptor = serde_json::from_slice(&fs::read(executable)?)?;
        if self.failure.get() == WorkerFailure::Descriptor {
            d.version = "9.9.9".into();
        }
        Ok(d)
    }
    fn worker(&self, _: &Path, request: &WorkerRequest) -> Result<BoardState> {
        if request.operation == "migrate" && self.failure.get() == WorkerFailure::Migration {
            let (_, paths) = worker::paths(&request.config)?;
            let db = rusqlite::Connection::open(paths.database())?;
            db.execute_batch("CREATE TABLE d1_partial_migration(value TEXT); INSERT INTO d1_partial_migration VALUES('synthetic failure');")?;
            return Err(Error::Rejected(
                "deliberate migration failure after staged mutation".into(),
            ));
        }
        if request.operation == "migrate" && self.failure.get() == WorkerFailure::Validation {
            let mut state = worker::execute(request)?;
            state.schema += 1;
            return Ok(state);
        }
        worker::execute(request)
    }
}

struct Fixture {
    temp: tempfile::TempDir,
    board: PathBuf,
    install: PathBuf,
    source: PathBuf,
    key: Ed25519KeyPair,
    pin: PathBuf,
    catalog_key: Vec<u8>,
}
impl Fixture {
    fn new(runner: &ModelRunner) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let board = temp.path().join("board");
        let catalog_key = populate_board(&board);
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let key = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let key = Ed25519KeyPair::from_pkcs8(key.as_ref()).unwrap();
        let pin = temp.path().join("release-public-key.hex");
        fs::write(&pin, io::hex(key.public_key().as_ref())).unwrap();
        let install = temp.path().join("installation");
        let fixture = Self {
            temp,
            board,
            install,
            source,
            key,
            pin,
            catalog_key,
        };
        fixture.package("0.1.0", None);
        manager::adopt(
            &fixture.config(),
            &fixture.install,
            &fixture.source,
            &fixture.pin,
            &fixture.source.join("0.1.0").join(release::binary_name()),
            runner,
        )
        .unwrap();
        fixture
    }
    fn config(&self) -> PathBuf {
        self.board.join(crate::BOARD_CONFIG_FILE)
    }
    fn db(&self) -> PathBuf {
        worker::paths(&self.config())
            .unwrap()
            .1
            .database()
            .to_path_buf()
    }
    fn manager<'a>(&self, runner: &'a dyn Runner, capacity: &'a dyn io::Capacity) -> Manager<'a> {
        Manager::open(&self.install, runner, capacity).unwrap()
    }
    fn package(&self, version: &str, change: Option<fn(&mut ReleaseManifest)>) {
        let root = self.source.join(version);
        fs::create_dir(&root).unwrap();
        let mut descriptor = RuntimeDescriptor::current();
        descriptor.version = version.into();
        let bytes = serde_json::to_vec(&descriptor).unwrap();
        fs::write(root.join(release::binary_name()), &bytes).unwrap();
        let mut manifest = ReleaseManifest {
            format: FORMAT,
            runtime: descriptor,
            minimum_upgrade_version: "0.1.0".into(),
            executable: release::binary_name().into(),
            size_bytes: bytes.len() as u64,
            sha256: io::digest(&bytes),
        };
        if let Some(change) = change {
            change(&mut manifest);
        }
        self.sign_manifest(&root, &manifest);
    }
    fn sign_manifest(&self, root: &Path, manifest: &ReleaseManifest) {
        let bytes = serde_json::to_vec_pretty(manifest).unwrap();
        let mut signed = RELEASE_DOMAIN.to_vec();
        signed.extend_from_slice(&bytes);
        fs::write(root.join("release.json"), bytes).unwrap();
        fs::write(
            root.join("release.sig"),
            io::hex(self.key.sign(&signed).as_ref()),
        )
        .unwrap();
    }
    fn policy(&self, change: impl FnOnce(&mut StoragePolicy)) {
        let (mut installation, _) = control::load(&self.install).unwrap();
        change(&mut installation.storage);
        fs::write(
            self.install.join("installation.toml"),
            toml::to_string_pretty(&installation).unwrap(),
        )
        .unwrap();
    }
    fn new_evidence(&self) {
        new_evidence(&self.board, &self.catalog_key);
    }
}

fn fingerprint(root: &Path) -> BTreeMap<String, (u64, String)> {
    io::inventory(root)
        .unwrap()
        .into_iter()
        .map(|(name, path)| (name, io::hash(&path).unwrap()))
        .collect()
}

fn populate_board(board: &Path) -> Vec<u8> {
    use sf_core::{
        CallerState, CredentialHasher, FileActor, FileStorage, MessageActor, MessageBackend,
        MessageKind, MessageVisibility, NewMessage,
    };
    let mut plan = crate::SetupPlan::stock_defaults("D1 Disposable Board", "D1 Sysop", "Sysop", 1);
    plan.config.caller.password = sf_core::PasswordHashConfig {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    crate::setup_board(board, &plan, b"synthetic d1 setup password").unwrap();
    let config_path = board.join(crate::BOARD_CONFIG_FILE);
    let mut config = RuntimeConfig::load(&config_path).unwrap();
    config.format_version = 1;
    config.node = Some(sf_core::LegacyNodeConfig { number: 1 });
    config.nodes = None;
    config.ftn.enabled = true;
    config.ftn.akas.push(sf_core::ftn::Aka {
        id: "local".into(),
        endpoint: "1:100/1@d1-test".parse().unwrap(),
        enabled: true,
        primary: true,
    });
    config.save_atomic(&config_path).unwrap();
    let paths = LogicalPaths::resolve(board, &config.validate().unwrap()).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let hasher = CredentialHasher::new(&config.caller.password).unwrap();
    let caller = db
        .create_caller(
            b"D1 Caller",
            &hasher.hash(b"synthetic caller password").unwrap(),
            SecurityLevel::new(20).unwrap(),
            CallerState::Active,
            false,
            100,
        )
        .unwrap();
    let sysop = db.caller_by_name(b"Sysop").unwrap().unwrap();
    let actor = MessageActor::new(
        caller.id,
        SecurityLevel::new(config.caller.sysop_security).unwrap(),
    );
    let conference = db
        .create_conference(&sf_core::ConferenceDefinition {
            posting_identity: Some(sf_core::PostingIdentityPolicy::HandleAllowed),
            number: 20,
            name: "D1 Network Conference".into(),
            description: "Synthetic local mapping".into(),
            access_mode: sf_core::ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            post_security: SecurityLevel::new(0).unwrap(),
            public_only: true,
            caller_deletion_enabled: true,
            maximum_lines: 99,
            privileged_security_levels: vec![],
        })
        .unwrap();
    let mut message = NewMessage {
        identity_preview: None,
        conference_id: conference.id,
        recipient_caller_id: None,
        recipient_name: "All Callers".into(),
        subject: b"D1 preserved message".to_vec(),
        body: b"Synthetic body and reply preservation.\r\n".to_vec(),
        created_at: 100,
        parent_message_id: None,
        visibility: MessageVisibility::Public,
        kind: MessageKind::Standard,
    };
    let parent = db.post(actor, message.clone()).unwrap();
    message.parent_message_id = Some(parent.id);
    message.subject = b"D1 reply".to_vec();
    db.post(actor, message).unwrap();
    let storage = FileStorage::new(&paths).unwrap();
    let area = db.all_file_areas().unwrap().remove(0);
    storage
        .write_seed_file(
            &mut db,
            &area,
            "D1MAN.TXT",
            "Managed payload",
            b"managed D1 payload",
            100,
        )
        .unwrap();
    let external_file = storage
        .write_seed_file(
            &mut db,
            &area,
            "D1EXT.TXT",
            "External reference",
            b"external D1 payload",
            100,
        )
        .unwrap();
    let external = board.parent().unwrap().join("external-media");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("D1EXT.TXT"), b"external D1 payload").unwrap();
    let file_actor = FileActor::new(
        sysop.id,
        SecurityLevel::new(config.caller.sysop_security).unwrap(),
    );
    let root = db
        .add_storage_root(
            file_actor,
            sf_core::StorageRootDefinition {
                area_id: area.id,
                stable_key: "d1-external",
                label: "Synthetic removable media",
                configured_locator: external.to_str().unwrap(),
                priority: 1,
                mode: sf_core::StorageRootMode::ReadOnly,
                occurred_at: 100,
            },
        )
        .unwrap();
    db.set_storage_availability(
        file_actor,
        root.id,
        root.state_version,
        sf_core::StorageAvailability::Available,
        101,
    )
    .unwrap();
    db.set_file_storage_locator(
        file_actor,
        external_file.id,
        root.id,
        "D1EXT.TXT",
        external_file.state_version,
        1,
        102,
    )
    .unwrap();
    db.configure_ftn_alias(
        &config.ftn,
        "d1-fixture",
        &sf_core::ftn::MailboxAlias {
            aka: "local".into(),
            alias: "Test Sysop".into(),
            caller_id: sysop.id.get(),
        },
        100,
    )
    .unwrap();
    use sf_core::qwk_network as qwk;
    db.configure_qwk_link(
        "d1-fixture",
        &qwk::Link {
            id: "d1-peer".into(),
            network: "d1-test".into(),
            local_id: "D1LOCAL".into(),
            remote_id: "D1PEER".into(),
            name: "Synthetic QWK peer".into(),
            profile: qwk::Profile::QwkHeaders,
            role: qwk::PartnerRole::Node,
            enabled: false,
            inbound: true,
            outbound: true,
            version: 1,
            posting_identity: sf_core::PostingIdentityPolicy::HandleAllowed,
        },
        &[qwk::Mapping {
            wire_conference: 1,
            area: "d1test".into(),
            conference_id: conference.id.get(),
            enabled: true,
            inbound: true,
            outbound: true,
            version: 1,
            posting_identity: sf_core::PostingIdentityPolicy::HandleAllowed,
        }],
        0,
        100,
    )
    .unwrap();
    use sf_core::circuitnet::{
        self as cnet,
        catalog::{Authority, Body, Entry, Intent, Lifecycle, Signed},
    };
    let network = cnet::NetworkId::new("D1TEST").unwrap();
    let local = cnet::NodeId::new("D1ROOT").unwrap();
    db.circuitnet_configure(
        "d1-fixture",
        &cnet::Profile {
            network: network.clone(),
            display_name: "Synthetic D1 network".into(),
            local: local.clone(),
            enabled: true,
            trusted_offline: true,
            topology: cnet::Topology {
                nodes: vec![
                    cnet::Node {
                        id: local.clone(),
                        role: cnet::Role::Root,
                        parent: None,
                    },
                    cnet::Node {
                        id: cnet::NodeId::new("D1PEER").unwrap(),
                        role: cnet::Role::End,
                        parent: Some(local.clone()),
                    },
                ],
            },
        },
        0,
        100,
    )
    .unwrap();
    let key = Signed::generate_key().unwrap();
    let authority = Authority {
        network: network.clone(),
        catalog_id: "d".repeat(32),
        publisher: local.clone(),
        public_key: Signed::public_key(&key).unwrap(),
    };
    db.circuitnet_catalog_pin("d1-fixture", &authority, 101)
        .unwrap();
    db.circuitnet_catalog_publish(
        "d1-fixture",
        Body {
            format: "circuitnet-ng-catalog".into(),
            schema: 1,
            network: network.clone(),
            catalog_id: authority.catalog_id,
            revision: 1,
            previous_revision: 0,
            previous_hash: None,
            published_at: 102,
            publisher: local,
            governance_reference: "synthetic-fixture".into(),
            rationale: "D1 synthetic acceptance".into(),
            intent: Intent::Ordinary,
            entries: vec![Entry {
                access: sf_net::circuitnet::catalog::Access::Public,
                id: "e".repeat(32),
                codename: cnet::Codename::new("D1TEST").unwrap(),
                display_name: "D1 Test".into(),
                description: "Synthetic state".into(),
                category: "test".into(),
                required: true,
                status: Lifecycle::Active,
                effective_revision: 1,
                retired_revision: None,
                historical_reference: "synthetic".into(),
                moderator_role: "operator".into(),
            }],
        },
        &key,
        102,
    )
    .unwrap();
    db.circuitnet_catalog_choose(
        "d1-fixture",
        &network,
        &"e".repeat(32),
        Some(conference.id.get()),
        103,
    )
    .unwrap();
    db.circuitnet_scan(&network, 0, 105).unwrap();
    use sf_core::events::*;
    let event = Definition {
        id: "d1-event".into(),
        name: "D1 retained history".into(),
        enabled: true,
        action: Action::Binkp {
            link: "d1-test".into(),
        },
        schedule: Schedule::Manual,
        timezone: "UTC".into(),
        policy: ExchangePolicy::Manual,
        missed: MissedPolicy::RunOnce,
        minimum_spacing_seconds: 5,
    };
    db.event_save(&event, 0, 100).unwrap();
    db.event_run_now(&event.id, 1).unwrap();
    let run = db.event_claim(&event.id, 101, 1).unwrap().unwrap();
    db.event_complete(&run.run, Outcome::Succeeded, 0, 102)
        .unwrap();
    fs::write(
        paths.get(LogicalPath::Display).join("D1.BBS"),
        b"Synthetic D1 display\r\n",
    )
    .unwrap();
    // Synthetic trust material is sensitive state even with listeners disabled.
    fs::write(
        paths.get(LogicalPath::System).join("D1-TRUST.txt"),
        b"synthetic trust enrollment; not a production certificate",
    )
    .unwrap();
    db.validate_current_snapshot().unwrap();
    key
}

fn new_evidence(board: &Path, key: &[u8]) {
    let (_, paths) = worker::paths(&board.join(crate::BOARD_CONFIG_FILE)).unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    let network = sf_core::circuitnet::NetworkId::new("D1TEST").unwrap();
    let signed = db.circuitnet_catalog_current(&network).unwrap().unwrap();
    let mut body = signed.body.clone();
    body.previous_revision = body.revision;
    body.previous_hash = Some(signed.hash);
    body.revision += 1;
    body.published_at += 1;
    body.entries[0].description = "New protected evidence after COMMIT".into();
    body.entries[0].effective_revision = body.revision;
    db.circuitnet_catalog_publish("post-commit-fixture", body, key, 200)
        .unwrap();
    drop(db);
    let db = rusqlite::Connection::open(paths.database()).unwrap();
    assert!(
        db.execute("UPDATE ftn_serials SET next_serial=next_serial+5", [])
            .unwrap()
            > 0
    );
}

#[test]
fn complete_model_upgrade_rollback_and_restore_journey() {
    let runner = ModelRunner::new();
    let fixture = Fixture::new(&runner);
    fixture.package("0.1.1", None);
    let before = worker::database_fingerprints(&fixture.db()).unwrap();
    let untouched = fingerprint(fixture.temp.path());
    let mut manager = fixture.manager(&runner, &AMPLE);
    assert!(manager.check(false).unwrap().contains("0.1.1"));
    assert!(manager
        .check(true)
        .unwrap()
        .contains("No installation or board files changed"));
    assert_eq!(untouched, fingerprint(fixture.temp.path()));
    assert!(manager.update().unwrap().contains("0.1.0 -> 0.1.1"));
    assert_eq!(
        before,
        worker::database_fingerprints(&fixture.db()).unwrap()
    );
    assert_eq!(
        RuntimeConfig::load(&fixture.config())
            .unwrap()
            .format_version,
        2
    );
    fixture.package("0.1.5", None);
    assert!(manager.update().unwrap().contains("0.1.1 -> 0.1.5"));
    assert_eq!(
        before,
        worker::database_fingerprints(&fixture.db()).unwrap()
    );
    fixture.new_evidence();
    let newer = fingerprint(&fixture.board);
    let current = worker::database_fingerprints(&fixture.db()).unwrap();
    assert!(manager.rollback_list().unwrap().contains("compatible"));
    assert!(manager.rollback(true).unwrap().contains("0.1.5 -> 0.1.1"));
    assert_eq!(newer, fingerprint(&fixture.board));
    assert_eq!(
        current,
        worker::database_fingerprints(&fixture.db()).unwrap()
    );
    assert!(manager.update().unwrap().contains("0.1.1 -> 0.1.5"));
    assert_eq!(
        current,
        worker::database_fingerprints(&fixture.db()).unwrap()
    );
    let old_backup = manager
        .state
        .backups
        .keys()
        .find(|id| {
            snapshot::verify(&fixture.install.join("backups").join(id), None)
                .unwrap()
                .state
                .identities
                != current.1
        })
        .unwrap()
        .clone();
    assert!(manager
        .restore(&old_backup)
        .unwrap_err()
        .to_string()
        .contains("newer CircuitNET"));
    assert!(manager.state.transaction.is_none());
    let report = manager.backup().unwrap();
    assert!(report.contains("external payloads referenced (not copied): 1"));
    let id = manager
        .state
        .backups
        .keys()
        .find(|id| {
            snapshot::verify(&fixture.install.join("backups").join(id), None)
                .unwrap()
                .backup_type
                == "manual"
        })
        .unwrap()
        .clone();
    fs::write(fixture.board.join("display/D1.BBS"), b"later resource").unwrap();
    assert!(manager
        .restore(&id)
        .unwrap()
        .contains("Disaster restore complete"));
    assert_eq!(
        fs::read(fixture.board.join("display/D1.BBS")).unwrap(),
        b"Synthetic D1 display\r\n"
    );
    assert!(control::locator(&fixture.board).unwrap().is_some());
    assert!(manager.backups().unwrap().contains("manual"));
}

#[test]
fn migration_and_validation_failures_preserve_original_board_and_retry() {
    for failure in [
        WorkerFailure::Migration,
        WorkerFailure::Validation,
        WorkerFailure::Descriptor,
    ] {
        let runner = ModelRunner::new();
        let fixture = Fixture::new(&runner);
        fixture.package("0.1.1", None);
        let before = fingerprint(&fixture.board);
        runner.failure.set(failure);
        let mut manager = fixture.manager(&runner, &AMPLE);
        let result = manager.update();
        assert!(result.is_err());
        assert_eq!(before, fingerprint(&fixture.board));
        // Descriptor failure also prevents old-runtime validation, so recovery
        // correctly remains blocked until the runner can validate it again.
        runner.failure.set(WorkerFailure::None);
        manager.recover().unwrap();
        assert_eq!(manager.state.active, "0.1.0");
        assert!(manager.state.transaction.is_none());
        manager.update().unwrap();
        assert_eq!(manager.state.active, "0.1.1");
    }
}

#[test]
fn interrupted_phases_recover_from_persisted_authority_without_rewinding_committed_evidence() {
    for phase in [
        Phase::Backup,
        Phase::VerifyBackup,
        Phase::StageRelease,
        Phase::VerifyRelease,
        Phase::Migrate,
        Phase::Validate,
        Phase::ApplyRuntime,
        Phase::ValidateActive,
        Phase::Commit,
        Phase::Cleanup,
    ] {
        let runner = ModelRunner::new();
        let fixture = Fixture::new(&runner);
        fixture.package("0.1.1", None);
        let before = worker::database_fingerprints(&fixture.db()).unwrap();
        let mut manager = fixture.manager(&runner, &AMPLE);
        manager.fault = Some((phase, true));
        assert!(manager.update().is_err(), "{phase:?}");
        drop(manager);
        assert!(check_board_access(&fixture.board).is_err());
        let committed = matches!(phase, Phase::Commit | Phase::Cleanup);
        if committed {
            fixture.new_evidence();
        }
        let latest = worker::database_fingerprints(&fixture.db()).unwrap();
        let mut reopened = fixture.manager(&runner, &AMPLE);
        reopened.recover().unwrap();
        reopened.recover().unwrap();
        assert_eq!(
            reopened.state.active,
            if committed { "0.1.1" } else { "0.1.0" }
        );
        assert_eq!(
            worker::database_fingerprints(&fixture.db()).unwrap(),
            if committed { latest } else { before }
        );
        assert!(reopened.state.transaction.is_none());
    }
}

#[test]
fn incompatible_runtime_refuses_without_modifying_any_installation_or_board_file() {
    let runner = ModelRunner::new();
    let fixture = Fixture::new(&runner);
    fixture.package("0.1.1", None);
    let mut manager = fixture.manager(&runner, &AMPLE);
    manager.update().unwrap();
    fixture.new_evidence();
    manager
        .state
        .required_features
        .insert("future-protected-evidence".into(), 2);
    control::save(&fixture.install, &manager.state).unwrap();
    let before = fingerprint(fixture.temp.path());
    assert!(manager
        .rollback(true)
        .unwrap_err()
        .to_string()
        .contains("No compatible previous runtime"));
    assert_eq!(before, fingerprint(fixture.temp.path()));
    let mut descriptor = RuntimeDescriptor::current();
    let mut state = worker::inspect(&fixture.config(), &fixture.board, true).unwrap();
    descriptor.write_schema.maximum = 35;
    assert!(descriptor.compatible(&state).is_err());
    descriptor = RuntimeDescriptor::current();
    state.config_format = 99;
    assert!(descriptor.compatible(&state).is_err());
}

#[test]
fn capacity_and_payload_policies_are_explicit_and_fail_before_mutation() {
    let runner = ModelRunner::new();
    let fixture = Fixture::new(&runner);
    fixture.package("0.1.1", None);
    for change in [
        |p: &mut StoragePolicy| {
            p.backup_max_bytes = 1;
        },
        |p: &mut StoragePolicy| {
            p.max_single_file_bytes = 1;
        },
        |p: &mut StoragePolicy| {
            p.max_managed_files_bytes = 1;
        },
        |p: &mut StoragePolicy| {
            p.max_file_area_bytes = 1;
        },
    ] {
        fixture.policy(|p| {
            *p = StoragePolicy::default();
            change(p);
        });
        let before = fingerprint(fixture.temp.path());
        assert!(fixture.manager(&runner, &AMPLE).update().is_err());
        assert_eq!(before, fingerprint(fixture.temp.path()));
    }
    fixture.policy(|p| *p = StoragePolicy::default());
    for available in [0, 63 * 1024 * 1024, 256 * 1024 * 1024] {
        let before = fingerprint(fixture.temp.path());
        assert!(fixture
            .manager(&runner, &FixedCapacity(available))
            .update()
            .is_err());
        assert_eq!(before, fingerprint(fixture.temp.path()));
    }
    fixture.policy(|p| {
        p.file_payloads = PayloadPolicy::MetadataOnly;
        p.low_space_warning_bytes = 2 * 1024 * 1024 * 1024;
    });
    let mut manager = fixture.manager(&runner, &FixedCapacity(1024 * 1024 * 1024));
    assert!(manager.check(true).unwrap().contains("Low-space warning"));
    manager.backup().unwrap();
    let id = manager.state.backups.keys().next().unwrap().clone();
    let backup = snapshot::verify(
        &fixture.install.join("backups").join(&id),
        manager.state.backups.get(&id).map(String::as_str),
    )
    .unwrap();
    assert!(backup
        .references
        .iter()
        .filter(|r| r.class == StateClass::ManagedPayload)
        .all(|r| !r.copied));
    assert_eq!(
        backup
            .references
            .iter()
            .filter(|r| r.class == StateClass::ExternalReference)
            .count(),
        1
    );
    assert!(backup
        .entries
        .iter()
        .all(|e| e.class == StateClass::Critical));
    manager.restore(&id).unwrap();
    assert!(io::tree_bytes(&fixture.install.join("backups")).unwrap() > 0);
    fixture.policy(|p| {
        p.backup_max_bytes = backup.stored_content_bytes + MAX_DOCUMENT;
        p.backup_retention_bytes = p.backup_max_bytes;
    });
    assert!(fixture.manager(&runner, &AMPLE).backup().is_err());
    assert!(fixture.install.join("backups").join(&id).exists());
}

#[test]
fn corrupt_unauthenticated_incompatible_and_missing_release_artifacts_fail_closed() {
    for scenario in 0..8 {
        let runner = ModelRunner::new();
        let fixture = Fixture::new(&runner);
        fixture.package("0.1.1", None);
        let package = fixture.source.join("0.1.1");
        let mut manifest: ReleaseManifest =
            serde_json::from_slice(&fs::read(package.join("release.json")).unwrap()).unwrap();
        match scenario {
            0 => fs::write(package.join(release::binary_name()), b"corrupt").unwrap(),
            1 => {
                manifest.sha256 = "0".repeat(64);
                fixture.sign_manifest(&package, &manifest);
            }
            2 => {
                manifest.minimum_upgrade_version = "0.1.2".into();
                fixture.sign_manifest(&package, &manifest);
            }
            3 => {
                manifest.runtime.platform = "wrong-platform".into();
                fixture.sign_manifest(&package, &manifest);
            }
            4 => {
                manifest.runtime.architecture = "wrong-architecture".into();
                fixture.sign_manifest(&package, &manifest);
            }
            5 => fs::remove_file(package.join(release::binary_name())).unwrap(),
            6 => {
                manifest.runtime.target_schema = 37;
                fixture.sign_manifest(&package, &manifest);
            }
            _ => fs::write(package.join("release.sig"), "0".repeat(128)).unwrap(),
        }
        let before = fingerprint(fixture.temp.path());
        assert!(fixture.manager(&runner, &AMPLE).update().is_err());
        assert_eq!(before, fingerprint(fixture.temp.path()));
    }
}

#[test]
fn corrupt_checkpoint_blocks_restore_and_activation_recovery_explicitly() {
    let runner = ModelRunner::new();
    let fixture = Fixture::new(&runner);
    fixture.package("0.1.1", None);
    let mut manager = fixture.manager(&runner, &AMPLE);
    manager.backup().unwrap();
    let id = manager.state.backups.keys().next().unwrap().clone();
    let root = fixture.install.join("backups").join(&id);
    fs::write(
        root.join("board").join(crate::BOARD_CONFIG_FILE),
        b"corrupt",
    )
    .unwrap();
    let before = fingerprint(&fixture.board);
    assert!(manager.restore(&id).is_err());
    assert_eq!(before, fingerprint(&fixture.board));
    manager.fault = Some((Phase::ApplyRuntime, true));
    assert!(manager.update().is_err());
    let transaction = manager.state.transaction.clone().unwrap();
    let recovery = fixture
        .install
        .join("backups")
        .join(transaction.checkpoint.unwrap());
    fs::write(recovery.join(MANIFEST), b"corrupt").unwrap();
    drop(manager);
    assert!(fixture.manager(&runner, &AMPLE).recover().is_err());
    assert!(check_board_access(&fixture.board).is_err());
}

#[test]
fn unsafe_paths_overflow_and_unknown_documents_are_rejected() {
    let secret = "D1-SECRET-MUST-NOT-APPEAR-IN-ERRORS";
    let invalid = format!("password = \"{secret}\" [broken");
    let config = sf_core::RuntimeConfig::from_toml(&invalid).unwrap_err();
    assert!(config.to_string().contains(secret));
    assert!(!Error::Config(config).to_string().contains(secret));
    let config = sf_core::RuntimeConfig::from_toml(&invalid).unwrap_err();
    assert!(!Error::application(crate::ApplicationError::Config(config))
        .to_string()
        .contains(secret));
    let toml = toml::from_str::<Installation>(&invalid).unwrap_err();
    assert!(!Error::Toml(toml).to_string().contains(secret));

    for path in [
        "../escape",
        "a/../escape",
        "/absolute",
        "a\\b",
        "a:b",
        "trailing.",
        "a//b",
    ] {
        assert!(io::relative(path).is_err());
    }
    assert!(io::add(u64::MAX, 1).is_err());
    assert!(io::multiply(u64::MAX, 2).is_err());
    let mut value = serde_json::to_value(RuntimeDescriptor::current()).unwrap();
    value["untrusted_future_field"] = true.into();
    assert!(serde_json::from_value::<RuntimeDescriptor>(value).is_err());
}

#[test]
#[ignore = "requires separately compiled disposable signed runtimes; tools/build-d1-fixtures.py"]
fn native_cli_acceptance() {
    use std::process::{Command, Output};
    fn cli(binary: &Path, args: &[&str]) -> Output {
        Command::new(binary)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    }
    fn success(binary: &Path, args: &[&str]) -> String {
        let out = cli(binary, args);
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        println!("{}", text);
        text
    }
    fn offer(packages: &Path, source: &Path, version: &str) {
        let destination = source.join(version);
        fs::create_dir(&destination).unwrap();
        for (_, path) in io::inventory(&packages.join(version)).unwrap() {
            fs::copy(&path, destination.join(path.file_name().unwrap())).unwrap();
        }
    }
    let fixtures = PathBuf::from(
        std::env::var_os("SPITFIRE_D1_FIXTURES").expect("run tools/build-d1-fixtures.py first"),
    );
    let packages = fixtures.join("packages");
    let temp = tempfile::tempdir().unwrap();
    let board = temp.path().join("board");
    let key = populate_board(&board);
    let config = board.join(crate::BOARD_CONFIG_FILE);
    fs::copy(
        fixtures.join("synthetic-certificate.pem"),
        board.join("system/D1-CERT.pem"),
    )
    .unwrap();
    let install = temp.path().join("installation");
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    offer(&packages, &source, "0.1.0");
    let original = source.join("0.1.0").join(release::binary_name());
    success(
        &original,
        &[
            "deployment",
            "adopt",
            config.to_str().unwrap(),
            install.to_str().unwrap(),
            source.to_str().unwrap(),
            fixtures
                .join("test-release-key.der.public.hex")
                .to_str()
                .unwrap(),
        ],
    );
    let launcher = install.join("bin").join(release::binary_name());
    let database = worker::paths(&config).unwrap().1.database().to_path_buf();
    let initial = worker::database_fingerprints(&database).unwrap();
    offer(&packages, &source, "0.1.1");
    let untouched = fingerprint(&board);
    let journal = io::hash(&install.join("deployment.sqlite3")).unwrap();
    assert!(success(&launcher, &["update", "--check"]).contains("0.1.1"));
    assert!(success(&launcher, &["update", "--dry-run"])
        .contains("No installation or board files changed"));
    assert_eq!(untouched, fingerprint(&board));
    assert_eq!(
        journal,
        io::hash(&install.join("deployment.sqlite3")).unwrap()
    );
    assert!(success(&launcher, &["update"]).contains("0.1.0 -> 0.1.1"));
    assert_eq!(initial, worker::database_fingerprints(&database).unwrap());
    offer(&packages, &source, "0.1.5");
    assert!(success(&launcher, &["update"]).contains("0.1.1 -> 0.1.5"));
    assert_eq!(initial, worker::database_fingerprints(&database).unwrap());
    assert!(success(&launcher, &["version"]).contains("0.1.5"));
    offer(&packages, &source, "0.1.9");
    let before_failure = fingerprint(&board);
    let failure = cli(&launcher, &["update"]);
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr)
        .contains("Pre-upgrade recovery and validation: OK"));
    assert_eq!(before_failure, fingerprint(&board));
    assert!(success(&launcher, &["version"]).contains("0.1.5"));
    fs::remove_dir_all(source.join("0.1.9")).unwrap();
    new_evidence(&board, &key);
    let newer = fingerprint(&board);
    let evidence = worker::database_fingerprints(&database).unwrap();
    assert!(!cli(&launcher, &["rollback"]).status.success());
    assert_eq!(newer, fingerprint(&board));
    assert!(success(&launcher, &["rollback", "--yes"]).contains("0.1.5 -> 0.1.1"));
    assert_eq!(newer, fingerprint(&board));
    assert_eq!(evidence, worker::database_fingerprints(&database).unwrap());
    assert!(success(&launcher, &["update"]).contains("0.1.1 -> 0.1.5"));
    assert_eq!(evidence, worker::database_fingerprints(&database).unwrap());
    let backup = success(&launcher, &["backup"]);
    assert!(backup.contains("external payloads referenced (not copied): 1"));
    success(&launcher, &["backup", "list"]);
    let (_, control) = control::load(&install).unwrap();
    let id = control
        .backups
        .keys()
        .find(|id| {
            snapshot::verify(&install.join("backups").join(id), None)
                .unwrap()
                .backup_type
                == "manual"
        })
        .unwrap();
    fs::write(board.join("display/D1.BBS"), b"later resource").unwrap();
    assert!(!cli(&launcher, &["restore", id]).status.success());
    assert!(success(&launcher, &["restore", id, "--replace"]).contains("Disaster restore complete"));
    assert_eq!(
        fs::read(board.join("display/D1.BBS")).unwrap(),
        b"Synthetic D1 display\r\n"
    );
    offer(&packages, &source, "0.1.6");
    success(&launcher, &["update"]);
    let final_board = fingerprint(&board);
    let final_journal = io::hash(&install.join("deployment.sqlite3")).unwrap();
    let refused = cli(&launcher, &["rollback", "--yes"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("d1-synthetic-future-feature"));
    assert_eq!(final_board, fingerprint(&board));
    assert_eq!(
        final_journal,
        io::hash(&install.join("deployment.sqlite3")).unwrap()
    );
    println!("NATIVE D1 ACCEPTANCE: .0 -> .1 -> .5; precommit failure; newer catalog/FTN evidence; runtime rollback; repeat update; backup/restore; future-feature refusal: PASS");
}

#[test]
fn staged_schema_migration_is_forward_restart_safe_and_preserves_existing_tables() {
    let temp = tempfile::tempdir().unwrap();
    let board = temp.path().join("board");
    populate_board(&board);
    let config = board.join(crate::BOARD_CONFIG_FILE);
    let paths = worker::paths(&config).unwrap().1;
    let db = rusqlite::Connection::open(paths.database()).unwrap();
    let objects:Vec<(String,String)>=db.prepare("SELECT type,name FROM sqlite_master WHERE name LIKE 'conference_health_%' ORDER BY CASE type WHEN 'trigger' THEN 0 WHEN 'index' THEN 1 ELSE 2 END").unwrap()
        .query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<std::result::Result<_,_>>().unwrap();
    for (kind, name) in objects {
        db.execute_batch(&format!("DROP {} IF EXISTS \"{}\"", kind, name))
            .unwrap();
    }
    db.execute("DELETE FROM schema_migrations WHERE version=36", [])
        .unwrap();
    drop(db);
    let before = worker::database_fingerprints(paths.database()).unwrap();
    let request = WorkerRequest {
        format: FORMAT,
        operation: "migrate".into(),
        config,
        payload_root: board,
    };
    let after = worker::execute(&request).unwrap();
    assert_eq!(after.schema, 36);
    for (table, hash) in before.1 {
        assert_eq!(after.identities.get(&table), Some(&hash), "{table}");
    }
    let rerun = worker::execute(&request).unwrap();
    assert_eq!(after, rerun);
}

#[test]
fn verification_and_partial_activation_failures_restore_before_commit() {
    for phase in [
        Phase::VerifyBackup,
        Phase::ApplyRuntime,
        Phase::ValidateActive,
    ] {
        let runner = ModelRunner::new();
        let fixture = Fixture::new(&runner);
        fixture.package("0.1.1", None);
        let mut before = fingerprint(&fixture.board);
        let rows = worker::database_fingerprints(&fixture.db()).unwrap();
        let database_relative = fixture
            .db()
            .strip_prefix(io::real(&fixture.board, true).unwrap())
            .unwrap()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        before.remove(&database_relative);
        let mut manager = fixture.manager(&runner, &AMPLE);
        manager.fault = Some((phase, false));
        let error = manager.update().unwrap_err().to_string();
        assert!(
            error.contains("Pre-upgrade recovery and validation: OK"),
            "{error}"
        );
        let mut recovered = fingerprint(&fixture.board);
        recovered.remove(&database_relative);
        assert_eq!(before, recovered);
        // SQLite's snapshot API changes header bookkeeping, not durable rows.
        assert_eq!(rows, worker::database_fingerprints(&fixture.db()).unwrap());
        assert_eq!(manager.state.active, "0.1.0");
        assert!(manager.state.transaction.is_none());
        manager.fault = None;
        manager.update().unwrap();
    }
}

#[test]
fn maintenance_conflicts_and_missing_retained_runtime_refuse_without_mutation() {
    let runner = ModelRunner::new();
    let fixture = Fixture::new(&runner);
    fixture.package("0.1.1", None);
    let lock = control::InstallLock::acquire(&fixture.install).unwrap();
    let before = fingerprint(fixture.temp.path());
    assert!(fixture
        .manager(&runner, &AMPLE)
        .update()
        .unwrap_err()
        .to_string()
        .contains("another deployment"));
    assert_eq!(before, fingerprint(fixture.temp.path()));
    drop(lock);
    let mut manager = fixture.manager(&runner, &AMPLE);
    manager.update().unwrap();
    fs::remove_file(manager.executable("0.1.0").unwrap()).unwrap();
    let before = fingerprint(fixture.temp.path());
    assert!(manager.rollback(true).is_err());
    assert_eq!(before, fingerprint(fixture.temp.path()));
}
