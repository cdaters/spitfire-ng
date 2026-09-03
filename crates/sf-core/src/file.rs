use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroI64;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Caller, CallerError, CallerId, CallerState, DatabaseError, LogicalPath, LogicalPaths,
    RuntimeDatabase, SecurityLevel, SessionId, StorageAvailability, StorageRoot, StorageRootKind,
    Terminal, TerminalError,
};

pub const MAX_FILE_AREAS: u16 = u16::MAX;
pub const MAX_FILE_NAME_BYTES: usize = 64;
pub const MAX_FILE_DESCRIPTION_BYTES: usize = 4096;
pub const MAX_FILE_DESCRIPTION_LINES: usize = 20;
pub const MAX_DESCRIPTION_SEARCH_WORDS: usize = 6;
const COPY_BUFFER_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileAreaId(NonZeroI64);

impl FileAreaId {
    pub fn new(value: i64) -> Result<Self, FileError> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or(FileError::InvalidAreaId(value))
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileId(NonZeroI64);

impl FileId {
    pub fn new(value: i64) -> Result<Self, FileError> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or(FileError::InvalidFileId(value))
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccessMode {
    AtLeast,
    Exact,
}

impl FileAccessMode {
    const fn as_database_value(self) -> &'static str {
        match self {
            Self::AtLeast => "at-least",
            Self::Exact => "exact",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, FileError> {
        match value {
            "at-least" => Ok(Self::AtLeast),
            "exact" => Ok(Self::Exact),
            _ => Err(FileError::InvalidStoredAccessMode(value.to_owned())),
        }
    }

    const fn allows(self, actual: SecurityLevel, required: SecurityLevel) -> bool {
        match self {
            Self::AtLeast => actual.allows(required),
            Self::Exact => actual.get() == required.get(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccess {
    Full,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageRootAccess {
    ManagedReadWrite,
    ReadOnlySecondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalFileStorageRoot {
    pub logical_key: String,
    pub order: u16,
    pub access: StorageRootAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileLifecycle {
    Active,
    Offline,
    PendingReview,
    Disabled,
    Tombstoned,
}

impl FileLifecycle {
    pub(crate) const fn as_database_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Offline => "offline",
            Self::PendingReview => "pending-review",
            Self::Disabled => "disabled",
            Self::Tombstoned => "tombstoned",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, FileError> {
        match value {
            "active" => Ok(Self::Active),
            "offline" => Ok(Self::Offline),
            "pending-review" => Ok(Self::PendingReview),
            "disabled" => Ok(Self::Disabled),
            "tombstoned" => Ok(Self::Tombstoned),
            _ => Err(FileError::InvalidStoredLifecycle(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileIntegrity {
    Unknown,
    Present,
    Missing,
    DigestMismatch,
}

impl FileIntegrity {
    pub(crate) const fn as_database_value(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Present => "present",
            Self::Missing => "missing",
            Self::DigestMismatch => "digest-mismatch",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, FileError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "present" => Ok(Self::Present),
            "missing" => Ok(Self::Missing),
            "digest-mismatch" => Ok(Self::DigestMismatch),
            _ => Err(FileError::InvalidStoredIntegrity(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileArea {
    pub id: FileAreaId,
    pub number: u16,
    pub name: String,
    pub description: String,
    pub storage_key: String,
    pub access_mode: FileAccessMode,
    pub read_security: SecurityLevel,
    pub upload_security: SecurityLevel,
    pub preview: bool,
    pub no_charge: bool,
    pub maximum_upload_bytes: u64,
    pub privileged_security_levels: Vec<SecurityLevel>,
    pub active: bool,
    pub state_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAreaDefinition {
    pub number: u16,
    pub name: String,
    pub description: String,
    pub storage_key: String,
    pub access_mode: FileAccessMode,
    pub read_security: SecurityLevel,
    pub upload_security: SecurityLevel,
    pub preview: bool,
    pub no_charge: bool,
    pub maximum_upload_bytes: u64,
    pub privileged_security_levels: Vec<SecurityLevel>,
}

impl FileAreaDefinition {
    pub fn validate(&self) -> Result<(), FileError> {
        validate_area(self)
    }
}

impl FileArea {
    pub(crate) fn access(
        &self,
        caller: &Caller,
        sysop_security: SecurityLevel,
    ) -> Option<FileAccess> {
        let privileged = caller.security_level.is_sysop(sysop_security)
            || self
                .privileged_security_levels
                .contains(&caller.security_level);
        if privileged
            || self
                .access_mode
                .allows(caller.security_level, self.read_security)
        {
            Some(FileAccess::Full)
        } else if self.preview {
            Some(FileAccess::Preview)
        } else {
            None
        }
    }

    pub(crate) fn allows_upload(&self, caller: &Caller, sysop_security: SecurityLevel) -> bool {
        caller.security_level.is_sysop(sysop_security)
            || self
                .privileged_security_levels
                .contains(&caller.security_level)
            || self
                .access_mode
                .allows(caller.security_level, self.upload_security)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
    pub id: FileId,
    pub area_id: FileAreaId,
    pub filename: String,
    pub description: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub uploaded_at: i64,
    pub uploader_caller_id: Option<CallerId>,
    pub uploader_name: String,
    pub download_count: u64,
    pub lifecycle: FileLifecycle,
    pub integrity: FileIntegrity,
    pub state_version: u64,
    pub description_source: String,
    pub description_source_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewFileEntry {
    pub area_id: FileAreaId,
    pub filename: String,
    pub description: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub uploaded_at: i64,
    pub uploader_caller_id: Option<CallerId>,
    pub uploader_name: String,
    pub lifecycle: FileLifecycle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FileStatistics {
    pub new_since_checkpoint: u64,
    pub available_files: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileActor {
    caller_id: CallerId,
    sysop_security: SecurityLevel,
}

impl FileActor {
    pub const fn new(caller_id: CallerId, sysop_security: SecurityLevel) -> Self {
        Self {
            caller_id,
            sysop_security,
        }
    }

    pub const fn caller_id(self) -> CallerId {
        self.caller_id
    }

    pub(crate) const fn sysop_security(self) -> SecurityLevel {
        self.sysop_security
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileSearch {
    Filename(String),
    Description(Vec<String>),
    NewSince(i64),
}

pub trait FileBackend {
    fn file_areas(&self, actor: FileActor) -> Result<Vec<(FileArea, FileAccess)>, FileError>;
    fn file_area(
        &self,
        actor: FileActor,
        area_number: u16,
    ) -> Result<(FileArea, FileAccess), FileError>;
    fn files(&self, actor: FileActor, area: FileAreaId) -> Result<Vec<FileEntry>, FileError>;
    fn search_files(
        &self,
        actor: FileActor,
        area: Option<FileAreaId>,
        search: &FileSearch,
    ) -> Result<Vec<FileEntry>, FileError>;
    fn new_file_checkpoint(&self, actor: FileActor) -> Result<Option<i64>, FileError>;
    fn record_new_file_check(&mut self, actor: FileActor, checked_at: i64)
        -> Result<(), FileError>;
    fn file_statistics(
        &self,
        actor: FileActor,
        checkpoint: Option<i64>,
    ) -> Result<FileStatistics, FileError>;
    fn file(
        &self,
        actor: FileActor,
        area: FileAreaId,
        filename: &str,
        require_download: bool,
    ) -> Result<FileEntry, FileError>;
    fn record_download(&mut self, actor: FileActor, file: FileId) -> Result<(), FileError>;
    fn commit_upload(
        &mut self,
        storage: &FileStorage,
        staged: StagedUpload,
        actor: FileActor,
        area: &FileArea,
        description: &str,
        uploaded_at: i64,
    ) -> Result<FileEntry, FileError>;
}

impl RuntimeDatabase {
    pub fn ensure_file_area(
        &mut self,
        definition: &FileAreaDefinition,
    ) -> Result<FileArea, FileError> {
        definition.validate()?;
        self.connection
            .execute(
                r#"
                INSERT INTO file_areas (
                    area_number, name, description, storage_key, access_mode,
                    read_security, upload_security, preview, no_charge,
                    maximum_upload_bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ON CONFLICT(area_number) DO NOTHING
                "#,
                params![
                    definition.number,
                    definition.name,
                    definition.description,
                    definition.storage_key,
                    definition.access_mode.as_database_value(),
                    definition.read_security.get(),
                    definition.upload_security.get(),
                    definition.preview,
                    definition.no_charge,
                    definition.maximum_upload_bytes as i64,
                ],
            )
            .map_err(FileError::Sqlite)?;
        let area = self
            .load_area_by_number_including_disabled(definition.number)?
            .ok_or(FileError::AreaNotFound(definition.number))?;
        self.ensure_primary_storage_root(&area)?;
        self.ensure_area_privileged_levels(area.id, &definition.privileged_security_levels)?;
        self.load_area_by_number_including_disabled(definition.number)?
            .ok_or(FileError::AreaNotFound(definition.number))
    }

    pub fn create_file_area(
        &mut self,
        definition: &FileAreaDefinition,
    ) -> Result<FileArea, FileError> {
        definition.validate()?;
        if self
            .load_area_by_number_including_disabled(definition.number)?
            .is_some()
        {
            return Err(FileError::AreaAlreadyExists(definition.number));
        }
        if self.storage_key_exists(&definition.storage_key)? {
            return Err(FileError::StorageKeyAlreadyExists(
                definition.storage_key.clone(),
            ));
        }
        let transaction = self.connection.transaction().map_err(FileError::Sqlite)?;
        transaction
            .execute(
                r#"
                INSERT INTO file_areas (
                    area_number, name, description, storage_key, access_mode,
                    read_security, upload_security, preview, no_charge,
                    maximum_upload_bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    definition.number,
                    definition.name,
                    definition.description,
                    definition.storage_key,
                    definition.access_mode.as_database_value(),
                    definition.read_security.get(),
                    definition.upload_security.get(),
                    definition.preview,
                    definition.no_charge,
                    definition.maximum_upload_bytes as i64,
                ],
            )
            .map_err(FileError::Sqlite)?;
        let id = FileAreaId::new(transaction.last_insert_rowid())?;
        replace_privileged_levels(&transaction, id, &definition.privileged_security_levels)?;
        transaction.commit().map_err(FileError::Sqlite)?;
        let area = self
            .load_area_by_number_including_disabled(definition.number)?
            .ok_or(FileError::AreaNotFound(definition.number))?;
        self.ensure_primary_storage_root(&area)?;
        Ok(area)
    }

    pub fn update_file_area(
        &mut self,
        existing_number: u16,
        definition: &FileAreaDefinition,
    ) -> Result<FileArea, FileError> {
        definition.validate()?;
        if existing_number != definition.number {
            return Err(FileError::AreaRenumberNotSupported);
        }
        let existing = self
            .load_area_by_number_including_disabled(existing_number)?
            .ok_or(FileError::AreaNotFound(existing_number))?;
        if existing.storage_key != definition.storage_key {
            return Err(FileError::StorageRelocationNotSupported);
        }
        let transaction = self.connection.transaction().map_err(FileError::Sqlite)?;
        transaction
            .execute(
                r#"
                UPDATE file_areas SET name = ?2, description = ?3,
                    access_mode = ?5, read_security = ?6, upload_security = ?7,
                    preview = ?8, no_charge = ?9, maximum_upload_bytes = ?10,
                    state_version = state_version + 1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE area_number = ?1
                "#,
                params![
                    definition.number,
                    definition.name,
                    definition.description,
                    definition.storage_key,
                    definition.access_mode.as_database_value(),
                    definition.read_security.get(),
                    definition.upload_security.get(),
                    definition.preview,
                    definition.no_charge,
                    definition.maximum_upload_bytes as i64,
                ],
            )
            .map_err(FileError::Sqlite)?;
        replace_privileged_levels(
            &transaction,
            existing.id,
            &definition.privileged_security_levels,
        )?;
        transaction.commit().map_err(FileError::Sqlite)?;
        self.load_area_by_number_including_disabled(existing_number)?
            .ok_or(FileError::AreaNotFound(existing_number))
    }

    pub fn set_file_area_enabled(&self, area_number: u16, enabled: bool) -> Result<(), FileError> {
        let changed = self
            .connection
            .execute(
                "UPDATE file_areas SET active = ?2, state_version = state_version + 1, updated_at = CURRENT_TIMESTAMP WHERE area_number = ?1",
                params![area_number, enabled],
            )
            .map_err(FileError::Sqlite)?;
        if changed == 0 {
            return Err(FileError::AreaNotFound(area_number));
        }
        Ok(())
    }

    pub fn all_file_areas(&self) -> Result<Vec<FileArea>, FileError> {
        let schema = self.schema_version()?;
        let sql = if schema >= 15 {
            "SELECT area_id, area_number, name, description, storage_key, access_mode, read_security, upload_security, preview, no_charge, maximum_upload_bytes, active, state_version FROM file_areas ORDER BY area_number"
        } else {
            "SELECT area_id, area_number, name, description, storage_key, access_mode, read_security, upload_security, preview, no_charge, maximum_upload_bytes, active FROM file_areas ORDER BY area_number"
        };
        let mut statement = self.connection.prepare(sql).map_err(FileError::Sqlite)?;
        let rows = statement
            .query_map([], |row| stored_area_at_schema(row, schema))
            .map_err(FileError::Sqlite)?;
        let mut areas = Vec::new();
        for row in rows {
            let mut area = row.map_err(FileError::Sqlite)?;
            area.privileged_security_levels = self.file_privileged_levels(area.id)?;
            areas.push(area);
        }
        Ok(areas)
    }

    /// Returns every catalog row, including disabled entries, in stable
    /// area/filename order for board-level preservation workflows.
    pub fn all_cataloged_files(&self) -> Result<Vec<(FileArea, FileEntry)>, FileError> {
        let schema = self.schema_version()?;
        let mut catalog = Vec::new();
        for area in self.all_file_areas()? {
            let files = if schema >= 15 {
                query_files(
                    &self.connection,
                    "SELECT file_id, area_id, filename, description, size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name, download_count, lifecycle, integrity_state, state_version, description_source, description_source_digest FROM files WHERE area_id = ?1 ORDER BY normalized_filename, file_id",
                    params![area.id.get()],
                )?
            } else {
                query_files_legacy(
                    &self.connection,
                    "SELECT file_id, area_id, filename, description, size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name, download_count, state FROM files WHERE area_id = ?1 ORDER BY normalized_filename, file_id",
                    params![area.id.get()],
                )?
            };
            catalog.extend(files.into_iter().map(|file| (area.clone(), file)));
        }
        Ok(catalog)
    }

    /// Returns only catalog entries whose authoritative schema-16 locator is
    /// in board-managed storage. External/read-only bytes remain referenced
    /// by the database but are deliberately not copied into a cold backup.
    pub fn managed_cataloged_files(&self) -> Result<Vec<(FileArea, FileEntry)>, FileError> {
        let catalog = self.all_cataloged_files()?;
        if self.schema_version()? < 16 {
            return Ok(catalog);
        }
        let mut managed = Vec::new();
        for (area, file) in catalog {
            let is_managed: bool = self
                .connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM file_storage_locators l JOIN file_storage_roots r ON r.storage_root_id=l.storage_root_id WHERE l.file_id=?1 AND r.root_kind='managed')",
                    params![file.id.get()],
                    |row| row.get(0),
                )
                .map_err(FileError::Sqlite)?;
            if is_managed {
                managed.push((area, file));
            }
        }
        Ok(managed)
    }

    pub fn file_count(&self, area: FileAreaId) -> Result<u64, FileError> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM files WHERE area_id = ?1 AND lifecycle = 'active'",
                params![area.get()],
                |row| row.get(0),
            )
            .map_err(FileError::Sqlite)?;
        stored_u64(count)
    }

    pub fn file_operations_ready_for_cold_backup(&self) -> Result<bool, FileError> {
        let count: i64 = self.connection.query_row(
            "SELECT (SELECT COUNT(*) FROM file_operations WHERE phase NOT IN ('committed','rolled-back')) + (SELECT COUNT(*) FROM file_active_uses WHERE expires_at>CURRENT_TIMESTAMP)",
            [],
            |row| row.get(0),
        ).map_err(FileError::Sqlite)?;
        Ok(count == 0)
    }

    pub fn insert_file_entry(&mut self, entry: &NewFileEntry) -> Result<FileEntry, FileError> {
        validate_new_file(entry)?;
        let normalized = normalize_filename(&entry.filename)?;
        let schema = self.schema_version()?;
        let transaction = self.connection.transaction().map_err(FileError::Sqlite)?;
        transaction
            .execute(
                r#"
                INSERT INTO files (
                    area_id, filename, normalized_filename, description, size_bytes,
                    sha256, uploaded_at, uploader_caller_id, uploader_name,
                    description_source, lifecycle, review_submitted_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                    CASE WHEN ?11='pending-review' THEN CURRENT_TIMESTAMP ELSE NULL END)
                "#,
                params![
                    entry.area_id.get(),
                    entry.filename,
                    normalized,
                    entry.description,
                    sqlite_i64(entry.size_bytes)?,
                    entry.sha256,
                    entry.uploaded_at,
                    entry.uploader_caller_id.map(CallerId::get),
                    entry.uploader_name,
                    if entry.uploader_caller_id.is_some() {
                        "caller"
                    } else {
                        "system"
                    },
                    entry.lifecycle.as_database_value(),
                ],
            )
            .map_err(|error| duplicate_file_error(error, &entry.filename))?;
        let id = FileId::new(transaction.last_insert_rowid())?;
        if schema >= 16 {
            let mapped = transaction
                .execute(
                    "INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path) SELECT ?1,storage_root_id,?2 FROM file_storage_roots WHERE area_id=?3 AND priority=0",
                    params![id.get(), entry.filename, entry.area_id.get()],
                )
                .map_err(FileError::Sqlite)?;
            if mapped != 1 {
                return Err(FileError::Maintenance(
                    "primary storage locator is unavailable".to_owned(),
                ));
            }
        }
        transaction
            .execute(
                "UPDATE file_areas SET state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE area_id=?1",
                params![entry.area_id.get()],
            )
            .map_err(FileError::Sqlite)?;
        if let Some(caller_id) = entry.uploader_caller_id {
            let changed = transaction
                .execute(
                    "UPDATE callers SET files_uploaded = files_uploaded + 1, upload_bytes = upload_bytes + ?2, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1 AND account_state = 'active'",
                    params![caller_id.get(), sqlite_i64(entry.size_bytes)?],
                )
                .map_err(FileError::Sqlite)?;
            if changed != 1 {
                return Err(FileError::CallerUnavailable);
            }
        }
        transaction.commit().map_err(FileError::Sqlite)?;
        self.load_file_by_id(id)?
            .ok_or(FileError::FileIdNotFound(id.get()))
    }

    fn ensure_primary_storage_root(&self, area: &FileArea) -> Result<(), FileError> {
        if self.schema_version()? < 16 {
            return Ok(());
        }
        self.connection
            .execute(
                "INSERT INTO file_storage_roots(area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy) VALUES(?1,?2,?3,'managed','read-write',0,?4,'enabled','available','direct-if-safe') ON CONFLICT(area_id,priority) DO NOTHING",
                params![area.id.get(), format!("area-{}-primary", area.id.get()), format!("{} primary", area.name), area.storage_key],
            )
            .map_err(FileError::Sqlite)?;
        Ok(())
    }

    fn ensure_area_privileged_levels(
        &mut self,
        area: FileAreaId,
        levels: &[SecurityLevel],
    ) -> Result<(), FileError> {
        let transaction = self.connection.transaction().map_err(FileError::Sqlite)?;
        for level in levels {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO file_area_privileged_security (area_id, security_level) VALUES (?1, ?2)",
                    params![area.get(), level.get()],
                )
                .map_err(FileError::Sqlite)?;
        }
        transaction.commit().map_err(FileError::Sqlite)
    }

    fn storage_key_exists(&self, key: &str) -> Result<bool, FileError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM file_areas WHERE storage_key = ?1)",
                params![key],
                |row| row.get(0),
            )
            .map_err(FileError::Sqlite)
    }

    fn file_privileged_levels(&self, area: FileAreaId) -> Result<Vec<SecurityLevel>, FileError> {
        let mut statement = self
            .connection
            .prepare("SELECT security_level FROM file_area_privileged_security WHERE area_id = ?1 ORDER BY security_level")
            .map_err(FileError::Sqlite)?;
        let rows = statement
            .query_map(params![area.get()], |row| row.get::<_, u16>(0))
            .map_err(FileError::Sqlite)?;
        rows.map(|row| {
            SecurityLevel::new(row.map_err(FileError::Sqlite)?).map_err(FileError::InvalidCaller)
        })
        .collect()
    }

    fn load_area_by_number_including_disabled(
        &self,
        number: u16,
    ) -> Result<Option<FileArea>, FileError> {
        let mut area = self
            .connection
            .query_row(
                "SELECT area_id, area_number, name, description, storage_key, access_mode, read_security, upload_security, preview, no_charge, maximum_upload_bytes, active, state_version FROM file_areas WHERE area_number = ?1",
                params![number],
                stored_area,
            )
            .optional()
            .map_err(FileError::Sqlite)?;
        if let Some(area) = &mut area {
            area.privileged_security_levels = self.file_privileged_levels(area.id)?;
        }
        Ok(area)
    }

    pub(crate) fn load_area_by_id(&self, id: FileAreaId) -> Result<Option<FileArea>, FileError> {
        let mut area = self
            .connection
            .query_row(
                "SELECT area_id, area_number, name, description, storage_key, access_mode, read_security, upload_security, preview, no_charge, maximum_upload_bytes, active, state_version FROM file_areas WHERE area_id = ?1",
                params![id.get()],
                stored_area,
            )
            .optional()
            .map_err(FileError::Sqlite)?;
        if let Some(area) = &mut area {
            area.privileged_security_levels = self.file_privileged_levels(area.id)?;
        }
        Ok(area)
    }

    pub(crate) fn load_file_by_id(&self, id: FileId) -> Result<Option<FileEntry>, FileError> {
        self.connection
            .query_row(
                "SELECT file_id, area_id, filename, description, size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name, download_count, lifecycle, integrity_state, state_version, description_source, description_source_digest FROM files WHERE file_id = ?1",
                params![id.get()],
                stored_file,
            )
            .optional()
            .map_err(FileError::Sqlite)
    }

    pub(crate) fn active_file_actor(&self, actor: FileActor) -> Result<Caller, FileError> {
        let caller = self
            .caller_by_id(actor.caller_id)?
            .ok_or(FileError::CallerUnavailable)?;
        if caller.state != CallerState::Active {
            return Err(FileError::CallerUnavailable);
        }
        Ok(caller)
    }

    pub(crate) fn authorized_area(
        &self,
        actor: FileActor,
        area: FileAreaId,
    ) -> Result<(Caller, FileArea, FileAccess), FileError> {
        let caller = self.active_file_actor(actor)?;
        let area = self
            .load_area_by_id(area)?
            .filter(|area| area.active)
            .ok_or(FileError::AreaIdNotFound(area.get()))?;
        let access = area
            .access(&caller, actor.sysop_security)
            .ok_or(FileError::AreaAccessDenied(area.number))?;
        Ok((caller, area, access))
    }
}

impl FileBackend for RuntimeDatabase {
    fn file_areas(&self, actor: FileActor) -> Result<Vec<(FileArea, FileAccess)>, FileError> {
        let caller = self.active_file_actor(actor)?;
        Ok(self
            .all_file_areas()?
            .into_iter()
            .filter(|area| area.active)
            .filter_map(|area| {
                area.access(&caller, actor.sysop_security)
                    .map(|access| (area, access))
            })
            .collect())
    }

    fn file_area(
        &self,
        actor: FileActor,
        area_number: u16,
    ) -> Result<(FileArea, FileAccess), FileError> {
        let area = self
            .load_area_by_number_including_disabled(area_number)?
            .filter(|area| area.active)
            .ok_or(FileError::AreaNotFound(area_number))?;
        let (_, area, access) = self.authorized_area(actor, area.id)?;
        Ok((area, access))
    }

    fn files(&self, actor: FileActor, area: FileAreaId) -> Result<Vec<FileEntry>, FileError> {
        self.authorized_area(actor, area)?;
        query_files(
            &self.connection,
            "SELECT file_id, area_id, filename, description, size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name, download_count, lifecycle, integrity_state, state_version, description_source, description_source_digest FROM files WHERE area_id = ?1 AND lifecycle IN ('active', 'offline') ORDER BY normalized_filename",
            params![area.get()],
        )
    }

    fn search_files(
        &self,
        actor: FileActor,
        selected_area: Option<FileAreaId>,
        search: &FileSearch,
    ) -> Result<Vec<FileEntry>, FileError> {
        validate_search(search)?;
        let visible = self.file_areas(actor)?;
        let mut results = Vec::new();
        for (area, _) in visible {
            if selected_area.is_some_and(|selected| selected != area.id) {
                continue;
            }
            for file in self.files(actor, area.id)? {
                let matches = match search {
                    FileSearch::Filename(pattern) => {
                        let pattern = normalize_search_pattern(pattern)?;
                        wildcard_matches(&pattern, &file.filename.to_ascii_uppercase())
                    }
                    FileSearch::Description(words) => {
                        let description = file.description.to_ascii_lowercase();
                        words
                            .iter()
                            .all(|word| description.contains(&word.to_ascii_lowercase()))
                    }
                    FileSearch::NewSince(timestamp) => file.uploaded_at >= *timestamp,
                };
                if matches {
                    results.push(file);
                }
            }
        }
        results.sort_by_key(|file| (file.uploaded_at, file.id));
        Ok(results)
    }

    fn new_file_checkpoint(&self, actor: FileActor) -> Result<Option<i64>, FileError> {
        self.active_file_actor(actor)?;
        self.connection
            .query_row(
                "SELECT last_files_checked_at FROM callers WHERE caller_id = ?1 AND account_state = 'active'",
                params![actor.caller_id().get()],
                |row| row.get(0),
            )
            .optional()
            .map_err(FileError::Sqlite)?
            .ok_or(FileError::CallerUnavailable)
    }

    fn record_new_file_check(
        &mut self,
        actor: FileActor,
        checked_at: i64,
    ) -> Result<(), FileError> {
        if checked_at < 0 {
            return Err(FileError::InvalidNewFileTimestamp(checked_at));
        }
        self.active_file_actor(actor)?;
        let changed = self
            .connection
            .execute(
                r#"
                UPDATE callers
                   SET last_files_checked_at = CASE
                           WHEN last_files_checked_at IS NULL OR last_files_checked_at < ?2
                           THEN ?2
                           ELSE last_files_checked_at
                       END,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE caller_id = ?1 AND account_state = 'active'
                "#,
                params![actor.caller_id().get(), checked_at],
            )
            .map_err(FileError::Sqlite)?;
        if changed != 1 {
            return Err(FileError::CallerUnavailable);
        }
        Ok(())
    }

    fn file_statistics(
        &self,
        actor: FileActor,
        checkpoint: Option<i64>,
    ) -> Result<FileStatistics, FileError> {
        if checkpoint.is_some_and(|timestamp| timestamp < 0) {
            return Err(FileError::InvalidNewFileTimestamp(
                checkpoint.unwrap_or_default(),
            ));
        }
        let mut statistics = FileStatistics::default();
        for (area, access) in self.file_areas(actor)? {
            if access != FileAccess::Full {
                continue;
            }
            for file in self.files(actor, area.id)? {
                if file.lifecycle != FileLifecycle::Active
                    || matches!(
                        file.integrity,
                        FileIntegrity::Missing | FileIntegrity::DigestMismatch
                    )
                {
                    continue;
                }
                statistics.available_files = statistics
                    .available_files
                    .checked_add(1)
                    .ok_or(FileError::FileStatisticsOverflow)?;
                statistics.available_bytes = statistics
                    .available_bytes
                    .checked_add(file.size_bytes)
                    .ok_or(FileError::FileStatisticsOverflow)?;
                if checkpoint.is_none_or(|timestamp| file.uploaded_at >= timestamp) {
                    statistics.new_since_checkpoint = statistics
                        .new_since_checkpoint
                        .checked_add(1)
                        .ok_or(FileError::FileStatisticsOverflow)?;
                }
            }
        }
        Ok(statistics)
    }

    fn file(
        &self,
        actor: FileActor,
        area: FileAreaId,
        filename: &str,
        require_download: bool,
    ) -> Result<FileEntry, FileError> {
        let (_, authorized_area, access) = self.authorized_area(actor, area)?;
        if require_download && access != FileAccess::Full {
            return Err(FileError::DownloadDenied(authorized_area.number));
        }
        let normalized = normalize_filename(filename)?;
        self.connection
            .query_row(
                "SELECT file_id, area_id, filename, description, size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name, download_count, lifecycle, integrity_state, state_version, description_source, description_source_digest FROM files WHERE area_id = ?1 AND normalized_filename = ?2 AND lifecycle = 'active' AND integrity_state NOT IN ('missing','digest-mismatch')",
                params![area.get(), normalized],
                stored_file,
            )
            .optional()
            .map_err(FileError::Sqlite)?
            .ok_or_else(|| FileError::FileNotFound(filename.to_owned()))
    }

    fn record_download(&mut self, actor: FileActor, file_id: FileId) -> Result<(), FileError> {
        let caller = self.active_file_actor(actor)?;
        let file = self
            .load_file_by_id(file_id)?
            .filter(|file| {
                file.lifecycle == FileLifecycle::Active
                    && !matches!(
                        file.integrity,
                        FileIntegrity::Missing | FileIntegrity::DigestMismatch
                    )
            })
            .ok_or(FileError::FileIdNotFound(file_id.get()))?;
        let area = self
            .load_area_by_id(file.area_id)?
            .filter(|area| area.active)
            .ok_or(FileError::AreaIdNotFound(file.area_id.get()))?;
        if area.access(&caller, actor.sysop_security) != Some(FileAccess::Full) {
            return Err(FileError::DownloadDenied(area.number));
        }
        let transaction = self.connection.transaction().map_err(FileError::Sqlite)?;
        if !area.no_charge {
            let changed = transaction
                .execute(
                    "UPDATE callers SET files_downloaded = files_downloaded + 1, download_bytes = download_bytes + ?2, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1 AND account_state = 'active'",
                    params![caller.id.get(), sqlite_i64(file.size_bytes)?],
                )
                .map_err(FileError::Sqlite)?;
            if changed != 1 {
                return Err(FileError::CallerUnavailable);
            }
        }
        transaction
            .execute(
                "UPDATE files SET download_count = download_count + 1, updated_at = CURRENT_TIMESTAMP WHERE file_id = ?1",
                params![file.id.get()],
            )
            .map_err(FileError::Sqlite)?;
        transaction.commit().map_err(FileError::Sqlite)
    }

    fn commit_upload(
        &mut self,
        storage: &FileStorage,
        staged: StagedUpload,
        actor: FileActor,
        area: &FileArea,
        description: &str,
        uploaded_at: i64,
    ) -> Result<FileEntry, FileError> {
        storage.commit_upload(staged, self, actor, area, description, uploaded_at)
    }
}

fn replace_privileged_levels(
    transaction: &rusqlite::Transaction<'_>,
    area: FileAreaId,
    levels: &[SecurityLevel],
) -> Result<(), FileError> {
    transaction
        .execute(
            "DELETE FROM file_area_privileged_security WHERE area_id = ?1",
            params![area.get()],
        )
        .map_err(FileError::Sqlite)?;
    for level in levels {
        transaction
            .execute(
                "INSERT INTO file_area_privileged_security (area_id, security_level) VALUES (?1, ?2)",
                params![area.get(), level.get()],
            )
            .map_err(FileError::Sqlite)?;
    }
    Ok(())
}

fn stored_area(row: &Row<'_>) -> rusqlite::Result<FileArea> {
    stored_area_at_schema(row, 15)
}

fn stored_area_at_schema(row: &Row<'_>, schema: u32) -> rusqlite::Result<FileArea> {
    let id = FileAreaId::new(row.get(0)?).map_err(sql_conversion)?;
    let mode =
        FileAccessMode::from_database_value(&row.get::<_, String>(5)?).map_err(sql_conversion)?;
    let read = SecurityLevel::new(row.get(6)?).map_err(sql_conversion)?;
    let upload = SecurityLevel::new(row.get(7)?).map_err(sql_conversion)?;
    let maximum: i64 = row.get(10)?;
    Ok(FileArea {
        id,
        number: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        storage_key: row.get(4)?,
        access_mode: mode,
        read_security: read,
        upload_security: upload,
        preview: row.get(8)?,
        no_charge: row.get(9)?,
        maximum_upload_bytes: u64::try_from(maximum).map_err(sql_conversion)?,
        privileged_security_levels: Vec::new(),
        active: row.get(11)?,
        state_version: if schema >= 15 {
            u64::try_from(row.get::<_, i64>(12)?).map_err(sql_conversion)?
        } else {
            1
        },
    })
}

fn stored_file(row: &Row<'_>) -> rusqlite::Result<FileEntry> {
    let size: i64 = row.get(4)?;
    let downloads: i64 = row.get(9)?;
    let uploader: Option<i64> = row.get(7)?;
    Ok(FileEntry {
        id: FileId::new(row.get(0)?).map_err(sql_conversion)?,
        area_id: FileAreaId::new(row.get(1)?).map_err(sql_conversion)?,
        filename: row.get(2)?,
        description: row.get(3)?,
        size_bytes: u64::try_from(size).map_err(sql_conversion)?,
        sha256: row.get(5)?,
        uploaded_at: row.get(6)?,
        uploader_caller_id: uploader
            .map(CallerId::new)
            .transpose()
            .map_err(sql_conversion)?,
        uploader_name: row.get(8)?,
        download_count: u64::try_from(downloads).map_err(sql_conversion)?,
        lifecycle: FileLifecycle::from_database_value(&row.get::<_, String>(10)?)
            .map_err(sql_conversion)?,
        integrity: FileIntegrity::from_database_value(&row.get::<_, String>(11)?)
            .map_err(sql_conversion)?,
        state_version: u64::try_from(row.get::<_, i64>(12)?).map_err(sql_conversion)?,
        description_source: row.get(13)?,
        description_source_digest: row.get(14)?,
    })
}

fn query_files<P: rusqlite::Params>(
    connection: &rusqlite::Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<FileEntry>, FileError> {
    let mut statement = connection.prepare(sql).map_err(FileError::Sqlite)?;
    let rows = statement
        .query_map(parameters, stored_file)
        .map_err(FileError::Sqlite)?;
    rows.map(|row| row.map_err(FileError::Sqlite)).collect()
}

fn query_files_legacy<P: rusqlite::Params>(
    connection: &rusqlite::Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<FileEntry>, FileError> {
    let mut statement = connection.prepare(sql).map_err(FileError::Sqlite)?;
    let rows = statement
        .query_map(parameters, |row| {
            let size: i64 = row.get(4)?;
            let downloads: i64 = row.get(9)?;
            let uploader: Option<i64> = row.get(7)?;
            Ok(FileEntry {
                id: FileId::new(row.get(0)?).map_err(sql_conversion)?,
                area_id: FileAreaId::new(row.get(1)?).map_err(sql_conversion)?,
                filename: row.get(2)?,
                description: row.get(3)?,
                size_bytes: u64::try_from(size).map_err(sql_conversion)?,
                sha256: row.get(5)?,
                uploaded_at: row.get(6)?,
                uploader_caller_id: uploader
                    .map(CallerId::new)
                    .transpose()
                    .map_err(sql_conversion)?,
                uploader_name: row.get(8)?,
                download_count: u64::try_from(downloads).map_err(sql_conversion)?,
                lifecycle: if row.get::<_, String>(10)? == "available" {
                    FileLifecycle::Active
                } else {
                    FileLifecycle::Disabled
                },
                integrity: FileIntegrity::Unknown,
                state_version: 1,
                description_source: "legacy-import".to_owned(),
                description_source_digest: None,
            })
        })
        .map_err(FileError::Sqlite)?;
    rows.map(|row| row.map_err(FileError::Sqlite)).collect()
}

fn validate_area(definition: &FileAreaDefinition) -> Result<(), FileError> {
    if definition.number == 0 {
        return Err(FileError::InvalidAreaNumber(definition.number));
    }
    if definition.name.trim().is_empty()
        || definition.name.len() > 60
        || !printable_ascii(&definition.name)
    {
        return Err(FileError::InvalidAreaName);
    }
    if definition.description.len() > 255 || !printable_ascii(&definition.description) {
        return Err(FileError::InvalidAreaDescription);
    }
    validate_storage_key(&definition.storage_key)?;
    if definition.maximum_upload_bytes == 0 || definition.maximum_upload_bytes > 1024 * 1024 * 1024
    {
        return Err(FileError::InvalidMaximumUploadBytes(
            definition.maximum_upload_bytes,
        ));
    }
    if definition.privileged_security_levels.len() > 5 {
        return Err(FileError::TooManyPrivilegedSecurityLevels);
    }
    let unique: std::collections::HashSet<_> =
        definition.privileged_security_levels.iter().collect();
    if unique.len() != definition.privileged_security_levels.len() {
        return Err(FileError::DuplicatePrivilegedSecurityLevel);
    }
    Ok(())
}

fn validate_new_file(entry: &NewFileEntry) -> Result<(), FileError> {
    normalize_filename(&entry.filename)?;
    if entry.description.trim().is_empty()
        || entry.description.len() > MAX_FILE_DESCRIPTION_BYTES
        || entry.description.lines().count() > MAX_FILE_DESCRIPTION_LINES
        || !safe_file_description(&entry.description)
    {
        return Err(FileError::InvalidDescription);
    }
    if entry.sha256.len() != 64 || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(FileError::InvalidSha256);
    }
    if entry.uploader_name.is_empty() || entry.uploader_name.len() > 60 {
        return Err(FileError::InvalidUploaderName);
    }
    Ok(())
}

fn printable_ascii(value: &str) -> bool {
    value.bytes().all(|byte| matches!(byte, b' '..=b'~'))
}

fn safe_file_description(value: &str) -> bool {
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' => {}
            '\r' if characters.next_if_eq(&'\n').is_some() => {}
            value if !value.is_control() => {}
            _ => return false,
        }
    }
    true
}

pub fn normalize_filename(filename: &str) -> Result<String, FileError> {
    if filename.is_empty() || filename.len() > MAX_FILE_NAME_BYTES {
        return Err(FileError::InvalidFilename(filename.to_owned()));
    }
    if filename == "."
        || filename == ".."
        || filename.starts_with('.')
        || !filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
        || Path::new(filename).components().count() != 1
    {
        return Err(FileError::InvalidFilename(filename.to_owned()));
    }
    Ok(filename.to_ascii_uppercase())
}

fn validate_storage_key(key: &str) -> Result<(), FileError> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || Path::new(key)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FileError::InvalidStorageKey(key.to_owned()));
    }
    Ok(())
}

fn validate_search(search: &FileSearch) -> Result<(), FileError> {
    match search {
        FileSearch::Filename(pattern) => {
            normalize_search_pattern(pattern)?;
        }
        FileSearch::Description(words) => {
            if words.is_empty()
                || words.len() > MAX_DESCRIPTION_SEARCH_WORDS
                || words.iter().any(|word| {
                    word.is_empty()
                        || word.len() > 32
                        || !word.bytes().all(|byte| byte.is_ascii_graphic())
                })
            {
                return Err(FileError::InvalidDescriptionSearch);
            }
        }
        FileSearch::NewSince(timestamp) if *timestamp < 0 => {
            return Err(FileError::InvalidNewFileTimestamp(*timestamp))
        }
        FileSearch::NewSince(_) => {}
    }
    Ok(())
}

fn normalize_search_pattern(pattern: &str) -> Result<String, FileError> {
    let trimmed = pattern.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("*.*")
        || trimmed.len() > MAX_FILE_NAME_BYTES
        || !trimmed.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'*' | b'?')
        })
    {
        return Err(FileError::InvalidSearchPattern(pattern.to_owned()));
    }
    let mut normalized = trimmed.to_ascii_uppercase();
    if !normalized.contains('.') {
        normalized.push_str(".*");
    }
    Ok(normalized)
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut p, mut v, mut star, mut retry) = (0, 0, None, 0);
    while v < value.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = v;
        } else if let Some(star_at) = star {
            p = star_at + 1;
            retry += 1;
            v = retry;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

fn duplicate_file_error(error: rusqlite::Error, filename: &str) -> FileError {
    if matches!(
        error,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        )
    ) {
        FileError::DuplicateFilename(filename.to_owned())
    } else {
        FileError::Sqlite(error)
    }
}

fn sql_conversion(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Null, Box::new(error))
}

fn stored_u64(value: i64) -> Result<u64, FileError> {
    u64::try_from(value).map_err(|_| FileError::InvalidStoredCounter(value))
}

fn sqlite_i64(value: u64) -> Result<i64, FileError> {
    i64::try_from(value).map_err(|_| FileError::CounterOverflow(value))
}

#[derive(Clone, Debug)]
pub struct FileStorage {
    files_root: PathBuf,
    staging_root: PathBuf,
}

/// A confined, seekable transfer source. External roots configured for
/// staging are copied into a delete-on-drop temporary file; managed roots are
/// streamed directly after integrity verification.
pub struct PreparedDownload {
    source: PreparedDownloadSource,
}

enum PreparedDownloadSource {
    Direct(File),
    Staged(tempfile::NamedTempFile),
}

impl Read for PreparedDownload {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.source {
            PreparedDownloadSource::Direct(file) => file.read(buffer),
            PreparedDownloadSource::Staged(file) => file.read(buffer),
        }
    }
}

impl Seek for PreparedDownload {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match &mut self.source {
            PreparedDownloadSource::Direct(file) => file.seek(position),
            PreparedDownloadSource::Staged(file) => file.seek(position),
        }
    }
}

impl FileStorage {
    pub fn new(paths: &LogicalPaths) -> Result<Self, FileError> {
        let files_root = paths.get(LogicalPath::External).join("files");
        let staging_root = paths.get(LogicalPath::Work).join("upload-staging");
        create_real_directory(&files_root)?;
        create_real_directory(&staging_root)?;
        Ok(Self {
            files_root: files_root
                .canonicalize()
                .map_err(|source| FileError::StorageIo {
                    path: files_root,
                    source,
                })?,
            staging_root: staging_root
                .canonicalize()
                .map_err(|source| FileError::StorageIo {
                    path: staging_root,
                    source,
                })?,
        })
    }

    /// Opens the trusted catalog byte root without creating or repairing any
    /// live-board directories. Cold backup uses this read-only construction;
    /// upload staging is deliberately outside the snapshot.
    pub fn open_existing(paths: &LogicalPaths) -> Result<Self, FileError> {
        let files_root = paths.get(LogicalPath::External).join("files");
        let metadata =
            fs::symlink_metadata(&files_root).map_err(|source| FileError::StorageIo {
                path: files_root.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(FileError::UnsafeStorageObject(files_root));
        }
        let files_root = files_root
            .canonicalize()
            .map_err(|source| FileError::StorageIo {
                path: files_root,
                source,
            })?;
        Ok(Self {
            files_root,
            staging_root: paths.get(LogicalPath::Work).join("upload-staging"),
        })
    }

    pub fn ensure_area(&self, area: &FileArea) -> Result<PathBuf, FileError> {
        validate_storage_key(&area.storage_key)?;
        let directory = self.files_root.join(&area.storage_key);
        create_real_directory(&directory)?;
        let canonical = directory
            .canonicalize()
            .map_err(|source| FileError::StorageIo {
                path: directory.clone(),
                source,
            })?;
        if !canonical.starts_with(&self.files_root) {
            return Err(FileError::StorageEscape(directory));
        }
        Ok(canonical)
    }

    /// Returns the current single confined writable authority. The ordered
    /// result is deliberately shaped for future read-only secondary roots;
    /// legacy FA<x>.TXT host paths are never accepted directly here.
    pub fn logical_roots(&self, area: &FileArea) -> Result<Vec<LogicalFileStorageRoot>, FileError> {
        validate_storage_key(&area.storage_key)?;
        Ok(vec![LogicalFileStorageRoot {
            logical_key: area.storage_key.clone(),
            order: 0,
            access: StorageRootAccess::ManagedReadWrite,
        }])
    }

    pub(crate) fn file_path(
        &self,
        area: &FileArea,
        file: &FileEntry,
    ) -> Result<PathBuf, FileError> {
        let directory = self.ensure_area(area)?;
        normalize_filename(&file.filename)?;
        Ok(directory.join(&file.filename))
    }

    pub(crate) fn confined_relative_path(&self, relative: &str) -> Result<PathBuf, FileError> {
        let relative = Path::new(relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(FileError::StorageEscape(relative.to_path_buf()));
        }
        Ok(self.files_root.join(relative))
    }

    pub fn write_seed_file(
        &self,
        database: &mut RuntimeDatabase,
        area: &FileArea,
        filename: &str,
        description: &str,
        bytes: &[u8],
        uploaded_at: i64,
    ) -> Result<FileEntry, FileError> {
        normalize_filename(filename)?;
        let directory = self.ensure_area(area)?;
        let path = directory.join(filename);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| FileError::StorageIo {
                path: path.clone(),
                source,
            })?;
        if let Err(source) = output.write_all(bytes).and_then(|()| output.sync_all()) {
            let _ = fs::remove_file(&path);
            return Err(FileError::StorageIo { path, source });
        }
        let entry = NewFileEntry {
            area_id: area.id,
            filename: filename.to_owned(),
            description: description.to_owned(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_bytes(bytes),
            uploaded_at,
            uploader_caller_id: None,
            uploader_name: "SPITFIRE NG".to_owned(),
            lifecycle: FileLifecycle::Active,
        };
        match database.insert_file_entry(&entry) {
            Ok(file) => Ok(file),
            Err(error) => {
                let _ = fs::remove_file(&path);
                Err(error)
            }
        }
    }

    pub fn open_download(&self, area: &FileArea, file: &FileEntry) -> Result<File, FileError> {
        let directory = self.ensure_area(area)?;
        let normalized = normalize_filename(&file.filename)?;
        if normalized != file.filename.to_ascii_uppercase() {
            return Err(FileError::InvalidFilename(file.filename.clone()));
        }
        let path = directory.join(&file.filename);
        let metadata = fs::symlink_metadata(&path).map_err(|source| FileError::StorageIo {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileError::UnsafeStorageObject(path));
        }
        let canonical = path.canonicalize().map_err(|source| FileError::StorageIo {
            path: path.clone(),
            source,
        })?;
        if !canonical.starts_with(&directory) {
            return Err(FileError::StorageEscape(path));
        }
        let mut input = File::open(&canonical).map_err(|source| FileError::StorageIo {
            path: canonical.clone(),
            source,
        })?;
        let (size, hash) = hash_reader(&mut input)?;
        if size != file.size_bytes || hash != file.sha256 {
            return Err(FileError::ContentMismatch(file.filename.clone()));
        }
        input
            .seek(SeekFrom::Start(0))
            .map_err(|source| FileError::StorageIo {
                path: canonical,
                source,
            })?;
        Ok(input)
    }

    /// Opens bytes through schema-16 logical storage authority. Caller input
    /// never participates in root or path selection.
    pub fn open_resolved_download(
        &self,
        root: &StorageRoot,
        locator: &crate::FileStorageLocator,
        file: &FileEntry,
    ) -> Result<File, FileError> {
        if locator.file_id != file.id
            || locator.storage_root_id != root.id
            || root.area_id != file.area_id
            || root.availability != StorageAvailability::Available
        {
            return Err(FileError::StorageUnavailable(file.filename.clone()));
        }
        let base = match root.kind {
            StorageRootKind::Managed => {
                validate_storage_key(&root.configured_locator)?;
                self.files_root.join(&root.configured_locator)
            }
            StorageRootKind::External => {
                let configured = PathBuf::from(&root.configured_locator);
                if !configured.is_absolute() {
                    return Err(FileError::StorageEscape(configured));
                }
                configured
            }
        };
        let base_metadata = fs::symlink_metadata(&base).map_err(|source| FileError::StorageIo {
            path: base.clone(),
            source,
        })?;
        if base_metadata.file_type().is_symlink() || !base_metadata.is_dir() {
            return Err(FileError::UnsafeStorageObject(base));
        }
        let canonical_base = base.canonicalize().map_err(|source| FileError::StorageIo {
            path: base.clone(),
            source,
        })?;
        let relative = Path::new(&locator.relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(FileError::StorageEscape(relative.to_path_buf()));
        }
        let path = canonical_base.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| FileError::StorageIo {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileError::UnsafeStorageObject(path));
        }
        let canonical = path.canonicalize().map_err(|source| FileError::StorageIo {
            path: path.clone(),
            source,
        })?;
        if !canonical.starts_with(&canonical_base) {
            return Err(FileError::StorageEscape(path));
        }
        let mut input = File::open(&canonical).map_err(|source| FileError::StorageIo {
            path: canonical.clone(),
            source,
        })?;
        let (size, hash) = hash_reader(&mut input)?;
        if size != file.size_bytes || hash != file.sha256 {
            return Err(FileError::ContentMismatch(file.filename.clone()));
        }
        input
            .seek(SeekFrom::Start(0))
            .map_err(|source| FileError::StorageIo {
                path: canonical,
                source,
            })?;
        Ok(input)
    }

    /// Prepares a bounded transfer source according to the authoritative root
    /// policy. The temporary filename and configured host root never enter
    /// caller-visible or audit data.
    pub fn prepare_resolved_download(
        &self,
        root: &StorageRoot,
        locator: &crate::FileStorageLocator,
        file: &FileEntry,
    ) -> Result<PreparedDownload, FileError> {
        let mut source = self.open_resolved_download(root, locator, file)?;
        if root.kind != StorageRootKind::External || !root.staging_always {
            return Ok(PreparedDownload {
                source: PreparedDownloadSource::Direct(source),
            });
        }
        create_real_directory(&self.staging_root)?;
        let mut staged = tempfile::Builder::new()
            .prefix(".download-")
            .tempfile_in(&self.staging_root)
            .map_err(|source| FileError::StorageIo {
                path: self.staging_root.clone(),
                source,
            })?;
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let read = source.read(&mut buffer).map_err(FileError::TransferIo)?;
            if read == 0 {
                break;
            }
            size = size
                .checked_add(read as u64)
                .ok_or(FileError::CounterOverflow(u64::MAX))?;
            digest.update(&buffer[..read]);
            staged
                .write_all(&buffer[..read])
                .map_err(FileError::TransferIo)?;
        }
        let sha256 = format!("{:x}", digest.finalize());
        if size != file.size_bytes || sha256 != file.sha256 {
            return Err(FileError::ContentMismatch(file.filename.clone()));
        }
        staged.as_file().sync_all().map_err(FileError::TransferIo)?;
        staged
            .seek(SeekFrom::Start(0))
            .map_err(FileError::TransferIo)?;
        Ok(PreparedDownload {
            source: PreparedDownloadSource::Staged(staged),
        })
    }

    /// Performs the filesystem half of an operator-directed storage-root
    /// probe.  The returned observation is not authoritative until the daemon
    /// records it through the versioned storage-root command.
    pub fn probe_storage_root(&self, root: &StorageRoot) -> StorageAvailability {
        let base = match root.kind {
            StorageRootKind::Managed => {
                if validate_storage_key(&root.configured_locator).is_err() {
                    return StorageAvailability::Unavailable;
                }
                self.files_root.join(&root.configured_locator)
            }
            StorageRootKind::External => {
                let configured = PathBuf::from(&root.configured_locator);
                if !configured.is_absolute() {
                    return StorageAvailability::Unavailable;
                }
                configured
            }
        };
        match fs::symlink_metadata(base) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                StorageAvailability::Available
            }
            _ => StorageAvailability::Unavailable,
        }
    }

    /// Opens a verified file only when the complete payload is suitable for
    /// the initial stock ASCII transfer. Validating before the first terminal
    /// write prevents a binary file from being partially emitted.
    pub fn open_ascii_download(
        &self,
        area: &FileArea,
        file: &FileEntry,
    ) -> Result<File, FileError> {
        let mut input = self.open_download(area, file)?;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let read = input.read(&mut buffer).map_err(FileError::TransferIo)?;
            if read == 0 {
                break;
            }
            if buffer[..read].iter().any(|byte| {
                *byte >= 0x7f || (*byte < 0x20 && !matches!(*byte, b'\r' | b'\n' | b'\t'))
            }) {
                return Err(FileError::NotAsciiText);
            }
        }
        input
            .seek(SeekFrom::Start(0))
            .map_err(FileError::TransferIo)?;
        Ok(input)
    }

    pub fn begin_upload(
        &self,
        session: SessionId,
        filename: &str,
    ) -> Result<StagedUpload, FileError> {
        let normalized = normalize_filename(filename)?;
        let session_directory = self.staging_root.join(format!("session-{}", session.get()));
        create_real_directory(&session_directory)?;
        let path = session_directory.join(format!("{normalized}.part"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    FileError::UploadAlreadyStaged(normalized.clone())
                } else {
                    FileError::StorageIo {
                        path: path.clone(),
                        source,
                    }
                }
            })?;
        Ok(StagedUpload {
            path,
            session_directory,
            filename: filename.to_owned(),
            file: Some(file),
            committed: false,
        })
    }

    pub fn commit_upload(
        &self,
        mut staged: StagedUpload,
        database: &mut RuntimeDatabase,
        actor: FileActor,
        area: &FileArea,
        description: &str,
        uploaded_at: i64,
    ) -> Result<FileEntry, FileError> {
        let (caller, current_area, _) = database.authorized_area(actor, area.id)?;
        if !current_area.allows_upload(&caller, actor.sysop_security) {
            return Err(FileError::UploadDenied(current_area.number));
        }
        if database
            .upload_is_denied(actor, &staged.filename)
            .map_err(|error| FileError::Maintenance(error.to_string()))?
        {
            return Err(FileError::UploadDeniedByPolicy);
        }
        let mut staged_file = staged.file.take().ok_or(FileError::StagingClosed)?;
        staged_file
            .flush()
            .and_then(|()| staged_file.sync_all())
            .map_err(|source| FileError::StorageIo {
                path: staged.path.clone(),
                source,
            })?;
        let (size, hash) = hash_reader(&mut staged_file)?;
        if size > current_area.maximum_upload_bytes {
            return Err(FileError::UploadTooLarge {
                actual: size,
                maximum: current_area.maximum_upload_bytes,
            });
        }
        let directory = self.ensure_area(&current_area)?;
        let destination = directory.join(&staged.filename);
        let pending_review = description.starts_with('/');
        let description = if pending_review {
            description.trim_start_matches('/').trim()
        } else {
            description
        };
        let description = database
            .normalize_upload_description(description)
            .map_err(|error| FileError::Maintenance(error.to_string()))?;
        let normalized_filename = normalize_filename(&staged.filename)?;
        let operation_id = format!(
            "upload-{}-{}-{}",
            caller.id.get(),
            current_area.id.get(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let operation = database
            .connection
            .transaction()
            .map_err(FileError::Sqlite)?;
        operation
            .execute(
                "DELETE FROM file_operation_leases WHERE expires_at<=CURRENT_TIMESTAMP",
                [],
            )
            .map_err(FileError::Sqlite)?;
        operation
            .execute(
                "INSERT INTO file_operations(operation_id,kind,destination_area_id,expected_area_version,phase,staging_path,digest,actor_caller_id) VALUES(?1,'upload',?2,?3,'staged',?4,?5,?6)",
                params![operation_id, current_area.id.get(), current_area.state_version as i64, format!("{}/{}", current_area.storage_key, staged.filename), hash, caller.id.get()],
            )
            .map_err(FileError::Sqlite)?;
        operation
            .execute(
                "INSERT INTO file_operation_leases(lease_kind,area_id,normalized_filename,operation_id,expires_at) VALUES('name',?1,?2,?3,datetime('now','+5 minutes'))",
                params![current_area.id.get(), normalized_filename, operation_id],
            )
            .map_err(|_| FileError::DuplicateFilename(staged.filename.clone()))?;
        operation.commit().map_err(FileError::Sqlite)?;
        let publish_result = (|| {
            let mut source = File::open(&staged.path).map_err(|source| FileError::StorageIo {
                path: staged.path.clone(),
                source,
            })?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|source| {
                    if source.kind() == std::io::ErrorKind::AlreadyExists {
                        FileError::DuplicateFilename(staged.filename.clone())
                    } else {
                        FileError::StorageIo {
                            path: destination.clone(),
                            source,
                        }
                    }
                })?;
            std::io::copy(&mut source, &mut output)
                .and_then(|_| output.sync_all())
                .map_err(|source| FileError::StorageIo {
                    path: destination.clone(),
                    source,
                })?;
            Ok::<(), FileError>(())
        })();
        if let Err(error) = publish_result {
            let _ = fs::remove_file(&destination);
            let _ = database.connection.execute(
                "UPDATE file_operations SET phase='rolled-back',error_code='byte-publish-failed',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
                params![operation_id],
            );
            let _ = database.connection.execute(
                "DELETE FROM file_operation_leases WHERE operation_id=?1",
                params![operation_id],
            );
            return Err(error);
        }
        let entry = NewFileEntry {
            area_id: current_area.id,
            filename: staged.filename.clone(),
            description,
            size_bytes: size,
            sha256: hash,
            uploaded_at,
            uploader_caller_id: Some(caller.id),
            uploader_name: caller.display_name,
            lifecycle: if pending_review {
                FileLifecycle::PendingReview
            } else {
                FileLifecycle::Active
            },
        };
        database
            .connection
            .execute(
                "UPDATE file_operations SET phase='bytes-published',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
                params![operation_id],
            )
            .map_err(FileError::Sqlite)?;
        let result = database.insert_file_entry(&entry);
        if result.is_err() {
            let _ = fs::remove_file(&destination);
            let _ = database.connection.execute(
                "UPDATE file_operations SET phase='rolled-back',error_code='catalog-insert-failed',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
                params![operation_id],
            );
            let _ = database.connection.execute(
                "DELETE FROM file_operation_leases WHERE operation_id=?1",
                params![operation_id],
            );
        }
        let file = result?;
        let completion = database
            .connection
            .transaction()
            .map_err(FileError::Sqlite)?;
        completion
            .execute(
                "UPDATE file_operations SET file_id=?2,phase='committed',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
                params![operation_id, file.id.get()],
            )
            .map_err(FileError::Sqlite)?;
        completion
            .execute(
                "DELETE FROM file_operation_leases WHERE operation_id=?1",
                params![operation_id],
            )
            .map_err(FileError::Sqlite)?;
        completion
            .execute(
                "INSERT INTO file_events(operation,actor_caller_id,file_id,area_id,operation_id,digest,detail) VALUES('file-added',?1,?2,?3,?4,?5,?6)",
                params![caller.id.get(), file.id.get(), current_area.id.get(), operation_id, file.sha256, if pending_review { "pending-review" } else { "caller-upload" }],
            )
            .map_err(FileError::Sqlite)?;
        completion.commit().map_err(FileError::Sqlite)?;
        staged.committed = true;
        let _ = fs::remove_file(&staged.path);
        let _ = fs::remove_dir(&staged.session_directory);
        Ok(file)
    }

    /// Opens a resolved download only when every byte is evidence-supported
    /// seven-bit text and therefore safe for the bounded ASCII engine.
    pub fn open_resolved_ascii_download(
        &self,
        root: &crate::StorageRoot,
        locator: &crate::FileStorageLocator,
        entry: &FileEntry,
    ) -> Result<PreparedDownload, FileError> {
        let mut source = self.prepare_resolved_download(root, locator, entry)?;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let read = source.read(&mut buffer).map_err(FileError::TransferIo)?;
            if read == 0 {
                break;
            }
            if buffer[..read].iter().any(|byte| {
                *byte >= 0x7f || (*byte < 0x20 && !matches!(*byte, b'\r' | b'\n' | b'\t'))
            }) {
                return Err(FileError::NotAsciiText);
            }
        }
        source
            .seek(SeekFrom::Start(0))
            .map_err(FileError::TransferIo)?;
        Ok(source)
    }
}

fn create_real_directory(path: &Path) -> Result<(), FileError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(FileError::UnsafeStorageObject(path.to_path_buf()));
        }
        return Ok(());
    }
    fs::create_dir_all(path).map_err(|source| FileError::StorageIo {
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|source| FileError::StorageIo {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FileError::UnsafeStorageObject(path.to_path_buf()));
    }
    Ok(())
}

pub struct StagedUpload {
    path: PathBuf,
    session_directory: PathBuf,
    filename: String,
    file: Option<File>,
    committed: bool,
}

impl StagedUpload {
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), FileError> {
        self.file
            .as_mut()
            .ok_or(FileError::StagingClosed)?
            .write_all(bytes)
            .map_err(|source| FileError::StorageIo {
                path: self.path.clone(),
                source,
            })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedUpload {
    fn drop(&mut self) {
        if !self.committed {
            self.file.take();
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_dir(&self.session_directory);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferDirection {
    Download,
    Upload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferReport {
    pub direction: TransferDirection,
    pub bytes: u64,
    pub completed: bool,
}

pub trait FileTransfer {
    fn download(
        &mut self,
        terminal: &mut dyn Terminal,
        input: &mut dyn Read,
    ) -> Result<TransferReport, FileError>;

    fn upload(
        &mut self,
        terminal: &mut dyn Terminal,
        staged: &mut StagedUpload,
        maximum_bytes: u64,
    ) -> Result<TransferReport, FileError>;
}

/// Stock SPITFIRE documents ASCII as an internal transfer protocol. This
/// bounded implementation intentionally handles text files only; binary
/// X/Y/ZMODEM and Telink remain explicit follow-up work.
#[derive(Default)]
pub struct AsciiTransfer;

impl FileTransfer for AsciiTransfer {
    fn download(
        &mut self,
        terminal: &mut dyn Terminal,
        input: &mut dyn Read,
    ) -> Result<TransferReport, FileError> {
        let mut total = 0_u64;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let read = input.read(&mut buffer).map_err(FileError::TransferIo)?;
            if read == 0 {
                break;
            }
            if buffer[..read]
                .iter()
                .any(|byte| !byte.is_ascii() || *byte == 0)
            {
                return Err(FileError::NotAsciiText);
            }
            terminal.write_all(&buffer[..read])?;
            total = total.saturating_add(read as u64);
        }
        Ok(TransferReport {
            direction: TransferDirection::Download,
            bytes: total,
            completed: true,
        })
    }

    fn upload(
        &mut self,
        terminal: &mut dyn Terminal,
        staged: &mut StagedUpload,
        maximum_bytes: u64,
    ) -> Result<TransferReport, FileError> {
        let mut total = 0_u64;
        loop {
            let Some(line) = terminal.read_line(255)? else {
                return Ok(TransferReport {
                    direction: TransferDirection::Upload,
                    bytes: total,
                    completed: false,
                });
            };
            if line.eq_ignore_ascii_case(b"/A") {
                return Ok(TransferReport {
                    direction: TransferDirection::Upload,
                    bytes: total,
                    completed: false,
                });
            }
            if line.eq_ignore_ascii_case(b"/S") {
                return Ok(TransferReport {
                    direction: TransferDirection::Upload,
                    bytes: total,
                    completed: true,
                });
            }
            if line.iter().any(|byte| !byte.is_ascii() || *byte == 0) {
                return Err(FileError::NotAsciiText);
            }
            let added = (line.len() + 2) as u64;
            if total.saturating_add(added) > maximum_bytes {
                return Err(FileError::UploadTooLarge {
                    actual: total.saturating_add(added),
                    maximum: maximum_bytes,
                });
            }
            staged.write_all(&line)?;
            staged.write_all(b"\r\n")?;
            total += added;
        }
    }
}

fn hash_reader(input: &mut File) -> Result<(u64, String), FileError> {
    input
        .seek(SeekFrom::Start(0))
        .map_err(FileError::TransferIo)?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = input.read(&mut buffer).map_err(FileError::TransferIo)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        size = size.saturating_add(read as u64);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("file-area identifier must be positive, got {0}")]
    InvalidAreaId(i64),
    #[error("file identifier must be positive, got {0}")]
    InvalidFileId(i64),
    #[error("file-area number must be in 1..={MAX_FILE_AREAS}, got {0}")]
    InvalidAreaNumber(u16),
    #[error("file-area name must contain 1..=60 bytes")]
    InvalidAreaName,
    #[error("file-area description must contain at most 255 bytes")]
    InvalidAreaDescription,
    #[error("file-area storage key is unsafe: {0:?}")]
    InvalidStorageKey(String),
    #[error("maximum upload size must be in 1..=1073741824 bytes, got {0}")]
    InvalidMaximumUploadBytes(u64),
    #[error("a file area may contain at most five privileged security levels")]
    TooManyPrivilegedSecurityLevels,
    #[error("file-area privileged security levels must be unique")]
    DuplicatePrivilegedSecurityLevel,
    #[error("file area {0} already exists")]
    AreaAlreadyExists(u16),
    #[error("file-area storage key already exists: {0:?}")]
    StorageKeyAlreadyExists(String),
    #[error("file-area renumbering is not supported; preserve stable identity")]
    AreaRenumberNotSupported,
    #[error("file-area storage relocation requires a dedicated safe move workflow")]
    StorageRelocationNotSupported,
    #[error("file area {0} does not exist")]
    AreaNotFound(u16),
    #[error("file-area identifier {0} does not exist")]
    AreaIdNotFound(i64),
    #[error("access to file area {0} is denied")]
    AreaAccessDenied(u16),
    #[error("downloads from file area {0} are denied")]
    DownloadDenied(u16),
    #[error("uploads to file area {0} are denied")]
    UploadDenied(u16),
    #[error("upload filename is denied by board policy")]
    UploadDeniedByPolicy,
    #[error("unsafe filename: {0:?}")]
    InvalidFilename(String),
    #[error("invalid file description")]
    InvalidDescription,
    #[error("invalid uploader name")]
    InvalidUploaderName,
    #[error("invalid SHA-256 value")]
    InvalidSha256,
    #[error("file {0:?} already exists in the selected area")]
    DuplicateFilename(String),
    #[error("file {0:?} does not exist in the selected area")]
    FileNotFound(String),
    #[error("file identifier {0} does not exist")]
    FileIdNotFound(i64),
    #[error("caller account is unavailable")]
    CallerUnavailable,
    #[error("invalid filename search pattern: {0:?}")]
    InvalidSearchPattern(String),
    #[error("description search requires one to six printable words")]
    InvalidDescriptionSearch,
    #[error("new-file search timestamp must be nonnegative, got {0}")]
    InvalidNewFileTimestamp(i64),
    #[error("database contains unknown file-area access mode {0:?}")]
    InvalidStoredAccessMode(String),
    #[error("database contains unknown file lifecycle {0:?}")]
    InvalidStoredLifecycle(String),
    #[error("database contains unknown file integrity state {0:?}")]
    InvalidStoredIntegrity(String),
    #[error("database contains an invalid negative or oversized counter {0}")]
    InvalidStoredCounter(i64),
    #[error("counter {0} is too large for SQLite")]
    CounterOverflow(u64),
    #[error("authorized file statistics exceed the supported range")]
    FileStatisticsOverflow,
    #[error("upload contains {actual} bytes; maximum is {maximum}")]
    UploadTooLarge { actual: u64, maximum: u64 },
    #[error("upload for {0:?} is already staged in this session")]
    UploadAlreadyStaged(String),
    #[error("upload staging file is already closed")]
    StagingClosed,
    #[error("ASCII transfer accepts only non-NUL 7-bit text")]
    NotAsciiText,
    #[error("stored file content does not match catalog size/hash: {0:?}")]
    ContentMismatch(String),
    #[error("the storage source for file {0:?} is unavailable")]
    StorageUnavailable(String),
    #[error("unsafe symlink or non-file storage object: {0}")]
    UnsafeStorageObject(PathBuf),
    #[error("file storage path escaped its configured root: {0}")]
    StorageEscape(PathBuf),
    #[error("file storage operation failed for {path}: {source}")]
    StorageIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("file transfer I/O failed: {0}")]
    TransferIo(#[source] std::io::Error),
    #[error(transparent)]
    Terminal(#[from] TerminalError),
    #[error(transparent)]
    InvalidCaller(#[from] CallerError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite file operation failed: {0}")]
    Sqlite(#[source] rusqlite::Error),
    #[error("file-maintenance policy failed: {0}")]
    Maintenance(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallerState, CredentialHasher, PasswordHashConfig, RuntimeConfig};
    use std::sync::{Arc, Barrier};

    fn test_board() -> (
        tempfile::TempDir,
        RuntimeDatabase,
        FileStorage,
        FileActor,
        CallerId,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &config).unwrap();
        paths.create_directories().unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        database.migrate().unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"test-only file password").unwrap();
        let caller = database
            .create_caller(
                b"File Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let actor = FileActor::new(caller.id, SecurityLevel::new(50).unwrap());
        let storage = FileStorage::new(&paths).unwrap();
        (temp, database, storage, actor, caller.id)
    }

    fn area(number: u16, key: &str, security: u16) -> FileAreaDefinition {
        FileAreaDefinition {
            number,
            name: format!("Area {number}"),
            description: "Synthetic file area".to_owned(),
            storage_key: key.to_owned(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(security).unwrap(),
            upload_security: SecurityLevel::new(security).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: Vec::new(),
        }
    }

    #[test]
    fn areas_files_listing_search_and_accounting_are_persistent() {
        let (_temp, mut database, storage, actor, caller_id) = test_board();
        let public = database.create_file_area(&area(1, "general", 5)).unwrap();
        let restricted = database
            .create_file_area(&area(2, "restricted", 50))
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &public,
                "HELLO.TXT",
                "Useful fixture greeting",
                b"hello\r\n",
                200,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &restricted,
                "SECRET.TXT",
                "restricted metadata",
                b"secret\r\n",
                201,
            )
            .unwrap();
        assert_eq!(database.file_areas(actor).unwrap().len(), 1);
        assert_eq!(database.files(actor, public.id).unwrap().len(), 1);
        assert_eq!(
            database
                .search_files(actor, None, &FileSearch::Filename("HELLO".to_owned()))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            database
                .search_files(
                    actor,
                    None,
                    &FileSearch::Description(vec!["fixture".to_owned(), "greeting".to_owned()])
                )
                .unwrap()
                .len(),
            1
        );
        assert!(database
            .search_files(
                actor,
                None,
                &FileSearch::Description(vec!["restricted".to_owned()])
            )
            .unwrap()
            .is_empty());
        let file = database.file(actor, public.id, "hello.txt", true).unwrap();
        database.record_download(actor, file.id).unwrap();
        let caller = database.caller_by_id(caller_id).unwrap().unwrap();
        assert_eq!(caller.files_downloaded, 1);
        assert_eq!(caller.download_bytes, 7);
    }

    #[test]
    fn stock_filename_and_description_search_matrix_is_bounded_and_authorized() {
        let (_temp, mut database, storage, actor, _caller_id) = test_board();
        let first = database
            .create_file_area(&area(1, "first-search", 5))
            .unwrap();
        let second = database
            .create_file_area(&area(2, "second-search", 5))
            .unwrap();
        let restricted = database
            .create_file_area(&area(3, "restricted-search", 50))
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &first,
                "SF351.ZIP",
                "SPITFIRE release archive with utilities documentation",
                b"first",
                100,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &second,
                "SF370.TXT",
                "SPITFIRE release notes for version thirty seven",
                b"second",
                200,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &restricted,
                "SFSECRET.ZIP",
                "SPITFIRE release archive with utilities documentation",
                b"secret",
                300,
            )
            .unwrap();

        let names = |files: Vec<FileEntry>| {
            files
                .into_iter()
                .map(|file| file.filename)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(
                database
                    .search_files(actor, None, &FileSearch::Filename("sf35?.zip".to_owned()),)
                    .unwrap(),
            ),
            vec!["SF351.ZIP"]
        );
        assert_eq!(
            names(
                database
                    .search_files(actor, None, &FileSearch::Filename("SF351".to_owned()),)
                    .unwrap(),
            ),
            vec!["SF351.ZIP"]
        );
        assert_eq!(
            names(
                database
                    .search_files(
                        actor,
                        Some(second.id),
                        &FileSearch::Filename("SF*".to_owned()),
                    )
                    .unwrap(),
            ),
            vec!["SF370.TXT"]
        );
        assert!(matches!(
            database.search_files(actor, None, &FileSearch::Filename("*.*".to_owned()),),
            Err(FileError::InvalidSearchPattern(_))
        ));
        assert_eq!(
            names(
                database
                    .search_files(
                        actor,
                        None,
                        &FileSearch::Description(vec![
                            "spitfire".to_owned(),
                            "release".to_owned(),
                            "archive".to_owned(),
                            "with".to_owned(),
                            "utilities".to_owned(),
                            "documentation".to_owned(),
                        ]),
                    )
                    .unwrap(),
            ),
            vec!["SF351.ZIP"]
        );
        assert!(matches!(
            database.search_files(
                actor,
                None,
                &FileSearch::Description(vec!["word".to_owned(); 7]),
            ),
            Err(FileError::InvalidDescriptionSearch)
        ));
    }

    #[test]
    fn new_file_checkpoint_statistics_privacy_and_multiline_descriptions_persist() {
        let (_temp, mut database, storage, actor, _caller_id) = test_board();
        let full = database.create_file_area(&area(1, "full", 5)).unwrap();
        let mut preview_definition = area(2, "preview-stats", 50);
        preview_definition.preview = true;
        let preview = database.create_file_area(&preview_definition).unwrap();
        let restricted = database
            .create_file_area(&area(3, "restricted-stats", 50))
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &full,
                "PUBLIC.TXT",
                "First stock line\r\nExtended description line",
                b"one",
                100,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &preview,
                "PREVIEW.TXT",
                "Visible but not downloadable",
                b"two",
                200,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &restricted,
                "SECRET.TXT",
                "Not visible",
                b"hidden",
                300,
            )
            .unwrap();

        assert_eq!(database.new_file_checkpoint(actor).unwrap(), None);
        assert_eq!(
            database.file_statistics(actor, Some(150)).unwrap(),
            FileStatistics {
                new_since_checkpoint: 0,
                available_files: 1,
                available_bytes: 3,
            }
        );
        assert_eq!(
            database
                .search_files(actor, None, &FileSearch::NewSince(0))
                .unwrap()
                .iter()
                .map(|file| file.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["PUBLIC.TXT", "PREVIEW.TXT"]
        );

        let other = database
            .create_caller(
                b"Other File Caller",
                "test-only-stored-hash",
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let other_actor = FileActor::new(other.id, SecurityLevel::new(50).unwrap());
        database.record_new_file_check(actor, 250).unwrap();
        database.record_new_file_check(actor, 200).unwrap();
        assert_eq!(database.new_file_checkpoint(actor).unwrap(), Some(250));
        assert_eq!(database.new_file_checkpoint(other_actor).unwrap(), None);

        let path = database.path().to_owned();
        let mut second_node = RuntimeDatabase::open(&path).unwrap();
        second_node.record_new_file_check(actor, 300).unwrap();
        database.record_new_file_check(actor, 275).unwrap();
        second_node.record_new_file_check(other_actor, 180).unwrap();
        assert_eq!(database.new_file_checkpoint(actor).unwrap(), Some(300));
        assert_eq!(
            database.new_file_checkpoint(other_actor).unwrap(),
            Some(180)
        );
        drop(second_node);
        drop(database);
        let reopened = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(reopened.new_file_checkpoint(actor).unwrap(), Some(300));
        assert_eq!(
            reopened.new_file_checkpoint(other_actor).unwrap(),
            Some(180)
        );
        assert_eq!(
            reopened.files(actor, full.id).unwrap()[0].description,
            "First stock line\r\nExtended description line"
        );
    }

    #[test]
    fn preview_security_lists_but_cannot_download_or_upload() {
        let (_temp, mut database, storage, actor, _caller) = test_board();
        let mut definition = area(1, "preview", 50);
        definition.preview = true;
        let preview = database.create_file_area(&definition).unwrap();
        storage
            .write_seed_file(
                &mut database,
                &preview,
                "VIEW.TXT",
                "Previewable",
                b"view\r\n",
                200,
            )
            .unwrap();
        assert_eq!(database.file_area(actor, 1).unwrap().1, FileAccess::Preview);
        assert!(matches!(
            database.file(actor, preview.id, "VIEW.TXT", true),
            Err(FileError::DownloadDenied(1))
        ));
        let mut staged = storage
            .begin_upload(SessionId::new(1).unwrap(), "NEW.TXT")
            .unwrap();
        staged.write_all(b"new\r\n").unwrap();
        assert!(matches!(
            storage.commit_upload(staged, &mut database, actor, &preview, "New", 300),
            Err(FileError::UploadDenied(1))
        ));
    }

    #[test]
    fn staging_commit_is_bounded_hashed_and_duplicate_safe() {
        let (_temp, mut database, storage, actor, caller_id) = test_board();
        let area = database.create_file_area(&area(1, "uploads", 5)).unwrap();
        let mut staged = storage
            .begin_upload(SessionId::new(8).unwrap(), "UPLOAD.TXT")
            .unwrap();
        staged.write_all(b"uploaded\r\n").unwrap();
        let staging_path = staged.path().to_path_buf();
        let entry = storage
            .commit_upload(staged, &mut database, actor, &area, "Uploaded text", 300)
            .unwrap();
        assert!(!staging_path.exists());
        assert_eq!(entry.size_bytes, 10);
        assert_eq!(entry.sha256, sha256_bytes(b"uploaded\r\n"));
        let caller = database.caller_by_id(caller_id).unwrap().unwrap();
        assert_eq!(caller.files_uploaded, 1);
        assert_eq!(caller.upload_bytes, 10);

        let mut duplicate = storage
            .begin_upload(SessionId::new(9).unwrap(), "UPLOAD.TXT")
            .unwrap();
        duplicate.write_all(b"replacement\r\n").unwrap();
        assert!(matches!(
            storage.commit_upload(duplicate, &mut database, actor, &area, "Duplicate", 301),
            Err(FileError::DuplicateFilename(_))
        ));
        assert_eq!(database.file_count(area.id).unwrap(), 1);
    }

    #[test]
    fn zero_byte_files_are_valid_catalog_and_upload_objects() {
        let (_temp, mut database, storage, actor, _caller_id) = test_board();
        let area = database.create_file_area(&area(1, "empty", 5)).unwrap();
        let seeded = storage
            .write_seed_file(
                &mut database,
                &area,
                "EMPTY.BIN",
                "Valid empty file",
                b"",
                300,
            )
            .unwrap();
        assert_eq!(seeded.size_bytes, 0);
        assert_eq!(
            seeded.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            storage
                .open_download(&area, &seeded)
                .unwrap()
                .metadata()
                .unwrap()
                .len(),
            0
        );

        let staged = storage
            .begin_upload(SessionId::new(12).unwrap(), "EMPTYUP.BIN")
            .unwrap();
        let uploaded = storage
            .commit_upload(
                staged,
                &mut database,
                actor,
                &area,
                "Uploaded empty file",
                301,
            )
            .unwrap();
        assert_eq!(uploaded.size_bytes, 0);
        assert_eq!(uploaded.sha256, seeded.sha256);
        assert_eq!(database.file_count(area.id).unwrap(), 2);
    }

    #[test]
    fn external_transfer_staging_is_bounded_verified_and_survives_media_loss() {
        let (temp, mut database, storage, actor, _caller_id) = test_board();
        database
            .connection
            .execute(
                "UPDATE callers SET security_level=50 WHERE caller_id=?1",
                params![actor.caller_id().get()],
            )
            .unwrap();
        let area = database.create_file_area(&area(1, "external", 5)).unwrap();
        let bytes = (0_u8..=255)
            .cycle()
            .take(4 * 1024 * 1024 + 37)
            .collect::<Vec<_>>();
        let entry = storage
            .write_seed_file(
                &mut database,
                &area,
                "LARGE.BIN",
                "Generated large source",
                &bytes,
                300,
            )
            .unwrap();
        let external = temp.path().join("read-only-media");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("LARGE.BIN"), &bytes).unwrap();
        let root = database
            .add_storage_root(
                actor,
                crate::StorageRootDefinition {
                    area_id: area.id,
                    stable_key: "large-media",
                    label: "Large media",
                    configured_locator: external.to_str().unwrap(),
                    priority: 1,
                    mode: crate::StorageRootMode::ReadOnly,
                    occurred_at: 301,
                },
            )
            .unwrap();
        database
            .set_storage_availability(
                actor,
                root.id,
                root.state_version,
                StorageAvailability::Available,
                302,
            )
            .unwrap();
        database
            .set_file_storage_locator(
                actor,
                entry.id,
                root.id,
                "LARGE.BIN",
                entry.state_version,
                1,
                303,
            )
            .unwrap();
        let (root, locator) = database.resolve_file_storage(entry.id).unwrap();
        let mut prepared = storage
            .prepare_resolved_download(&root, &locator, &entry)
            .unwrap();
        fs::remove_file(external.join("LARGE.BIN")).unwrap();
        let mut received = Vec::new();
        prepared.read_to_end(&mut received).unwrap();
        assert_eq!(received, bytes);
        drop(prepared);
        assert_eq!(fs::read_dir(&storage.staging_root).unwrap().count(), 0);
        assert_ne!(
            database
                .load_file_by_id(entry.id)
                .unwrap()
                .unwrap()
                .integrity,
            FileIntegrity::Missing
        );
    }

    #[test]
    fn canceled_staging_cleans_up_and_filename_traversal_is_rejected() {
        let (_temp, _database, storage, _actor, _caller) = test_board();
        assert!(normalize_filename("../escape.txt").is_err());
        assert!(normalize_filename("/absolute.txt").is_err());
        assert!(normalize_filename("dir/file.txt").is_err());
        let path = {
            let mut staged = storage
                .begin_upload(SessionId::new(4).unwrap(), "CANCEL.TXT")
                .unwrap();
            staged.write_all(b"partial").unwrap();
            staged.path().to_path_buf()
        };
        assert!(!path.exists());
    }

    #[test]
    fn terminal_control_metadata_is_rejected_before_storage() {
        let (_temp, mut database, _storage, _actor, _caller) = test_board();
        let mut invalid_area = area(1, "controls", 5);
        invalid_area.name = "Unsafe\u{1b}[2J".to_owned();
        assert!(matches!(
            database.create_file_area(&invalid_area),
            Err(FileError::InvalidAreaName)
        ));
        let stored_area = database.create_file_area(&area(1, "safe", 5)).unwrap();
        assert!(matches!(
            database.insert_file_entry(&NewFileEntry {
                area_id: stored_area.id,
                filename: "BAD.TXT".to_owned(),
                description: "Unsafe\u{1b}[2JDescription".to_owned(),
                size_bytes: 1,
                sha256: sha256_bytes(b"x"),
                uploaded_at: 200,
                uploader_caller_id: None,
                uploader_name: "SPITFIRE NG".to_owned(),
                lifecycle: FileLifecycle::Active,
            }),
            Err(FileError::InvalidDescription)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_file_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let (temp, mut database, storage, actor, _caller) = test_board();
        let area = database.create_file_area(&area(1, "links", 5)).unwrap();
        let entry = storage
            .write_seed_file(
                &mut database,
                &area,
                "LINK.TXT",
                "Link test",
                b"original\r\n",
                200,
            )
            .unwrap();
        let area_path = storage.ensure_area(&area).unwrap();
        fs::remove_file(area_path.join("LINK.TXT")).unwrap();
        let outside = temp.path().join("outside.txt");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, area_path.join("LINK.TXT")).unwrap();
        assert!(matches!(
            storage.open_download(&area, &entry),
            Err(FileError::UnsafeStorageObject(_))
        ));
        assert!(database.file(actor, area.id, "LINK.TXT", true).is_ok());
    }

    #[test]
    fn changed_file_bytes_are_rejected_before_download() {
        let (_temp, mut database, storage, _actor, _caller) = test_board();
        let area = database.create_file_area(&area(1, "integrity", 5)).unwrap();
        let entry = storage
            .write_seed_file(
                &mut database,
                &area,
                "HASH.TXT",
                "Integrity test",
                b"original\r\n",
                200,
            )
            .unwrap();
        let area_path = storage.ensure_area(&area).unwrap();
        fs::write(area_path.join("HASH.TXT"), b"modified\r\n").unwrap();
        assert!(matches!(
            storage.open_download(&area, &entry),
            Err(FileError::ContentMismatch(_))
        ));
    }

    #[test]
    fn ascii_transfer_download_and_upload_are_deterministic() {
        let (_temp, _database, storage, _actor, _caller) = test_board();
        let mut source = tempfile::tempfile().unwrap();
        source.write_all(b"download\r\n").unwrap();
        source.seek(SeekFrom::Start(0)).unwrap();
        let mut terminal = crate::InMemoryTerminal::with_lines([
            b"first line".to_vec(),
            b"second line".to_vec(),
            b"/S".to_vec(),
        ]);
        let download = AsciiTransfer.download(&mut terminal, &mut source).unwrap();
        assert_eq!(download.bytes, 10);
        let mut staged = storage
            .begin_upload(SessionId::new(11).unwrap(), "ASCII.TXT")
            .unwrap();
        let upload = AsciiTransfer
            .upload(&mut terminal, &mut staged, 1024)
            .unwrap();
        assert!(upload.completed);
        assert_eq!(upload.bytes, 25);
    }

    #[test]
    fn ascii_transfer_supports_an_explicit_empty_file() {
        let (_temp, _database, storage, _actor, _caller) = test_board();
        let mut source = std::io::Cursor::new(Vec::<u8>::new());
        let mut download_terminal = crate::InMemoryTerminal::default();
        let download = AsciiTransfer
            .download(&mut download_terminal, &mut source)
            .unwrap();
        assert!(download.completed);
        assert_eq!(download.bytes, 0);
        assert!(download_terminal.output().is_empty());

        let mut upload_terminal = crate::InMemoryTerminal::with_lines([b"/S".to_vec()]);
        let mut staged = storage
            .begin_upload(SessionId::new(13).unwrap(), "EMPTY.TXT")
            .unwrap();
        let upload = AsciiTransfer
            .upload(&mut upload_terminal, &mut staged, 1024)
            .unwrap();
        assert!(upload.completed);
        assert_eq!(upload.bytes, 0);
    }

    #[test]
    fn ascii_download_preflight_rejects_binary_before_transfer() {
        let (_temp, mut database, storage, actor, _caller) = test_board();
        let area = database.create_file_area(&area(1, "binary", 5)).unwrap();
        let entry = storage
            .write_seed_file(
                &mut database,
                &area,
                "BINARY.DAT",
                "Synthetic binary fixture",
                b"prefix\0suffix",
                200,
            )
            .unwrap();
        assert!(matches!(
            storage.open_ascii_download(&area, &entry),
            Err(FileError::NotAsciiText)
        ));
        for (name, bytes) in [
            ("ESCAPE.TXT", b"safe\x1b[2Junsafe".as_slice()),
            ("DELETE.TXT", b"safe\x7funsafe".as_slice()),
        ] {
            let unsafe_entry = storage
                .write_seed_file(
                    &mut database,
                    &area,
                    name,
                    "Synthetic terminal-control fixture",
                    bytes,
                    201,
                )
                .unwrap();
            assert!(matches!(
                storage.open_ascii_download(&area, &unsafe_entry),
                Err(FileError::NotAsciiText)
            ));
        }
        assert_eq!(
            database
                .file(actor, area.id, "BINARY.DAT", true)
                .unwrap()
                .download_count,
            0
        );
    }

    #[test]
    fn interrupted_download_reports_failure_without_accounting() {
        struct BrokenTerminal;

        impl Terminal for BrokenTerminal {
            fn info(&self) -> crate::TerminalInfo {
                crate::TerminalInfo::in_memory()
            }

            fn write_all(&mut self, _bytes: &[u8]) -> Result<(), TerminalError> {
                Err(
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "synthetic disconnect")
                        .into(),
                )
            }

            fn read_line(
                &mut self,
                _maximum_bytes: usize,
            ) -> Result<Option<Vec<u8>>, TerminalError> {
                Ok(None)
            }
        }

        let (_temp, mut database, storage, actor, caller_id) = test_board();
        let area = database.create_file_area(&area(1, "interrupt", 5)).unwrap();
        let entry = storage
            .write_seed_file(
                &mut database,
                &area,
                "BREAK.TXT",
                "Interrupted transfer",
                b"transfer body\r\n",
                200,
            )
            .unwrap();
        let mut input = storage.open_ascii_download(&area, &entry).unwrap();
        assert!(matches!(
            AsciiTransfer.download(&mut BrokenTerminal, &mut input),
            Err(FileError::Terminal(_))
        ));
        let caller = database.caller_by_id(caller_id).unwrap().unwrap();
        assert_eq!(caller.files_downloaded, 0);
        assert_eq!(
            database
                .file(actor, area.id, "BREAK.TXT", true)
                .unwrap()
                .download_count,
            0
        );
    }

    #[test]
    fn simultaneous_duplicate_uploads_commit_exactly_one_file() {
        let (_temp, mut database, storage, actor, _caller) = test_board();
        let area = database.create_file_area(&area(1, "race", 5)).unwrap();
        let mut first = storage
            .begin_upload(SessionId::new(21).unwrap(), "RACE.TXT")
            .unwrap();
        let mut second = storage
            .begin_upload(SessionId::new(22).unwrap(), "RACE.TXT")
            .unwrap();
        first.write_all(b"first\r\n").unwrap();
        second.write_all(b"second\r\n").unwrap();
        let database_path = database.path().to_path_buf();
        drop(database);
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [first, second]
            .into_iter()
            .map(|staged| {
                let barrier = Arc::clone(&barrier);
                let storage = storage.clone();
                let area = area.clone();
                let database_path = database_path.clone();
                std::thread::spawn(move || {
                    let mut database = RuntimeDatabase::open(&database_path).unwrap();
                    barrier.wait();
                    storage.commit_upload(staged, &mut database, actor, &area, "Race", 300)
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(FileError::DuplicateFilename(_))))
                .count(),
            1
        );
        let database = RuntimeDatabase::open(&database_path).unwrap();
        assert_eq!(database.file_count(area.id).unwrap(), 1);
    }

    #[test]
    fn area_edits_preserve_identity_and_disable_without_deleting_files() {
        let (_temp, mut database, storage, _actor, _caller) = test_board();
        let stored_area = database.create_file_area(&area(1, "stable", 5)).unwrap();
        storage
            .write_seed_file(
                &mut database,
                &stored_area,
                "STABLE.TXT",
                "Stable",
                b"stable\r\n",
                200,
            )
            .unwrap();
        let mut changed = area(1, "stable", 5);
        changed.name = "Renamed".to_owned();
        let updated = database.update_file_area(1, &changed).unwrap();
        assert_eq!(updated.id, stored_area.id);
        database.set_file_area_enabled(1, false).unwrap();
        assert_eq!(database.file_count(stored_area.id).unwrap(), 1);
        assert!(!database.all_file_areas().unwrap()[0].active);
    }

    #[test]
    fn storage_probe_distinguishes_available_missing_and_symlink_roots() {
        let (temp, _database, storage, _actor, _caller) = test_board();
        let available = temp.path().join("probe-available");
        fs::create_dir(&available).unwrap();
        let root = StorageRoot {
            id: crate::StorageRootId::new(1).unwrap(),
            area_id: FileAreaId::new(1).unwrap(),
            stable_key: "probe".to_owned(),
            label: "Probe".to_owned(),
            kind: StorageRootKind::External,
            mode: crate::StorageRootMode::ReadOnly,
            priority: 1,
            configured_locator: available.to_string_lossy().into_owned(),
            configured_state: crate::StorageRootState::Enabled,
            availability: StorageAvailability::Unknown,
            staging_always: true,
            state_version: 1,
        };
        assert_eq!(
            storage.probe_storage_root(&root),
            StorageAvailability::Available
        );
        let mut missing = root.clone();
        missing.configured_locator = temp
            .path()
            .join("probe-missing")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            storage.probe_storage_root(&missing),
            StorageAvailability::Unavailable
        );
        #[cfg(unix)]
        {
            let link = temp.path().join("probe-link");
            std::os::unix::fs::symlink(&available, &link).unwrap();
            let mut symlink = root;
            symlink.configured_locator = link.to_string_lossy().into_owned();
            assert_eq!(
                storage.probe_storage_root(&symlink),
                StorageAvailability::Unavailable
            );
        }
    }
}
