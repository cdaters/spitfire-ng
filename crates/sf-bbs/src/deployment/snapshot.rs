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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use sf_core::{LogicalPath, RuntimeDatabase};

use super::io::{self, Capacity};
use super::model::*;
use super::worker;
use super::{ensure, Error, Result};

pub struct Plan {
    pub snapshot: Snapshot,
    pub source_root: PathBuf,
    pub critical_bytes: u64,
}
#[derive(Debug)]
pub struct SpaceReport {
    pub backup_bytes: u64,
    pub installation_required: u64,
    pub board_required: u64,
    pub warning: bool,
}

pub fn plan(installation: &Installation, source: &RuntimeDescriptor, kind: &str) -> Result<Plan> {
    let state = worker::inspect(
        &installation.config,
        installation
            .config
            .parent()
            .ok_or_else(|| Error::Rejected("missing board root".into()))?,
        false,
    )?;
    source.compatible(&state)?;
    let (config, paths) = worker::paths(&installation.config)?;
    let root = paths.root();
    let config_name = installation
        .config
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::Rejected("config filename must be UTF-8".into()))?
        .to_owned();
    io::relative(&config_name)?;
    let mut entries = BTreeMap::new();
    let mut add_path = |path: &Path| -> Result<()> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::Rejected("critical state is outside board root".into()))?
            .to_str()
            .ok_or_else(|| Error::Rejected("state path must be UTF-8".into()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        io::relative(&relative)?;
        let (size_bytes, sha256) = io::hash(path)?;
        entries.insert(
            relative.clone(),
            Entry {
                path: relative,
                class: StateClass::Critical,
                size_bytes,
                sha256,
            },
        );
        Ok(())
    };
    add_path(&installation.config)?;
    add_path(paths.database())?;
    for logical in [LogicalPath::System, LogicalPath::Display] {
        for (_, path) in io::inventory(paths.get(logical))? {
            add_path(&path)?;
        }
    }
    let critical_bytes = entries
        .values()
        .try_fold(0, |sum, e| io::add(sum, e.size_bytes))?;
    let db = RuntimeDatabase::open_read_only(paths.database())?;
    let connection =
        Connection::open_with_flags(paths.database(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut references = Vec::new();
    let managed_catalog: BTreeMap<_, _> = db
        .managed_cataloged_files()?
        .into_iter()
        .map(|(a, f)| (f.id.get(), (a, f)))
        .collect();
    let rows = connection.prepare("SELECT f.file_id,a.area_number,r.root_kind,r.configured_locator,r.availability,l.relative_path,f.size_bytes,f.sha256 FROM files f JOIN file_areas a USING(area_id) JOIN file_storage_locators l USING(file_id) JOIN file_storage_roots r USING(storage_root_id) ORDER BY f.file_id")?
        .query_map([], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,u16>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,i64>(6)?,r.get::<_,String>(7)?)))?
        .collect::<std::result::Result<Vec<_>,_>>()?;
    let mut area_bytes = BTreeMap::<u16, u64>::new();
    let managed_copy = installation.storage.file_payloads == PayloadPolicy::ManagedOnly;
    for (id, area, kind, storage_root, availability, relative, size_bytes, sha256) in rows {
        let size_bytes =
            u64::try_from(size_bytes).map_err(|_| Error::Rejected("negative file size".into()))?;
        let managed = kind == "managed";
        ensure(managed || kind == "external", "unknown Files storage class")?;
        io::unhex(&sha256, 32)?;
        let path = if managed {
            let file_id = sf_core::FileId::new(id)?;
            if state.schema >= 33
                && db
                    .file_admission(file_id)
                    .map_err(|_| Error::Rejected("invalid file admission".into()))?
                    .is_some()
            {
                format!(
                    "{}/files/.content/{sha256}",
                    config.paths.external.display()
                )
            } else {
                // Managed locator authority is relative to the board root's
                // external directory; use the domain catalog's area key.
                let (file_area, file) = managed_catalog.get(&id).ok_or_else(|| {
                    Error::Rejected("managed file catalog/locator mismatch".into())
                })?;
                format!(
                    "{}/files/{}/{}",
                    config.paths.external.display(),
                    file_area.storage_key,
                    file.filename
                )
            }
        } else {
            relative
        };
        if managed {
            io::relative(&path)?;
            ensure(
                size_bytes <= installation.storage.max_single_file_bytes,
                "managed single-file storage threshold exceeded",
            )?;
            let total = io::add(*area_bytes.get(&area).unwrap_or(&0), size_bytes)?;
            ensure(
                total <= installation.storage.max_file_area_bytes,
                "managed File Area storage threshold exceeded",
            )?;
            area_bytes.insert(area, total);
            ensure(
                io::hash(&root.join(&path))? == (size_bytes, sha256.clone()),
                "managed payload size/hash mismatch",
            )?;
            if managed_copy {
                entries.insert(
                    path.clone(),
                    Entry {
                        path: path.clone(),
                        class: StateClass::ManagedPayload,
                        size_bytes,
                        sha256: sha256.clone(),
                    },
                );
            }
        }
        references.push(PayloadReference {
            class: if managed {
                StateClass::ManagedPayload
            } else {
                StateClass::ExternalReference
            },
            file_id: Some(id),
            area: Some(area),
            path,
            storage_root,
            availability,
            size_bytes,
            sha256,
            copied: managed && managed_copy,
        });
    }
    // Quarantine and unlinked retained native content are authoritative managed
    // bytes too, even when no Files row currently selects them.
    if state.schema >= 33 {
        for (sha256, size_bytes) in db
            .content_catalog()
            .map_err(|_| Error::Rejected("invalid content catalog".into()))?
        {
            let path = format!(
                "{}/files/.content/{sha256}",
                config.paths.external.display()
            );
            ensure(
                size_bytes <= installation.storage.max_single_file_bytes,
                "managed single-file storage threshold exceeded",
            )?;
            ensure(
                io::hash(&root.join(&path))? == (size_bytes, sha256.clone()),
                "native content size/hash mismatch",
            )?;
            if !references
                .iter()
                .any(|r| r.class == StateClass::ManagedPayload && r.path == path)
            {
                references.push(PayloadReference {
                    class: StateClass::ManagedPayload,
                    file_id: None,
                    area: None,
                    path: path.clone(),
                    storage_root: "native-content".into(),
                    availability: "verified".into(),
                    size_bytes,
                    sha256: sha256.clone(),
                    copied: managed_copy,
                });
            }
            if managed_copy {
                entries.insert(
                    path.clone(),
                    Entry {
                        path,
                        class: StateClass::ManagedPayload,
                        size_bytes,
                        sha256,
                    },
                );
            }
        }
    }
    let unique: BTreeMap<_, _> = references
        .iter()
        .filter(|r| r.class == StateClass::ManagedPayload)
        .map(|r| (&r.path, r.size_bytes))
        .collect();
    let managed_total = unique.values().try_fold(0, |sum, n| io::add(sum, *n))?;
    ensure(
        managed_total <= installation.storage.max_managed_files_bytes,
        "total managed Files storage threshold exceeded",
    )?;
    let copied_payload_bytes = entries
        .values()
        .filter(|e| e.class == StateClass::ManagedPayload)
        .try_fold(0, |sum, e| io::add(sum, e.size_bytes))?;
    let stored_content_bytes = io::add(critical_bytes, copied_payload_bytes)?;
    let total_logical_bytes = references
        .iter()
        .try_fold(critical_bytes, |sum, r| io::add(sum, r.size_bytes))?;
    ensure(
        entries.len() + references.len() <= MAX_ENTRIES,
        "backup inventory exceeds limit",
    )?;
    let snapshot = Snapshot {
        format: FORMAT,
        id: io::id(),
        installation_id: installation.id.clone(),
        created_at: chrono::Utc::now().timestamp(),
        backup_type: kind.into(),
        installation: installation.clone(),
        source_runtime: source.clone(),
        state,
        config_name,
        payload_policy: installation.storage.file_payloads,
        entries: entries.into_values().collect(),
        references,
        excluded_classes: vec![
            "runtime-binaries (retained separately under releases)".into(),
            "external payload bytes".into(),
            "logs, runtime endpoints, incomplete staging and derived caches outside critical resource trees".into(),
            "uncataloged payloads and other files outside the declared critical/resource inventory".into(),
        ],
        total_logical_bytes,
        stored_content_bytes,
        restore_requirements: vec![
            "compatible runtime".into(),
            "explicit destructive restore confirmation".into(),
            "matching managed payloads from stopped target if referenced".into(),
            "external media verified separately; native network recovery guards apply".into(),
        ],
    };
    Ok(Plan {
        snapshot,
        source_root: root.to_path_buf(),
        critical_bytes,
    })
}

pub fn space(
    plan: &Plan,
    policy: &StoragePolicy,
    install: &Path,
    runtime_bytes: u64,
    update: bool,
    capacity: &dyn Capacity,
) -> Result<SpaceReport> {
    policy.validate()?;
    let allowance = MAX_DOCUMENT;
    let backup_bytes = io::add(plan.snapshot.stored_content_bytes, allowance)?;
    ensure(
        backup_bytes <= policy.backup_max_bytes,
        "required backup exceeds configured backup maximum; no update was started",
    )?;
    let retained = io::tree_bytes(&install.join("backups"))?;
    ensure(
        io::add(retained, backup_bytes)? <= policy.backup_retention_bytes,
        "backup retention capacity exceeded; existing checkpoints were not pruned",
    )?;
    let installation_required = io::add(
        backup_bytes,
        if update {
            io::add(
                io::multiply(plan.critical_bytes, 2)?,
                io::add(runtime_bytes, allowance)?,
            )?
        } else {
            0
        },
    )?;
    let board_required = if update {
        io::multiply(plan.critical_bytes, 2)?
    } else {
        0
    };
    // Conservative even when both directories share a volume: never count its
    // free space twice. This also avoids platform-specific volume identity rules.
    let combined = io::add(installation_required, board_required)?;
    let mut warning = false;
    for path in [install, plan.source_root.as_path()] {
        let available = capacity.available(path)?;
        ensure(
            available >= policy.minimum_free_bytes && available >= policy.hard_stop_free_bytes,
            "free space is below the configured hard-stop/minimum threshold",
        )?;
        let remaining = available.checked_sub(combined).ok_or_else(|| {
            Error::Rejected("insufficient space for verified backup, staging and recovery".into())
        })?;
        ensure(
            remaining >= policy.reserve_free_bytes.max(policy.hard_stop_free_bytes),
            "operation would cross the configured free-space reserve",
        )?;
        warning |= remaining < policy.low_space_warning_bytes;
    }
    Ok(SpaceReport {
        backup_bytes,
        installation_required,
        board_required,
        warning,
    })
}

pub fn create(plan: &Plan, destination: &Path) -> Result<(Snapshot, String)> {
    ensure(!destination.exists(), "backup destination already exists")?;
    let parent = destination
        .parent()
        .ok_or_else(|| Error::Rejected("missing backup parent".into()))?;
    let temp = tempfile::Builder::new()
        .prefix(".checkpoint-")
        .tempdir_in(parent)?;
    io::private_dir(&temp.path().join("board"))?;
    let (config, paths) = worker::paths(&plan.source_root.join(&plan.snapshot.config_name))?;
    let database_relative = paths
        .database()
        .strip_prefix(&plan.source_root)
        .map_err(|_| Error::Rejected("invalid database path".into()))?
        .to_str()
        .ok_or_else(|| Error::Rejected("database path is not UTF-8".into()))?
        .replace(std::path::MAIN_SEPARATOR, "/");
    let mut snapshot = plan.snapshot.clone();
    for entry in &mut snapshot.entries {
        let path = io::parents(&temp.path().join("board"), &entry.path)?;
        let actual = if entry.path == database_relative {
            RuntimeDatabase::open_read_only(paths.database())?.backup_to(&path)?;
            io::sync_dir(
                path.parent()
                    .ok_or_else(|| Error::Rejected("missing snapshot database parent".into()))?,
            )?;
            io::hash(&path)?
        } else {
            ensure(
                io::hash(&plan.source_root.join(&entry.path))?
                    == (entry.size_bytes, entry.sha256.clone()),
                "board changed after backup planning",
            )?;
            io::copy(&plan.source_root.join(&entry.path), &path)?
        };
        if entry.path != database_relative {
            ensure(
                actual == (entry.size_bytes, entry.sha256.clone()),
                "backup copy verification failed",
            )?;
        }
        entry.size_bytes = actual.0;
        entry.sha256 = actual.1;
    }
    // Empty logical directories are structural, not included-state omissions.
    sf_core::LogicalPaths::resolve(&temp.path().join("board"), &config.validate()?)?
        .create_directories()?;
    snapshot.stored_content_bytes = snapshot
        .entries
        .iter()
        .try_fold(0, |sum, e| io::add(sum, e.size_bytes))?;
    let hash = io::write_json(&temp.path().join(MANIFEST), &snapshot)?;
    verify(temp.path(), Some(&hash))?;
    io::sync_dir(&temp.path().join("board"))?;
    io::sync_dir(temp.path())?;
    let staging = temp.keep();
    fs::rename(&staging, destination)?;
    io::sync_dir(parent)?;
    Ok((snapshot, hash))
}

pub fn verify(root: &Path, expected_hash: Option<&str>) -> Result<Snapshot> {
    let bytes = io::read(&root.join(MANIFEST), MAX_DOCUMENT)?;
    if let Some(expected) = expected_hash {
        ensure(
            io::digest(&bytes) == expected,
            "backup manifest integrity mismatch",
        )?;
    }
    let snapshot: Snapshot = serde_json::from_slice(&bytes)?;
    ensure(
        snapshot.format == FORMAT && snapshot.created_at >= 0,
        "unsupported recovery snapshot format",
    )?;
    io::safe_id(&snapshot.id)?;
    io::safe_id(&snapshot.installation_id)?;
    ensure(
        snapshot.installation.id == snapshot.installation_id
            && snapshot.installation.format == FORMAT,
        "snapshot installation metadata mismatch",
    )?;
    snapshot.installation.storage.validate()?;
    io::unhex(&snapshot.installation.release_public_key, 32)?;
    snapshot.source_runtime.validate()?;
    snapshot.source_runtime.compatible(&snapshot.state)?;
    ensure(
        snapshot.entries.len() + snapshot.references.len() <= MAX_ENTRIES,
        "excessive backup inventory",
    )?;
    io::relative(&snapshot.config_name)?;
    ensure(
        !snapshot.config_name.contains('/'),
        "invalid snapshot config filename",
    )?;
    let board = io::real(&root.join("board"), true)?;
    let (config, paths) = worker::paths(&board.join(&snapshot.config_name))?;
    let database_relative = paths
        .database()
        .strip_prefix(&board)
        .map_err(|_| Error::Rejected("snapshot database escaped root".into()))?
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let references_by_id: BTreeMap<_, _> = snapshot
        .references
        .iter()
        .filter_map(|r| r.file_id.map(|id| (id, r)))
        .collect();
    ensure(
        references_by_id.len()
            == snapshot
                .references
                .iter()
                .filter(|r| r.file_id.is_some())
                .count(),
        "duplicate payload identity",
    )?;
    let copied: BTreeSet<_> = snapshot
        .references
        .iter()
        .filter(|r| r.class == StateClass::ManagedPayload && r.copied)
        .map(|r| (&r.path, r.size_bytes, &r.sha256))
        .collect();
    let entries_by_path: BTreeMap<_, _> = snapshot.entries.iter().map(|e| (&e.path, e)).collect();
    let mut names = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total = 0;
    for entry in &snapshot.entries {
        io::relative(&entry.path)?;
        io::unhex(&entry.sha256, 32)?;
        ensure(
            names.insert(entry.path.clone()) && folded.insert(entry.path.to_ascii_lowercase()),
            "duplicate snapshot entry",
        )?;
        let allowed = match entry.class {
            StateClass::Critical => {
                entry.path == snapshot.config_name
                    || entry.path == database_relative
                    || Path::new(&entry.path).starts_with(&config.paths.system)
                    || Path::new(&entry.path).starts_with(&config.paths.display)
            }
            StateClass::ManagedPayload => {
                copied.contains(&(&entry.path, entry.size_bytes, &entry.sha256))
            }
            _ => false,
        };
        ensure(allowed, "snapshot entry violates state-class boundary")?;
        ensure(
            io::hash(&board.join(&entry.path))? == (entry.size_bytes, entry.sha256.clone()),
            "backup entry integrity mismatch",
        )?;
        total = io::add(total, entry.size_bytes)?;
    }
    ensure(
        names.contains(&snapshot.config_name) && names.contains(&database_relative),
        "backup missing critical database/configuration",
    )?;
    let actual: BTreeSet<_> = io::inventory(&board)?
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    ensure(
        names == actual && total == snapshot.stored_content_bytes,
        "backup inventory/size mismatch",
    )?;
    let top = io::inventory(root)?;
    ensure(
        top.len() == names.len() + 1,
        "undeclared files in recovery snapshot",
    )?;
    let db = RuntimeDatabase::open_read_only(paths.database())?;
    let identity = db.validate_snapshot_at_version(snapshot.state.schema)?;
    ensure(
        config.format_version == snapshot.state.config_format
            && identity == config.validate()?.identity
            && identity.name() == snapshot.state.board_name
            && identity.sysop_name() == snapshot.state.sysop_name,
        "backup identity/schema mismatch",
    )?;
    let (counts, identities) = worker::database_fingerprints(paths.database())?;
    ensure(
        counts == snapshot.state.counts && identities == snapshot.state.identities,
        "backup durable-state fingerprint mismatch",
    )?;
    for reference in &snapshot.references {
        io::unhex(&reference.sha256, 32)?;
        ensure(
            matches!(
                reference.class,
                StateClass::ManagedPayload | StateClass::ExternalReference
            ),
            "invalid payload reference class",
        )?;
        if reference.class == StateClass::ExternalReference {
            ensure(
                !reference.copied,
                "external content must remain explicitly referenced",
            )?;
        } else {
            io::relative(&reference.path)?;
            ensure(
                Path::new(&reference.path).starts_with(config.paths.external.join("files")),
                "managed reference escaped Files root",
            )?;
            ensure(
                reference.copied == (snapshot.payload_policy == PayloadPolicy::ManagedOnly),
                "payload policy/reference mismatch",
            )?;
            ensure(
                !reference.copied
                    || entries_by_path.get(&reference.path).is_some_and(|e| {
                        e.class == StateClass::ManagedPayload
                            && e.size_bytes == reference.size_bytes
                            && e.sha256 == reference.sha256
                    }),
                "copied payload missing from snapshot",
            )?;
        }
    }
    // Exact locator/size/hash coverage is checked from the backed database,
    // including disabled external roots, without touching external media.
    let connection =
        Connection::open_with_flags(paths.database(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut query=connection.prepare("SELECT f.file_id,f.size_bytes,f.sha256,r.root_kind,r.configured_locator,r.availability,l.relative_path,a.area_number FROM files f JOIN file_areas a USING(area_id) JOIN file_storage_locators l USING(file_id) JOIN file_storage_roots r USING(storage_root_id) ORDER BY f.file_id")?;
    let mut rows = query.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let size: i64 = row.get(1)?;
        let hash: String = row.get(2)?;
        let kind: String = row.get(3)?;
        let storage_root: String = row.get(4)?;
        let availability: String = row.get(5)?;
        let relative: String = row.get(6)?;
        let area: u16 = row.get(7)?;
        let size =
            u64::try_from(size).map_err(|_| Error::Rejected("negative backed file size".into()))?;
        ensure(
            references_by_id.get(&id).is_some_and(|r| {
                r.size_bytes == size
                    && r.sha256 == hash
                    && r.area == Some(area)
                    && r.storage_root == storage_root
                    && r.availability == availability
                    && (r.class == StateClass::ManagedPayload) == (kind == "managed")
                    && (kind != "external" || r.path == relative)
            }),
            "backup payload reference disagrees with database",
        )?;
        count += 1;
    }
    ensure(
        count == references_by_id.len(),
        "backup payload reference inventory mismatch",
    )?;
    Ok(snapshot)
}

pub fn stage(snapshot_root: &Path, snapshot: &Snapshot, destination: &Path) -> Result<()> {
    io::private_dir(destination)?;
    for entry in snapshot
        .entries
        .iter()
        .filter(|e| e.class == StateClass::Critical)
    {
        let actual = io::copy(
            &snapshot_root.join("board").join(&entry.path),
            &io::parents(destination, &entry.path)?,
        )?;
        ensure(
            actual == (entry.size_bytes, entry.sha256.clone()),
            "staging checkpoint integrity mismatch",
        )?;
    }
    let config = sf_core::RuntimeConfig::load(&destination.join(&snapshot.config_name))?;
    sf_core::LogicalPaths::resolve(destination, &config.validate()?)?.create_directories()?;
    Ok(())
}

pub fn unchanged_resources(snapshot: &Snapshot, stage: &Path) -> Result<()> {
    let stage = io::real(stage, true)?;
    let (_, paths) = worker::paths(&stage.join(&snapshot.config_name))?;
    let database = paths
        .database()
        .strip_prefix(&stage)
        .map_err(|_| Error::Rejected("invalid stage path".into()))?
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    for e in snapshot.entries.iter().filter(|e| {
        e.class == StateClass::Critical && e.path != snapshot.config_name && e.path != database
    }) {
        ensure(
            io::hash(&stage.join(&e.path))? == (e.size_bytes, e.sha256.clone()),
            "runtime attempted an unsupported board-resource migration",
        )?;
    }
    Ok(())
}

pub fn restore_precommit(root: &Path, snapshot: &Snapshot, config_path: &Path) -> Result<()> {
    let source_config = root.join("board").join(&snapshot.config_name);
    let (_, source_paths) = worker::paths(&source_config)?;
    let target_root = config_path
        .parent()
        .ok_or_else(|| Error::Rejected("missing board root".into()))?;
    let config = sf_core::RuntimeConfig::load(&source_config)?;
    let target_paths = sf_core::LogicalPaths::resolve(target_root, &config.validate()?)?;
    // Resources and payloads never enter D1's write set. Verify critical
    // resource custody before replacing only the database and config.
    unchanged_resources(snapshot, target_root)?;
    clean_sidecars(target_paths.database())?;
    io::replace(source_paths.database(), target_paths.database())?;
    io::replace(&source_config, config_path)?;
    ensure(
        worker::database_fingerprints(target_paths.database())?.1 == snapshot.state.identities,
        "pre-upgrade recovery state verification failed",
    )
}
pub fn clean_sidecars(database: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = database.as_os_str().to_os_string();
        name.push(suffix);
        let path = PathBuf::from(name);
        if path.exists() {
            io::real(&path, false)?;
            fs::remove_file(&path)?;
        }
    }
    io::sync_dir(
        database
            .parent()
            .ok_or_else(|| Error::Rejected("missing database parent".into()))?,
    )
}
