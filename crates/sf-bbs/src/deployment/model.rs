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

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ensure, Error, Result};

pub const FORMAT: u32 = 1;
pub const PRODUCT: &str = "spitfire-ng";
pub const RELEASE_DOMAIN: &[u8] = b"SPITFIRE-NG-RELEASE-V1\n";
pub const LOCATOR: &str = ".spitfire-installation.toml";
pub const MANIFEST: &str = "spitfire-recovery.json";
pub const MAX_DOCUMENT: u64 = 16 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 100_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub minimum: u32,
    pub maximum: u32,
}
impl Range {
    pub fn contains(&self, value: u32) -> bool {
        self.minimum <= value && value <= self.maximum
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDescriptor {
    pub format: u32,
    pub product: String,
    pub version: String,
    pub platform: String,
    pub architecture: String,
    pub manager_protocol: u32,
    pub read_schema: Range,
    pub write_schema: Range,
    pub migration_source: Range,
    pub target_schema: u32,
    pub config_formats: Vec<u32>,
    pub durable_features: BTreeMap<String, u32>,
}

pub fn durable_features() -> BTreeMap<String, u32> {
    [
        "circuitnet-catalog-history",
        "circuitnet-custody",
        "circuitnet-publication",
        "ftn-custody",
        "qwk-custody",
        "message-identity",
        "files-identity",
        "trust-enrollment",
        "events-history",
        "caller-session-custody",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), 1))
    .collect()
}

impl RuntimeDescriptor {
    pub fn current() -> Self {
        let schema = sf_core::SCHEMA_VERSION;
        Self {
            format: FORMAT,
            product: PRODUCT.into(),
            version: sf_core::PRODUCT_VERSION.into(),
            platform: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            manager_protocol: FORMAT,
            read_schema: Range {
                minimum: schema,
                maximum: schema,
            },
            write_schema: Range {
                minimum: schema,
                maximum: schema,
            },
            migration_source: Range {
                minimum: 10,
                maximum: schema,
            },
            target_schema: schema,
            config_formats: vec![1, sf_core::CONFIG_FORMAT_VERSION],
            durable_features: durable_features(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.format == FORMAT && self.product == PRODUCT && self.manager_protocol == FORMAT,
            "unsupported runtime product or deployment protocol",
        )?;
        version(&self.version)?;
        ensure(
            self.platform == std::env::consts::OS && self.architecture == std::env::consts::ARCH,
            "release platform/architecture does not match this host",
        )?;
        for range in [
            &self.read_schema,
            &self.write_schema,
            &self.migration_source,
        ] {
            ensure(
                range.minimum > 0 && range.minimum <= range.maximum,
                "invalid runtime schema range",
            )?;
        }
        ensure(
            self.read_schema.contains(self.target_schema)
                && self.write_schema.contains(self.target_schema)
                && self.migration_source.contains(self.target_schema),
            "runtime cannot operate its target schema",
        )?;
        ensure(
            !self.config_formats.is_empty()
                && self.config_formats.len() <= 16
                && self.durable_features.len() <= 64,
            "invalid runtime capabilities",
        )
    }
    pub fn compatible(&self, state: &BoardState) -> Result<()> {
        self.validate()?;
        ensure(
            self.read_schema.contains(state.schema) && self.write_schema.contains(state.schema),
            &format!(
                "SPITFIRE NG {} cannot safely read and write current board schema {}",
                self.version, state.schema
            ),
        )?;
        ensure(
            self.config_formats.contains(&state.config_format),
            "runtime cannot operate current configuration format",
        )?;
        for (name, required) in &state.required_features {
            ensure(
                self.durable_features
                    .get(name)
                    .is_some_and(|found| found >= required),
                &format!(
                    "runtime cannot preserve required durable feature {name} generation {required}"
                ),
            )?;
        }
        Ok(())
    }
}

pub fn version(value: &str) -> Result<semver::Version> {
    let parsed = semver::Version::parse(value)
        .map_err(|_| Error::Rejected("invalid release SemVer".into()))?;
    ensure(
        parsed.to_string() == value && value.len() <= 96 && parsed.build.is_empty(),
        "release version must be canonical SemVer without build metadata",
    )?;
    Ok(parsed)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub format: u32,
    pub runtime: RuntimeDescriptor,
    pub minimum_upgrade_version: String,
    pub executable: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRecord {
    pub manifest: ReleaseManifest,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BoardState {
    pub schema: u32,
    pub config_format: u32,
    pub board_name: String,
    pub sysop_name: String,
    pub required_features: BTreeMap<String, u32>,
    pub counts: BTreeMap<String, u64>,
    pub identities: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadPolicy {
    ManagedOnly,
    MetadataOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoragePolicy {
    pub file_payloads: PayloadPolicy,
    pub backup_max_bytes: u64,
    pub backup_retention_bytes: u64,
    pub minimum_free_bytes: u64,
    pub reserve_free_bytes: u64,
    pub hard_stop_free_bytes: u64,
    pub low_space_warning_bytes: u64,
    pub max_managed_files_bytes: u64,
    pub max_file_area_bytes: u64,
    pub max_single_file_bytes: u64,
}
impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            file_payloads: PayloadPolicy::ManagedOnly,
            backup_max_bytes: 16 * 1024 * 1024 * 1024,
            backup_retention_bytes: 64 * 1024 * 1024 * 1024,
            minimum_free_bytes: 64 * 1024 * 1024,
            reserve_free_bytes: 256 * 1024 * 1024,
            hard_stop_free_bytes: 64 * 1024 * 1024,
            low_space_warning_bytes: 1024 * 1024 * 1024,
            max_managed_files_bytes: u64::MAX,
            max_file_area_bytes: u64::MAX,
            max_single_file_bytes: 1024 * 1024 * 1024,
        }
    }
}
impl StoragePolicy {
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.backup_max_bytes > 0 && self.backup_retention_bytes >= self.backup_max_bytes,
            "backup retention must accommodate at least one maximum backup",
        )?;
        ensure(
            self.low_space_warning_bytes >= self.hard_stop_free_bytes,
            "low-space warning must not be below the hard-stop threshold",
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub format: u32,
    pub id: String,
    pub config: PathBuf,
    pub source: PathBuf,
    pub release_public_key: String,
    pub storage: StoragePolicy,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Locator {
    pub format: u32,
    pub id: String,
    pub installation: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    Backup,
    VerifyBackup,
    StageRelease,
    VerifyRelease,
    Migrate,
    Validate,
    ApplyRuntime,
    ValidateActive,
    Commit,
    Cleanup,
    ManualRestore,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    pub id: String,
    pub phase: Phase,
    pub old: String,
    pub target: String,
    pub checkpoint: Option<String>,
    pub checkpoint_sha256: Option<String>,
    pub stage_sha256: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub format: u32,
    pub installation_id: String,
    pub active: String,
    pub previous: Vec<String>,
    pub releases: BTreeMap<String, ReleaseRecord>,
    pub required_features: BTreeMap<String, u32>,
    pub backups: BTreeMap<String, String>,
    pub transaction: Option<Transaction>,
    pub last_operation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StateClass {
    Critical,
    ManagedPayload,
    ExternalReference,
    Cache,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub class: StateClass,
    pub size_bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PayloadReference {
    pub class: StateClass,
    pub file_id: Option<i64>,
    pub area: Option<u16>,
    pub path: String,
    pub storage_root: String,
    pub availability: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub copied: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub format: u32,
    pub id: String,
    pub installation_id: String,
    pub created_at: i64,
    pub backup_type: String,
    pub installation: Installation,
    pub source_runtime: RuntimeDescriptor,
    pub state: BoardState,
    pub config_name: String,
    pub payload_policy: PayloadPolicy,
    pub entries: Vec<Entry>,
    pub references: Vec<PayloadReference>,
    pub excluded_classes: Vec<String>,
    pub total_logical_bytes: u64,
    pub stored_content_bytes: u64,
    pub restore_requirements: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequest {
    pub format: u32,
    pub operation: String,
    pub config: PathBuf,
    pub payload_root: PathBuf,
}
