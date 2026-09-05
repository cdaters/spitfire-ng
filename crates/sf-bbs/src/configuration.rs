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

//! Typed file-backed configuration authority. UI clients never own persistence.
use crate::board_lock::BoardOperationLock;
use crate::{ApplicationError, OperatorControlError};
use serde::{Deserialize, Serialize};
use sf_core::configuration::*;
use sf_core::{
    LocalOperatorCapability, LocalOperatorIdentity, LogicalPaths, RuntimeConfig, RuntimeDatabase,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const CONFIGURATION_MINOR: u16 = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretStatus {
    Missing,
    Configured,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigurationSnapshot {
    pub version: ConfigurationVersion,
    pub config: RuntimeConfig,
    pub restart_required: bool,
    pub ssh_keys: Vec<SecretStatus>,
    pub capabilities: Vec<LocalOperatorCapability>,
    pub domains: Vec<ConfigurationDomainSummary>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigurationDomainSummary {
    pub kind: String,
    pub number: u16,
    pub name: String,
    pub active: bool,
    pub read_security: u16,
    pub write_security: u16,
    pub version: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ConfigurationResult {
    Saved {
        version: ConfigurationVersion,
        effects: Vec<ConfigurationEffect>,
        restart_required: bool,
    },
    Conflict {
        current: ConfigurationVersion,
    },
    Invalid {
        issues: Vec<ConfigurationIssue>,
    },
    Replayed {
        result_class: Option<String>,
        revision: Option<u64>,
    },
    Denied,
    RecoveryRequired,
}

pub(crate) struct ConfigurationAuthority {
    path: PathBuf,
    database: PathBuf,
    active: RuntimeConfig,
    gate: Mutex<RuntimeConfig>,
}

fn failure() -> ApplicationError {
    OperatorControlError::Service("configuration authority unavailable".into()).into()
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn configuration_version(
    config: &RuntimeConfig,
) -> Result<ConfigurationVersion, ApplicationError> {
    let mut canonical = config.clone();
    canonical.configuration_commit = None;
    let bytes = canonical.to_toml()?;
    Ok(ConfigurationVersion {
        revision: config.revision,
        digest: format!("{:x}", Sha256::digest(bytes.as_bytes())),
    })
}

impl ConfigurationAuthority {
    pub(crate) fn new(path: PathBuf, config: RuntimeConfig) -> Result<Self, ApplicationError> {
        let paths = LogicalPaths::resolve(path.parent().ok_or_else(failure)?, &config.validate()?)?;
        let authority = Self {
            path,
            database: paths.database().to_path_buf(),
            active: config.clone(),
            gate: Mutex::new(config),
        };
        authority.recover()?;
        Ok(authority)
    }
    fn recover(&self) -> Result<(), ApplicationError> {
        let config = RuntimeConfig::load(&self.path)?;
        if let Some(commit) = &config.configuration_commit {
            // A manual recovery edit changes the digest. Never use its old link
            // as proof of the altered payload; require explicit recovery.
            if configuration_version(&config)?.digest != commit.digest {
                return Err(failure());
            }
            RuntimeDatabase::open(&self.database)?.finish_configuration_command(
                commit,
                config.revision,
                now(),
            )?;
        }
        RuntimeDatabase::open(&self.database)?.reject_uncommitted_configuration_commands(now())?;
        Ok(())
    }
    fn restart_required(&self, stored: &RuntimeConfig) -> bool {
        ConfigurationField::fields(stored).iter().any(|field| {
            field.effect() == ConfigurationEffect::RestartRequired
                && field.value(stored) != field.value(&self.active)
        }) || cfg!(windows) && stored.operators != self.active.operators
    }
    pub(crate) fn current(&self) -> Result<RuntimeConfig, ApplicationError> {
        Ok(self.gate.lock().map_err(|_| failure())?.clone())
    }
    pub(crate) fn snapshot(
        &self,
        principal: &str,
        offline: bool,
    ) -> Result<ConfigurationSnapshot, ApplicationError> {
        let _guard = self.gate.lock().map_err(|_| failure())?;
        let mut config = RuntimeConfig::load(&self.path)?;
        config.validate()?;
        let capabilities = capabilities(&config, principal);
        if !offline && !capabilities.contains(&LocalOperatorCapability::ReadConfiguration) {
            return Err(OperatorControlError::AuthorizationDenied.into());
        }
        let version = configuration_version(&config)?;
        let restart_required = !offline && self.restart_required(&config);
        let paths =
            LogicalPaths::resolve(self.path.parent().ok_or_else(failure)?, &config.validate()?)?;
        let ssh_keys = config
            .transports
            .iter()
            .filter_map(|t| {
                if let sf_core::TransportAdapterConfig::Ssh { host_key, .. } = &t.adapter {
                    let key = paths.get(sf_core::LogicalPath::System).join(host_key);
                    Some(match std::fs::symlink_metadata(&key) {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SecretStatus::Missing,
                        Ok(metadata) if metadata.is_file() && metadata.len() <= 65536 => {
                            if russh::keys::load_secret_key(&key, None).is_ok() {
                                SecretStatus::Configured
                            } else {
                                SecretStatus::Invalid
                            }
                        }
                        _ => SecretStatus::Invalid,
                    })
                } else {
                    None
                }
            })
            .collect();
        let database = RuntimeDatabase::open_read_only(&self.database)?;
        let mut domains = Vec::new();
        for conference in database.all_conferences()?.into_iter().take(128) {
            domains.push(ConfigurationDomainSummary {
                kind: "messages".into(),
                number: conference.number,
                name: conference.name,
                active: conference.active,
                read_security: conference.read_security.get(),
                write_security: conference.post_security.get(),
                version: None,
            });
        }
        for area in database.all_file_areas()?.into_iter().take(128) {
            domains.push(ConfigurationDomainSummary {
                kind: "files".into(),
                number: area.number,
                name: area.name,
                active: area.active,
                read_security: area.read_security.get(),
                write_security: area.upload_security.get(),
                version: Some(area.state_version),
            });
        }
        config.configuration_commit = None;
        // Opaque modem commands may include device credentials. Their bytes
        // remain in canonical storage and are never part of this projection.
        for transport in &mut config.transports {
            if let sf_core::TransportAdapterConfig::Modem {
                initialization,
                answer,
                ..
            } = &mut transport.adapter
            {
                *initialization = "[redacted]".into();
                *answer = "[redacted]".into();
            }
        }
        Ok(ConfigurationSnapshot {
            version,
            config,
            restart_required,
            ssh_keys,
            capabilities,
            domains,
        })
    }
    pub(crate) fn apply(
        &self,
        principal: &str,
        generation: &str,
        command_id: &str,
        candidate: &ConfigurationCandidate,
        offline: bool,
    ) -> Result<ConfigurationResult, ApplicationError> {
        let mut published = self.gate.lock().map_err(|_| failure())?;
        self.recover()?;
        let stored = RuntimeConfig::load(&self.path)?;
        let current = configuration_version(&stored)?;
        let grants = capabilities(&stored, principal);
        let sensitive = candidate.operators.is_some()
            || candidate.edits.iter().any(|edit| edit.field.sensitive());
        if !offline
            && (!grants.contains(&LocalOperatorCapability::ChangeOnlineConfiguration)
                || sensitive
                    && !grants.contains(&LocalOperatorCapability::ChangeSensitiveConfiguration))
        {
            self.audit(
                principal,
                command_id,
                "denied",
                "denied",
                "capability-denied",
            )?;
            return Ok(ConfigurationResult::Denied);
        }
        if command_id.len() < 16
            || command_id.len() > 64
            || !command_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(OperatorControlError::InvalidCommand.into());
        }
        let mut hash = Sha256::new();
        hash.update(b"spitfire-ng/configuration-command/v1\0");
        hash.update(generation.as_bytes());
        hash.update(serde_json::to_vec(candidate).map_err(|_| failure())?);
        let fingerprint = format!("{:x}", hash.finalize());
        let mut database = RuntimeDatabase::open(&self.database)?;
        match database.accept_operator_command(&sf_core::NewOperatorCommandReceipt {
            command_id: command_id.into(),
            daemon_generation: generation.into(),
            operator_id: principal.into(),
            command_family: "configuration".into(),
            command_type: "configuration.apply".into(),
            request_fingerprint: fingerprint.clone(),
            target_kind: Some("configuration".into()),
            target_id: None,
            target_generation: Some(candidate.expected.digest.clone()),
            received_at: now(),
        })? {
            sf_core::CommandReceiptResult::Replayed(receipt) => {
                return Ok(ConfigurationResult::Replayed {
                    result_class: receipt.result_class,
                    revision: receipt.result_version,
                })
            }
            sf_core::CommandReceiptResult::Accepted => {}
            _ => return Err(OperatorControlError::Conflict.into()),
        }
        self.audit(
            principal,
            command_id,
            "allowed",
            "succeeded",
            "configuration-attempt",
        )?;
        if current != candidate.expected {
            if !database.reject_operator_command(command_id, "configuration-conflict", now())? {
                return Err(failure());
            }
            self.audit(
                principal,
                command_id,
                "allowed",
                "rejected",
                "configuration-conflict",
            )?;
            return Ok(ConfigurationResult::Conflict { current });
        }
        let mut replacement = match candidate.validate(&stored) {
            Ok(value) => value,
            Err(issues) => {
                if !database.reject_operator_command(command_id, "configuration-invalid", now())? {
                    return Err(failure());
                }
                self.audit(
                    principal,
                    command_id,
                    "allowed",
                    "rejected",
                    "configuration-invalid",
                )?;
                return Ok(ConfigurationResult::Invalid { issues });
            }
        };
        replacement.revision = stored
            .revision
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(failure)?;
        let mut effects = vec![];
        for edit in &candidate.edits {
            let effect = edit.field.effect();
            if edit.field.value(&stored) != edit.field.value(&replacement)
                && !effects.contains(&effect)
            {
                effects.push(effect);
            }
        }
        if candidate.operators.is_some() && stored.operators != replacement.operators {
            effects.push(ConfigurationEffect::Live);
            if cfg!(windows) {
                effects.push(ConfigurationEffect::RestartRequired);
            }
        }
        let version = configuration_version(&replacement)?;
        let restart_required = !offline && self.restart_required(&replacement);
        let result_class = if restart_required {
            "configuration-restart-required"
        } else {
            "configuration-saved"
        };
        replacement.configuration_commit = Some(ConfigurationCommit {
            command_id: command_id.into(),
            principal: principal.into(),
            generation: generation.into(),
            fingerprint,
            digest: version.digest.clone(),
            result_class: result_class.into(),
        });
        // A single bounded prior-generation backup is complete and synced before
        // replacement. No rename-to-backup gap can remove the current config.
        for edit in &candidate.edits {
            if edit.field.value(&stored) != edit.field.value(&replacement) {
                self.audit(
                    principal,
                    command_id,
                    "allowed",
                    "succeeded",
                    edit.field.label_key(),
                )?;
            }
        }
        if candidate.operators.is_some() {
            self.audit(
                principal,
                command_id,
                "allowed",
                "succeeded",
                "operator-profiles-requested",
            )?;
        }
        stored.save_atomic(&self.path.with_extension("toml.previous"))?;
        replacement.save_atomic(&self.path)?;
        *published = replacement;
        // After replacement a failure is explicitly recoverable, not a rejection.
        // Subsequent operations/startup finish the receipt from the commit link.
        if sync_parent(&self.path).is_err() || self.recover().is_err() {
            return Ok(ConfigurationResult::RecoveryRequired);
        }
        Ok(ConfigurationResult::Saved {
            version,
            effects,
            restart_required,
        })
    }
    fn audit(
        &self,
        principal: &str,
        command: &str,
        authorization: &str,
        outcome: &str,
        detail: &str,
    ) -> Result<(), ApplicationError> {
        RuntimeDatabase::open(&self.database)?.record_operator_control_audit(
            &sf_core::NewOperatorControlAudit {
                occurred_at: now(),
                operator_kind: "host-operator".into(),
                operator_id: Some(principal.into()),
                operation: if outcome == "succeeded" {
                    "configuration.prepare"
                } else {
                    "configuration.apply"
                }
                .into(),
                authorization_result: authorization.into(),
                target_kind: Some("configuration".into()),
                target_id: None,
                command_id: Some(command.into()),
                correlation_id: None,
                outcome: outcome.into(),
                detail_code: Some(detail.into()),
            },
        )?;
        Ok(())
    }
}
fn sync_parent(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    std::fs::File::open(path.parent().unwrap_or(Path::new(".")))?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
pub(crate) fn capabilities(
    config: &RuntimeConfig,
    principal: &str,
) -> Vec<LocalOperatorCapability> {
    config
        .operators
        .local_identities
        .iter()
        .find_map(|identity| {
            let (id, grants) = match identity {
                LocalOperatorIdentity::Unix {
                    uid, capabilities, ..
                } => (format!("unix-uid:{uid}"), capabilities),
                LocalOperatorIdentity::Windows {
                    sid, capabilities, ..
                } => (format!("windows-sid:{sid}"), capabilities),
            };
            id.eq_ignore_ascii_case(principal).then(|| grants.clone())
        })
        .unwrap_or_default()
}

/// Explicit cold-board authority. The lifetime lock prevents daemon startup;
/// no connection failure automatically selects this mode.
pub struct OfflineConfiguration {
    _lock: BoardOperationLock,
    authority: ConfigurationAuthority,
    principal: String,
    generation: String,
}
impl OfflineConfiguration {
    pub fn open(path: &Path) -> Result<Self, ApplicationError> {
        let path = path.canonicalize().map_err(|_| failure())?;
        let root = path.parent().ok_or_else(failure)?;
        let lock = BoardOperationLock::acquire(root)?;
        let config = RuntimeConfig::load(&path)?;
        let paths = LogicalPaths::resolve(root, &config.validate()?)?;
        let database = RuntimeDatabase::open_read_only(paths.database())?;
        if database.schema_version()? != 19
            || database.validate_current_snapshot()? != config.validate()?.identity
        {
            return Err(failure());
        }
        let principal = current_operator_identity()?;
        Ok(Self {
            _lock: lock,
            authority: ConfigurationAuthority::new(path, config)?,
            principal,
            generation: crate::operator_control::random_token(),
        })
    }
    pub fn snapshot(&self) -> Result<ConfigurationSnapshot, ApplicationError> {
        self.authority.snapshot(&self.principal, true)
    }
    pub fn apply(
        &self,
        command_id: &str,
        candidate: &ConfigurationCandidate,
    ) -> Result<ConfigurationResult, ApplicationError> {
        self.authority.apply(
            &self.principal,
            &self.generation,
            command_id,
            candidate,
            true,
        )
    }
}

pub fn current_operator_identity() -> Result<String, ApplicationError> {
    #[cfg(unix)]
    {
        Ok(format!(
            "unix-uid:{}",
            std::os::unix::fs::MetadataExt::uid(
                &tempfile::tempfile()
                    .map_err(|_| failure())?
                    .metadata()
                    .map_err(|_| failure())?
            )
        ))
    }
    #[cfg(windows)]
    {
        Ok(format!(
            "windows-sid:{}",
            crate::operator_control::windows_current_process_sid()?
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(OperatorControlError::PlatformUnavailable.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn fixture() -> (tempfile::TempDir, crate::FixtureReport) {
        let temp = tempfile::tempdir().unwrap();
        let report = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        (temp, report)
    }
    fn edit(
        snapshot: &ConfigurationSnapshot,
        field: ConfigurationField,
        value: &str,
    ) -> ConfigurationCandidate {
        ConfigurationCandidate {
            expected: snapshot.version.clone(),
            edits: vec![ConfigurationEdit {
                field,
                value: value.into(),
            }],
            operators: None,
        }
    }
    fn enroll(path: &Path) {
        let mut config = RuntimeConfig::load(path).unwrap();
        let capabilities = match &mut config.operators.local_identities[0] {
            LocalOperatorIdentity::Unix { capabilities, .. }
            | LocalOperatorIdentity::Windows { capabilities, .. } => capabilities,
        };
        capabilities.extend([
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::ChangeOnlineConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
        ]);
        config.save_atomic(path).unwrap();
    }
    #[test]
    fn offline_atomic_version_conflict_backup_and_reopen() {
        let (_temp, board) = fixture();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let initial = offline.snapshot().unwrap();
        assert!(crate::BoardRuntime::load(&board.config_path).is_err());
        let candidate = edit(&initial, ConfigurationField::InactivityMinutes, "7");
        let command = "a".repeat(32);
        assert!(matches!(
            offline.apply(&command, &candidate).unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        assert!(matches!(
            offline.apply(&command, &candidate).unwrap(),
            ConfigurationResult::Replayed {
                revision: Some(1),
                ..
            }
        ));
        let after = offline.snapshot().unwrap();
        assert_eq!(after.version.revision, 1);
        assert_eq!(after.config.caller.inactivity_minutes, 7);
        assert!(matches!(
            offline.apply(&"b".repeat(32), &candidate).unwrap(),
            ConfigurationResult::Conflict { .. }
        ));
        let prior =
            RuntimeConfig::load(&board.config_path.with_extension("toml.previous")).unwrap();
        assert_eq!(prior.revision, initial.version.revision);
        assert_eq!(
            prior.caller.inactivity_minutes,
            initial.config.caller.inactivity_minutes
        );
        drop(offline);
        let runtime = crate::BoardRuntime::load(&board.config_path).unwrap();
        assert_eq!(
            runtime
                .configuration
                .current()
                .unwrap()
                .caller
                .inactivity_minutes,
            7
        );
        assert_eq!(runtime.schema_version(), 19);
    }
    #[test]
    fn online_concurrent_clients_cas_then_refresh_and_retry() {
        let (_temp, board) = fixture();
        enroll(&board.config_path);
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let service = crate::OperatorService::new(runtime.clone());
        let principal = current_operator_identity().unwrap();
        let a = service.configuration_snapshot(&principal).unwrap();
        let b = service.configuration_snapshot(&principal).unwrap();
        assert_eq!(a.version, b.version);
        let first = edit(&a, ConfigurationField::InactivityMinutes, "7");
        assert!(matches!(
            service
                .apply_configuration(&principal, &"c".repeat(32), &first)
                .unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        let second = edit(&b, ConfigurationField::DailyCalls, "12");
        assert!(matches!(
            service
                .apply_configuration(&principal, &"d".repeat(32), &second)
                .unwrap(),
            ConfigurationResult::Conflict { .. }
        ));
        let fresh = service.configuration_snapshot(&principal).unwrap();
        assert_eq!(fresh.config.caller.inactivity_minutes, 7);
        let retry = edit(&fresh, ConfigurationField::DailyCalls, "12");
        assert!(matches!(
            service
                .apply_configuration(&principal, &"e".repeat(32), &retry)
                .unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        assert_eq!(
            service
                .configuration_snapshot(&principal)
                .unwrap()
                .version
                .revision,
            2
        );
    }
    #[test]
    fn bootstrap_stays_read_only_and_offline_enrollment_is_explicit() {
        let (_temp, board) = fixture();
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let principal = current_operator_identity().unwrap();
        let service = crate::OperatorService::new(runtime.clone());
        assert!(service.configuration_snapshot(&principal).is_err());
        let config = RuntimeConfig::load(&board.config_path).unwrap();
        let candidate = ConfigurationCandidate {
            expected: configuration_version(&config).unwrap(),
            edits: vec![],
            operators: Some(config.operators.clone()),
        };
        assert!(matches!(
            service
                .apply_configuration(&principal, &"1".repeat(32), &candidate)
                .unwrap(),
            ConfigurationResult::Denied
        ));
        drop(service);
        drop(runtime);
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let snapshot = offline.snapshot().unwrap();
        let mut operators = snapshot.config.operators.clone();
        let caps = match &mut operators.local_identities[0] {
            LocalOperatorIdentity::Unix { capabilities, .. }
            | LocalOperatorIdentity::Windows { capabilities, .. } => capabilities,
        };
        caps.extend([
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::ChangeOnlineConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
        ]);
        let enrollment = ConfigurationCandidate {
            expected: snapshot.version,
            edits: vec![],
            operators: Some(operators),
        };
        assert!(matches!(
            offline.apply(&"2".repeat(32), &enrollment).unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        drop(offline);
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let service = crate::OperatorService::new(runtime);
        assert!(service.configuration_snapshot(&principal).is_ok());
    }
    #[test]
    fn invalid_save_preserves_exact_configuration_and_no_backup_is_created() {
        let (_temp, board) = fixture();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let before = std::fs::read(&board.config_path).unwrap();
        let candidate = edit(
            &offline.snapshot().unwrap(),
            ConfigurationField::InactivityMinutes,
            "0",
        );
        assert!(matches!(
            offline.apply(&"3".repeat(32), &candidate).unwrap(),
            ConfigurationResult::Invalid { .. }
        ));
        assert_eq!(std::fs::read(&board.config_path).unwrap(), before);
        assert!(!board.config_path.with_extension("toml.previous").exists());
    }
    #[test]
    fn self_revocation_recovers_only_by_deliberate_exclusive_offline_enrollment() {
        let (_temp, board) = fixture();
        enroll(&board.config_path);
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let service = crate::OperatorService::new(runtime.clone());
        let principal = current_operator_identity().unwrap();
        let initial = service.configuration_snapshot(&principal).unwrap();
        let mut operators = initial.config.operators.clone();
        match &mut operators.local_identities[0] {
            LocalOperatorIdentity::Unix { capabilities, .. }
            | LocalOperatorIdentity::Windows { capabilities, .. } => {
                *capabilities = LocalOperatorCapability::READ_ONLY.to_vec();
            }
        }
        assert!(matches!(
            service
                .apply_configuration(
                    &principal,
                    &"61".repeat(16),
                    &ConfigurationCandidate {
                        expected: initial.version,
                        edits: vec![],
                        operators: Some(operators)
                    }
                )
                .unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        assert!(service.configuration_snapshot(&principal).is_err());
        assert!(matches!(
            OfflineConfiguration::open(&board.config_path),
            Err(ApplicationError::BoardInUse(_))
        ));
        drop(service);
        drop(runtime);
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let snapshot = offline.snapshot().unwrap();
        assert_eq!(snapshot.capabilities, LocalOperatorCapability::READ_ONLY);
        assert!(matches!(
            offline
                .apply(
                    &"62".repeat(16),
                    &ConfigurationCandidate {
                        expected: snapshot.version,
                        edits: vec![],
                        operators: Some(initial.config.operators),
                    }
                )
                .unwrap(),
            ConfigurationResult::Saved { .. }
        ));
        drop(offline);
        let service = crate::OperatorService::new(Arc::new(
            crate::BoardRuntime::load(&board.config_path).unwrap(),
        ));
        assert!(service.configuration_snapshot(&principal).is_ok());
    }
    #[test]
    fn invalid_saved_configuration_fails_closed_and_known_good_backup_recovers_new_root() {
        let (temp, board) = fixture();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let candidate = edit(
            &offline.snapshot().unwrap(),
            ConfigurationField::InactivityMinutes,
            "7",
        );
        offline.apply(&"63".repeat(16), &candidate).unwrap();
        drop(offline);
        let known_good = std::fs::read(&board.config_path).unwrap();
        let backup = temp.path().join("backup");
        crate::backup_board(&board.config_path, &backup).unwrap();
        let broken = b"unknown-invalid-configuration = [";
        std::fs::write(&board.config_path, broken).unwrap();
        assert!(OfflineConfiguration::open(&board.config_path).is_err());
        assert!(crate::BoardRuntime::load(&board.config_path).is_err());
        assert_eq!(std::fs::read(&board.config_path).unwrap(), broken);
        let restored = temp.path().join("recovered");
        crate::restore_board(&backup, &restored, false).unwrap();
        let path = restored.join("spitfire.toml");
        assert_eq!(std::fs::read(&path).unwrap(), known_good);
        let offline = OfflineConfiguration::open(&path).unwrap();
        assert_eq!(
            offline.snapshot().unwrap().config.caller.inactivity_minutes,
            7
        );
        assert!(matches!(
            crate::BoardRuntime::load(&path),
            Err(ApplicationError::BoardInUse(_))
        ));
        assert!(crate::interactive_setup(&restored).is_err());
        drop(offline);
        let runtime = crate::BoardRuntime::load(&path).unwrap();
        let database = RuntimeDatabase::open_read_only(runtime.database_path()).unwrap();
        assert_eq!(database.schema_version().unwrap(), 19);
        database.validate_current_snapshot().unwrap();
        assert_eq!(std::fs::read(&board.config_path).unwrap(), broken);
    }
    #[test]
    fn audit_failure_prevents_configuration_replacement_and_false_success() {
        let (_temp, board) = fixture();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let initial = offline.snapshot().unwrap();
        let original = std::fs::read(&board.config_path).unwrap();
        let database = rusqlite::Connection::open(&offline.authority.database).unwrap();
        database.execute_batch("CREATE TRIGGER fail_config_audit BEFORE INSERT ON operator_control_audit BEGIN SELECT RAISE(ABORT, 'injected audit failure'); END;").unwrap();
        assert!(offline
            .apply(
                &"64".repeat(16),
                &edit(&initial, ConfigurationField::InactivityMinutes, "7")
            )
            .is_err());
        assert_eq!(std::fs::read(&board.config_path).unwrap(), original);
        database
            .execute_batch("DROP TRIGGER fail_config_audit;")
            .unwrap();
        drop(offline);
        let _recovered = OfflineConfiguration::open(&board.config_path).unwrap();
        let (state, result): (String, String) = database
            .query_row(
                "SELECT state,result_class FROM operator_command_journal WHERE command_id=?1",
                ["64".repeat(16)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (state.as_str(), result.as_str()),
            ("rejected", "configuration-not-committed")
        );
    }
    #[test]
    fn restart_required_is_persisted_without_changing_active_nodes() {
        let (_temp, board) = fixture();
        enroll(&board.config_path);
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let principal = current_operator_identity().unwrap();
        let service = crate::OperatorService::new(runtime.clone());
        let candidate = edit(
            &service.configuration_snapshot(&principal).unwrap(),
            ConfigurationField::NodeCount,
            "3",
        );
        assert!(matches!(
            service
                .apply_configuration(&principal, &"4".repeat(32), &candidate)
                .unwrap(),
            ConfigurationResult::Saved {
                restart_required: true,
                ..
            }
        ));
        assert_eq!(runtime.node_snapshots().unwrap().len(), 1);
        assert!(
            service
                .configuration_snapshot(&principal)
                .unwrap()
                .restart_required
        );
        drop(service);
        drop(runtime);
        let runtime = crate::BoardRuntime::load(&board.config_path).unwrap();
        assert_eq!(runtime.node_snapshots().unwrap().len(), 3);
        assert!(
            !runtime
                .configuration
                .snapshot(&principal, false)
                .unwrap()
                .restart_required
        );
    }
    #[test]
    fn write_failure_leaves_previous_configuration_and_recovery_does_not_invent_success() {
        let (_temp, board) = fixture();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let initial = offline.snapshot().unwrap();
        std::fs::create_dir(board.config_path.with_extension("toml.previous")).unwrap();
        assert!(offline
            .apply(
                &"5".repeat(32),
                &edit(&initial, ConfigurationField::InactivityMinutes, "7")
            )
            .is_err());
        assert_eq!(offline.snapshot().unwrap().version, initial.version);
        let database = rusqlite::Connection::open(&offline.authority.database).unwrap();
        drop(offline);
        let _reopened = OfflineConfiguration::open(&board.config_path).unwrap();
        let successes: i64 = database.query_row(
            "SELECT COUNT(*) FROM operator_control_audit WHERE operation='configuration.apply' AND outcome='succeeded'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(successes, 0);
        let state: String = database
            .query_row(
                "SELECT state FROM operator_command_journal WHERE command_id=?1",
                ["5".repeat(32)],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "rejected");
    }
}

#[cfg(all(test, any(unix, windows)))]
mod protocol_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    #[test]
    fn configuration_wire_two_clients_and_live_capability_revocation() {
        let temp = tempfile::tempdir().unwrap();
        let board = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        let mut config = RuntimeConfig::load(&board.config_path).unwrap();
        let caps = match &mut config.operators.local_identities[0] {
            LocalOperatorIdentity::Unix { capabilities, .. }
            | LocalOperatorIdentity::Windows { capabilities, .. } => capabilities,
        };
        caps.extend([
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::ChangeOnlineConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
        ]);
        config.save_atomic(&board.config_path).unwrap();
        let runtime = Arc::new(crate::BoardRuntime::load(&board.config_path).unwrap());
        let shutdown = Arc::new(AtomicBool::new(false));
        let server = crate::operator_control::start_operator_server(
            runtime,
            board.config_path.clone(),
            shutdown.clone(),
        )
        .unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut a = crate::OperatorClient::connect(&board.config_path)
                    .await
                    .unwrap();
                let mut b = crate::OperatorClient::connect(&board.config_path)
                    .await
                    .unwrap();
                let sa = a.configuration_snapshot().await.unwrap();
                let sb = b.configuration_snapshot().await.unwrap();
                assert_eq!(sa.version, sb.version);
                let candidate = ConfigurationCandidate {
                    expected: sa.version,
                    edits: vec![ConfigurationEdit {
                        field: ConfigurationField::InactivityMinutes,
                        value: "7".into(),
                    }],
                    operators: None,
                };
                let id = "a7".repeat(16);
                assert!(matches!(
                    a.apply_configuration(id.clone(), candidate.clone())
                        .await
                        .unwrap(),
                    ConfigurationResult::Saved { .. }
                ));
                assert!(matches!(
                    a.apply_configuration(id, candidate.clone()).await.unwrap(),
                    ConfigurationResult::Replayed {
                        revision: Some(1),
                        ..
                    }
                ));
                assert!(matches!(
                    b.apply_configuration("b7".repeat(16), candidate.clone())
                        .await
                        .unwrap(),
                    ConfigurationResult::Conflict { .. }
                ));
                let fresh = b.configuration_snapshot().await.unwrap();
                let mut candidate = candidate;
                candidate.expected = fresh.version;
                candidate.edits[0].value = "8".into();
                assert!(matches!(
                    b.apply_configuration("c7".repeat(16), candidate)
                        .await
                        .unwrap(),
                    ConfigurationResult::Saved { .. }
                ));
                let fresh = a.configuration_snapshot().await.unwrap();
                let mut operators = fresh.config.operators;
                let caps = match &mut operators.local_identities[0] {
                    LocalOperatorIdentity::Unix { capabilities, .. }
                    | LocalOperatorIdentity::Windows { capabilities, .. } => capabilities,
                };
                caps.retain(|cap| *cap != LocalOperatorCapability::ChangeOnlineConfiguration);
                let revoke = ConfigurationCandidate {
                    expected: fresh.version,
                    edits: vec![],
                    operators: Some(operators),
                };
                assert!(matches!(
                    a.apply_configuration("d7".repeat(16), revoke)
                        .await
                        .unwrap(),
                    ConfigurationResult::Saved { .. }
                ));
                let fresh = b.configuration_snapshot().await.unwrap();
                let denied = ConfigurationCandidate {
                    expected: fresh.version,
                    edits: vec![ConfigurationEdit {
                        field: ConfigurationField::InactivityMinutes,
                        value: "9".into(),
                    }],
                    operators: None,
                };
                assert!(matches!(
                    b.apply_configuration("e7".repeat(16), denied)
                        .await
                        .unwrap(),
                    ConfigurationResult::Denied
                ));
                assert_eq!(
                    b.configuration_snapshot()
                        .await
                        .unwrap()
                        .config
                        .caller
                        .inactivity_minutes,
                    8
                );
            });
        shutdown.store(true, Ordering::SeqCst);
        server.join().unwrap();
    }
    #[test]
    fn receipt_failure_after_atomic_replace_recovers_once_without_private_values_in_audit() {
        let temp = tempfile::tempdir().unwrap();
        let board = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        let snapshot = offline.snapshot().unwrap();
        let candidate = ConfigurationCandidate {
            expected: snapshot.version,
            edits: vec![ConfigurationEdit {
                field: ConfigurationField::InactivityMinutes,
                value: "7".into(),
            }],
            operators: None,
        };
        let db = rusqlite::Connection::open(&board.database_path).unwrap();
        db.execute_batch("CREATE TRIGGER fail_configuration_receipt BEFORE UPDATE ON operator_command_journal WHEN NEW.state='completed' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        let command = "f7".repeat(16);
        assert!(matches!(
            offline.apply(&command, &candidate).unwrap(),
            ConfigurationResult::RecoveryRequired
        ));
        assert_eq!(RuntimeConfig::load(&board.config_path).unwrap().revision, 1);
        db.execute_batch("DROP TRIGGER fail_configuration_receipt;")
            .unwrap();
        drop(offline);
        let reopened = OfflineConfiguration::open(&board.config_path).unwrap();
        assert_eq!(reopened.snapshot().unwrap().version.revision, 1);
        let state: String = db
            .query_row(
                "SELECT state FROM operator_command_journal WHERE command_id=?1",
                [&command],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "completed");
        let count:i64=db.query_row("SELECT COUNT(*) FROM operator_control_audit WHERE command_id=?1 AND detail_code='configuration-saved'",[&command],|r|r.get(0)).unwrap();
        assert_eq!(count, 1);
        drop(reopened);
        let _again = OfflineConfiguration::open(&board.config_path).unwrap();
        let count:i64=db.query_row("SELECT COUNT(*) FROM operator_control_audit WHERE command_id=?1 AND detail_code='configuration-saved'",[&command],|r|r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod secret_tests {
    use super::*;
    #[test]
    fn actual_secret_states_and_opaque_modem_commands_never_reveal_values() {
        let temp = tempfile::tempdir().unwrap();
        let board = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        let mut config = RuntimeConfig::load(&board.config_path).unwrap();
        config.transports.push(sf_core::TransportConfig {
            name: Some("ssh".into()),
            enabled: false,
            adapter: sf_core::TransportAdapterConfig::Ssh {
                listen: "127.0.0.1:2222".parse().unwrap(),
                host_key: "ssh/host-ed25519".into(),
                terminal: sf_core::NetworkTerminalDefaults::default(),
                maximum_unauthenticated_connections: 32,
                maximum_authentication_attempts: 3,
                handshake_timeout_seconds: 30,
            },
        });
        config.transports.push(sf_core::TransportConfig {
            name: Some("modem".into()),
            enabled: false,
            adapter: sf_core::TransportAdapterConfig::Modem {
                device: "synthetic-device".into(),
                baud: 9600,
                initialization: "AT+CPIN=private-status-sentinel".into(),
                answer: "ATA".into(),
                terminal: sf_core::NetworkTerminalDefaults::default(),
            },
        });
        config.save_atomic(&board.config_path).unwrap();
        let offline = OfflineConfiguration::open(&board.config_path).unwrap();
        assert_eq!(
            offline.snapshot().unwrap().ssh_keys,
            vec![SecretStatus::Missing]
        );
        let system = board.root.join("system");
        crate::transports::load_or_generate_host_key(&system, Path::new("ssh/host-ed25519"))
            .unwrap();
        let key = std::fs::read_to_string(system.join("ssh/host-ed25519")).unwrap();
        let snapshot = offline.snapshot().unwrap();
        assert_eq!(snapshot.ssh_keys, vec![SecretStatus::Configured]);
        let wire = serde_json::to_string(&snapshot).unwrap();
        assert!(!wire.contains("private-status-sentinel"));
        assert!(!wire.contains(key.trim()));
        assert!(!format!("{snapshot:?}").contains(key.trim()));
        std::fs::write(
            system.join("ssh/host-ed25519"),
            "private-key-invalid-sentinel",
        )
        .unwrap();
        let snapshot = offline.snapshot().unwrap();
        assert_eq!(snapshot.ssh_keys, vec![SecretStatus::Invalid]);
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private-key-invalid-sentinel"));
        // A normal field save retains opaque canonical bytes, even though its
        // read projection redacts them.
        let candidate = ConfigurationCandidate {
            expected: snapshot.version,
            edits: vec![ConfigurationEdit {
                field: ConfigurationField::InactivityMinutes,
                value: "7".into(),
            }],
            operators: None,
        };
        offline.apply(&"91".repeat(16), &candidate).unwrap();
        assert!(RuntimeConfig::load(&board.config_path)
            .unwrap()
            .to_toml()
            .unwrap()
            .contains("private-status-sentinel"));
    }
}
