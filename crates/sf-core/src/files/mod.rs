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

//! Shared native file admission; CircuitNET consumes this authority.
pub mod archive;
pub mod scanner;

use crate::{
    FileAdminActor, FileArea, FileAreaId, FileEntry, FileError, FileId, FileLifecycle, FileStorage,
    NewFileEntry, RuntimeDatabase,
};
use archive::{ArchiveLimits, Inspection};
use rusqlite::{params, OptionalExtension};
use scanner::{Clamd, NoScanner, ScanResult, Scanner};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilesError {
    #[error("{0}")]
    File(#[from] FileError),
    #[error("native file database operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("native file storage operation failed")]
    Io(#[from] std::io::Error),
    #[error("invalid native file metadata")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Policy(String),
}
pub type Result<T> = std::result::Result<T, FilesError>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanPolicy {
    Disabled,
    Optional,
    Required,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetyPolicy {
    pub scanning: ScanPolicy,
    pub approval_required: bool,
    pub scanner: Option<Clamd>,
    pub archives: ArchiveLimits,
}
impl Default for SafetyPolicy {
    fn default() -> Self {
        Self {
            scanning: ScanPolicy::Required,
            approval_required: true,
            scanner: None,
            archives: ArchiveLimits::default(),
        }
    }
}
impl SafetyPolicy {
    fn legacy() -> Self {
        Self {
            scanning: ScanPolicy::Disabled,
            approval_required: false,
            ..Self::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.archives.validate().map_err(FilesError::Policy)?;
        if self
            .scanner
            .as_ref()
            .is_some_and(|s| !s.address.ip().is_loopback())
        {
            return Err(FilesError::Policy("scanner-must-be-local".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdmissionStatus {
    Inspecting,
    Quarantined,
    PendingApproval,
    Published,
    Rejected,
}
impl AdmissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inspecting => "inspecting",
            Self::Quarantined => "quarantined",
            Self::PendingApproval => "pending-approval",
            Self::Published => "published",
            Self::Rejected => "rejected",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        match s {
            "inspecting" => Ok(Self::Inspecting),
            "quarantined" => Ok(Self::Quarantined),
            "pending-approval" => Ok(Self::PendingApproval),
            "published" => Ok(Self::Published),
            "rejected" => Ok(Self::Rejected),
            _ => Err(FilesError::Policy("invalid-admission-state".into())),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Admission {
    pub file_id: i64,
    pub sha256: String,
    pub original_filename: String,
    pub source: String,
    pub status: AdmissionStatus,
    pub policy: SafetyPolicy,
    pub report: Option<Inspection>,
}
#[derive(Clone, Debug)]
pub struct ImportResult {
    pub file: FileEntry,
    pub duplicate_content: bool,
    pub duplicate_description: bool,
    pub existing: bool,
}

impl RuntimeDatabase {
    pub(crate) fn validate_files_authority(&self) -> Result<()> {
        let invalid:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM files f LEFT JOIN file_validation v USING(file_id) WHERE f.lifecycle='active' AND f.safety_required=1 AND COALESCE(v.status,'missing')<>'published') OR EXISTS(SELECT 1 FROM file_validation v JOIN files f USING(file_id) JOIN file_content c ON c.sha256=v.sha256 WHERE v.sha256<>f.sha256 OR c.size_bytes<>f.size_bytes)",[],|r|r.get(0))?;
        if invalid {
            return Err(FilesError::Policy("invalid-native-file-authority".into()));
        }
        for (_, file) in self.all_cataloged_files()? {
            if let Some(admission) = self.file_admission(file.id)? {
                admission.policy.validate()?;
                if matches!(
                    admission.status,
                    AdmissionStatus::Published | AdmissionStatus::PendingApproval
                ) {
                    let report = admission
                        .report
                        .ok_or_else(|| FilesError::Policy("missing-file-inspection".into()))?;
                    if report.archive_error.is_some()
                        || matches!(
                            report.scan.result,
                            ScanResult::MalwareDetected | ScanResult::Suspicious
                        )
                        || (admission.policy.scanning == ScanPolicy::Required
                            && report.scan.result != ScanResult::Clean)
                    {
                        return Err(FilesError::Policy("invalid-file-approval".into()));
                    }
                }
            }
        }
        Ok(())
    }
    fn duplicate_diz(&self, file: FileId) -> Result<bool> {
        Ok(self.connection.query_row("SELECT EXISTS(SELECT 1 FROM file_validation a JOIN file_validation b ON a.file_id<>b.file_id WHERE a.file_id=?1 AND json_extract(a.report,'$.diz.suggestion')<>'' AND json_extract(a.report,'$.diz.suggestion')=json_extract(b.report,'$.diz.suggestion'))",[file.get()],|r|r.get(0))?)
    }
    pub fn content_catalog(&self) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .connection
            .prepare("SELECT sha256,size_bytes FROM file_content ORDER BY sha256")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
    pub fn file_safety_policy(&self, area: FileAreaId) -> Result<SafetyPolicy> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT policy FROM file_area_safety WHERE area_id=?1",
                [area.get()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match json {
            Some(j) => serde_json::from_str(&j)?,
            None => SafetyPolicy::legacy(),
        })
    }
    pub fn set_file_safety_policy(
        &mut self,
        actor: FileAdminActor,
        area: FileAreaId,
        policy: &SafetyPolicy,
    ) -> Result<()> {
        self.authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        policy.validate()?;
        self.connection.execute("INSERT INTO file_area_safety VALUES(?1,?2) ON CONFLICT(area_id) DO UPDATE SET policy=excluded.policy",params![area.get(),serde_json::to_string(policy)?])?;
        Ok(())
    }
    pub fn file_admission(&self, file: FileId) -> Result<Option<Admission>> {
        let row = self.connection.query_row("SELECT sha256,original_filename,source,status,policy,report FROM file_validation WHERE file_id=?1",[file.get()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?))).optional()?;
        row.map(
            |(sha256, original_filename, source, status, policy, report)| {
                Ok(Admission {
                    file_id: file.get(),
                    sha256,
                    original_filename,
                    source,
                    status: AdmissionStatus::parse(&status)?,
                    policy: serde_json::from_str(&policy)?,
                    report: report.map(|j| serde_json::from_str(&j)).transpose()?,
                })
            },
        )
        .transpose()
    }
    pub fn file_admissions(&self, actor: FileAdminActor) -> Result<Vec<Admission>> {
        self.authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        let mut stmt = self
            .connection
            .prepare("SELECT file_id FROM file_validation ORDER BY file_id DESC LIMIT 1000")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                self.file_admission(FileId::new(id)?)?
                    .ok_or_else(|| FilesError::Policy("file-unavailable".into()))
            })
            .collect()
    }
    fn admission_status(
        &mut self,
        file: FileId,
        status: AdmissionStatus,
        operation: &str,
    ) -> Result<()> {
        self.admission_status_by(file, status, operation, "native-admission")
    }
    fn admission_status_by(
        &mut self,
        file: FileId,
        status: AdmissionStatus,
        operation: &str,
        actor: &str,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute(
            "UPDATE file_validation SET status=?2,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1",
            params![file.get(), status.as_str()],
        )?;
        let lifecycle = match status {
            AdmissionStatus::Published => "active",
            AdmissionStatus::Rejected => "disabled",
            _ => "pending-review",
        };
        tx.execute("UPDATE files SET lifecycle=?2,state_version=state_version+1,review_submitted_at=COALESCE(review_submitted_at,unixepoch()),reviewed_at=CASE WHEN ?2='pending-review' THEN NULL ELSE unixepoch() END,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND lifecycle<>'tombstoned'",params![file.get(),lifecycle])?;
        tx.execute(
            "INSERT INTO file_safety_history(file_id,operation,result,actor) VALUES(?1,?2,?3,?4)",
            params![file.get(), operation, status.as_str(), actor],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn review_file_admission(
        &mut self,
        actor: FileAdminActor,
        file: FileId,
        approve: bool,
    ) -> Result<Admission> {
        self.authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        let mut admission = self
            .file_admission(file)?
            .ok_or_else(|| FilesError::Policy("file-not-inspected".into()))?;
        if approve {
            if admission.status != AdmissionStatus::PendingApproval {
                return Err(FilesError::Policy("rescan-required-before-approval".into()));
            }
            let entry = self
                .load_file_by_id(file)?
                .ok_or_else(|| FilesError::Policy("file-unavailable".into()))?;
            let current = self.file_safety_policy(entry.area_id)?;
            admission.policy.approval_required = current.approval_required;
            if serde_json::to_string(&current)? != serde_json::to_string(&admission.policy)? {
                return Err(FilesError::Policy("policy-changed-rescan-required".into()));
            }
        }
        self.admission_status_by(
            file,
            if approve {
                AdmissionStatus::Published
            } else {
                AdmissionStatus::Rejected
            },
            if approve { "approve" } else { "reject" },
            &admin_label(actor),
        )?;
        self.file_admission(file)?
            .ok_or_else(|| FilesError::Policy("file-unavailable".into()))
    }
    pub fn use_file_description(
        &mut self,
        actor: FileAdminActor,
        file: FileId,
        edited: Option<&str>,
    ) -> Result<()> {
        self.authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        let description = match edited {
            Some(text) => text.to_owned(),
            None => {
                self.file_admission(file)?
                    .and_then(|a| a.report)
                    .and_then(|r| r.diz)
                    .ok_or_else(|| FilesError::Policy("file-id-diz-unavailable".into()))?
                    .suggestion
            }
        };
        if description.len() > crate::MAX_FILE_DESCRIPTION_BYTES
            || description.lines().count() > crate::MAX_FILE_DESCRIPTION_LINES
            || description
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(FilesError::Policy("invalid-description".into()));
        }
        self.connection.execute("UPDATE files SET description=?2,description_source=?3,state_version=state_version+1 WHERE file_id=?1 AND lifecycle<>'tombstoned'",params![file.get(),description,if edited.is_some(){"operator"}else{"file-id-diz"}])?;
        Ok(())
    }
}

impl FileStorage {
    fn content_root(&self) -> Result<PathBuf> {
        let root = self.files_root.join(".content");
        if !root.exists() {
            fs::create_dir(&root)?;
        }
        if fs::symlink_metadata(&root)?.file_type().is_symlink() || !root.is_dir() {
            return Err(FilesError::Policy("unsafe-content-store".into()));
        }
        Ok(root)
    }
    pub fn open_content(&self, hash: &str, size: u64) -> Result<File> {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(FilesError::Policy("invalid-content-hash".into()));
        }
        let path = self.content_root()?.join(hash);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(FilesError::Policy("unsafe-content-object".into()));
        }
        let mut file = File::open(path)?;
        if file.metadata()?.len() != size {
            return Err(FilesError::Policy("content-integrity-failure".into()));
        }
        let (actual, digest) = digest(&mut file, size)?;
        if actual != size || digest != hash {
            return Err(FilesError::Policy("content-integrity-failure".into()));
        }
        file.rewind()?;
        Ok(file)
    }
    fn store_content(&self, input: &mut dyn Read, limit: u64) -> Result<(String, u64, bool)> {
        let root = self.content_root()?;
        let mut temporary = tempfile::NamedTempFile::new_in(&root)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0; 65536];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .ok_or_else(|| FilesError::Policy("source-limit".into()))?;
            if bytes > limit {
                return Err(FilesError::Policy("source-limit".into()));
            }
            hash.update(&buffer[..n]);
            temporary.write_all(&buffer[..n])?;
        }
        temporary.as_file().sync_all()?;
        let hash = format!("{:x}", hash.finalize());
        let destination = root.join(&hash);
        let exists = destination.exists();
        if exists {
            self.open_content(&hash, bytes)?;
        } else {
            let mut permissions = temporary.as_file().metadata()?.permissions();
            permissions.set_readonly(true);
            temporary.as_file().set_permissions(permissions)?;
            match temporary.persist_noclobber(&destination) {
                Ok(_) => (),
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.open_content(&hash, bytes)?;
                }
                Err(e) => return Err(e.error.into()),
            }
        }
        Ok((hash, bytes, exists))
    }
    /// Operator and network admission share this method; callers retain their existing permission checks.
    #[allow(clippy::too_many_arguments)]
    pub fn import_file(
        &self,
        database: &mut RuntimeDatabase,
        actor: FileAdminActor,
        area: &FileArea,
        filename: &str,
        description: &str,
        input: &mut dyn Read,
        source: &str,
        scanner: Option<&dyn Scanner>,
    ) -> Result<ImportResult> {
        database
            .authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        let normalized = crate::normalize_filename(filename)?;
        let area = database
            .load_area_by_id(area.id)?
            .ok_or_else(|| FilesError::Policy("area-unavailable".into()))?;
        if !area.active {
            return Err(FilesError::Policy("area-disabled".into()));
        }
        if !matches!(source, "operator" | "caller" | "circuitnet") {
            return Err(FilesError::Policy("invalid-import-source".into()));
        }
        let policy = database.file_safety_policy(area.id)?;
        policy.validate()?;
        let (hash, size, reused) = self.store_content(
            input,
            policy.archives.source_bytes.min(area.maximum_upload_bytes),
        )?;
        let existing: Option<i64> = database
            .connection
            .query_row(
                "SELECT file_id FROM files WHERE area_id=?1 AND normalized_filename=?2",
                params![area.id.get(), normalized],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            let file = database
                .load_file_by_id(FileId::new(id)?)?
                .ok_or_else(|| FilesError::Policy("file-unavailable".into()))?;
            if file.sha256 != hash
                || file.size_bytes != size
                || file.lifecycle == FileLifecycle::Tombstoned
            {
                return Err(FilesError::Policy(
                    "filename-collision-choose-another-name".into(),
                ));
            }
            let admission = database.file_admission(file.id)?;
            // A fresh remote publication cannot undo an explicit local rejection.
            // Only the authorized local rescan workflow can reconsider that decision.
            if source == "circuitnet"
                && admission
                    .as_ref()
                    .is_some_and(|a| a.status == AdmissionStatus::Rejected)
            {
                return Ok(ImportResult {
                    duplicate_description: database.duplicate_diz(file.id)?,
                    file,
                    duplicate_content: true,
                    existing: true,
                });
            }
            if source == "circuitnet" || admission.is_none() {
                self.link_content(&area, &file)?;
                self.finish_admission(database, &file, source, &policy, scanner)?;
                let file = database
                    .load_file_by_id(file.id)?
                    .ok_or_else(|| FilesError::Policy("file-unavailable".into()))?;
                return Ok(ImportResult {
                    duplicate_description: database.duplicate_diz(file.id)?,
                    file,
                    duplicate_content: true,
                    existing: true,
                });
            }
            return Ok(ImportResult {
                duplicate_description: database.duplicate_diz(file.id)?,
                file,
                duplicate_content: true,
                existing: true,
            });
        }
        let path = self.ensure_area(&area)?.join(filename);
        fs::hard_link(self.content_root()?.join(&hash), &path)?;
        let inserted = database.insert_file_entry_with_custody(
            &NewFileEntry {
                area_id: area.id,
                filename: filename.into(),
                description: description.into(),
                size_bytes: size,
                sha256: hash.clone(),
                uploaded_at: chrono::Utc::now().timestamp(),
                uploader_caller_id: None,
                uploader_name: "System".into(),
                lifecycle: FileLifecycle::PendingReview,
            },
            true,
        );
        let file = match inserted {
            Ok(f) => f,
            Err(e) => {
                let _ = fs::remove_file(&path);
                return Err(e.into());
            }
        };
        self.finish_admission(database, &file, source, &policy, scanner)?;
        let file = database
            .load_file_by_id(file.id)?
            .ok_or_else(|| FilesError::Policy("file-unavailable".into()))?;
        Ok(ImportResult {
            duplicate_description: database.duplicate_diz(file.id)?,
            file,
            duplicate_content: reused,
            existing: false,
        })
    }
    pub fn rescan_file(
        &self,
        database: &mut RuntimeDatabase,
        actor: FileAdminActor,
        file: FileId,
        scanner: Option<&dyn Scanner>,
    ) -> Result<Admission> {
        database
            .authorize_file_admin(actor)
            .map_err(|_| FilesError::Policy("operator-authorization-required".into()))?;
        let entry = database
            .load_file_by_id(file)?
            .ok_or_else(|| FilesError::Policy("file-unavailable".into()))?;
        if entry.lifecycle == FileLifecycle::Tombstoned {
            return Err(FilesError::Policy("file-unavailable".into()));
        }
        let policy = database.file_safety_policy(entry.area_id)?;
        let old = database.file_admission(file)?;
        if old.is_none() {
            let area = database
                .load_area_by_id(entry.area_id)?
                .ok_or_else(|| FilesError::Policy("area-unavailable".into()))?;
            self.admit_completed_upload(database, &area, &entry, true)?;
            return database
                .file_admission(file)?
                .ok_or_else(|| FilesError::Policy("file-unavailable".into()));
        }
        let old = old.expect("checked admission");
        self.finish_admission(database, &entry, &old.source, &policy, scanner)?;
        database
            .file_admission(file)?
            .ok_or_else(|| FilesError::Policy("file-unavailable".into()))
    }
    pub(crate) fn admit_completed_upload(
        &self,
        database: &mut RuntimeDatabase,
        area: &FileArea,
        file: &FileEntry,
        caller_review: bool,
    ) -> Result<FileEntry> {
        let mut source = self.open_download(area, file)?;
        let policy = database.file_safety_policy(area.id)?;
        let (hash, size, _) = self.store_content(
            &mut source,
            policy.archives.source_bytes.min(area.maximum_upload_bytes),
        )?;
        if hash != file.sha256 || size != file.size_bytes {
            return Err(FilesError::Policy("content-integrity-failure".into()));
        }
        self.link_content(area, file)?;
        let mut effective = policy;
        effective.approval_required |= caller_review;
        self.finish_admission(database, file, "caller", &effective, None)?;
        database
            .load_file_by_id(file.id)?
            .ok_or_else(|| FilesError::Policy("file-unavailable".into()))
    }
    fn link_content(&self, area: &FileArea, file: &FileEntry) -> Result<()> {
        self.open_content(&file.sha256, file.size_bytes)?;
        let directory = self.ensure_area(area)?;
        let temp = directory.join(format!(".custody-{:032x}", rand::random::<u128>()));
        fs::hard_link(self.content_root()?.join(&file.sha256), &temp)?;
        if let Err(e) = fs::rename(&temp, directory.join(&file.filename)) {
            let _ = fs::remove_file(&temp);
            return Err(e.into());
        }
        Ok(())
    }
    pub fn check_content(&self, database: &RuntimeDatabase) -> Result<Vec<ContentCheck>> {
        let mut referenced = std::collections::BTreeMap::<String, (u64, usize)>::new();
        for (_, file) in database.managed_cataloged_files()? {
            if database.file_admission(file.id)?.is_some() {
                let value = referenced
                    .entry(file.sha256.clone())
                    .or_insert((file.size_bytes, 0));
                if value.0 != file.size_bytes {
                    return Err(FilesError::Policy("conflicting-content-size".into()));
                }
                value.1 += 1;
            }
        }
        for (hash, size) in database.content_catalog()? {
            referenced.entry(hash).or_insert((size, 0));
        }
        let mut result = Vec::new();
        for (hash, (size, refs)) in &referenced {
            let status = match self.open_content(hash, *size) {
                Ok(_) => "verified",
                Err(FilesError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => "missing",
                Err(_) => "corrupt",
            };
            result.push(ContentCheck {
                sha256: hash.clone(),
                references: *refs,
                status: status.into(),
            });
        }
        for entry in fs::read_dir(self.content_root()?)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.len() == 64
                && name.bytes().all(|b| b.is_ascii_hexdigit())
                && !referenced.contains_key(name.as_ref())
            {
                result.push(ContentCheck {
                    sha256: name.into_owned(),
                    references: 0,
                    status: "unreferenced-retained".into(),
                });
            }
        }
        Ok(result)
    }
    /// Cold restore has already verified each logical payload against the manifest.
    /// Reconstitute canonical custody and hard-link references without changing decisions.
    pub fn rebuild_restored_content(&self, database: &RuntimeDatabase) -> Result<()> {
        for (hash, size) in database.content_catalog()? {
            let file = self.open_content(&hash, size)?;
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)?;
        }
        for (area, file) in database.managed_cataloged_files()? {
            if database.file_admission(file.id)?.is_none() {
                continue;
            }
            self.link_content(&area, &file)?;
        }
        Ok(())
    }
    fn finish_admission(
        &self,
        database: &mut RuntimeDatabase,
        file: &FileEntry,
        source: &str,
        policy: &SafetyPolicy,
        scanner: Option<&dyn Scanner>,
    ) -> Result<()> {
        let tx = database.connection.transaction()?;
        tx.execute("INSERT INTO file_content(sha256,size_bytes) VALUES(?1,?2) ON CONFLICT(sha256) DO NOTHING",params![file.sha256,file.size_bytes as i64])?;
        tx.execute(
            "UPDATE files SET lifecycle='pending-review',review_submitted_at=COALESCE(review_submitted_at,unixepoch()),reviewed_at=NULL WHERE file_id=?1",
            [file.id.get()],
        )?;
        tx.execute("INSERT INTO file_validation(file_id,sha256,original_filename,source,status,policy) VALUES(?1,?2,?3,?4,'inspecting',?5) ON CONFLICT(file_id) DO UPDATE SET status='inspecting',policy=excluded.policy,report=NULL,updated_at=CURRENT_TIMESTAMP",params![file.id.get(),file.sha256,file.filename,source,serde_json::to_string(policy)?])?;
        tx.commit()?;
        let mut content = self.open_content(&file.sha256, file.size_bytes)?;
        let no_scanner = NoScanner;
        let configured: &dyn Scanner = policy
            .scanner
            .as_ref()
            .map(|s| s as &dyn Scanner)
            .unwrap_or(&no_scanner);
        let provider = if policy.scanning == ScanPolicy::Disabled {
            None
        } else {
            Some(scanner.unwrap_or(configured))
        };
        let report = archive::inspect(&mut content, &file.filename, &policy.archives, provider);
        let bad = report.archive_error.is_some()
            || matches!(
                report.scan.result,
                ScanResult::MalwareDetected | ScanResult::Suspicious
            )
            || (policy.scanning == ScanPolicy::Required && report.scan.result != ScanResult::Clean);
        let status = if bad {
            AdmissionStatus::Quarantined
        } else if policy.approval_required {
            AdmissionStatus::PendingApproval
        } else {
            AdmissionStatus::Published
        };
        database.connection.execute(
            "UPDATE file_validation SET report=?2 WHERE file_id=?1",
            params![file.id.get(), serde_json::to_string(&report)?],
        )?;
        database.admission_status(file.id, status, "inspect")
    }
}
fn digest(reader: &mut dyn Read, limit: u64) -> Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or_else(|| FilesError::Policy("source-limit".into()))?;
        if size > limit {
            return Err(FilesError::Policy("source-limit".into()));
        }
        hash.update(&buffer[..n]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentCheck {
    pub sha256: String,
    pub references: usize,
    pub status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub published: u64,
    pub pending_approval: u64,
    pub quarantined: u64,
    pub inspecting: u64,
    pub rejected: u64,
    pub scanner_unavailable: u64,
}
impl RuntimeDatabase {
    pub fn files_summary(&self) -> Result<Summary> {
        let mut summary = Summary::default();
        let mut stmt = self
            .connection
            .prepare("SELECT status,COUNT(*) FROM file_validation GROUP BY status")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (status, count) = row?;
            let n = count as u64;
            match status.as_str() {
                "published" => summary.published = n,
                "pending-approval" => summary.pending_approval = n,
                "quarantined" => summary.quarantined = n,
                "inspecting" => summary.inspecting = n,
                "rejected" => summary.rejected = n,
                _ => (),
            }
        }
        summary.scanner_unavailable=self.connection.query_row("SELECT COUNT(*) FROM file_validation WHERE json_extract(report,'$.scan.result') IN ('scanner-unavailable','scanner-error')",[],|r|r.get::<_,i64>(0))? as u64;
        Ok(summary)
    }
}

fn admin_label(actor: FileAdminActor) -> String {
    match actor {
        FileAdminActor::LocalOperator => "local-operator".into(),
        FileAdminActor::ThresholdSysop(a) => format!("caller:{}", a.caller_id().get()),
    }
}
