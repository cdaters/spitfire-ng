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
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sf_core::{
    EventAttributes, EventCategory, EventId, EventOutcome, EventSeverity, FileStorage, LogicalPath,
    LogicalPaths, NewOperationalEvent, RuntimeConfig, RuntimeDatabase, TerminalInfo,
    ValidatedConfig, SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::board_lock::BoardOperationLock;
use crate::resources::load_stock_resources;
use crate::ApplicationError;

pub const BACKUP_MANIFEST_FILE: &str = "spitfire-backup.toml";
const BACKUP_FORMAT_VERSION: u32 = 1;
const DATABASE_BACKUP_PATH: &str = "database/runtime.sqlite3";
const MAX_BACKUP_ENTRIES: usize = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupReport {
    pub destination: PathBuf,
    pub board_name: String,
    pub schema_version: u32,
    pub resource_files: usize,
    pub cataloged_files: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreReport {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub board_name: String,
    pub schema_version: u32,
    pub resource_files: usize,
    pub cataloged_files: usize,
    pub replaced_existing: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum BackupEntryKind {
    Configuration,
    Database,
    SystemResource,
    DisplayResource,
    CatalogedFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BackupEntry {
    kind: BackupEntryKind,
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BackupManifest {
    format_version: u32,
    created_by_version: String,
    created_at: i64,
    schema_version: u32,
    board_name: String,
    sysop_name: String,
    config_name: String,
    entries: Vec<BackupEntry>,
}

struct ValidatedBackup {
    root: PathBuf,
    manifest: BackupManifest,
    config: RuntimeConfig,
    resource_files: usize,
    cataloged_files: usize,
}

struct BackupObservationGuard {
    database_path: PathBuf,
    started_event_id: EventId,
    started_at: i64,
    finished: bool,
}

impl BackupObservationGuard {
    fn mark_finished(&mut self) {
        self.finished = true;
    }
}

impl Drop for BackupObservationGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut failed = NewOperationalEvent::new(
            now_unix_seconds().unwrap_or(self.started_at),
            EventCategory::Backup,
            EventSeverity::Error,
            "backup.failed",
            EventOutcome::Failed,
        );
        failed.correlation_id = Some(format!("backup-{}", self.started_event_id.get()));
        failed.idempotency_key = Some(format!("backup-failed-{}", self.started_event_id.get()));
        failed.attributes = EventAttributes::Backup {
            state: "failed".to_owned(),
            bytes: None,
        };
        let result = RuntimeDatabase::open(&self.database_path)
            .and_then(|mut database| database.record_operational_event(&failed));
        if let Err(error) = result {
            tracing::warn!(error = %error, "backup failure could not be recorded as an operational event");
        }
    }
}

/// Creates one cold, immutable-by-convention directory snapshot. The caller
/// supplies a nonexistent destination outside the managed board tree.
pub fn backup_board(
    config_path: &Path,
    destination: &Path,
) -> Result<BackupReport, ApplicationError> {
    let canonical_config = canonical_regular_file(config_path)?;
    let root = canonical_config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| ApplicationError::MissingBoardRoot(canonical_config.clone()))?
        .to_path_buf();
    let _operation_lock = BoardOperationLock::acquire(&root)?;
    let destination = new_path_with_existing_parent(destination)?;
    if destination.exists() {
        return Err(BoardBackupError::DestinationExists(destination).into());
    }

    let config = RuntimeConfig::load(&canonical_config)?;
    let validated = config.validate()?;
    let paths = LogicalPaths::resolve(&root, &validated)?;
    validate_native_layout(&validated, &paths)?;
    validate_real_logical_directories(&paths)?;
    reject_destination_inside_board(&destination, &root, &paths)?;

    let mut database = RuntimeDatabase::open(paths.database())?;
    let identity = database.validate_current_snapshot()?;
    if identity != validated.identity {
        return Err(sf_core::DatabaseError::BoardIdentityMismatch {
            configured_name: validated.identity.name().to_owned(),
            configured_sysop: validated.identity.sysop_name().to_owned(),
            stored_name: identity.name().to_owned(),
            stored_sysop: identity.sysop_name().to_owned(),
        }
        .into());
    }
    if database.schema_version()? >= 15 && !database.file_operations_ready_for_cold_backup()? {
        return Err(BoardBackupError::UnnormalizedFileOperations.into());
    }
    if database.schema_version()? >= 16 && !database.transfer_operations_ready_for_cold_backup()? {
        return Err(BoardBackupError::UnnormalizedTransferOperations.into());
    }
    let backup_started_at = now_unix_seconds()?;
    let mut started = NewOperationalEvent::new(
        backup_started_at,
        EventCategory::Backup,
        EventSeverity::Notice,
        "backup.started",
        EventOutcome::Observed,
    );
    started.attributes = EventAttributes::Backup {
        state: "started".to_owned(),
        bytes: None,
    };
    let started = database.record_operational_event(&started)?;
    let mut observation = BackupObservationGuard {
        database_path: paths.database().to_path_buf(),
        started_event_id: started.id,
        started_at: backup_started_at,
        finished: false,
    };

    let parent = destination.parent().expect("validated destination parent");
    let temporary = tempfile::Builder::new()
        .prefix(".spitfire-backup-")
        .tempdir_in(parent)
        .map_err(|source| io_error("create backup staging directory", parent, source))?;
    let staging = temporary.path();
    let config_name = one_component_name(&canonical_config)?;
    let mut entries = Vec::new();

    let configuration_path = format!("configuration/{config_name}");
    entries.push(copy_entry(
        &canonical_config,
        staging,
        &configuration_path,
        BackupEntryKind::Configuration,
    )?);

    let database_destination = staging.join(DATABASE_BACKUP_PATH);
    create_parent_directories(&database_destination)?;
    database.backup_to(&database_destination)?;
    let snapshot = RuntimeDatabase::open_read_only(&database_destination)?;
    let snapshot_identity = snapshot.validate_current_snapshot()?;
    if snapshot_identity != identity {
        return Err(BoardBackupError::IdentityMismatch.into());
    }
    // Windows will not publish (rename) the completed staging directory while
    // SQLite still holds this validation handle open inside it.
    drop(snapshot);
    entries.push(entry_for_file(
        &database_destination,
        DATABASE_BACKUP_PATH,
        BackupEntryKind::Database,
    )?);

    copy_resource_tree(
        paths.get(LogicalPath::System),
        staging,
        "resources/system",
        BackupEntryKind::SystemResource,
        &mut entries,
    )?;
    copy_resource_tree(
        paths.get(LogicalPath::Display),
        staging,
        "resources/display",
        BackupEntryKind::DisplayResource,
        &mut entries,
    )?;

    let storage = FileStorage::open_existing(&paths)?;
    let catalog = database.managed_cataloged_files()?;
    for (area, file) in &catalog {
        let mut source = storage.open_download(area, file)?;
        let relative = format!("files/{}/{}", area.storage_key, file.filename);
        let destination_path = staging.join(&relative);
        create_parent_directories(&destination_path)?;
        let (size_bytes, sha256) = copy_reader(&mut source, &destination_path)?;
        if size_bytes != file.size_bytes || sha256 != file.sha256 {
            return Err(BoardBackupError::CatalogMismatch(relative).into());
        }
        entries.push(BackupEntry {
            kind: BackupEntryKind::CatalogedFile,
            path: relative,
            size_bytes,
            sha256,
        });
    }

    entries.sort_by(|left, right| (left.kind, &left.path).cmp(&(right.kind, &right.path)));
    ensure_entry_set_is_safe(&entries)?;
    let created_at = now_unix_seconds()?;
    let manifest = BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        created_by_version: sf_core::PRODUCT_VERSION.to_owned(),
        created_at,
        schema_version: SCHEMA_VERSION,
        board_name: identity.name().to_owned(),
        sysop_name: identity.sysop_name().to_owned(),
        config_name,
        entries,
    };
    write_manifest(staging, &manifest)?;

    // Validate the completed staging tree through the same reader used by
    // restore before publishing it under the requested name.
    validate_backup_directory(staging)?;
    let resource_files = manifest
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind,
                BackupEntryKind::SystemResource | BackupEntryKind::DisplayResource
            )
        })
        .count();
    let total_bytes = manifest
        .entries
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size_bytes))
        .ok_or(BoardBackupError::SizeOverflow)?;
    let staged_path = temporary.keep();
    if let Err(source) = fs::rename(&staged_path, &destination) {
        let _ = fs::remove_dir_all(&staged_path);
        return Err(io_error("publish backup directory", &destination, source).into());
    }
    let mut completed = NewOperationalEvent::new(
        created_at,
        EventCategory::Backup,
        EventSeverity::Notice,
        "backup.completed",
        EventOutcome::Succeeded,
    );
    completed.correlation_id = Some(format!("backup-{}", started.id.get()));
    completed.idempotency_key = Some(format!("backup-completed-{}", started.id.get()));
    completed.attributes = EventAttributes::Backup {
        state: "completed".to_owned(),
        bytes: Some(total_bytes),
    };
    observation.mark_finished();
    if let Err(error) = database.record_operational_event(&completed) {
        tracing::warn!(error = %error, "backup completed but its operational event could not be recorded");
    }
    Ok(BackupReport {
        destination,
        board_name: manifest.board_name,
        schema_version: manifest.schema_version,
        resource_files,
        cataloged_files: catalog.len(),
        total_bytes,
    })
}

fn now_unix_seconds() -> Result<i64, BoardBackupError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| BoardBackupError::Clock)?
            .as_secs(),
    )
    .map_err(|_| BoardBackupError::Clock)
}

/// Restores a validated snapshot to a new directory, or atomically exchanges
/// an existing stopped board only when `replace` is explicit.
pub fn restore_board(
    backup_directory: &Path,
    target_root: &Path,
    replace: bool,
) -> Result<RestoreReport, ApplicationError> {
    let backup = validate_backup_directory(backup_directory)?;
    let target = new_path_with_existing_parent(target_root)?;
    if backup.root == target || backup.root.starts_with(&target) || target.starts_with(&backup.root)
    {
        return Err(BoardBackupError::BackupTargetOverlap.into());
    }
    let _operation_lock = BoardOperationLock::acquire(&target)?;

    match (target.exists(), replace) {
        (true, false) => return Err(BoardBackupError::RestoreTargetExists(target).into()),
        (false, true) => return Err(BoardBackupError::ReplaceTargetMissing(target).into()),
        _ => {}
    }
    if target.exists() {
        validate_existing_restore_target(&target, &backup.manifest)?;
    }

    let parent = target.parent().expect("validated restore target parent");
    let temporary = tempfile::Builder::new()
        .prefix(".spitfire-restore-")
        .tempdir_in(parent)
        .map_err(|source| io_error("create restore staging directory", parent, source))?;
    stage_restored_board(&backup, temporary.path())?;
    validate_staged_board(temporary.path(), &backup)?;

    let staged_path = temporary.keep();
    if !replace {
        if let Err(source) = fs::rename(&staged_path, &target) {
            let _ = fs::remove_dir_all(&staged_path);
            return Err(io_error("publish restored board", &target, source).into());
        }
    } else {
        let rollback = rollback_path(&target)?;
        if rollback.exists() {
            let _ = fs::remove_dir_all(&staged_path);
            return Err(BoardBackupError::RollbackExists(rollback).into());
        }
        fs::rename(&target, &rollback)
            .map_err(|source| io_error("prepare restore rollback", &target, source))?;
        if let Err(source) = fs::rename(&staged_path, &target) {
            let recovery = fs::rename(&rollback, &target);
            let _ = fs::remove_dir_all(&staged_path);
            if recovery.is_err() {
                return Err(BoardBackupError::RollbackFailed {
                    target,
                    rollback,
                    publish: source,
                }
                .into());
            }
            return Err(io_error("publish replacement board", &target, source).into());
        }
        fs::remove_dir_all(&rollback)
            .map_err(|source| io_error("remove completed restore rollback", &rollback, source))?;
    }

    Ok(RestoreReport {
        config_path: target.join(&backup.manifest.config_name),
        root: target,
        board_name: backup.manifest.board_name,
        schema_version: backup.manifest.schema_version,
        resource_files: backup.resource_files,
        cataloged_files: backup.cataloged_files,
        replaced_existing: replace,
    })
}

fn validate_backup_directory(path: &Path) -> Result<ValidatedBackup, BoardBackupError> {
    let root = canonical_real_directory(path)?;
    let manifest_path = root.join(BACKUP_MANIFEST_FILE);
    let manifest_input = fs::read_to_string(&manifest_path)
        .map_err(|source| io_error("read backup manifest", &manifest_path, source))?;
    let manifest: BackupManifest =
        toml::from_str(&manifest_input).map_err(BoardBackupError::ManifestParse)?;
    if manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(BoardBackupError::UnsupportedFormat {
            found: manifest.format_version,
            supported: BACKUP_FORMAT_VERSION,
        });
    }
    if !(10..=SCHEMA_VERSION).contains(&manifest.schema_version) {
        return Err(BoardBackupError::UnsupportedSchema {
            found: manifest.schema_version,
            minimum: 10,
            maximum: SCHEMA_VERSION,
        });
    }
    if manifest.created_at < 0 {
        return Err(BoardBackupError::InvalidManifest("negative creation time"));
    }
    validate_single_component(&manifest.config_name)?;
    ensure_entry_set_is_safe(&manifest.entries)?;

    let actual_files = collect_regular_files(&root)?
        .into_iter()
        .map(|(relative, _)| relative)
        .collect::<BTreeSet<_>>();
    let mut declared_files = manifest
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    declared_files.insert(BACKUP_MANIFEST_FILE.to_owned());
    if actual_files != declared_files {
        return Err(BoardBackupError::ManifestInventoryMismatch);
    }
    for entry in &manifest.entries {
        let (size_bytes, sha256) = hash_file(&root.join(&entry.path))?;
        if size_bytes != entry.size_bytes || sha256 != entry.sha256 {
            return Err(BoardBackupError::ChecksumMismatch(entry.path.clone()));
        }
    }

    let configuration_path = format!("configuration/{}", manifest.config_name);
    require_exact_entry(
        &manifest.entries,
        BackupEntryKind::Configuration,
        &configuration_path,
    )?;
    require_exact_entry(
        &manifest.entries,
        BackupEntryKind::Database,
        DATABASE_BACKUP_PATH,
    )?;
    let config = RuntimeConfig::load(&root.join(&configuration_path))?;
    let validated = config.validate()?;
    let synthetic_paths = LogicalPaths::resolve(&root, &validated)?;
    validate_native_layout(&validated, &synthetic_paths)?;

    let database = RuntimeDatabase::open_read_only(&root.join(DATABASE_BACKUP_PATH))?;
    let identity = database.validate_snapshot_at_version(manifest.schema_version)?;
    if identity.name() != manifest.board_name
        || identity.sysop_name() != manifest.sysop_name
        || identity != validated.identity
    {
        return Err(BoardBackupError::IdentityMismatch);
    }

    let mut expected_catalog = BTreeMap::new();
    for (area, file) in database.managed_cataloged_files()? {
        expected_catalog.insert(
            format!("files/{}/{}", area.storage_key, file.filename),
            (file.size_bytes, file.sha256),
        );
    }
    let declared_catalog = manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == BackupEntryKind::CatalogedFile)
        .map(|entry| (entry.path.clone(), (entry.size_bytes, entry.sha256.clone())))
        .collect::<BTreeMap<_, _>>();
    if expected_catalog != declared_catalog {
        return Err(BoardBackupError::CatalogInventoryMismatch);
    }

    for entry in &manifest.entries {
        validate_entry_location(entry, &manifest.config_name)?;
    }
    let resource_files = manifest
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind,
                BackupEntryKind::SystemResource | BackupEntryKind::DisplayResource
            )
        })
        .count();
    Ok(ValidatedBackup {
        root,
        config,
        resource_files,
        cataloged_files: expected_catalog.len(),
        manifest,
    })
}

fn stage_restored_board(
    backup: &ValidatedBackup,
    staging_root: &Path,
) -> Result<(), BoardBackupError> {
    let validated = backup.config.validate()?;
    let paths = LogicalPaths::resolve(staging_root, &validated)?;
    validate_native_layout(&validated, &paths)?;
    paths.create_directories()?;

    for entry in &backup.manifest.entries {
        let destination = match entry.kind {
            BackupEntryKind::Configuration => staging_root.join(&backup.manifest.config_name),
            BackupEntryKind::Database => paths.database().to_path_buf(),
            BackupEntryKind::SystemResource => paths
                .get(LogicalPath::System)
                .join(strip_prefix(&entry.path, "resources/system/")?),
            BackupEntryKind::DisplayResource => paths
                .get(LogicalPath::Display)
                .join(strip_prefix(&entry.path, "resources/display/")?),
            BackupEntryKind::CatalogedFile => paths.get(LogicalPath::External).join(&entry.path),
        };
        create_parent_directories(&destination)?;
        let copied = copy_entry_to_path(&backup.root.join(&entry.path), &destination)?;
        if copied.0 != entry.size_bytes || copied.1 != entry.sha256 {
            return Err(BoardBackupError::ChecksumMismatch(entry.path.clone()));
        }
    }
    let mut restored_database = RuntimeDatabase::open(paths.database())?;
    if restored_database.schema_version()? >= 16 {
        restored_database.normalize_external_storage_after_restore(
            i64::try_from(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| BoardBackupError::Clock)?
                    .as_secs(),
            )
            .map_err(|_| BoardBackupError::Clock)?,
        )?;
    }
    Ok(())
}

fn validate_staged_board(
    staging_root: &Path,
    backup: &ValidatedBackup,
) -> Result<(), BoardBackupError> {
    let config_path = staging_root.join(&backup.manifest.config_name);
    let config = RuntimeConfig::load(&config_path)?;
    if config != backup.config {
        return Err(BoardBackupError::RestoredConfigurationMismatch);
    }
    let validated = config.validate()?;
    let paths = LogicalPaths::resolve(staging_root, &validated)?;
    let database = RuntimeDatabase::open_read_only(paths.database())?;
    let identity = database.validate_snapshot_at_version(backup.manifest.schema_version)?;
    if identity != validated.identity {
        return Err(BoardBackupError::IdentityMismatch);
    }
    let storage = FileStorage::new(&paths)?;
    for (area, file) in database.managed_cataloged_files()? {
        storage.open_download(&area, &file)?;
    }
    let presentation = crate::PresentationResolver::load(&paths, &validated.presentation);
    load_stock_resources(&paths, &TerminalInfo::in_memory(), &presentation)
        .map_err(|error| BoardBackupError::ResourceValidation(error.to_string()))?;
    Ok(())
}

fn validate_existing_restore_target(
    target: &Path,
    manifest: &BackupManifest,
) -> Result<(), BoardBackupError> {
    let metadata = fs::symlink_metadata(target)
        .map_err(|source| io_error("inspect restore target", target, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BoardBackupError::UnsafeObject(target.to_path_buf()));
    }
    let config_path = target.join(&manifest.config_name);
    let config = RuntimeConfig::load(&config_path)?;
    let validated = config.validate()?;
    let paths = LogicalPaths::resolve(target, &validated)?;
    validate_native_layout(&validated, &paths)?;
    if validated.identity.name() != manifest.board_name
        || validated.identity.sysop_name() != manifest.sysop_name
    {
        return Err(BoardBackupError::RestoreIdentityMismatch {
            expected: format!("{} / {}", manifest.board_name, manifest.sysop_name),
            found: format!(
                "{} / {}",
                validated.identity.name(),
                validated.identity.sysop_name()
            ),
        });
    }
    Ok(())
}

fn validate_native_layout(
    config: &ValidatedConfig,
    paths: &LogicalPaths,
) -> Result<(), BoardBackupError> {
    let configured = [
        (LogicalPath::System, &config.paths.system),
        (LogicalPath::Work, &config.paths.work),
        (LogicalPath::Display, &config.paths.display),
        (LogicalPath::Message, &config.paths.message),
        (LogicalPath::External, &config.paths.external),
    ];
    if configured.iter().any(|(_, path)| path.is_absolute()) {
        return Err(BoardBackupError::NonPortableLayout);
    }
    for (index, (left_kind, _)) in configured.iter().enumerate() {
        let left = paths.get(*left_kind);
        if left == paths.root() || !left.starts_with(paths.root()) {
            return Err(BoardBackupError::NonPortableLayout);
        }
        for (right_kind, _) in configured.iter().skip(index + 1) {
            let right = paths.get(*right_kind);
            if left == right || left.starts_with(right) || right.starts_with(left) {
                return Err(BoardBackupError::OverlappingLogicalPaths {
                    left: left_kind.historical_name(),
                    right: right_kind.historical_name(),
                });
            }
        }
    }
    Ok(())
}

fn validate_real_logical_directories(paths: &LogicalPaths) -> Result<(), BoardBackupError> {
    for logical in LogicalPath::ALL {
        let path = paths.get(logical);
        let relative = path
            .strip_prefix(paths.root())
            .map_err(|_| BoardBackupError::NonPortableLayout)?;
        let mut current = paths.root().to_path_buf();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(BoardBackupError::NonPortableLayout);
            };
            current.push(component);
            let metadata = fs::symlink_metadata(&current)
                .map_err(|source| io_error("inspect logical directory", &current, source))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(BoardBackupError::UnsafeObject(current));
            }
        }
    }
    Ok(())
}

fn copy_resource_tree(
    source_root: &Path,
    staging: &Path,
    prefix: &str,
    kind: BackupEntryKind,
    entries: &mut Vec<BackupEntry>,
) -> Result<(), BoardBackupError> {
    for (relative, source) in collect_regular_files(source_root)? {
        let path = format!("{prefix}/{relative}");
        entries.push(copy_entry(&source, staging, &path, kind)?);
    }
    Ok(())
}

fn collect_regular_files(root: &Path) -> Result<Vec<(String, PathBuf)>, BoardBackupError> {
    let metadata =
        fs::symlink_metadata(root).map_err(|source| io_error("inspect directory", root, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BoardBackupError::UnsafeObject(root.to_path_buf()));
    }
    let mut pending = vec![(PathBuf::new(), root.to_path_buf())];
    let mut files = Vec::new();
    while let Some((relative_directory, directory)) = pending.pop() {
        let mut children = fs::read_dir(&directory)
            .map_err(|source| io_error("read directory", &directory, source))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| io_error("read directory entry", &directory, source))?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children.into_iter().rev() {
            let path = child.path();
            let relative = relative_directory.join(child.file_name());
            let metadata = fs::symlink_metadata(&path)
                .map_err(|source| io_error("inspect backup source", &path, source))?;
            if metadata.file_type().is_symlink() {
                return Err(BoardBackupError::UnsafeObject(path));
            }
            if metadata.is_dir() {
                pending.push((relative, path));
            } else if metadata.is_file() {
                let relative = portable_relative_path(&relative)?;
                files.push((relative, path));
                if files.len() > MAX_BACKUP_ENTRIES {
                    return Err(BoardBackupError::TooManyEntries(MAX_BACKUP_ENTRIES));
                }
            } else {
                return Err(BoardBackupError::UnsafeObject(path));
            }
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn copy_entry(
    source: &Path,
    staging: &Path,
    relative: &str,
    kind: BackupEntryKind,
) -> Result<BackupEntry, BoardBackupError> {
    validate_relative_path(relative)?;
    let destination = staging.join(relative);
    create_parent_directories(&destination)?;
    let (size_bytes, sha256) = copy_entry_to_path(source, &destination)?;
    Ok(BackupEntry {
        kind,
        path: relative.to_owned(),
        size_bytes,
        sha256,
    })
}

fn copy_entry_to_path(
    source: &Path,
    destination: &Path,
) -> Result<(u64, String), BoardBackupError> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|source_error| io_error("inspect source file", source, source_error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BoardBackupError::UnsafeObject(source.to_path_buf()));
    }
    let mut input = File::open(source)
        .map_err(|source_error| io_error("open source file", source, source_error))?;
    copy_reader(&mut input, destination)
}

fn copy_reader(input: &mut File, destination: &Path) -> Result<(u64, String), BoardBackupError> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source| io_error("create snapshot file", destination, source))?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|source| io_error("read snapshot source", destination, source))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|source| io_error("write snapshot file", destination, source))?;
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(BoardBackupError::SizeOverflow)?;
    }
    output
        .sync_all()
        .map_err(|source| io_error("synchronize snapshot file", destination, source))?;
    Ok((size_bytes, format!("{:x}", hasher.finalize())))
}

fn entry_for_file(
    path: &Path,
    relative: &str,
    kind: BackupEntryKind,
) -> Result<BackupEntry, BoardBackupError> {
    let (size_bytes, sha256) = hash_file(path)?;
    Ok(BackupEntry {
        kind,
        path: relative.to_owned(),
        size_bytes,
        sha256,
    })
}

fn hash_file(path: &Path) -> Result<(u64, String), BoardBackupError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect snapshot file", path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BoardBackupError::UnsafeObject(path.to_path_buf()));
    }
    let mut input =
        File::open(path).map_err(|source| io_error("open snapshot file", path, source))?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|source| io_error("read snapshot file", path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(BoardBackupError::SizeOverflow)?;
    }
    Ok((size_bytes, format!("{:x}", hasher.finalize())))
}

fn write_manifest(root: &Path, manifest: &BackupManifest) -> Result<(), BoardBackupError> {
    let path = root.join(BACKUP_MANIFEST_FILE);
    let encoded = toml::to_string_pretty(manifest).map_err(BoardBackupError::ManifestSerialize)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| io_error("create backup manifest", &path, source))?;
    output
        .write_all(encoded.as_bytes())
        .and_then(|()| output.sync_all())
        .map_err(|source| io_error("write backup manifest", &path, source))
}

fn ensure_entry_set_is_safe(entries: &[BackupEntry]) -> Result<(), BoardBackupError> {
    if entries.len() > MAX_BACKUP_ENTRIES {
        return Err(BoardBackupError::TooManyEntries(MAX_BACKUP_ENTRIES));
    }
    let mut exact = BTreeSet::new();
    let mut case_folded = BTreeSet::new();
    for entry in entries {
        validate_relative_path(&entry.path)?;
        validate_sha256(&entry.sha256)?;
        if !exact.insert(entry.path.clone()) || !case_folded.insert(entry.path.to_ascii_lowercase())
        {
            return Err(BoardBackupError::DuplicateEntry(entry.path.clone()));
        }
    }
    Ok(())
}

fn validate_entry_location(entry: &BackupEntry, config_name: &str) -> Result<(), BoardBackupError> {
    let expected = match entry.kind {
        BackupEntryKind::Configuration => entry.path == format!("configuration/{config_name}"),
        BackupEntryKind::Database => entry.path == DATABASE_BACKUP_PATH,
        BackupEntryKind::SystemResource => entry.path.starts_with("resources/system/"),
        BackupEntryKind::DisplayResource => entry.path.starts_with("resources/display/"),
        BackupEntryKind::CatalogedFile => {
            let components = entry.path.split('/').collect::<Vec<_>>();
            components.len() == 3 && components.first() == Some(&"files")
        }
    };
    if expected {
        Ok(())
    } else {
        Err(BoardBackupError::InvalidEntryLocation(entry.path.clone()))
    }
}

fn require_exact_entry(
    entries: &[BackupEntry],
    kind: BackupEntryKind,
    path: &str,
) -> Result<(), BoardBackupError> {
    if entries
        .iter()
        .filter(|entry| entry.kind == kind && entry.path == path)
        .count()
        == 1
    {
        Ok(())
    } else {
        Err(BoardBackupError::MissingRequiredEntry(path.to_owned()))
    }
}

fn validate_relative_path(value: &str) -> Result<(), BoardBackupError> {
    if value.is_empty() || value.contains('\\') {
        return Err(BoardBackupError::UnsafeManifestPath(value.to_owned()));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_)) || component.as_os_str().to_str().is_none()
        })
    {
        return Err(BoardBackupError::UnsafeManifestPath(value.to_owned()));
    }
    Ok(())
}

fn validate_single_component(value: &str) -> Result<(), BoardBackupError> {
    validate_relative_path(value)?;
    if Path::new(value).components().count() != 1 {
        return Err(BoardBackupError::UnsafeManifestPath(value.to_owned()));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), BoardBackupError> {
    if value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Ok(())
    } else {
        Err(BoardBackupError::InvalidChecksum(value.to_owned()))
    }
}

fn strip_prefix<'a>(value: &'a str, prefix: &str) -> Result<&'a str, BoardBackupError> {
    value
        .strip_prefix(prefix)
        .filter(|suffix| !suffix.is_empty())
        .ok_or_else(|| BoardBackupError::InvalidEntryLocation(value.to_owned()))
}

fn portable_relative_path(path: &Path) -> Result<String, BoardBackupError> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(BoardBackupError::UnsafeObject(path.to_path_buf()));
        };
        components.push(
            value
                .to_str()
                .ok_or_else(|| BoardBackupError::NonUtf8Path(path.to_path_buf()))?,
        );
    }
    Ok(components.join("/"))
}

fn canonical_regular_file(path: &Path) -> Result<PathBuf, ApplicationError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        ApplicationError::Backup(BoardBackupError::Io {
            operation: "inspect configuration",
            path: path.to_path_buf(),
            source,
        })
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BoardBackupError::UnsafeObject(path.to_path_buf()).into());
    }
    path.canonicalize()
        .map_err(|source| ApplicationError::ResolveConfiguration {
            path: path.to_path_buf(),
            source,
        })
}

fn canonical_real_directory(path: &Path) -> Result<PathBuf, BoardBackupError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect backup directory", path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BoardBackupError::UnsafeObject(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| io_error("resolve backup directory", path, source))
}

fn new_path_with_existing_parent(path: &Path) -> Result<PathBuf, BoardBackupError> {
    let name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| BoardBackupError::InvalidDestination(path.to_path_buf()))?;
    let parent = match path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => parent.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|source| io_error("resolve current directory", Path::new("."), source))?,
    };
    let parent = parent
        .canonicalize()
        .map_err(|source| io_error("resolve destination parent", &parent, source))?;
    Ok(parent.join(name))
}

fn one_component_name(path: &Path) -> Result<String, BoardBackupError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BoardBackupError::NonUtf8Path(path.to_path_buf()))?;
    validate_single_component(name)?;
    Ok(name.to_owned())
}

fn reject_destination_inside_board(
    destination: &Path,
    root: &Path,
    paths: &LogicalPaths,
) -> Result<(), BoardBackupError> {
    if destination.starts_with(root)
        || LogicalPath::ALL
            .iter()
            .any(|logical| destination.starts_with(paths.get(*logical)))
    {
        Err(BoardBackupError::DestinationInsideBoard(
            destination.to_path_buf(),
        ))
    } else {
        Ok(())
    }
}

fn create_parent_directories(path: &Path) -> Result<(), BoardBackupError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| BoardBackupError::InvalidDestination(path.to_path_buf()))?;
    fs::create_dir_all(parent).map_err(|source| io_error("create directory", parent, source))
}

fn rollback_path(target: &Path) -> Result<PathBuf, BoardBackupError> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| BoardBackupError::InvalidDestination(target.to_path_buf()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BoardBackupError::NonUtf8Path(target.to_path_buf()))?;
    Ok(parent.join(format!(".{name}.spitfire-restore-rollback")))
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> BoardBackupError {
    BoardBackupError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug, Error)]
pub enum BoardBackupError {
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("backup destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("cold backup requires all file-maintenance operations to be committed or rolled back")]
    UnnormalizedFileOperations,
    #[error("cold backup requires active transfers and quota reservations to be drained")]
    UnnormalizedTransferOperations,
    #[error("restore target already exists; pass --replace only after verifying the target: {0}")]
    RestoreTargetExists(PathBuf),
    #[error("--replace requires an existing board target: {0}")]
    ReplaceTargetMissing(PathBuf),
    #[error("invalid backup/restore destination: {0}")]
    InvalidDestination(PathBuf),
    #[error("backup destination must be outside the board's managed paths: {0}")]
    DestinationInsideBoard(PathBuf),
    #[error("backup directory and restore target must not contain one another")]
    BackupTargetOverlap,
    #[error("native backup/restore requires relative board logical paths")]
    NonPortableLayout,
    #[error("logical {left} and {right} paths overlap; cold restore cannot exchange them safely")]
    OverlappingLogicalPaths {
        left: &'static str,
        right: &'static str,
    },
    #[error("unsafe symlink or non-file/non-directory object: {0}")]
    UnsafeObject(PathBuf),
    #[error("backup path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("backup contains more than the supported {0} files")]
    TooManyEntries(usize),
    #[error("backup byte count exceeds the supported range")]
    SizeOverflow,
    #[error("system clock is before the Unix epoch or exceeds the supported range")]
    Clock,
    #[error("could not serialize backup manifest: {0}")]
    ManifestSerialize(#[source] toml::ser::Error),
    #[error("backup manifest is malformed: {0}")]
    ManifestParse(#[source] toml::de::Error),
    #[error("backup format {found} is unsupported; this build supports {supported}")]
    UnsupportedFormat { found: u32, supported: u32 },
    #[error(
        "backup schema {found} is unsupported; this build restores schemas {minimum} through {maximum}"
    )]
    UnsupportedSchema {
        found: u32,
        minimum: u32,
        maximum: u32,
    },
    #[error("backup manifest is invalid: {0}")]
    InvalidManifest(&'static str),
    #[error("unsafe path in backup manifest: {0:?}")]
    UnsafeManifestPath(String),
    #[error("invalid SHA-256 value in backup manifest: {0:?}")]
    InvalidChecksum(String),
    #[error("duplicate or case-conflicting backup entry: {0}")]
    DuplicateEntry(String),
    #[error("backup file inventory does not exactly match its manifest")]
    ManifestInventoryMismatch,
    #[error("backup checksum or size does not match for {0}")]
    ChecksumMismatch(String),
    #[error("required backup entry is missing: {0}")]
    MissingRequiredEntry(String),
    #[error("backup entry is stored under the wrong content boundary: {0}")]
    InvalidEntryLocation(String),
    #[error("backup database/configuration/manifest board identities do not match")]
    IdentityMismatch,
    #[error("backup catalog rows and cataloged byte entries do not match")]
    CatalogInventoryMismatch,
    #[error("cataloged file bytes do not match metadata: {0}")]
    CatalogMismatch(String),
    #[error("restored configuration differs from the validated snapshot")]
    RestoredConfigurationMismatch,
    #[error("restored resources failed validation: {0}")]
    ResourceValidation(String),
    #[error("replacement target identifies {found}, but the backup identifies {expected}")]
    RestoreIdentityMismatch { expected: String, found: String },
    #[error("an earlier restore rollback still exists and must be inspected: {0}")]
    RollbackExists(PathBuf),
    #[error(
        "replacement publish failed ({publish}) and rollback recovery also failed; inspect target {target} and rollback {rollback}"
    )]
    RollbackFailed {
        target: PathBuf,
        rollback: PathBuf,
        publish: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] sf_core::ConfigError),
    #[error(transparent)]
    Paths(#[from] sf_core::PathError),
    #[error(transparent)]
    Database(#[from] sf_core::DatabaseError),
    #[error(transparent)]
    Transfer(#[from] sf_core::TransferRuntimeError),
    #[error(transparent)]
    File(#[from] sf_core::FileError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{
        CallerState, CopyRecipient, CredentialHasher, EventQuery, FileActor, InMemoryTerminal,
        MessageActor, MessageBackend, MessageKind, MessageRecipient, MessageVisibility, NewMessage,
        NewOperatorCommandReceipt, NewOperatorControlAudit, NodeId, ObservabilityService,
        OperatorPrincipal, OperatorPrincipalKind, PasswordHashConfig, SecurityLevel,
        TransferMethod, TransferProtocol, TransferQueue, TransferRuntimeState,
    };

    use crate::{setup_board, BoardRuntime, ConnectionReport, SetupPlan, BOARD_CONFIG_FILE};

    fn installed_board(parent: &Path, directory: &str) -> PathBuf {
        let root = parent.join(directory);
        let mut plan = SetupPlan::stock_defaults("Backup Board", "Backup Sysop", "Sysop", 2);
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        setup_board(&root, &plan, b"test-only backup sysop password").unwrap();
        let config_path = root.join(BOARD_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        fs::write(
            paths.get(LogicalPath::Display).join("PRESERVE.BBS"),
            b"Preserved resource\r\n",
        )
        .unwrap();
        fs::create_dir(paths.get(LogicalPath::Display).join("nested")).unwrap();
        fs::write(
            paths.get(LogicalPath::Display).join("nested/EXTRA.BBS"),
            b"Preserved nested resource\r\n",
        )
        .unwrap();

        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let hasher = CredentialHasher::new(&validated.caller.password).unwrap();
        let caller = database
            .create_caller(
                b"Backup Caller",
                &hasher.hash(b"test-only caller password").unwrap(),
                SecurityLevel::new(20).unwrap(),
                CallerState::Active,
                false,
                1_777_000_000,
            )
            .unwrap();
        let recipient = database
            .create_caller(
                b"Backup Recipient",
                &hasher.hash(b"test-only recipient password").unwrap(),
                SecurityLevel::new(20).unwrap(),
                CallerState::Active,
                false,
                1_777_000_000,
            )
            .unwrap();
        let sysop = database.caller_by_name(b"Sysop").unwrap().unwrap();
        let actor = MessageActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let conference = database.conference(actor, 1).unwrap();
        let sysop_actor = MessageActor::new(
            sysop.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let mut mutation_message = NewMessage {
            conference_id: conference.id,
            recipient_caller_id: Some(recipient.id),
            recipient_name: recipient.display_name.clone(),
            subject: b"Backup mutation preservation".to_vec(),
            body: b"CC, receipt, tombstone, lineage, and audit remain in SQLite.\r\n".to_vec(),
            created_at: 1_777_000_002,
            parent_message_id: None,
            visibility: MessageVisibility::Private,
            kind: MessageKind::Standard,
        };
        let deliveries = database
            .post_with_cc(
                actor,
                mutation_message.clone(),
                &[MessageRecipient {
                    caller_id: sysop.id,
                    display_name: sysop.display_name,
                }],
            )
            .unwrap();
        let recipient_actor = MessageActor::new(
            recipient.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        database
            .mark_read(recipient_actor, conference.id, deliveries[0].number)
            .unwrap();
        database
            .delete_message(
                sysop_actor,
                conference.id,
                deliveries[1].number,
                deliveries[1].state_version,
            )
            .unwrap();
        database
            .copy_message(
                sysop_actor,
                conference.id,
                deliveries[0].number,
                deliveries[0].state_version,
                conference.number,
                CopyRecipient::Preserve,
                1_777_000_003,
            )
            .unwrap();
        mutation_message.recipient_caller_id = None;
        mutation_message.recipient_name = "All Callers".to_owned();
        mutation_message.visibility = MessageVisibility::Public;
        mutation_message.subject = b"Backup preservation".to_vec();
        mutation_message.body = b"Callers, messages, and receipts remain in SQLite.\r\n".to_vec();
        mutation_message.created_at = 1_777_000_001;
        database.post(actor, mutation_message).unwrap();
        root
    }

    fn restored_database(root: &Path) -> RuntimeDatabase {
        let config = RuntimeConfig::load(&root.join(BOARD_CONFIG_FILE)).unwrap();
        let paths = LogicalPaths::resolve(root, &config.validate().unwrap()).unwrap();
        RuntimeDatabase::open_read_only(paths.database()).unwrap()
    }

    fn downgrade_schema_19_to_18(connection: &rusqlite::Connection) {
        let has_schema_19: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=19)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if !has_schema_19 {
            return;
        }
        connection.execute_batch("DROP TRIGGER operator_control_audit_no_delete; DROP TRIGGER operator_control_audit_no_update; DROP TABLE operator_control_audit; DROP TABLE operator_command_journal; DELETE FROM schema_migrations WHERE version=19;").unwrap();
    }

    fn downgrade_schema_18_to_17(connection: &rusqlite::Connection) {
        downgrade_schema_19_to_18(connection);
        let has_schema_18: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=18)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if !has_schema_18 {
            return;
        }
        connection
            .execute_batch(
                r#"
                DROP TRIGGER operator_observability_audit_no_delete;
                DROP TRIGGER operator_observability_audit_no_update;
                DROP TABLE operator_observability_audit;
                DROP TABLE operator_notifications;
                DROP TABLE operational_daily_summaries;
                DROP TABLE operational_retention_policy;
                DROP TRIGGER operational_events_no_update;
                DROP TABLE operational_events;
                DELETE FROM schema_migrations WHERE version=18;
                "#,
            )
            .unwrap();
    }

    fn rewrite_backup_database_as_schema_17(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_18_to_17(&connection);
        drop(connection);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 17;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn downgrade_schema_17_to_16(connection: &rusqlite::Connection) {
        downgrade_schema_18_to_17(connection);
        let has_schema_17: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=17)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if !has_schema_17 {
            return;
        }
        connection.execute_batch(r#"
            PRAGMA foreign_keys=OFF;
            PRAGMA legacy_alter_table=ON;
            DROP INDEX files_area_filename_live;
            DROP INDEX files_area_listing;
            DROP INDEX files_upload_time;
            ALTER TABLE files RENAME TO files_schema_17;
            CREATE TABLE files (
                file_id INTEGER PRIMARY KEY,
                area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
                filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 64),
                normalized_filename TEXT NOT NULL CHECK (length(normalized_filename) BETWEEN 1 AND 64),
                description TEXT NOT NULL CHECK (length(CAST(description AS BLOB)) BETWEEN 1 AND 4096),
                size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
                sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
                uploaded_at INTEGER NOT NULL,
                uploader_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                uploader_name TEXT NOT NULL CHECK (length(uploader_name) BETWEEN 1 AND 60),
                download_count INTEGER NOT NULL DEFAULT 0 CHECK (download_count >= 0),
                lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','offline','pending-review','disabled','tombstoned')),
                integrity_state TEXT NOT NULL DEFAULT 'unknown' CHECK (integrity_state IN ('unknown','present','missing','digest-mismatch')),
                state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
                description_source TEXT NOT NULL DEFAULT 'legacy-import' CHECK (description_source IN ('caller','operator','file-id-diz','legacy-import','system')),
                description_source_digest TEXT CHECK (description_source_digest IS NULL OR (length(description_source_digest)=64 AND description_source_digest NOT GLOB '*[^0-9a-f]*')),
                review_submitted_at INTEGER CHECK (review_submitted_at IS NULL OR review_submitted_at >= 0),
                reviewed_by_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                reviewed_at INTEGER CHECK (reviewed_at IS NULL OR reviewed_at >= 0),
                tombstoned_at INTEGER CHECK (tombstoned_at IS NULL OR tombstoned_at >= 0),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO files SELECT * FROM files_schema_17;
            DROP TABLE files_schema_17;
            CREATE UNIQUE INDEX files_area_filename_live ON files(area_id,normalized_filename) WHERE lifecycle<>'tombstoned';
            CREATE INDEX files_area_listing ON files(area_id,normalized_filename,lifecycle,integrity_state);
            CREATE INDEX files_upload_time ON files(uploaded_at,area_id,lifecycle);
            DELETE FROM schema_migrations WHERE version=17;
            PRAGMA legacy_alter_table=OFF;
            PRAGMA foreign_keys=ON;
        "#).unwrap();
    }

    fn downgrade_schema_16_to_15(connection: &rusqlite::Connection) {
        downgrade_schema_17_to_16(connection);
        connection
            .execute_batch(
                r#"
            PRAGMA foreign_keys = OFF;
            DROP TRIGGER transfer_events_no_delete;
            DROP TRIGGER transfer_events_no_update;
            DROP TABLE transfer_events;
            DROP TABLE file_storage_locators;
            DROP TABLE file_storage_roots;
            DROP TABLE transfer_settlements;
            DROP TABLE transfer_quota_reservation_items;
            DROP TABLE transfer_quota_reservations;
            DROP TABLE transfer_records;
            DROP TABLE transfer_daily_usage;
            DROP TABLE transfer_policies;
            DROP TABLE transfer_timezone_policy;

            CREATE TEMP TABLE schema_15_caller_access_events AS
            SELECT * FROM caller_access_events;
            CREATE TEMP TABLE schema_15_caller_security_adjustments AS
            SELECT * FROM caller_security_adjustments;
            DROP TRIGGER caller_access_events_no_update;
            DROP TRIGGER caller_access_events_no_delete;
            DROP INDEX caller_security_adjustments_one_active_kind;
            DROP TABLE caller_security_adjustments;
            DROP TABLE caller_access_events;

            CREATE TABLE caller_access_events (
                event_id INTEGER PRIMARY KEY,
                occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
                operation TEXT NOT NULL CHECK (operation IN (
                    'caller-created', 'security-changed', 'disabled', 'enabled',
                    'tombstoned', 'restored', 'purge-protection-changed',
                    'subscription-updated', 'subscription-expired',
                    'subscription-adjustment-resolved', 'subscription-warning',
                    'joker-denied'
                )),
                outcome TEXT NOT NULL DEFAULT 'committed' CHECK (outcome IN ('committed', 'denied')),
                subject_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                actor_kind TEXT NOT NULL CHECK (actor_kind IN ('caller', 'threshold-sysop', 'local-operator', 'system-policy')),
                actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                prior_lifecycle TEXT CHECK (prior_lifecycle IS NULL OR prior_lifecycle IN ('active', 'disabled', 'deleted')),
                new_lifecycle TEXT CHECK (new_lifecycle IS NULL OR new_lifecycle IN ('active', 'disabled', 'deleted')),
                prior_state_version INTEGER CHECK (prior_state_version IS NULL OR prior_state_version >= 0),
                new_state_version INTEGER CHECK (new_state_version IS NULL OR new_state_version >= 0),
                prior_base_security INTEGER CHECK (prior_base_security IS NULL OR prior_base_security BETWEEN 0 AND 9999),
                new_base_security INTEGER CHECK (new_base_security IS NULL OR new_base_security BETWEEN 0 AND 9999),
                adjustment_kind TEXT CHECK (adjustment_kind IS NULL OR adjustment_kind = 'subscription-expired'),
                policy_generation INTEGER CHECK (policy_generation IS NULL OR policy_generation > 0)
            );
            INSERT INTO caller_access_events SELECT * FROM schema_15_caller_access_events;

            CREATE TABLE caller_security_adjustments (
                adjustment_id INTEGER PRIMARY KEY,
                caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
                kind TEXT NOT NULL CHECK (kind = 'subscription-expired'),
                target_security_level INTEGER NOT NULL CHECK (target_security_level BETWEEN 0 AND 9999),
                status TEXT NOT NULL CHECK (status IN ('active', 'resolved')),
                applied_at INTEGER NOT NULL CHECK (applied_at >= 0),
                resolved_at INTEGER CHECK (
                    (status = 'active' AND resolved_at IS NULL)
                    OR (status = 'resolved' AND resolved_at IS NOT NULL AND resolved_at >= applied_at)
                ),
                state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
                applied_event_id INTEGER NOT NULL REFERENCES caller_access_events(event_id) ON DELETE RESTRICT,
                resolved_event_id INTEGER REFERENCES caller_access_events(event_id) ON DELETE RESTRICT
            );
            INSERT INTO caller_security_adjustments SELECT * FROM schema_15_caller_security_adjustments;
            CREATE UNIQUE INDEX caller_security_adjustments_one_active_kind
                ON caller_security_adjustments (caller_id, kind) WHERE status = 'active';
            CREATE INDEX caller_access_events_subject
                ON caller_access_events (subject_caller_id, event_id);
            CREATE TRIGGER caller_access_events_no_update
            BEFORE UPDATE ON caller_access_events BEGIN
                SELECT RAISE(ABORT, 'caller access events are append-only');
            END;
            CREATE TRIGGER caller_access_events_no_delete
            BEFORE DELETE ON caller_access_events BEGIN
                SELECT RAISE(ABORT, 'caller access events are append-only');
            END;
            DROP TABLE schema_15_caller_security_adjustments;
            DROP TABLE schema_15_caller_access_events;
            DELETE FROM schema_migrations WHERE version = 16;
            PRAGMA foreign_keys = ON;
            "#,
            )
            .unwrap();
    }

    fn rewrite_backup_database_as_schema_15(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_16_to_15(&connection);
        drop(connection);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 15;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn downgrade_schema_15_to_14(connection: &rusqlite::Connection) {
        connection.execute_batch(r#"
            PRAGMA foreign_keys = OFF;
            DROP TRIGGER file_events_no_delete;
            DROP TRIGGER file_events_no_update;
            DROP TABLE file_active_uses;
            DROP TABLE file_operation_leases;
            DROP TABLE file_events;
            DROP TABLE file_legacy_publications;
            DROP TABLE file_operations;
            DROP TABLE file_requests;
            DROP TABLE file_uppercase_terms;
            DROP TABLE file_upload_denials;
            DROP TABLE file_policy;
            DROP INDEX files_area_filename_live;
            DROP INDEX files_area_listing;
            DROP INDEX files_upload_time;
            ALTER TABLE files RENAME TO files_schema_15;
            CREATE TABLE files (
                file_id INTEGER PRIMARY KEY,
                area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
                filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 64),
                normalized_filename TEXT NOT NULL CHECK (length(normalized_filename) BETWEEN 1 AND 64),
                description TEXT NOT NULL CHECK (length(description) BETWEEN 1 AND 4096),
                size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
                sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
                uploaded_at INTEGER NOT NULL,
                uploader_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                uploader_name TEXT NOT NULL CHECK (length(uploader_name) BETWEEN 1 AND 60),
                download_count INTEGER NOT NULL DEFAULT 0 CHECK (download_count >= 0),
                state TEXT NOT NULL DEFAULT 'available' CHECK (state IN ('available', 'disabled')),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE (area_id, normalized_filename)
            );
            INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,state,created_at,updated_at)
            SELECT file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,
                   CASE lifecycle WHEN 'active' THEN 'available' ELSE 'disabled' END,
                   created_at,updated_at FROM files_schema_15;
            DROP TABLE files_schema_15;
            CREATE INDEX files_area_listing ON files(area_id, normalized_filename, state);
            CREATE INDEX files_upload_time ON files(uploaded_at, area_id, state);
            ALTER TABLE file_areas DROP COLUMN state_version;
            DELETE FROM schema_migrations WHERE version=15;
            PRAGMA foreign_keys = ON;
        "#).unwrap();
    }

    fn rewrite_backup_database_as_schema_10(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_16_to_15(&connection);
        downgrade_schema_15_to_14(&connection);
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = OFF;
                DROP TRIGGER public_information_events_no_delete;
                DROP TRIGGER public_information_events_no_update;
                DROP TABLE public_information_events;
                DROP TABLE public_information_resource_state;
                DROP TABLE other_bbs_entries;
                DROP TABLE public_information_policy;
                ALTER TABLE callers DROP COLUMN publicity_state_version;
                ALTER TABLE callers DROP COLUMN public_directory_listed;
                DELETE FROM schema_migrations WHERE version = 14;

                DROP TRIGGER callers_login_identifier_update;
                DROP TRIGGER callers_login_identifier_insert;
                DROP TRIGGER caller_identity_events_no_delete;
                DROP TRIGGER caller_identity_events_no_update;
                DROP TABLE caller_identity_events;
                DROP INDEX callers_login_identifier_unique;
                ALTER TABLE callers DROP COLUMN real_name;
                ALTER TABLE callers DROP COLUMN login_identifier;
                DELETE FROM schema_migrations WHERE version = 13;

                DROP TRIGGER caller_access_events_no_delete;
                DROP TRIGGER caller_access_events_no_update;
                DROP TABLE caller_security_adjustments;
                DROP TABLE caller_access_events;
                ALTER TABLE callers DROP COLUMN lifecycle_prior_state;
                ALTER TABLE callers DROP COLUMN purge_protected;
                ALTER TABLE callers DROP COLUMN subscription_expires_on;
                ALTER TABLE callers DROP COLUMN state_version;
                DELETE FROM schema_migrations WHERE version = 12;

                CREATE TEMP TABLE message_export AS
                SELECT
                    m.message_id, m.conference_id, m.message_number,
                    m.author_caller_id, m.author_name,
                    r.caller_id AS recipient_caller_id,
                    COALESCE(r.display_name_snapshot, 'All Callers') AS recipient_name,
                    p.subject, p.body, m.created_at, m.parent_message_id,
                    m.visibility, p.content_kind AS kind,
                    CASE WHEN m.lifecycle_state = 'deleted' THEN 1 ELSE 0 END AS deleted
                  FROM messages m
                  JOIN message_fanouts f ON f.fanout_id = m.fanout_id
                  JOIN message_payloads p ON p.payload_id = f.payload_id
                  LEFT JOIN message_delivery_recipients r ON r.message_id = m.message_id;

                CREATE TEMP TABLE receipt_export AS
                SELECT caller_id, message_id, received_at
                  FROM caller_message_receipts;

                DROP TRIGGER message_mutation_events_append_only_delete;
                DROP TRIGGER message_mutation_events_append_only_update;
                DROP TRIGGER message_payloads_immutable_delete;
                DROP TRIGGER message_payloads_immutable_update;
                DROP TABLE message_lineage;
                DROP TABLE message_mutation_events;
                DROP TABLE caller_message_receipts;
                DROP TABLE message_delivery_recipients;
                DROP TABLE messages;
                DROP TABLE message_fanouts;
                DROP TABLE message_payloads;
                ALTER TABLE message_conferences DROP COLUMN caller_deletion_enabled;

                CREATE TABLE messages (
                    message_id INTEGER PRIMARY KEY,
                    conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
                    message_number INTEGER NOT NULL CHECK (message_number > 0),
                    author_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                    author_name TEXT NOT NULL CHECK (length(author_name) BETWEEN 1 AND 60),
                    recipient_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
                    recipient_name TEXT NOT NULL CHECK (length(recipient_name) BETWEEN 1 AND 60),
                    subject BLOB NOT NULL CHECK (length(subject) BETWEEN 1 AND 72),
                    body BLOB NOT NULL CHECK (length(body) BETWEEN 1 AND 65536),
                    created_at INTEGER NOT NULL,
                    parent_message_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
                    visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
                    kind TEXT NOT NULL CHECK (kind IN ('standard', 'sysop-comment')),
                    deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
                    UNIQUE (conference_id, message_number)
                );
                INSERT INTO messages SELECT * FROM message_export ORDER BY message_id;
                CREATE INDEX messages_conference_scan
                    ON messages (conference_id, message_number, deleted);
                CREATE INDEX messages_recipient_scan
                    ON messages (recipient_caller_id, visibility, deleted);
                CREATE INDEX messages_author_scan
                    ON messages (author_caller_id, visibility, deleted);

                CREATE TABLE caller_message_receipts (
                    caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE CASCADE,
                    message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
                    received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (caller_id, message_id)
                );
                INSERT INTO caller_message_receipts
                SELECT * FROM receipt_export ORDER BY caller_id, message_id;
                CREATE INDEX caller_message_receipts_message
                    ON caller_message_receipts (message_id, caller_id);

                DELETE FROM schema_migrations WHERE version = 11;
                PRAGMA foreign_keys = ON;
                "#,
            )
            .unwrap();
        drop(connection);

        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 10;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn rewrite_backup_database_as_schema_11(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_16_to_15(&connection);
        downgrade_schema_15_to_14(&connection);
        connection
            .execute_batch(
                r#"
            PRAGMA foreign_keys = OFF;
            DROP TRIGGER public_information_events_no_delete;
            DROP TRIGGER public_information_events_no_update;
            DROP TABLE public_information_events;
            DROP TABLE public_information_resource_state;
            DROP TABLE other_bbs_entries;
            DROP TABLE public_information_policy;
            ALTER TABLE callers DROP COLUMN publicity_state_version;
            ALTER TABLE callers DROP COLUMN public_directory_listed;
            DELETE FROM schema_migrations WHERE version = 14;
            DROP TRIGGER callers_login_identifier_update;
            DROP TRIGGER callers_login_identifier_insert;
            DROP TRIGGER caller_identity_events_no_delete;
            DROP TRIGGER caller_identity_events_no_update;
            DROP TABLE caller_identity_events;
            DROP INDEX callers_login_identifier_unique;
            ALTER TABLE callers DROP COLUMN real_name;
            ALTER TABLE callers DROP COLUMN login_identifier;
            DELETE FROM schema_migrations WHERE version = 13;
            DROP TRIGGER caller_access_events_no_delete;
            DROP TRIGGER caller_access_events_no_update;
            DROP TABLE caller_security_adjustments;
            DROP TABLE caller_access_events;
            ALTER TABLE callers DROP COLUMN lifecycle_prior_state;
            ALTER TABLE callers DROP COLUMN purge_protected;
            ALTER TABLE callers DROP COLUMN subscription_expires_on;
            ALTER TABLE callers DROP COLUMN state_version;
            DELETE FROM schema_migrations WHERE version = 12;
            PRAGMA foreign_keys = ON;
            "#,
            )
            .unwrap();
        drop(connection);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 11;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn rewrite_backup_database_as_schema_13(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_16_to_15(&connection);
        downgrade_schema_15_to_14(&connection);
        connection
            .execute_batch(
                r#"
            PRAGMA foreign_keys = OFF;
            DROP TRIGGER public_information_events_no_delete;
            DROP TRIGGER public_information_events_no_update;
            DROP TABLE public_information_events;
            DROP TABLE public_information_resource_state;
            DROP TABLE other_bbs_entries;
            DROP TABLE public_information_policy;
            ALTER TABLE callers DROP COLUMN publicity_state_version;
            ALTER TABLE callers DROP COLUMN public_directory_listed;
            DELETE FROM schema_migrations WHERE version = 14;
            PRAGMA foreign_keys = ON;
        "#,
            )
            .unwrap();
        drop(connection);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 13;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn rewrite_backup_database_as_schema_14(backup: &Path) {
        let database_path = backup.join(DATABASE_BACKUP_PATH);
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        downgrade_schema_16_to_15(&connection);
        downgrade_schema_15_to_14(&connection);
        drop(connection);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.schema_version = 14;
        let (size_bytes, sha256) = hash_file(&database_path).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.kind == BackupEntryKind::Database)
            .unwrap();
        entry.size_bytes = size_bytes;
        entry.sha256 = sha256;
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn cold_backup_and_new_restore_preserve_all_authoritative_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let mut selected = RuntimeConfig::load(&config_path).unwrap();
        selected.presentation = sf_core::PresentationConfig {
            mode: sf_core::PresentationMode::Profile,
            menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
            active_profile: Some(crate::CLASSIC_PROFILE_ID.to_owned()),
            base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
        };
        selected.caller.post_login_journey = sf_core::PostLoginJourney::Stock;
        selected.save_atomic(&config_path).unwrap();
        let joker_bytes = b"Blocked Synthetic Caller\r\n@Synthetic Fragment\r\n";
        fs::write(source.join("system/JOKER.DAT"), joker_bytes).unwrap();
        let validated = selected.validate().unwrap();
        let paths = LogicalPaths::resolve(&source, &validated).unwrap();
        crate::transports::load_or_generate_host_key(
            paths.get(LogicalPath::System),
            Path::new("ssh/host-ed25519"),
        )
        .unwrap();
        let original_host_key = fs::read(source.join("system/ssh/host-ed25519")).unwrap();
        let mut access_database = RuntimeDatabase::open(paths.database()).unwrap();
        let access_caller = access_database
            .caller_by_name(b"Backup Caller")
            .unwrap()
            .unwrap();
        let access_caller = access_database
            .update_caller_identity(
                access_caller.id,
                access_caller.state_version,
                b"backup-auth",
                b"Backup Caller",
                Some("Backup Real Name".to_owned()),
                &validated.caller,
                1_777_000_099,
            )
            .unwrap();
        let access_caller = access_database
            .change_caller_base_security(
                access_caller.id,
                access_caller.state_version,
                SecurityLevel::new(29).unwrap(),
                sf_core::CallerAccessActor::LocalOperator,
                &validated.caller,
                1_777_000_100,
            )
            .unwrap();
        let access_caller = access_database
            .update_caller_subscription(
                access_caller.id,
                access_caller.state_version,
                Some(chrono::NaiveDate::from_ymd_opt(2027, 8, 29).unwrap()),
                sf_core::CallerAccessActor::LocalOperator,
                &validated.caller,
                1_777_000_101,
                validated.timezone,
            )
            .unwrap();
        access_database
            .set_caller_purge_protection(
                access_caller.id,
                access_caller.state_version,
                false,
                sf_core::CallerAccessActor::LocalOperator,
                &validated.caller,
                1_777_000_102,
            )
            .unwrap();
        let publicity = access_database
            .update_caller_publicity(
                sf_core::PublicInformationActor::Caller(access_caller.id),
                access_caller.id,
                access_caller.publicity_state_version,
                true,
                1_777_000_103,
            )
            .unwrap();
        assert!(publicity.listed);
        access_database
            .update_public_directory_policy(
                sf_core::PublicInformationActor::LocalOperator,
                1,
                true,
                true,
                false,
                false,
                1_777_000_104,
            )
            .unwrap();
        let other_bbs = access_database
            .add_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                sf_core::NewOtherBbsEntry {
                    name: "Backup Fixture BBS".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "backup.example:2222".to_owned(),
                },
                1_777_000_105,
            )
            .unwrap();
        for (kind, digest) in crate::resources::public_resource_digests(&paths).unwrap() {
            access_database
                .observe_public_resource(kind, &digest, 1_777_000_106)
                .unwrap();
        }
        drop(access_database);
        let original_config = fs::read(&config_path).unwrap();
        fs::write(source.join("work/runtime-status.toml"), b"transient").unwrap();
        fs::write(
            source.join("work/upload-staging/incomplete.part"),
            b"untrusted partial upload",
        )
        .unwrap();
        let expected_message_storage = restored_database(&source)
            .message_mutation_storage_stats()
            .unwrap();
        let backup = temp.path().join("snapshot");
        let report = backup_board(&config_path, &backup).unwrap();

        assert_eq!(report.board_name, "Backup Board");
        assert_eq!(report.schema_version, SCHEMA_VERSION);
        assert!(report.resource_files >= 1);
        assert_eq!(report.cataloged_files, 2);
        assert!(backup.join(BACKUP_MANIFEST_FILE).is_file());

        let restored = temp.path().join("restored");
        let restore = restore_board(&backup, &restored, false).unwrap();
        assert!(!restore.replaced_existing);
        assert_eq!(fs::read(&restore.config_path).unwrap(), original_config);
        assert!(!restored.join("work/runtime-status.toml").exists());
        assert!(!restored
            .join("work/upload-staging/incomplete.part")
            .exists());
        assert_eq!(
            fs::read(restored.join("display/PRESERVE.BBS")).unwrap(),
            b"Preserved resource\r\n"
        );
        assert_eq!(
            fs::read(restored.join("display/nested/EXTRA.BBS")).unwrap(),
            b"Preserved nested resource\r\n"
        );
        assert_eq!(
            fs::read(restored.join("system/JOKER.DAT")).unwrap(),
            joker_bytes
        );
        assert_eq!(
            fs::read(restored.join("system/ssh/host-ed25519")).unwrap(),
            original_host_key
        );
        assert_eq!(
            fs::read(restored.join("system/presentation-profiles/modern-ng/profile.toml")).unwrap(),
            fs::read(source.join("system/presentation-profiles/modern-ng/profile.toml")).unwrap()
        );
        assert_eq!(
            fs::read(restored.join("system/presentation-profiles/minimal-terminal/profile.toml"))
                .unwrap(),
            fs::read(source.join("system/presentation-profiles/minimal-terminal/profile.toml"))
                .unwrap()
        );
        assert_eq!(
            fs::read(restored.join("system/presentation-profiles/classic-spitfire/profile.toml"))
                .unwrap(),
            fs::read(source.join("system/presentation-profiles/classic-spitfire/profile.toml"))
                .unwrap()
        );
        assert_eq!(
            fs::read(
                restored.join(
                    "system/presentation-profiles/classic-spitfire/LICENSES/ASSET-LICENSE.txt"
                )
            )
            .unwrap(),
            fs::read(
                source.join(
                    "system/presentation-profiles/classic-spitfire/LICENSES/ASSET-LICENSE.txt"
                )
            )
            .unwrap()
        );
        assert_eq!(
            fs::read(restored.join("system/language-packs/en-US/language.toml")).unwrap(),
            fs::read(source.join("system/language-packs/en-US/language.toml")).unwrap()
        );
        let restored_status = crate::board_status(&restore.config_path).unwrap();
        assert!(restored_status.contains("Active: classic-spitfire 1.6.0"));
        assert!(restored_status.contains("Base: modern-ng 1.5.0"));
        assert!(restored_status.contains("Status: ready"));
        assert!(restored_status.contains("Default locale: en-US"));
        assert!(restored_status.contains("Package: en-US 1.11.0"));
        assert!(restored_status.contains("Status: READY"));

        let database = restored_database(&restored);
        let restored_caller = database.caller_by_name(b"Backup Caller").unwrap().unwrap();
        assert_eq!(restored_caller.base_security_level.get(), 29);
        assert_eq!(restored_caller.login_identifier, "backup-auth");
        assert_eq!(restored_caller.display_name, "Backup Caller");
        assert!(restored_caller.public_directory_listed);
        assert!(database.public_directory_policy().unwrap().enabled);
        let restored_other_bbs = database.other_bbs_entries(true).unwrap();
        assert_eq!(restored_other_bbs.len(), 1);
        assert_eq!(restored_other_bbs[0].id, other_bbs.id);
        for kind in ["bulletins", "newsletter", "thoughts"] {
            assert_eq!(
                database
                    .public_resource_state(kind)
                    .unwrap()
                    .unwrap()
                    .generation,
                1
            );
        }
        assert_eq!(
            restored_caller.real_name.as_deref(),
            Some("Backup Real Name")
        );
        assert_eq!(
            restored_caller.subscription_expires_on.unwrap().to_string(),
            "2027-08-29"
        );
        assert!(!restored_caller.purge_protected);
        assert!(restored_caller.state_version >= 3);
        assert_eq!(
            database.message_mutation_storage_stats().unwrap(),
            expected_message_storage
        );
        assert!(expected_message_storage.recipient_relations >= 3);
        assert!(expected_message_storage.tombstones >= 1);
        assert!(expected_message_storage.receipts >= 1);
        assert!(expected_message_storage.lineage_relations >= 1);
        assert!(expected_message_storage.audit_events >= 3);
        let caller = database.caller_by_name(b"Backup Caller").unwrap().unwrap();
        let actor = MessageActor::new(caller.id, SecurityLevel::new(255).unwrap());
        let conference = database.conference(actor, 1).unwrap();
        let messages = database.messages(actor, conference.id).unwrap();
        assert!(messages
            .iter()
            .any(|message| message.subject == b"Backup preservation"));
        let config = RuntimeConfig::load(&restore.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        for (area, file) in database.all_cataloged_files().unwrap() {
            storage.open_download(&area, &file).unwrap();
        }
        drop(database);
        let runtime = BoardRuntime::load(&restore.config_path).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Backup Caller".to_vec(),
            b"test-only caller password".to_vec(),
            b"N".to_vec(),
            b"G".to_vec(),
        ]);
        let ConnectionReport::Completed(connection) =
            runtime.run_connection(&mut terminal).unwrap()
        else {
            panic!("restored board unexpectedly reported all nodes busy");
        };
        assert_eq!(connection.caller_name.as_deref(), Some("Backup Caller"));
        assert!(terminal
            .output()
            .windows(b"SPITFIRE MESSAGE SUMMARY".len())
            .any(|window| window == b"SPITFIRE MESSAGE SUMMARY"));
        assert!(terminal
            .output()
            .windows(b"MAIN MENU - Selection?".len())
            .any(|window| window == b"MAIN MENU - Selection?"));
        assert!(terminal.output().contains(&0x1b));
    }

    #[test]
    fn schema_15_backup_preserves_file_requests_policies_and_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-15-file-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let config = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &config).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"Backup Caller").unwrap().unwrap();
        let actor = sf_core::FileActor::new(
            caller.id,
            SecurityLevel::new(config.caller.sysop_security).unwrap(),
        );
        let (_, file) = database.all_cataloged_files().unwrap().remove(0);
        let offline = database
            .set_file_lifecycle(
                sf_core::FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                sf_core::FileLifecycle::Offline,
            )
            .unwrap();
        let request = database
            .create_file_request_on_board_day(actor, offline.id, None, "2026-08-29")
            .unwrap();
        database
            .replace_upload_denials(
                sf_core::FileAdminActor::LocalOperator,
                1,
                &["PRIVATE*.ZIP".to_owned()],
            )
            .unwrap();
        drop(database);

        let backup = temp.path().join("schema-15-file-backup");
        backup_board(&config_path, &backup).unwrap();
        rewrite_backup_database_as_schema_15(&backup);
        let restored = temp.path().join("schema-15-file-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 15);
        let restored_config = RuntimeConfig::load(&report.config_path)
            .unwrap()
            .validate()
            .unwrap();
        let restored_paths = LogicalPaths::resolve(&restored, &restored_config).unwrap();
        let snapshot = RuntimeDatabase::open_read_only(restored_paths.database()).unwrap();
        snapshot.validate_snapshot_at_version(15).unwrap();
        drop(snapshot);
        let mut restored_database = RuntimeDatabase::open(restored_paths.database()).unwrap();
        assert_eq!(
            restored_database.migrate().unwrap(),
            sf_core::MigrationReport {
                starting_version: 15,
                ending_version: SCHEMA_VERSION,
                applied: 4,
            }
        );
        let restored_requests = restored_database
            .pending_file_requests(sf_core::FileAdminActor::LocalOperator)
            .unwrap();
        assert_eq!(restored_requests.len(), 1);
        assert_eq!(restored_requests[0].request_id, request.request_id);
        assert_eq!(restored_requests[0].file_id, offline.id);
        assert!(restored_database
            .upload_is_denied(actor, "PRIVATE-DATA.ZIP")
            .unwrap());
        let restored_file = restored_database
            .all_cataloged_files()
            .unwrap()
            .into_iter()
            .find(|(_, entry)| entry.id == offline.id)
            .unwrap()
            .1;
        assert_eq!(restored_file.lifecycle, sf_core::FileLifecycle::Offline);
    }

    #[test]
    fn schema_17_backup_and_new_root_restore_preserve_zero_byte_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-17-zero-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let config = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &config).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let (area, _) = database.all_cataloged_files().unwrap().remove(0);
        let storage = FileStorage::new(&paths).unwrap();
        let empty = storage
            .write_seed_file(
                &mut database,
                &area,
                "EMPTY.BIN",
                "Valid empty backup fixture",
                b"",
                1_777_000_020,
            )
            .unwrap();
        drop(database);

        let backup = temp.path().join("schema-17-zero-backup");
        backup_board(&config_path, &backup).unwrap();
        let restored = temp.path().join("schema-17-zero-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, SCHEMA_VERSION);
        let restored_config = RuntimeConfig::load(&report.config_path)
            .unwrap()
            .validate()
            .unwrap();
        let restored_paths = LogicalPaths::resolve(&restored, &restored_config).unwrap();
        let restored_database = RuntimeDatabase::open_read_only(restored_paths.database()).unwrap();
        restored_database.validate_current_snapshot().unwrap();
        let (restored_area, restored_empty) = restored_database
            .all_cataloged_files()
            .unwrap()
            .into_iter()
            .find(|(_, file)| file.id == empty.id)
            .unwrap();
        assert_eq!(restored_empty.size_bytes, 0);
        let restored_storage = FileStorage::open_existing(&restored_paths).unwrap();
        assert_eq!(
            restored_storage
                .open_download(&restored_area, &restored_empty)
                .unwrap()
                .metadata()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn schema_18_backup_restore_preserves_observability_and_restarts_live_state_fresh() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-18-observability-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let mut warning = NewOperationalEvent::new(
            1_800_000_000,
            EventCategory::Storage,
            EventSeverity::Warning,
            "storage.unavailable",
            EventOutcome::Unavailable,
        );
        warning.idempotency_key = Some("backup-observability-storage-warning".to_owned());
        warning.attributes = EventAttributes::Storage {
            state: "unavailable".to_owned(),
        };
        let warning = database.record_operational_event(&warning).unwrap();
        let actor = OperatorPrincipal {
            kind: OperatorPrincipalKind::HostOperator,
            stable_id: Some("backup-test-operator".to_owned()),
        };
        database
            .update_retention_policy(1, 45, 500, None, &actor, 1_800_000_001)
            .unwrap()
            .unwrap();
        database
            .accept_operator_command(&NewOperatorCommandReceipt {
                command_id: "backup-command-0001".to_owned(),
                daemon_generation: "1".repeat(32),
                operator_id: "unix-uid:501".to_owned(),
                command_family: "system".to_owned(),
                command_type: "system.example".to_owned(),
                request_fingerprint: "a".repeat(64),
                target_kind: None,
                target_id: None,
                target_generation: None,
                received_at: 1_800_000_001,
            })
            .unwrap();
        database
            .record_operator_control_audit(&NewOperatorControlAudit {
                occurred_at: 1_800_000_002,
                operator_kind: "host-operator".to_owned(),
                operator_id: Some("unix-uid:501".to_owned()),
                operation: "operator.authenticate".to_owned(),
                authorization_result: "allowed".to_owned(),
                target_kind: None,
                target_id: None,
                command_id: None,
                correlation_id: None,
                outcome: "succeeded".to_owned(),
                detail_code: None,
            })
            .unwrap();
        drop(database);

        let backup = temp.path().join("schema-18-observability-backup");
        backup_board(&config_path, &backup).unwrap();
        let restored = temp.path().join("schema-18-observability-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, SCHEMA_VERSION);
        let restored_config = RuntimeConfig::load(&report.config_path)
            .unwrap()
            .validate()
            .unwrap();
        let restored_paths = LogicalPaths::resolve(&restored, &restored_config).unwrap();
        let restored_database = RuntimeDatabase::open_read_only(restored_paths.database()).unwrap();
        restored_database.validate_current_snapshot().unwrap();
        assert_eq!(
            restored_database.retention_policy().unwrap().detail_days,
            45
        );
        assert_eq!(restored_database.notifications(false, 10).unwrap().len(), 1);
        let events = restored_database
            .query_operational_events(&EventQuery {
                limit: Some(100),
                ..EventQuery::default()
            })
            .unwrap();
        assert!(events.events.iter().any(|event| event.id == warning.id));
        assert!(events
            .events
            .iter()
            .any(|event| event.event_code == "backup.started"));
        let summary = restored_database
            .daily_operational_summary(warning.board_day, warning.timezone_policy_version)
            .unwrap()
            .unwrap();
        assert_eq!(summary.warning_events, 1);
        let audit_count: i64 = rusqlite::Connection::open(restored_paths.database())
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM operator_observability_audit",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audit_count, 1);
        let control_counts: (i64, i64) = rusqlite::Connection::open(restored_paths.database())
            .unwrap()
            .query_row("SELECT (SELECT COUNT(*) FROM operator_command_journal),(SELECT COUNT(*) FROM operator_control_audit)", [], |row| Ok((row.get(0)?,row.get(1)?)))
            .unwrap();
        assert_eq!(control_counts, (1, 1));
        let highest_restored_id = events
            .events
            .iter()
            .map(|event| event.id.get())
            .max()
            .unwrap();
        drop(restored_database);
        let mut writable = RuntimeDatabase::open(restored_paths.database()).unwrap();
        let mut after_restore = NewOperationalEvent::new(
            1_800_000_100,
            EventCategory::System,
            EventSeverity::Info,
            "system.restore-verified",
            EventOutcome::Succeeded,
        );
        after_restore.idempotency_key = Some("post-restore-sequence-check".to_owned());
        let after_restore = writable.record_operational_event(&after_restore).unwrap();
        assert!(after_restore.id.get() > highest_restored_id);
        let live = ObservabilityService::new(restored_paths.database(), 1_800_000_101);
        assert!(live.refresh_live(1_800_000_101).unwrap().is_empty());
    }

    #[test]
    fn schema_17_restore_migrates_without_fabricating_observability_history() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-17-observability-source");
        let backup = temp.path().join("schema-17-observability-backup");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        rewrite_backup_database_as_schema_17(&backup);
        let restored = temp.path().join("schema-17-observability-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 17);
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            sf_core::MigrationReport {
                starting_version: 17,
                ending_version: SCHEMA_VERSION,
                applied: 2,
            }
        );
        assert!(database
            .query_operational_events(&EventQuery::default())
            .unwrap()
            .events
            .is_empty());
        assert!(database.notifications(true, 100).unwrap().is_empty());
    }

    #[test]
    fn failed_cold_backup_creates_a_safe_actionable_notification() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "failed-backup-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &validated).unwrap();
        let database = RuntimeDatabase::open_read_only(paths.database()).unwrap();
        let (area, file) = database.managed_cataloged_files().unwrap().remove(0);
        drop(database);
        let cataloged = paths
            .get(LogicalPath::External)
            .join("files")
            .join(area.storage_key)
            .join(file.filename);
        fs::write(cataloged, b"changed after cataloging").unwrap();
        assert!(backup_board(&config_path, &temp.path().join("failed-backup")).is_err());
        let database = restored_database(&source);
        let failures = database
            .query_operational_events(&EventQuery {
                category: Some(EventCategory::Backup),
                outcome: Some(EventOutcome::Failed),
                limit: Some(10),
                ..EventQuery::default()
            })
            .unwrap();
        assert_eq!(failures.events.len(), 1);
        assert_eq!(failures.events[0].event_code, "backup.failed");
        assert_eq!(database.notifications(false, 10).unwrap().len(), 1);
        let serialized = format!("{:?}", failures.events[0]);
        assert!(!serialized.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn external_catalog_survives_restore_without_media_then_rebinds_by_stable_id() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "external-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let config = RuntimeConfig::load(&config_path).unwrap();
        let paths = LogicalPaths::resolve(&source, &config.validate().unwrap()).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let sysop = database.caller_by_name(b"Sysop").unwrap().unwrap();
        let actor = FileActor::new(
            sysop.id,
            SecurityLevel::new(config.caller.sysop_security).unwrap(),
        );
        let area = database.all_file_areas().unwrap().remove(0);
        let bytes = b"external restore fixture";
        let file = storage
            .write_seed_file(
                &mut database,
                &area,
                "ARCHIVE.BIN",
                "External restore fixture",
                bytes,
                1_777_100_000,
            )
            .unwrap();
        let external = temp.path().join("original-external-media");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("ARCHIVE.BIN"), bytes).unwrap();
        let root = database
            .add_storage_root(
                actor,
                sf_core::StorageRootDefinition {
                    area_id: area.id,
                    stable_key: "external-archive",
                    label: "External archive",
                    configured_locator: external.to_str().unwrap(),
                    priority: 1,
                    mode: sf_core::StorageRootMode::ReadOnly,
                    occurred_at: 1_777_100_001,
                },
            )
            .unwrap();
        database
            .set_storage_availability(
                actor,
                root.id,
                root.state_version,
                sf_core::StorageAvailability::Available,
                1_777_100_002,
            )
            .unwrap();
        database
            .set_file_storage_locator(
                actor,
                file.id,
                root.id,
                "ARCHIVE.BIN",
                file.state_version,
                1,
                1_777_100_003,
            )
            .unwrap();
        let backup = temp.path().join("external-backup");
        backup_board(&config_path, &backup).unwrap();
        fs::remove_dir_all(&external).unwrap();
        let restored = temp.path().join("external-restored");
        restore_board(&backup, &restored, false).unwrap();

        let restored_config = RuntimeConfig::load(&restored.join(BOARD_CONFIG_FILE)).unwrap();
        let restored_paths =
            LogicalPaths::resolve(&restored, &restored_config.validate().unwrap()).unwrap();
        let mut restored_database = RuntimeDatabase::open(restored_paths.database()).unwrap();
        let restored_sysop = restored_database.caller_by_name(b"Sysop").unwrap().unwrap();
        let restored_actor = FileActor::new(
            restored_sysop.id,
            SecurityLevel::new(restored_config.caller.sysop_security).unwrap(),
        );
        let restored_file = restored_database
            .all_cataloged_files()
            .unwrap()
            .into_iter()
            .map(|(_, file)| file)
            .find(|candidate| candidate.id == file.id)
            .unwrap();
        assert_eq!(restored_file.id, file.id);
        let (restored_root, _) = restored_database.resolve_file_storage(file.id).unwrap();
        assert_eq!(
            restored_root.availability,
            sf_core::StorageAvailability::Unknown
        );
        let rebound = temp.path().join("rebound-external-media");
        fs::create_dir(&rebound).unwrap();
        fs::write(rebound.join("ARCHIVE.BIN"), bytes).unwrap();
        let rebound_version = restored_database
            .rebind_external_storage_root(
                restored_actor,
                restored_root.id,
                restored_root.state_version,
                rebound.to_str().unwrap(),
                1_777_100_004,
            )
            .unwrap();
        restored_database
            .set_storage_availability(
                restored_actor,
                restored_root.id,
                rebound_version,
                sf_core::StorageAvailability::Available,
                1_777_100_005,
            )
            .unwrap();
        let (available_root, locator) = restored_database.resolve_file_storage(file.id).unwrap();
        let restored_storage = FileStorage::new(&restored_paths).unwrap();
        let mut input = restored_storage
            .prepare_resolved_download(&available_root, &locator, &restored_file)
            .unwrap();
        let mut restored_bytes = Vec::new();
        input.read_to_end(&mut restored_bytes).unwrap();
        assert_eq!(restored_bytes, bytes);
    }

    #[test]
    fn schema_15_backup_rejects_staged_file_operations_until_recovery_normalizes_them() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-15-staged-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let config = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &config).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let (source_area, file) = database.all_cataloged_files().unwrap().remove(0);
        let destination = database
            .create_file_area(&sf_core::FileAreaDefinition {
                number: 9,
                name: "Backup Recovery Destination".to_owned(),
                description: "Synthetic staged-operation backup fixture".to_owned(),
                storage_key: "backup-recovery-destination".to_owned(),
                access_mode: sf_core::FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(1).unwrap(),
                upload_security: SecurityLevel::new(1).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        storage.ensure_area(&destination).unwrap();
        assert!(matches!(
            database.move_file_with_failure(
                &storage,
                sf_core::FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version,
                Some(sf_core::FailureInjectionPoint::AfterStage)
            ),
            Err(sf_core::FileMaintenanceError::InjectedFailure)
        ));
        drop(database);

        let rejected = temp.path().join("schema-15-staged-rejected");
        assert!(matches!(
            backup_board(&config_path, &rejected),
            Err(ApplicationError::Backup(
                BoardBackupError::UnnormalizedFileOperations
            ))
        ));
        assert!(!rejected.exists());

        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        assert_eq!(database.recover_file_operations(&storage).unwrap(), 1);
        assert_eq!(
            database
                .all_cataloged_files()
                .unwrap()
                .into_iter()
                .find(|(_, entry)| entry.id == file.id)
                .unwrap()
                .1
                .area_id,
            source_area.id
        );
        drop(database);
        let normalized = temp.path().join("schema-15-staged-normalized");
        backup_board(&config_path, &normalized).unwrap();
        assert!(normalized.is_dir());
    }

    #[test]
    fn schema_16_backup_rejects_active_transfer_reservations_until_released() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-16-transfer-source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let config = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&source, &config).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"Backup Caller").unwrap().unwrap();
        let actor = FileActor::new(
            caller.id,
            SecurityLevel::new(config.caller.sysop_security).unwrap(),
        );
        let (_, file) = database.all_cataloged_files().unwrap().remove(0);
        let mut queue = TransferQueue::default();
        queue.tag(&file, false).unwrap();
        let reservation = database
            .reserve_download_queue(
                actor,
                NodeId::new(1).unwrap(),
                config.timezone,
                TransferMethod::Binary(TransferProtocol::YmodemBatch),
                &queue,
                1_777_000_010,
            )
            .unwrap();
        drop(database);

        let rejected = temp.path().join("schema-16-active-transfer-rejected");
        assert!(matches!(
            backup_board(&config_path, &rejected),
            Err(ApplicationError::Backup(
                BoardBackupError::UnnormalizedTransferOperations
            ))
        ));
        assert!(!rejected.exists());

        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        database
            .release_transfer(
                &reservation.id,
                TransferRuntimeState::Cancelled,
                Some(sf_core::TransferCancelSource::Operator),
                Some("backup-drain"),
                1_777_000_011,
            )
            .unwrap();
        drop(database);
        let normalized = temp.path().join("schema-16-transfer-normalized");
        backup_board(&config_path, &normalized).unwrap();
        assert!(normalized.is_dir());
    }

    #[test]
    fn schema_10_backup_restores_exactly_and_migrates_only_on_writable_startup() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-10-source");
        let backup = temp.path().join("schema-10-snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        rewrite_backup_database_as_schema_10(&backup);

        let restored = temp.path().join("schema-10-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 10);
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        let snapshot = RuntimeDatabase::open_read_only(paths.database()).unwrap();
        snapshot.validate_snapshot_at_version(10).unwrap();
        drop(snapshot);

        let mut migrated = RuntimeDatabase::open(paths.database()).unwrap();
        let migration = migrated.migrate().unwrap();
        assert_eq!(migration.starting_version, 10);
        assert_eq!(migration.ending_version, SCHEMA_VERSION);
        assert_eq!(migration.applied, 9);
        migrated.validate_current_snapshot().unwrap();
    }

    #[test]
    fn schema_11_backup_restores_exactly_then_migrates_to_current_schema() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-11-source");
        let backup = temp.path().join("schema-11-snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        rewrite_backup_database_as_schema_11(&backup);
        let restored = temp.path().join("schema-11-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 11);
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        RuntimeDatabase::open_read_only(paths.database())
            .unwrap()
            .validate_snapshot_at_version(11)
            .unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            sf_core::MigrationReport {
                starting_version: 11,
                ending_version: SCHEMA_VERSION,
                applied: 8,
            }
        );
        database.validate_current_snapshot().unwrap();
        let caller = database.caller_by_name(b"Backup Caller").unwrap().unwrap();
        assert_eq!(caller.base_security_level, caller.security_level);
        assert_eq!(caller.subscription_expires_on, None);
        assert_eq!(caller.state_version, 0);
    }

    #[test]
    fn schema_13_backup_restores_exactly_then_migrates_to_current_schema() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-13-source");
        let backup = temp.path().join("schema-13-snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        rewrite_backup_database_as_schema_13(&backup);
        let restored = temp.path().join("schema-13-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 13);
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        RuntimeDatabase::open_read_only(paths.database())
            .unwrap()
            .validate_snapshot_at_version(13)
            .unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            sf_core::MigrationReport {
                starting_version: 13,
                ending_version: SCHEMA_VERSION,
                applied: 6
            }
        );
        assert!(!database.public_directory_policy().unwrap().enabled);
        assert!(database.other_bbs_entries(true).unwrap().is_empty());
        for caller in database.all_callers().unwrap() {
            assert!(!caller.public_directory_listed);
            assert_eq!(caller.publicity_state_version, 0);
        }
        database.validate_current_snapshot().unwrap();
    }

    #[test]
    fn schema_14_backup_restores_exactly_then_migrates_to_current_schema() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "schema-14-source");
        let backup = temp.path().join("schema-14-snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        rewrite_backup_database_as_schema_14(&backup);
        let restored = temp.path().join("schema-14-restored");
        let report = restore_board(&backup, &restored, false).unwrap();
        assert_eq!(report.schema_version, 14);
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let paths = LogicalPaths::resolve(&restored, &config.validate().unwrap()).unwrap();
        RuntimeDatabase::open_read_only(paths.database())
            .unwrap()
            .validate_snapshot_at_version(14)
            .unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            sf_core::MigrationReport {
                starting_version: 14,
                ending_version: SCHEMA_VERSION,
                applied: 5,
            }
        );
        database.validate_current_snapshot().unwrap();
        assert!(database
            .pending_file_requests(sf_core::FileAdminActor::LocalOperator)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn restore_refuses_older_and_newer_schema_manifests_before_target_creation() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "version-source");
        for (schema_version, directory) in [(9, "older"), (SCHEMA_VERSION + 1, "newer")] {
            let backup = temp.path().join(format!("{directory}-snapshot"));
            backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
            let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
            let mut manifest: BackupManifest =
                toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
            manifest.schema_version = schema_version;
            fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
            let target = temp.path().join(format!("{directory}-target"));
            assert!(matches!(
                restore_board(&backup, &target, false),
                Err(ApplicationError::Backup(
                    BoardBackupError::UnsupportedSchema { .. }
                ))
            ));
            assert!(!target.exists());
        }
    }

    #[test]
    fn validated_replace_restores_the_checkpoint_and_removes_later_state() {
        let temp = tempfile::tempdir().unwrap();
        let target = installed_board(temp.path(), "board");
        let config_path = target.join(BOARD_CONFIG_FILE);
        let backup = temp.path().join("snapshot");
        backup_board(&config_path, &backup).unwrap();

        let config = RuntimeConfig::load(&config_path).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&target, &validated).unwrap();
        fs::write(
            paths.get(LogicalPath::Display).join("PRESERVE.BBS"),
            b"later resource\r\n",
        )
        .unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let hasher = CredentialHasher::new(&validated.caller.password).unwrap();
        database
            .create_caller(
                b"Later Caller",
                &hasher.hash(b"test-only later password").unwrap(),
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1_777_100_000,
            )
            .unwrap();
        drop(database);

        let report = restore_board(&backup, &target, true).unwrap();
        assert!(report.replaced_existing);
        assert_eq!(
            fs::read(target.join("display/PRESERVE.BBS")).unwrap(),
            b"Preserved resource\r\n"
        );
        let database = restored_database(&target);
        assert!(database.caller_by_name(b"Backup Caller").unwrap().is_some());
        assert!(database.caller_by_name(b"Later Caller").unwrap().is_none());
        assert!(!rollback_path(&target).unwrap().exists());
    }

    #[test]
    fn corruption_is_rejected_before_destructive_restore() {
        let temp = tempfile::tempdir().unwrap();
        let target = installed_board(temp.path(), "board");
        let config_path = target.join(BOARD_CONFIG_FILE);
        let backup = temp.path().join("snapshot");
        backup_board(&config_path, &backup).unwrap();
        fs::write(target.join("display/PRESERVE.BBS"), b"live target\r\n").unwrap();

        let catalog_path = collect_regular_files(&backup.join("files"))
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .1;
        fs::write(&catalog_path, b"corrupt").unwrap();
        assert!(matches!(
            restore_board(&backup, &target, true),
            Err(ApplicationError::Backup(
                BoardBackupError::ChecksumMismatch(_)
            ))
        ));
        assert_eq!(
            fs::read(target.join("display/PRESERVE.BBS")).unwrap(),
            b"live target\r\n"
        );
        assert!(!rollback_path(&target).unwrap().exists());
    }

    #[test]
    fn manifest_traversal_and_extra_files_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "source");
        let backup = temp.path().join("snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &backup).unwrap();
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: BackupManifest =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.entries[0].path = "../outside".to_owned();
        fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
        assert!(matches!(
            restore_board(&backup, &temp.path().join("restored"), false),
            Err(ApplicationError::Backup(
                BoardBackupError::UnsafeManifestPath(_)
            ))
        ));

        let clean_backup = temp.path().join("clean-snapshot");
        backup_board(&source.join(BOARD_CONFIG_FILE), &clean_backup).unwrap();
        fs::write(clean_backup.join("undeclared.bin"), b"extra").unwrap();
        assert!(matches!(
            restore_board(&clean_backup, &temp.path().join("restored"), false),
            Err(ApplicationError::Backup(
                BoardBackupError::ManifestInventoryMismatch
            ))
        ));
    }

    #[test]
    fn missing_catalog_bytes_and_live_board_lock_fail_without_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "source");
        let config_path = source.join(BOARD_CONFIG_FILE);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let locked_backup = temp.path().join("locked-snapshot");
        assert!(matches!(
            backup_board(&config_path, &locked_backup),
            Err(ApplicationError::BoardInUse(_))
        ));
        assert!(!locked_backup.exists());
        drop(runtime);

        let config = RuntimeConfig::load(&config_path).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&source, &validated).unwrap();
        let database = RuntimeDatabase::open_read_only(paths.database()).unwrap();
        let (area, file) = database.all_cataloged_files().unwrap().remove(0);
        fs::remove_file(
            paths
                .get(LogicalPath::External)
                .join("files")
                .join(area.storage_key)
                .join(file.filename),
        )
        .unwrap();
        drop(database);
        let incomplete = temp.path().join("incomplete-snapshot");
        assert!(backup_board(&config_path, &incomplete).is_err());
        assert!(!incomplete.exists());
    }

    #[test]
    fn sysop_cli_reports_contents_and_requires_explicit_replace() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "source");
        let backup = temp.path().join("snapshot");
        let output = crate::run_cli([
            std::ffi::OsString::from("backup"),
            source.join(BOARD_CONFIG_FILE).into_os_string(),
            backup.clone().into_os_string(),
        ])
        .unwrap();
        assert!(output.contains("cold backup complete"));
        assert!(output.contains("Runtime status and incomplete upload staging are excluded"));

        let restored = temp.path().join("restored");
        let output = crate::run_cli([
            std::ffi::OsString::from("restore"),
            backup.clone().into_os_string(),
            restored.clone().into_os_string(),
        ])
        .unwrap();
        assert!(output.contains("Existing board replaced: no"));
        assert!(matches!(
            crate::run_cli([
                std::ffi::OsString::from("restore"),
                backup.into_os_string(),
                restored.into_os_string(),
            ]),
            Err(ApplicationError::Backup(
                BoardBackupError::RestoreTargetExists(_)
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn logical_directory_symlinks_are_never_followed_into_a_backup() {
        let temp = tempfile::tempdir().unwrap();
        let source = installed_board(temp.path(), "source");
        fs::rename(source.join("display"), source.join("real-display")).unwrap();
        std::os::unix::fs::symlink("real-display", source.join("display")).unwrap();
        let destination = temp.path().join("snapshot");
        assert!(matches!(
            backup_board(&source.join(BOARD_CONFIG_FILE), &destination),
            Err(ApplicationError::Backup(BoardBackupError::UnsafeObject(path)))
                if path.file_name().is_some_and(|name| name == "display")
        ));
        assert!(!destination.exists());
    }
}
