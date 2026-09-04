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

//! Schema-15 file inspection, request, policy, and maintenance domain.
//!
//! SQLite semantic rows plus confined managed bytes are authoritative. Legacy
//! listings and policy files are adapters, never a second writable authority.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    insert_operational_event_tx, CallerId, CallerState, EventAttributes, EventCategory,
    EventOutcome, EventSeverity, FileActor, FileArea, FileAreaId, FileEntry, FileError, FileId,
    FileIntegrity, FileLifecycle, FileStorage, NewOperationalEvent, RetentionClass,
    RuntimeDatabase, SessionId, MAX_FILE_DESCRIPTION_BYTES, MAX_FILE_DESCRIPTION_LINES,
};

pub const MAX_TEXT_PREVIEW_BYTES: u64 = 256 * 1024;
pub const MAX_TEXT_PREVIEW_LINES: usize = 2_000;
pub const MAX_ZIP_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ZIP_MEMBERS: usize = 4_096;
pub const MAX_ZIP_MEMBER_NAME_BYTES: usize = 1_024;
pub const MAX_ZIP_METADATA_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ZIP_DECLARED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_DIZ_BYTES: u64 = 64 * 1024;
pub const MAX_DIZ_EXPANSION_RATIO: u64 = 200;
pub const MAX_FILE_REQUEST_REASON_BYTES: usize = 255;
pub const MAX_UPLOAD_DENIAL_RULES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEncodingPolicy {
    Auto,
    Ascii,
    Utf8,
    Cp437,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectedTextEncoding {
    Ascii,
    Utf8,
    Cp437,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInspection {
    pub file_id: FileId,
    pub encoding: DetectedTextEncoding,
    pub lines: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveMember {
    pub filename: String,
    pub compressed_bytes: u64,
    pub uncompressed_bytes: u64,
    pub modified: Option<String>,
    pub unsafe_path: bool,
    pub encrypted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveInspection {
    pub file_id: FileId,
    pub members: Vec<ArchiveMember>,
    pub declared_uncompressed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DizInspection {
    pub archive_file_id: FileId,
    pub member_name: String,
    pub text: TextInspection,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAdminActor {
    ThresholdSysop(FileActor),
    LocalOperator,
}

impl FileAdminActor {
    fn caller_id(self) -> Option<CallerId> {
        match self {
            Self::ThresholdSysop(actor) => Some(actor.caller_id()),
            Self::LocalOperator => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileRequestReason {
    Offline,
    Missing,
}

impl FileRequestReason {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileRequestStatus {
    Pending,
    Fulfilled,
    Rejected,
    Cancelled,
    Stale,
}

impl FileRequestStatus {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Fulfilled => "fulfilled",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Stale => "stale",
        }
    }

    fn from_database(value: &str) -> Result<Self, FileMaintenanceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "fulfilled" => Ok(Self::Fulfilled),
            "rejected" => Ok(Self::Rejected),
            "cancelled" => Ok(Self::Cancelled),
            "stale" => Ok(Self::Stale),
            _ => Err(FileMaintenanceError::InvalidStoredState(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileRequest {
    pub request_id: i64,
    pub file_id: FileId,
    pub requesting_caller_id: CallerId,
    pub reason: FileRequestReason,
    pub detail: Option<String>,
    pub status: FileRequestStatus,
    pub state_version: u64,
    pub created_at: String,
    pub created_board_day: String,
    pub resolved_at: Option<String>,
    pub resolved_by_caller_id: Option<CallerId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadDenialRule {
    pub rule_id: i64,
    pub pattern: String,
    pub enabled: bool,
    pub state_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicateMatchKind {
    SameBaseDifferentExtension,
    VersionFamily,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateWarning {
    pub file_id: FileId,
    pub filename: String,
    pub kind: DuplicateMatchKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyPolicyImport {
    pub patterns: Vec<String>,
    pub rejected_lines: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceMode {
    Online,
    Maintenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileOperationResult {
    pub operation_id: String,
    pub file: FileEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileUseToken {
    use_id: String,
    pub file_id: FileId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationResult {
    pub present: usize,
    pub missing: usize,
    pub digest_mismatch: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureInjectionPoint {
    AfterJournal,
    AfterStage,
    AfterPublish,
    AfterCatalogCommit,
    BeforeSourceRemoval,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileOperatorCommand {
    Add {
        area_id: FileAreaId,
        expected_area_version: u64,
        filename: String,
        description: String,
        bytes: Vec<u8>,
    },
    MetadataUpdate {
        file_id: FileId,
        expected_version: u64,
        description: String,
    },
    LifecycleUpdate {
        file_id: FileId,
        expected_version: u64,
        lifecycle: FileLifecycle,
    },
    Move {
        file_id: FileId,
        expected_file_version: u64,
        destination_area_id: FileAreaId,
        expected_destination_version: u64,
    },
    Remove {
        file_id: FileId,
        expected_version: u64,
        confirmed: bool,
    },
    Review {
        file_id: FileId,
        expected_version: u64,
        accept: bool,
    },
    RequestResolve {
        request_id: i64,
        expected_version: u64,
        status: FileRequestStatus,
    },
    Reconcile {
        mode: MaintenanceMode,
    },
    PublishLegacyListing {
        area_id: FileAreaId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileOperatorResponse {
    File(FileEntry),
    Operation(FileOperationResult),
    Request(FileRequest),
    Reconciliation(ReconciliationResult),
    PublicationDigest(String),
}

impl RuntimeDatabase {
    pub fn begin_file_download_use(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        session_id: SessionId,
    ) -> Result<FileUseToken, FileMaintenanceError> {
        self.begin_file_use(actor, file_id, session_id, "download", true)
    }

    pub fn begin_file_inspection_use(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        session_id: SessionId,
    ) -> Result<FileUseToken, FileMaintenanceError> {
        self.begin_file_use(actor, file_id, session_id, "inspect", false)
    }

    fn begin_file_use(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        session_id: SessionId,
        use_kind: &str,
        require_full_access: bool,
    ) -> Result<FileUseToken, FileMaintenanceError> {
        let file = self
            .load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        let (_, _area, access) = self.authorized_area(actor, file.area_id)?;
        if require_full_access && access != crate::FileAccess::Full
            || file.lifecycle != FileLifecycle::Active
            || matches!(
                file.integrity,
                FileIntegrity::Missing | FileIntegrity::DigestMismatch
            )
        {
            return Err(FileMaintenanceError::AuthorizationDenied);
        }
        self.connection.execute(
            "DELETE FROM file_active_uses WHERE expires_at<=CURRENT_TIMESTAMP",
            [],
        )?;
        let use_id = format!(
            "{}-{}-{}-{}",
            use_kind,
            session_id.get(),
            file_id.get(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        self.connection.execute(
            "INSERT INTO file_active_uses(use_id,file_id,session_id,use_kind,expires_at) VALUES(?1,?2,?3,?4,datetime('now','+1 day'))",
            params![use_id, file_id.get(), i64::try_from(session_id.get()).map_err(|_| FileMaintenanceError::InvalidSessionId)?, use_kind],
        )?;
        Ok(FileUseToken { use_id, file_id })
    }

    pub fn finish_file_use(&self, token: FileUseToken) -> Result<(), FileMaintenanceError> {
        self.connection.execute(
            "DELETE FROM file_active_uses WHERE use_id=?1 AND file_id=?2",
            params![token.use_id, token.file_id.get()],
        )?;
        Ok(())
    }

    pub fn reauthorize_file_inspection(
        &self,
        actor: FileActor,
        file_id: FileId,
    ) -> Result<(), FileMaintenanceError> {
        self.inspectable_file(actor, file_id).map(|_| ())
    }

    pub fn dispatch_file_operator_command(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        command: FileOperatorCommand,
    ) -> Result<FileOperatorResponse, FileMaintenanceError> {
        match command {
            FileOperatorCommand::Add {
                area_id,
                expected_area_version,
                filename,
                description,
                bytes,
            } => self
                .add_managed_file(
                    storage,
                    actor,
                    area_id,
                    expected_area_version,
                    &filename,
                    &description,
                    &bytes,
                )
                .map(FileOperatorResponse::Operation),
            FileOperatorCommand::MetadataUpdate {
                file_id,
                expected_version,
                description,
            } => self
                .update_file_metadata(actor, file_id, expected_version, &description)
                .map(FileOperatorResponse::File),
            FileOperatorCommand::LifecycleUpdate {
                file_id,
                expected_version,
                lifecycle,
            } => self
                .set_file_lifecycle(actor, file_id, expected_version, lifecycle)
                .map(FileOperatorResponse::File),
            FileOperatorCommand::Move {
                file_id,
                expected_file_version,
                destination_area_id,
                expected_destination_version,
            } => self
                .move_file(
                    storage,
                    actor,
                    file_id,
                    expected_file_version,
                    destination_area_id,
                    expected_destination_version,
                )
                .map(FileOperatorResponse::Operation),
            FileOperatorCommand::Remove {
                file_id,
                expected_version,
                confirmed,
            } => self
                .remove_file(actor, file_id, expected_version, confirmed)
                .map(FileOperatorResponse::Operation),
            FileOperatorCommand::Review {
                file_id,
                expected_version,
                accept,
            } => self
                .review_pending_file(actor, file_id, expected_version, accept)
                .map(FileOperatorResponse::File),
            FileOperatorCommand::RequestResolve {
                request_id,
                expected_version,
                status,
            } => self
                .resolve_file_request(actor, request_id, expected_version, status)
                .map(FileOperatorResponse::Request),
            FileOperatorCommand::Reconcile { mode } => self
                .reconcile_files(storage, actor, mode)
                .map(FileOperatorResponse::Reconciliation),
            FileOperatorCommand::PublishLegacyListing { area_id } => self
                .publish_legacy_listing(storage, actor, area_id)
                .map(FileOperatorResponse::PublicationDigest),
        }
    }

    pub fn inspect_text_file(
        &mut self,
        storage: &FileStorage,
        actor: FileActor,
        file_id: FileId,
        policy: TextEncodingPolicy,
    ) -> Result<TextInspection, FileMaintenanceError> {
        let (area, file) = self.inspectable_file(actor, file_id)?;
        if file.size_bytes > MAX_TEXT_PREVIEW_BYTES {
            return Err(FileMaintenanceError::TextTooLarge(file.size_bytes));
        }
        let mut input = self.open_and_record_integrity(storage, &area, &file)?;
        let mut bytes = Vec::with_capacity(file.size_bytes as usize);
        std::io::Read::by_ref(&mut input)
            .take(MAX_TEXT_PREVIEW_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(FileMaintenanceError::Io)?;
        if bytes.len() as u64 > MAX_TEXT_PREVIEW_BYTES {
            return Err(FileMaintenanceError::TextTooLarge(bytes.len() as u64));
        }
        decode_text(file.id, &bytes, policy)
    }

    pub fn inspect_zip_file(
        &mut self,
        storage: &FileStorage,
        actor: FileActor,
        file_id: FileId,
    ) -> Result<ArchiveInspection, FileMaintenanceError> {
        self.inspect_zip_file_with_deadline(storage, actor, file_id, Duration::from_secs(5))
    }

    fn inspect_zip_file_with_deadline(
        &mut self,
        storage: &FileStorage,
        actor: FileActor,
        file_id: FileId,
        deadline: Duration,
    ) -> Result<ArchiveInspection, FileMaintenanceError> {
        let (area, file) = self.inspectable_file(actor, file_id)?;
        if file.size_bytes > MAX_ZIP_BYTES {
            return Err(FileMaintenanceError::ArchiveTooLarge(file.size_bytes));
        }
        let mut input = self.open_and_record_integrity(storage, &area, &file)?;
        let declared_members = declared_standard_zip_member_count(&mut input)?;
        let started = Instant::now();
        let mut archive = zip::ZipArchive::new(input).map_err(FileMaintenanceError::Zip)?;
        if declared_members.is_some_and(|count| count != archive.len()) {
            return Err(FileMaintenanceError::ArchiveDirectoryInconsistent);
        }
        if archive.len() > MAX_ZIP_MEMBERS {
            return Err(FileMaintenanceError::TooManyArchiveMembers(archive.len()));
        }
        if archive
            .has_overlapping_files()
            .map_err(FileMaintenanceError::Zip)?
        {
            return Err(FileMaintenanceError::OverlappingArchiveMembers);
        }
        let mut members = Vec::with_capacity(archive.len());
        let mut metadata_bytes = archive.comment().len();
        let mut declared = 0_u64;
        for index in 0..archive.len() {
            if started.elapsed() >= deadline {
                return Err(FileMaintenanceError::InspectionDeadline);
            }
            let member = archive
                .by_index_raw(index)
                .map_err(FileMaintenanceError::Zip)?;
            if member.encrypted() {
                return Err(FileMaintenanceError::EncryptedArchiveMember);
            }
            if !matches!(
                member.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ) {
                return Err(FileMaintenanceError::UnsupportedArchiveCompression);
            }
            if member.name_raw().len() > MAX_ZIP_MEMBER_NAME_BYTES {
                return Err(FileMaintenanceError::ArchiveMemberNameTooLong);
            }
            metadata_bytes = metadata_bytes
                .checked_add(member.name_raw().len())
                .and_then(|value| value.checked_add(member.comment().len()))
                .and_then(|value| value.checked_add(member.extra_data().map_or(0, <[u8]>::len)))
                .ok_or(FileMaintenanceError::ArchiveMetadataTooLarge)?;
            if metadata_bytes > MAX_ZIP_METADATA_BYTES {
                return Err(FileMaintenanceError::ArchiveMetadataTooLarge);
            }
            declared = declared
                .checked_add(member.size())
                .ok_or(FileMaintenanceError::ArchiveDeclaredSizeTooLarge)?;
            if declared > MAX_ZIP_DECLARED_BYTES {
                return Err(FileMaintenanceError::ArchiveDeclaredSizeTooLarge);
            }
            let (filename, unsafe_path) =
                safe_archive_name(member.name_raw(), member.enclosed_name());
            members.push(ArchiveMember {
                filename,
                compressed_bytes: member.compressed_size(),
                uncompressed_bytes: member.size(),
                modified: member.last_modified().map(|value| value.to_string()),
                unsafe_path,
                encrypted: member.encrypted(),
            });
        }
        Ok(ArchiveInspection {
            file_id,
            members,
            declared_uncompressed_bytes: declared,
        })
    }

    pub fn inspect_file_id_diz(
        &mut self,
        storage: &FileStorage,
        actor: FileActor,
        file_id: FileId,
        policy: TextEncodingPolicy,
    ) -> Result<Option<DizInspection>, FileMaintenanceError> {
        let (area, file) = self.inspectable_file(actor, file_id)?;
        if file.size_bytes > MAX_ZIP_BYTES {
            return Err(FileMaintenanceError::ArchiveTooLarge(file.size_bytes));
        }
        let mut input = self.open_and_record_integrity(storage, &area, &file)?;
        let declared_members = declared_standard_zip_member_count(&mut input)?;
        let mut archive = zip::ZipArchive::new(input).map_err(FileMaintenanceError::Zip)?;
        if declared_members.is_some_and(|count| count != archive.len()) {
            return Err(FileMaintenanceError::ArchiveDirectoryInconsistent);
        }
        if archive.len() > MAX_ZIP_MEMBERS
            || archive
                .has_overlapping_files()
                .map_err(FileMaintenanceError::Zip)?
        {
            return Err(FileMaintenanceError::UnsafeArchive);
        }
        let mut match_index = None;
        let mut match_name = String::new();
        for index in 0..archive.len() {
            let member = archive
                .by_index_raw(index)
                .map_err(FileMaintenanceError::Zip)?;
            if member.encrypted() {
                return Err(FileMaintenanceError::EncryptedArchiveMember);
            }
            if !matches!(
                member.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ) {
                return Err(FileMaintenanceError::UnsupportedArchiveCompression);
            }
            let Some(path) = member.enclosed_name() else {
                continue;
            };
            if !member.is_file() {
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("FILE_ID.DIZ"))
            {
                if match_index.replace(index).is_some() {
                    return Err(FileMaintenanceError::AmbiguousFileIdDiz);
                }
                match_name = sanitized_text(&decode_filename(member.name_raw()));
            }
        }
        let Some(index) = match_index else {
            return Ok(None);
        };
        let mut member = archive.by_index(index).map_err(FileMaintenanceError::Zip)?;
        if member.size() > MAX_DIZ_BYTES
            || member.compressed_size() == 0 && member.size() > 0
            || member.size()
                > member
                    .compressed_size()
                    .saturating_mul(MAX_DIZ_EXPANSION_RATIO)
        {
            return Err(FileMaintenanceError::FileIdDizTooLarge);
        }
        let mut bytes = Vec::with_capacity(member.size() as usize);
        member
            .by_ref()
            .take(MAX_DIZ_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(FileMaintenanceError::Io)?;
        if bytes.len() as u64 > MAX_DIZ_BYTES {
            return Err(FileMaintenanceError::FileIdDizTooLarge);
        }
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let mut text = decode_text(file_id, &bytes, policy)?;
        text.lines.truncate(MAX_FILE_DESCRIPTION_LINES);
        let mut total = 0_usize;
        text.lines.retain(|line| {
            total = total.saturating_add(line.len());
            total <= MAX_FILE_DESCRIPTION_BYTES
        });
        text.truncated |= total > MAX_FILE_DESCRIPTION_BYTES;
        Ok(Some(DizInspection {
            archive_file_id: file_id,
            member_name: match_name,
            text,
            sha256: digest,
        }))
    }

    fn inspectable_file(
        &self,
        actor: FileActor,
        file_id: FileId,
    ) -> Result<(FileArea, FileEntry), FileMaintenanceError> {
        let file = self
            .load_file_by_id(file_id)?
            .ok_or(FileError::FileIdNotFound(file_id.get()))?;
        let (caller, area, _access) = self.authorized_area(actor, file.area_id)?;
        let pending_review_for_sysop = file.lifecycle == FileLifecycle::PendingReview
            && caller.security_level.is_sysop(actor.sysop_security());
        if !matches!(
            file.lifecycle,
            FileLifecycle::Active | FileLifecycle::Offline
        ) && !pending_review_for_sysop
        {
            return Err(FileMaintenanceError::FileUnavailable);
        }
        if matches!(
            file.integrity,
            FileIntegrity::Missing | FileIntegrity::DigestMismatch
        ) {
            return Err(FileMaintenanceError::FileUnavailable);
        }
        Ok((area, file))
    }

    fn open_and_record_integrity(
        &mut self,
        storage: &FileStorage,
        _area: &FileArea,
        file: &FileEntry,
    ) -> Result<File, FileMaintenanceError> {
        let (root, locator) = self
            .resolve_file_storage(file.id)
            .map_err(|_| FileMaintenanceError::FileUnavailable)?;
        match storage.open_resolved_download(&root, &locator, file) {
            Ok(input) => {
                if file.integrity != FileIntegrity::Present {
                    self.set_integrity_observation(file.id, FileIntegrity::Present)?;
                }
                Ok(input)
            }
            Err(FileError::ContentMismatch(_)) => {
                self.set_integrity_observation(file.id, FileIntegrity::DigestMismatch)?;
                Err(FileMaintenanceError::FileUnavailable)
            }
            Err(FileError::StorageIo { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                self.set_integrity_observation(file.id, FileIntegrity::Missing)?;
                Err(FileMaintenanceError::FileUnavailable)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn set_integrity_observation(
        &self,
        file_id: FileId,
        integrity: FileIntegrity,
    ) -> Result<(), FileMaintenanceError> {
        self.connection.execute(
            "UPDATE files SET integrity_state=?2,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND lifecycle<>'tombstoned'",
            params![file_id.get(), integrity.as_database_value()],
        )?;
        Ok(())
    }

    pub fn create_file_request(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        detail: Option<&str>,
    ) -> Result<FileRequest, FileMaintenanceError> {
        self.create_file_request_on_board_day(
            actor,
            file_id,
            detail,
            &Utc::now().date_naive().to_string(),
        )
    }

    pub fn create_file_request_on_board_day(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        detail: Option<&str>,
        board_day: &str,
    ) -> Result<FileRequest, FileMaintenanceError> {
        let caller = self.active_file_actor(actor)?;
        if chrono::NaiveDate::parse_from_str(board_day, "%Y-%m-%d").is_err() {
            return Err(FileMaintenanceError::InvalidBoardDay);
        }
        let file = self
            .load_file_by_id(file_id)?
            .ok_or(FileError::FileIdNotFound(file_id.get()))?;
        self.authorized_area(actor, file.area_id)?;
        let reason = if file.lifecycle == FileLifecycle::Offline {
            FileRequestReason::Offline
        } else if file.integrity == FileIntegrity::Missing {
            FileRequestReason::Missing
        } else {
            return Err(FileMaintenanceError::FileNotRequestable);
        };
        let detail = detail.map(str::trim).filter(|value| !value.is_empty());
        if detail.is_some_and(|value| value.len() > MAX_FILE_REQUEST_REASON_BYTES) {
            return Err(FileMaintenanceError::RequestDetailTooLong);
        }
        let transaction = self.connection.transaction()?;
        let open_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM file_requests WHERE requesting_caller_id=?1 AND status='pending'",
            params![caller.id.get()],
            |row| row.get(0),
        )?;
        let daily_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM file_requests WHERE requesting_caller_id=?1 AND created_board_day=?2",
            params![caller.id.get(), board_day],
            |row| row.get(0),
        )?;
        let existing: Option<i64> = transaction
            .query_row(
                "SELECT request_id FROM file_requests WHERE file_id=?1 AND requesting_caller_id=?2 AND status='pending'",
                params![file_id.get(), caller.id.get()],
                |row| row.get(0),
            )
            .optional()?;
        let request_id = if let Some(id) = existing {
            id
        } else {
            if open_count >= 25 || daily_count >= 100 {
                return Err(FileMaintenanceError::RequestLimitReached);
            }
            transaction.execute(
                "INSERT INTO file_requests(file_id,requesting_caller_id,reason,reason_detail,created_board_day) VALUES(?1,?2,?3,?4,?5)",
                params![file_id.get(), caller.id.get(), reason.database_value(), detail, board_day],
            )?;
            let id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO file_events(operation,actor_caller_id,file_id,request_id,detail) VALUES('request-created',?1,?2,?3,?4)",
                params![caller.id.get(), file_id.get(), id, reason.database_value()],
            )?;
            id
        };
        transaction.commit()?;
        self.file_request(request_id)
    }

    pub fn pending_file_requests(
        &self,
        actor: FileAdminActor,
    ) -> Result<Vec<FileRequest>, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let mut statement = self.connection.prepare(
            "SELECT request_id,file_id,requesting_caller_id,reason,reason_detail,status,state_version,created_at,created_board_day,resolved_at,resolved_by_caller_id FROM file_requests WHERE status='pending' ORDER BY created_at,request_id LIMIT 1000",
        )?;
        let rows = statement.query_map([], stored_request)?;
        rows.map(|row| row.map_err(FileMaintenanceError::Sqlite))
            .collect()
    }

    pub fn cancel_own_file_request(
        &mut self,
        actor: FileActor,
        request_id: i64,
        expected_version: u64,
    ) -> Result<FileRequest, FileMaintenanceError> {
        let caller = self.active_file_actor(actor)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_requests SET status='cancelled',state_version=state_version+1,resolved_at=CURRENT_TIMESTAMP,resolved_by_caller_id=?2,updated_at=CURRENT_TIMESTAMP WHERE request_id=?1 AND requesting_caller_id=?2 AND status='pending' AND state_version=?3",
            params![request_id, caller.id.get(), expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,request_id,detail) VALUES('request-cancelled',?1,(SELECT file_id FROM file_requests WHERE request_id=?2),?2,'caller-cancelled')",
            params![caller.id.get(), request_id],
        )?;
        transaction.commit()?;
        self.file_request(request_id)
    }

    pub fn resolve_file_request(
        &mut self,
        actor: FileAdminActor,
        request_id: i64,
        expected_version: u64,
        status: FileRequestStatus,
    ) -> Result<FileRequest, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        if status == FileRequestStatus::Pending {
            return Err(FileMaintenanceError::InvalidRequestTransition);
        }
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_requests SET status=?2,state_version=state_version+1,resolved_at=CURRENT_TIMESTAMP,resolved_by_caller_id=?3,updated_at=CURRENT_TIMESTAMP WHERE request_id=?1 AND status='pending' AND state_version=?4",
            params![request_id, status.database_value(), actor.caller_id().map(CallerId::get), expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,request_id,detail) VALUES('request-resolved',?1,(SELECT file_id FROM file_requests WHERE request_id=?2),?2,?3)",
            params![actor.caller_id().map(CallerId::get), request_id, status.database_value()],
        )?;
        transaction.commit()?;
        self.file_request(request_id)
    }

    fn file_request(&self, request_id: i64) -> Result<FileRequest, FileMaintenanceError> {
        self.connection
            .query_row(
                "SELECT request_id,file_id,requesting_caller_id,reason,reason_detail,status,state_version,created_at,created_board_day,resolved_at,resolved_by_caller_id FROM file_requests WHERE request_id=?1",
                params![request_id],
                stored_request,
            )
            .optional()?
            .ok_or(FileMaintenanceError::RequestNotFound(request_id))
    }

    pub(crate) fn authorize_file_admin(
        &self,
        actor: FileAdminActor,
    ) -> Result<(), FileMaintenanceError> {
        match actor {
            FileAdminActor::LocalOperator => Ok(()),
            FileAdminActor::ThresholdSysop(actor) => {
                let caller = self.active_file_actor(actor)?;
                if caller.state == CallerState::Active
                    && caller.security_level.is_sysop(actor.sysop_security())
                {
                    Ok(())
                } else {
                    Err(FileMaintenanceError::AuthorizationDenied)
                }
            }
        }
    }

    pub fn update_file_metadata(
        &mut self,
        actor: FileAdminActor,
        file_id: FileId,
        expected_version: u64,
        description: &str,
    ) -> Result<FileEntry, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        validate_description(description)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET description=?2,description_source='operator',description_source_digest=NULL,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?3 AND lifecycle<>'tombstoned'",
            params![file_id.get(), description, expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,prior_version,next_version,detail) VALUES('metadata-edited',?1,?2,?3,?4,'description')",
            params![actor.caller_id().map(CallerId::get), file_id.get(), expected_version as i64, expected_version.saturating_add(1) as i64],
        )?;
        transaction.commit()?;
        self.load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)
    }

    pub fn set_file_lifecycle(
        &mut self,
        actor: FileAdminActor,
        file_id: FileId,
        expected_version: u64,
        lifecycle: FileLifecycle,
    ) -> Result<FileEntry, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        if matches!(
            lifecycle,
            FileLifecycle::PendingReview | FileLifecycle::Tombstoned
        ) {
            return Err(FileMaintenanceError::InvalidLifecycleTransition);
        }
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET lifecycle=?2,state_version=state_version+1,tombstoned_at=NULL,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?3 AND lifecycle IN ('active','offline','disabled')",
            params![file_id.get(), lifecycle.as_database_value(), expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,prior_version,next_version,detail) VALUES('lifecycle-changed',?1,?2,?3,?4,?5)",
            params![actor.caller_id().map(CallerId::get), file_id.get(), expected_version as i64, expected_version.saturating_add(1) as i64, lifecycle.as_database_value()],
        )?;
        transaction.commit()?;
        self.load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)
    }

    pub fn review_pending_file(
        &mut self,
        actor: FileAdminActor,
        file_id: FileId,
        expected_version: u64,
        accept: bool,
    ) -> Result<FileEntry, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let lifecycle = if accept { "active" } else { "tombstoned" };
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET lifecycle=?2,state_version=state_version+1,reviewed_by_caller_id=?3,reviewed_at=CURRENT_TIMESTAMP,tombstoned_at=CASE WHEN ?2='tombstoned' THEN CURRENT_TIMESTAMP ELSE NULL END,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?4 AND lifecycle='pending-review'",
            params![file_id.get(), lifecycle, actor.caller_id().map(CallerId::get), expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,prior_version,next_version,detail) VALUES(?1,?2,?3,?4,?5,?6)",
            params![if accept { "review-accepted" } else { "review-rejected" }, actor.caller_id().map(CallerId::get), file_id.get(), expected_version as i64, expected_version.saturating_add(1) as i64, if accept { "accepted" } else { "rejected" }],
        )?;
        transaction.commit()?;
        self.load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)
    }

    pub fn apply_reviewed_file_id_diz(
        &mut self,
        actor: FileAdminActor,
        file_id: FileId,
        expected_version: u64,
        inspection: &DizInspection,
    ) -> Result<FileEntry, FileMaintenanceError> {
        if inspection.archive_file_id != file_id {
            return Err(FileMaintenanceError::FileIdDizMismatch);
        }
        let description = inspection.text.lines.join("\n");
        validate_description(&description)?;
        self.authorize_file_admin(actor)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET description=?2,description_source='file-id-diz',description_source_digest=?3,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?4 AND lifecycle<>'tombstoned'",
            params![file_id.get(), description, inspection.sha256, expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,prior_version,next_version,detail) VALUES('file-id-diz-applied',?1,?2,?3,?4,'reviewed')",
            params![actor.caller_id().map(CallerId::get), file_id.get(), expected_version as i64, expected_version.saturating_add(1) as i64],
        )?;
        transaction.commit()?;
        self.load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)
    }

    pub fn upload_duplicate_warnings(
        &self,
        actor: FileActor,
        area_id: FileAreaId,
        filename: &str,
    ) -> Result<Vec<DuplicateWarning>, FileMaintenanceError> {
        self.authorized_area(actor, area_id)?;
        let normalized = crate::normalize_filename(filename)?;
        let (base, extension) = split_filename(&normalized);
        let comprehensive: bool = self.connection.query_row(
            "SELECT comprehensive_upload_search FROM file_policy WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        let family = trim_version_digits(base);
        let mut statement = self.connection.prepare(
            "SELECT file_id,filename FROM files WHERE lifecycle<>'tombstoned' ORDER BY normalized_filename,file_id LIMIT 10000",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut warnings = Vec::new();
        for row in rows {
            let (id, existing) = row?;
            if existing.eq_ignore_ascii_case(&normalized) {
                return Err(FileError::DuplicateFilename(filename.to_owned()).into());
            }
            let existing_normalized = existing.to_ascii_uppercase();
            let (existing_base, existing_extension) = split_filename(&existing_normalized);
            let kind = if existing_base == base && existing_extension != extension {
                Some(DuplicateMatchKind::SameBaseDifferentExtension)
            } else if comprehensive
                && !family.is_empty()
                && trim_version_digits(existing_base) == family
            {
                Some(DuplicateMatchKind::VersionFamily)
            } else {
                None
            };
            if let Some(kind) = kind {
                warnings.push(DuplicateWarning {
                    file_id: FileId::new(id)?,
                    filename: existing,
                    kind,
                });
                if warnings.len() == 50 {
                    break;
                }
            }
        }
        Ok(warnings)
    }

    pub fn upload_is_denied(
        &self,
        actor: FileActor,
        filename: &str,
    ) -> Result<bool, FileMaintenanceError> {
        let caller = self.active_file_actor(actor)?;
        if caller.security_level.is_sysop(actor.sysop_security()) {
            return Ok(false);
        }
        let normalized = crate::normalize_filename(filename)?;
        let mut statement = self.connection.prepare(
            "SELECT normalized_pattern FROM file_upload_denials WHERE lifecycle='active' ORDER BY rule_id LIMIT 1024",
        )?;
        let patterns = statement.query_map([], |row| row.get::<_, String>(0))?;
        for pattern in patterns {
            if dos_wildcard_matches(&pattern?, &normalized) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn normalize_upload_description(
        &self,
        description: &str,
    ) -> Result<String, FileMaintenanceError> {
        validate_description(description)?;
        let policy: String = self.connection.query_row(
            "SELECT description_normalization FROM file_policy WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if policy == "preserve" {
            return Ok(description.to_owned());
        }
        if policy != "historical-uppercase" {
            return Err(FileMaintenanceError::InvalidStoredState(policy));
        }
        let mut normalized = description.to_ascii_uppercase();
        let mut statement = self
            .connection
            .prepare("SELECT term FROM file_uppercase_terms ORDER BY normalized_term LIMIT 1024")?;
        let terms = statement.query_map([], |row| row.get::<_, String>(0))?;
        for term in terms {
            let term = term?;
            normalized = normalized.replace(&term.to_ascii_uppercase(), &term);
        }
        Ok(normalized)
    }

    pub fn replace_upload_denials(
        &mut self,
        actor: FileAdminActor,
        expected_policy_version: u64,
        patterns: &[String],
    ) -> Result<u64, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        if patterns.len() > MAX_UPLOAD_DENIAL_RULES {
            return Err(FileMaintenanceError::TooManyUploadDenials);
        }
        let normalized = patterns
            .iter()
            .map(|pattern| normalize_denial_pattern(pattern))
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = self.connection.transaction()?;
        let current: i64 = transaction.query_row(
            "SELECT state_version FROM file_policy WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if current != expected_policy_version as i64 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute("DELETE FROM file_upload_denials", [])?;
        for pattern in &normalized {
            transaction.execute(
                "INSERT INTO file_upload_denials(pattern,normalized_pattern) VALUES(?1,?1)",
                params![pattern],
            )?;
        }
        let next = expected_policy_version.saturating_add(1);
        transaction.execute(
            "UPDATE file_policy SET state_version=?1,updated_at=CURRENT_TIMESTAMP WHERE singleton=1",
            params![next as i64],
        )?;
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,prior_version,next_version,detail) VALUES('denial-policy-changed',?1,?2,?3,?4)",
            params![actor.caller_id().map(CallerId::get), expected_policy_version as i64, next as i64, format!("{} rules", normalized.len())],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub fn set_description_normalization(
        &mut self,
        actor: FileAdminActor,
        expected_policy_version: u64,
        historical_uppercase: bool,
    ) -> Result<u64, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let next = expected_policy_version.saturating_add(1);
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_policy SET description_normalization=?1,state_version=?2,updated_at=CURRENT_TIMESTAMP WHERE singleton=1 AND state_version=?3",
            params![if historical_uppercase { "historical-uppercase" } else { "preserve" }, next as i64, expected_policy_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,prior_version,next_version,detail) VALUES('description-policy-changed',?1,?2,?3,?4)",
            params![actor.caller_id().map(CallerId::get), expected_policy_version as i64, next as i64, if historical_uppercase { "historical-uppercase" } else { "preserve" }],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    // The explicit authority, version, identity, metadata, and byte arguments
    // are intentionally kept visible at this daemon-command boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn add_managed_file(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        area_id: FileAreaId,
        expected_area_version: u64,
        filename: &str,
        description: &str,
        bytes: &[u8],
    ) -> Result<FileOperationResult, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let area = self
            .load_area_by_id(area_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        if area.state_version != expected_area_version || bytes.is_empty() {
            return Err(FileMaintenanceError::StaleConflict);
        }
        let normalized = crate::normalize_filename(filename)?;
        validate_description(description)?;
        let operation_id = new_operation_id_raw("add");
        self.begin_operation(
            &operation_id,
            "add",
            actor,
            None,
            Some(area_id),
            Some(expected_area_version),
        )?;
        let reservation_result = (|| {
            let reservation = self.connection.transaction()?;
            reservation.execute(
                "INSERT INTO file_operation_leases(lease_kind,area_id,operation_id,expires_at) VALUES('area',?1,?2,datetime('now','+5 minutes'))",
                params![area_id.get(), operation_id],
            )?;
            reservation.execute(
                "INSERT INTO file_operation_leases(lease_kind,area_id,normalized_filename,operation_id,expires_at) VALUES('name',?1,?2,?3,datetime('now','+5 minutes'))",
                params![area_id.get(), normalized, operation_id],
            )?;
            reservation.commit()
        })();
        if let Err(error) = reservation_result {
            self.mark_operation_needs_review(&operation_id, "reservation-conflict")?;
            return Err(lease_error_without_file(error));
        }
        let directory = storage.ensure_area(&area)?;
        let staging_directory = directory.join(".spitfire-ng-staging");
        create_confined_directory(&staging_directory, &directory)?;
        let staged_path = staging_directory.join(format!("{operation_id}.part"));
        let mut staged = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged_path)
            .map_err(FileMaintenanceError::Io)?;
        staged.write_all(bytes).map_err(FileMaintenanceError::Io)?;
        staged.sync_all().map_err(FileMaintenanceError::Io)?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        self.connection.execute(
            "UPDATE file_operations SET phase='staged',staging_path=?2,digest=?3,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, format!("{}/.spitfire-ng-staging/{}.part", area.storage_key, operation_id), digest],
        )?;
        let destination = directory.join(filename);
        if destination.exists() {
            self.mark_operation_needs_review(&operation_id, "destination collision")?;
            return Err(FileError::DuplicateFilename(filename.to_owned()).into());
        }
        fs::rename(&staged_path, &destination).map_err(FileMaintenanceError::Io)?;
        self.connection.execute(
            "UPDATE file_operations SET phase='bytes-published',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id],
        )?;
        sync_directory(&directory)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO files(area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_name,lifecycle,integrity_state,description_source) VALUES(?1,?2,?3,?4,?5,?6,?7,'SPITFIRE NG','active','present','operator')",
            params![area_id.get(), filename, normalized, description, bytes.len() as i64, digest, Utc::now().timestamp()],
        ).map_err(|error| {
            let _ = fs::remove_file(&destination);
            FileMaintenanceError::Sqlite(error)
        })?;
        let file_id = FileId::new(transaction.last_insert_rowid())?;
        let locator_added = transaction.execute(
            "INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path) SELECT ?1,storage_root_id,?2 FROM file_storage_roots WHERE area_id=?3 AND priority=0",
            params![file_id.get(), filename, area_id.get()],
        )?;
        if locator_added != 1 {
            let _ = fs::remove_file(&destination);
            return Err(FileMaintenanceError::FileUnavailable);
        }
        let changed = transaction.execute(
            "UPDATE file_areas SET state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE area_id=?1 AND state_version=?2",
            params![area_id.get(), expected_area_version as i64],
        )?;
        if changed != 1 {
            let _ = fs::remove_file(&destination);
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "UPDATE file_operations SET file_id=?2,phase='catalog-committed',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, file_id.get()],
        )?;
        transaction.commit()?;
        self.finish_operation(&operation_id, actor, file_id, "file-added")?;
        Ok(FileOperationResult {
            operation_id,
            file: self
                .load_file_by_id(file_id)?
                .ok_or(FileMaintenanceError::FileUnavailable)?,
        })
    }

    pub fn publish_legacy_listing(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        area_id: FileAreaId,
    ) -> Result<String, FileMaintenanceError> {
        self.publish_legacy_listing_named(storage, actor, area_id, "SFFILES.BBS")
    }

    /// Publishes the semantic catalog through the bounded historical
    /// SFFILES.<x> filename family used by read-only/extended media adapters.
    pub fn publish_numbered_legacy_listing(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        area_id: FileAreaId,
        index: u16,
    ) -> Result<String, FileMaintenanceError> {
        if index == 0 {
            return Err(FileMaintenanceError::LegacyPublicationUnrepresentable);
        }
        self.publish_legacy_listing_named(storage, actor, area_id, &format!("SFFILES.{index}"))
    }

    fn publish_legacy_listing_named(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        area_id: FileAreaId,
        target_name: &str,
    ) -> Result<String, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let area = self
            .load_area_by_id(area_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        let operation_id = new_operation_id_raw("publish");
        self.begin_operation(
            &operation_id,
            "publish-listing",
            actor,
            None,
            Some(area_id),
            Some(area.state_version),
        )?;
        if let Err(error) = self.connection.execute(
            "INSERT INTO file_operation_leases(lease_kind,area_id,operation_id,expires_at) VALUES('area',?1,?2,datetime('now','+5 minutes'))",
            params![area_id.get(), operation_id],
        ) {
            self.mark_operation_needs_review(&operation_id, "area-reservation-conflict")?;
            return Err(lease_error_without_file(error));
        }
        let mut statement = self.connection.prepare(
            "SELECT filename,size_bytes,uploaded_at,description,lifecycle FROM files WHERE area_id=?1 AND lifecycle IN ('active','offline') ORDER BY normalized_filename,file_id",
        )?;
        let rows = statement.query_map(params![area_id.get()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut output = String::new();
        for row in rows {
            let (filename, size, timestamp, description, lifecycle) = row?;
            if filename.len() > 12 {
                self.mark_operation_needs_review(&operation_id, "legacy-name-width")?;
                return Err(FileMaintenanceError::LegacyPublicationUnrepresentable);
            }
            let date = chrono::DateTime::from_timestamp(timestamp, 0)
                .map(|date| date.format("%m-%d-%y").to_string())
                .unwrap_or_else(|| "00-00-00".to_owned());
            let size_field = if lifecycle == "offline" {
                " OFFLINE".to_owned()
            } else {
                format!("{:>8}", comma_number(size as u64))
            };
            let first = description.lines().next().unwrap_or_default();
            output.push_str(&format!("{filename:<13}{size_field}  {date}  {first}\r\n"));
            for continuation in description.lines().skip(1) {
                output.push_str(&format!(
                    "                                 {continuation}\r\n"
                ));
            }
        }
        drop(statement);
        let Some(encoded) = crate::encode_text(&output, crate::TerminalTextEncoding::Cp437) else {
            self.mark_operation_needs_review(&operation_id, "legacy-encoding")?;
            return Err(FileMaintenanceError::LegacyPublicationUnrepresentable);
        };
        let directory = storage.ensure_area(&area)?;
        let temporary = directory.join(format!("{target_name}.{operation_id}.part"));
        let destination = directory.join(target_name);
        self.connection.execute(
            "UPDATE file_operations SET phase='staged',staging_path=?2,digest=?3,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, format!("{}/{target_name}.{operation_id}.part", area.storage_key), format!("{:x}", Sha256::digest(&encoded))],
        )?;
        let mut staged = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(FileMaintenanceError::Io)?;
        staged
            .write_all(&encoded)
            .map_err(FileMaintenanceError::Io)?;
        staged.sync_all().map_err(FileMaintenanceError::Io)?;
        drop(staged);
        fs::rename(&temporary, &destination).map_err(FileMaintenanceError::Io)?;
        let digest = format!("{:x}", Sha256::digest(&encoded));
        self.connection.execute(
            "UPDATE file_operations SET phase='bytes-published',digest=?2,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, digest],
        )?;
        sync_directory(&directory)?;
        let generation: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(generation),0)+1 FROM file_legacy_publications WHERE area_id=?1",
            params![area_id.get()],
            |row| row.get(0),
        )?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO file_legacy_publications(area_id,generation,sha256,published_at) VALUES(?1,?2,?3,?4) ON CONFLICT(area_id) DO UPDATE SET generation=excluded.generation,sha256=excluded.sha256,state_version=file_legacy_publications.state_version+1,published_at=excluded.published_at",
            params![area_id.get(), generation, digest, Utc::now().timestamp()],
        )?;
        transaction.execute(
            "UPDATE file_operations SET phase='committed',digest=?2,completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, digest],
        )?;
        transaction.execute(
            "DELETE FROM file_operation_leases WHERE operation_id=?1",
            params![operation_id],
        )?;
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,area_id,operation_id,digest,detail) VALUES('listing-published',?1,?2,?3,?4,?5)",
            params![actor.caller_id().map(CallerId::get), area_id.get(), operation_id, digest, target_name],
        )?;
        transaction.commit()?;
        Ok(digest)
    }

    pub fn move_file(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        file_id: FileId,
        expected_file_version: u64,
        destination_area_id: FileAreaId,
        expected_destination_version: u64,
    ) -> Result<FileOperationResult, FileMaintenanceError> {
        self.move_file_with_failure(
            storage,
            actor,
            file_id,
            expected_file_version,
            destination_area_id,
            expected_destination_version,
            None,
        )
    }

    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn move_file_with_failure(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        file_id: FileId,
        expected_file_version: u64,
        destination_area_id: FileAreaId,
        expected_destination_version: u64,
        failure: Option<FailureInjectionPoint>,
    ) -> Result<FileOperationResult, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        let file = self
            .load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        let (source_root, _) = self
            .resolve_file_storage(file_id)
            .map_err(|_| FileMaintenanceError::FileUnavailable)?;
        if source_root.mode == crate::StorageRootMode::ReadOnly {
            return Err(FileMaintenanceError::ReadOnlyStorage);
        }
        if file.state_version != expected_file_version
            || file.lifecycle == FileLifecycle::Tombstoned
            || file.area_id == destination_area_id
        {
            return Err(FileMaintenanceError::StaleConflict);
        }
        self.connection.execute(
            "DELETE FROM file_active_uses WHERE expires_at<=CURRENT_TIMESTAMP",
            [],
        )?;
        let active_uses: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM file_active_uses WHERE file_id=?1",
            params![file_id.get()],
            |row| row.get(0),
        )?;
        if active_uses != 0 {
            return Err(FileMaintenanceError::FileInUse);
        }
        let source_area = self
            .load_area_by_id(file.area_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        let destination_area = self
            .load_area_by_id(destination_area_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        if destination_area.state_version != expected_destination_version {
            return Err(FileMaintenanceError::StaleConflict);
        }
        let operation_id = new_operation_id("move", file_id);
        self.begin_operation(
            &operation_id,
            "move",
            actor,
            Some(&file),
            Some(destination_area_id),
            Some(expected_destination_version),
        )?;
        if failure == Some(FailureInjectionPoint::AfterJournal) {
            return Err(FileMaintenanceError::InjectedFailure);
        }
        let source = storage.open_download(&source_area, &file)?;
        let destination_directory = storage.ensure_area(&destination_area)?;
        let staging_directory = destination_directory.join(".spitfire-ng-staging");
        create_confined_directory(&staging_directory, &destination_directory)?;
        let staged_path = staging_directory.join(format!("{operation_id}.part"));
        copy_and_verify(source, &staged_path, file.size_bytes, &file.sha256)?;
        self.connection.execute(
            "UPDATE file_operations SET phase='staged',staging_path=?2,digest=?3,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1 AND phase='planned'",
            params![operation_id, format!("{}/.spitfire-ng-staging/{}.part", destination_area.storage_key, operation_id), file.sha256],
        )?;
        if failure == Some(FailureInjectionPoint::AfterStage) {
            return Err(FileMaintenanceError::InjectedFailure);
        }
        let destination = destination_directory.join(&file.filename);
        if destination.exists() {
            self.mark_operation_needs_review(&operation_id, "destination collision")?;
            return Err(FileMaintenanceError::DestinationCollision);
        }
        fs::rename(&staged_path, &destination).map_err(FileMaintenanceError::Io)?;
        self.connection.execute(
            "UPDATE file_operations SET phase='bytes-published',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1 AND phase='staged'",
            params![operation_id],
        )?;
        sync_directory(&destination_directory)?;
        if failure == Some(FailureInjectionPoint::AfterPublish) {
            return Err(FileMaintenanceError::InjectedFailure);
        }
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET area_id=?2,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND area_id=?3 AND state_version=?4",
            params![file_id.get(), destination_area_id.get(), source_area.id.get(), expected_file_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        let locator_changed = transaction.execute(
            "UPDATE file_storage_locators SET storage_root_id=(SELECT storage_root_id FROM file_storage_roots WHERE area_id=?2 AND priority=0),relative_path=?3,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1",
            params![file_id.get(), destination_area_id.get(), file.filename],
        )?;
        if locator_changed != 1 {
            return Err(FileMaintenanceError::FileUnavailable);
        }
        let area_changed = transaction.execute(
            "UPDATE file_areas SET state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE area_id=?1 AND state_version=?2",
            params![destination_area_id.get(), expected_destination_version as i64],
        )?;
        if area_changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "UPDATE file_areas SET state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE area_id=?1",
            params![source_area.id.get()],
        )?;
        transaction.execute(
            "UPDATE file_operations SET phase='catalog-committed',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id],
        )?;
        transaction.commit()?;
        if failure == Some(FailureInjectionPoint::AfterCatalogCommit)
            || failure == Some(FailureInjectionPoint::BeforeSourceRemoval)
        {
            return Err(FileMaintenanceError::InjectedFailure);
        }
        let source_path = storage.file_path(&source_area, &file)?;
        match fs::remove_file(&source_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                self.mark_operation_needs_review(&operation_id, "source cleanup failed")?;
                return Err(FileMaintenanceError::Io(error));
            }
        }
        if let Err(error) = sync_directory(
            source_path
                .parent()
                .ok_or(FileMaintenanceError::StorageEscape)?,
        ) {
            self.mark_operation_needs_review(&operation_id, "source directory sync failed")?;
            return Err(error);
        }
        self.finish_operation(&operation_id, actor, file_id, "file-moved")?;
        Ok(FileOperationResult {
            operation_id,
            file: self
                .load_file_by_id(file_id)?
                .ok_or(FileMaintenanceError::FileUnavailable)?,
        })
    }

    pub fn remove_file(
        &mut self,
        actor: FileAdminActor,
        file_id: FileId,
        expected_version: u64,
        confirmed: bool,
    ) -> Result<FileOperationResult, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        if !confirmed {
            return Err(FileMaintenanceError::ConfirmationRequired);
        }
        let file = self
            .load_file_by_id(file_id)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        let (source_root, _) = self
            .resolve_file_storage(file_id)
            .map_err(|_| FileMaintenanceError::FileUnavailable)?;
        if source_root.mode == crate::StorageRootMode::ReadOnly {
            return Err(FileMaintenanceError::ReadOnlyStorage);
        }
        if file.state_version != expected_version || file.lifecycle == FileLifecycle::Tombstoned {
            return Err(FileMaintenanceError::StaleConflict);
        }
        self.connection.execute(
            "DELETE FROM file_active_uses WHERE expires_at<=CURRENT_TIMESTAMP",
            [],
        )?;
        let active_uses: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM file_active_uses WHERE file_id=?1",
            params![file_id.get()],
            |row| row.get(0),
        )?;
        if active_uses != 0 {
            return Err(FileMaintenanceError::FileInUse);
        }
        let operation_id = new_operation_id("remove", file_id);
        self.begin_operation(&operation_id, "remove", actor, Some(&file), None, None)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE files SET lifecycle='tombstoned',state_version=state_version+1,tombstoned_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?2 AND lifecycle<>'tombstoned'",
            params![file_id.get(), expected_version as i64],
        )?;
        if changed != 1 {
            return Err(FileMaintenanceError::StaleConflict);
        }
        transaction.execute(
            "UPDATE file_requests SET status='stale',state_version=state_version+1,resolved_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND status='pending'",
            params![file_id.get()],
        )?;
        transaction.execute(
            "UPDATE file_operations SET phase='catalog-committed',updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id],
        )?;
        transaction.commit()?;
        self.finish_operation(&operation_id, actor, file_id, "file-tombstoned")?;
        Ok(FileOperationResult {
            operation_id,
            file: self
                .load_file_by_id(file_id)?
                .ok_or(FileMaintenanceError::FileUnavailable)?,
        })
    }

    pub fn reconcile_files(
        &mut self,
        storage: &FileStorage,
        actor: FileAdminActor,
        mode: MaintenanceMode,
    ) -> Result<ReconciliationResult, FileMaintenanceError> {
        self.authorize_file_admin(actor)?;
        if mode != MaintenanceMode::Maintenance {
            return Err(FileMaintenanceError::MaintenanceModeRequired);
        }
        let operation_id = new_operation_id_raw("reconcile");
        self.begin_operation(&operation_id, "reconcile", actor, None, None, None)?;
        let catalog = self.all_cataloged_files()?;
        let mut result = ReconciliationResult {
            present: 0,
            missing: 0,
            digest_mismatch: 0,
        };
        for (area, file) in catalog {
            if file.lifecycle == FileLifecycle::Tombstoned {
                continue;
            }
            let integrity = match storage.open_download(&area, &file) {
                Ok(_) => {
                    result.present += 1;
                    FileIntegrity::Present
                }
                Err(FileError::ContentMismatch(_)) => {
                    result.digest_mismatch += 1;
                    FileIntegrity::DigestMismatch
                }
                Err(FileError::StorageIo { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    result.missing += 1;
                    FileIntegrity::Missing
                }
                Err(error) => return Err(error.into()),
            };
            self.set_integrity_observation(file.id, integrity)?;
        }
        self.connection.execute(
            "UPDATE file_operations SET phase='committed',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id],
        )?;
        self.connection.execute(
            "INSERT INTO file_events(operation,actor_caller_id,operation_id,detail) VALUES('reconciled',?1,?2,?3)",
            params![
                actor.caller_id().map(CallerId::get),
                operation_id,
                format!(
                    "present={};missing={};digest-mismatch={}",
                    result.present, result.missing, result.digest_mismatch
                )
            ],
        )?;
        Ok(result)
    }

    pub fn recover_file_operations(
        &mut self,
        storage: &FileStorage,
    ) -> Result<usize, FileMaintenanceError> {
        let mut statement = self.connection.prepare(
            "SELECT operation_id,kind,phase,file_id,source_area_id,destination_area_id,staging_path FROM file_operations WHERE phase NOT IN ('committed','rolled-back','needs-review') ORDER BY created_at,operation_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        let pending = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut classified = 0;
        for (operation_id, kind, phase, file_id, source_area, destination_area, staging_path) in
            pending
        {
            let classification = if kind == "move" {
                self.recover_move(
                    storage,
                    file_id,
                    source_area,
                    destination_area,
                    staging_path.as_deref(),
                    &phase,
                )?
            } else if matches!(phase.as_str(), "planned" | "staged" | "validated") {
                if let Some(relative) = staging_path.as_deref() {
                    if let Ok(path) = storage.confined_relative_path(relative) {
                        let _ = fs::remove_file(path);
                    }
                }
                "rolled-back"
            } else {
                "needs-review"
            };
            self.connection.execute(
                "UPDATE file_operations SET phase=?2,error_code=CASE WHEN ?2='needs-review' THEN 'recovery-ambiguous' ELSE NULL END,completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
                params![operation_id, classification],
            )?;
            self.connection.execute(
                "DELETE FROM file_operation_leases WHERE operation_id=?1",
                params![operation_id],
            )?;
            classified += 1;
        }
        Ok(classified)
    }

    fn recover_move(
        &self,
        storage: &FileStorage,
        file_id: Option<i64>,
        source_area_id: Option<i64>,
        destination_area_id: Option<i64>,
        staging_path: Option<&str>,
        phase: &str,
    ) -> Result<&'static str, FileMaintenanceError> {
        let (Some(file_id), Some(source_id), Some(destination_id)) =
            (file_id, source_area_id, destination_area_id)
        else {
            return Ok("needs-review");
        };
        let file = self
            .load_file_by_id(FileId::new(file_id)?)?
            .ok_or(FileMaintenanceError::FileUnavailable)?;
        if phase == "catalog-committed" && file.area_id.get() == destination_id {
            let source_area = self
                .load_area_by_id(FileAreaId::new(source_id)?)?
                .ok_or(FileMaintenanceError::FileUnavailable)?;
            let source_path = storage.file_path(&source_area, &file)?;
            if source_path.exists() {
                fs::remove_file(source_path).map_err(FileMaintenanceError::Io)?;
            }
            return Ok("committed");
        }
        if matches!(phase, "planned" | "staged") {
            if let Some(relative) = staging_path {
                if let Ok(path) = storage.confined_relative_path(relative) {
                    let _ = fs::remove_file(path);
                }
            }
            return Ok("rolled-back");
        }
        Ok("needs-review")
    }

    fn begin_operation(
        &mut self,
        operation_id: &str,
        kind: &str,
        actor: FileAdminActor,
        file: Option<&FileEntry>,
        destination_area: Option<FileAreaId>,
        expected_area_version: Option<u64>,
    ) -> Result<(), FileMaintenanceError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM file_operation_leases WHERE expires_at<=CURRENT_TIMESTAMP",
            [],
        )?;
        transaction.execute(
            "INSERT INTO file_operations(operation_id,kind,file_id,source_area_id,destination_area_id,expected_file_version,expected_area_version,phase,actor_caller_id) VALUES(?1,?2,?3,?4,?5,?6,?7,'planned',?8)",
            params![operation_id, kind, file.map(|value| value.id.get()), file.map(|value| value.area_id.get()), destination_area.map(FileAreaId::get), file.map(|value| value.state_version as i64), expected_area_version.map(|value| value as i64), actor.caller_id().map(CallerId::get)],
        )?;
        if let Some(file) = file {
            transaction.execute(
                "INSERT INTO file_operation_leases(lease_kind,file_id,operation_id,expires_at) VALUES('file',?1,?2,datetime('now','+5 minutes'))",
                params![file.id.get(), operation_id],
            ).map_err(|error| lease_error(error, file.id))?;
            transaction.execute(
                "INSERT INTO file_operation_leases(lease_kind,area_id,operation_id,expires_at) VALUES('area',?1,?2,datetime('now','+5 minutes'))",
                params![file.area_id.get(), operation_id],
            ).map_err(|error| lease_error(error, file.id))?;
            if let Some(destination) = destination_area {
                transaction.execute(
                    "INSERT INTO file_operation_leases(lease_kind,area_id,operation_id,expires_at) VALUES('area',?1,?2,datetime('now','+5 minutes'))",
                    params![destination.get(), operation_id],
                ).map_err(|error| lease_error(error, file.id))?;
                transaction.execute(
                    "INSERT INTO file_operation_leases(lease_kind,area_id,normalized_filename,operation_id,expires_at) VALUES('name',?1,?2,?3,datetime('now','+5 minutes'))",
                    params![destination.get(), file.filename.to_ascii_uppercase(), operation_id],
                ).map_err(|error| lease_error(error, file.id))?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    fn finish_operation(
        &mut self,
        operation_id: &str,
        actor: FileAdminActor,
        file_id: FileId,
        event: &str,
    ) -> Result<(), FileMaintenanceError> {
        let transaction = self.connection.transaction()?;
        let size_bytes = transaction
            .query_row(
                "SELECT size_bytes FROM files WHERE file_id=?1",
                params![file_id.get()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .and_then(|value| u64::try_from(value).ok());
        transaction.execute(
            "UPDATE file_operations SET phase='committed',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id],
        )?;
        transaction.execute(
            "DELETE FROM file_operation_leases WHERE operation_id=?1",
            params![operation_id],
        )?;
        transaction.execute(
            "INSERT INTO file_events(operation,actor_caller_id,file_id,operation_id) VALUES(?1,?2,?3,?4)",
            params![event, actor.caller_id().map(CallerId::get), file_id.get(), operation_id],
        )?;
        let event_code = match event {
            "file-added" => "file.added",
            "file-moved" => "file.moved",
            "file-tombstoned" => "file.removed",
            _ => "file.changed",
        };
        let mut operational = NewOperationalEvent::new(
            Utc::now().timestamp(),
            EventCategory::File,
            EventSeverity::Info,
            event_code,
            EventOutcome::Succeeded,
        );
        operational.caller_id = actor.caller_id();
        operational.correlation_id = Some(operation_id.to_owned());
        operational.idempotency_key = Some(format!("operational-{operation_id}"));
        operational.object_kind = Some("file".to_owned());
        operational.object_id = Some(file_id.get().to_string());
        operational.retention_class = RetentionClass::SummarySource;
        operational.attributes = EventAttributes::File {
            operation: event_code.to_owned(),
            bytes: size_bytes,
        };
        insert_operational_event_tx(&transaction, &operational).map_err(FileError::Database)?;
        transaction.commit()?;
        Ok(())
    }

    fn mark_operation_needs_review(
        &self,
        operation_id: &str,
        code: &str,
    ) -> Result<(), FileMaintenanceError> {
        self.connection.execute(
            "UPDATE file_operations SET phase='needs-review',error_code=?2,completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE operation_id=?1",
            params![operation_id, code],
        )?;
        self.connection.execute(
            "DELETE FROM file_operation_leases WHERE operation_id=?1",
            params![operation_id],
        )?;
        Ok(())
    }
}

fn validate_description(description: &str) -> Result<(), FileMaintenanceError> {
    if description.trim().is_empty()
        || description.len() > MAX_FILE_DESCRIPTION_BYTES
        || description.lines().count() > MAX_FILE_DESCRIPTION_LINES
        || description
            .chars()
            .any(|character| character == '\u{1b}' || character == '\0')
    {
        return Err(FileMaintenanceError::InvalidDescription);
    }
    Ok(())
}

fn split_filename(filename: &str) -> (&str, &str) {
    filename
        .rsplit_once('.')
        .map_or((filename, ""), |(base, extension)| (base, extension))
}

fn comma_number(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(character);
    }
    result
}

fn trim_version_digits(value: &str) -> &str {
    value.trim_end_matches(|character: char| character.is_ascii_digit())
}

fn normalize_denial_pattern(pattern: &str) -> Result<String, FileMaintenanceError> {
    let normalized = pattern.trim().to_ascii_uppercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'*' | b'?')
        })
    {
        return Err(FileMaintenanceError::InvalidUploadDenialPattern(
            pattern.to_owned(),
        ));
    }
    Ok(normalized)
}

fn dos_wildcard_matches(pattern: &str, value: &str) -> bool {
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

pub fn parse_legacy_sfnoup(bytes: &[u8]) -> LegacyPolicyImport {
    let text = decode_cp437(bytes);
    let mut patterns = Vec::new();
    let mut rejected_lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        match normalize_denial_pattern(line) {
            Ok(pattern) if patterns.len() < MAX_UPLOAD_DENIAL_RULES => patterns.push(pattern),
            _ => rejected_lines.push(index + 1),
        }
    }
    LegacyPolicyImport {
        patterns,
        rejected_lines,
    }
}

fn new_operation_id(kind: &str, file_id: FileId) -> String {
    format!(
        "{kind}-{}-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        file_id.get()
    )
}

fn new_operation_id_raw(kind: &str) -> String {
    format!(
        "{kind}-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

fn create_confined_directory(path: &Path, root: &Path) -> Result<(), FileMaintenanceError> {
    fs::create_dir_all(path).map_err(FileMaintenanceError::Io)?;
    let canonical = path.canonicalize().map_err(FileMaintenanceError::Io)?;
    if !canonical.starts_with(root) {
        return Err(FileMaintenanceError::StorageEscape);
    }
    Ok(())
}

fn copy_and_verify(
    mut source: File,
    destination: &Path,
    expected_size: u64,
    expected_digest: &str,
) -> Result<(), FileMaintenanceError> {
    source.rewind().map_err(FileMaintenanceError::Io)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(FileMaintenanceError::Io)?;
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = source.read(&mut buffer).map_err(FileMaintenanceError::Io)?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(FileMaintenanceError::Io)?;
        hash.update(&buffer[..read]);
        total = total.saturating_add(read as u64);
    }
    output.sync_all().map_err(FileMaintenanceError::Io)?;
    let digest = format!("{:x}", hash.finalize());
    if total != expected_size || digest != expected_digest {
        let _ = fs::remove_file(destination);
        return Err(FileMaintenanceError::DigestVerificationFailed);
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), FileMaintenanceError> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(FileMaintenanceError::Io)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn lease_error(error: rusqlite::Error, file_id: FileId) -> FileMaintenanceError {
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
        FileMaintenanceError::LeaseConflict(file_id)
    } else {
        FileMaintenanceError::Sqlite(error)
    }
}

fn lease_error_without_file(error: rusqlite::Error) -> FileMaintenanceError {
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
        FileMaintenanceError::AreaLeaseConflict
    } else {
        FileMaintenanceError::Sqlite(error)
    }
}

fn decode_text(
    file_id: FileId,
    bytes: &[u8],
    policy: TextEncodingPolicy,
) -> Result<TextInspection, FileMaintenanceError> {
    if bytes.iter().any(|byte| *byte == 0 || *byte == 0x1b)
        || bytes
            .iter()
            .filter(|byte| matches!(**byte, 0x00..=0x08 | 0x0b | 0x0e..=0x1f | 0x7f))
            .count()
            > bytes.len().saturating_div(100).max(2)
    {
        return Err(FileMaintenanceError::BinaryOrTerminalControl);
    }
    let (encoding, decoded) = match policy {
        TextEncodingPolicy::Ascii => {
            if !bytes.is_ascii() {
                return Err(FileMaintenanceError::InvalidTextEncoding);
            }
            (
                DetectedTextEncoding::Ascii,
                String::from_utf8_lossy(bytes).into_owned(),
            )
        }
        TextEncodingPolicy::Utf8 => (
            DetectedTextEncoding::Utf8,
            std::str::from_utf8(bytes)
                .map_err(|_| FileMaintenanceError::InvalidTextEncoding)?
                .to_owned(),
        ),
        TextEncodingPolicy::Cp437 => (DetectedTextEncoding::Cp437, decode_cp437(bytes)),
        TextEncodingPolicy::Auto if bytes.is_ascii() => (
            DetectedTextEncoding::Ascii,
            String::from_utf8_lossy(bytes).into_owned(),
        ),
        TextEncodingPolicy::Auto => match std::str::from_utf8(bytes) {
            Ok(value) => (DetectedTextEncoding::Utf8, value.to_owned()),
            Err(_) => (DetectedTextEncoding::Cp437, decode_cp437(bytes)),
        },
    };
    let decoded = decoded.replace("\r\n", "\n").replace('\r', "\n");
    let all_lines = decoded.split('\n').collect::<Vec<_>>();
    let truncated = all_lines.len() > MAX_TEXT_PREVIEW_LINES;
    let lines = all_lines
        .into_iter()
        .take(MAX_TEXT_PREVIEW_LINES)
        .map(sanitized_text)
        .collect();
    Ok(TextInspection {
        file_id,
        encoding,
        lines,
        truncated,
    })
}

fn declared_standard_zip_member_count(
    input: &mut File,
) -> Result<Option<usize>, FileMaintenanceError> {
    const MAX_EOCD_SEARCH: u64 = 65_557;
    let length = input
        .seek(SeekFrom::End(0))
        .map_err(FileMaintenanceError::Io)?;
    let search_bytes = length.min(MAX_EOCD_SEARCH);
    input
        .seek(SeekFrom::End(-(search_bytes as i64)))
        .map_err(FileMaintenanceError::Io)?;
    let mut tail = vec![0_u8; search_bytes as usize];
    input
        .read_exact(&mut tail)
        .map_err(FileMaintenanceError::Io)?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(FileMaintenanceError::Io)?;
    let Some(offset) = tail.windows(4).rposition(|window| window == b"PK\x05\x06") else {
        return Ok(None);
    };
    if offset + 22 > tail.len() {
        return Ok(None);
    }
    let comment_bytes = usize::from(u16::from_le_bytes([tail[offset + 20], tail[offset + 21]]));
    if offset + 22 + comment_bytes != tail.len() {
        return Ok(None);
    }
    let count = u16::from_le_bytes([tail[offset + 10], tail[offset + 11]]);
    Ok((count != u16::MAX).then_some(usize::from(count)))
}

fn decode_cp437(bytes: &[u8]) -> String {
    const EXTENDED: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ";
    let table = EXTENDED.chars().collect::<Vec<_>>();
    bytes
        .iter()
        .map(|byte| {
            if byte.is_ascii() {
                *byte as char
            } else {
                table[(*byte - 0x80) as usize]
            }
        })
        .collect()
}

fn decode_filename(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .unwrap_or_else(|_| decode_cp437(bytes))
}

fn sanitized_text(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\t' => ' ',
            character if character.is_control() => '�',
            character => character,
        })
        .collect()
}

fn safe_archive_name(raw: &[u8], enclosed: Option<PathBuf>) -> (String, bool) {
    let unsafe_path = enclosed.is_none()
        || enclosed.as_ref().is_some_and(|path| {
            path.components()
                .any(|part| !matches!(part, Component::Normal(_)))
        });
    (sanitized_text(&decode_filename(raw)), unsafe_path)
}

fn stored_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRequest> {
    let reason = match row.get::<_, String>(3)?.as_str() {
        "offline" => FileRequestReason::Offline,
        "missing" => FileRequestReason::Missing,
        value => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(FileMaintenanceError::InvalidStoredState(value.to_owned())),
            ));
        }
    };
    let status = FileRequestStatus::from_database(&row.get::<_, String>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let resolved_by: Option<i64> = row.get(10)?;
    Ok(FileRequest {
        request_id: row.get(0)?,
        file_id: FileId::new(row.get(1)?).map_err(sql_conversion)?,
        requesting_caller_id: CallerId::new(row.get(2)?).map_err(sql_conversion)?,
        reason,
        detail: row.get(4)?,
        status,
        state_version: u64::try_from(row.get::<_, i64>(6)?).map_err(sql_conversion)?,
        created_at: row.get(7)?,
        created_board_day: row.get(8)?,
        resolved_at: row.get(9)?,
        resolved_by_caller_id: resolved_by
            .map(CallerId::new)
            .transpose()
            .map_err(sql_conversion)?,
    })
}

fn sql_conversion(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Null, Box::new(error))
}

#[derive(Debug, Error)]
pub enum FileMaintenanceError {
    #[error(transparent)]
    File(#[from] FileError),
    #[error("SQLite file-maintenance operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("ZIP inspection failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("bounded inspection I/O failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("text file contains {0} bytes; maximum is {MAX_TEXT_PREVIEW_BYTES}")]
    TextTooLarge(u64),
    #[error("text file is binary or contains unsafe terminal control bytes")]
    BinaryOrTerminalControl,
    #[error("text does not match the selected encoding")]
    InvalidTextEncoding,
    #[error("archive contains {0} members; maximum is {MAX_ZIP_MEMBERS}")]
    TooManyArchiveMembers(usize),
    #[error("archive contains overlapping member data")]
    OverlappingArchiveMembers,
    #[error("archive file contains {0} bytes; maximum is {MAX_ZIP_BYTES}")]
    ArchiveTooLarge(u64),
    #[error("archive member name exceeds the supported bound")]
    ArchiveMemberNameTooLong,
    #[error("archive metadata exceeds the supported bound")]
    ArchiveMetadataTooLarge,
    #[error("archive central-directory entry count is inconsistent")]
    ArchiveDirectoryInconsistent,
    #[error("archive declared uncompressed size exceeds the supported bound")]
    ArchiveDeclaredSizeTooLarge,
    #[error("archive inspection exceeded its deadline")]
    InspectionDeadline,
    #[error("archive is unsafe for bounded inspection")]
    UnsafeArchive,
    #[error("encrypted archive members are not supported for inspection")]
    EncryptedArchiveMember,
    #[error("archive compression method is not supported by this build")]
    UnsupportedArchiveCompression,
    #[error("archive contains multiple FILE_ID.DIZ candidates")]
    AmbiguousFileIdDiz,
    #[error("FILE_ID.DIZ exceeds its content or expansion bound")]
    FileIdDizTooLarge,
    #[error("file is unavailable for inspection")]
    FileUnavailable,
    #[error("file is not offline or missing and cannot be requested")]
    FileNotRequestable,
    #[error("file-request detail exceeds the supported bound")]
    RequestDetailTooLong,
    #[error("invalid board-local request date")]
    InvalidBoardDay,
    #[error("session identifier exceeds SQLite range")]
    InvalidSessionId,
    #[error("caller file-request limit reached")]
    RequestLimitReached,
    #[error("file request {0} does not exist")]
    RequestNotFound(i64),
    #[error("request transition is not valid")]
    InvalidRequestTransition,
    #[error("operation conflicts with a newer authoritative version")]
    StaleConflict,
    #[error("operator authorization denied")]
    AuthorizationDenied,
    #[error("database contains invalid file-maintenance state {0:?}")]
    InvalidStoredState(String),
    #[error("file description is empty, oversized, or unsafe")]
    InvalidDescription,
    #[error("file lifecycle transition is not permitted by this command")]
    InvalidLifecycleTransition,
    #[error("reviewed FILE_ID.DIZ does not belong to the selected archive")]
    FileIdDizMismatch,
    #[error("upload-denial policy exceeds {MAX_UPLOAD_DENIAL_RULES} rules")]
    TooManyUploadDenials,
    #[error("invalid upload-denial pattern {0:?}")]
    InvalidUploadDenialPattern(String),
    #[error("destination already contains the selected filename")]
    DestinationCollision,
    #[error("destructive file maintenance requires explicit confirmation")]
    ConfirmationRequired,
    #[error("deep file reconciliation requires maintenance mode")]
    MaintenanceModeRequired,
    #[error("file-operation lease is already held for file {0:?}")]
    LeaseConflict(FileId),
    #[error("file-area or filename reservation is already held")]
    AreaLeaseConflict,
    #[error("file has an active transfer or inspection use")]
    FileInUse,
    #[error("staged bytes failed size or digest verification")]
    DigestVerificationFailed,
    #[error("file-maintenance path escaped managed storage")]
    StorageEscape,
    #[error("the file is owned by read-only storage")]
    ReadOnlyStorage,
    #[error("native metadata cannot be represented safely in a historical SFFILES publication")]
    LegacyPublicationUnrepresentable,
    #[error("synthetic failure injected for recovery testing")]
    InjectedFailure,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallerState, CredentialHasher, FileAccessMode, FileAreaDefinition, FileBackend,
        LogicalPaths, PasswordHashConfig, RuntimeConfig, SecurityLevel, SessionId,
    };
    use std::io::{Cursor, Write};

    struct Fixture {
        _temp: tempfile::TempDir,
        database: RuntimeDatabase,
        storage: FileStorage,
        full: FileActor,
        preview: FileActor,
        area: FileArea,
    }

    fn fixture() -> Fixture {
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
        let hash = hasher.hash(b"synthetic file-maintenance password").unwrap();
        let sysop = database
            .create_caller(
                b"Synthetic File Sysop",
                &hash,
                SecurityLevel::new(100).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let preview = database
            .create_caller(
                b"Synthetic Preview Caller",
                &hash,
                SecurityLevel::new(5).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let area = database
            .create_file_area(&FileAreaDefinition {
                number: 1,
                name: "Synthetic Files".to_owned(),
                description: "Synthetic test area".to_owned(),
                storage_key: "synthetic".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(50).unwrap(),
                upload_security: SecurityLevel::new(50).unwrap(),
                preview: true,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        Fixture {
            _temp: temp,
            database,
            storage: FileStorage::new(&paths).unwrap(),
            full: FileActor::new(sysop.id, SecurityLevel::new(50).unwrap()),
            preview: FileActor::new(preview.id, SecurityLevel::new(50).unwrap()),
            area,
        }
    }

    fn destination_area(fixture: &mut Fixture, number: u16, key: &str) -> FileArea {
        let area = fixture
            .database
            .create_file_area(&FileAreaDefinition {
                number,
                name: format!("Destination {number}"),
                description: "Synthetic failure-matrix destination".to_owned(),
                storage_key: key.to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(1).unwrap(),
                upload_security: SecurityLevel::new(1).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        fixture.storage.ensure_area(&area).unwrap();
        area
    }

    fn additional_actor(fixture: &mut Fixture, name: &str, security: u16) -> FileActor {
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher
            .hash(b"synthetic additional caller password")
            .unwrap();
        let caller = fixture
            .database
            .create_caller(
                name.as_bytes(),
                &hash,
                SecurityLevel::new(security).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        FileActor::new(caller.id, SecurityLevel::new(50).unwrap())
    }

    fn latest_operation_phase(fixture: &Fixture, file_id: FileId) -> String {
        fixture
            .database
            .connection
            .query_row(
                "SELECT phase FROM file_operations WHERE file_id=?1 ORDER BY created_at DESC,operation_id DESC LIMIT 1",
                params![file_id.get()],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn empty_zip() -> Vec<u8> {
        zip::ZipWriter::new(Cursor::new(Vec::new()))
            .finish()
            .unwrap()
            .into_inner()
    }

    fn zip_with_empty_members(count: usize, long_names: bool, comment: bool) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        if comment {
            writer.set_raw_comment(vec![b'x'].into()).unwrap();
        }
        for index in 0..count {
            let name = if long_names {
                let prefix = format!("{index:04}-");
                format!(
                    "{prefix}{}",
                    "n".repeat(MAX_ZIP_MEMBER_NAME_BYTES - prefix.len())
                )
            } else {
                format!("member-{index:04}.txt")
            };
            writer.start_file(name, options).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn patch_zip_flags(bytes: &mut [u8], flag: u16) {
        let mut offset = 0;
        while offset + 10 <= bytes.len() {
            let flags_offset = if bytes[offset..].starts_with(b"PK\x03\x04") {
                Some(offset + 6)
            } else if bytes[offset..].starts_with(b"PK\x01\x02") {
                Some(offset + 8)
            } else {
                None
            };
            if let Some(flags_offset) = flags_offset {
                bytes[flags_offset..flags_offset + 2].copy_from_slice(&flag.to_le_bytes());
            }
            offset += 1;
        }
    }

    fn patch_central_uncompressed_sizes(bytes: &mut [u8], size: u32) {
        let mut offset = 0;
        while offset + 28 <= bytes.len() {
            if bytes[offset..].starts_with(b"PK\x01\x02") {
                bytes[offset + 24..offset + 28].copy_from_slice(&size.to_le_bytes());
            }
            offset += 1;
        }
    }

    fn patch_zip_compression(bytes: &mut [u8], method: u16) {
        let mut offset = 0;
        while offset + 12 <= bytes.len() {
            let method_offset = if bytes[offset..].starts_with(b"PK\x03\x04") {
                Some(offset + 8)
            } else if bytes[offset..].starts_with(b"PK\x01\x02") {
                Some(offset + 10)
            } else {
                None
            };
            if let Some(method_offset) = method_offset {
                bytes[method_offset..method_offset + 2].copy_from_slice(&method.to_le_bytes());
            }
            offset += 1;
        }
    }

    fn patch_zip_timestamp(bytes: &mut [u8], time: u16, date: u16) {
        let mut offset = 0;
        while offset + 16 <= bytes.len() {
            let timestamp_offset = if bytes[offset..].starts_with(b"PK\x03\x04") {
                Some(offset + 10)
            } else if bytes[offset..].starts_with(b"PK\x01\x02") {
                Some(offset + 12)
            } else {
                None
            };
            if let Some(timestamp_offset) = timestamp_offset {
                bytes[timestamp_offset..timestamp_offset + 2].copy_from_slice(&time.to_le_bytes());
                bytes[timestamp_offset + 2..timestamp_offset + 4]
                    .copy_from_slice(&date.to_le_bytes());
            }
            offset += 1;
        }
    }

    fn patch_first_member_name(
        bytes: &mut [u8],
        replacement: u8,
        mut local: bool,
        mut central: bool,
    ) {
        let mut offset = 0;
        while offset + 47 <= bytes.len() {
            if local && bytes[offset..].starts_with(b"PK\x03\x04") {
                bytes[offset + 30] = replacement;
                local = false;
            } else if central && bytes[offset..].starts_with(b"PK\x01\x02") {
                bytes[offset + 46] = replacement;
                central = false;
            }
            offset += 1;
        }
    }

    fn zip64_bytes() -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        writer.start_file("zip64.txt", options).unwrap();
        writer.write_all(b"small synthetic ZIP64 member").unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn make_duplicate_member_names(bytes: &mut [u8]) {
        let mut local_count = 0;
        let mut central_count = 0;
        let mut offset = 0;
        while offset + 53 <= bytes.len() {
            if bytes[offset..].starts_with(b"PK\x03\x04") {
                local_count += 1;
                if local_count == 2 {
                    bytes[offset + 30..offset + 37].copy_from_slice(b"one.txt");
                }
            } else if bytes[offset..].starts_with(b"PK\x01\x02") {
                central_count += 1;
                if central_count == 2 {
                    bytes[offset + 46..offset + 53].copy_from_slice(b"one.txt");
                }
            }
            offset += 1;
        }
        assert_eq!((local_count, central_count), (2, 2));
    }

    #[test]
    fn preview_actor_can_inspect_bounded_text_but_controls_and_binary_are_rejected() {
        let mut fixture = fixture();
        let text = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "README.TXT",
                "Safe text",
                b"Hello\r\nCP437: \x82\r\n",
                1_700_000_000,
            )
            .unwrap();
        let inspection = fixture
            .database
            .inspect_text_file(
                &fixture.storage,
                fixture.preview,
                text.id,
                TextEncodingPolicy::Auto,
            )
            .unwrap();
        assert_eq!(inspection.encoding, DetectedTextEncoding::Cp437);
        assert!(inspection.lines.iter().any(|line| line.contains('é')));

        let unsafe_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ESCAPE.TXT",
                "Unsafe control fixture",
                b"hello\x1b[2J",
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_text_file(
                &fixture.storage,
                fixture.preview,
                unsafe_file.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::BinaryOrTerminalControl)
        ));
    }

    #[test]
    fn zip_listing_is_metadata_only_and_diz_is_bounded_review_input() {
        let mut fixture = fixture();
        let bytes = zip_bytes(&[
            ("docs/FILE_ID.DIZ", b"First line\r\nSecond line"),
            ("../unsafe.txt", b"not extracted"),
        ]);
        let archive = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ARCHIVE.ZIP",
                "Synthetic archive",
                &bytes,
                1_700_000_000,
            )
            .unwrap();
        let listing = fixture
            .database
            .inspect_zip_file(&fixture.storage, fixture.preview, archive.id)
            .unwrap();
        assert_eq!(listing.members.len(), 2);
        assert!(listing.members.iter().any(|member| member.unsafe_path));
        assert!(!fixture._temp.path().join("unsafe.txt").exists());
        let diz = fixture
            .database
            .inspect_file_id_diz(
                &fixture.storage,
                fixture.full,
                archive.id,
                TextEncodingPolicy::Auto,
            )
            .unwrap()
            .unwrap();
        assert_eq!(diz.text.lines, ["First line", "Second line"]);
        let applied = fixture
            .database
            .apply_reviewed_file_id_diz(
                FileAdminActor::ThresholdSysop(fixture.full),
                archive.id,
                archive.state_version + 1,
                &diz,
            )
            .unwrap();
        assert_eq!(applied.description_source, "file-id-diz");
    }

    #[test]
    fn malformed_oversized_and_bomb_like_inputs_fail_closed() {
        let mut fixture = fixture();
        let malformed = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "BROKEN.ZIP",
                "Malformed archive",
                b"not a zip archive",
                1_700_000_000,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, malformed.id),
            Err(FileMaintenanceError::Zip(_))
        ));

        let oversized_bytes = vec![b'x'; MAX_TEXT_PREVIEW_BYTES as usize + 1];
        let oversized = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "LARGE.TXT",
                "Oversized text",
                &oversized_bytes,
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_text_file(
                &fixture.storage,
                fixture.preview,
                oversized.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::TextTooLarge(_))
        ));

        let expanded = vec![b'A'; MAX_DIZ_BYTES as usize];
        let bomb = zip_bytes(&[("FILE_ID.DIZ", &expanded)]);
        let bomb_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "BOMB.ZIP",
                "Expansion ratio fixture",
                &bomb,
                1_700_000_002,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_file_id_diz(
                &fixture.storage,
                fixture.preview,
                bomb_file.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::FileIdDizTooLarge)
        ));

        let duplicate = zip_bytes(&[("FILE_ID.DIZ", b"one"), ("docs/file_id.diz", b"two")]);
        let duplicate_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DUPDIZ.ZIP",
                "Duplicate DIZ fixture",
                &duplicate,
                1_700_000_003,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_file_id_diz(
                &fixture.storage,
                fixture.preview,
                duplicate_file.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::AmbiguousFileIdDiz)
        ));
    }

    #[test]
    fn archive_failure_matrix_is_metadata_only_bounded_and_deterministic() {
        let mut fixture = fixture();
        let cases = [
            ("EMPTY.ZIP", empty_zip(), 0),
            (
                "MULTI.ZIP",
                zip_bytes(&[("one.txt", b"one"), ("two.txt", b"two")]),
                2,
            ),
            ("LONGNAME.ZIP", zip_with_empty_members(1, true, false), 1),
            (
                "TRAVERSE.ZIP",
                zip_bytes(&[("../../host-secret.txt", b"not read")]),
                1,
            ),
        ];
        for (filename, bytes, member_count) in cases {
            let file = fixture
                .storage
                .write_seed_file(
                    &mut fixture.database,
                    &fixture.area,
                    filename,
                    "Archive acceptance fixture",
                    &bytes,
                    1_700_000_100,
                )
                .unwrap();
            let inspected = fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, file.id)
                .unwrap();
            assert_eq!(inspected.members.len(), member_count);
            if filename == "TRAVERSE.ZIP" {
                assert!(inspected.members[0].unsafe_path);
                assert!(!fixture._temp.path().join("host-secret.txt").exists());
            }
        }

        let high_ratio = zip_bytes(&[("compressible.txt", &vec![b'A'; 1024 * 1024])]);
        let high_ratio_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "RATIO.ZIP",
                "Metadata-only compression ratio fixture",
                &high_ratio,
                1_700_000_101,
            )
            .unwrap();
        let ratio_listing = fixture
            .database
            .inspect_zip_file(&fixture.storage, fixture.preview, high_ratio_file.id)
            .unwrap();
        assert_eq!(ratio_listing.members[0].uncompressed_bytes, 1024 * 1024);
        assert!(ratio_listing.members[0].compressed_bytes < 16 * 1024);

        let near_limit = zip_with_empty_members(MAX_ZIP_MEMBERS, false, false);
        let near_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "NEARLIMIT.ZIP",
                "Near member limit",
                &near_limit,
                1_700_000_102,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, near_file.id)
                .unwrap()
                .members
                .len(),
            MAX_ZIP_MEMBERS
        );

        let over_limit = zip_with_empty_members(MAX_ZIP_MEMBERS + 1, false, false);
        let over_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "OVERLIMIT.ZIP",
                "Over member limit",
                &over_limit,
                1_700_000_103,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, over_file.id),
            Err(FileMaintenanceError::TooManyArchiveMembers(count))
                if count == MAX_ZIP_MEMBERS + 1
        ));

        let metadata_heavy = zip_with_empty_members(MAX_ZIP_MEMBERS, true, true);
        let metadata_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "METADATA.ZIP",
                "Metadata limit",
                &metadata_heavy,
                1_700_000_104,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, metadata_file.id),
            Err(FileMaintenanceError::ArchiveMetadataTooLarge)
        ));

        let mut declared = zip_bytes(&[("one.bin", b"1"), ("two.bin", b"2")]);
        patch_central_uncompressed_sizes(&mut declared, u32::MAX);
        let declared_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DECLARED.ZIP",
                "Declared size limit",
                &declared,
                1_700_000_105,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, declared_file.id),
            Err(FileMaintenanceError::ArchiveDeclaredSizeTooLarge)
        ));

        let mut encrypted = zip_bytes(&[("secret.txt", b"encrypted-flag fixture")]);
        patch_zip_flags(&mut encrypted, 1);
        let encrypted_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ENCRYPT.ZIP",
                "Encrypted member limit",
                &encrypted,
                1_700_000_106,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, encrypted_file.id),
            Err(FileMaintenanceError::EncryptedArchiveMember)
        ));

        let timeout_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "TIMEOUT.ZIP",
                "Deadline fixture",
                &zip_bytes(&[("one.txt", b"one")]),
                1_700_000_107,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_zip_file_with_deadline(
                &fixture.storage,
                fixture.preview,
                timeout_file.id,
                Duration::ZERO
            ),
            Err(FileMaintenanceError::InspectionDeadline)
        ));

        let oversized_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ARCHSIZE.ZIP",
                "Archive size fixture",
                &empty_zip(),
                1_700_000_108,
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "UPDATE files SET size_bytes=?2 WHERE file_id=?1",
                params![oversized_file.id.get(), (MAX_ZIP_BYTES + 1) as i64],
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_zip_file(
                &fixture.storage,
                fixture.preview,
                oversized_file.id
            ),
            Err(FileMaintenanceError::ArchiveTooLarge(size)) if size == MAX_ZIP_BYTES + 1
        ));
    }

    #[test]
    fn malformed_truncated_and_diz_failure_matrix_fails_closed() {
        let mut fixture = fixture();
        let mut malformed_central = zip_bytes(&[("one.txt", b"one")]);
        let central = malformed_central
            .windows(4)
            .position(|bytes| bytes == b"PK\x01\x02")
            .unwrap();
        malformed_central[central] = b'X';
        let mut truncated = zip_bytes(&[("one.txt", b"one")]);
        truncated.truncate(truncated.len() - 12);
        for (filename, bytes) in [
            ("CENTRAL.ZIP", malformed_central),
            ("TRUNCATE.ZIP", truncated),
        ] {
            let file = fixture
                .storage
                .write_seed_file(
                    &mut fixture.database,
                    &fixture.area,
                    filename,
                    "Malformed archive fixture",
                    &bytes,
                    1_700_000_200,
                )
                .unwrap();
            assert!(matches!(
                fixture
                    .database
                    .inspect_zip_file(&fixture.storage, fixture.preview, file.id),
                Err(FileMaintenanceError::Zip(_))
            ));
        }

        let oversized_diz = vec![b'Z'; MAX_DIZ_BYTES as usize + 1];
        let oversized_archive = zip_bytes(&[("FILE_ID.DIZ", &oversized_diz)]);
        let oversized = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DIZSIZE.ZIP",
                "Oversized DIZ fixture",
                &oversized_archive,
                1_700_000_201,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_file_id_diz(
                &fixture.storage,
                fixture.preview,
                oversized.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::FileIdDizTooLarge)
        ));

        let mut encrypted_diz = zip_bytes(&[("FILE_ID.DIZ", b"not disclosed")]);
        patch_zip_flags(&mut encrypted_diz, 1);
        let encrypted = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DIZCRYPT.ZIP",
                "Encrypted DIZ fixture",
                &encrypted_diz,
                1_700_000_202,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_file_id_diz(
                &fixture.storage,
                fixture.preview,
                encrypted.id,
                TextEncodingPolicy::Auto
            ),
            Err(FileMaintenanceError::EncryptedArchiveMember)
        ));
    }

    #[test]
    fn zip64_names_duplicates_codecs_and_aliasing_are_deterministic() {
        let mut fixture = fixture();

        let zip64 = zip64_bytes();
        assert!(zip64.windows(2).any(|window| window == [0x01, 0x00]));
        let zip64_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ZIP64.ZIP",
                "Synthetic in-bounds ZIP64 fixture",
                &zip64,
                1_700_000_300,
            )
            .unwrap();
        let inspected = fixture
            .database
            .inspect_zip_file(&fixture.storage, fixture.preview, zip64_file.id)
            .unwrap();
        assert_eq!(inspected.members.len(), 1);
        assert_eq!(inspected.members[0].filename, "zip64.txt");

        let mut cp437_name = zip_bytes(&[("x", b"name")]);
        patch_first_member_name(&mut cp437_name, 0x82, true, true);
        let cp437_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "CP437.ZIP",
                "CP437 filename fixture",
                &cp437_name,
                1_700_000_301,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, cp437_file.id)
                .unwrap()
                .members[0]
                .filename,
            "é"
        );

        let mut duplicate_names = zip_bytes(&[("one.txt", b"one"), ("two.txt", b"two")]);
        make_duplicate_member_names(&mut duplicate_names);
        let duplicate_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DUPNAME.ZIP",
                "Duplicate ordinary names",
                &duplicate_names,
                1_700_000_302,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, duplicate_file.id),
            Err(FileMaintenanceError::ArchiveDirectoryInconsistent)
        ));

        let mut unsupported = zip_bytes(&[("unsupported.bin", b"bytes")]);
        patch_zip_compression(&mut unsupported, 98);
        let unsupported_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "CODEC.ZIP",
                "Unsupported compression fixture",
                &unsupported,
                1_700_000_303,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_zip_file(
                &fixture.storage,
                fixture.preview,
                unsupported_file.id
            ),
            Err(FileMaintenanceError::UnsupportedArchiveCompression)
        ));

        let mut aliased = zip_bytes(&[("one.txt", b"one"), ("two.txt", b"two")]);
        let central_offsets = aliased
            .windows(4)
            .enumerate()
            .filter_map(|(offset, signature)| (signature == b"PK\x01\x02").then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(central_offsets.len(), 2);
        aliased[central_offsets[1] + 42..central_offsets[1] + 46]
            .copy_from_slice(&0_u32.to_le_bytes());
        let aliased_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ALIAS.ZIP",
                "Aliased local-header fixture",
                &aliased,
                1_700_000_304,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, aliased_file.id),
            Err(FileMaintenanceError::OverlappingArchiveMembers)
                | Err(FileMaintenanceError::Zip(_))
        ));

        let mut disagreement = zip_bytes(&[("a", b"bytes")]);
        patch_first_member_name(&mut disagreement, b'b', true, false);
        let disagreement_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DISAGREE.ZIP",
                "Local and central name disagreement",
                &disagreement,
                1_700_000_305,
            )
            .unwrap();
        let result = fixture.database.inspect_zip_file(
            &fixture.storage,
            fixture.preview,
            disagreement_file.id,
        );
        assert!(result.is_ok() || matches!(result, Err(FileMaintenanceError::Zip(_))));

        let mut invalid_unicode = zip_bytes(&[("x", b"bytes")]);
        patch_first_member_name(&mut invalid_unicode, 0x82, true, true);
        patch_zip_flags(&mut invalid_unicode, 1 << 11);
        let invalid_unicode_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "UNICODE.ZIP",
                "Malformed UTF-8 filename flag",
                &invalid_unicode,
                1_700_000_306,
            )
            .unwrap();
        let result = fixture.database.inspect_zip_file(
            &fixture.storage,
            fixture.preview,
            invalid_unicode_file.id,
        );
        assert!(result.is_ok() || matches!(result, Err(FileMaintenanceError::Zip(_))));
        if let Ok(listing) = result {
            assert!(!listing.members[0].filename.chars().any(char::is_control));
        }

        let mut empty_edge = zip_bytes(&[("x", b"bytes")]);
        patch_first_member_name(&mut empty_edge, 0, true, true);
        let empty_edge_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "EMPTYNAME.ZIP",
                "Embedded-NUL filename edge",
                &empty_edge,
                1_700_000_307,
            )
            .unwrap();
        let result = fixture.database.inspect_zip_file(
            &fixture.storage,
            fixture.preview,
            empty_edge_file.id,
        );
        assert!(result.is_ok() || matches!(result, Err(FileMaintenanceError::Zip(_))));
        if let Ok(listing) = result {
            assert!(!listing.members[0].filename.contains('\0'));
        }

        let mut odd_timestamp = zip_bytes(&[("odd-time.txt", b"bytes")]);
        patch_zip_timestamp(&mut odd_timestamp, u16::MAX, u16::MAX);
        let odd_timestamp_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "ODDTIME.ZIP",
                "Odd timestamp fixture",
                &odd_timestamp,
                1_700_000_308,
            )
            .unwrap();
        let result = fixture.database.inspect_zip_file(
            &fixture.storage,
            fixture.preview,
            odd_timestamp_file.id,
        );
        assert!(result.is_ok() || matches!(result, Err(FileMaintenanceError::Zip(_))));

        let mut malformed_extra = zip_bytes(&[("extra.txt", b"bytes")]);
        let central = malformed_extra
            .windows(4)
            .position(|signature| signature == b"PK\x01\x02")
            .unwrap();
        malformed_extra[central + 30..central + 32].copy_from_slice(&u16::MAX.to_le_bytes());
        let malformed_extra_file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "EXTRA.ZIP",
                "Malformed extra-field length",
                &malformed_extra,
                1_700_000_309,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.inspect_zip_file(
                &fixture.storage,
                fixture.preview,
                malformed_extra_file.id
            ),
            Err(FileMaintenanceError::Zip(_))
        ));
    }

    #[test]
    fn text_encoding_line_endings_controls_and_exact_bounds_are_safe() {
        let file_id = FileId::new(1).unwrap();
        for (bytes, expected) in [
            (b"one\ntwo".as_slice(), vec!["one", "two"]),
            (b"one\rtwo".as_slice(), vec!["one", "two"]),
            (b"one\r\ntwo".as_slice(), vec!["one", "two"]),
            (b"x".as_slice(), vec!["x"]),
            (b"".as_slice(), vec![""]),
        ] {
            assert_eq!(
                decode_text(file_id, bytes, TextEncodingPolicy::Auto)
                    .unwrap()
                    .lines,
                expected
            );
        }

        let invalid_utf8 = b"caf\x82";
        assert_eq!(
            decode_text(file_id, invalid_utf8, TextEncodingPolicy::Auto)
                .unwrap()
                .encoding,
            DetectedTextEncoding::Cp437
        );
        assert!(matches!(
            decode_text(file_id, invalid_utf8, TextEncodingPolicy::Utf8),
            Err(FileMaintenanceError::InvalidTextEncoding)
        ));
        assert_eq!(
            decode_text(file_id, "snowman: ☃".as_bytes(), TextEncodingPolicy::Auto)
                .unwrap()
                .encoding,
            DetectedTextEncoding::Utf8
        );

        for unsafe_bytes in [
            b"text\0tail".as_slice(),
            b"text\x1b[2J".as_slice(),
            b"text\x1b]0;title\x07".as_slice(),
            b"text\x1bPprivate\x1b\\".as_slice(),
        ] {
            assert!(matches!(
                decode_text(file_id, unsafe_bytes, TextEncodingPolicy::Auto),
                Err(FileMaintenanceError::BinaryOrTerminalControl)
            ));
        }
        let sanitized = decode_text(
            file_id,
            "left\u{0085}right".as_bytes(),
            TextEncodingPolicy::Utf8,
        )
        .unwrap();
        assert_eq!(sanitized.lines, ["left�right"]);
        let sanitized_ascii =
            decode_text(file_id, b"left\x01right\x7f", TextEncodingPolicy::Auto).unwrap();
        assert_eq!(sanitized_ascii.lines, ["left�right�"]);

        let exact_lines = "x\n".repeat(MAX_TEXT_PREVIEW_LINES - 1);
        let exact = decode_text(file_id, exact_lines.as_bytes(), TextEncodingPolicy::Auto).unwrap();
        assert_eq!(exact.lines.len(), MAX_TEXT_PREVIEW_LINES);
        assert!(!exact.truncated);
        let over_lines = format!("{exact_lines}x\n");
        let over = decode_text(file_id, over_lines.as_bytes(), TextEncodingPolicy::Auto).unwrap();
        assert_eq!(over.lines.len(), MAX_TEXT_PREVIEW_LINES);
        assert!(over.truncated);

        let long_line = vec![b'x'; MAX_TEXT_PREVIEW_BYTES as usize];
        let long = decode_text(file_id, &long_line, TextEncodingPolicy::Auto).unwrap();
        assert_eq!(long.lines[0].len(), MAX_TEXT_PREVIEW_BYTES as usize);
        assert!(!long.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn inspection_entry_rejects_symlinks_and_special_files() {
        use std::os::unix::fs::symlink;

        let mut fixture = fixture();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "SPECIAL.TXT",
                "Special file fixture",
                b"safe original",
                1_700_000_400,
            )
            .unwrap();
        let path = fixture.storage.file_path(&fixture.area, &file).unwrap();
        fs::remove_file(&path).unwrap();
        symlink("../outside", &path).unwrap();
        assert!(fixture
            .database
            .inspect_text_file(
                &fixture.storage,
                fixture.preview,
                file.id,
                TextEncodingPolicy::Auto
            )
            .is_err());

        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(fixture
            .database
            .inspect_text_file(
                &fixture.storage,
                fixture.preview,
                file.id,
                TextEncodingPolicy::Auto
            )
            .is_err());
    }

    #[test]
    fn requests_are_private_idempotent_and_versioned() {
        let mut fixture = fixture();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "OFFLINE.ZIP",
                "Offline synthetic file",
                b"bytes",
                1_700_000_000,
            )
            .unwrap();
        let offline = fixture
            .database
            .set_file_lifecycle(
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                FileLifecycle::Offline,
            )
            .unwrap();
        let first = fixture
            .database
            .create_file_request(fixture.preview, offline.id, None)
            .unwrap();
        let duplicate = fixture
            .database
            .create_file_request(fixture.preview, offline.id, None)
            .unwrap();
        assert_eq!(first.request_id, duplicate.request_id);
        let resolved = fixture
            .database
            .resolve_file_request(
                FileAdminActor::LocalOperator,
                first.request_id,
                first.state_version,
                FileRequestStatus::Fulfilled,
            )
            .unwrap();
        assert_eq!(resolved.status, FileRequestStatus::Fulfilled);
        assert!(matches!(
            fixture.database.resolve_file_request(
                FileAdminActor::LocalOperator,
                first.request_id,
                first.state_version,
                FileRequestStatus::Rejected
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
    }

    #[test]
    fn slash_upload_is_pending_review_and_hidden_until_acceptance() {
        let mut fixture = fixture();
        let mut staged = fixture
            .storage
            .begin_upload(SessionId::new(1).unwrap(), "REVIEW.TXT")
            .unwrap();
        staged.write_all(b"review me").unwrap();
        let pending = fixture
            .database
            .commit_upload(
                &fixture.storage,
                staged,
                fixture.full,
                &fixture.area,
                "/ private review",
                1_700_000_000,
            )
            .unwrap();
        assert_eq!(pending.lifecycle, FileLifecycle::PendingReview);
        assert!(fixture
            .database
            .files(fixture.full, fixture.area.id)
            .unwrap()
            .is_empty());
        let accepted = fixture
            .database
            .review_pending_file(
                FileAdminActor::LocalOperator,
                pending.id,
                pending.state_version,
                true,
            )
            .unwrap();
        assert_eq!(accepted.lifecycle, FileLifecycle::Active);
    }

    #[test]
    fn request_dedup_multi_caller_races_and_lifecycle_changes_are_bounded() {
        let mut fixture = fixture();
        let second = additional_actor(&mut fixture, "Synthetic Second Requester", 5);
        let offline = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "REQUESTS.ZIP",
                "Request state fixture",
                b"unavailable bytes",
                1_700_000_500,
            )
            .unwrap();
        let offline = fixture
            .database
            .set_file_lifecycle(
                FileAdminActor::LocalOperator,
                offline.id,
                offline.state_version,
                FileLifecycle::Offline,
            )
            .unwrap();
        let first = fixture
            .database
            .create_file_request(fixture.preview, offline.id, Some("first"))
            .unwrap();
        assert_eq!(
            fixture
                .database
                .create_file_request(fixture.preview, offline.id, Some("repeat"))
                .unwrap()
                .request_id,
            first.request_id
        );
        let second_request = fixture
            .database
            .create_file_request(second, offline.id, None)
            .unwrap();
        assert_ne!(first.request_id, second_request.request_id);

        let cancelled = fixture
            .database
            .cancel_own_file_request(fixture.preview, first.request_id, first.state_version)
            .unwrap();
        assert_eq!(cancelled.status, FileRequestStatus::Cancelled);
        assert!(matches!(
            fixture.database.resolve_file_request(
                FileAdminActor::LocalOperator,
                first.request_id,
                first.state_version,
                FileRequestStatus::Fulfilled
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));

        let removed = fixture
            .database
            .remove_file(
                FileAdminActor::LocalOperator,
                offline.id,
                offline.state_version,
                true,
            )
            .unwrap();
        assert_eq!(removed.file.lifecycle, FileLifecycle::Tombstoned);
        let remaining = fixture
            .database
            .file_request(second_request.request_id)
            .unwrap();
        assert_eq!(remaining.status, FileRequestStatus::Stale);
        assert!(matches!(
            fixture.database.resolve_file_request(
                FileAdminActor::LocalOperator,
                second_request.request_id,
                second_request.state_version,
                FileRequestStatus::Fulfilled
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));

        fixture
            .database
            .connection
            .execute(
                "UPDATE callers SET account_state='disabled' WHERE caller_id=?1",
                params![second.caller_id().get()],
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .create_file_request(second, offline.id, None),
            Err(FileMaintenanceError::File(FileError::CallerUnavailable))
        ));
    }

    #[test]
    fn pending_review_is_caller_invisible_but_sysop_inspectable_and_cas_reviewed() {
        let mut fixture = fixture();
        let archive_bytes = zip_bytes(&[("FILE_ID.DIZ", b"Reviewed description")]);
        let mut staged = fixture
            .storage
            .begin_upload(SessionId::new(11).unwrap(), "PENDING.ZIP")
            .unwrap();
        staged.write_all(&archive_bytes).unwrap();
        let pending = fixture
            .database
            .commit_upload(
                &fixture.storage,
                staged,
                fixture.full,
                &fixture.area,
                "/ private review",
                1_700_000_510,
            )
            .unwrap();

        assert!(fixture
            .database
            .files(fixture.preview, fixture.area.id)
            .unwrap()
            .is_empty());
        assert!(matches!(
            fixture
                .database
                .inspect_zip_file(&fixture.storage, fixture.preview, pending.id),
            Err(FileMaintenanceError::FileUnavailable)
        ));
        assert!(matches!(
            fixture
                .database
                .create_file_request(fixture.preview, pending.id, None),
            Err(FileMaintenanceError::FileNotRequestable)
        ));
        assert!(matches!(
            fixture.database.begin_file_download_use(
                fixture.preview,
                pending.id,
                SessionId::new(12).unwrap()
            ),
            Err(FileMaintenanceError::AuthorizationDenied)
        ));

        let diz = fixture
            .database
            .inspect_file_id_diz(
                &fixture.storage,
                fixture.full,
                pending.id,
                TextEncodingPolicy::Auto,
            )
            .unwrap()
            .unwrap();
        let inspected = fixture
            .database
            .load_file_by_id(pending.id)
            .unwrap()
            .unwrap();
        let edited = fixture
            .database
            .apply_reviewed_file_id_diz(
                FileAdminActor::ThresholdSysop(fixture.full),
                pending.id,
                inspected.state_version,
                &diz,
            )
            .unwrap();
        assert_eq!(edited.lifecycle, FileLifecycle::PendingReview);
        assert!(matches!(
            fixture.database.review_pending_file(
                FileAdminActor::LocalOperator,
                pending.id,
                pending.state_version,
                true
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        let accepted = fixture
            .database
            .review_pending_file(
                FileAdminActor::LocalOperator,
                pending.id,
                edited.state_version,
                true,
            )
            .unwrap();
        assert_eq!(accepted.lifecycle, FileLifecycle::Active);
        assert!(matches!(
            fixture.database.review_pending_file(
                FileAdminActor::LocalOperator,
                pending.id,
                accepted.state_version,
                false
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
    }

    #[test]
    fn duplicate_warning_and_policy_edge_matrix_is_ordered_and_versioned() {
        let mut fixture = fixture();
        for (index, filename) in ["GAME1.ZIP", "GAME2.ARJ", "GAME3.LZH", "GAME.ZIP"]
            .into_iter()
            .enumerate()
        {
            fixture
                .storage
                .write_seed_file(
                    &mut fixture.database,
                    &fixture.area,
                    filename,
                    "Duplicate edge fixture",
                    filename.as_bytes(),
                    1_700_000_520 + index as i64,
                )
                .unwrap();
        }
        fixture
            .database
            .connection
            .execute(
                "UPDATE file_policy SET comprehensive_upload_search=1 WHERE singleton=1",
                [],
            )
            .unwrap();
        assert!(matches!(
            fixture.database.upload_duplicate_warnings(
                fixture.preview,
                fixture.area.id,
                "game1.zip"
            ),
            Err(FileMaintenanceError::File(FileError::DuplicateFilename(_)))
        ));
        let warnings = fixture
            .database
            .upload_duplicate_warnings(fixture.preview, fixture.area.id, "GAME4.TXT")
            .unwrap();
        assert_eq!(warnings.len(), 4);
        assert!(warnings.windows(2).all(|pair| {
            (pair[0].filename.to_ascii_uppercase(), pair[0].file_id)
                < (pair[1].filename.to_ascii_uppercase(), pair[1].file_id)
        }));

        let version: i64 = fixture
            .database
            .connection
            .query_row("SELECT state_version FROM file_policy", [], |row| {
                row.get(0)
            })
            .unwrap();
        let next = fixture
            .database
            .replace_upload_denials(
                FileAdminActor::LocalOperator,
                version as u64,
                &["*.ZIP".to_owned(), "BAD?.ARJ".to_owned()],
            )
            .unwrap();
        assert!(fixture
            .database
            .upload_is_denied(fixture.preview, "lower.zip")
            .unwrap());
        assert!(fixture
            .database
            .upload_is_denied(fixture.preview, "bad1.arj")
            .unwrap());
        assert!(!fixture
            .database
            .upload_is_denied(fixture.preview, "bad12.arj")
            .unwrap());
        assert!(matches!(
            fixture.database.replace_upload_denials(
                FileAdminActor::LocalOperator,
                version as u64,
                &["OTHER.*".to_owned()]
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        for invalid in ["", "../BAD", "A/B", &"X".repeat(65)] {
            assert!(matches!(
                normalize_denial_pattern(invalid),
                Err(FileMaintenanceError::InvalidUploadDenialPattern(_))
            ));
        }
        assert_eq!(next, version as u64 + 1);

        assert_eq!(
            fixture
                .database
                .normalize_upload_description("Mixed café\nsecond Line")
                .unwrap(),
            "Mixed café\nsecond Line"
        );
        let next = fixture
            .database
            .set_description_normalization(FileAdminActor::LocalOperator, next, true)
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "INSERT INTO file_uppercase_terms(term,normalized_term) VALUES('PKZIP','PKZIP')",
                [],
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .normalize_upload_description("Mixed café\npkzip archive")
                .unwrap(),
            "MIXED CAFé\nPKZIP ARCHIVE"
        );
        assert_eq!(next, version as u64 + 2);

        let imported = parse_legacy_sfnoup(b"; comment\r\nGOOD*.ZIP\r\n\r\nBAD/PATH\r\n");
        assert_eq!(imported.patterns, ["GOOD*.ZIP"]);
        assert_eq!(imported.rejected_lines, [4]);
    }

    #[test]
    fn duplicate_warnings_and_native_sfnoup_policy_are_bounded() {
        let mut fixture = fixture();
        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "GAME1.ZIP",
                "Version family",
                b"one",
                1_700_000_000,
            )
            .unwrap();
        fixture
            .database
            .set_description_normalization(FileAdminActor::LocalOperator, 1, true)
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "UPDATE file_policy SET comprehensive_upload_search=1 WHERE singleton=1",
                [],
            )
            .unwrap();
        let warnings = fixture
            .database
            .upload_duplicate_warnings(fixture.full, fixture.area.id, "GAME2.ARJ")
            .unwrap();
        assert!(warnings
            .iter()
            .any(|warning| warning.kind == DuplicateMatchKind::VersionFamily));
        let version: i64 = fixture
            .database
            .connection
            .query_row("SELECT state_version FROM file_policy", [], |row| {
                row.get(0)
            })
            .unwrap();
        fixture
            .database
            .replace_upload_denials(
                FileAdminActor::LocalOperator,
                version as u64,
                &["BAD*.ZIP".to_owned()],
            )
            .unwrap();
        assert!(fixture
            .database
            .upload_is_denied(fixture.preview, "BADWARE.ZIP")
            .unwrap());
        assert!(!fixture
            .database
            .upload_is_denied(fixture.full, "BADWARE.ZIP")
            .unwrap());
        let imported = parse_legacy_sfnoup(b"BAD*.ZIP\r\n../unsafe\r\n");
        assert_eq!(imported.patterns, ["BAD*.ZIP"]);
        assert_eq!(imported.rejected_lines, [2]);
    }

    #[test]
    fn move_preserves_file_id_remove_tombstones_and_recovery_finishes_committed_move() {
        let mut fixture = fixture();
        let destination = fixture
            .database
            .create_file_area(&FileAreaDefinition {
                number: 2,
                name: "Destination".to_owned(),
                description: "Synthetic destination".to_owned(),
                storage_key: "destination".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(1).unwrap(),
                upload_security: SecurityLevel::new(1).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "MOVE.TXT",
                "Move fixture",
                b"move bytes",
                1_700_000_000,
            )
            .unwrap();
        let result = fixture
            .database
            .move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version,
            )
            .unwrap();
        assert_eq!(result.file.id, file.id);
        assert_eq!(result.file.area_id, destination.id);
        let removed = fixture
            .database
            .remove_file(
                FileAdminActor::LocalOperator,
                file.id,
                result.file.state_version,
                true,
            )
            .unwrap();
        assert_eq!(removed.file.lifecycle, FileLifecycle::Tombstoned);

        let second = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "RECOVER.TXT",
                "Recovery fixture",
                b"recover bytes",
                1_700_000_001,
            )
            .unwrap();
        let current_destination = fixture
            .database
            .load_area_by_id(destination.id)
            .unwrap()
            .unwrap();
        assert!(matches!(
            fixture.database.move_file_with_failure(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                second.id,
                second.state_version,
                destination.id,
                current_destination.state_version,
                Some(FailureInjectionPoint::AfterCatalogCommit)
            ),
            Err(FileMaintenanceError::InjectedFailure)
        ));
        assert_eq!(
            fixture
                .database
                .recover_file_operations(&fixture.storage)
                .unwrap(),
            1
        );
        let phase: String = fixture.database.connection.query_row(
            "SELECT phase FROM file_operations WHERE file_id=?1 ORDER BY created_at DESC LIMIT 1",
            params![second.id.get()],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(phase, "committed");
    }

    #[test]
    fn every_move_journal_phase_has_a_deterministic_restart_classification() {
        for (index, point, expected_phase, expected_destination) in [
            (0, FailureInjectionPoint::AfterJournal, "rolled-back", false),
            (1, FailureInjectionPoint::AfterStage, "rolled-back", false),
            (
                2,
                FailureInjectionPoint::AfterPublish,
                "needs-review",
                false,
            ),
            (
                3,
                FailureInjectionPoint::AfterCatalogCommit,
                "committed",
                true,
            ),
            (
                4,
                FailureInjectionPoint::BeforeSourceRemoval,
                "committed",
                true,
            ),
        ] {
            let mut fixture = fixture();
            let destination = destination_area(&mut fixture, 2, "destination");
            let filename = format!("PHASE{index}.TXT");
            let file = fixture
                .storage
                .write_seed_file(
                    &mut fixture.database,
                    &fixture.area,
                    &filename,
                    "Move phase fixture",
                    b"phase bytes",
                    1_700_001_000 + index,
                )
                .unwrap();
            assert!(matches!(
                fixture.database.move_file_with_failure(
                    &fixture.storage,
                    FileAdminActor::LocalOperator,
                    file.id,
                    file.state_version,
                    destination.id,
                    destination.state_version,
                    Some(point)
                ),
                Err(FileMaintenanceError::InjectedFailure)
            ));
            assert_eq!(
                fixture
                    .database
                    .recover_file_operations(&fixture.storage)
                    .unwrap(),
                1
            );
            assert_eq!(latest_operation_phase(&fixture, file.id), expected_phase);
            let catalog = fixture.database.load_file_by_id(file.id).unwrap().unwrap();
            assert_eq!(catalog.area_id == destination.id, expected_destination);
            let source_path = fixture.storage.file_path(&fixture.area, &file).unwrap();
            let destination_path = fixture.storage.file_path(&destination, &file).unwrap();
            match expected_phase {
                "rolled-back" => {
                    assert!(source_path.is_file());
                    assert!(!destination_path.exists());
                }
                "needs-review" => {
                    assert!(source_path.is_file());
                    assert!(destination_path.is_file());
                }
                "committed" => {
                    assert!(!source_path.exists());
                    assert!(destination_path.is_file());
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn filesystem_database_and_lease_failures_never_silently_diverge() {
        let make_fixture = fixture;
        let mut conflict_fixture = make_fixture();
        let destination = destination_area(&mut conflict_fixture, 2, "destination");
        let file = conflict_fixture
            .storage
            .write_seed_file(
                &mut conflict_fixture.database,
                &conflict_fixture.area,
                "CONFLICT.TXT",
                "Conflict matrix fixture",
                b"authoritative bytes",
                1_700_002_000,
            )
            .unwrap();

        assert!(matches!(
            conflict_fixture.database.move_file(
                &conflict_fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version + 1,
                destination.id,
                destination.state_version
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        assert!(matches!(
            conflict_fixture.database.move_file(
                &conflict_fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version + 1
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));

        conflict_fixture
            .database
            .begin_operation(
                "held-lease",
                "move",
                FileAdminActor::LocalOperator,
                Some(&file),
                Some(destination.id),
                Some(destination.state_version),
            )
            .unwrap();
        assert!(matches!(
            conflict_fixture.database.move_file(
                &conflict_fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version
            ),
            Err(FileMaintenanceError::LeaseConflict(id)) if id == file.id
        ));
        conflict_fixture
            .database
            .connection
            .execute("DELETE FROM file_operation_leases", [])
            .unwrap();
        conflict_fixture
            .database
            .connection
            .execute(
                "UPDATE file_operations SET phase='rolled-back' WHERE operation_id='held-lease'",
                [],
            )
            .unwrap();

        conflict_fixture
            .storage
            .write_seed_file(
                &mut conflict_fixture.database,
                &destination,
                "CONFLICT.TXT",
                "Destination collision fixture",
                b"different bytes",
                1_700_002_001,
            )
            .unwrap();
        let refreshed_destination = conflict_fixture
            .database
            .load_area_by_id(destination.id)
            .unwrap()
            .unwrap();
        assert!(matches!(
            conflict_fixture.database.move_file(
                &conflict_fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                refreshed_destination.state_version
            ),
            Err(FileMaintenanceError::DestinationCollision)
        ));
        assert_eq!(
            latest_operation_phase(&conflict_fixture, file.id),
            "needs-review"
        );
        assert_eq!(
            fs::read(
                conflict_fixture
                    .storage
                    .file_path(&conflict_fixture.area, &file)
                    .unwrap()
            )
            .unwrap(),
            b"authoritative bytes"
        );

        let mut short_fixture = make_fixture();
        let destination = destination_area(&mut short_fixture, 2, "destination");
        let file = short_fixture
            .storage
            .write_seed_file(
                &mut short_fixture.database,
                &short_fixture.area,
                "SHORT.TXT",
                "Short-copy fixture",
                b"complete authoritative bytes",
                1_700_002_010,
            )
            .unwrap();
        fs::write(
            short_fixture
                .storage
                .file_path(&short_fixture.area, &file)
                .unwrap(),
            b"short",
        )
        .unwrap();
        assert!(short_fixture
            .database
            .move_file(
                &short_fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version
            )
            .is_err());
        short_fixture
            .database
            .recover_file_operations(&short_fixture.storage)
            .unwrap();
        assert_eq!(
            latest_operation_phase(&short_fixture, file.id),
            "rolled-back"
        );
        assert_eq!(
            short_fixture
                .database
                .load_file_by_id(file.id)
                .unwrap()
                .unwrap()
                .area_id,
            short_fixture.area.id
        );

        let mut fixture = make_fixture();
        let destination = destination_area(&mut fixture, 2, "destination");
        let destination_directory = fixture.storage.ensure_area(&destination).unwrap();
        fs::write(
            destination_directory.join(".spitfire-ng-staging"),
            b"blocks staging directory creation",
        )
        .unwrap();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "STAGING.TXT",
                "Staging failure fixture",
                b"bytes remain at source",
                1_700_002_020,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version
            ),
            Err(FileMaintenanceError::Io(_))
        ));
        fixture
            .database
            .recover_file_operations(&fixture.storage)
            .unwrap();
        assert_eq!(latest_operation_phase(&fixture, file.id), "rolled-back");
    }

    #[test]
    fn database_phase_failures_preserve_reviewable_or_rollback_safe_state() {
        let make_fixture = fixture;
        let mut fixture = make_fixture();
        let destination = destination_area(&mut fixture, 2, "destination");
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "DBFAIL.TXT",
                "Database commit failure fixture",
                b"database failure bytes",
                1_700_003_000,
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER inject_move_catalog_failure BEFORE UPDATE OF area_id ON files BEGIN SELECT RAISE(ABORT,'injected catalog failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version
            ),
            Err(FileMaintenanceError::Sqlite(_))
        ));
        fixture
            .database
            .connection
            .execute_batch("DROP TRIGGER inject_move_catalog_failure;")
            .unwrap();
        fixture
            .database
            .recover_file_operations(&fixture.storage)
            .unwrap();
        assert_eq!(latest_operation_phase(&fixture, file.id), "needs-review");
        assert_eq!(
            fixture
                .database
                .load_file_by_id(file.id)
                .unwrap()
                .unwrap()
                .area_id,
            fixture.area.id
        );
        assert!(fixture
            .storage
            .file_path(&fixture.area, &file)
            .unwrap()
            .is_file());
        assert!(fixture
            .storage
            .file_path(&destination, &file)
            .unwrap()
            .is_file());

        let mut fixture = make_fixture();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "REMOVE.TXT",
                "Remove rollback fixture",
                b"recoverable remove bytes",
                1_700_003_010,
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER inject_remove_failure BEFORE UPDATE OF lifecycle ON files BEGIN SELECT RAISE(ABORT,'injected remove failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.remove_file(
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                true
            ),
            Err(FileMaintenanceError::Sqlite(_))
        ));
        fixture
            .database
            .connection
            .execute_batch("DROP TRIGGER inject_remove_failure;")
            .unwrap();
        fixture
            .database
            .recover_file_operations(&fixture.storage)
            .unwrap();
        assert_eq!(latest_operation_phase(&fixture, file.id), "rolled-back");
        assert_eq!(
            fixture
                .database
                .load_file_by_id(file.id)
                .unwrap()
                .unwrap()
                .lifecycle,
            FileLifecycle::Active
        );
        assert!(fixture
            .storage
            .file_path(&fixture.area, &file)
            .unwrap()
            .is_file());
    }

    #[test]
    fn request_review_and_reconciliation_failures_are_atomic_and_recoverable() {
        let mut fixture = fixture();
        let offline = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "REQUEST.TXT",
                "Request transition fixture",
                b"offline bytes",
                1_700_004_000,
            )
            .unwrap();
        let offline = fixture
            .database
            .set_file_lifecycle(
                FileAdminActor::LocalOperator,
                offline.id,
                offline.state_version,
                FileLifecycle::Offline,
            )
            .unwrap();
        let request = fixture
            .database
            .create_file_request(fixture.preview, offline.id, None)
            .unwrap();
        fixture
            .database
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER inject_request_failure BEFORE UPDATE OF status ON file_requests BEGIN SELECT RAISE(ABORT,'injected request failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.resolve_file_request(
                FileAdminActor::LocalOperator,
                request.request_id,
                request.state_version,
                FileRequestStatus::Rejected
            ),
            Err(FileMaintenanceError::Sqlite(_))
        ));
        fixture
            .database
            .connection
            .execute_batch("DROP TRIGGER inject_request_failure;")
            .unwrap();
        let stored = fixture
            .database
            .pending_file_requests(FileAdminActor::LocalOperator)
            .unwrap();
        assert_eq!(stored[0].status, FileRequestStatus::Pending);
        assert_eq!(stored[0].state_version, request.state_version);

        let mut staged = fixture
            .storage
            .begin_upload(SessionId::new(9).unwrap(), "REVIEWFAIL.TXT")
            .unwrap();
        staged.write_all(b"pending review bytes").unwrap();
        let pending = fixture
            .database
            .commit_upload(
                &fixture.storage,
                staged,
                fixture.full,
                &fixture.area,
                "/ operator review",
                1_700_004_001,
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER inject_review_failure BEFORE UPDATE OF lifecycle ON files WHEN OLD.lifecycle='pending-review' BEGIN SELECT RAISE(ABORT,'injected review failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.review_pending_file(
                FileAdminActor::LocalOperator,
                pending.id,
                pending.state_version,
                true
            ),
            Err(FileMaintenanceError::Sqlite(_))
        ));
        fixture
            .database
            .connection
            .execute_batch("DROP TRIGGER inject_review_failure;")
            .unwrap();
        assert_eq!(
            fixture
                .database
                .load_file_by_id(pending.id)
                .unwrap()
                .unwrap()
                .lifecycle,
            FileLifecycle::PendingReview
        );

        fixture
            .database
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER inject_reconcile_failure BEFORE UPDATE OF integrity_state ON files BEGIN SELECT RAISE(ABORT,'injected reconcile failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.reconcile_files(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                MaintenanceMode::Maintenance
            ),
            Err(FileMaintenanceError::Sqlite(_))
        ));
        fixture
            .database
            .connection
            .execute_batch("DROP TRIGGER inject_reconcile_failure;")
            .unwrap();
        fixture
            .database
            .recover_file_operations(&fixture.storage)
            .unwrap();
        let phase: String = fixture
            .database
            .connection
            .query_row(
                "SELECT phase FROM file_operations WHERE kind='reconcile' ORDER BY created_at DESC,operation_id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(phase, "rolled-back");
    }

    #[test]
    fn active_transfer_blocks_move_and_remove_until_the_use_is_released() {
        let mut fixture = fixture();
        let destination = fixture
            .database
            .create_file_area(&FileAreaDefinition {
                number: 2,
                name: "Destination".to_owned(),
                description: "Synthetic destination".to_owned(),
                storage_key: "destination".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(1).unwrap(),
                upload_security: SecurityLevel::new(1).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "BUSY.TXT",
                "Active-use fixture",
                b"busy bytes",
                1_700_000_000,
            )
            .unwrap();
        let token = fixture
            .database
            .begin_file_download_use(fixture.full, file.id, SessionId::new(7).unwrap())
            .unwrap();
        assert!(matches!(
            fixture.database.move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version,
            ),
            Err(FileMaintenanceError::FileInUse)
        ));
        assert!(matches!(
            fixture.database.remove_file(
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                true,
            ),
            Err(FileMaintenanceError::FileInUse)
        ));
        fixture.database.finish_file_use(token).unwrap();
        let moved = fixture
            .database
            .move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version,
            )
            .unwrap();
        assert_eq!(moved.file.id, file.id);
    }

    #[test]
    fn typed_operator_dispatch_reauthorizes_and_uses_expected_versions() {
        let mut fixture = fixture();
        let response = fixture
            .database
            .dispatch_file_operator_command(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                FileOperatorCommand::Add {
                    area_id: fixture.area.id,
                    expected_area_version: fixture.area.state_version,
                    filename: "ADMIN.TXT".to_owned(),
                    description: "Admin add".to_owned(),
                    bytes: b"operator bytes".to_vec(),
                },
            )
            .unwrap();
        let FileOperatorResponse::Operation(added) = response else {
            panic!("typed add returned the wrong projection");
        };
        let response = fixture
            .database
            .dispatch_file_operator_command(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                FileOperatorCommand::MetadataUpdate {
                    file_id: added.file.id,
                    expected_version: added.file.state_version,
                    description: "Edited café description".to_owned(),
                },
            )
            .unwrap();
        let FileOperatorResponse::File(edited) = response else {
            panic!("typed metadata update returned the wrong projection");
        };
        assert_eq!(edited.description, "Edited café description");
        assert!(matches!(
            fixture.database.dispatch_file_operator_command(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                FileOperatorCommand::MetadataUpdate {
                    file_id: edited.id,
                    expected_version: added.file.state_version,
                    description: "Stale edit".to_owned(),
                },
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        let metadata_events: i64 = fixture
            .database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM file_events WHERE operation='metadata-edited' AND file_id=?1",
                params![edited.id.get()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(metadata_events, 1);
    }

    #[test]
    fn deep_reconciliation_requires_maintenance_mode_and_publication_preserves_columns() {
        let mut fixture = fixture();
        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "LIST.TXT",
                "First line\nSecond line",
                b"listing",
                1_700_000_000,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.reconcile_files(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                MaintenanceMode::Online
            ),
            Err(FileMaintenanceError::MaintenanceModeRequired)
        ));
        let result = fixture
            .database
            .reconcile_files(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                MaintenanceMode::Maintenance,
            )
            .unwrap();
        assert_eq!(result.present, 1);
        fixture
            .database
            .publish_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id,
            )
            .unwrap();
        let listing = fs::read_to_string(
            fixture
                .storage
                .ensure_area(&fixture.area)
                .unwrap()
                .join("SFFILES.BBS"),
        )
        .unwrap();
        let first = listing.lines().next().unwrap();
        assert!(first.starts_with("LIST.TXT"));
        assert_eq!(&first[23..31], "11-14-23");
        assert!(first[33..].starts_with("First line"));
    }

    #[test]
    fn numbered_extended_listing_is_a_bounded_semantic_publication_adapter() {
        let mut fixture = fixture();
        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "CDROM.TXT",
                "Read-only publication fixture",
                b"listing",
                1_700_000_000,
            )
            .unwrap();
        fixture
            .database
            .publish_numbered_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id,
                7,
            )
            .unwrap();
        let directory = fixture.storage.ensure_area(&fixture.area).unwrap();
        let numbered = fs::read(directory.join("SFFILES.7")).unwrap();
        assert!(String::from_utf8_lossy(&numbered).contains("CDROM.TXT"));
        assert!(matches!(
            fixture.database.publish_numbered_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id,
                0,
            ),
            Err(FileMaintenanceError::LegacyPublicationUnrepresentable)
        ));
        let detail: String = fixture
            .database
            .connection
            .query_row(
                "SELECT detail FROM file_events WHERE operation='listing-published' ORDER BY event_id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(detail, "SFFILES.7");
        assert!(!detail.contains(fixture._temp.path().to_str().unwrap()));
    }

    #[test]
    fn legacy_listing_publication_is_cp437_and_refuses_unrepresentable_width() {
        let mut fixture = fixture();
        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "CAFE.TXT",
                "Café listing",
                b"listing",
                1_700_000_000,
            )
            .unwrap();
        fixture
            .database
            .publish_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id,
            )
            .unwrap();
        let listing = fs::read(
            fixture
                .storage
                .ensure_area(&fixture.area)
                .unwrap()
                .join("SFFILES.BBS"),
        )
        .unwrap();
        assert!(listing.contains(&0x82));

        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "TOO-LONG-NAME.TXT",
                "Native filename",
                b"listing",
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.publish_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id,
            ),
            Err(FileMaintenanceError::LegacyPublicationUnrepresentable)
        ));
    }

    #[test]
    fn publication_rename_failure_is_journaled_and_restart_cleans_staging() {
        let mut fixture = fixture();
        fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "PUBLISH.TXT",
                "Publication interruption fixture",
                b"publication bytes",
                1_700_000_600,
            )
            .unwrap();
        let directory = fixture.storage.ensure_area(&fixture.area).unwrap();
        fs::create_dir(directory.join("SFFILES.BBS")).unwrap();
        assert!(matches!(
            fixture.database.publish_legacy_listing(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                fixture.area.id
            ),
            Err(FileMaintenanceError::Io(_))
        ));
        let (operation_id, phase, staging_path): (String, String, String) = fixture
            .database
            .connection
            .query_row(
                "SELECT operation_id,phase,staging_path FROM file_operations WHERE kind='publish-listing' ORDER BY created_at DESC,operation_id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(phase, "staged");
        let staging = fixture
            .storage
            .confined_relative_path(&staging_path)
            .unwrap();
        assert!(staging.is_file());
        assert_eq!(
            fixture
                .database
                .recover_file_operations(&fixture.storage)
                .unwrap(),
            1
        );
        assert!(!staging.exists());
        let recovered: String = fixture
            .database
            .connection
            .query_row(
                "SELECT phase FROM file_operations WHERE operation_id=?1",
                params![operation_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recovered, "rolled-back");
    }

    #[test]
    fn expired_leases_are_reclaimed_and_mutation_races_fail_by_version() {
        let mut fixture = fixture();
        let destination = destination_area(&mut fixture, 2, "destination");
        let file = fixture
            .storage
            .write_seed_file(
                &mut fixture.database,
                &fixture.area,
                "LEASE.TXT",
                "Lease expiry fixture",
                b"lease bytes",
                1_700_000_610,
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "INSERT INTO file_operations(operation_id,kind,file_id,source_area_id,expected_file_version,phase) VALUES('expired-holder','move',?1,?2,?3,'rolled-back')",
                params![file.id.get(), fixture.area.id.get(), file.state_version as i64],
            )
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "INSERT INTO file_operation_leases(lease_kind,file_id,operation_id,expires_at) VALUES('file',?1,'expired-holder',datetime('now','-1 minute'))",
                params![file.id.get()],
            )
            .unwrap();
        let edited = fixture
            .database
            .update_file_metadata(
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                "Edited before competing move",
            )
            .unwrap();
        assert!(matches!(
            fixture.database.move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                destination.id,
                destination.state_version
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        let moved = fixture
            .database
            .move_file(
                &fixture.storage,
                FileAdminActor::LocalOperator,
                file.id,
                edited.state_version,
                destination.id,
                destination.state_version,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.remove_file(
                FileAdminActor::LocalOperator,
                file.id,
                edited.state_version,
                true
            ),
            Err(FileMaintenanceError::StaleConflict)
        ));
        assert_eq!(moved.file.area_id, destination.id);
    }
}
