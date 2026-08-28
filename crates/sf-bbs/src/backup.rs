use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sf_core::{
    FileStorage, LogicalPath, LogicalPaths, RuntimeConfig, RuntimeDatabase, TerminalInfo,
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

    let database = RuntimeDatabase::open_read_only(paths.database())?;
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
    let catalog = database.all_cataloged_files()?;
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
    let created_at = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| BoardBackupError::Clock)?
            .as_secs(),
    )
    .map_err(|_| BoardBackupError::Clock)?;
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
    Ok(BackupReport {
        destination,
        board_name: manifest.board_name,
        schema_version: manifest.schema_version,
        resource_files,
        cataloged_files: catalog.len(),
        total_bytes,
    })
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
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(BoardBackupError::UnsupportedSchema {
            found: manifest.schema_version,
            supported: SCHEMA_VERSION,
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
    let identity = database.validate_current_snapshot()?;
    if identity.name() != manifest.board_name
        || identity.sysop_name() != manifest.sysop_name
        || identity != validated.identity
    {
        return Err(BoardBackupError::IdentityMismatch);
    }

    let mut expected_catalog = BTreeMap::new();
    for (area, file) in database.all_cataloged_files()? {
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
    let identity = database.validate_current_snapshot()?;
    if identity != validated.identity {
        return Err(BoardBackupError::IdentityMismatch);
    }
    let storage = FileStorage::new(&paths)?;
    for (area, file) in database.all_cataloged_files()? {
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
    #[error("backup schema {found} is unsupported; this build requires {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
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
    File(#[from] sf_core::FileError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{
        CallerState, CredentialHasher, InMemoryTerminal, MessageActor, MessageBackend, MessageKind,
        MessageVisibility, NewMessage, PasswordHashConfig, SecurityLevel,
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
        let actor = MessageActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let conference = database.conference(actor, 1).unwrap();
        database
            .post(
                actor,
                NewMessage {
                    conference_id: conference.id,
                    recipient_caller_id: None,
                    recipient_name: "All Callers".to_owned(),
                    subject: b"Backup preservation".to_vec(),
                    body: b"Callers, messages, and receipts remain in SQLite.\r\n".to_vec(),
                    created_at: 1_777_000_001,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();
        root
    }

    fn restored_database(root: &Path) -> RuntimeDatabase {
        let config = RuntimeConfig::load(&root.join(BOARD_CONFIG_FILE)).unwrap();
        let paths = LogicalPaths::resolve(root, &config.validate().unwrap()).unwrap();
        RuntimeDatabase::open_read_only(paths.database()).unwrap()
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
        let original_config = fs::read(&config_path).unwrap();
        fs::write(source.join("work/runtime-status.toml"), b"transient").unwrap();
        fs::write(
            source.join("work/upload-staging/incomplete.part"),
            b"untrusted partial upload",
        )
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
        assert!(restored_status.contains("Active: classic-spitfire 1.2.0"));
        assert!(restored_status.contains("Base: modern-ng 1.1.0"));
        assert!(restored_status.contains("Status: ready"));
        assert!(restored_status.contains("Default locale: en-US"));
        assert!(restored_status.contains("Package: en-US 1.1.1"));
        assert!(restored_status.contains("Status: READY"));

        let database = restored_database(&restored);
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
