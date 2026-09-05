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

//! Closed configuration fields shared by clients and online/offline authority.
use crate::config::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationEffect {
    Live,
    NewSessions,
    RestartRequired,
    OfflineOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationVersion {
    pub revision: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationField {
    Timezone,
    BoardAccess,
    PrivateSecurity,
    NodeCount,
    MinutesPerCall,
    MinutesPerDay,
    FirstDayMinutes,
    DailyCalls,
    InactivityMinutes,
    NewCallerSecurity,
    SysopSecurity,
    LoginAttempts,
    MinimumPasswordLength,
    MaximumPasswordLength,
    PostLoginJourney,
    ProfileAddress,
    ProfilePhone,
    ProfileEmail,
    ProfileBirthday,
    SubscriptionEnabled,
    SubscriptionWarningDays,
    SubscriptionExpiredSecurity,
    PresentationMode,
    ActiveProfile,
    BaseProfile,
    MenuMode,
    DefaultLocale,
    ListenerEnabled { index: usize },
    ListenerAddress { index: usize },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationEdit {
    pub field: ConfigurationField,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationCandidate {
    pub expected: ConfigurationVersion,
    pub edits: Vec<ConfigurationEdit>,
    /// Whole-list replacement is explicit, versioned, and never merged.
    pub operators: Option<OperatorConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationIssue {
    pub field: Option<ConfigurationField>,
    pub message_key: String,
}

impl ConfigurationField {
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::Timezone => "sfconfig-field-timezone",
            Self::BoardAccess => "sfconfig-field-board-access",
            Self::PrivateSecurity => "sfconfig-field-private-security",
            Self::NodeCount => "sfconfig-field-node-count",
            Self::MinutesPerCall => "sfconfig-field-minutes-per-call",
            Self::MinutesPerDay => "sfconfig-field-minutes-per-day",
            Self::FirstDayMinutes => "sfconfig-field-first-day-minutes",
            Self::DailyCalls => "sfconfig-field-daily-calls",
            Self::InactivityMinutes => "sfconfig-field-inactivity",
            Self::NewCallerSecurity => "sfconfig-field-new-security",
            Self::SysopSecurity => "sfconfig-field-sysop-security",
            Self::LoginAttempts => "sfconfig-field-login-attempts",
            Self::MinimumPasswordLength => "sfconfig-field-min-password",
            Self::MaximumPasswordLength => "sfconfig-field-max-password",
            Self::PostLoginJourney => "sfconfig-field-journey",
            Self::ProfileAddress => "sfconfig-field-address",
            Self::ProfilePhone => "sfconfig-field-phone",
            Self::ProfileEmail => "sfconfig-field-email",
            Self::ProfileBirthday => "sfconfig-field-birthday",
            Self::SubscriptionEnabled => "sfconfig-field-subscription",
            Self::SubscriptionWarningDays => "sfconfig-field-warning-days",
            Self::SubscriptionExpiredSecurity => "sfconfig-field-expired-security",
            Self::PresentationMode => "sfconfig-field-presentation-mode",
            Self::ActiveProfile => "sfconfig-field-active-profile",
            Self::BaseProfile => "sfconfig-field-base-profile",
            Self::MenuMode => "sfconfig-field-menu-mode",
            Self::DefaultLocale => "sfconfig-field-locale",
            Self::ListenerEnabled { .. } => "sfconfig-field-listener-enabled",
            Self::ListenerAddress { .. } => "sfconfig-field-listener-address",
        }
    }
    pub fn section(&self) -> &'static str {
        match self {
            Self::Timezone | Self::BoardAccess | Self::PrivateSecurity => "general",
            Self::NodeCount | Self::ListenerEnabled { .. } | Self::ListenerAddress { .. } => {
                "nodes"
            }
            Self::NewCallerSecurity
            | Self::SysopSecurity
            | Self::LoginAttempts
            | Self::MinimumPasswordLength
            | Self::MaximumPasswordLength => "security",
            Self::PresentationMode
            | Self::ActiveProfile
            | Self::BaseProfile
            | Self::MenuMode
            | Self::DefaultLocale => "presentation",
            _ => "callers",
        }
    }
    pub fn effect(&self) -> ConfigurationEffect {
        match self {
            Self::Timezone
            | Self::NodeCount
            | Self::ListenerEnabled { .. }
            | Self::ListenerAddress { .. }
            | Self::PresentationMode
            | Self::ActiveProfile
            | Self::BaseProfile
            | Self::MenuMode
            | Self::DefaultLocale => ConfigurationEffect::RestartRequired,
            _ => ConfigurationEffect::NewSessions,
        }
    }
    pub fn sensitive(&self) -> bool {
        matches!(self.section(), "security")
            || matches!(
                self,
                Self::BoardAccess
                    | Self::PrivateSecurity
                    | Self::ListenerEnabled { .. }
                    | Self::ListenerAddress { .. }
            )
    }
    pub fn fields(config: &RuntimeConfig) -> Vec<Self> {
        use ConfigurationField::*;
        let mut fields = vec![
            Timezone,
            BoardAccess,
            PrivateSecurity,
            NodeCount,
            MinutesPerCall,
            MinutesPerDay,
            FirstDayMinutes,
            DailyCalls,
            InactivityMinutes,
            NewCallerSecurity,
            SysopSecurity,
            LoginAttempts,
            MinimumPasswordLength,
            MaximumPasswordLength,
            PostLoginJourney,
            ProfileAddress,
            ProfilePhone,
            ProfileEmail,
            ProfileBirthday,
            SubscriptionEnabled,
            SubscriptionWarningDays,
            SubscriptionExpiredSecurity,
            PresentationMode,
            ActiveProfile,
            BaseProfile,
            MenuMode,
            DefaultLocale,
        ];
        for (index, transport) in config.transports.iter().enumerate() {
            fields.push(ListenerEnabled { index });
            if matches!(
                transport.adapter,
                TransportAdapterConfig::Telnet { .. }
                    | TransportAdapterConfig::Raw { .. }
                    | TransportAdapterConfig::Rlogin { .. }
                    | TransportAdapterConfig::Ssh { .. }
            ) {
                fields.push(ListenerAddress { index });
            }
        }
        fields
    }
    pub fn value(&self, config: &RuntimeConfig) -> String {
        let c = &config.caller;
        match self {
            Self::Timezone => config.board.timezone.clone(),
            Self::BoardAccess => if config.board.access.is_private() {
                "private"
            } else {
                "public"
            }
            .into(),
            Self::PrivateSecurity => config.board.private_security_level.to_string(),
            Self::NodeCount => config
                .nodes
                .as_ref()
                .map(|n| n.count)
                .unwrap_or(1)
                .to_string(),
            Self::MinutesPerCall => c.minutes_per_call.to_string(),
            Self::MinutesPerDay => c.minutes_per_day.to_string(),
            Self::FirstDayMinutes => c.new_caller_first_day_minutes.to_string(),
            Self::DailyCalls => c.maximum_daily_calls.to_string(),
            Self::InactivityMinutes => c.inactivity_minutes.to_string(),
            Self::NewCallerSecurity => c.new_caller_security.to_string(),
            Self::SysopSecurity => c.sysop_security.to_string(),
            Self::LoginAttempts => c.maximum_login_attempts.to_string(),
            Self::MinimumPasswordLength => c.minimum_password_length.to_string(),
            Self::MaximumPasswordLength => c.maximum_password_length.to_string(),
            Self::PostLoginJourney => if c.post_login_journey == PostLoginJourney::Stock {
                "stock"
            } else {
                "none"
            }
            .into(),
            Self::ProfileAddress => profile_value(c.profile.address),
            Self::ProfilePhone => profile_value(c.profile.phone),
            Self::ProfileEmail => profile_value(c.profile.email),
            Self::ProfileBirthday => profile_value(c.profile.birthday),
            Self::SubscriptionEnabled => c.subscription.enabled.to_string(),
            Self::SubscriptionWarningDays => c.subscription.warning_days.to_string(),
            Self::SubscriptionExpiredSecurity => c.subscription.expired_security.to_string(),
            Self::PresentationMode => if config.presentation.mode == PresentationMode::Profile {
                "profile"
            } else {
                "legacy-resources"
            }
            .into(),
            Self::ActiveProfile => config
                .presentation
                .active_profile
                .clone()
                .unwrap_or_default(),
            Self::BaseProfile => config.presentation.base_profile.clone().unwrap_or_default(),
            Self::MenuMode => if config.presentation.menu_mode == MenuPresentationMode::Generated {
                "generated"
            } else {
                "display-overrides"
            }
            .into(),
            Self::DefaultLocale => config.language.default_locale.clone(),
            Self::ListenerEnabled { index } => config
                .transports
                .get(*index)
                .map(|t| t.enabled.to_string())
                .unwrap_or_default(),
            Self::ListenerAddress { index } => config
                .transports
                .get(*index)
                .and_then(|t| match t.adapter {
                    TransportAdapterConfig::Telnet { listen, .. }
                    | TransportAdapterConfig::Raw { listen, .. }
                    | TransportAdapterConfig::Rlogin { listen, .. }
                    | TransportAdapterConfig::Ssh { listen, .. } => Some(listen.to_string()),
                    _ => None,
                })
                .unwrap_or_default(),
        }
    }
    pub fn apply(&self, config: &mut RuntimeConfig, value: &str) -> Result<(), ConfigurationIssue> {
        let invalid = || ConfigurationIssue {
            field: Some(self.clone()),
            message_key: "sfconfig-invalid-value".into(),
        };
        if value.len() > 256 || value.chars().any(char::is_control) {
            return Err(invalid());
        }
        let c = &mut config.caller;
        macro_rules! number {
            ($target:expr) => {
                $target = value.parse().map_err(|_| invalid())?
            };
        }
        match self {
            Self::Timezone => config.board.timezone = value.into(),
            Self::BoardAccess => {
                config.board.access = match value {
                    "public" => BoardAccessMode::Public,
                    "private" => BoardAccessMode::Private,
                    _ => return Err(invalid()),
                }
            }
            Self::PrivateSecurity => number!(config.board.private_security_level),
            Self::NodeCount => {
                let count = value.parse().map_err(|_| invalid())?;
                config.format_version = CONFIG_FORMAT_VERSION;
                config.node = None;
                let nodes = config.nodes.get_or_insert(NodePoolConfig {
                    count,
                    overrides: vec![],
                });
                nodes.count = count;
                nodes.overrides.retain(|n| n.number <= count);
            }
            Self::MinutesPerCall => number!(c.minutes_per_call),
            Self::MinutesPerDay => number!(c.minutes_per_day),
            Self::FirstDayMinutes => number!(c.new_caller_first_day_minutes),
            Self::DailyCalls => number!(c.maximum_daily_calls),
            Self::InactivityMinutes => number!(c.inactivity_minutes),
            Self::NewCallerSecurity => number!(c.new_caller_security),
            Self::SysopSecurity => number!(c.sysop_security),
            Self::LoginAttempts => number!(c.maximum_login_attempts),
            Self::MinimumPasswordLength => number!(c.minimum_password_length),
            Self::MaximumPasswordLength => number!(c.maximum_password_length),
            Self::PostLoginJourney => {
                c.post_login_journey = match value {
                    "stock" => PostLoginJourney::Stock,
                    "none" => PostLoginJourney::None,
                    _ => return Err(invalid()),
                }
            }
            Self::ProfileAddress
            | Self::ProfilePhone
            | Self::ProfileEmail
            | Self::ProfileBirthday => {
                let policy = match value {
                    "disabled" => ProfileFieldPolicy::Disabled,
                    "optional" => ProfileFieldPolicy::Optional,
                    "required" => ProfileFieldPolicy::Required,
                    _ => return Err(invalid()),
                };
                match self {
                    Self::ProfileAddress => c.profile.address = policy,
                    Self::ProfilePhone => c.profile.phone = policy,
                    Self::ProfileEmail => c.profile.email = policy,
                    _ => c.profile.birthday = policy,
                }
            }
            Self::SubscriptionEnabled => number!(c.subscription.enabled),
            Self::SubscriptionWarningDays => number!(c.subscription.warning_days),
            Self::SubscriptionExpiredSecurity => number!(c.subscription.expired_security),
            Self::PresentationMode => {
                config.presentation.mode = match value {
                    "profile" => PresentationMode::Profile,
                    "legacy-resources" => PresentationMode::LegacyResources,
                    _ => return Err(invalid()),
                }
            }
            Self::ActiveProfile => {
                config.presentation.active_profile = (!value.is_empty()).then(|| value.into())
            }
            Self::BaseProfile => {
                config.presentation.base_profile = (!value.is_empty()).then(|| value.into())
            }
            Self::MenuMode => {
                config.presentation.menu_mode = match value {
                    "generated" => MenuPresentationMode::Generated,
                    "display-overrides" => MenuPresentationMode::DisplayOverrides,
                    _ => return Err(invalid()),
                }
            }
            Self::DefaultLocale => config.language.default_locale = value.into(),
            Self::ListenerEnabled { index } => number!(
                config
                    .transports
                    .get_mut(*index)
                    .ok_or_else(invalid)?
                    .enabled
            ),
            Self::ListenerAddress { index } => match &mut config
                .transports
                .get_mut(*index)
                .ok_or_else(invalid)?
                .adapter
            {
                TransportAdapterConfig::Telnet { listen, .. }
                | TransportAdapterConfig::Raw { listen, .. }
                | TransportAdapterConfig::Rlogin { listen, .. }
                | TransportAdapterConfig::Ssh { listen, .. } => number!(*listen),
                _ => return Err(invalid()),
            },
        }
        Ok(())
    }
}
fn profile_value(policy: ProfileFieldPolicy) -> String {
    match policy {
        ProfileFieldPolicy::Disabled => "disabled",
        ProfileFieldPolicy::Optional => "optional",
        ProfileFieldPolicy::Required => "required",
    }
    .into()
}

impl ConfigurationCandidate {
    pub fn validate(
        &self,
        original: &RuntimeConfig,
    ) -> Result<RuntimeConfig, Vec<ConfigurationIssue>> {
        if self.edits.len() > 128 {
            return Err(vec![ConfigurationIssue {
                field: None,
                message_key: "sfconfig-too-many-edits".into(),
            }]);
        }
        let mut candidate = original.clone();
        let mut issues = vec![];
        let mut seen = vec![];
        for edit in &self.edits {
            if seen.contains(&edit.field) {
                issues.push(ConfigurationIssue {
                    field: Some(edit.field.clone()),
                    message_key: "sfconfig-duplicate-field".into(),
                });
                continue;
            }
            seen.push(edit.field.clone());
            if let Err(issue) = edit.field.apply(&mut candidate, &edit.value) {
                issues.push(issue);
            }
        }
        if let Some(operators) = &self.operators {
            candidate.operators = operators.clone();
        }
        if let Err(error) = candidate.validate() {
            use ConfigurationField::*;
            let (field, key) = match error {
                ConfigError::InvalidInactivityLimit => {
                    (Some(InactivityMinutes), "sfconfig-validation-minutes")
                }
                ConfigError::InvalidDailyCallLimit => {
                    (Some(DailyCalls), "sfconfig-validation-calls")
                }
                ConfigError::InvalidLoginAttempts => {
                    (Some(LoginAttempts), "sfconfig-validation-attempts")
                }
                ConfigError::InvalidNodeCount(_) => (Some(NodeCount), "sfconfig-validation-nodes"),
                ConfigError::InvalidBoardTimezone(_) => {
                    (Some(Timezone), "sfconfig-validation-timezone")
                }
                ConfigError::InvalidPrivateSecurityLevel => {
                    (Some(PrivateSecurity), "sfconfig-validation-security")
                }
                ConfigError::InvalidPasswordLength => (None, "sfconfig-validation-password"),
                ConfigError::InvalidTimeLimit => (None, "sfconfig-validation-minutes"),
                ConfigError::InvalidSecurityLevel => (None, "sfconfig-validation-security"),
                ConfigError::InvalidOperatorConfiguration => {
                    (None, "sfconfig-validation-operators")
                }
                ConfigError::InvalidSubscriptionPolicy => {
                    (None, "sfconfig-validation-subscription")
                }
                ConfigError::DuplicateListener(_) | ConfigError::InvalidListenerPort(_) => {
                    (None, "sfconfig-validation-listeners")
                }
                ConfigError::MissingPresentationProfile
                | ConfigError::LegacyPresentationHasProfiles
                | ConfigError::InvalidPresentationProfileId(_) => {
                    (None, "sfconfig-validation-profiles")
                }
                _ => (None, "sfconfig-invalid-section"),
            };
            issues.push(ConfigurationIssue {
                field,
                message_key: key.into(),
            });
        }
        if issues.is_empty() {
            Ok(candidate)
        } else {
            Err(issues)
        }
    }
}

/// Recovery link to the existing schema-19 receipt. No candidate or secret bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationCommit {
    pub command_id: String,
    pub principal: String,
    pub generation: String,
    pub fingerprint: String,
    pub digest: String,
    pub result_class: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(config: &RuntimeConfig, edits: Vec<ConfigurationEdit>) -> ConfigurationCandidate {
        ConfigurationCandidate {
            expected: ConfigurationVersion {
                revision: config.revision,
                digest: "0".repeat(64),
            },
            edits,
            operators: None,
        }
    }
    #[test]
    fn every_exposed_field_round_trips_through_shared_validation() {
        let config = RuntimeConfig::synthetic_fixture();
        for field in ConfigurationField::fields(&config) {
            let edit = ConfigurationEdit {
                value: field.value(&config),
                field,
            };
            let result = candidate(&config, vec![edit]).validate(&config).unwrap();
            assert_eq!(result, config);
        }
    }
    #[test]
    fn malformed_duplicate_and_cross_field_candidates_are_rejected() {
        let config = RuntimeConfig::synthetic_fixture();
        let edit = ConfigurationEdit {
            field: ConfigurationField::InactivityMinutes,
            value: "0".into(),
        };
        assert!(candidate(&config, vec![edit.clone()])
            .validate(&config)
            .is_err());
        assert!(candidate(&config, vec![edit.clone(), edit])
            .validate(&config)
            .is_err());
        let edits = vec![
            ConfigurationEdit {
                field: ConfigurationField::MinimumPasswordLength,
                value: "100".into(),
            },
            ConfigurationEdit {
                field: ConfigurationField::MaximumPasswordLength,
                value: "12".into(),
            },
        ];
        assert!(candidate(&config, edits).validate(&config).is_err());
        assert!(serde_json::from_str::<ConfigurationField>("\"arbitrary-file\"").is_err());
    }
    #[test]
    fn explicit_effects_and_sensitive_classes_have_no_implicit_grants() {
        assert_eq!(
            ConfigurationField::ListenerEnabled { index: 0 }.effect(),
            ConfigurationEffect::RestartRequired
        );
        assert_eq!(
            ConfigurationField::InactivityMinutes.effect(),
            ConfigurationEffect::NewSessions
        );
        assert!(ConfigurationField::BoardAccess.sensitive());
        for cap in [
            LocalOperatorCapability::ReadConfiguration,
            LocalOperatorCapability::ChangeOnlineConfiguration,
            LocalOperatorCapability::ChangeSensitiveConfiguration,
        ] {
            assert!(!LocalOperatorCapability::READ_ONLY.contains(&cap));
        }
    }
}
