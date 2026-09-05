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

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    canonicalize_caller_name, BoardIdentity, BoardIdentityError, NodeDefinition, NodeId, PathError,
};

pub const CONFIG_FORMAT_VERSION: u32 = 2;
const LEGACY_CONFIG_FORMAT_VERSION: u32 = 1;

/// Human-readable runtime configuration. Paths remain logical until validated
/// and resolved by `LogicalPaths`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub format_version: u32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_commit: Option<crate::configuration::ConfigurationCommit>,
    pub board: BoardConfig,
    /// Legacy Increment 0 singleton-node configuration. New configurations
    /// serialize `nodes`; loading remains supported so existing boards can be
    /// upgraded without a reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<LegacyNodeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<NodePoolConfig>,
    pub paths: PathConfig,
    pub storage: StorageConfig,
    /// Board presentation selection. Configurations created before M031 omit
    /// this section and remain in explicit legacy-resource mode.
    #[serde(default)]
    pub presentation: PresentationConfig,
    /// Board-default engine language. Presentation profile selection remains
    /// independent and never changes this value.
    #[serde(default)]
    pub language: LanguageConfig,
    #[serde(default)]
    pub caller: CallerConfig,
    #[serde(default)]
    pub transports: Vec<TransportConfig>,
    /// Local operating-system identities allowed to use the protected
    /// operator control endpoint. An empty list uses the board-owner
    /// bootstrap rule so existing boards remain locally manageable.
    #[serde(default)]
    pub operators: OperatorConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorConfig {
    #[serde(default)]
    pub local_identities: Vec<LocalOperatorIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "platform", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LocalOperatorIdentity {
    Unix {
        uid: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default = "default_operator_capabilities")]
        capabilities: Vec<LocalOperatorCapability>,
    },
    Windows {
        sid: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default = "default_operator_capabilities")]
        capabilities: Vec<LocalOperatorCapability>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocalOperatorCapability {
    BoardStatistics,
    NodeStatus,
    OperationalEvents,
    CallerActivity,
    Notifications,
    MaintenanceStatus,
    /// Permit acknowledging an existing operator notification.
    AcknowledgeNotifications,
    /// Permit bounded adjustments to a live caller session allowance.
    AdjustSessionTime,
    ManagePageAvailability,
    ManageCallerPages,
    ChatWithCaller,
    DisconnectSession,
    RequestGracefulShutdown,
    ReadConfiguration,
    ChangeOnlineConfiguration,
    ChangeSensitiveConfiguration,
}

/// Matches the existing bounded operator discovery capability-list capacity.
/// This is a storage/validation ceiling, never an implicit grant.
pub const MAX_LOCAL_OPERATOR_CAPABILITIES: usize = 32;

impl LocalOperatorCapability {
    /// Complete implemented vocabulary for explicit enrollment, never a preset.
    pub const ALL: [Self; 16] = [
        Self::BoardStatistics,
        Self::NodeStatus,
        Self::OperationalEvents,
        Self::CallerActivity,
        Self::Notifications,
        Self::MaintenanceStatus,
        Self::AcknowledgeNotifications,
        Self::AdjustSessionTime,
        Self::ManagePageAvailability,
        Self::ManageCallerPages,
        Self::ChatWithCaller,
        Self::DisconnectSession,
        Self::RequestGracefulShutdown,
        Self::ReadConfiguration,
        Self::ChangeOnlineConfiguration,
        Self::ChangeSensitiveConfiguration,
    ];
    /// Explicitly enumerate the B021-A bootstrap boundary. New controls must
    /// never enter this list merely because they are added to the enum.
    pub const READ_ONLY: [Self; 6] = [
        Self::BoardStatistics,
        Self::NodeStatus,
        Self::OperationalEvents,
        Self::CallerActivity,
        Self::Notifications,
        Self::MaintenanceStatus,
    ];
}

fn default_operator_capabilities() -> Vec<LocalOperatorCapability> {
    LocalOperatorCapability::READ_ONLY.to_vec()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    #[serde(default = "default_locale")]
    pub default_locale: String,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            default_locale: default_locale(),
        }
    }
}

/// Static board selection for presentation resources. Terminal capability and
/// caller ANSI/text preference continue to select a representation; they do
/// not select a profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationConfig {
    #[serde(default)]
    pub mode: PresentationMode,
    /// Selects whether exact-security BBS/CLR menu artwork is considered.
    /// Generated menus always remain engine-owned and derive from `.MNU`.
    #[serde(default)]
    pub menu_mode: MenuPresentationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_profile: Option<String>,
}

impl PresentationConfig {
    pub fn modern_default() -> Self {
        Self {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some("modern-ng".to_owned()),
            base_profile: Some("modern-ng".to_owned()),
        }
    }
}

impl Default for PresentationConfig {
    fn default() -> Self {
        Self {
            mode: PresentationMode::LegacyResources,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: None,
            base_profile: None,
        }
    }
}

/// Board-owned choice between optional exact-security artwork and SPITFIRE's
/// engine-generated menus. Profiles cannot select this value.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MenuPresentationMode {
    #[default]
    DisplayOverrides,
    Generated,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresentationMode {
    /// Compatibility mode for pre-M031 boards: SYSTEM help and DISPLAY assets
    /// retain their former direct lookup behavior.
    #[default]
    LegacyResources,
    /// Resolve board overrides over configured active/base packages.
    Profile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoardConfig {
    pub name: String,
    pub sysop: String,
    #[serde(default = "default_board_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub access: BoardAccessMode,
    #[serde(default)]
    pub private_security_level: u16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoardAccessMode {
    #[default]
    Public,
    Private,
}

impl BoardAccessMode {
    pub const fn is_private(self) -> bool {
        matches!(self, Self::Private)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyNodeConfig {
    pub number: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodePoolConfig {
    pub count: u32,
    #[serde(default)]
    pub overrides: Vec<NodeOverrideConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeOverrideConfig {
    pub number: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathConfig {
    pub system: PathBuf,
    pub work: PathBuf,
    pub display: PathBuf,
    pub message: PathBuf,
    pub external: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub database_file: PathBuf,
}

/// Modern storage and stock-compatible access policy for SPITFIRE callers.
/// Historical caller records remain a compatibility input, not the native
/// credential representation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallerConfig {
    #[serde(default = "default_sysop_caller_name")]
    pub sysop_caller_name: String,
    #[serde(default = "default_new_caller_security")]
    pub new_caller_security: u16,
    #[serde(default = "default_sysop_security")]
    pub sysop_security: u16,
    #[serde(default = "default_minutes_per_call")]
    pub minutes_per_call: u32,
    #[serde(default = "default_minutes_per_day")]
    pub minutes_per_day: u32,
    #[serde(default = "default_new_caller_minutes")]
    pub new_caller_first_day_minutes: u32,
    #[serde(default = "default_daily_calls")]
    pub maximum_daily_calls: u32,
    #[serde(default = "default_inactivity_minutes")]
    pub inactivity_minutes: u32,
    #[serde(default = "default_login_attempts")]
    pub maximum_login_attempts: u8,
    #[serde(default = "default_minimum_password_length")]
    pub minimum_password_length: usize,
    #[serde(default = "default_maximum_password_length")]
    pub maximum_password_length: usize,
    #[serde(default)]
    pub password: PasswordHashConfig,
    #[serde(default)]
    pub security_limits: Vec<SecurityLimitConfig>,
    #[serde(default)]
    pub profile: CallerProfilePolicy,
    #[serde(default)]
    pub subscription: SubscriptionConfig,
    /// Optional engine-owned sequence run after successful authentication and
    /// before Main. Presentation profiles cannot select or redefine it.
    #[serde(default)]
    pub post_login_journey: PostLoginJourney,
}

impl Default for CallerConfig {
    fn default() -> Self {
        Self {
            sysop_caller_name: default_sysop_caller_name(),
            new_caller_security: default_new_caller_security(),
            sysop_security: default_sysop_security(),
            minutes_per_call: default_minutes_per_call(),
            minutes_per_day: default_minutes_per_day(),
            new_caller_first_day_minutes: default_new_caller_minutes(),
            maximum_daily_calls: default_daily_calls(),
            inactivity_minutes: default_inactivity_minutes(),
            maximum_login_attempts: default_login_attempts(),
            minimum_password_length: default_minimum_password_length(),
            maximum_password_length: default_maximum_password_length(),
            password: PasswordHashConfig::default(),
            security_limits: Vec::new(),
            profile: CallerProfilePolicy::default(),
            subscription: SubscriptionConfig::default(),
            post_login_journey: PostLoginJourney::None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub warning_days: u16,
    #[serde(default)]
    pub expired_security: u16,
}

/// Fixed post-authentication behavior owned by the common session engine.
///
/// This is deliberately board policy rather than presentation-profile data:
/// a profile cannot execute commands or define a state machine.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostLoginJourney {
    #[default]
    None,
    Stock,
}

/// Privacy-conscious collection policy for traditional caller information.
/// The same values drive setup/configuration, registration, and profile edits.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallerProfilePolicy {
    #[serde(default)]
    pub address: ProfileFieldPolicy,
    #[serde(default)]
    pub phone: ProfileFieldPolicy,
    #[serde(default)]
    pub email: ProfileFieldPolicy,
    #[serde(default)]
    pub birthday: ProfileFieldPolicy,
}

impl CallerProfilePolicy {
    pub const fn all_disabled(&self) -> bool {
        !self.address.enabled()
            && !self.phone.enabled()
            && !self.email.enabled()
            && !self.birthday.enabled()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileFieldPolicy {
    #[default]
    Disabled,
    Optional,
    Required,
}

impl ProfileFieldPolicy {
    pub const fn enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub const fn required(self) -> bool {
        matches!(self, Self::Required)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordHashConfig {
    #[serde(default = "default_argon2_memory")]
    pub memory_kib: u32,
    #[serde(default = "default_argon2_iterations")]
    pub iterations: u32,
    #[serde(default = "default_argon2_parallelism")]
    pub parallelism: u32,
}

impl Default for PasswordHashConfig {
    fn default() -> Self {
        Self {
            memory_kib: default_argon2_memory(),
            iterations: default_argon2_iterations(),
            parallelism: default_argon2_parallelism(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityLimitConfig {
    pub security_level: u16,
    pub minutes_per_call: u32,
    pub minutes_per_day: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkTerminalDefaults {
    #[serde(default = "default_true")]
    pub ansi: bool,
    #[serde(default = "default_true")]
    pub cp437: bool,
    #[serde(default = "default_width")]
    pub width: u16,
    #[serde(default = "default_height")]
    pub height: u16,
}

impl Default for NetworkTerminalDefaults {
    fn default() -> Self {
        Self {
            ansi: true,
            cp437: true,
            width: default_width(),
            height: default_height(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TransportConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub adapter: TransportAdapterConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TransportAdapterConfig {
    Telnet {
        listen: SocketAddr,
        #[serde(default)]
        terminal: NetworkTerminalDefaults,
    },
    Raw {
        listen: SocketAddr,
        #[serde(default)]
        terminal: NetworkTerminalDefaults,
    },
    Rlogin {
        listen: SocketAddr,
        /// Enables the plaintext SyncTERM/Synchronet credential convention.
        /// It is intentionally off unless the Sysop accepts that risk.
        #[serde(default)]
        auto_login: bool,
        #[serde(default)]
        terminal: NetworkTerminalDefaults,
    },
    Ssh {
        listen: SocketAddr,
        /// Relative to the configured SYSTEM directory.
        #[serde(default = "default_ssh_host_key")]
        host_key: PathBuf,
        #[serde(default = "ssh_terminal_defaults")]
        terminal: NetworkTerminalDefaults,
        #[serde(default = "default_ssh_unauthenticated_connections")]
        maximum_unauthenticated_connections: u16,
        #[serde(default = "default_ssh_auth_attempts")]
        maximum_authentication_attempts: u8,
        #[serde(default = "default_ssh_handshake_timeout_seconds")]
        handshake_timeout_seconds: u64,
    },
    Serial {
        device: String,
        baud: u32,
        #[serde(default)]
        terminal: NetworkTerminalDefaults,
    },
    Modem {
        device: String,
        baud: u32,
        initialization: String,
        #[serde(default = "default_answer_command")]
        answer: String,
        #[serde(default)]
        terminal: NetworkTerminalDefaults,
    },
}

impl TransportConfig {
    pub const fn network_listener(&self) -> Option<SocketAddr> {
        match &self.adapter {
            TransportAdapterConfig::Telnet { listen, .. }
            | TransportAdapterConfig::Raw { listen, .. }
            | TransportAdapterConfig::Rlogin { listen, .. }
            | TransportAdapterConfig::Ssh { listen, .. } => Some(*listen),
            TransportAdapterConfig::Serial { .. } | TransportAdapterConfig::Modem { .. } => None,
        }
    }

    pub fn serial_device(&self) -> Option<&str> {
        match &self.adapter {
            TransportAdapterConfig::Serial { device, .. }
            | TransportAdapterConfig::Modem { device, .. } => Some(device),
            _ => None,
        }
    }

    pub fn effective_name(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| {
            format!(
                "{}-{}",
                match &self.adapter {
                    TransportAdapterConfig::Telnet { .. } => "telnet",
                    TransportAdapterConfig::Raw { .. } => "raw",
                    TransportAdapterConfig::Rlogin { .. } => "rlogin",
                    TransportAdapterConfig::Ssh { .. } => "ssh",
                    TransportAdapterConfig::Serial { .. } => "serial",
                    TransportAdapterConfig::Modem { .. } => "modem",
                },
                index + 1
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedConfig {
    pub identity: BoardIdentity,
    pub timezone: Tz,
    pub board_access: BoardAccessMode,
    pub private_security_level: u16,
    pub nodes: Vec<NodeDefinition>,
    pub paths: PathConfig,
    pub database_file: PathBuf,
    pub presentation: PresentationConfig,
    pub language: LanguageConfig,
    pub caller: CallerConfig,
    pub transports: Vec<TransportConfig>,
    pub operators: OperatorConfig,
}

impl RuntimeConfig {
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        toml::from_str(input).map_err(ConfigError::Parse)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let input = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml(&input)
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(ConfigError::Serialize)
    }

    /// Validates and atomically replaces a static board configuration. The
    /// caller remains responsible for coordinating changes with a live board.
    pub fn save_atomic(&self, path: &Path) -> Result<(), ConfigError> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| ConfigError::InvalidSavePath(path.to_path_buf()))?;
        if !parent.is_dir() {
            return Err(ConfigError::InvalidSavePath(path.to_path_buf()));
        }
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| ConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .write_all(self.to_toml()?.as_bytes())
            .and_then(|()| temporary.as_file_mut().sync_all())
            .map_err(|source| ConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .persist(path)
            .map_err(|error| ConfigError::Write {
                path: path.to_path_buf(),
                source: error.error,
            })?;
        Ok(())
    }

    pub fn validate(&self) -> Result<ValidatedConfig, ConfigError> {
        if !matches!(
            self.format_version,
            LEGACY_CONFIG_FORMAT_VERSION | CONFIG_FORMAT_VERSION
        ) {
            return Err(ConfigError::UnsupportedFormatVersion {
                found: self.format_version,
                supported: CONFIG_FORMAT_VERSION,
            });
        }
        if self.format_version == LEGACY_CONFIG_FORMAT_VERSION && self.node.is_none() {
            return Err(ConfigError::LegacyVersionRequiresSingletonNode);
        }

        let identity = BoardIdentity::new(&self.board.name, &self.board.sysop)?;
        let timezone = self
            .board
            .timezone
            .parse::<Tz>()
            .map_err(|_| ConfigError::InvalidBoardTimezone(self.board.timezone.clone()))?;
        if self.board.private_security_level > 9_999 {
            return Err(ConfigError::InvalidPrivateSecurityLevel);
        }
        let nodes = validate_nodes(self.node.as_ref(), self.nodes.as_ref())?;
        validate_path_config(&self.paths)?;
        validate_database_file(&self.storage.database_file)?;
        validate_presentation(&self.presentation)?;
        crate::normalize_locale(&self.language.default_locale)
            .map_err(|_| ConfigError::InvalidDefaultLocale(self.language.default_locale.clone()))?;
        validate_transports(&self.transports)?;
        validate_caller(&self.caller)?;
        validate_operators(&self.operators)?;

        Ok(ValidatedConfig {
            identity,
            timezone,
            board_access: self.board.access,
            private_security_level: self.board.private_security_level,
            nodes,
            paths: self.paths.clone(),
            database_file: self.storage.database_file.clone(),
            presentation: self.presentation.clone(),
            language: self.language.clone(),
            caller: self.caller.clone(),
            transports: self.transports.clone(),
            operators: self.operators.clone(),
        })
    }

    pub fn synthetic_fixture() -> Self {
        Self {
            format_version: CONFIG_FORMAT_VERSION,
            revision: 0,
            configuration_commit: None,
            board: BoardConfig {
                name: "SPITFIRE NG Fixture Board".to_owned(),
                sysop: "Fixture Sysop".to_owned(),
                timezone: default_board_timezone(),
                access: BoardAccessMode::Public,
                private_security_level: 0,
            },
            node: None,
            nodes: Some(NodePoolConfig {
                count: 1,
                overrides: vec![NodeOverrideConfig {
                    number: 1,
                    enabled: true,
                    description: Some("Fixture Node 1".to_owned()),
                }],
            }),
            paths: PathConfig {
                system: PathBuf::from("system"),
                work: PathBuf::from("work"),
                display: PathBuf::from("display"),
                message: PathBuf::from("message"),
                external: PathBuf::from("external"),
            },
            storage: StorageConfig {
                database_file: PathBuf::from("spitfire-ng.sqlite3"),
            },
            presentation: PresentationConfig::modern_default(),
            language: LanguageConfig::default(),
            caller: CallerConfig {
                security_limits: vec![SecurityLimitConfig {
                    security_level: 10,
                    minutes_per_call: 45,
                    minutes_per_day: 60,
                }],
                ..CallerConfig::default()
            },
            transports: vec![
                TransportConfig {
                    name: Some("telnet".to_owned()),
                    enabled: true,
                    adapter: TransportAdapterConfig::Telnet {
                        listen: SocketAddr::from(([127, 0, 0, 1], 2323)),
                        terminal: NetworkTerminalDefaults::default(),
                    },
                },
                TransportConfig {
                    name: Some("raw".to_owned()),
                    enabled: true,
                    adapter: TransportAdapterConfig::Raw {
                        listen: SocketAddr::from(([127, 0, 0, 1], 2324)),
                        terminal: NetworkTerminalDefaults::default(),
                    },
                },
                TransportConfig {
                    name: Some("rlogin".to_owned()),
                    enabled: true,
                    adapter: TransportAdapterConfig::Rlogin {
                        listen: SocketAddr::from(([127, 0, 0, 1], 2513)),
                        auto_login: false,
                        terminal: NetworkTerminalDefaults::default(),
                    },
                },
            ],
            operators: OperatorConfig::default(),
        }
    }
}

fn validate_operators(operators: &OperatorConfig) -> Result<(), ConfigError> {
    if operators.local_identities.len() > 32 {
        return Err(ConfigError::InvalidOperatorConfiguration);
    }
    let mut identities = HashSet::new();
    for identity in &operators.local_identities {
        let (key, label, capabilities) = match identity {
            LocalOperatorIdentity::Unix {
                uid,
                label,
                capabilities,
            } => (format!("unix:{uid}"), label, capabilities),
            LocalOperatorIdentity::Windows {
                sid,
                label,
                capabilities,
            } => {
                if sid.len() < 5
                    || sid.len() > 184
                    || !sid.starts_with("S-")
                    || sid[2..]
                        .bytes()
                        .any(|byte| !byte.is_ascii_digit() && byte != b'-')
                {
                    return Err(ConfigError::InvalidOperatorConfiguration);
                }
                (format!("windows:{sid}"), label, capabilities)
            }
        };
        if !identities.insert(key)
            || capabilities.is_empty()
            || capabilities.len() > MAX_LOCAL_OPERATOR_CAPABILITIES
            || capabilities.iter().copied().collect::<HashSet<_>>().len() != capabilities.len()
            || label.as_ref().is_some_and(|value| {
                value.is_empty() || value.len() > 64 || value.chars().any(char::is_control)
            })
        {
            return Err(ConfigError::InvalidOperatorConfiguration);
        }
    }
    Ok(())
}

fn default_sysop_caller_name() -> String {
    "Sysop".to_owned()
}

fn default_board_timezone() -> String {
    "UTC".to_owned()
}

fn default_locale() -> String {
    crate::EMBEDDED_LOCALE.to_owned()
}

const fn default_new_caller_security() -> u16 {
    10
}

const fn default_sysop_security() -> u16 {
    50
}

const fn default_minutes_per_call() -> u32 {
    60
}

const fn default_minutes_per_day() -> u32 {
    60
}

const fn default_new_caller_minutes() -> u32 {
    45
}

const fn default_daily_calls() -> u32 {
    10
}

const fn default_inactivity_minutes() -> u32 {
    3
}

const fn default_login_attempts() -> u8 {
    3
}

const fn default_minimum_password_length() -> usize {
    10
}

const fn default_maximum_password_length() -> usize {
    128
}

const fn default_argon2_memory() -> u32 {
    19_456
}

const fn default_argon2_iterations() -> u32 {
    2
}

const fn default_argon2_parallelism() -> u32 {
    1
}

const fn default_true() -> bool {
    true
}

fn default_ssh_host_key() -> PathBuf {
    PathBuf::from("ssh/host-ed25519")
}

fn ssh_terminal_defaults() -> NetworkTerminalDefaults {
    NetworkTerminalDefaults {
        ansi: true,
        cp437: false,
        width: default_width(),
        height: default_height(),
    }
}

const fn default_ssh_unauthenticated_connections() -> u16 {
    32
}

const fn default_ssh_auth_attempts() -> u8 {
    3
}

const fn default_ssh_handshake_timeout_seconds() -> u64 {
    30
}

const fn default_width() -> u16 {
    80
}

const fn default_height() -> u16 {
    25
}

fn default_answer_command() -> String {
    "ATA".to_owned()
}

fn validate_nodes(
    legacy: Option<&LegacyNodeConfig>,
    pool: Option<&NodePoolConfig>,
) -> Result<Vec<NodeDefinition>, ConfigError> {
    if legacy.is_some() && pool.is_some() {
        return Err(ConfigError::ConflictingNodeConfiguration);
    }
    if let Some(legacy) = legacy {
        return Ok(vec![NodeDefinition {
            id: NodeId::new(legacy.number)?,
            enabled: true,
            description: Some(format!("Legacy Node {}", legacy.number)),
        }]);
    }
    let pool = pool.ok_or(ConfigError::MissingNodeConfiguration)?;
    if pool.count == 0 || pool.count > 4_096 {
        return Err(ConfigError::InvalidNodeCount(pool.count));
    }
    let mut overrides = std::collections::BTreeMap::new();
    for value in &pool.overrides {
        if value.number == 0 || value.number > pool.count {
            return Err(ConfigError::InvalidNodeOverride(value.number));
        }
        if value
            .description
            .as_ref()
            .is_some_and(|description| description.trim().is_empty() || description.len() > 80)
        {
            return Err(ConfigError::InvalidNodeDescription(value.number));
        }
        if overrides.insert(value.number, value).is_some() {
            return Err(ConfigError::DuplicateNodeOverride(value.number));
        }
    }
    let nodes = (1..=pool.count)
        .map(|number| {
            let value = overrides.get(&number);
            Ok(NodeDefinition {
                id: NodeId::new(number)?,
                enabled: value.is_none_or(|override_| override_.enabled),
                description: value.and_then(|override_| override_.description.clone()),
            })
        })
        .collect::<Result<Vec<_>, ConfigError>>()?;
    if !nodes.iter().any(|node| node.enabled) {
        return Err(ConfigError::NoEnabledNodes);
    }
    Ok(nodes)
}

fn validate_transports(transports: &[TransportConfig]) -> Result<(), ConfigError> {
    let mut listeners = HashSet::new();
    let mut devices = HashSet::new();
    let mut names = HashSet::new();
    for (index, transport) in transports.iter().enumerate() {
        let name = transport.effective_name(index);
        if name.trim().is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ConfigError::InvalidTransportName(name));
        }
        if !names.insert(name.clone()) {
            return Err(ConfigError::DuplicateTransportName(name));
        }
        if let Some(address) = transport.network_listener() {
            if address.port() == 0 {
                return Err(ConfigError::InvalidListenerPort(address));
            }
            if transport.enabled && !listeners.insert(address) {
                return Err(ConfigError::DuplicateListener(address));
            }
        }
        if let Some(device) = transport.serial_device() {
            if device.trim().is_empty() {
                return Err(ConfigError::InvalidSerialDevice);
            }
            if transport.enabled && !devices.insert(device) {
                return Err(ConfigError::DuplicateSerialDevice(device.to_owned()));
            }
        }
        match &transport.adapter {
            TransportAdapterConfig::Telnet { terminal, .. }
            | TransportAdapterConfig::Raw { terminal, .. }
            | TransportAdapterConfig::Rlogin { terminal, .. }
            | TransportAdapterConfig::Serial { terminal, .. }
            | TransportAdapterConfig::Modem { terminal, .. } => {
                validate_terminal_defaults(terminal)?
            }
            TransportAdapterConfig::Ssh {
                host_key,
                terminal,
                maximum_unauthenticated_connections,
                maximum_authentication_attempts,
                handshake_timeout_seconds,
                ..
            } => {
                validate_terminal_defaults(terminal)?;
                validate_ssh_host_key(host_key)?;
                if !(1..=1_024).contains(maximum_unauthenticated_connections)
                    || !(1..=10).contains(maximum_authentication_attempts)
                    || !(5..=300).contains(handshake_timeout_seconds)
                {
                    return Err(ConfigError::InvalidSshResourceLimits);
                }
            }
        }
        match &transport.adapter {
            TransportAdapterConfig::Serial { baud, .. }
            | TransportAdapterConfig::Modem { baud, .. }
                if !(300..=4_000_000).contains(baud) =>
            {
                return Err(ConfigError::InvalidSerialBaud(*baud));
            }
            TransportAdapterConfig::Modem {
                initialization,
                answer,
                ..
            } if !is_safe_modem_command(initialization) || !is_safe_modem_command(answer) => {
                return Err(ConfigError::InvalidModemCommand)
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_caller(caller: &CallerConfig) -> Result<(), ConfigError> {
    if canonicalize_caller_name(caller.sysop_caller_name.as_bytes()).is_err() {
        return Err(ConfigError::InvalidSysopCallerName);
    }
    if caller.new_caller_security > 9_999 || caller.sysop_security > 9_999 {
        return Err(ConfigError::InvalidSecurityLevel);
    }
    if caller.subscription.warning_days > 365 || caller.subscription.expired_security > 9_999 {
        return Err(ConfigError::InvalidSubscriptionPolicy);
    }
    if caller.minutes_per_call == 0
        || caller.minutes_per_day == 0
        || caller.new_caller_first_day_minutes == 0
        || caller.minutes_per_call > 1_440
        || caller.minutes_per_day > 1_440
        || caller.new_caller_first_day_minutes > 1_440
    {
        return Err(ConfigError::InvalidTimeLimit);
    }
    if caller.maximum_daily_calls == 0 || caller.maximum_daily_calls > 10_000 {
        return Err(ConfigError::InvalidDailyCallLimit);
    }
    if caller.inactivity_minutes == 0 || caller.inactivity_minutes > 1_440 {
        return Err(ConfigError::InvalidInactivityLimit);
    }
    if caller.maximum_login_attempts == 0 || caller.maximum_login_attempts > 10 {
        return Err(ConfigError::InvalidLoginAttempts);
    }
    if caller.minimum_password_length < 8
        || caller.minimum_password_length > caller.maximum_password_length
        || caller.maximum_password_length > 1_024
    {
        return Err(ConfigError::InvalidPasswordLength);
    }
    if argon2::Params::new(
        caller.password.memory_kib,
        caller.password.iterations,
        caller.password.parallelism,
        None,
    )
    .is_err()
    {
        return Err(ConfigError::InvalidPasswordHashParameters);
    }
    let mut levels = HashSet::new();
    for limit in &caller.security_limits {
        if limit.security_level > 9_999
            || limit.minutes_per_call == 0
            || limit.minutes_per_day == 0
            || limit.minutes_per_call > 1_440
            || limit.minutes_per_day > 1_440
        {
            return Err(ConfigError::InvalidSecurityLimit(limit.security_level));
        }
        if !levels.insert(limit.security_level) {
            return Err(ConfigError::DuplicateSecurityLimit(limit.security_level));
        }
    }
    Ok(())
}

fn validate_terminal_defaults(defaults: &NetworkTerminalDefaults) -> Result<(), ConfigError> {
    if defaults.width == 0 || defaults.height == 0 {
        return Err(ConfigError::InvalidTerminalDimensions {
            width: defaults.width,
            height: defaults.height,
        });
    }
    Ok(())
}

fn is_safe_modem_command(command: &str) -> bool {
    !command.is_empty()
        && command.len() <= 128
        && command
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
}

fn validate_path_config(paths: &PathConfig) -> Result<(), ConfigError> {
    for (name, path) in [
        ("SYSTEM", &paths.system),
        ("WORK", &paths.work),
        ("DISPLAY", &paths.display),
        ("MESSAGE", &paths.message),
        ("EXTERNAL", &paths.external),
    ] {
        crate::paths::validate_configured_path(name, path)?;
    }
    Ok(())
}

fn validate_presentation(presentation: &PresentationConfig) -> Result<(), ConfigError> {
    match presentation.mode {
        PresentationMode::LegacyResources => {
            if presentation.active_profile.is_some() || presentation.base_profile.is_some() {
                return Err(ConfigError::LegacyPresentationHasProfiles);
            }
        }
        PresentationMode::Profile => {
            let active = presentation
                .active_profile
                .as_deref()
                .ok_or(ConfigError::MissingPresentationProfile)?;
            let base = presentation
                .base_profile
                .as_deref()
                .ok_or(ConfigError::MissingPresentationProfile)?;
            for profile in [active, base] {
                if !valid_profile_id(profile) {
                    return Err(ConfigError::InvalidPresentationProfileId(
                        profile.to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn valid_profile_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn validate_database_file(path: &Path) -> Result<(), ConfigError> {
    let mut components = path.components();
    let first = components.next();
    if first.is_none() || components.next().is_some() || path.as_os_str().is_empty() {
        return Err(ConfigError::InvalidDatabaseFile(path.to_path_buf()));
    }
    if !matches!(first, Some(std::path::Component::Normal(_))) {
        return Err(ConfigError::InvalidDatabaseFile(path.to_path_buf()));
    }
    Ok(())
}

fn validate_ssh_host_key(path: &Path) -> Result<(), ConfigError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(ConfigError::InvalidSshHostKey(path.to_path_buf()));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("local operator configuration is invalid or exceeds its bounds")]
    InvalidOperatorConfiguration,
    #[error("could not read configuration {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write configuration {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("configuration output path must have an existing parent: {0}")]
    InvalidSavePath(PathBuf),
    #[error("configuration is not valid TOML: {0}")]
    Parse(#[source] toml::de::Error),
    #[error("could not serialize configuration: {0}")]
    Serialize(#[source] toml::ser::Error),
    #[error("configuration format version {found} is unsupported; expected {supported}")]
    UnsupportedFormatVersion { found: u32, supported: u32 },
    #[error(transparent)]
    InvalidBoardIdentity(#[from] BoardIdentityError),
    #[error(transparent)]
    InvalidNode(#[from] crate::NodeError),
    #[error("configuration must contain exactly one of [node] or [nodes]")]
    MissingNodeConfiguration,
    #[error("configuration cannot contain both legacy [node] and modern [nodes]")]
    ConflictingNodeConfiguration,
    #[error("configuration format 1 requires the legacy [node] section; use format 2 for [nodes]")]
    LegacyVersionRequiresSingletonNode,
    #[error("nodes.count must be in 1..=4096, got {0}")]
    InvalidNodeCount(u32),
    #[error("node override {0} is outside the configured node range")]
    InvalidNodeOverride(u32),
    #[error("node override {0} is duplicated")]
    DuplicateNodeOverride(u32),
    #[error("node {0} description must contain 1..=80 bytes")]
    InvalidNodeDescription(u32),
    #[error("at least one configured node must be enabled")]
    NoEnabledNodes,
    #[error(transparent)]
    InvalidPath(#[from] PathError),
    #[error("storage.database_file must be one relative file name, got {0}")]
    InvalidDatabaseFile(PathBuf),
    #[error("legacy-resource presentation mode cannot name active/base profiles")]
    LegacyPresentationHasProfiles,
    #[error("profile presentation mode requires active_profile and base_profile")]
    MissingPresentationProfile,
    #[error("presentation profile ID must be 1..=64 lowercase ASCII letters/digits separated by hyphens: {0:?}")]
    InvalidPresentationProfileId(String),
    #[error("listener port must be nonzero: {0}")]
    InvalidListenerPort(SocketAddr),
    #[error("duplicate listener address: {0}")]
    DuplicateListener(SocketAddr),
    #[error("transport name must contain 1..=64 ASCII letters, digits, '-' or '_': {0:?}")]
    InvalidTransportName(String),
    #[error("transport name is duplicated: {0}")]
    DuplicateTransportName(String),
    #[error("serial device must not be empty")]
    InvalidSerialDevice,
    #[error("serial device is configured more than once: {0}")]
    DuplicateSerialDevice(String),
    #[error("serial baud must be between 300 and 4000000, got {0}")]
    InvalidSerialBaud(u32),
    #[error("terminal dimensions must be nonzero, got {width}x{height}")]
    InvalidTerminalDimensions { width: u16, height: u16 },
    #[error("modem initialization and answer commands must be 1..128 printable ASCII bytes")]
    InvalidModemCommand,
    #[error("caller.sysop_caller_name must normalize to 1..30 printable ASCII bytes")]
    InvalidSysopCallerName,
    #[error("SPITFIRE security levels must be in 0..=9999")]
    InvalidSecurityLevel,
    #[error("board.private_security_level must be in 0..=9999")]
    InvalidPrivateSecurityLevel,
    #[error("board.timezone is not a recognized IANA timezone: {0:?}")]
    InvalidBoardTimezone(String),
    #[error("language.default_locale is not a valid normalized BCP 47 locale: {0:?}")]
    InvalidDefaultLocale(String),
    #[error("caller time limits must be in 1..=1440 minutes")]
    InvalidTimeLimit,
    #[error("caller.maximum_daily_calls must be in 1..=10000")]
    InvalidDailyCallLimit,
    #[error("caller.inactivity_minutes must be in 1..=1440")]
    InvalidInactivityLimit,
    #[error("caller.maximum_login_attempts must be in 1..=10")]
    InvalidLoginAttempts,
    #[error(
        "caller.subscription warning_days must be in 0..=365 and expired_security in 0..=9999"
    )]
    InvalidSubscriptionPolicy,
    #[error("password lengths must satisfy 8 <= minimum <= maximum <= 1024")]
    InvalidPasswordLength,
    #[error("Argon2id memory, iteration, and parallelism parameters must be nonzero")]
    InvalidPasswordHashParameters,
    #[error("invalid time policy for security level {0}")]
    InvalidSecurityLimit(u16),
    #[error("security level {0} has more than one time policy")]
    DuplicateSecurityLimit(u16),
    #[error("SSH host key must be a safe relative path beneath SYSTEM: {0}")]
    InvalidSshHostKey(PathBuf),
    #[error("SSH limits require 1..=1024 unauthenticated connections, 1..=10 attempts, and a 5..=300 second handshake timeout")]
    InvalidSshResourceLimits,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_toml() -> &'static str {
        r#"
format_version = 1

[board]
name = "Test Board"
sysop = "Test Sysop"

[node]
number = 1

[paths]
system = "system"
work = "work"
display = "display"
message = "message"
external = "external"

[storage]
database_file = "spitfire-ng.sqlite3"
"#
    }

    #[test]
    fn complete_operator_vocabulary_fits_bound_round_trips_and_never_expands_bootstrap() {
        let all = LocalOperatorCapability::ALL;
        let mut identifiers = std::collections::BTreeSet::new();
        assert!(all.len() <= MAX_LOCAL_OPERATOR_CAPABILITIES);
        for cap in all {
            let serialized = serde_json::to_string(&cap).unwrap();
            assert!(identifiers.insert(serialized.clone()));
            assert_eq!(
                serde_json::from_str::<LocalOperatorCapability>(&serialized).unwrap(),
                cap
            );
        }
        assert_eq!(LocalOperatorCapability::READ_ONLY.len(), 6);
        assert!(LocalOperatorCapability::READ_ONLY
            .iter()
            .all(|cap| all.contains(cap)));
        let mut config = RuntimeConfig::synthetic_fixture();
        config
            .operators
            .local_identities
            .push(LocalOperatorIdentity::Unix {
                uid: 7,
                label: None,
                capabilities: all.to_vec(),
            });
        config.validate().unwrap();
        assert_eq!(
            default_operator_capabilities(),
            LocalOperatorCapability::READ_ONLY
        );
    }
    #[test]
    fn parses_and_validates_configuration() {
        let config = RuntimeConfig::from_toml(valid_toml()).unwrap();
        let validated = config.validate().unwrap();
        assert_eq!(validated.identity.name(), "Test Board");
        assert_eq!(validated.nodes[0].id.get(), 1);
        assert_eq!(validated.paths.message, PathBuf::from("message"));
    }

    #[test]
    fn rejects_unknown_and_invalid_configuration() {
        let unknown = format!("{}\nunexpected = true\n", valid_toml());
        assert!(matches!(
            RuntimeConfig::from_toml(&unknown),
            Err(ConfigError::Parse(_))
        ));

        let mut config = RuntimeConfig::from_toml(valid_toml()).unwrap();
        config.node.as_mut().unwrap().number = 0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidNode(_))
        ));

        config.node.as_mut().unwrap().number = 1;
        config.paths.work = PathBuf::from("../escape");
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidPath(_))
        ));
    }

    #[test]
    fn serializes_a_round_trip_fixture_configuration() {
        let original = RuntimeConfig::synthetic_fixture();
        let encoded = original.to_toml().unwrap();
        assert_eq!(RuntimeConfig::from_toml(&encoded).unwrap(), original);
    }

    #[test]
    fn operator_defaults_are_read_only_and_full_explicit_profiles_round_trip() {
        assert_eq!(
            default_operator_capabilities(),
            LocalOperatorCapability::READ_ONLY
        );
        assert_eq!(LocalOperatorCapability::READ_ONLY.len(), 6);
        assert!(!LocalOperatorCapability::READ_ONLY
            .contains(&LocalOperatorCapability::AcknowledgeNotifications));
        assert!(!LocalOperatorCapability::READ_ONLY
            .contains(&LocalOperatorCapability::AdjustSessionTime));
        let legacy = RuntimeConfig::from_toml(valid_toml()).unwrap();
        assert!(legacy.operators.local_identities.is_empty());
        let implicit: LocalOperatorIdentity =
            toml::from_str("platform = 'unix'\nuid = 7\n").unwrap();
        let LocalOperatorIdentity::Unix { capabilities, .. } = implicit else {
            unreachable!()
        };
        assert_eq!(capabilities, LocalOperatorCapability::READ_ONLY);
        let mut granted = LocalOperatorCapability::READ_ONLY.to_vec();
        granted.extend([
            LocalOperatorCapability::AcknowledgeNotifications,
            LocalOperatorCapability::AdjustSessionTime,
            LocalOperatorCapability::ManagePageAvailability,
            LocalOperatorCapability::ManageCallerPages,
            LocalOperatorCapability::ChatWithCaller,
            LocalOperatorCapability::DisconnectSession,
            LocalOperatorCapability::RequestGracefulShutdown,
        ]);
        assert_eq!(granted.len(), 13);
        assert!(granted[6..]
            .iter()
            .all(|capability| !LocalOperatorCapability::READ_ONLY.contains(capability)));
        assert!(granted.len() <= MAX_LOCAL_OPERATOR_CAPABILITIES);
        for identity in [
            LocalOperatorIdentity::Unix {
                uid: 7,
                label: None,
                capabilities: granted.clone(),
            },
            LocalOperatorIdentity::Windows {
                sid: "S-1-5-21-7".to_owned(),
                label: None,
                capabilities: granted.clone(),
            },
        ] {
            let mut config = RuntimeConfig::synthetic_fixture();
            config.operators.local_identities = vec![identity];
            config.validate().unwrap();
            let encoded = config.to_toml().unwrap();
            assert_eq!(
                RuntimeConfig::from_toml(&encoded).unwrap().operators,
                config.operators
            );
        }
    }

    #[test]
    fn operator_profiles_reject_duplicate_oversized_unknown_and_malformed_grants() {
        assert_eq!(MAX_LOCAL_OPERATOR_CAPABILITIES, 32);
        for capabilities in [
            vec![],
            vec![LocalOperatorCapability::NodeStatus; 2],
            vec![LocalOperatorCapability::NodeStatus; MAX_LOCAL_OPERATOR_CAPABILITIES + 1],
        ] {
            let config = OperatorConfig {
                local_identities: vec![LocalOperatorIdentity::Unix {
                    uid: 7,
                    label: None,
                    capabilities,
                }],
            };
            assert!(matches!(
                validate_operators(&config),
                Err(ConfigError::InvalidOperatorConfiguration)
            ));
        }
        for unknown in ["*", "administrator", "host-shutdown", "node_status"] {
            let input = format!(
                "platform = 'unix'\nuid = 7\ncapabilities = ['node-status', '{unknown}']\n"
            );
            assert!(toml::from_str::<LocalOperatorIdentity>(&input).is_err());
        }
        for input in [
            "platform = 'unix'\nuid = 7\ncapabilities = '*'\n",
            "platform = 'unix'\nuid = 7\ncapabilities = [7]\n",
            "platform = 'unix'\nuid = 7\nadmin = true\n",
        ] {
            assert!(toml::from_str::<LocalOperatorIdentity>(input).is_err());
        }
    }

    #[test]
    fn validates_profile_selection_and_preserves_omitted_legacy_mode() {
        let legacy = RuntimeConfig::from_toml(valid_toml()).unwrap();
        assert_eq!(legacy.presentation.mode, PresentationMode::LegacyResources);
        assert!(legacy.validate().is_ok());

        let mut config = RuntimeConfig::synthetic_fixture();
        assert_eq!(config.presentation, PresentationConfig::modern_default());
        config.presentation.active_profile = Some("../escape".to_owned());
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidPresentationProfileId(_))
        ));
        config.presentation = PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: None,
            base_profile: Some("modern-ng".to_owned()),
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigError::MissingPresentationProfile)
        ));
        config.presentation = PresentationConfig {
            mode: PresentationMode::LegacyResources,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some("modern-ng".to_owned()),
            base_profile: None,
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigError::LegacyPresentationHasProfiles)
        ));
    }

    #[test]
    fn parses_transport_list_and_rejects_unknown_options() {
        let input = format!(
            "{}\n[[transports]]\ntype = \"raw\"\nlisten = \"127.0.0.1:2324\"\n\n[transports.terminal]\nansi = false\ncp437 = true\nwidth = 80\nheight = 25\n",
            valid_toml()
        );
        let config = RuntimeConfig::from_toml(&input).unwrap();
        assert!(config.validate().is_ok());
        assert!(matches!(
            config.transports[0].adapter,
            TransportAdapterConfig::Raw { .. }
        ));

        let unknown = input.replace("height = 25", "height = 25\ntrust_me = true");
        assert!(matches!(
            RuntimeConfig::from_toml(&unknown),
            Err(ConfigError::Parse(_))
        ));

        let unknown_listener = input.replace(
            "listen = \"127.0.0.1:2324\"",
            "listen = \"127.0.0.1:2324\"\ntrust_remote_identity = true",
        );
        assert!(matches!(
            RuntimeConfig::from_toml(&unknown_listener),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn rejects_listener_device_and_serial_conflicts() {
        let mut config = RuntimeConfig::synthetic_fixture();
        let mut duplicate_listener = config.transports[0].clone();
        duplicate_listener.name = Some("duplicate-address".to_owned());
        config.transports.push(duplicate_listener);
        assert!(matches!(
            config.validate(),
            Err(ConfigError::DuplicateListener(_))
        ));

        config.transports = vec![TransportConfig {
            name: Some("serial".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Serial {
                device: " ".to_owned(),
                baud: 38_400,
                terminal: NetworkTerminalDefaults::default(),
            },
        }];
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidSerialDevice)
        ));

        config.transports = vec![TransportConfig {
            name: Some("serial".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Serial {
                device: "loop".to_owned(),
                baud: 0,
                terminal: NetworkTerminalDefaults::default(),
            },
        }];
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidSerialBaud(0))
        ));
    }

    #[test]
    fn validates_bounded_ssh_configuration() {
        let mut config = RuntimeConfig::synthetic_fixture();
        config.transports = vec![TransportConfig {
            name: Some("ssh".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Ssh {
                listen: SocketAddr::from(([127, 0, 0, 1], 2222)),
                host_key: PathBuf::from("ssh/host-ed25519"),
                terminal: ssh_terminal_defaults(),
                maximum_unauthenticated_connections: 32,
                maximum_authentication_attempts: 3,
                handshake_timeout_seconds: 30,
            },
        }];
        config.validate().unwrap();
        if let TransportAdapterConfig::Ssh {
            ref mut host_key, ..
        } = config.transports[0].adapter
        {
            *host_key = PathBuf::from("../outside");
        }
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidSshHostKey(_))
        ));
    }

    #[test]
    fn validates_caller_policy_and_sync_term_rlogin_opt_in() {
        let mut config = RuntimeConfig::synthetic_fixture();
        assert!(matches!(
            config.transports[2].adapter,
            TransportAdapterConfig::Rlogin {
                auto_login: false,
                ..
            }
        ));
        config.caller.minimum_password_length = 4;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidPasswordLength)
        ));
        config.caller.minimum_password_length = 10;
        config.caller.security_limits.push(SecurityLimitConfig {
            security_level: 10,
            minutes_per_call: 30,
            minutes_per_day: 30,
        });
        assert!(matches!(
            config.validate(),
            Err(ConfigError::DuplicateSecurityLimit(10))
        ));
    }

    #[test]
    fn rejects_unrepresentable_sysop_names_and_invalid_argon2_parameters() {
        let mut config = RuntimeConfig::synthetic_fixture();
        config.caller.sysop_caller_name = "Sýsop".to_owned();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidSysopCallerName)
        ));

        config.caller.sysop_caller_name = "Sysop".to_owned();
        config.caller.password.memory_kib = 8;
        config.caller.password.parallelism = 2;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidPasswordHashParameters)
        ));
    }

    #[test]
    fn validates_timezone_private_board_idle_and_profile_policy_round_trip() {
        let mut config = RuntimeConfig::synthetic_fixture();
        config.board.timezone = "America/Phoenix".to_owned();
        config.board.access = BoardAccessMode::Private;
        config.board.private_security_level = 25;
        config.caller.inactivity_minutes = 7;
        config.caller.profile = CallerProfilePolicy {
            address: ProfileFieldPolicy::Optional,
            phone: ProfileFieldPolicy::Disabled,
            email: ProfileFieldPolicy::Required,
            birthday: ProfileFieldPolicy::Optional,
        };
        let encoded = config.to_toml().unwrap();
        let decoded = RuntimeConfig::from_toml(&encoded).unwrap();
        let validated = decoded.validate().unwrap();
        assert_eq!(validated.timezone, chrono_tz::America::Phoenix);
        assert_eq!(validated.board_access, BoardAccessMode::Private);
        assert_eq!(validated.private_security_level, 25);
        assert_eq!(validated.caller.profile, config.caller.profile);

        let mut invalid = config.clone();
        invalid.board.timezone = "Moon/Tranquility".to_owned();
        assert!(matches!(
            invalid.validate(),
            Err(ConfigError::InvalidBoardTimezone(_))
        ));
        invalid = config;
        invalid.caller.inactivity_minutes = 0;
        assert!(matches!(
            invalid.validate(),
            Err(ConfigError::InvalidInactivityLimit)
        ));
    }
}
