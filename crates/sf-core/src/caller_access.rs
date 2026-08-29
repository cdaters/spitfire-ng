use std::fs;
use std::path::Path;

use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

use crate::{
    canonicalize_caller_name, Caller, CallerConfig, CallerId, CallerState, DatabaseError,
    RuntimeDatabase, SecurityLevel,
};

pub const MAX_JOKER_BYTES: usize = 64 * 1024;
pub const MAX_JOKER_LINE_BYTES: usize = 120;
pub const MAX_JOKER_RULES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallerAccessActor {
    LocalOperator,
    ThresholdSysop(CallerId),
    SystemPolicy,
}

impl CallerAccessActor {
    fn database_values(self) -> (&'static str, Option<i64>) {
        match self {
            Self::LocalOperator => ("local-operator", None),
            Self::ThresholdSysop(id) => ("threshold-sysop", Some(id.get())),
            Self::SystemPolicy => ("system-policy", None),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionEvaluation {
    pub warning: bool,
    pub adjustment_applied: bool,
    pub effective_security: SecurityLevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JokerRuleKind {
    CompleteName,
    Substring,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JokerRule {
    kind: JokerRuleKind,
    normalized: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JokerPolicy {
    generation: u64,
    source_bytes: Vec<u8>,
    rules: Vec<JokerRule>,
}

impl JokerPolicy {
    pub fn empty() -> Self {
        Self {
            generation: 1,
            source_bytes: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn load(path: &Path, generation: u64, named_sysop: &str) -> Result<Self, JokerError> {
        match fs::read(path) {
            Ok(bytes) => Self::parse(bytes, generation, named_sysop),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                generation,
                ..Self::empty()
            }),
            Err(source) => Err(JokerError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    pub fn parse(bytes: Vec<u8>, generation: u64, named_sysop: &str) -> Result<Self, JokerError> {
        if generation == 0 {
            return Err(JokerError::InvalidGeneration);
        }
        if bytes.len() > MAX_JOKER_BYTES {
            return Err(JokerError::TooLarge(bytes.len()));
        }
        let (_, normalized_sysop) = canonicalize_caller_name(named_sysop.as_bytes())
            .map_err(|_| JokerError::InvalidNamedSysop)?;
        let mut rules = Vec::new();
        for raw in bytes.split(|byte| *byte == b'\n') {
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            if raw.is_empty() {
                continue;
            }
            if raw.len() > MAX_JOKER_LINE_BYTES {
                return Err(JokerError::LineTooLong(raw.len()));
            }
            if raw
                .iter()
                .any(|byte| byte.is_ascii_control() || !byte.is_ascii())
            {
                return Err(JokerError::InvalidByte);
            }
            let (kind, value) = if let Some(value) = raw.strip_prefix(b"@") {
                (JokerRuleKind::Substring, value)
            } else {
                (JokerRuleKind::CompleteName, raw)
            };
            if value.is_empty() {
                return Err(JokerError::EmptyRule);
            }
            let normalized = value.iter().map(u8::to_ascii_lowercase).collect::<Vec<_>>();
            let rule = JokerRule { kind, normalized };
            if rule.matches(normalized_sysop.as_bytes()) {
                return Err(JokerError::NamedSysopDenied);
            }
            rules.push(rule);
            if rules.len() > MAX_JOKER_RULES {
                return Err(JokerError::TooManyRules(rules.len()));
            }
        }
        Ok(Self {
            generation,
            source_bytes: bytes,
            rules,
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }

    pub fn denial_for(&self, caller_name: &[u8]) -> Result<Option<JokerRuleKind>, JokerError> {
        let (_, normalized) =
            canonicalize_caller_name(caller_name).map_err(|_| JokerError::InvalidCallerName)?;
        Ok(self
            .rules
            .iter()
            .find(|rule| rule.matches(normalized.as_bytes()))
            .map(|rule| rule.kind))
    }
}

impl JokerRule {
    fn matches(&self, normalized_name: &[u8]) -> bool {
        match self.kind {
            JokerRuleKind::CompleteName => normalized_name == self.normalized,
            JokerRuleKind::Substring => normalized_name
                .windows(self.normalized.len())
                .any(|window| window == self.normalized),
        }
    }
}

impl RuntimeDatabase {
    pub fn mutate_caller_lifecycle(
        &mut self,
        caller_id: CallerId,
        expected_version: u64,
        target: CallerState,
        actor: CallerAccessActor,
        caller_config: &CallerConfig,
        now: i64,
    ) -> Result<Caller, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_actor(&transaction, actor, caller_config)?;
        let current = access_row(&transaction, caller_id)?;
        ensure_version(expected_version, current.state_version)?;
        if current.normalized_name == normalized_sysop(caller_config)?
            && target != CallerState::Active
        {
            return Err(DatabaseError::ProtectedNamedSysop);
        }
        let prior_for_tombstone = if target == CallerState::Deleted {
            match current.state {
                CallerState::Active | CallerState::Disabled => {
                    Some(current.state.as_database_value())
                }
                CallerState::Deleted => current.prior_state.as_deref(),
            }
        } else {
            None
        };
        if current.state == target {
            transaction.commit().map_err(DatabaseError::Sqlite)?;
            return self
                .caller_by_id(caller_id)?
                .ok_or(DatabaseError::MissingCaller(caller_id.get()));
        }
        let operation = match target {
            CallerState::Active if current.state == CallerState::Deleted => "restored",
            CallerState::Active => "enabled",
            CallerState::Disabled => "disabled",
            CallerState::Deleted => "tombstoned",
        };
        let new_version = current.state_version + 1;
        transaction
            .execute(
                "UPDATE callers SET account_state=?2, lifecycle_prior_state=?3, state_version=?4, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1 AND state_version=?5",
                params![caller_id.get(), target.as_database_value(), prior_for_tombstone, sql_u64(new_version)?, sql_u64(expected_version)?],
            )
            .map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            now,
            operation,
            caller_id,
            actor,
            Some(current.state),
            Some(target),
            Some(current.state_version),
            Some(new_version),
            None,
            None,
            None,
            None,
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn change_caller_base_security(
        &mut self,
        caller_id: CallerId,
        expected_version: u64,
        security: SecurityLevel,
        actor: CallerAccessActor,
        caller_config: &CallerConfig,
        now: i64,
    ) -> Result<Caller, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_actor(&transaction, actor, caller_config)?;
        let current = access_row(&transaction, caller_id)?;
        ensure_version(expected_version, current.state_version)?;
        if current.normalized_name == normalized_sysop(caller_config)?
            && security.get() < caller_config.sysop_security
        {
            return Err(DatabaseError::ProtectedNamedSysop);
        }
        if current.base_security == security.get() {
            transaction.commit().map_err(DatabaseError::Sqlite)?;
            return self
                .caller_by_id(caller_id)?
                .ok_or(DatabaseError::MissingCaller(caller_id.get()));
        }
        let new_version = current.state_version + 1;
        transaction.execute(
            "UPDATE callers SET security_level=?2, state_version=?3, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1 AND state_version=?4",
            params![caller_id.get(), security.get(), sql_u64(new_version)?, sql_u64(expected_version)?],
        ).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            now,
            "security-changed",
            caller_id,
            actor,
            None,
            None,
            Some(current.state_version),
            Some(new_version),
            Some(current.base_security),
            Some(security.get()),
            None,
            None,
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn set_caller_purge_protection(
        &mut self,
        caller_id: CallerId,
        expected_version: u64,
        protected: bool,
        actor: CallerAccessActor,
        caller_config: &CallerConfig,
        now: i64,
    ) -> Result<Caller, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_actor(&transaction, actor, caller_config)?;
        let current = access_row(&transaction, caller_id)?;
        ensure_version(expected_version, current.state_version)?;
        if current.normalized_name == normalized_sysop(caller_config)? && !protected {
            return Err(DatabaseError::ProtectedNamedSysop);
        }
        if current.purge_protected == protected {
            transaction.commit().map_err(DatabaseError::Sqlite)?;
            return self
                .caller_by_id(caller_id)?
                .ok_or(DatabaseError::MissingCaller(caller_id.get()));
        }
        let new_version = current.state_version + 1;
        transaction.execute(
            "UPDATE callers SET purge_protected=?2, state_version=?3, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1 AND state_version=?4",
            params![caller_id.get(), protected, sql_u64(new_version)?, sql_u64(expected_version)?],
        ).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            now,
            "purge-protection-changed",
            caller_id,
            actor,
            None,
            None,
            Some(current.state_version),
            Some(new_version),
            None,
            None,
            None,
            None,
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_caller_subscription(
        &mut self,
        caller_id: CallerId,
        expected_version: u64,
        expires_on: Option<NaiveDate>,
        actor: CallerAccessActor,
        caller_config: &CallerConfig,
        now: i64,
        timezone: Tz,
    ) -> Result<Caller, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_actor(&transaction, actor, caller_config)?;
        let current = access_row(&transaction, caller_id)?;
        ensure_version(expected_version, current.state_version)?;
        let date = expires_on.map(|value| value.format("%Y-%m-%d").to_string());
        let new_version = current.state_version + 1;
        transaction.execute(
            "UPDATE callers SET subscription_expires_on=?2, state_version=?3, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1 AND state_version=?4",
            params![caller_id.get(), date, sql_u64(new_version)?, sql_u64(expected_version)?],
        ).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            now,
            "subscription-updated",
            caller_id,
            actor,
            None,
            None,
            Some(current.state_version),
            Some(new_version),
            None,
            None,
            None,
            None,
        )?;
        if expires_on.is_none_or(|date| board_date(now, timezone).is_ok_and(|today| today <= date))
        {
            resolve_subscription_adjustment(&transaction, caller_id, now)?;
        }
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn evaluate_caller_subscription(
        &mut self,
        caller_id: CallerId,
        config: &CallerConfig,
        now: i64,
        timezone: Tz,
    ) -> Result<SubscriptionEvaluation, DatabaseError> {
        self.evaluate_subscription_policy(caller_id, config, now, timezone, true)
    }

    pub fn enforce_caller_access_at_dispatch(
        &mut self,
        caller_id: CallerId,
        config: &CallerConfig,
        now: i64,
        timezone: Tz,
    ) -> Result<SubscriptionEvaluation, DatabaseError> {
        self.evaluate_subscription_policy(caller_id, config, now, timezone, false)
    }

    fn evaluate_subscription_policy(
        &mut self,
        caller_id: CallerId,
        config: &CallerConfig,
        now: i64,
        timezone: Tz,
        emit_warning: bool,
    ) -> Result<SubscriptionEvaluation, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let current = access_row(&transaction, caller_id)?;
        let today = board_date(now, timezone)?;
        let expires = current
            .subscription_expires_on
            .as_deref()
            .map(parse_iso_date)
            .transpose()?;
        let mut warning = false;
        let mut applied = false;
        if config.subscription.enabled {
            if let Some(expires) = expires {
                if today > expires && current.normalized_name != normalized_sysop(config)? {
                    let active: bool = transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM caller_security_adjustments WHERE caller_id=?1 AND kind='subscription-expired' AND status='active')",
                        params![caller_id.get()], |row| row.get(0)).map_err(DatabaseError::Sqlite)?;
                    if !active {
                        let event_id = insert_event(
                            &transaction,
                            now,
                            "subscription-expired",
                            caller_id,
                            CallerAccessActor::SystemPolicy,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            Some("subscription-expired"),
                            None,
                        )?;
                        transaction.execute(
                            "INSERT INTO caller_security_adjustments (caller_id, kind, target_security_level, status, applied_at, applied_event_id) VALUES (?1, 'subscription-expired', ?2, 'active', ?3, ?4)",
                            params![caller_id.get(), config.subscription.expired_security, now, event_id],
                        ).map_err(DatabaseError::Sqlite)?;
                        applied = true;
                    }
                } else if today <= expires {
                    resolve_subscription_adjustment(&transaction, caller_id, now)?;
                    let days = (expires - today).num_days();
                    warning = config.subscription.warning_days > 0
                        && days >= 0
                        && days <= i64::from(config.subscription.warning_days);
                    if warning && emit_warning {
                        insert_event(
                            &transaction,
                            now,
                            "subscription-warning",
                            caller_id,
                            CallerAccessActor::SystemPolicy,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        )?;
                    }
                }
            }
        }
        let effective: u16 = transaction.query_row(
            "SELECT MIN(c.security_level, COALESCE((SELECT MIN(target_security_level) FROM caller_security_adjustments WHERE caller_id=c.caller_id AND status='active'), c.security_level)) FROM callers c WHERE c.caller_id=?1",
            params![caller_id.get()], |row| row.get(0)).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(SubscriptionEvaluation {
            warning: warning && emit_warning,
            adjustment_applied: applied,
            effective_security: SecurityLevel::new(effective)?,
        })
    }

    pub fn record_joker_denial(
        &self,
        caller_id: Option<CallerId>,
        generation: u64,
        now: i64,
    ) -> Result<(), DatabaseError> {
        self.connection.execute(
            "INSERT INTO caller_access_events (occurred_at, operation, outcome, subject_caller_id, actor_kind, policy_generation) VALUES (?1, 'joker-denied', 'denied', ?2, 'system-policy', ?3)",
            params![now, caller_id.map(CallerId::get), sql_u64(generation)?],
        ).map_err(DatabaseError::Sqlite)?;
        Ok(())
    }
}

#[derive(Debug)]
struct AccessRow {
    normalized_name: String,
    base_security: u16,
    state: CallerState,
    state_version: u64,
    subscription_expires_on: Option<String>,
    purge_protected: bool,
    prior_state: Option<String>,
}

fn access_row(transaction: &Transaction<'_>, id: CallerId) -> Result<AccessRow, DatabaseError> {
    let stored = transaction.query_row(
        "SELECT normalized_name, security_level, account_state, state_version, subscription_expires_on, purge_protected, lifecycle_prior_state FROM callers WHERE caller_id=?1",
        params![id.get()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u16>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, bool>(5)?, row.get::<_, Option<String>>(6)?)))
        .optional().map_err(DatabaseError::Sqlite)?;
    stored.map(|(name, security, state, version, subscription, purge, prior)| -> Result<AccessRow, DatabaseError> { Ok(AccessRow {
            normalized_name: name,
            base_security: security,
            state: CallerState::from_database_value(&state).map_err(DatabaseError::InvalidStoredCaller)?,
            state_version: u64::try_from(version).map_err(|_| DatabaseError::InvalidStoredCounter(version))?,
            subscription_expires_on: subscription,
            purge_protected: purge,
            prior_state: prior,
        }) }).transpose()?.ok_or(DatabaseError::MissingCaller(id.get()))
}

fn authorize_actor(
    transaction: &Transaction<'_>,
    actor: CallerAccessActor,
    config: &CallerConfig,
) -> Result<(), DatabaseError> {
    match actor {
        CallerAccessActor::LocalOperator | CallerAccessActor::SystemPolicy => Ok(()),
        CallerAccessActor::ThresholdSysop(id) => {
            let security: Option<u16> = transaction.query_row(
                "SELECT MIN(c.security_level, COALESCE((SELECT MIN(target_security_level) FROM caller_security_adjustments WHERE caller_id=c.caller_id AND status='active'), c.security_level)) FROM callers c WHERE c.caller_id=?1 AND c.account_state='active'",
                params![id.get()], |row| row.get(0)).optional().map_err(DatabaseError::Sqlite)?;
            if security.is_some_and(|value| value >= config.sysop_security) {
                Ok(())
            } else {
                Err(DatabaseError::CallerAccessUnauthorized)
            }
        }
    }
}

fn normalized_sysop(config: &CallerConfig) -> Result<String, DatabaseError> {
    canonicalize_caller_name(config.sysop_caller_name.as_bytes())
        .map(|(_, name)| name)
        .map_err(Into::into)
}

fn ensure_version(expected: u64, actual: u64) -> Result<(), DatabaseError> {
    if expected == actual {
        Ok(())
    } else {
        Err(DatabaseError::CallerStateConflict { expected, actual })
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    transaction: &Transaction<'_>,
    now: i64,
    operation: &str,
    subject: CallerId,
    actor: CallerAccessActor,
    prior: Option<CallerState>,
    new: Option<CallerState>,
    prior_version: Option<u64>,
    new_version: Option<u64>,
    prior_security: Option<u16>,
    new_security: Option<u16>,
    adjustment: Option<&str>,
    generation: Option<u64>,
) -> Result<i64, DatabaseError> {
    let (actor_kind, actor_id) = actor.database_values();
    transaction.execute(
        "INSERT INTO caller_access_events (occurred_at, operation, subject_caller_id, actor_kind, actor_caller_id, prior_lifecycle, new_lifecycle, prior_state_version, new_state_version, prior_base_security, new_base_security, adjustment_kind, policy_generation) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![now, operation, subject.get(), actor_kind, actor_id, prior.map(CallerState::as_database_value), new.map(CallerState::as_database_value), prior_version.map(sql_u64).transpose()?, new_version.map(sql_u64).transpose()?, prior_security, new_security, adjustment, generation.map(sql_u64).transpose()?],
    ).map_err(DatabaseError::Sqlite)?;
    Ok(transaction.last_insert_rowid())
}

fn resolve_subscription_adjustment(
    transaction: &Transaction<'_>,
    caller_id: CallerId,
    now: i64,
) -> Result<bool, DatabaseError> {
    let adjustment: Option<i64> = transaction.query_row(
        "SELECT adjustment_id FROM caller_security_adjustments WHERE caller_id=?1 AND kind='subscription-expired' AND status='active'",
        params![caller_id.get()], |row| row.get(0)).optional().map_err(DatabaseError::Sqlite)?;
    let Some(adjustment_id) = adjustment else {
        return Ok(false);
    };
    let event = insert_event(
        transaction,
        now,
        "subscription-adjustment-resolved",
        caller_id,
        CallerAccessActor::SystemPolicy,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("subscription-expired"),
        None,
    )?;
    transaction.execute(
        "UPDATE caller_security_adjustments SET status='resolved', resolved_at=?2, resolved_event_id=?3, state_version=state_version+1 WHERE adjustment_id=?1 AND status='active'",
        params![adjustment_id, now, event],
    ).map_err(DatabaseError::Sqlite)?;
    Ok(true)
}

fn board_date(now: i64, timezone: Tz) -> Result<NaiveDate, DatabaseError> {
    let utc = chrono::DateTime::<Utc>::from_timestamp(now, 0)
        .ok_or(crate::CallerError::InvalidTimestamp(now))?;
    Ok(utc.with_timezone(&timezone).date_naive())
}

fn parse_iso_date(value: &str) -> Result<NaiveDate, DatabaseError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| DatabaseError::InvalidSubscriptionDate(value.to_owned()))
}

fn sql_u64(value: u64) -> Result<i64, DatabaseError> {
    i64::try_from(value).map_err(|_| DatabaseError::CounterOverflow(value))
}

#[derive(Debug, Error)]
pub enum JokerError {
    #[error("could not read JOKER policy {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JOKER policy generation must be positive")]
    InvalidGeneration,
    #[error("JOKER policy exceeds {MAX_JOKER_BYTES} bytes: {0}")]
    TooLarge(usize),
    #[error("JOKER policy line exceeds {MAX_JOKER_LINE_BYTES} bytes: {0}")]
    LineTooLong(usize),
    #[error("JOKER policy contains a control or non-ASCII byte")]
    InvalidByte,
    #[error("JOKER policy contains an empty rule")]
    EmptyRule,
    #[error("JOKER policy exceeds {MAX_JOKER_RULES} rules: {0}")]
    TooManyRules(usize),
    #[error("configured named Sysop identity is invalid")]
    InvalidNamedSysop,
    #[error("caller name is invalid")]
    InvalidCallerName,
    #[error("JOKER policy would deny the configured named Sysop")]
    NamedSysopDenied,
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};
    use tempfile::TempDir;

    use super::*;
    use crate::{CallerState, RuntimeDatabase, SecurityLevel, SubscriptionConfig};

    fn database() -> (TempDir, RuntimeDatabase, Caller, CallerConfig) {
        let temp = TempDir::new().unwrap();
        let mut database = RuntimeDatabase::open(&temp.path().join("board.sqlite3")).unwrap();
        database.migrate().unwrap();
        let config = CallerConfig {
            sysop_caller_name: "Fixture Sysop".to_owned(),
            sysop_security: 50,
            subscription: SubscriptionConfig {
                enabled: true,
                warning_days: 7,
                expired_security: 5,
            },
            ..CallerConfig::default()
        };
        let caller = database
            .create_caller(
                b"Lifecycle Caller",
                "test-hash",
                SecurityLevel::new(25).unwrap(),
                CallerState::Active,
                false,
                1_700_000_000,
            )
            .unwrap();
        (temp, database, caller, config)
    }

    #[test]
    fn joker_policy_is_bounded_ascii_case_insensitive_and_private() {
        let policy =
            JokerPolicy::parse(b"Bad Caller\r\n@TRouble\n\n".to_vec(), 9, "Fixture Sysop").unwrap();
        assert_eq!(
            policy.denial_for(b"bad caller").unwrap(),
            Some(JokerRuleKind::CompleteName)
        );
        assert_eq!(
            policy.denial_for(b"No Trouble Here").unwrap(),
            Some(JokerRuleKind::Substring)
        );
        assert_eq!(policy.denial_for(b"Allowed Caller").unwrap(), None);
        assert_eq!(policy.source_bytes(), b"Bad Caller\r\n@TRouble\n\n");
        assert!(matches!(
            JokerPolicy::parse(b"@sysop".to_vec(), 1, "Fixture Sysop"),
            Err(JokerError::NamedSysopDenied)
        ));
        assert!(matches!(
            JokerPolicy::parse(vec![b'A'; MAX_JOKER_LINE_BYTES + 1], 1, "Fixture Sysop"),
            Err(JokerError::LineTooLong(_))
        ));
        assert!(matches!(
            JokerPolicy::parse(b"bad\tcaller".to_vec(), 1, "Fixture Sysop"),
            Err(JokerError::InvalidByte)
        ));
    }

    #[test]
    fn lifecycle_is_recoverable_versioned_audited_and_conflict_safe() {
        let (_temp, mut database, caller, config) = database();
        let disabled = database
            .mutate_caller_lifecycle(
                caller.id,
                caller.state_version,
                CallerState::Disabled,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_100,
            )
            .unwrap();
        assert_eq!(disabled.state, CallerState::Disabled);
        assert_eq!(disabled.state_version, 1);
        assert!(matches!(
            database.mutate_caller_lifecycle(
                caller.id,
                caller.state_version,
                CallerState::Deleted,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_101,
            ),
            Err(DatabaseError::CallerStateConflict { .. })
        ));
        let deleted = database
            .mutate_caller_lifecycle(
                caller.id,
                disabled.state_version,
                CallerState::Deleted,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_102,
            )
            .unwrap();
        assert_eq!(deleted.state, CallerState::Deleted);
        assert_eq!(deleted.lifecycle_prior_state, Some(CallerState::Disabled));
        let restored = database
            .mutate_caller_lifecycle(
                caller.id,
                deleted.state_version,
                CallerState::Active,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_103,
            )
            .unwrap();
        assert_eq!(restored.id, caller.id);
        assert_eq!(restored.state, CallerState::Active);
        let operations: String = database
            .connection
            .query_row(
                "SELECT group_concat(operation, ',') FROM caller_access_events WHERE subject_caller_id=?1",
                params![caller.id.get()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(operations, "disabled,tombstoned,restored");
        assert!(database
            .connection
            .execute("UPDATE caller_access_events SET operation='enabled'", [])
            .is_err());
    }

    #[test]
    fn base_security_survives_expiry_and_renewal_restores_effective_security() {
        let (_temp, mut database, caller, config) = database();
        let phoenix = chrono_tz::America::Phoenix;
        let before = phoenix
            .with_ymd_and_hms(2026, 8, 29, 12, 0, 0)
            .unwrap()
            .timestamp();
        let subscribed = database
            .update_caller_subscription(
                caller.id,
                caller.state_version,
                Some(NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()),
                CallerAccessActor::LocalOperator,
                &config,
                before,
                phoenix,
            )
            .unwrap();
        let warning = database
            .evaluate_caller_subscription(caller.id, &config, before, phoenix)
            .unwrap();
        assert!(warning.warning);
        assert!(!warning.adjustment_applied);
        let expired_at = phoenix
            .with_ymd_and_hms(2026, 8, 30, 0, 0, 0)
            .unwrap()
            .timestamp();
        let expired = database
            .evaluate_caller_subscription(caller.id, &config, expired_at, phoenix)
            .unwrap();
        assert!(expired.adjustment_applied);
        assert_eq!(expired.effective_security.get(), 5);
        let repeated = database
            .evaluate_caller_subscription(caller.id, &config, expired_at + 60, phoenix)
            .unwrap();
        assert!(!repeated.adjustment_applied);
        assert_eq!(
            database
                .caller_by_id(caller.id)
                .unwrap()
                .unwrap()
                .base_security_level
                .get(),
            25
        );
        let restricted = database.caller_by_id(caller.id).unwrap().unwrap();
        let changed = database
            .change_caller_base_security(
                caller.id,
                restricted.state_version,
                SecurityLevel::new(40).unwrap(),
                CallerAccessActor::LocalOperator,
                &config,
                expired_at + 120,
            )
            .unwrap();
        assert_eq!(changed.base_security_level.get(), 40);
        assert_eq!(changed.security_level.get(), 5);
        let renewed = database
            .update_caller_subscription(
                caller.id,
                changed.state_version,
                Some(NaiveDate::from_ymd_opt(2027, 8, 29).unwrap()),
                CallerAccessActor::LocalOperator,
                &config,
                expired_at + 180,
                phoenix,
            )
            .unwrap();
        assert_eq!(renewed.security_level.get(), 40);
        assert_eq!(renewed.base_security_level.get(), 40);
        assert_eq!(
            subscribed.subscription_expires_on.unwrap().to_string(),
            "2026-08-29"
        );
    }

    #[test]
    fn named_sysop_is_protected_from_lifecycle_security_and_purge_mutation() {
        let (_temp, mut database, _caller, config) = database();
        let sysop = database
            .create_caller(
                b"Fixture Sysop",
                "test-hash",
                SecurityLevel::new(50).unwrap(),
                CallerState::Active,
                false,
                1_700_000_000,
            )
            .unwrap();
        assert!(matches!(
            database.mutate_caller_lifecycle(
                sysop.id,
                0,
                CallerState::Disabled,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_001
            ),
            Err(DatabaseError::ProtectedNamedSysop)
        ));
        assert!(matches!(
            database.change_caller_base_security(
                sysop.id,
                0,
                SecurityLevel::new(49).unwrap(),
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_001
            ),
            Err(DatabaseError::ProtectedNamedSysop)
        ));
        assert!(matches!(
            database.set_caller_purge_protection(
                sysop.id,
                0,
                false,
                CallerAccessActor::LocalOperator,
                &config,
                1_700_000_001
            ),
            Err(DatabaseError::ProtectedNamedSysop)
        ));
    }

    #[test]
    fn joker_audit_contains_no_name_or_rule_content() {
        let (_temp, database, caller, _config) = database();
        database
            .record_joker_denial(Some(caller.id), 7, 1_700_000_500)
            .unwrap();
        let columns: (String, String, i64) = database.connection.query_row(
            "SELECT operation, actor_kind, policy_generation FROM caller_access_events WHERE operation='joker-denied'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
        assert_eq!(
            columns,
            ("joker-denied".to_owned(), "system-policy".to_owned(), 7)
        );
        let schema: String = database
            .connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='caller_access_events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        for forbidden in [
            "password",
            "address",
            "phone",
            "email",
            "birthday",
            "rule_text",
        ] {
            assert!(!schema.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn two_operator_connections_commit_only_one_versioned_mutation() {
        let (temp, database, caller, config) = database();
        let path = database.path().to_owned();
        drop(database);
        let caller_id = caller.id;
        let state_version = caller.state_version;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let results = [CallerState::Disabled, CallerState::Deleted]
            .into_iter()
            .map(|target| {
                let path = path.clone();
                let barrier = barrier.clone();
                let config = config.clone();
                std::thread::spawn(move || {
                    let mut database = RuntimeDatabase::open(&path).unwrap();
                    barrier.wait();
                    database.mutate_caller_lifecycle(
                        caller_id,
                        state_version,
                        target,
                        CallerAccessActor::LocalOperator,
                        &config,
                        1_700_001_000,
                    )
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = results
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(DatabaseError::CallerStateConflict { .. })))
                .count(),
            1
        );
        let database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database
                .caller_by_id(caller_id)
                .unwrap()
                .unwrap()
                .state_version,
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM caller_access_events WHERE subject_caller_id=?1",
                    params![caller_id.get()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        drop(temp);
    }
}
