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

//! Daemon-authoritative transfer policy, queue, accounting, and storage state.
//!
//! Wire engines remain in `transfer`; this module owns the semantic boundary
//! around them.  It deliberately persists accounting and recovery identity,
//! while caller queues and protocol byte-stream state stay session-local.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroI64;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, OptionalExtension};
use thiserror::Error;

use crate::{
    insert_operational_event_tx, EventAttributes, EventCategory, EventOutcome, EventSeverity,
    FileAccess, FileActor, FileAreaId, FileEntry, FileId, FileIntegrity, FileLifecycle,
    NewOperationalEvent, NodeId, RetentionClass, RuntimeDatabase, SecurityLevel, TransferProtocol,
};

pub const MAX_BATCH_QUEUE_ITEMS: usize = 99;
pub const MAX_BATCH_QUEUE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const ALL_PROTOCOLS_MASK: u16 = 0x01ff;
pub const MAX_LEGACY_POLICY_BYTES: usize = 64 * 1024;
pub const MAX_LEGACY_POLICY_LINES: usize = 256;
pub const MAX_LEGACY_STORAGE_ROOTS: usize = 64;
const RESERVATION_TTL_SECONDS: i64 = 60 * 60;
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TransferId(String);

impl TransferId {
    pub fn new(value: impl Into<String>) -> Result<Self, TransferRuntimeError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    pub fn generated(session_id: i64) -> Self {
        Self(format!(
            "transfer-{session_id}-{}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ReservationId(String);

impl ReservationId {
    pub fn new(value: impl Into<String>) -> Result<Self, TransferRuntimeError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    fn for_transfer(transfer: &TransferId) -> Self {
        Self(format!("reservation-{}", transfer.as_str()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferMethod {
    Ascii,
    Binary(TransferProtocol),
}

impl TransferMethod {
    pub const fn database_value(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Binary(TransferProtocol::XmodemChecksum) => "xmodem-checksum",
            Self::Binary(TransferProtocol::XmodemCrc) => "xmodem-crc",
            Self::Binary(TransferProtocol::Xmodem1k) => "xmodem-1k",
            Self::Binary(TransferProtocol::Xmodem1kG) => "xmodem-1k-g",
            Self::Binary(TransferProtocol::YmodemBatch) => "ymodem-batch",
            Self::Binary(TransferProtocol::YmodemGBatch) => "ymodem-g-batch",
            Self::Binary(TransferProtocol::ZmodemBatch) => "zmodem-batch",
            Self::Binary(TransferProtocol::Telink) => "telink",
        }
    }

    pub const fn supports_batch(self) -> bool {
        matches!(self, Self::Binary(protocol) if protocol.is_batch())
    }

    pub const fn mask(self) -> u16 {
        match self {
            Self::Ascii => 1 << 0,
            Self::Binary(TransferProtocol::XmodemChecksum) => 1 << 1,
            Self::Binary(TransferProtocol::XmodemCrc) => 1 << 2,
            Self::Binary(TransferProtocol::Xmodem1k) => 1 << 3,
            Self::Binary(TransferProtocol::YmodemBatch) => 1 << 4,
            Self::Binary(TransferProtocol::ZmodemBatch) => 1 << 5,
            Self::Binary(TransferProtocol::Xmodem1kG) => 1 << 6,
            Self::Binary(TransferProtocol::YmodemGBatch) => 1 << 7,
            Self::Binary(TransferProtocol::Telink) => 1 << 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferRuntimeState {
    Planned,
    Authorized,
    Reserved,
    Negotiating,
    Transferring,
    Settling,
    Completed,
    Cancelled,
    Failed,
    NeedsReview,
}

impl TransferRuntimeState {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Authorized => "authorized",
            Self::Reserved => "reserved",
            Self::Negotiating => "negotiating",
            Self::Transferring => "transferring",
            Self::Settling => "settling",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::NeedsReview => "needs-review",
        }
    }

    const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::NeedsReview
        )
    }

    fn from_database_value(value: &str) -> Result<Self, TransferRuntimeError> {
        match value {
            "planned" => Ok(Self::Planned),
            "authorized" => Ok(Self::Authorized),
            "reserved" => Ok(Self::Reserved),
            "negotiating" => Ok(Self::Negotiating),
            "transferring" => Ok(Self::Transferring),
            "settling" => Ok(Self::Settling),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            "needs-review" => Ok(Self::NeedsReview),
            _ => Err(TransferRuntimeError::InvalidStoredState),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferCancelSource {
    Caller,
    Operator,
    Disconnect,
    Timeout,
    DaemonShutdown,
}

#[derive(Clone, Copy, Debug)]
pub struct TransferStateChange<'a> {
    pub expected_version: u64,
    pub state: TransferRuntimeState,
    pub bytes_transferred: u64,
    pub error_class: Option<&'a str>,
    pub cancel_source: Option<TransferCancelSource>,
    pub occurred_at: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct UploadCreditRequest<'a> {
    pub transfer_id: &'a TransferId,
    pub item_id: &'a str,
    pub actor: FileActor,
    pub node_id: NodeId,
    pub method: TransferMethod,
    pub file_id: FileId,
    pub active_seconds: u64,
    pub timezone: Tz,
    pub occurred_at: i64,
}

impl TransferCancelSource {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::Operator => "operator",
            Self::Disconnect => "disconnect",
            Self::Timeout => "timeout",
            Self::DaemonShutdown => "daemon-shutdown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferPolicy {
    pub security_level: SecurityLevel,
    pub daily_file_limit: Option<u64>,
    pub daily_byte_limit: Option<u64>,
    pub ratio_warning_thousandths: Option<u64>,
    pub ratio_enforcement_thousandths: Option<u64>,
    pub ratio_violation_security: Option<SecurityLevel>,
    pub upload_credit_thousandths: u64,
    pub upload_credit_file_cap_seconds: u64,
    pub upload_credit_day_cap_seconds: u64,
    pub protocol_mask: u16,
    pub state_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RatioStatus {
    Healthy,
    Warning,
    Denied,
}

impl TransferPolicy {
    pub fn unlimited(security_level: SecurityLevel) -> Self {
        Self {
            security_level,
            daily_file_limit: None,
            daily_byte_limit: None,
            ratio_warning_thousandths: None,
            ratio_enforcement_thousandths: None,
            ratio_violation_security: None,
            upload_credit_thousandths: 0,
            upload_credit_file_cap_seconds: 0,
            upload_credit_day_cap_seconds: 0,
            protocol_mask: ALL_PROTOCOLS_MASK,
            state_version: 0,
        }
    }

    pub fn validate(&self) -> Result<(), TransferRuntimeError> {
        if self.daily_file_limit == Some(0)
            || self.daily_byte_limit == Some(0)
            || self.ratio_warning_thousandths == Some(0)
            || self.ratio_enforcement_thousandths == Some(0)
            || self.protocol_mask > ALL_PROTOCOLS_MASK
            || self.upload_credit_thousandths > 100_000
            || self.ratio_warning_thousandths.is_some_and(|warning| {
                self.ratio_enforcement_thousandths
                    .is_some_and(|enforcement| warning > enforcement)
            })
        {
            return Err(TransferRuntimeError::InvalidPolicy);
        }
        Ok(())
    }

    pub const fn allows_protocol(&self, protocol: TransferMethod) -> bool {
        self.protocol_mask & protocol.mask() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedFile {
    pub item_id: String,
    pub file_id: FileId,
    pub area_id: FileAreaId,
    pub filename: String,
    pub expected_file_version: u64,
    pub bytes: u64,
    pub no_charge: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransferQueue {
    items: Vec<QueuedFile>,
}

impl TransferQueue {
    pub fn items(&self) -> &[QueuedFile] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|item| item.bytes).sum()
    }

    pub fn chargeable_totals(&self) -> (u64, u64) {
        self.items
            .iter()
            .filter(|item| !item.no_charge)
            .fold((0_u64, 0_u64), |(files, bytes), item| {
                (files + 1, bytes + item.bytes)
            })
    }

    pub fn tag(&mut self, file: &FileEntry, no_charge: bool) -> Result<bool, TransferRuntimeError> {
        if self.items.iter().any(|item| item.file_id == file.id) {
            return Ok(false);
        }
        if self.items.len() >= MAX_BATCH_QUEUE_ITEMS {
            return Err(TransferRuntimeError::QueueFull);
        }
        if file.lifecycle != FileLifecycle::Active
            || matches!(
                file.integrity,
                FileIntegrity::Missing | FileIntegrity::DigestMismatch
            )
        {
            return Err(TransferRuntimeError::FileUnavailable);
        }
        let aggregate = self
            .total_bytes()
            .checked_add(file.size_bytes)
            .ok_or(TransferRuntimeError::ResourceLimit)?;
        if aggregate > MAX_BATCH_QUEUE_BYTES {
            return Err(TransferRuntimeError::ResourceLimit);
        }
        self.items.push(QueuedFile {
            item_id: format!("file-{}", file.id.get()),
            file_id: file.id,
            area_id: file.area_id,
            filename: file.filename.clone(),
            expected_file_version: file.state_version,
            bytes: file.size_bytes,
            no_charge,
        });
        Ok(true)
    }

    pub fn untag(&mut self, file_id: FileId) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.file_id != file_id);
        before != self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn retain_unsettled(&mut self, settled_item_ids: &[String]) {
        self.items
            .retain(|item| !settled_item_ids.contains(&item.item_id));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaReservation {
    pub id: ReservationId,
    pub transfer_id: TransferId,
    pub board_day: String,
    pub timezone_policy_version: u64,
    pub chargeable_files: u64,
    pub chargeable_bytes: u64,
    pub state_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveTransferSummary {
    pub transfer_id: TransferId,
    pub caller_id: crate::CallerId,
    pub node_id: NodeId,
    pub direction: TransferDirectionKind,
    pub protocol: String,
    pub state: TransferRuntimeState,
    pub bytes_expected: u64,
    pub bytes_transferred: u64,
    pub state_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferDirectionKind {
    Download,
    Upload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyTransferUsage {
    pub caller_id: crate::CallerId,
    pub board_day: String,
    pub chargeable_download_files: u64,
    pub chargeable_download_bytes: u64,
    pub upload_credit_seconds: u64,
    pub state_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageRootKind {
    Managed,
    External,
}

impl StorageRootKind {
    fn from_database_value(value: &str) -> Result<Self, TransferRuntimeError> {
        match value {
            "managed" => Ok(Self::Managed),
            "external" => Ok(Self::External),
            _ => Err(TransferRuntimeError::InvalidStoredState),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageRootMode {
    ReadWrite,
    ReadOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct StorageRootDefinition<'a> {
    pub area_id: FileAreaId,
    pub stable_key: &'a str,
    pub label: &'a str,
    pub configured_locator: &'a str,
    pub priority: u8,
    pub mode: StorageRootMode,
    pub occurred_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageRootState {
    Enabled,
    Maintenance,
    Disabled,
}

impl StorageRootState {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Maintenance => "maintenance",
            Self::Disabled => "disabled",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, TransferRuntimeError> {
        match value {
            "enabled" => Ok(Self::Enabled),
            "maintenance" => Ok(Self::Maintenance),
            "disabled" => Ok(Self::Disabled),
            _ => Err(TransferRuntimeError::InvalidStoredState),
        }
    }
}

impl StorageRootMode {
    const fn database_value(self) -> &'static str {
        match self {
            Self::ReadWrite => "read-write",
            Self::ReadOnly => "read-only",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, TransferRuntimeError> {
        match value {
            "read-write" => Ok(Self::ReadWrite),
            "read-only" => Ok(Self::ReadOnly),
            _ => Err(TransferRuntimeError::InvalidStoredState),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageAvailability {
    Unknown,
    Available,
    Unavailable,
}

impl StorageAvailability {
    const fn database_value(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Available => "available",
            Self::Unavailable => "unavailable",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, TransferRuntimeError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "available" => Ok(Self::Available),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(TransferRuntimeError::InvalidStoredState),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageRootId(NonZeroI64);

impl StorageRootId {
    pub fn new(value: i64) -> Result<Self, TransferRuntimeError> {
        NonZeroI64::new(value)
            .filter(|id| id.get() > 0)
            .map(Self)
            .ok_or(TransferRuntimeError::InvalidStorageRootId)
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageRoot {
    pub id: StorageRootId,
    pub area_id: FileAreaId,
    pub stable_key: String,
    pub label: String,
    pub kind: StorageRootKind,
    pub mode: StorageRootMode,
    pub priority: u8,
    pub configured_locator: String,
    pub configured_state: StorageRootState,
    pub availability: StorageAvailability,
    pub staging_always: bool,
    pub state_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStorageLocator {
    pub file_id: FileId,
    pub storage_root_id: StorageRootId,
    pub relative_path: String,
    pub state_version: u64,
}

/// Lossless bounded compatibility view of one documented DAILYLMT.DAT row.
/// Fields owned by other domains remain in `tokens`; native transfer policy
/// imports only DLPD, KB, VWR, and VER.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDailyLimitRecord {
    pub security_level: SecurityLevel,
    pub daily_files: Option<u64>,
    pub daily_decimal_kilobytes: Option<u64>,
    pub ratio_warning: Option<u64>,
    pub ratio_enforcement: Option<u64>,
    pub tokens: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDailyLimitDocument {
    pub source: Vec<u8>,
    pub records: Vec<LegacyDailyLimitRecord>,
}

impl LegacyDailyLimitDocument {
    pub fn parse(source: &[u8]) -> Result<Self, TransferRuntimeError> {
        if source.len() > MAX_LEGACY_POLICY_BYTES || source.contains(&0) || !source.is_ascii() {
            return Err(TransferRuntimeError::InvalidLegacyAdapter);
        }
        let text =
            std::str::from_utf8(source).map_err(|_| TransferRuntimeError::InvalidLegacyAdapter)?;
        let mut records = Vec::new();
        let mut security_levels = BTreeSet::new();
        for raw_line in text.lines() {
            let line = raw_line.trim_end_matches('\r').trim();
            if line.is_empty() {
                continue;
            }
            if records.len() >= MAX_LEGACY_POLICY_LINES || line.len() > 4096 {
                return Err(TransferRuntimeError::ResourceLimit);
            }
            let mut parts = line.split(',');
            let security_level = parts
                .next()
                .and_then(|value| value.trim().parse::<u16>().ok())
                .and_then(|value| SecurityLevel::new(value).ok())
                .ok_or(TransferRuntimeError::InvalidLegacyAdapter)?;
            if !security_levels.insert(security_level.get()) {
                return Err(TransferRuntimeError::InvalidLegacyAdapter);
            }
            let mut known = BTreeMap::<String, u64>::new();
            let mut tokens = Vec::new();
            for part in parts {
                let token = part.trim();
                if token.is_empty() || token.len() > 128 {
                    return Err(TransferRuntimeError::InvalidLegacyAdapter);
                }
                tokens.push(token.to_owned());
                let Some((key, value)) = token.split_once('=') else {
                    // QL is the only documented valueless flag. Unknown
                    // tokens remain round-trippable but never gain authority.
                    if !token.eq_ignore_ascii_case("QL") {
                        continue;
                    }
                    continue;
                };
                let key = key.trim().to_ascii_uppercase();
                let value = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| TransferRuntimeError::InvalidLegacyAdapter)?;
                if known.insert(key, value).is_some() {
                    return Err(TransferRuntimeError::InvalidLegacyAdapter);
                }
            }
            records.push(LegacyDailyLimitRecord {
                security_level,
                daily_files: known.get("DLPD").copied().filter(|value| *value != 0),
                daily_decimal_kilobytes: known.get("KB").copied(),
                ratio_warning: known.get("VWR").copied(),
                ratio_enforcement: known.get("VER").copied(),
                tokens,
            });
        }
        Ok(Self {
            source: source.to_vec(),
            records,
        })
    }

    pub fn transfer_policies(&self) -> Result<Vec<TransferPolicy>, TransferRuntimeError> {
        self.records
            .iter()
            .map(|record| {
                let mut policy = TransferPolicy::unlimited(record.security_level);
                policy.daily_file_limit = record.daily_files;
                policy.daily_byte_limit = record
                    .daily_decimal_kilobytes
                    .map(|value| {
                        value
                            .checked_mul(1000)
                            .ok_or(TransferRuntimeError::ResourceLimit)
                    })
                    .transpose()?;
                policy.ratio_warning_thousandths = record
                    .ratio_warning
                    .map(|value| {
                        value
                            .checked_mul(1000)
                            .ok_or(TransferRuntimeError::ResourceLimit)
                    })
                    .transpose()?;
                policy.ratio_enforcement_thousandths = record
                    .ratio_enforcement
                    .map(|value| {
                        value
                            .checked_mul(1000)
                            .ok_or(TransferRuntimeError::ResourceLimit)
                    })
                    .transpose()?;
                policy.validate()?;
                Ok(policy)
            })
            .collect()
    }
}

/// Lossless compatibility view of ordered FA<x>.TXT directory lines. Paths
/// are evidence only until an operator maps them to approved confined roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyExtendedStorageDocument {
    pub source: Vec<u8>,
    pub ordered_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyStorageRootMapping<'a> {
    pub legacy_path: &'a str,
    pub stable_key: &'a str,
    pub label: &'a str,
    pub configured_locator: &'a str,
}

impl LegacyExtendedStorageDocument {
    pub fn parse(source: &[u8]) -> Result<Self, TransferRuntimeError> {
        if source.len() > MAX_LEGACY_POLICY_BYTES || source.contains(&0) || !source.is_ascii() {
            return Err(TransferRuntimeError::InvalidLegacyAdapter);
        }
        let text =
            std::str::from_utf8(source).map_err(|_| TransferRuntimeError::InvalidLegacyAdapter)?;
        let mut ordered_paths = Vec::new();
        for raw_line in text.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            if ordered_paths.len() >= MAX_LEGACY_STORAGE_ROOTS || line.len() > 255 {
                return Err(TransferRuntimeError::ResourceLimit);
            }
            ordered_paths.push(line.to_owned());
        }
        Ok(Self {
            source: source.to_vec(),
            ordered_paths,
        })
    }
}

impl RuntimeDatabase {
    /// Restore-time normalization: external media is never assumed present at
    /// a new board root. Managed roots keep their snapshot state; external
    /// roots require an explicit operator probe after restore.
    pub fn normalize_external_storage_after_restore(
        &mut self,
        now: i64,
    ) -> Result<usize, TransferRuntimeError> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_storage_roots SET availability='unknown',state_version=state_version+1,updated_at=CURRENT_TIMESTAMP WHERE root_kind='external' AND availability!='unknown'",
            [],
        )?;
        if changed != 0 {
            transaction.execute(
                "INSERT INTO transfer_events(occurred_at,operation,outcome,detail) VALUES(?1,'storage-root-updated','committed',?2)",
                params![now, format!("external-roots={changed}")],
            )?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn active_transfers(
        &self,
        actor: FileActor,
    ) -> Result<Vec<ActiveTransferSummary>, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let mut statement = self.connection.prepare(
            "SELECT transfer_id,caller_id,node_id,direction,protocol,state,bytes_expected,bytes_transferred,state_version FROM transfer_records WHERE state NOT IN ('completed','cancelled','failed','needs-review') ORDER BY started_at,transfer_id",
        )?;
        let rows = statement.query_map([], |row| {
            let direction = match row.get::<_, String>(3)?.as_str() {
                "download" => TransferDirectionKind::Download,
                "upload" => TransferDirectionKind::Upload,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(ActiveTransferSummary {
                transfer_id: TransferId::new(row.get::<_, String>(0)?).map_err(sql_conversion)?,
                caller_id: crate::CallerId::new(row.get(1)?).map_err(caller_sql_conversion)?,
                node_id: NodeId::new(row.get(2)?).map_err(node_sql_conversion)?,
                direction,
                protocol: row.get(4)?,
                state: TransferRuntimeState::from_database_value(&row.get::<_, String>(5)?)
                    .map_err(sql_conversion)?,
                bytes_expected: stored_u64(row.get(6)?)?,
                bytes_transferred: stored_u64(row.get(7)?)?,
                state_version: stored_u64(row.get(8)?)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(TransferRuntimeError::Sqlite)
    }

    pub fn cancel_transfer_as_operator(
        &mut self,
        actor: FileActor,
        transfer_id: &TransferId,
        expected_version: u64,
        now: i64,
    ) -> Result<(), TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let (actual, reservation): (i64, Option<String>) = self
            .connection
            .query_row(
                "SELECT t.state_version,(SELECT reservation_id FROM transfer_quota_reservations r WHERE r.transfer_id=t.transfer_id) FROM transfer_records t WHERE t.transfer_id=?1",
                params![transfer_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?)),
            )
            .optional()?
            .ok_or(TransferRuntimeError::TransferNotFound)?;
        let actual = stored_u64(actual)?;
        if actual != expected_version {
            return Err(TransferRuntimeError::StaleVersion {
                expected: expected_version,
                actual,
            });
        }
        let reservation = reservation.ok_or(TransferRuntimeError::Conflict)?;
        self.release_transfer(
            &ReservationId::new(reservation)?,
            TransferRuntimeState::Cancelled,
            Some(TransferCancelSource::Operator),
            Some("operator-cancelled"),
            now,
        )
    }

    pub fn daily_transfer_usage(
        &self,
        actor: FileActor,
        caller_id: crate::CallerId,
        board_day: &str,
    ) -> Result<Option<DailyTransferUsage>, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) || board_day.len() != 10 {
            return Err(TransferRuntimeError::Unauthorized);
        }
        self.connection
            .query_row(
                "SELECT chargeable_download_files,chargeable_download_bytes,upload_credit_seconds,state_version FROM transfer_daily_usage WHERE caller_id=?1 AND board_day=?2 ORDER BY timezone_policy_version DESC LIMIT 1",
                params![caller_id.get(),board_day],
                |row| Ok(DailyTransferUsage {
                    caller_id,
                    board_day: board_day.to_owned(),
                    chargeable_download_files: stored_u64(row.get(0)?)?,
                    chargeable_download_bytes: stored_u64(row.get(1)?)?,
                    upload_credit_seconds: stored_u64(row.get(2)?)?,
                    state_version: stored_u64(row.get(3)?)?,
                }),
            )
            .optional()
            .map_err(TransferRuntimeError::Sqlite)
    }

    pub fn transfer_policy(
        &self,
        security_level: SecurityLevel,
    ) -> Result<TransferPolicy, TransferRuntimeError> {
        self.connection
            .query_row(
                "SELECT daily_file_limit,daily_byte_limit,ratio_warning_thousandths,ratio_enforcement_thousandths,ratio_violation_security,upload_credit_thousandths,upload_credit_file_cap_seconds,upload_credit_day_cap_seconds,protocol_mask,state_version FROM transfer_policies WHERE security_level=?1",
                params![security_level.get()],
                |row| {
                    Ok(TransferPolicy {
                        security_level,
                        daily_file_limit: optional_u64(row.get(0)?)?,
                        daily_byte_limit: optional_u64(row.get(1)?)?,
                        ratio_warning_thousandths: optional_u64(row.get(2)?)?,
                        ratio_enforcement_thousandths: optional_u64(row.get(3)?)?,
                        ratio_violation_security: row.get::<_,Option<u16>>(4)?.map(SecurityLevel::new).transpose().map_err(caller_sql_conversion)?,
                        upload_credit_thousandths: stored_u64(row.get(5)?)?,
                        upload_credit_file_cap_seconds: stored_u64(row.get(6)?)?,
                        upload_credit_day_cap_seconds: stored_u64(row.get(7)?)?,
                        protocol_mask: u16::try_from(row.get::<_, i64>(8)?).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(8, 0))?,
                        state_version: stored_u64(row.get(9)?)?,
                    })
                },
            )
            .optional()
            .map_err(TransferRuntimeError::Sqlite)?
            .map_or_else(
                || Ok(TransferPolicy::unlimited(security_level)),
                |policy| {
                    policy.validate()?;
                    Ok(policy)
                },
            )
    }

    pub fn download_ratio_status(
        &self,
        actor: FileActor,
    ) -> Result<RatioStatus, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        let policy = self.transfer_policy(caller.security_level)?;
        if ratio_exceeded(
            caller.files_downloaded,
            caller.files_uploaded,
            caller.download_bytes,
            caller.upload_bytes,
            policy.ratio_enforcement_thousandths,
        ) {
            Ok(RatioStatus::Denied)
        } else if ratio_exceeded(
            caller.files_downloaded,
            caller.files_uploaded,
            caller.download_bytes,
            caller.upload_bytes,
            policy.ratio_warning_thousandths,
        ) {
            Ok(RatioStatus::Warning)
        } else {
            Ok(RatioStatus::Healthy)
        }
    }

    pub fn update_transfer_policy(
        &mut self,
        actor: FileActor,
        policy: &TransferPolicy,
        expected_version: u64,
        now: i64,
    ) -> Result<TransferPolicy, TransferRuntimeError> {
        policy.validate()?;
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let transaction = self.connection.transaction()?;
        let current: Option<u64> = transaction
            .query_row(
                "SELECT state_version FROM transfer_policies WHERE security_level=?1",
                params![policy.security_level.get()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(stored_u64)
            .transpose()?;
        let current_version = current.unwrap_or(0);
        if current_version != expected_version {
            return Err(TransferRuntimeError::StaleVersion {
                expected: expected_version,
                actual: current_version,
            });
        }
        let next_version = current_version + 1;
        transaction.execute(
            "INSERT INTO transfer_policies(security_level,daily_file_limit,daily_byte_limit,ratio_warning_thousandths,ratio_enforcement_thousandths,ratio_violation_security,upload_credit_thousandths,upload_credit_file_cap_seconds,upload_credit_day_cap_seconds,protocol_mask,state_version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(security_level) DO UPDATE SET daily_file_limit=excluded.daily_file_limit,daily_byte_limit=excluded.daily_byte_limit,ratio_warning_thousandths=excluded.ratio_warning_thousandths,ratio_enforcement_thousandths=excluded.ratio_enforcement_thousandths,ratio_violation_security=excluded.ratio_violation_security,upload_credit_thousandths=excluded.upload_credit_thousandths,upload_credit_file_cap_seconds=excluded.upload_credit_file_cap_seconds,upload_credit_day_cap_seconds=excluded.upload_credit_day_cap_seconds,protocol_mask=excluded.protocol_mask,state_version=excluded.state_version,updated_at=CURRENT_TIMESTAMP",
            params![policy.security_level.get(), optional_i64(policy.daily_file_limit)?, optional_i64(policy.daily_byte_limit)?, optional_i64(policy.ratio_warning_thousandths)?, optional_i64(policy.ratio_enforcement_thousandths)?, policy.ratio_violation_security.map(SecurityLevel::get), sqlite_i64(policy.upload_credit_thousandths)?, sqlite_i64(policy.upload_credit_file_cap_seconds)?, sqlite_i64(policy.upload_credit_day_cap_seconds)?, i64::from(policy.protocol_mask), sqlite_i64(next_version)?],
        )?;
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,outcome,prior_version,next_version,detail) VALUES(?1,'transfer-policy-changed',?2,'committed',?3,?4,?5)",
            params![now, caller.id.get(), sqlite_i64(current_version)?, sqlite_i64(next_version)?, format!("security={}", policy.security_level.get())],
        )?;
        transaction.commit()?;
        self.transfer_policy(policy.security_level)
    }

    pub fn authorize_upload_protocol(
        &self,
        actor: FileActor,
        method: TransferMethod,
    ) -> Result<(), TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if self
            .transfer_policy(caller.security_level)?
            .allows_protocol(method)
        {
            Ok(())
        } else {
            Err(TransferRuntimeError::ProtocolUnsupported)
        }
    }

    pub fn synchronize_transfer_timezone(
        &mut self,
        timezone: Tz,
        now: i64,
    ) -> Result<u64, TransferRuntimeError> {
        let (current, version): (String, i64) = self.connection.query_row(
            "SELECT timezone_name,state_version FROM transfer_timezone_policy WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let version = stored_u64(version)?;
        if current == timezone.name() {
            return Ok(version);
        }
        let active: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM transfer_quota_reservations WHERE state='active'",
            [],
            |row| row.get(0),
        )?;
        if active != 0 {
            return Err(TransferRuntimeError::Conflict);
        }
        let next = version + 1;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE transfer_timezone_policy SET timezone_name=?1,state_version=?2,updated_at=CURRENT_TIMESTAMP WHERE singleton=1 AND state_version=?3",
            params![timezone.name(), sqlite_i64(next)?, sqlite_i64(version)?],
        )?;
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,outcome,prior_version,next_version,detail) VALUES(?1,'timezone-policy-changed','committed',?2,?3,?4)",
            params![now, sqlite_i64(version)?, sqlite_i64(next)?, timezone.name()],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_download_queue(
        &mut self,
        actor: FileActor,
        node_id: NodeId,
        timezone: Tz,
        method: TransferMethod,
        queue: &TransferQueue,
        now: i64,
    ) -> Result<QuotaReservation, TransferRuntimeError> {
        if queue.is_empty() {
            return Err(TransferRuntimeError::EmptyQueue);
        }
        if queue.len() > 1 && !method.supports_batch() {
            return Err(TransferRuntimeError::ProtocolUnsupportedForBatch);
        }
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        let policy = self.transfer_policy(caller.security_level)?;
        if !policy.allows_protocol(method) {
            return Err(TransferRuntimeError::ProtocolUnsupported);
        }
        let timezone_version = self.synchronize_transfer_timezone(timezone, now)?;
        let board_day = Utc
            .timestamp_opt(now, 0)
            .single()
            .ok_or(TransferRuntimeError::InvalidClock)?
            .with_timezone(&timezone)
            .date_naive()
            .to_string();

        for item in queue.items() {
            let (area, access) = self
                .authorized_area(actor, item.area_id)
                .map(|(_, area, access)| (area, access))
                .map_err(|_| TransferRuntimeError::Unauthorized)?;
            if access != FileAccess::Full {
                return Err(TransferRuntimeError::PreviewDenied);
            }
            let file = self
                .load_file_by_id(item.file_id)
                .map_err(|_| TransferRuntimeError::FileUnavailable)?
                .ok_or(TransferRuntimeError::FileUnavailable)?;
            if file.area_id != item.area_id
                || file.state_version != item.expected_file_version
                || file.lifecycle != FileLifecycle::Active
                || matches!(
                    file.integrity,
                    FileIntegrity::Missing | FileIntegrity::DigestMismatch
                )
                || file.size_bytes != item.bytes
                || area.no_charge != item.no_charge
            {
                return Err(TransferRuntimeError::StaleQueueItem(file.id));
            }
            let (root, locator) = self.resolve_file_storage(file.id)?;
            if root.availability != StorageAvailability::Available {
                return Err(TransferRuntimeError::StorageUnavailable);
            }
            if locator.relative_path.is_empty() {
                return Err(TransferRuntimeError::FileUnavailable);
            }
        }

        if ratio_exceeded(
            caller.files_downloaded,
            caller.files_uploaded,
            caller.download_bytes,
            caller.upload_bytes,
            policy.ratio_enforcement_thousandths,
        ) {
            return Err(TransferRuntimeError::RatioDenied);
        }

        let (chargeable_files, chargeable_bytes) = queue.chargeable_totals();
        let transfer_id = TransferId::generated(node_id.get() as i64);
        let reservation_id = ReservationId::for_transfer(&transfer_id);
        let transaction = self.connection.transaction()?;
        let active: (i64, i64) = transaction.query_row(
            "SELECT COALESCE(SUM(reserved_file_count),0),COALESCE(SUM(reserved_bytes),0) FROM transfer_quota_reservations WHERE caller_id=?1 AND board_day=?2 AND timezone_policy_version=?3 AND state='active' AND expires_at>?4",
            params![caller.id.get(), board_day, sqlite_i64(timezone_version)?, now],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let usage: (i64, i64) = transaction.query_row(
            "SELECT COALESCE(MAX(chargeable_download_files),0),COALESCE(MAX(chargeable_download_bytes),0) FROM transfer_daily_usage WHERE caller_id=?1 AND board_day=?2 AND timezone_policy_version=?3",
            params![caller.id.get(), board_day, sqlite_i64(timezone_version)?],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let projected_files = stored_u64(active.0)?
            .checked_add(stored_u64(usage.0)?)
            .and_then(|value| value.checked_add(chargeable_files))
            .ok_or(TransferRuntimeError::ResourceLimit)?;
        let projected_bytes = stored_u64(active.1)?
            .checked_add(stored_u64(usage.1)?)
            .and_then(|value| value.checked_add(chargeable_bytes))
            .ok_or(TransferRuntimeError::ResourceLimit)?;
        if policy
            .daily_file_limit
            .is_some_and(|limit| projected_files > limit)
            || policy
                .daily_byte_limit
                .is_some_and(|limit| projected_bytes > limit)
        {
            return Err(TransferRuntimeError::DailyLimitExceeded);
        }
        transaction.execute(
            "INSERT INTO transfer_records(transfer_id,caller_id,node_id,direction,protocol,state,bytes_expected,state_version,started_at,updated_at) VALUES(?1,?2,?3,'download',?4,'reserved',?5,1,?6,?6)",
            params![transfer_id.as_str(), caller.id.get(), node_id.get(), method.database_value(), sqlite_i64(queue.total_bytes())?, now],
        )?;
        transaction.execute(
            "INSERT INTO transfer_quota_reservations(reservation_id,transfer_id,caller_id,board_day,timezone_policy_version,policy_security_level,policy_state_version,reserved_file_count,reserved_bytes,state,state_version,expires_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'active',1,?10,?11,?11)",
            params![reservation_id.as_str(), transfer_id.as_str(), caller.id.get(), board_day, sqlite_i64(timezone_version)?, caller.security_level.get(), sqlite_i64(policy.state_version)?, sqlite_i64(chargeable_files)?, sqlite_i64(chargeable_bytes)?, now.saturating_add(RESERVATION_TTL_SECONDS), now],
        )?;
        for item in queue.items() {
            transaction.execute(
                "INSERT INTO transfer_quota_reservation_items(reservation_id,item_id,file_id,expected_file_version,reserved_bytes,no_charge) VALUES(?1,?2,?3,?4,?5,?6)",
                params![reservation_id.as_str(), item.item_id, item.file_id.get(), sqlite_i64(item.expected_file_version)?, sqlite_i64(item.bytes)?, item.no_charge],
            )?;
        }
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,transfer_id,reservation_id,protocol,direction,outcome,byte_count,detail) VALUES(?1,'reservation-created',?2,?3,?4,?5,'download','reserved',?6,?7)",
            params![now, caller.id.get(), transfer_id.as_str(), reservation_id.as_str(), method.database_value(), sqlite_i64(chargeable_bytes)?, format!("items={}", queue.len())],
        )?;
        let mut event = NewOperationalEvent::new(
            now,
            EventCategory::Transfer,
            EventSeverity::Info,
            "transfer.started",
            EventOutcome::Observed,
        );
        event.caller_id = Some(caller.id);
        event.node_id = Some(node_id.get());
        event.object_kind = Some("transfer".to_owned());
        event.object_id = Some(transfer_id.as_str().to_owned());
        event.correlation_id = Some(transfer_id.as_str().to_owned());
        event.idempotency_key = Some(format!("transfer-started-{}", transfer_id.as_str()));
        event.attributes = EventAttributes::Transfer {
            protocol: Some(method.database_value().to_owned()),
            direction: Some("download".to_owned()),
            bytes: Some(queue.total_bytes()),
            files: Some(
                u64::try_from(queue.len()).map_err(|_| TransferRuntimeError::ResourceLimit)?,
            ),
        };
        insert_operational_event_tx(&transaction, &event)?;
        transaction.commit()?;
        Ok(QuotaReservation {
            id: reservation_id,
            transfer_id,
            board_day,
            timezone_policy_version: timezone_version,
            chargeable_files,
            chargeable_bytes,
            state_version: 1,
        })
    }

    pub fn set_transfer_state(
        &mut self,
        transfer_id: &TransferId,
        change: TransferStateChange<'_>,
    ) -> Result<u64, TransferRuntimeError> {
        let TransferStateChange {
            expected_version,
            state,
            bytes_transferred,
            error_class,
            cancel_source,
            occurred_at: now,
        } = change;
        if error_class.is_some_and(|value| value.is_empty() || value.len() > 64) {
            return Err(TransferRuntimeError::InvalidErrorClass);
        }
        let next = expected_version + 1;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE transfer_records SET state=?2,bytes_transferred=?3,error_class=?4,cancel_source=?5,state_version=?6,updated_at=?7,completed_at=CASE WHEN ?8 THEN ?7 ELSE completed_at END WHERE transfer_id=?1 AND state_version=?9",
            params![transfer_id.as_str(), state.database_value(), sqlite_i64(bytes_transferred)?, error_class, cancel_source.map(TransferCancelSource::database_value), sqlite_i64(next)?, now, state.terminal(), sqlite_i64(expected_version)?],
        )?;
        if changed != 1 {
            let actual = transaction
                .query_row(
                    "SELECT state_version FROM transfer_records WHERE transfer_id=?1",
                    params![transfer_id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .map(stored_u64)
                .transpose()?
                .ok_or(TransferRuntimeError::TransferNotFound)?;
            return Err(TransferRuntimeError::StaleVersion {
                expected: expected_version,
                actual,
            });
        }
        let (caller_id, protocol, direction, node_id): (i64, String, String, i64) = transaction.query_row(
            "SELECT caller_id,protocol,direction,node_id FROM transfer_records WHERE transfer_id=?1",
            params![transfer_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let operation = match state {
            TransferRuntimeState::Completed => "transfer-completed",
            TransferRuntimeState::Cancelled => "transfer-cancelled",
            TransferRuntimeState::Failed | TransferRuntimeState::NeedsReview => "transfer-failed",
            _ => "transfer-state-changed",
        };
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,transfer_id,protocol,direction,outcome,prior_version,next_version,byte_count,detail) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![now,operation,caller_id,transfer_id.as_str(),protocol,direction,state.database_value(),sqlite_i64(expected_version)?,sqlite_i64(next)?,sqlite_i64(bytes_transferred)?,error_class.unwrap_or("state-transition")],
        )?;
        if state.terminal() {
            let (event_code, severity, outcome) = match state {
                TransferRuntimeState::Completed => (
                    "transfer.completed",
                    EventSeverity::Info,
                    EventOutcome::Succeeded,
                ),
                TransferRuntimeState::Cancelled => (
                    "transfer.cancelled",
                    EventSeverity::Notice,
                    EventOutcome::Cancelled,
                ),
                TransferRuntimeState::Failed | TransferRuntimeState::NeedsReview => (
                    "transfer.failed",
                    EventSeverity::Warning,
                    EventOutcome::Failed,
                ),
                _ => unreachable!("terminal transfer state checked above"),
            };
            let mut event = NewOperationalEvent::new(
                now,
                EventCategory::Transfer,
                severity,
                event_code,
                outcome,
            );
            event.caller_id = Some(
                crate::CallerId::new(caller_id)
                    .map_err(|_| TransferRuntimeError::InvalidStoredState)?,
            );
            event.node_id =
                Some(u32::try_from(node_id).map_err(|_| TransferRuntimeError::InvalidStoredState)?);
            event.object_kind = Some("transfer".to_owned());
            event.object_id = Some(transfer_id.as_str().to_owned());
            event.correlation_id = Some(transfer_id.as_str().to_owned());
            event.idempotency_key = Some(format!("transfer-state-{}-{next}", transfer_id.as_str()));
            event.retention_class = RetentionClass::SummarySource;
            event.attributes = EventAttributes::Transfer {
                protocol: Some(protocol),
                direction: Some(direction),
                bytes: Some(bytes_transferred),
                files: None,
            };
            insert_operational_event_tx(&transaction, &event)?;
        }
        transaction.commit()?;
        Ok(next)
    }

    pub fn settle_download_item(
        &mut self,
        reservation_id: &ReservationId,
        item_id: &str,
        completed_bytes: u64,
        now: i64,
    ) -> Result<bool, TransferRuntimeError> {
        validate_identifier(item_id)?;
        let transaction = self.connection.transaction()?;
        let reservation: (String, i64, String, i64, String, i64) = transaction
            .query_row(
                "SELECT r.transfer_id,r.caller_id,r.board_day,r.timezone_policy_version,r.state,t.node_id FROM transfer_quota_reservations r JOIN transfer_records t ON t.transfer_id=r.transfer_id WHERE r.reservation_id=?1",
                params![reservation_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
            )
            .optional()?
            .ok_or(TransferRuntimeError::ReservationNotFound)?;
        if reservation.4 == "needs-review" {
            return Err(TransferRuntimeError::RecoveryRequired);
        }
        let item: (i64, i64, bool, String) = transaction
            .query_row(
                "SELECT file_id,reserved_bytes,no_charge,state FROM transfer_quota_reservation_items WHERE reservation_id=?1 AND item_id=?2",
                params![reservation_id.as_str(), item_id],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
            )
            .optional()?
            .ok_or(TransferRuntimeError::QueueItemNotFound)?;
        if item.3 == "settled" {
            transaction.rollback()?;
            return Ok(false);
        }
        if item.3 != "reserved" || reservation.4 != "active" {
            return Err(TransferRuntimeError::Conflict);
        }
        let reserved_bytes = stored_u64(item.1)?;
        if completed_bytes != reserved_bytes {
            return Err(TransferRuntimeError::IntegrityFailure);
        }
        transaction.execute(
            "INSERT INTO transfer_settlements(transfer_id,item_id,file_id,direction,completed_bytes,no_charge,settled_at) VALUES(?1,?2,?3,'download',?4,?5,?6)",
            params![reservation.0, item_id, item.0, sqlite_i64(completed_bytes)?, item.2, now],
        )?;
        if !item.2 {
            transaction.execute(
                "INSERT INTO transfer_daily_usage(caller_id,board_day,timezone_policy_version,chargeable_download_files,chargeable_download_bytes) VALUES(?1,?2,?3,1,?4) ON CONFLICT(caller_id,board_day,timezone_policy_version) DO UPDATE SET chargeable_download_files=chargeable_download_files+1,chargeable_download_bytes=chargeable_download_bytes+excluded.chargeable_download_bytes,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP",
                params![reservation.1, reservation.2, reservation.3, sqlite_i64(completed_bytes)?],
            )?;
            transaction.execute(
                "UPDATE callers SET files_downloaded=files_downloaded+1,download_bytes=download_bytes+?2,updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1",
                params![reservation.1, sqlite_i64(completed_bytes)?],
            )?;
            apply_ratio_adjustment_if_required(&transaction, reservation.1, now)?;
        }
        transaction.execute(
            "UPDATE files SET download_count=download_count+1,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1",
            params![item.0],
        )?;
        transaction.execute(
            "UPDATE transfer_quota_reservation_items SET state='settled',settled_bytes=?3,state_version=state_version+1 WHERE reservation_id=?1 AND item_id=?2",
            params![reservation_id.as_str(), item_id, sqlite_i64(completed_bytes)?],
        )?;
        let remaining: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM transfer_quota_reservation_items WHERE reservation_id=?1 AND state='reserved'",
            params![reservation_id.as_str()],
            |row| row.get(0),
        )?;
        if remaining == 0 {
            transaction.execute(
                "UPDATE transfer_quota_reservations SET state='settled',state_version=state_version+1,updated_at=?2 WHERE reservation_id=?1",
                params![reservation_id.as_str(), now],
            )?;
        }
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,transfer_id,reservation_id,file_id,direction,outcome,byte_count,detail) VALUES(?1,'quota-settled',?2,?3,?4,?5,'download','completed',?6,?7)",
            params![now, reservation.1, reservation.0, reservation_id.as_str(), item.0, sqlite_i64(completed_bytes)?, if item.2 { "no-charge" } else { "chargeable" }],
        )?;
        let mut event = NewOperationalEvent::new(
            now,
            EventCategory::Transfer,
            EventSeverity::Info,
            "transfer.download.completed",
            EventOutcome::Succeeded,
        );
        event.caller_id = Some(
            crate::CallerId::new(reservation.1)
                .map_err(|_| TransferRuntimeError::InvalidStoredState)?,
        );
        event.node_id = Some(
            u32::try_from(reservation.5).map_err(|_| TransferRuntimeError::InvalidStoredState)?,
        );
        event.object_kind = Some("file".to_owned());
        event.object_id = Some(item.0.to_string());
        event.correlation_id = Some(reservation.0.clone());
        event.idempotency_key = Some(format!(
            "download-settled-{}-{item_id}",
            reservation_id.as_str()
        ));
        event.retention_class = RetentionClass::SummarySource;
        event.attributes = EventAttributes::Transfer {
            protocol: None,
            direction: Some("download".to_owned()),
            bytes: Some(completed_bytes),
            files: Some(1),
        };
        insert_operational_event_tx(&transaction, &event)?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn release_transfer(
        &mut self,
        reservation_id: &ReservationId,
        state: TransferRuntimeState,
        cancel_source: Option<TransferCancelSource>,
        error_class: Option<&str>,
        now: i64,
    ) -> Result<(), TransferRuntimeError> {
        if !matches!(
            state,
            TransferRuntimeState::Cancelled | TransferRuntimeState::Failed
        ) {
            return Err(TransferRuntimeError::InvalidTransition);
        }
        let transaction = self.connection.transaction()?;
        let (transfer_id, caller_id, reservation_state): (String, i64, String) = transaction
            .query_row(
                "SELECT transfer_id,caller_id,state FROM transfer_quota_reservations WHERE reservation_id=?1",
                params![reservation_id.as_str()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
            )
            .optional()?
            .ok_or(TransferRuntimeError::ReservationNotFound)?;
        if reservation_state == "released" {
            transaction.rollback()?;
            return Ok(());
        }
        if reservation_state != "active" {
            return Err(TransferRuntimeError::Conflict);
        }
        transaction.execute(
            "UPDATE transfer_quota_reservation_items SET state='released',state_version=state_version+1 WHERE reservation_id=?1 AND state='reserved'",
            params![reservation_id.as_str()],
        )?;
        transaction.execute(
            "UPDATE transfer_quota_reservations SET state='released',state_version=state_version+1,updated_at=?2 WHERE reservation_id=?1 AND state='active'",
            params![reservation_id.as_str(), now],
        )?;
        transaction.execute(
            "UPDATE transfer_records SET state=?2,cancel_source=?3,error_class=?4,state_version=state_version+1,updated_at=?5,completed_at=?5 WHERE transfer_id=?1 AND state NOT IN ('completed','cancelled','failed','needs-review')",
            params![transfer_id, state.database_value(), cancel_source.map(TransferCancelSource::database_value), error_class, now],
        )?;
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,transfer_id,reservation_id,direction,outcome,detail) VALUES(?1,'quota-released',?2,?3,?4,'download',?5,?6)",
            params![now, caller_id, transfer_id, reservation_id.as_str(), state.database_value(), error_class.unwrap_or("released")],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn apply_upload_credit(
        &mut self,
        request: UploadCreditRequest<'_>,
    ) -> Result<u64, TransferRuntimeError> {
        let UploadCreditRequest {
            transfer_id,
            item_id,
            actor,
            node_id,
            method,
            file_id,
            active_seconds,
            timezone,
            occurred_at: now,
        } = request;
        validate_identifier(item_id)?;
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        let file = self
            .load_file_by_id(file_id)
            .map_err(|_| TransferRuntimeError::FileUnavailable)?
            .ok_or(TransferRuntimeError::FileUnavailable)?;
        if file.uploader_caller_id != Some(caller.id)
            || !matches!(
                file.lifecycle,
                FileLifecycle::Active | FileLifecycle::PendingReview
            )
        {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let policy = self.transfer_policy(caller.security_level)?;
        if !policy.allows_protocol(method) {
            return Err(TransferRuntimeError::ProtocolUnsupported);
        }
        let timezone_version = self.synchronize_transfer_timezone(timezone, now)?;
        let board_day = Utc
            .timestamp_opt(now, 0)
            .single()
            .ok_or(TransferRuntimeError::InvalidClock)?
            .with_timezone(&timezone)
            .date_naive()
            .to_string();
        let raw = active_seconds
            .checked_mul(policy.upload_credit_thousandths)
            .ok_or(TransferRuntimeError::ResourceLimit)?
            / 1000;
        let per_file = if policy.upload_credit_file_cap_seconds == 0 {
            raw
        } else {
            raw.min(policy.upload_credit_file_cap_seconds)
        };
        let transaction = self.connection.transaction()?;
        resolve_ratio_adjustment_if_restored(&transaction, caller.id.get(), now)?;
        let existing: Option<u64> = transaction.query_row(
            "SELECT upload_credit_seconds FROM transfer_settlements WHERE transfer_id=?1 AND item_id=?2",
            params![transfer_id.as_str(), item_id],
            |row| row.get::<_,i64>(0),
        ).optional()?.map(stored_u64).transpose()?;
        if let Some(existing) = existing {
            transaction.rollback()?;
            return Ok(existing);
        }
        let used: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(upload_credit_seconds),0) FROM transfer_daily_usage WHERE caller_id=?1 AND board_day=?2 AND timezone_policy_version=?3",
            params![caller.id.get(), board_day, sqlite_i64(timezone_version)?],
            |row| row.get(0),
        )?;
        let day_remaining = if policy.upload_credit_day_cap_seconds == 0 {
            u64::MAX
        } else {
            policy
                .upload_credit_day_cap_seconds
                .saturating_sub(stored_u64(used)?)
        };
        let credit = per_file.min(day_remaining);
        transaction.execute(
            "INSERT INTO transfer_records(transfer_id,caller_id,node_id,direction,protocol,state,bytes_expected,bytes_transferred,state_version,started_at,updated_at,completed_at) VALUES(?1,?2,?3,'upload',?4,'completed',?5,?5,1,?6,?6,?6) ON CONFLICT(transfer_id) DO NOTHING",
            params![transfer_id.as_str(), caller.id.get(), node_id.get(), method.database_value(), sqlite_i64(file.size_bytes)?, now],
        )?;
        transaction.execute(
            "INSERT INTO transfer_settlements(transfer_id,item_id,file_id,direction,completed_bytes,no_charge,upload_credit_seconds,settled_at) VALUES(?1,?2,?3,'upload',?4,0,?5,?6)",
            params![transfer_id.as_str(), item_id, file.id.get(), sqlite_i64(file.size_bytes)?, sqlite_i64(credit)?, now],
        )?;
        transaction.execute(
            "INSERT INTO transfer_daily_usage(caller_id,board_day,timezone_policy_version,upload_credit_seconds) VALUES(?1,?2,?3,?4) ON CONFLICT(caller_id,board_day,timezone_policy_version) DO UPDATE SET upload_credit_seconds=upload_credit_seconds+excluded.upload_credit_seconds,state_version=state_version+1,updated_at=CURRENT_TIMESTAMP",
            params![caller.id.get(), board_day, sqlite_i64(timezone_version)?, sqlite_i64(credit)?],
        )?;
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,transfer_id,file_id,direction,outcome,byte_count,detail) VALUES(?1,'upload-credit-applied',?2,?3,?4,'upload','completed',?5,?6)",
            params![now, caller.id.get(), transfer_id.as_str(), file.id.get(), sqlite_i64(file.size_bytes)?, format!("credit-seconds={credit}")],
        )?;
        let mut event = NewOperationalEvent::new(
            now,
            EventCategory::Transfer,
            EventSeverity::Info,
            "transfer.upload.completed",
            EventOutcome::Succeeded,
        );
        event.caller_id = Some(caller.id);
        event.node_id = Some(node_id.get());
        event.object_kind = Some("file".to_owned());
        event.object_id = Some(file.id.get().to_string());
        event.correlation_id = Some(transfer_id.as_str().to_owned());
        event.idempotency_key = Some(format!("upload-settled-{}-{item_id}", transfer_id.as_str()));
        event.retention_class = RetentionClass::SummarySource;
        event.attributes = EventAttributes::Transfer {
            protocol: Some(method.database_value().to_owned()),
            direction: Some("upload".to_owned()),
            bytes: Some(file.size_bytes),
            files: Some(1),
        };
        insert_operational_event_tx(&transaction, &event)?;
        transaction.commit()?;
        Ok(credit)
    }

    pub fn storage_roots(
        &self,
        area_id: FileAreaId,
    ) -> Result<Vec<StorageRoot>, TransferRuntimeError> {
        let mut statement = self.connection.prepare(
            "SELECT storage_root_id,area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy,state_version FROM file_storage_roots WHERE area_id=?1 AND configured_state<>'disabled' ORDER BY priority,storage_root_id",
        )?;
        let rows = statement.query_map(params![area_id.get()], stored_root)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(TransferRuntimeError::Sqlite)
    }

    pub fn resolve_file_storage(
        &self,
        file_id: FileId,
    ) -> Result<(StorageRoot, FileStorageLocator), TransferRuntimeError> {
        self.connection
            .query_row(
                "SELECT r.storage_root_id,r.area_id,r.stable_key,r.label,r.root_kind,r.access_mode,r.priority,r.configured_locator,r.configured_state,r.availability,r.staging_policy,r.state_version,l.file_id,l.storage_root_id,l.relative_path,l.state_version FROM file_storage_locators l JOIN file_storage_roots r ON r.storage_root_id=l.storage_root_id WHERE l.file_id=?1 AND r.configured_state='enabled'",
                params![file_id.get()],
                |row| {
                    let root = stored_root(row)?;
                    let locator = FileStorageLocator {
                        file_id: FileId::new(row.get(12)?).map_err(file_sql_conversion)?,
                        storage_root_id: StorageRootId::new(row.get(13)?).map_err(sql_conversion)?,
                        relative_path: row.get(14)?,
                        state_version: stored_u64(row.get(15)?)?,
                    };
                    Ok((root, locator))
                },
            )
            .optional()?
            .ok_or(TransferRuntimeError::StorageUnavailable)
    }

    pub fn add_storage_root(
        &mut self,
        actor: FileActor,
        definition: StorageRootDefinition<'_>,
    ) -> Result<StorageRoot, TransferRuntimeError> {
        let StorageRootDefinition {
            area_id,
            stable_key,
            label,
            configured_locator,
            priority,
            mode,
            occurred_at: now,
        } = definition;
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security())
            || priority == 0
            || stable_key.is_empty()
            || stable_key.len() > 96
            || label.is_empty()
            || label.len() > 96
            || configured_locator.is_empty()
            || configured_locator.len() > 255
        {
            return Err(TransferRuntimeError::Unauthorized);
        }
        self.load_area_by_id(area_id)
            .map_err(|_| TransferRuntimeError::FileUnavailable)?
            .ok_or(TransferRuntimeError::FileUnavailable)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO file_storage_roots(area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy) VALUES(?1,?2,?3,'external',?4,?5,?6,'enabled','unknown','always-stage')",
            params![area_id.get(), stable_key, label, mode.database_value(), priority, configured_locator],
        )?;
        let id = StorageRootId::new(transaction.last_insert_rowid())?;
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,storage_root_id,outcome,next_version,detail) VALUES(?1,'storage-root-added',?2,?3,'committed',1,?4)",
            params![now, caller.id.get(), id.get(), stable_key],
        )?;
        transaction.commit()?;
        self.storage_roots(area_id)?
            .into_iter()
            .find(|root| root.id == id)
            .ok_or(TransferRuntimeError::StorageUnavailable)
    }

    /// Imports FA<x>.TXT ordering only through an explicit operator-approved
    /// mapping. Legacy path text remains evidence and never becomes native
    /// host-path authority.
    pub fn import_legacy_extended_roots(
        &mut self,
        actor: FileActor,
        area_id: FileAreaId,
        document: &LegacyExtendedStorageDocument,
        mappings: &[LegacyStorageRootMapping<'_>],
        now: i64,
    ) -> Result<Vec<StorageRoot>, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        if mappings.len() != document.ordered_paths.len()
            || mappings.len() > MAX_LEGACY_STORAGE_ROOTS
        {
            return Err(TransferRuntimeError::InvalidLegacyAdapter);
        }
        self.load_area_by_id(area_id)
            .map_err(|_| TransferRuntimeError::FileUnavailable)?
            .ok_or(TransferRuntimeError::FileUnavailable)?;
        let mut stable_keys = BTreeSet::new();
        for (index, mapping) in mappings.iter().enumerate() {
            let locator = std::path::Path::new(mapping.configured_locator);
            if mapping.legacy_path != document.ordered_paths[index]
                || validate_identifier(mapping.stable_key).is_err()
                || !stable_keys.insert(mapping.stable_key)
                || mapping.label.is_empty()
                || mapping.label.len() > 96
                || mapping.configured_locator.is_empty()
                || mapping.configured_locator.len() > 255
                || !locator.is_absolute()
            {
                return Err(TransferRuntimeError::InvalidLegacyAdapter);
            }
        }
        let transaction = self.connection.transaction()?;
        let mut ids = Vec::with_capacity(mappings.len());
        for (index, mapping) in mappings.iter().enumerate() {
            let priority =
                u8::try_from(index + 1).map_err(|_| TransferRuntimeError::ResourceLimit)?;
            transaction.execute(
                "INSERT INTO file_storage_roots(area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy) VALUES(?1,?2,?3,'external','read-only',?4,?5,'enabled','unknown','always-stage')",
                params![area_id.get(), mapping.stable_key, mapping.label, priority, mapping.configured_locator],
            )?;
            let id = StorageRootId::new(transaction.last_insert_rowid())?;
            ids.push(id);
            transaction.execute(
                "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,storage_root_id,outcome,next_version,detail) VALUES(?1,'storage-root-added',?2,?3,'committed',1,?4)",
                params![now, caller.id.get(), id.get(), format!("stable-key={}", mapping.stable_key)],
            )?;
        }
        transaction.commit()?;
        let roots = self.storage_roots(area_id)?;
        Ok(roots
            .into_iter()
            .filter(|root| ids.contains(&root.id))
            .collect())
    }

    pub fn set_storage_availability(
        &mut self,
        actor: FileActor,
        root_id: StorageRootId,
        expected_version: u64,
        availability: StorageAvailability,
        now: i64,
    ) -> Result<u64, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        if availability != StorageAvailability::Available {
            let active: i64 = self.connection.query_row(
                "SELECT COUNT(*) FROM file_active_uses u JOIN file_storage_locators l ON l.file_id=u.file_id WHERE l.storage_root_id=?1 AND u.expires_at>CURRENT_TIMESTAMP",
                params![root_id.get()],
                |row| row.get(0),
            )?;
            if active != 0 {
                return Err(TransferRuntimeError::Conflict);
            }
        }
        let next = expected_version + 1;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_storage_roots SET availability=?2,state_version=?3,updated_at=CURRENT_TIMESTAMP WHERE storage_root_id=?1 AND state_version=?4",
            params![root_id.get(), availability.database_value(), sqlite_i64(next)?, sqlite_i64(expected_version)?],
        )?;
        if changed != 1 {
            return Err(TransferRuntimeError::Conflict);
        }
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,storage_root_id,outcome,prior_version,next_version,detail) VALUES(?1,'storage-root-probed',?2,?3,'committed',?4,?5,?6)",
            params![now, caller.id.get(), root_id.get(), sqlite_i64(expected_version)?, sqlite_i64(next)?, availability.database_value()],
        )?;
        if availability != StorageAvailability::Available {
            let mut event = NewOperationalEvent::new(
                now,
                EventCategory::Storage,
                if availability == StorageAvailability::Unavailable {
                    EventSeverity::Warning
                } else {
                    EventSeverity::Notice
                },
                if availability == StorageAvailability::Unavailable {
                    "storage.unavailable"
                } else {
                    "storage.unknown"
                },
                if availability == StorageAvailability::Unavailable {
                    EventOutcome::Unavailable
                } else {
                    EventOutcome::Observed
                },
            );
            event.caller_id = Some(caller.id);
            event.object_kind = Some("storage-root".to_owned());
            event.object_id = Some(root_id.get().to_string());
            event.idempotency_key = Some(format!("storage-state-{}-{next}", root_id.get()));
            event.attributes = EventAttributes::Storage {
                state: availability.database_value().to_owned(),
            };
            insert_operational_event_tx(&transaction, &event)?;
        }
        transaction.commit()?;
        Ok(next)
    }

    /// Rebinds an external storage root without granting the supplied host
    /// path any caller-visible or audit authority.  A rebind always returns
    /// the root to `Unknown` availability so a separate filesystem probe must
    /// succeed before transfer preflight can use it.
    pub fn rebind_external_storage_root(
        &mut self,
        actor: FileActor,
        root_id: StorageRootId,
        expected_version: u64,
        configured_locator: &str,
        now: i64,
    ) -> Result<u64, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        let locator = std::path::Path::new(configured_locator);
        if !caller.security_level.is_sysop(actor.sysop_security())
            || configured_locator.is_empty()
            || configured_locator.len() > 255
            || !locator.is_absolute()
        {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let (kind, actual_version): (String, i64) = self
            .connection
            .query_row(
                "SELECT root_kind,state_version FROM file_storage_roots WHERE storage_root_id=?1",
                params![root_id.get()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(TransferRuntimeError::StorageUnavailable)?;
        if kind != "external" {
            return Err(TransferRuntimeError::Conflict);
        }
        let actual_version = stored_u64(actual_version)?;
        if actual_version != expected_version {
            return Err(TransferRuntimeError::StaleVersion {
                expected: expected_version,
                actual: actual_version,
            });
        }
        let active: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM file_active_uses u JOIN file_storage_locators l ON l.file_id=u.file_id WHERE l.storage_root_id=?1 AND u.expires_at>CURRENT_TIMESTAMP",
            params![root_id.get()],
            |row| row.get(0),
        )?;
        if active != 0 {
            return Err(TransferRuntimeError::Conflict);
        }
        let next = expected_version + 1;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_storage_roots SET configured_locator=?2,availability='unknown',state_version=?3,updated_at=CURRENT_TIMESTAMP WHERE storage_root_id=?1 AND state_version=?4",
            params![root_id.get(), configured_locator, sqlite_i64(next)?, sqlite_i64(expected_version)?],
        )?;
        if changed != 1 {
            return Err(TransferRuntimeError::Conflict);
        }
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,storage_root_id,outcome,prior_version,next_version,detail) VALUES(?1,'storage-root-updated',?2,?3,'committed',?4,?5,'external-root-rebound')",
            params![now, caller.id.get(), root_id.get(), sqlite_i64(expected_version)?, sqlite_i64(next)?],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub fn set_storage_root_state(
        &mut self,
        actor: FileActor,
        root_id: StorageRootId,
        expected_version: u64,
        state: StorageRootState,
        now: i64,
    ) -> Result<u64, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security()) {
            return Err(TransferRuntimeError::Unauthorized);
        }
        if state != StorageRootState::Enabled {
            let active: i64 = self.connection.query_row(
                "SELECT COUNT(*) FROM file_active_uses u JOIN file_storage_locators l ON l.file_id=u.file_id WHERE l.storage_root_id=?1 AND u.expires_at>CURRENT_TIMESTAMP",
                params![root_id.get()],
                |row| row.get(0),
            )?;
            if active != 0 {
                return Err(TransferRuntimeError::Conflict);
            }
        }
        let next = expected_version + 1;
        let changed = self.connection.execute(
            "UPDATE file_storage_roots SET configured_state=?2,state_version=?3,updated_at=CURRENT_TIMESTAMP WHERE storage_root_id=?1 AND state_version=?4",
            params![root_id.get(),state.database_value(),sqlite_i64(next)?,sqlite_i64(expected_version)?],
        )?;
        if changed != 1 {
            return Err(TransferRuntimeError::Conflict);
        }
        let operation = if state == StorageRootState::Disabled {
            "storage-root-disabled"
        } else {
            "storage-root-updated"
        };
        self.connection.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,storage_root_id,outcome,prior_version,next_version,detail) VALUES(?1,?2,?3,?4,'committed',?5,?6,?7)",
            params![now,operation,caller.id.get(),root_id.get(),sqlite_i64(expected_version)?,sqlite_i64(next)?,state.database_value()],
        )?;
        Ok(next)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_file_storage_locator(
        &mut self,
        actor: FileActor,
        file_id: FileId,
        root_id: StorageRootId,
        relative_path: &str,
        expected_file_version: u64,
        expected_locator_version: u64,
        now: i64,
    ) -> Result<FileStorageLocator, TransferRuntimeError> {
        let caller = self
            .active_file_actor(actor)
            .map_err(|_| TransferRuntimeError::Unauthorized)?;
        if !caller.security_level.is_sysop(actor.sysop_security())
            || relative_path.is_empty()
            || relative_path.len() > 255
            || std::path::Path::new(relative_path).is_absolute()
            || std::path::Path::new(relative_path)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(TransferRuntimeError::Unauthorized);
        }
        let file = self
            .load_file_by_id(file_id)
            .map_err(|_| TransferRuntimeError::FileUnavailable)?
            .ok_or(TransferRuntimeError::FileUnavailable)?;
        if file.state_version != expected_file_version {
            return Err(TransferRuntimeError::StaleVersion {
                expected: expected_file_version,
                actual: file.state_version,
            });
        }
        let root = self
            .storage_roots(file.area_id)?
            .into_iter()
            .find(|root| root.id == root_id)
            .ok_or(TransferRuntimeError::StorageUnavailable)?;
        if root.area_id != file.area_id {
            return Err(TransferRuntimeError::Conflict);
        }
        let active: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM file_active_uses WHERE file_id=?1 AND expires_at>CURRENT_TIMESTAMP",
            params![file.id.get()],
            |row| row.get(0),
        )?;
        if active != 0 {
            return Err(TransferRuntimeError::Conflict);
        }
        let next_locator = expected_locator_version + 1;
        let next_file = expected_file_version + 1;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE file_storage_locators SET storage_root_id=?2,relative_path=?3,state_version=?4,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?5",
            params![file.id.get(), root.id.get(), relative_path, sqlite_i64(next_locator)?, sqlite_i64(expected_locator_version)?],
        )?;
        if changed != 1 {
            return Err(TransferRuntimeError::Conflict);
        }
        let changed = transaction.execute(
            "UPDATE files SET state_version=?2,updated_at=CURRENT_TIMESTAMP WHERE file_id=?1 AND state_version=?3",
            params![file.id.get(), sqlite_i64(next_file)?, sqlite_i64(expected_file_version)?],
        )?;
        if changed != 1 {
            return Err(TransferRuntimeError::Conflict);
        }
        transaction.execute(
            "INSERT INTO transfer_events(occurred_at,operation,actor_caller_id,file_id,storage_root_id,outcome,prior_version,next_version,detail) VALUES(?1,'storage-locator-updated',?2,?3,?4,'committed',?5,?6,'locator-changed')",
            params![now, caller.id.get(), file.id.get(), root.id.get(), sqlite_i64(expected_locator_version)?, sqlite_i64(next_locator)?],
        )?;
        transaction.commit()?;
        Ok(FileStorageLocator {
            file_id,
            storage_root_id: root_id,
            relative_path: relative_path.to_owned(),
            state_version: next_locator,
        })
    }

    pub fn transfer_operations_ready_for_cold_backup(&self) -> Result<bool, TransferRuntimeError> {
        let count: i64 = self.connection.query_row(
            "SELECT (SELECT COUNT(*) FROM transfer_records WHERE state NOT IN ('completed','cancelled','failed','needs-review')) + (SELECT COUNT(*) FROM transfer_quota_reservations WHERE state='active')",
            [],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    pub fn reconcile_interrupted_transfers(
        &mut self,
        now: i64,
    ) -> Result<u64, TransferRuntimeError> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE transfer_quota_reservations SET state='released',state_version=state_version+1,updated_at=?1 WHERE state='active'",
            params![now],
        )?;
        transaction.execute(
            "UPDATE transfer_quota_reservation_items SET state='released',state_version=state_version+1 WHERE state='reserved' AND reservation_id IN (SELECT reservation_id FROM transfer_quota_reservations WHERE state='released')",
            [],
        )?;
        transaction.execute(
            "UPDATE transfer_records SET state='failed',error_class='daemon-restart',state_version=state_version+1,updated_at=?1,completed_at=?1 WHERE state NOT IN ('completed','cancelled','failed','needs-review') AND transfer_id IN (SELECT transfer_id FROM transfer_quota_reservations WHERE state='released')",
            params![now],
        )?;
        transaction.commit()?;
        u64::try_from(changed).map_err(|_| TransferRuntimeError::ResourceLimit)
    }
}

fn stored_root(row: &rusqlite::Row<'_>) -> rusqlite::Result<StorageRoot> {
    Ok(StorageRoot {
        id: StorageRootId::new(row.get(0)?).map_err(sql_conversion)?,
        area_id: FileAreaId::new(row.get(1)?).map_err(file_sql_conversion)?,
        stable_key: row.get(2)?,
        label: row.get(3)?,
        kind: StorageRootKind::from_database_value(&row.get::<_, String>(4)?)
            .map_err(sql_conversion)?,
        mode: StorageRootMode::from_database_value(&row.get::<_, String>(5)?)
            .map_err(sql_conversion)?,
        priority: u8::try_from(row.get::<_, i64>(6)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, 0))?,
        configured_locator: row.get(7)?,
        configured_state: StorageRootState::from_database_value(&row.get::<_, String>(8)?)
            .map_err(sql_conversion)?,
        availability: StorageAvailability::from_database_value(&row.get::<_, String>(9)?)
            .map_err(sql_conversion)?,
        staging_always: row.get::<_, String>(10)? == "always-stage",
        state_version: stored_u64(row.get(11)?)?,
    })
}

fn ratio_exceeded(
    downloaded_files: u64,
    uploaded_files: u64,
    downloaded_bytes: u64,
    uploaded_bytes: u64,
    threshold: Option<u64>,
) -> bool {
    let Some(threshold) = threshold else {
        return false;
    };
    ratio_component_exceeded(downloaded_files, uploaded_files, threshold)
        && ratio_component_exceeded(downloaded_bytes, uploaded_bytes, threshold)
}

fn ratio_component_exceeded(downloaded: u64, uploaded: u64, threshold: u64) -> bool {
    if downloaded == 0 {
        return false;
    }
    if uploaded == 0 {
        return true;
    }
    u128::from(downloaded) * 1000 > u128::from(uploaded) * u128::from(threshold)
}

fn apply_ratio_adjustment_if_required(
    transaction: &rusqlite::Transaction<'_>,
    caller_id: i64,
    now: i64,
) -> Result<(), TransferRuntimeError> {
    let caller: (i64, i64, i64, i64, i64) = transaction.query_row(
        "SELECT security_level,files_downloaded,files_uploaded,download_bytes,upload_bytes FROM callers WHERE caller_id=?1",
        params![caller_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
    )?;
    let policy: Option<(i64, i64, i64)> = transaction
        .query_row(
            "SELECT ratio_enforcement_thousandths,ratio_violation_security,state_version FROM transfer_policies WHERE security_level=?1 AND ratio_enforcement_thousandths IS NOT NULL AND ratio_violation_security IS NOT NULL",
            params![caller.0],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        )
        .optional()?;
    let Some((threshold, target, generation)) = policy else {
        return Ok(());
    };
    if !ratio_exceeded(
        stored_u64(caller.1)?,
        stored_u64(caller.2)?,
        stored_u64(caller.3)?,
        stored_u64(caller.4)?,
        Some(stored_u64(threshold)?),
    ) {
        return Ok(());
    }
    let active: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM caller_security_adjustments WHERE caller_id=?1 AND kind='ratio-violation' AND status='active')",
        params![caller_id],
        |row| row.get(0),
    )?;
    if active {
        return Ok(());
    }
    transaction.execute(
        "INSERT INTO caller_access_events(occurred_at,operation,subject_caller_id,actor_kind,adjustment_kind,policy_generation) VALUES(?1,'ratio-adjustment-applied',?2,'system-policy','ratio-violation',?3)",
        params![now,caller_id,generation],
    )?;
    let event_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO caller_security_adjustments(caller_id,kind,target_security_level,status,applied_at,applied_event_id) VALUES(?1,'ratio-violation',?2,'active',?3,?4)",
        params![caller_id,target,now,event_id],
    )?;
    Ok(())
}

fn resolve_ratio_adjustment_if_restored(
    transaction: &rusqlite::Transaction<'_>,
    caller_id: i64,
    now: i64,
) -> Result<(), TransferRuntimeError> {
    let active: Option<i64> = transaction
        .query_row(
            "SELECT adjustment_id FROM caller_security_adjustments WHERE caller_id=?1 AND kind='ratio-violation' AND status='active'",
            params![caller_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(adjustment_id) = active else {
        return Ok(());
    };
    let caller: (i64, i64, i64, i64, i64) = transaction.query_row(
        "SELECT security_level,files_downloaded,files_uploaded,download_bytes,upload_bytes FROM callers WHERE caller_id=?1",
        params![caller_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
    )?;
    let threshold: Option<i64> = transaction
        .query_row(
            "SELECT ratio_enforcement_thousandths FROM transfer_policies WHERE security_level=?1",
            params![caller.0],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if ratio_exceeded(
        stored_u64(caller.1)?,
        stored_u64(caller.2)?,
        stored_u64(caller.3)?,
        stored_u64(caller.4)?,
        threshold.map(stored_u64).transpose()?,
    ) {
        return Ok(());
    }
    transaction.execute(
        "INSERT INTO caller_access_events(occurred_at,operation,subject_caller_id,actor_kind,adjustment_kind) VALUES(?1,'ratio-adjustment-resolved',?2,'system-policy','ratio-violation')",
        params![now,caller_id],
    )?;
    let event_id = transaction.last_insert_rowid();
    transaction.execute(
        "UPDATE caller_security_adjustments SET status='resolved',resolved_at=?2,resolved_event_id=?3,state_version=state_version+1 WHERE adjustment_id=?1 AND status='active'",
        params![adjustment_id,now,event_id],
    )?;
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), TransferRuntimeError> {
    if value.is_empty()
        || value.len() > 96
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(TransferRuntimeError::InvalidIdentifier);
    }
    Ok(())
}

fn stored_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

fn optional_u64(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value.map(stored_u64).transpose()
}

fn sqlite_i64(value: u64) -> Result<i64, TransferRuntimeError> {
    i64::try_from(value).map_err(|_| TransferRuntimeError::ResourceLimit)
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>, TransferRuntimeError> {
    value.map(sqlite_i64).transpose()
}

fn sql_conversion(error: TransferRuntimeError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn file_sql_conversion(error: crate::FileError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(error))
}

fn caller_sql_conversion(error: crate::CallerError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(error))
}

fn node_sql_conversion(error: crate::NodeError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(error))
}

#[derive(Debug, Error)]
pub enum TransferRuntimeError {
    #[error(transparent)]
    Database(#[from] crate::DatabaseError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("caller is not authorized for this transfer operation")]
    Unauthorized,
    #[error("Preview access does not permit transfer")]
    PreviewDenied,
    #[error("the transfer queue is empty")]
    EmptyQueue,
    #[error("the transfer queue reached its bounded item limit")]
    QueueFull,
    #[error("the selected protocol does not support a multi-file queue")]
    ProtocolUnsupportedForBatch,
    #[error("the selected transfer protocol is disabled")]
    ProtocolUnsupported,
    #[error("a queued file changed after it was tagged")]
    StaleQueueItem(FileId),
    #[error("the requested file is unavailable")]
    FileUnavailable,
    #[error("the configured storage root is unavailable")]
    StorageUnavailable,
    #[error("the daily transfer limit would be exceeded")]
    DailyLimitExceeded,
    #[error("the caller's upload/download ratio denies this transfer")]
    RatioDenied,
    #[error("the transfer or queue exceeds a configured resource bound")]
    ResourceLimit,
    #[error("the transfer policy is invalid")]
    InvalidPolicy,
    #[error("the transfer clock is outside the supported range")]
    InvalidClock,
    #[error("the expected version {expected} is stale; current version is {actual}")]
    StaleVersion { expected: u64, actual: u64 },
    #[error("the transfer does not exist")]
    TransferNotFound,
    #[error("the quota reservation does not exist")]
    ReservationNotFound,
    #[error("the queue item does not exist")]
    QueueItemNotFound,
    #[error("the operation conflicts with current authoritative state")]
    Conflict,
    #[error("the operation needs explicit recovery review")]
    RecoveryRequired,
    #[error("the completed bytes did not match the reservation")]
    IntegrityFailure,
    #[error("the transfer transition is invalid")]
    InvalidTransition,
    #[error("the transfer identifier is invalid")]
    InvalidIdentifier,
    #[error("the transfer error class is invalid")]
    InvalidErrorClass,
    #[error("the storage-root identifier is invalid")]
    InvalidStorageRootId,
    #[error("stored transfer state is invalid")]
    InvalidStoredState,
    #[error("legacy transfer/storage adapter input is invalid")]
    InvalidLegacyAdapter,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BoardIdentity, CallerState, CredentialHasher, FileAccessMode, FileAreaDefinition,
        FileLifecycle, NewFileEntry, PasswordHashConfig,
    };

    struct Fixture {
        _temp: tempfile::TempDir,
        database: RuntimeDatabase,
        actor: FileActor,
        area_id: FileAreaId,
        file: FileEntry,
    }

    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("runtime.sqlite3");
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        database
            .ensure_board_identity(&BoardIdentity::new("Transfer Test", "Sysop").unwrap())
            .unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"test-only transfer password").unwrap();
        let caller = database
            .create_caller(
                b"Transfer Caller",
                &hash,
                SecurityLevel::new(50).unwrap(),
                CallerState::Active,
                false,
                1_700_000_000,
            )
            .unwrap();
        let area = database
            .ensure_file_area(&FileAreaDefinition {
                number: 1,
                name: "Files".to_owned(),
                description: "Transfer fixtures".to_owned(),
                storage_key: "files".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                upload_security: SecurityLevel::new(5).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1_048_576,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let file = database
            .insert_file_entry(&NewFileEntry {
                area_id: area.id,
                filename: "QUEUE.TXT".to_owned(),
                description: "Queue fixture".to_owned(),
                size_bytes: 12,
                sha256: "11".repeat(32),
                uploaded_at: 1_700_000_000,
                uploader_caller_id: None,
                uploader_name: "SPITFIRE NG".to_owned(),
                lifecycle: FileLifecycle::Active,
            })
            .unwrap();
        let actor = FileActor::new(caller.id, SecurityLevel::new(50).unwrap());
        Fixture {
            _temp: temp,
            database,
            actor,
            area_id: area.id,
            file,
        }
    }

    #[test]
    fn queues_are_bounded_ordered_and_idempotent() {
        let fixture = fixture();
        let mut queue = TransferQueue::default();
        assert!(queue.tag(&fixture.file, false).unwrap());
        assert!(!queue.tag(&fixture.file, false).unwrap());
        assert_eq!(queue.chargeable_totals(), (1, 12));
        assert!(queue.untag(fixture.file.id));
        assert!(queue.is_empty());
    }

    #[test]
    fn ratio_warning_and_enforcement_are_distinct_and_require_both_dimensions() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.ratio_warning_thousandths = Some(2_000);
        policy.ratio_enforcement_thousandths = Some(3_000);
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        fixture
            .database
            .connection
            .execute(
                "UPDATE callers SET files_downloaded=5,files_uploaded=2,download_bytes=500,upload_bytes=200 WHERE caller_id=?1",
                params![fixture.actor.caller_id().get()],
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .download_ratio_status(fixture.actor)
                .unwrap(),
            RatioStatus::Warning
        );
        fixture
            .database
            .connection
            .execute(
                "UPDATE callers SET files_downloaded=7,download_bytes=700 WHERE caller_id=?1",
                params![fixture.actor.caller_id().get()],
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .download_ratio_status(fixture.actor)
                .unwrap(),
            RatioStatus::Denied
        );
        fixture
            .database
            .connection
            .execute(
                "UPDATE callers SET upload_bytes=1000 WHERE caller_id=?1",
                params![fixture.actor.caller_id().get()],
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .download_ratio_status(fixture.actor)
                .unwrap(),
            RatioStatus::Healthy
        );
    }

    #[test]
    fn partial_batch_retains_failed_and_unstarted_items_in_order() {
        let mut fixture = fixture();
        let second = fixture
            .database
            .insert_file_entry(&NewFileEntry {
                area_id: fixture.area_id,
                filename: "SECOND.TXT".to_owned(),
                description: "Second queue fixture".to_owned(),
                size_bytes: 7,
                sha256: "22".repeat(32),
                uploaded_at: 1_700_000_001,
                uploader_caller_id: None,
                uploader_name: "SPITFIRE NG".to_owned(),
                lifecycle: FileLifecycle::Active,
            })
            .unwrap();
        let third = fixture
            .database
            .insert_file_entry(&NewFileEntry {
                area_id: fixture.area_id,
                filename: "THIRD.TXT".to_owned(),
                description: "Third queue fixture".to_owned(),
                size_bytes: 9,
                sha256: "33".repeat(32),
                uploaded_at: 1_700_000_002,
                uploader_caller_id: None,
                uploader_name: "SPITFIRE NG".to_owned(),
                lifecycle: FileLifecycle::Active,
            })
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        queue.tag(&second, false).unwrap();
        queue.tag(&third, false).unwrap();
        queue.retain_unsettled(&[queue.items()[0].item_id.clone()]);
        assert_eq!(
            queue
                .items()
                .iter()
                .map(|item| item.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["SECOND.TXT", "THIRD.TXT"]
        );
        assert_eq!(queue.total_bytes(), 16);
    }

    #[test]
    fn whole_batch_reservation_settles_once_and_releases_no_charge() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.daily_file_limit = Some(1);
        policy.daily_byte_limit = Some(12);
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Binary(TransferProtocol::YmodemBatch),
                &queue,
                1_700_000_001,
            )
            .unwrap();
        assert!(fixture
            .database
            .settle_download_item(
                &reservation.id,
                &queue.items()[0].item_id,
                12,
                1_700_000_002,
            )
            .unwrap());
        assert!(!fixture
            .database
            .settle_download_item(
                &reservation.id,
                &queue.items()[0].item_id,
                12,
                1_700_000_003,
            )
            .unwrap());
        let caller = fixture
            .database
            .caller_by_id(fixture.actor.caller_id())
            .unwrap()
            .unwrap();
        assert_eq!((caller.files_downloaded, caller.download_bytes), (1, 12));
    }

    #[test]
    fn same_caller_cannot_overspend_daily_limit_on_two_nodes() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.daily_file_limit = Some(1);
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Binary(TransferProtocol::YmodemBatch),
                &queue,
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.reserve_download_queue(
                fixture.actor,
                NodeId::new(2).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Binary(TransferProtocol::YmodemBatch),
                &queue,
                1_700_000_002,
            ),
            Err(TransferRuntimeError::DailyLimitExceeded)
        ));
    }

    #[test]
    fn spring_forward_no_dst_midnight_and_cross_midnight_settlement_use_civil_days() {
        let mut fixture = fixture();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let before_jump = Utc
            .with_ymd_and_hms(2026, 3, 8, 6, 59, 0)
            .single()
            .unwrap()
            .timestamp();
        let after_jump = Utc
            .with_ymd_and_hms(2026, 3, 8, 7, 1, 0)
            .single()
            .unwrap()
            .timestamp();
        let first = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::America::New_York,
                TransferMethod::Ascii,
                &queue,
                before_jump,
            )
            .unwrap();
        assert_eq!(first.board_day, "2026-03-08");
        fixture
            .database
            .release_transfer(
                &first.id,
                TransferRuntimeState::Cancelled,
                Some(TransferCancelSource::Caller),
                Some("clock-test"),
                after_jump,
            )
            .unwrap();
        let second = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(2).unwrap(),
                chrono_tz::America::New_York,
                TransferMethod::Ascii,
                &queue,
                after_jump,
            )
            .unwrap();
        assert_eq!(second.board_day, first.board_day);
        fixture
            .database
            .release_transfer(
                &second.id,
                TransferRuntimeState::Cancelled,
                Some(TransferCancelSource::Caller),
                Some("clock-test"),
                after_jump + 1,
            )
            .unwrap();

        let before_midnight = Utc
            .with_ymd_and_hms(2026, 6, 2, 6, 59, 0)
            .single()
            .unwrap()
            .timestamp();
        let after_midnight = Utc
            .with_ymd_and_hms(2026, 6, 2, 7, 1, 0)
            .single()
            .unwrap()
            .timestamp();
        let phoenix = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(3).unwrap(),
                chrono_tz::America::Phoenix,
                TransferMethod::Ascii,
                &queue,
                before_midnight,
            )
            .unwrap();
        assert_eq!(phoenix.board_day, "2026-06-01");
        fixture
            .database
            .settle_download_item(
                &phoenix.id,
                &queue.items()[0].item_id,
                fixture.file.size_bytes,
                after_midnight,
            )
            .unwrap();
        let usage: (String, i64) = fixture
            .database
            .connection
            .query_row(
                "SELECT board_day,chargeable_download_files FROM transfer_daily_usage WHERE caller_id=?1",
                params![fixture.actor.caller_id().get()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(usage, ("2026-06-01".to_owned(), 1));
    }

    #[test]
    fn daily_limit_boundaries_and_fall_back_hour_share_one_board_day() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.daily_file_limit = Some(2);
        policy.daily_byte_limit = Some(24);
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let timezone = chrono_tz::America::New_York;
        let first_repeated_hour = Utc
            .with_ymd_and_hms(2026, 11, 1, 5, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        let second_repeated_hour = Utc
            .with_ymd_and_hms(2026, 11, 1, 6, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        let first = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                timezone,
                TransferMethod::Ascii,
                &queue,
                first_repeated_hour,
            )
            .unwrap();
        assert_eq!(first.board_day, "2026-11-01");
        fixture
            .database
            .settle_download_item(
                &first.id,
                &queue.items()[0].item_id,
                fixture.file.size_bytes,
                first_repeated_hour + 1,
            )
            .unwrap();
        let second = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(2).unwrap(),
                timezone,
                TransferMethod::Ascii,
                &queue,
                second_repeated_hour,
            )
            .unwrap();
        assert_eq!(second.board_day, first.board_day);
        fixture
            .database
            .settle_download_item(
                &second.id,
                &queue.items()[0].item_id,
                fixture.file.size_bytes,
                second_repeated_hour + 1,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.reserve_download_queue(
                fixture.actor,
                NodeId::new(3).unwrap(),
                timezone,
                TransferMethod::Ascii,
                &queue,
                second_repeated_hour + 2,
            ),
            Err(TransferRuntimeError::DailyLimitExceeded)
        ));
    }

    #[test]
    fn timezone_change_conflicts_with_an_active_reservation() {
        let mut fixture = fixture();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Ascii,
                &queue,
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture
                .database
                .synchronize_transfer_timezone(chrono_tz::America::Phoenix, 1_700_000_002),
            Err(TransferRuntimeError::Conflict)
        ));
        fixture
            .database
            .release_transfer(
                &reservation.id,
                TransferRuntimeState::Cancelled,
                Some(TransferCancelSource::Caller),
                Some("caller-cancelled"),
                1_700_000_003,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .synchronize_transfer_timezone(chrono_tz::America::Phoenix, 1_700_000_004)
                .unwrap(),
            2
        );
    }

    #[test]
    fn successful_chargeable_download_applies_and_upload_restores_ratio_adjustment() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.ratio_enforcement_thousandths = Some(1_000);
        policy.ratio_violation_security = Some(SecurityLevel::new(5).unwrap());
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Ascii,
                &queue,
                1_700_000_001,
            )
            .unwrap();
        fixture
            .database
            .settle_download_item(
                &reservation.id,
                &queue.items()[0].item_id,
                fixture.file.size_bytes,
                1_700_000_002,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .caller_by_id(fixture.actor.caller_id())
                .unwrap()
                .unwrap()
                .security_level,
            SecurityLevel::new(5).unwrap()
        );

        fixture
            .database
            .connection
            .execute(
                "UPDATE callers SET files_uploaded=2,upload_bytes=100 WHERE caller_id=?1",
                params![fixture.actor.caller_id().get()],
            )
            .unwrap();
        let transaction = fixture.database.connection.transaction().unwrap();
        resolve_ratio_adjustment_if_restored(
            &transaction,
            fixture.actor.caller_id().get(),
            1_700_000_003,
        )
        .unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            fixture
                .database
                .caller_by_id(fixture.actor.caller_id())
                .unwrap()
                .unwrap()
                .security_level,
            SecurityLevel::new(50).unwrap()
        );
    }

    #[test]
    fn transfer_cancellation_is_idempotent_and_releases_quota_once() {
        let mut fixture = fixture();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Ascii,
                &queue,
                1_700_000_001,
            )
            .unwrap();
        for _ in 0..2 {
            fixture
                .database
                .release_transfer(
                    &reservation.id,
                    TransferRuntimeState::Cancelled,
                    Some(TransferCancelSource::Caller),
                    Some("caller-cancelled"),
                    1_700_000_002,
                )
                .unwrap();
        }
        let events: i64 = fixture
            .database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM transfer_events WHERE operation='quota-released'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
    }

    #[test]
    fn daemon_restart_reconciliation_releases_transfer_authority_once() {
        let mut fixture = fixture();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Binary(TransferProtocol::YmodemGBatch),
                &queue,
                1_700_000_001,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .reconcile_interrupted_transfers(1_700_000_002)
                .unwrap(),
            1
        );
        assert_eq!(
            fixture
                .database
                .reconcile_interrupted_transfers(1_700_000_003)
                .unwrap(),
            0
        );
        let state: (String, String, String) = fixture
            .database
            .connection
            .query_row(
                "SELECT t.state,t.error_class,r.state FROM transfer_records t JOIN transfer_quota_reservations r ON r.transfer_id=t.transfer_id WHERE r.reservation_id=?1",
                params![reservation.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            state,
            (
                "failed".to_owned(),
                "daemon-restart".to_owned(),
                "released".to_owned()
            )
        );
        assert_eq!(
            fixture
                .database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM transfer_settlements WHERE transfer_id=?1",
                    params![reservation.transfer_id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn no_charge_settlement_excludes_caller_and_daily_ratio_counters() {
        let mut fixture = fixture();
        fixture
            .database
            .connection
            .execute(
                "UPDATE file_areas SET no_charge=1 WHERE area_id=?1",
                params![fixture.area_id.get()],
            )
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, true).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Ascii,
                &queue,
                1_700_000_001,
            )
            .unwrap();
        fixture
            .database
            .settle_download_item(
                &reservation.id,
                &queue.items()[0].item_id,
                fixture.file.size_bytes,
                1_700_000_002,
            )
            .unwrap();
        let caller = fixture
            .database
            .caller_by_id(fixture.actor.caller_id())
            .unwrap()
            .unwrap();
        assert_eq!((caller.files_downloaded, caller.download_bytes), (0, 0));
        let usage: i64 = fixture
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM transfer_daily_usage", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(usage, 0);
    }

    #[test]
    fn upload_credit_is_fixed_point_capped_and_idempotent() {
        let mut fixture = fixture();
        fixture
            .database
            .connection
            .execute(
                "UPDATE files SET uploader_caller_id=?2 WHERE file_id=?1",
                params![fixture.file.id.get(), fixture.actor.caller_id().get()],
            )
            .unwrap();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.upload_credit_thousandths = 2_000;
        policy.upload_credit_file_cap_seconds = 30;
        policy.upload_credit_day_cap_seconds = 50;
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        let transfer = TransferId::new("upload-credit-test").unwrap();
        for _ in 0..2 {
            assert_eq!(
                fixture
                    .database
                    .apply_upload_credit(UploadCreditRequest {
                        transfer_id: &transfer,
                        item_id: "item-1",
                        actor: fixture.actor,
                        node_id: NodeId::new(1).unwrap(),
                        method: TransferMethod::Binary(TransferProtocol::Telink),
                        file_id: fixture.file.id,
                        active_seconds: 20,
                        timezone: chrono_tz::UTC,
                        occurred_at: 1_700_000_001,
                    })
                    .unwrap(),
                30
            );
        }
        let settlements: i64 = fixture
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM transfer_settlements", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(settlements, 1);

        let pending = fixture
            .database
            .insert_file_entry(&NewFileEntry {
                area_id: fixture.area_id,
                filename: "PENDING.BIN".to_owned(),
                description: "Pending credit fixture".to_owned(),
                size_bytes: 4,
                sha256: "44".repeat(32),
                uploaded_at: 1_700_000_002,
                uploader_caller_id: Some(fixture.actor.caller_id()),
                uploader_name: "Transfer Caller".to_owned(),
                lifecycle: FileLifecycle::PendingReview,
            })
            .unwrap();
        assert_eq!(
            fixture
                .database
                .apply_upload_credit(UploadCreditRequest {
                    transfer_id: &TransferId::new("upload-credit-pending").unwrap(),
                    item_id: "item-2",
                    actor: fixture.actor,
                    node_id: NodeId::new(2).unwrap(),
                    method: TransferMethod::Binary(TransferProtocol::Telink),
                    file_id: pending.id,
                    active_seconds: 10_000,
                    timezone: chrono_tz::UTC,
                    occurred_at: 1_700_000_002,
                })
                .unwrap(),
            20
        );
        let third = fixture
            .database
            .insert_file_entry(&NewFileEntry {
                area_id: fixture.area_id,
                filename: "THIRDUP.BIN".to_owned(),
                description: "Day cap fixture".to_owned(),
                size_bytes: 4,
                sha256: "55".repeat(32),
                uploaded_at: 1_700_000_003,
                uploader_caller_id: Some(fixture.actor.caller_id()),
                uploader_name: "Transfer Caller".to_owned(),
                lifecycle: FileLifecycle::Active,
            })
            .unwrap();
        assert_eq!(
            fixture
                .database
                .apply_upload_credit(UploadCreditRequest {
                    transfer_id: &TransferId::new("upload-credit-day-cap").unwrap(),
                    item_id: "item-3",
                    actor: fixture.actor,
                    node_id: NodeId::new(3).unwrap(),
                    method: TransferMethod::Binary(TransferProtocol::Telink),
                    file_id: third.id,
                    active_seconds: 10_000,
                    timezone: chrono_tz::UTC,
                    occurred_at: 1_700_000_003,
                })
                .unwrap(),
            0
        );
    }

    #[test]
    fn upload_protocol_policy_is_reauthorized_before_and_after_receive() {
        let mut fixture = fixture();
        let mut policy = TransferPolicy::unlimited(SecurityLevel::new(50).unwrap());
        policy.protocol_mask &= !TransferMethod::Binary(TransferProtocol::Telink).mask();
        fixture
            .database
            .update_transfer_policy(fixture.actor, &policy, 0, 1_700_000_000)
            .unwrap();
        assert!(matches!(
            fixture.database.authorize_upload_protocol(
                fixture.actor,
                TransferMethod::Binary(TransferProtocol::Telink)
            ),
            Err(TransferRuntimeError::ProtocolUnsupported)
        ));

        fixture
            .database
            .connection
            .execute(
                "UPDATE files SET uploader_caller_id=?2 WHERE file_id=?1",
                params![fixture.file.id.get(), fixture.actor.caller_id().get()],
            )
            .unwrap();
        assert!(matches!(
            fixture.database.apply_upload_credit(UploadCreditRequest {
                transfer_id: &TransferId::new("blocked-upload-protocol").unwrap(),
                item_id: "item-1",
                actor: fixture.actor,
                node_id: NodeId::new(1).unwrap(),
                method: TransferMethod::Binary(TransferProtocol::Telink),
                file_id: fixture.file.id,
                active_seconds: 20,
                timezone: chrono_tz::UTC,
                occurred_at: 1_700_000_001,
            }),
            Err(TransferRuntimeError::ProtocolUnsupported)
        ));
    }

    #[test]
    fn operator_transfer_projection_and_cancel_are_versioned() {
        let mut fixture = fixture();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let reservation = fixture
            .database
            .reserve_download_queue(
                fixture.actor,
                NodeId::new(2).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Binary(TransferProtocol::Telink),
                &queue,
                1_700_000_001,
            )
            .unwrap();
        let active = fixture.database.active_transfers(fixture.actor).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].transfer_id, reservation.transfer_id);
        assert_eq!(active[0].node_id, NodeId::new(2).unwrap());
        assert!(matches!(
            fixture.database.cancel_transfer_as_operator(
                fixture.actor,
                &reservation.transfer_id,
                2,
                1_700_000_002,
            ),
            Err(TransferRuntimeError::StaleVersion {
                expected: 2,
                actual: 1
            })
        ));
        fixture
            .database
            .cancel_transfer_as_operator(fixture.actor, &reservation.transfer_id, 1, 1_700_000_002)
            .unwrap();
        assert!(fixture
            .database
            .active_transfers(fixture.actor)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn read_only_external_root_locator_and_active_use_are_authoritative() {
        let mut fixture = fixture();
        let external = fixture._temp.path().join("external-read-only");
        std::fs::create_dir(&external).unwrap();
        let root = fixture
            .database
            .add_storage_root(
                fixture.actor,
                StorageRootDefinition {
                    area_id: fixture.area_id,
                    stable_key: "archive-one",
                    label: "Archive One",
                    configured_locator: external.to_str().unwrap(),
                    priority: 1,
                    mode: StorageRootMode::ReadOnly,
                    occurred_at: 1_700_000_000,
                },
            )
            .unwrap();
        let root_version = fixture
            .database
            .set_storage_availability(
                fixture.actor,
                root.id,
                root.state_version,
                StorageAvailability::Available,
                1_700_000_001,
            )
            .unwrap();
        fixture
            .database
            .set_file_storage_locator(
                fixture.actor,
                fixture.file.id,
                root.id,
                "QUEUE.TXT",
                fixture.file.state_version,
                1,
                1_700_000_002,
            )
            .unwrap();
        let resolved = fixture
            .database
            .resolve_file_storage(fixture.file.id)
            .unwrap();
        assert_eq!(resolved.0.mode, StorageRootMode::ReadOnly);
        assert_eq!(resolved.0.availability, StorageAvailability::Available);

        let use_token = fixture
            .database
            .begin_file_download_use(
                fixture.actor,
                fixture.file.id,
                crate::SessionId::new(1).unwrap(),
            )
            .unwrap();
        assert!(matches!(
            fixture.database.set_storage_root_state(
                fixture.actor,
                root.id,
                root_version,
                StorageRootState::Maintenance,
                1_700_000_003,
            ),
            Err(TransferRuntimeError::Conflict)
        ));
        fixture.database.finish_file_use(use_token).unwrap();
        fixture
            .database
            .set_storage_root_state(
                fixture.actor,
                root.id,
                root_version,
                StorageRootState::Maintenance,
                1_700_000_004,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.resolve_file_storage(fixture.file.id),
            Err(TransferRuntimeError::StorageUnavailable)
        ));
    }

    #[test]
    fn external_root_rebind_is_versioned_private_and_requires_reprobe() {
        let mut fixture = fixture();
        let original = fixture._temp.path().join("external-original");
        let rebound = fixture._temp.path().join("external-rebound");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&rebound).unwrap();
        let root = fixture
            .database
            .add_storage_root(
                fixture.actor,
                StorageRootDefinition {
                    area_id: fixture.area_id,
                    stable_key: "rebind-test",
                    label: "Rebind Test",
                    configured_locator: original.to_str().unwrap(),
                    priority: 1,
                    mode: StorageRootMode::ReadOnly,
                    occurred_at: 1_700_000_000,
                },
            )
            .unwrap();
        let available_version = fixture
            .database
            .set_storage_availability(
                fixture.actor,
                root.id,
                root.state_version,
                StorageAvailability::Available,
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            fixture.database.rebind_external_storage_root(
                fixture.actor,
                root.id,
                available_version + 1,
                rebound.to_str().unwrap(),
                1_700_000_002,
            ),
            Err(TransferRuntimeError::StaleVersion { .. })
        ));
        let rebound_version = fixture
            .database
            .rebind_external_storage_root(
                fixture.actor,
                root.id,
                available_version,
                rebound.to_str().unwrap(),
                1_700_000_003,
            )
            .unwrap();
        let rebound_root = fixture
            .database
            .storage_roots(fixture.area_id)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == root.id)
            .unwrap();
        assert_eq!(rebound_root.availability, StorageAvailability::Unknown);
        assert_eq!(rebound_root.state_version, rebound_version);
        let leaked: i64 = fixture
            .database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM transfer_events WHERE detail LIKE '%' || ?1 || '%'",
                params![rebound.to_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(leaked, 0);
    }

    #[test]
    fn legacy_extended_roots_require_explicit_ordered_confined_mapping() {
        let mut fixture = fixture();
        let one = fixture._temp.path().join("archive-one");
        let two = fixture._temp.path().join("archive-two");
        std::fs::create_dir(&one).unwrap();
        std::fs::create_dir(&two).unwrap();
        let document = LegacyExtendedStorageDocument::parse(b"D:\\FILES\r\nE:\\MORE\r\n").unwrap();
        let mappings = [
            LegacyStorageRootMapping {
                legacy_path: "D:\\FILES",
                stable_key: "fa-one",
                label: "Archive one",
                configured_locator: one.to_str().unwrap(),
            },
            LegacyStorageRootMapping {
                legacy_path: "E:\\MORE",
                stable_key: "fa-two",
                label: "Archive two",
                configured_locator: two.to_str().unwrap(),
            },
        ];
        let roots = fixture
            .database
            .import_legacy_extended_roots(
                fixture.actor,
                fixture.area_id,
                &document,
                &mappings,
                1_700_000_000,
            )
            .unwrap();
        assert_eq!(
            roots
                .iter()
                .map(|root| (root.stable_key.as_str(), root.priority, root.mode))
                .collect::<Vec<_>>(),
            vec![
                ("fa-one", 1, StorageRootMode::ReadOnly),
                ("fa-two", 2, StorageRootMode::ReadOnly),
            ]
        );
        let bad = [LegacyStorageRootMapping {
            legacy_path: "WRONG",
            stable_key: "bad",
            label: "Bad",
            configured_locator: one.to_str().unwrap(),
        }];
        assert!(matches!(
            fixture.database.import_legacy_extended_roots(
                fixture.actor,
                fixture.area_id,
                &document,
                &bad,
                1_700_000_001,
            ),
            Err(TransferRuntimeError::InvalidLegacyAdapter)
        ));
    }

    #[test]
    fn restore_normalization_never_assumes_external_media_is_present() {
        let mut fixture = fixture();
        let external = fixture._temp.path().join("restore-media");
        std::fs::create_dir(&external).unwrap();
        let root = fixture
            .database
            .add_storage_root(
                fixture.actor,
                StorageRootDefinition {
                    area_id: fixture.area_id,
                    stable_key: "restore-media",
                    label: "Restore media",
                    configured_locator: external.to_str().unwrap(),
                    priority: 1,
                    mode: StorageRootMode::ReadOnly,
                    occurred_at: 1_700_000_000,
                },
            )
            .unwrap();
        fixture
            .database
            .set_storage_availability(
                fixture.actor,
                root.id,
                root.state_version,
                StorageAvailability::Available,
                1_700_000_001,
            )
            .unwrap();
        assert_eq!(
            fixture
                .database
                .normalize_external_storage_after_restore(1_700_000_002)
                .unwrap(),
            1
        );
        let normalized = fixture
            .database
            .storage_roots(fixture.area_id)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == root.id)
            .unwrap();
        assert_eq!(normalized.availability, StorageAvailability::Unknown);
        assert_eq!(
            fixture
                .database
                .normalize_external_storage_after_restore(1_700_000_003)
                .unwrap(),
            0
        );
    }

    #[test]
    fn migrated_primary_root_and_locator_are_authoritative() {
        let fixture = fixture();
        let roots = fixture.database.storage_roots(fixture.area_id).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].kind, StorageRootKind::Managed);
        assert_eq!(roots[0].mode, StorageRootMode::ReadWrite);
        let (root, locator) = fixture
            .database
            .resolve_file_storage(fixture.file.id)
            .unwrap();
        assert_eq!(root.id, roots[0].id);
        assert_eq!(locator.relative_path, "QUEUE.TXT");
    }

    #[test]
    fn preview_is_rejected_before_reservation() {
        let mut fixture = fixture();
        fixture
            .database
            .connection
            .execute(
                "UPDATE file_areas SET read_security=100,preview=1 WHERE area_id=?1",
                params![fixture.area_id.get()],
            )
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&fixture.file, false).unwrap();
        let preview_actor =
            FileActor::new(fixture.actor.caller_id(), SecurityLevel::new(100).unwrap());
        assert!(matches!(
            fixture.database.reserve_download_queue(
                preview_actor,
                NodeId::new(1).unwrap(),
                chrono_tz::UTC,
                TransferMethod::Ascii,
                &queue,
                1_700_000_000,
            ),
            Err(TransferRuntimeError::PreviewDenied)
        ));
    }

    #[test]
    fn legacy_daily_limit_adapter_is_bounded_lossless_and_decimal_kb() {
        let source = b"10,MPC=45,DLPD=15,KB=2000,VWR=15,VER=25,QL\r\n";
        let document = LegacyDailyLimitDocument::parse(source).unwrap();
        assert_eq!(document.source, source);
        assert_eq!(document.records.len(), 1);
        let policies = document.transfer_policies().unwrap();
        assert_eq!(policies[0].daily_file_limit, Some(15));
        assert_eq!(policies[0].daily_byte_limit, Some(2_000_000));
        assert_eq!(policies[0].ratio_warning_thousandths, Some(15_000));
        assert_eq!(policies[0].ratio_enforcement_thousandths, Some(25_000));
        assert!(LegacyDailyLimitDocument::parse(b"10,DLPD=15\n10,DLPD=20\n").is_err());
        assert!(LegacyDailyLimitDocument::parse(b"10,DLPD=15,DLPD=20\n").is_err());
        assert!(LegacyDailyLimitDocument::parse(b"10,DLPD=\n").is_err());
        assert!(LegacyDailyLimitDocument::parse(b"10,DLPD=18446744073709551616\n").is_err());
    }

    #[test]
    fn legacy_extended_roots_preserve_order_without_granting_path_authority() {
        let source = b"D:\\FILES\r\nE:\\CDROM\\AREA1\r\n";
        let document = LegacyExtendedStorageDocument::parse(source).unwrap();
        assert_eq!(document.source, source);
        assert_eq!(
            document.ordered_paths,
            vec!["D:\\FILES".to_owned(), "E:\\CDROM\\AREA1".to_owned()]
        );
        let excessive = "X\n".repeat(MAX_LEGACY_STORAGE_ROOTS + 1);
        assert!(matches!(
            LegacyExtendedStorageDocument::parse(excessive.as_bytes()),
            Err(TransferRuntimeError::ResourceLimit)
        ));
    }
}
