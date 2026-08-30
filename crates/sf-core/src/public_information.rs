//! Privacy-bounded public-information persistence and projections.
//!
//! This module deliberately exposes projections rather than caller records.
//! Stable caller IDs remain internal ownership/audit identities; handles are
//! the only authentication-independent identity released to callers.

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use thiserror::Error;

use crate::{CallerId, CallerState, DatabaseError, RuntimeDatabase, SecurityLevel};

pub const MAX_DIRECTORY_QUERY_BYTES: usize = 30;
pub const MAX_DIRECTORY_RESULTS: usize = 50;
pub const MAX_DIRECTORY_PAGE_SIZE: usize = 50;
pub const MAX_OTHER_BBS_ENTRIES: usize = 512;
pub const MAX_OTHER_BBS_NAME_BYTES: usize = 60;
pub const MAX_OTHER_BBS_SPEED_BYTES: usize = 32;
pub const MAX_OTHER_BBS_DIAL_BYTES: usize = 64;
pub const MAX_NATIVE_THOUGHTS: usize = 256;
pub const MAX_NATIVE_THOUGHT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicInformationActor {
    Caller(CallerId),
    ThresholdSysop {
        caller_id: CallerId,
        sysop_security: SecurityLevel,
    },
    LocalOperator,
    SystemPolicy,
}

impl PublicInformationActor {
    fn database_values(self) -> (&'static str, Option<i64>) {
        match self {
            Self::Caller(id) => ("caller", Some(id.get())),
            Self::ThresholdSysop { caller_id, .. } => ("threshold-sysop", Some(caller_id.get())),
            Self::LocalOperator => ("local-operator", None),
            Self::SystemPolicy => ("system-policy", None),
        }
    }

    fn is_operator(self) -> bool {
        matches!(self, Self::ThresholdSysop { .. } | Self::LocalOperator)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicDirectoryPolicy {
    pub enabled: bool,
    pub show_last_call_date: bool,
    pub show_city_region: bool,
    pub caller_bbs_additions_enabled: bool,
    pub state_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicResourceState {
    pub kind: String,
    pub generation: u64,
    pub sha256: String,
    pub published_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerPublicity {
    pub listed: bool,
    pub state_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicCallerSummary {
    /// Internal identity used only to revalidate immediately before display.
    pub caller_id: CallerId,
    pub handle: String,
    pub last_call_at: Option<i64>,
    pub city_region: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtherBbsLifecycle {
    Active,
    Disabled,
}

impl OtherBbsLifecycle {
    fn database_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    fn from_database(value: &str) -> Result<Self, PublicInformationError> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(PublicInformationError::InvalidStoredState(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OtherBbsId(i64);

impl OtherBbsId {
    pub fn new(value: i64) -> Result<Self, PublicInformationError> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(PublicInformationError::InvalidId(value))
        }
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtherBbsEntry {
    pub id: OtherBbsId,
    pub name: String,
    pub speed: String,
    pub dial_string: String,
    pub order: u16,
    pub lifecycle: OtherBbsLifecycle,
    pub state_version: u64,
    /// Stable internal identity; never rendered as part of the public row.
    pub contributor_caller_id: Option<CallerId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOtherBbsEntry {
    pub name: String,
    pub speed: String,
    pub dial_string: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeThoughtCatalog {
    thoughts: Vec<String>,
}

pub trait ThoughtCatalogReader {
    fn thoughts(&self) -> &[String];
    fn select(&self, selector: u64) -> Option<&str>;
}

/// Explicit boundary for a future evidence-backed `SFBBSLST.DAT` adapter.
/// Native SQLite rows remain authoritative; no implementation is supplied
/// until delimiter, width, escaping, and round-trip behavior are proven.
pub trait LegacyOtherBbsAdapter {
    type Error;

    fn import(&self, bytes: &[u8]) -> Result<Vec<NewOtherBbsEntry>, Self::Error>;
    fn export(&self, entries: &[OtherBbsEntry]) -> Result<Vec<u8>, Self::Error>;
}

impl NativeThoughtCatalog {
    /// Reads the project-native `THOUGHTS.NG` format: validated UTF-8 with one
    /// non-empty thought per line. This is not a `THOUGHTS.BBS` parser.
    pub fn parse(bytes: &[u8]) -> Result<Self, PublicInformationError> {
        if bytes.len() > MAX_NATIVE_THOUGHTS * (MAX_NATIVE_THOUGHT_BYTES + 1) {
            return Err(PublicInformationError::ThoughtCatalogTooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| PublicInformationError::InvalidUtf8)?;
        let mut thoughts = Vec::new();
        for line in text.lines() {
            let thought = line.trim_end_matches('\r').trim();
            if thought.is_empty() {
                continue;
            }
            validate_text(thought, MAX_NATIVE_THOUGHT_BYTES, "thought")?;
            thoughts.push(thought.to_owned());
            if thoughts.len() > MAX_NATIVE_THOUGHTS {
                return Err(PublicInformationError::ThoughtCatalogTooLarge);
            }
        }
        Ok(Self { thoughts })
    }
}

impl ThoughtCatalogReader for NativeThoughtCatalog {
    fn thoughts(&self) -> &[String] {
        &self.thoughts
    }

    fn select(&self, selector: u64) -> Option<&str> {
        if self.thoughts.is_empty() {
            None
        } else {
            self.thoughts
                .get((selector as usize) % self.thoughts.len())
                .map(String::as_str)
        }
    }
}

#[derive(Debug, Error)]
pub enum PublicInformationError {
    #[error("public-information mutation is not authorized")]
    Unauthorized,
    #[error("caller public-directory preference conflicts with state version {actual}; expected {expected}")]
    CallerPublicityConflict { expected: u64, actual: u64 },
    #[error("public-directory policy conflicts with state version {actual}; expected {expected}")]
    PolicyConflict { expected: u64, actual: u64 },
    #[error("Other BBS entry conflicts with state version {actual}; expected {expected}")]
    OtherBbsConflict { expected: u64, actual: u64 },
    #[error("public directory is disabled")]
    DirectoryDisabled,
    #[error("caller is not eligible for public-directory participation")]
    CallerUnavailable,
    #[error("caller additions to Other BBS information are disabled")]
    CallerBbsAdditionsDisabled,
    #[error("Other BBS directory contains its maximum of {MAX_OTHER_BBS_ENTRIES} entries")]
    OtherBbsFull,
    #[error("Other BBS entry duplicates an existing row")]
    DuplicateOtherBbs,
    #[error("Other BBS entry {0} does not exist")]
    MissingOtherBbs(i64),
    #[error("invalid Other BBS entry identifier {0}")]
    InvalidId(i64),
    #[error("invalid stored public-information state {0:?}")]
    InvalidStoredState(String),
    #[error("{field} must contain 1..={maximum} bytes of text without controls")]
    InvalidText { field: &'static str, maximum: usize },
    #[error("caller locate query must contain 1..={MAX_DIRECTORY_QUERY_BYTES} ASCII bytes")]
    InvalidQuery,
    #[error("native thought catalog is not valid UTF-8")]
    InvalidUtf8,
    #[error("native thought catalog exceeds its record or byte bounds")]
    ThoughtCatalogTooLarge,
    #[error("requested ordering position is outside the Other BBS directory")]
    InvalidOrder,
    #[error("public resource kind is not recognized")]
    InvalidResourceKind,
    #[error("public resource digest must be lowercase SHA-256 hexadecimal")]
    InvalidResourceDigest,
}

impl RuntimeDatabase {
    pub fn public_directory_policy(&self) -> Result<PublicDirectoryPolicy, DatabaseError> {
        self.connection
            .query_row(
                "SELECT directory_enabled, show_last_call_date, show_city_region, caller_bbs_additions_enabled, state_version FROM public_information_policy WHERE singleton=1",
                [],
                |row| Ok(PublicDirectoryPolicy { enabled: row.get(0)?, show_last_call_date: row.get(1)?, show_city_region: row.get(2)?, caller_bbs_additions_enabled: row.get(3)?, state_version: row.get::<_, i64>(4)? as u64 }),
            )
            .map_err(DatabaseError::Sqlite)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_public_directory_policy(
        &mut self,
        actor: PublicInformationActor,
        expected_version: u64,
        enabled: bool,
        show_last_call_date: bool,
        show_city_region: bool,
        caller_bbs_additions_enabled: bool,
        occurred_at: i64,
    ) -> Result<PublicDirectoryPolicy, DatabaseError> {
        if !actor.is_operator() {
            return Err(PublicInformationError::Unauthorized.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_operator(&transaction, actor)?;
        let actual: u64 = transaction
            .query_row(
                "SELECT state_version FROM public_information_policy WHERE singleton=1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(DatabaseError::Sqlite)? as u64;
        if actual != expected_version {
            return Err(PublicInformationError::PolicyConflict {
                expected: expected_version,
                actual,
            }
            .into());
        }
        let next = actual + 1;
        transaction.execute("UPDATE public_information_policy SET directory_enabled=?1, show_last_call_date=?2, show_city_region=?3, caller_bbs_additions_enabled=?4, state_version=?5, updated_at=CURRENT_TIMESTAMP WHERE singleton=1", params![enabled, show_last_call_date, show_city_region, caller_bbs_additions_enabled, next as i64]).map_err(DatabaseError::Sqlite)?;
        let detail = format!("enabled={enabled};last-call={show_last_call_date};location={show_city_region};caller-additions={caller_bbs_additions_enabled}");
        insert_event(
            &transaction,
            occurred_at,
            "policy-changed",
            actor,
            None,
            None,
            Some(actual),
            Some(next),
            Some(&detail),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.public_directory_policy()
    }

    pub fn caller_publicity(&self, caller_id: CallerId) -> Result<CallerPublicity, DatabaseError> {
        self.connection.query_row("SELECT public_directory_listed, publicity_state_version FROM callers WHERE caller_id=?1", [caller_id.get()], |row| Ok(CallerPublicity { listed: row.get(0)?, state_version: row.get::<_, i64>(1)? as u64 })).optional().map_err(DatabaseError::Sqlite)?.ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn update_caller_publicity(
        &mut self,
        actor: PublicInformationActor,
        caller_id: CallerId,
        expected_version: u64,
        listed: bool,
        occurred_at: i64,
    ) -> Result<CallerPublicity, DatabaseError> {
        if matches!(actor, PublicInformationActor::Caller(id) if id != caller_id)
            || matches!(actor, PublicInformationActor::SystemPolicy)
        {
            return Err(PublicInformationError::Unauthorized.into());
        }
        // Operators may protect privacy by unlisting, but cannot opt a caller in.
        if actor.is_operator() && listed {
            return Err(PublicInformationError::Unauthorized.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        if actor.is_operator() {
            authorize_operator(&transaction, actor)?;
        }
        let (state, actual): (String, i64) = transaction
            .query_row(
                "SELECT account_state, publicity_state_version FROM callers WHERE caller_id=?1",
                [caller_id.get()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(DatabaseError::Sqlite)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))?;
        if state != CallerState::Active.as_database_value() {
            return Err(PublicInformationError::CallerUnavailable.into());
        }
        let actual = actual as u64;
        if actual != expected_version {
            return Err(PublicInformationError::CallerPublicityConflict {
                expected: expected_version,
                actual,
            }
            .into());
        }
        let next = actual + 1;
        transaction.execute("UPDATE callers SET public_directory_listed=?2, publicity_state_version=?3, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1", params![caller_id.get(), listed, next as i64]).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            occurred_at,
            if listed {
                "caller-listed"
            } else {
                "caller-unlisted"
            },
            actor,
            Some(caller_id),
            None,
            Some(actual),
            Some(next),
            Some(if listed {
                "listed=true"
            } else {
                "listed=false"
            }),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(CallerPublicity {
            listed,
            state_version: next,
        })
    }

    pub fn public_caller_directory(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PublicCallerSummary>, DatabaseError> {
        let policy = self.public_directory_policy()?;
        if !policy.enabled {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, MAX_DIRECTORY_PAGE_SIZE);
        query_public_callers(&self.connection, None, offset, limit, &policy)
    }

    pub fn locate_public_callers(
        &self,
        query: &str,
    ) -> Result<Vec<PublicCallerSummary>, DatabaseError> {
        if query.is_empty()
            || query.len() > MAX_DIRECTORY_QUERY_BYTES
            || !query.is_ascii()
            || query.as_bytes().iter().any(u8::is_ascii_control)
        {
            return Err(PublicInformationError::InvalidQuery.into());
        }
        let policy = self.public_directory_policy()?;
        if !policy.enabled {
            return Ok(Vec::new());
        }
        query_public_callers(
            &self.connection,
            Some(&query.to_ascii_lowercase()),
            0,
            MAX_DIRECTORY_RESULTS,
            &policy,
        )
    }

    pub fn revalidate_public_caller(
        &self,
        caller_id: CallerId,
    ) -> Result<Option<PublicCallerSummary>, DatabaseError> {
        let policy = self.public_directory_policy()?;
        if !policy.enabled {
            return Ok(None);
        }
        let rows = query_public_callers_by_id(&self.connection, caller_id, &policy)?;
        Ok(rows.into_iter().next())
    }

    pub fn other_bbs_entries(
        &self,
        include_disabled: bool,
    ) -> Result<Vec<OtherBbsEntry>, DatabaseError> {
        let sql = if include_disabled {
            "SELECT entry_id, bbs_name, speed_label, dial_string, display_order, lifecycle, state_version, contributor_caller_id FROM other_bbs_entries ORDER BY display_order, entry_id"
        } else {
            "SELECT entry_id, bbs_name, speed_label, dial_string, display_order, lifecycle, state_version, contributor_caller_id FROM other_bbs_entries WHERE lifecycle='active' ORDER BY display_order, entry_id"
        };
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map([], map_other_bbs)
            .map_err(DatabaseError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?
            .into_iter()
            .map(decode_other_bbs)
            .collect()
    }

    pub fn add_other_bbs(
        &mut self,
        actor: PublicInformationActor,
        entry: NewOtherBbsEntry,
        occurred_at: i64,
    ) -> Result<OtherBbsEntry, DatabaseError> {
        let entry = validate_new_other_bbs(entry)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let contributor = match actor {
            PublicInformationActor::Caller(id) => {
                let allowed: bool = transaction.query_row("SELECT p.caller_bbs_additions_enabled AND c.account_state='active' FROM public_information_policy p JOIN callers c ON c.caller_id=?1 WHERE p.singleton=1", [id.get()], |row| row.get(0)).optional().map_err(DatabaseError::Sqlite)?.unwrap_or(false);
                if !allowed {
                    return Err(PublicInformationError::CallerBbsAdditionsDisabled.into());
                }
                Some(id)
            }
            _ if actor.is_operator() => {
                authorize_operator(&transaction, actor)?;
                None
            }
            _ => return Err(PublicInformationError::Unauthorized.into()),
        };
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM other_bbs_entries", [], |row| {
                row.get(0)
            })
            .map_err(DatabaseError::Sqlite)?;
        if count as usize >= MAX_OTHER_BBS_ENTRIES {
            return Err(PublicInformationError::OtherBbsFull.into());
        }
        ensure_other_bbs_unique(&transaction, None, &entry)?;
        let order = count + 1;
        transaction.execute("INSERT INTO other_bbs_entries (bbs_name, speed_label, dial_string, display_order, contributor_caller_id) VALUES (?1,?2,?3,?4,?5)", params![entry.name, entry.speed, entry.dial_string, order, contributor.map(CallerId::get)]).map_err(DatabaseError::Sqlite)?;
        let id = OtherBbsId::new(transaction.last_insert_rowid())?;
        insert_event(
            &transaction,
            occurred_at,
            "other-bbs-added",
            actor,
            contributor,
            Some(id),
            None,
            Some(1),
            Some("entry-created"),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.other_bbs_by_id(id)
    }

    pub fn edit_other_bbs(
        &mut self,
        actor: PublicInformationActor,
        id: OtherBbsId,
        expected_version: u64,
        entry: NewOtherBbsEntry,
        occurred_at: i64,
    ) -> Result<OtherBbsEntry, DatabaseError> {
        if !actor.is_operator() {
            return Err(PublicInformationError::Unauthorized.into());
        }
        let entry = validate_new_other_bbs(entry)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_operator(&transaction, actor)?;
        let actual = other_bbs_version(&transaction, id)?;
        if actual != expected_version {
            return Err(PublicInformationError::OtherBbsConflict {
                expected: expected_version,
                actual,
            }
            .into());
        }
        ensure_other_bbs_unique(&transaction, Some(id), &entry)?;
        let next = actual + 1;
        transaction.execute("UPDATE other_bbs_entries SET bbs_name=?2,speed_label=?3,dial_string=?4,state_version=?5,updated_at=CURRENT_TIMESTAMP WHERE entry_id=?1", params![id.get(), entry.name, entry.speed, entry.dial_string, next as i64]).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            occurred_at,
            "other-bbs-edited",
            actor,
            None,
            Some(id),
            Some(actual),
            Some(next),
            Some("public-fields-replaced"),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.other_bbs_by_id(id)
    }

    pub fn set_other_bbs_lifecycle(
        &mut self,
        actor: PublicInformationActor,
        id: OtherBbsId,
        expected_version: u64,
        lifecycle: OtherBbsLifecycle,
        occurred_at: i64,
    ) -> Result<OtherBbsEntry, DatabaseError> {
        if !actor.is_operator() {
            return Err(PublicInformationError::Unauthorized.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_operator(&transaction, actor)?;
        let actual = other_bbs_version(&transaction, id)?;
        if actual != expected_version {
            return Err(PublicInformationError::OtherBbsConflict {
                expected: expected_version,
                actual,
            }
            .into());
        }
        let next = actual + 1;
        transaction.execute("UPDATE other_bbs_entries SET lifecycle=?2,state_version=?3,updated_at=CURRENT_TIMESTAMP WHERE entry_id=?1", params![id.get(), lifecycle.database_value(), next as i64]).map_err(DatabaseError::Sqlite)?;
        insert_event(
            &transaction,
            occurred_at,
            if lifecycle == OtherBbsLifecycle::Active {
                "other-bbs-restored"
            } else {
                "other-bbs-disabled"
            },
            actor,
            None,
            Some(id),
            Some(actual),
            Some(next),
            Some(if lifecycle == OtherBbsLifecycle::Active {
                "lifecycle=active"
            } else {
                "lifecycle=disabled"
            }),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.other_bbs_by_id(id)
    }

    pub fn reorder_other_bbs(
        &mut self,
        actor: PublicInformationActor,
        id: OtherBbsId,
        expected_version: u64,
        new_order: usize,
        occurred_at: i64,
    ) -> Result<OtherBbsEntry, DatabaseError> {
        if !actor.is_operator() {
            return Err(PublicInformationError::Unauthorized.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        authorize_operator(&transaction, actor)?;
        let actual = other_bbs_version(&transaction, id)?;
        if actual != expected_version {
            return Err(PublicInformationError::OtherBbsConflict {
                expected: expected_version,
                actual,
            }
            .into());
        }
        let mut ids = {
            let mut statement = transaction
                .prepare("SELECT entry_id FROM other_bbs_entries ORDER BY display_order, entry_id")
                .map_err(DatabaseError::Sqlite)?;
            let rows = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(DatabaseError::Sqlite)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::Sqlite)?
        };
        if new_order == 0 || new_order > ids.len() {
            return Err(PublicInformationError::InvalidOrder.into());
        }
        let old = ids
            .iter()
            .position(|value| *value == id.get())
            .ok_or(PublicInformationError::MissingOtherBbs(id.get()))?;
        ids.remove(old);
        ids.insert(new_order - 1, id.get());
        for (index, entry_id) in ids.iter().enumerate() {
            transaction
                .execute(
                    "UPDATE other_bbs_entries SET display_order=?2 WHERE entry_id=?1",
                    params![entry_id, (index + 1) as i64],
                )
                .map_err(DatabaseError::Sqlite)?;
        }
        let next = actual + 1;
        transaction.execute("UPDATE other_bbs_entries SET state_version=?2,updated_at=CURRENT_TIMESTAMP WHERE entry_id=?1", params![id.get(), next as i64]).map_err(DatabaseError::Sqlite)?;
        let detail = format!("display-order={new_order}");
        insert_event(
            &transaction,
            occurred_at,
            "other-bbs-reordered",
            actor,
            None,
            Some(id),
            Some(actual),
            Some(next),
            Some(&detail),
        )?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.other_bbs_by_id(id)
    }

    pub fn public_system_facts(&self) -> Result<(i64, u64), DatabaseError> {
        let (started, calls): (i64, i64) = self.connection.query_row("SELECT unixepoch(b.created_at), COALESCE((SELECT SUM(call_count) FROM callers),0) FROM board_identity b WHERE singleton=1", [], |row| Ok((row.get(0)?, row.get(1)?))).map_err(DatabaseError::Sqlite)?;
        Ok((
            started,
            u64::try_from(calls).map_err(|_| DatabaseError::InvalidStoredCounter(calls))?,
        ))
    }

    pub fn public_resource_state(
        &self,
        kind: &str,
    ) -> Result<Option<PublicResourceState>, DatabaseError> {
        validate_resource_kind(kind)?;
        self.connection.query_row("SELECT resource_kind,generation,sha256,published_at FROM public_information_resource_state WHERE resource_kind=?1", [kind], |row| Ok((row.get::<_, String>(0)?,row.get::<_, i64>(1)?,row.get::<_, String>(2)?,row.get::<_, i64>(3)?))).optional().map_err(DatabaseError::Sqlite)?.map(|row| Ok(PublicResourceState { kind: row.0, generation: u64::try_from(row.1).map_err(|_| PublicInformationError::InvalidStoredState(row.1.to_string()))?, sha256: row.2, published_at: row.3 })).transpose()
    }

    pub fn observe_public_resource(
        &mut self,
        kind: &str,
        sha256: &str,
        observed_at: i64,
    ) -> Result<PublicResourceState, DatabaseError> {
        validate_resource_kind(kind)?;
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(PublicInformationError::InvalidResourceDigest.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let prior = transaction.query_row("SELECT generation,sha256,published_at FROM public_information_resource_state WHERE resource_kind=?1", [kind], |row| Ok((row.get::<_, i64>(0)?,row.get::<_, String>(1)?,row.get::<_, i64>(2)?))).optional().map_err(DatabaseError::Sqlite)?;
        let state = match prior {
            Some((generation, digest, published_at)) if digest == sha256 => PublicResourceState {
                kind: kind.to_owned(),
                generation: generation as u64,
                sha256: digest,
                published_at,
            },
            prior => {
                let generation = prior.as_ref().map_or(1, |value| value.0 + 1);
                transaction.execute("INSERT INTO public_information_resource_state(resource_kind,generation,sha256,published_at) VALUES(?1,?2,?3,?4) ON CONFLICT(resource_kind) DO UPDATE SET generation=excluded.generation,sha256=excluded.sha256,published_at=excluded.published_at", params![kind,generation,sha256,observed_at]).map_err(DatabaseError::Sqlite)?;
                transaction.execute("INSERT INTO public_information_events(occurred_at,operation,actor_kind,resource_kind,resource_digest,prior_state_version,new_state_version,semantic_detail) VALUES(?1,'resource-published','system-policy',?2,?3,?4,?5,'board-resource-digest-changed')", params![observed_at,kind,sha256,prior.map(|value| value.0),generation]).map_err(DatabaseError::Sqlite)?;
                PublicResourceState {
                    kind: kind.to_owned(),
                    generation: generation as u64,
                    sha256: sha256.to_owned(),
                    published_at: observed_at,
                }
            }
        };
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(state)
    }

    fn other_bbs_by_id(&self, id: OtherBbsId) -> Result<OtherBbsEntry, DatabaseError> {
        let row = self.connection.query_row("SELECT entry_id,bbs_name,speed_label,dial_string,display_order,lifecycle,state_version,contributor_caller_id FROM other_bbs_entries WHERE entry_id=?1", [id.get()], map_other_bbs).optional().map_err(DatabaseError::Sqlite)?.ok_or(PublicInformationError::MissingOtherBbs(id.get()))?;
        decode_other_bbs(row)
    }
}

type StoredOtherBbs = (i64, String, String, String, i64, String, i64, Option<i64>);

fn map_other_bbs(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredOtherBbs> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

fn decode_other_bbs(row: StoredOtherBbs) -> Result<OtherBbsEntry, DatabaseError> {
    Ok(OtherBbsEntry {
        id: OtherBbsId::new(row.0)?,
        name: row.1,
        speed: row.2,
        dial_string: row.3,
        order: u16::try_from(row.4).map_err(|_| PublicInformationError::InvalidOrder)?,
        lifecycle: OtherBbsLifecycle::from_database(&row.5)?,
        state_version: u64::try_from(row.6)
            .map_err(|_| PublicInformationError::InvalidStoredState(row.6.to_string()))?,
        contributor_caller_id: row
            .7
            .map(CallerId::new)
            .transpose()
            .map_err(DatabaseError::InvalidStoredCaller)?,
    })
}

fn query_public_callers(
    connection: &rusqlite::Connection,
    query: Option<&str>,
    offset: usize,
    limit: usize,
    policy: &PublicDirectoryPolicy,
) -> Result<Vec<PublicCallerSummary>, DatabaseError> {
    let mut sql = String::from("SELECT caller_id,display_name,last_call_at,city,region FROM callers WHERE account_state='active' AND public_directory_listed=1");
    if query.is_some() {
        sql.push_str(" AND instr(lower(display_name),?1)>0");
    }
    sql.push_str(" ORDER BY lower(display_name),caller_id LIMIT ?2 OFFSET ?3");
    let query_value = query.unwrap_or("");
    let mut statement = connection.prepare(&sql).map_err(DatabaseError::Sqlite)?;
    let rows = statement
        .query_map(params![query_value, limit as i64, offset as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(DatabaseError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::Sqlite)?
        .into_iter()
        .map(|row| project_public_caller(row, policy))
        .collect()
}

fn query_public_callers_by_id(
    connection: &rusqlite::Connection,
    caller_id: CallerId,
    policy: &PublicDirectoryPolicy,
) -> Result<Vec<PublicCallerSummary>, DatabaseError> {
    let row = connection.query_row("SELECT caller_id,display_name,last_call_at,city,region FROM callers WHERE caller_id=?1 AND account_state='active' AND public_directory_listed=1", [caller_id.get()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?))).optional().map_err(DatabaseError::Sqlite)?;
    row.map(|value| project_public_caller(value, policy))
        .transpose()
        .map(|value| value.into_iter().collect())
}

fn project_public_caller(
    row: (i64, String, Option<i64>, Option<String>, Option<String>),
    policy: &PublicDirectoryPolicy,
) -> Result<PublicCallerSummary, DatabaseError> {
    let city_region = if policy.show_city_region {
        let values = [row.3, row.4]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>();
        (!values.is_empty()).then(|| values.join(", "))
    } else {
        None
    };
    Ok(PublicCallerSummary {
        caller_id: CallerId::new(row.0).map_err(DatabaseError::InvalidStoredCaller)?,
        handle: row.1,
        last_call_at: policy.show_last_call_date.then_some(row.2).flatten(),
        city_region,
    })
}

fn validate_new_other_bbs(
    mut entry: NewOtherBbsEntry,
) -> Result<NewOtherBbsEntry, PublicInformationError> {
    entry.name = entry.name.trim().to_owned();
    entry.speed = entry.speed.trim().to_owned();
    entry.dial_string = entry.dial_string.trim().to_owned();
    validate_text(&entry.name, MAX_OTHER_BBS_NAME_BYTES, "BBS name")?;
    validate_text(&entry.speed, MAX_OTHER_BBS_SPEED_BYTES, "speed")?;
    validate_text(&entry.dial_string, MAX_OTHER_BBS_DIAL_BYTES, "dial string")?;
    Ok(entry)
}

fn validate_text(
    value: &str,
    maximum: usize,
    field: &'static str,
) -> Result<(), PublicInformationError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        Err(PublicInformationError::InvalidText { field, maximum })
    } else {
        Ok(())
    }
}

fn canonical_bbs(entry: &NewOtherBbsEntry) -> (String, String, String) {
    fn part(value: &str) -> String {
        value
            .split_ascii_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase()
    }
    (
        part(&entry.name),
        part(&entry.speed),
        part(&entry.dial_string),
    )
}

fn ensure_other_bbs_unique(
    transaction: &rusqlite::Transaction<'_>,
    except: Option<OtherBbsId>,
    entry: &NewOtherBbsEntry,
) -> Result<(), DatabaseError> {
    let wanted = canonical_bbs(entry);
    let mut statement = transaction
        .prepare("SELECT entry_id,bbs_name,speed_label,dial_string FROM other_bbs_entries")
        .map_err(DatabaseError::Sqlite)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(DatabaseError::Sqlite)?;
    for row in rows {
        let (id, name, speed, dial) = row.map_err(DatabaseError::Sqlite)?;
        if except.is_some_and(|except| except.get() == id) {
            continue;
        }
        if canonical_bbs(&NewOtherBbsEntry {
            name,
            speed,
            dial_string: dial,
        }) == wanted
        {
            return Err(PublicInformationError::DuplicateOtherBbs.into());
        }
    }
    Ok(())
}

fn other_bbs_version(
    transaction: &rusqlite::Transaction<'_>,
    id: OtherBbsId,
) -> Result<u64, DatabaseError> {
    let value = transaction
        .query_row(
            "SELECT state_version FROM other_bbs_entries WHERE entry_id=?1",
            [id.get()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(DatabaseError::Sqlite)?
        .ok_or(PublicInformationError::MissingOtherBbs(id.get()))?;
    u64::try_from(value)
        .map_err(|_| PublicInformationError::InvalidStoredState(value.to_string()).into())
}

fn authorize_operator(
    transaction: &rusqlite::Transaction<'_>,
    actor: PublicInformationActor,
) -> Result<(), DatabaseError> {
    match actor {
        PublicInformationActor::LocalOperator => Ok(()),
        PublicInformationActor::ThresholdSysop {
            caller_id,
            sysop_security,
        } => {
            let effective: Option<u16> = transaction
                .query_row(
                    "SELECT MIN(c.security_level, COALESCE((SELECT MIN(target_security_level) FROM caller_security_adjustments WHERE caller_id=c.caller_id AND status='active'), c.security_level)) FROM callers c WHERE c.caller_id=?1 AND c.account_state='active'",
                    [caller_id.get()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(DatabaseError::Sqlite)?;
            if effective.is_some_and(|value| value >= sysop_security.get()) {
                Ok(())
            } else {
                Err(PublicInformationError::Unauthorized.into())
            }
        }
        PublicInformationActor::Caller(_) | PublicInformationActor::SystemPolicy => {
            Err(PublicInformationError::Unauthorized.into())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    transaction: &rusqlite::Transaction<'_>,
    occurred_at: i64,
    operation: &str,
    actor: PublicInformationActor,
    subject_caller_id: Option<CallerId>,
    other_bbs_id: Option<OtherBbsId>,
    prior_version: Option<u64>,
    new_version: Option<u64>,
    semantic_detail: Option<&str>,
) -> Result<(), DatabaseError> {
    let (kind, actor_id) = actor.database_values();
    transaction.execute("INSERT INTO public_information_events (occurred_at,operation,actor_kind,actor_caller_id,subject_caller_id,other_bbs_entry_id,prior_state_version,new_state_version,semantic_detail) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![occurred_at, operation, kind, actor_id, subject_caller_id.map(CallerId::get), other_bbs_id.map(OtherBbsId::get), prior_version.map(|v| v as i64), new_version.map(|v| v as i64), semantic_detail]).map_err(DatabaseError::Sqlite)?;
    Ok(())
}

fn validate_resource_kind(kind: &str) -> Result<(), PublicInformationError> {
    match kind {
        "bulletins" | "newsletter" | "thoughts" => Ok(()),
        _ => Err(PublicInformationError::InvalidResourceKind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardIdentity, CallerState, SecurityLevel};
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn database_path(temp: &tempfile::TempDir) -> PathBuf {
        temp.path().join("runtime.sqlite3")
    }

    fn test_database() -> (tempfile::TempDir, RuntimeDatabase, CallerId) {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        database
            .ensure_board_identity(&BoardIdentity::new("Public Test", "Fixture Sysop").unwrap())
            .unwrap();
        let caller = database
            .create_caller(
                b"PixelWizard",
                "synthetic-password-hash",
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1_700_000_000,
            )
            .unwrap();
        (temp, database, caller.id)
    }

    #[test]
    fn native_thought_catalog_is_bounded_and_deterministic() {
        let catalog = NativeThoughtCatalog::parse(b"First thought\nSecond thought\n").unwrap();
        assert_eq!(catalog.thoughts(), &["First thought", "Second thought"]);
        assert_eq!(catalog.select(0), Some("First thought"));
        assert_eq!(catalog.select(3), Some("Second thought"));
        assert!(NativeThoughtCatalog::parse(&vec![b'x'; MAX_NATIVE_THOUGHT_BYTES + 1]).is_err());
    }

    #[test]
    fn privacy_defaults_opt_in_projection_and_locate_are_authoritative() {
        let (_temp, mut database, caller_id) = test_database();
        let policy = database.public_directory_policy().unwrap();
        assert_eq!(
            policy,
            PublicDirectoryPolicy {
                enabled: false,
                show_last_call_date: false,
                show_city_region: false,
                caller_bbs_additions_enabled: false,
                state_version: 1
            }
        );
        assert_eq!(
            database.caller_publicity(caller_id).unwrap(),
            CallerPublicity {
                listed: false,
                state_version: 0
            }
        );
        assert!(database.public_caller_directory(0, 10).unwrap().is_empty());

        database
            .update_public_directory_policy(
                PublicInformationActor::LocalOperator,
                1,
                true,
                true,
                true,
                false,
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            database.update_caller_publicity(
                PublicInformationActor::LocalOperator,
                caller_id,
                0,
                true,
                1_700_000_001
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::Unauthorized
            ))
        ));
        assert!(matches!(
            database.update_public_directory_policy(
                PublicInformationActor::LocalOperator,
                1,
                false,
                false,
                false,
                false,
                1_700_000_001
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::PolicyConflict { .. }
            ))
        ));
        let publicity = database
            .update_caller_publicity(
                PublicInformationActor::Caller(caller_id),
                caller_id,
                0,
                true,
                1_700_000_002,
            )
            .unwrap();
        assert_eq!(
            publicity,
            CallerPublicity {
                listed: true,
                state_version: 1
            }
        );
        let rows = database.public_caller_directory(0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].handle, "PixelWizard");
        assert_eq!(database.locate_public_callers("wizard").unwrap(), rows);
        assert_eq!(database.locate_public_callers("PIXEL").unwrap(), rows);
        assert!(matches!(
            database.update_caller_publicity(
                PublicInformationActor::Caller(caller_id),
                caller_id,
                0,
                false,
                1_700_000_003
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::CallerPublicityConflict { .. }
            ))
        ));

        database
            .connection
            .execute(
                "UPDATE callers SET account_state='disabled' WHERE caller_id=?1",
                [caller_id.get()],
            )
            .unwrap();
        assert!(database.public_caller_directory(0, 10).unwrap().is_empty());
        assert!(database
            .locate_public_callers("pixelwizard")
            .unwrap()
            .is_empty());
        database
            .connection
            .execute(
                "UPDATE callers SET account_state='deleted' WHERE caller_id=?1",
                [caller_id.get()],
            )
            .unwrap();
        assert!(database.public_caller_directory(0, 10).unwrap().is_empty());
        assert!(database
            .locate_public_callers("pixelwizard")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn locate_is_ascii_substring_bounded_and_caps_fifty_deterministically() {
        let (_temp, mut database, first_id) = test_database();
        database
            .update_public_directory_policy(
                PublicInformationActor::LocalOperator,
                1,
                true,
                false,
                false,
                false,
                10,
            )
            .unwrap();
        database
            .update_caller_publicity(
                PublicInformationActor::Caller(first_id),
                first_id,
                0,
                true,
                11,
            )
            .unwrap();
        for number in 0..55 {
            let caller = database
                .create_caller(
                    format!("Match Caller {number:02}").as_bytes(),
                    "hash",
                    SecurityLevel::new(10).unwrap(),
                    CallerState::Active,
                    false,
                    20 + number,
                )
                .unwrap();
            database
                .update_caller_publicity(
                    PublicInformationActor::Caller(caller.id),
                    caller.id,
                    0,
                    true,
                    100 + number,
                )
                .unwrap();
        }
        let rows = database.locate_public_callers("match caller").unwrap();
        assert_eq!(rows.len(), MAX_DIRECTORY_RESULTS);
        assert_eq!(rows[0].handle, "Match Caller 00");
        assert_eq!(rows[49].handle, "Match Caller 49");
        let first_page = database.public_caller_directory(0, 10).unwrap();
        let second_page = database.public_caller_directory(10, 10).unwrap();
        assert_eq!(first_page.len(), 10);
        assert_eq!(second_page.len(), 10);
        assert!(first_page.last().unwrap().handle < second_page.first().unwrap().handle);
        assert!(database.locate_public_callers("é").is_err());
        assert!(database.locate_public_callers("").is_err());
    }

    #[test]
    fn other_bbs_crud_order_lifecycle_audit_and_conflicts_are_durable() {
        let (temp, mut first, _caller_id) = test_database();
        let one = first
            .add_other_bbs(
                PublicInformationActor::LocalOperator,
                NewOtherBbsEntry {
                    name: "Alpha Board".to_owned(),
                    speed: "56K".to_owned(),
                    dial_string: "alpha.example:23".to_owned(),
                },
                20,
            )
            .unwrap();
        let two = first
            .add_other_bbs(
                PublicInformationActor::LocalOperator,
                NewOtherBbsEntry {
                    name: "Beta Board".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "beta.example:2222".to_owned(),
                },
                21,
            )
            .unwrap();
        assert_eq!(
            first
                .other_bbs_entries(false)
                .unwrap()
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            vec![one.id, two.id]
        );
        let moved = first
            .reorder_other_bbs(
                PublicInformationActor::LocalOperator,
                two.id,
                two.state_version,
                1,
                22,
            )
            .unwrap();
        assert_eq!(moved.order, 1);
        let mut second = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        assert!(matches!(
            second.edit_other_bbs(
                PublicInformationActor::LocalOperator,
                two.id,
                two.state_version,
                NewOtherBbsEntry {
                    name: "Stale".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "stale.example".to_owned()
                },
                23
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::OtherBbsConflict { .. }
            ))
        ));
        let disabled = first
            .set_other_bbs_lifecycle(
                PublicInformationActor::LocalOperator,
                one.id,
                one.state_version,
                OtherBbsLifecycle::Disabled,
                24,
            )
            .unwrap();
        assert_eq!(disabled.lifecycle, OtherBbsLifecycle::Disabled);
        assert_eq!(first.other_bbs_entries(false).unwrap().len(), 1);
        let event_columns = first
            .connection
            .prepare("SELECT name FROM pragma_table_info('public_information_events') ORDER BY cid")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<HashSet<_>, _>>()
            .unwrap();
        assert!(!event_columns.contains("login_identifier"));
        assert!(!event_columns.contains("real_name"));
        assert!(!event_columns.contains("bbs_name"));
        first.validate_current_snapshot().unwrap();
    }

    #[test]
    fn caller_contributions_require_policy_and_preserve_stable_contributor_id() {
        let (_temp, mut database, caller_id) = test_database();
        let entry = NewOtherBbsEntry {
            name: "Caller Board".to_owned(),
            speed: "9600".to_owned(),
            dial_string: "555-0100".to_owned(),
        };
        assert!(matches!(
            database.add_other_bbs(PublicInformationActor::Caller(caller_id), entry.clone(), 30),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::CallerBbsAdditionsDisabled
            ))
        ));
        database
            .update_public_directory_policy(
                PublicInformationActor::LocalOperator,
                1,
                false,
                false,
                false,
                true,
                31,
            )
            .unwrap();
        let added = database
            .add_other_bbs(PublicInformationActor::Caller(caller_id), entry, 32)
            .unwrap();
        assert_eq!(added.contributor_caller_id, Some(caller_id));
    }

    #[test]
    fn threshold_operator_is_reauthorized_from_current_lifecycle_and_effective_security() {
        let (_temp, mut database, _caller_id) = test_database();
        let sysop = database
            .create_caller(
                b"Threshold Fixture",
                "hash",
                SecurityLevel::new(50).unwrap(),
                CallerState::Active,
                false,
                40,
            )
            .unwrap();
        let actor = PublicInformationActor::ThresholdSysop {
            caller_id: sysop.id,
            sysop_security: SecurityLevel::new(50).unwrap(),
        };
        let policy = database
            .update_public_directory_policy(actor, 1, true, false, false, false, 41)
            .unwrap();
        assert_eq!(policy.state_version, 2);
        database
            .connection
            .execute(
                "UPDATE callers SET account_state='disabled' WHERE caller_id=?1",
                [sysop.id.get()],
            )
            .unwrap();
        assert!(matches!(
            database.update_public_directory_policy(
                actor,
                policy.state_version,
                false,
                false,
                false,
                false,
                42
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::Unauthorized
            ))
        ));
        assert!(database.public_directory_policy().unwrap().enabled);
    }

    #[test]
    fn other_bbs_text_and_cardinality_bounds_fail_closed() {
        let (_temp, mut database, _caller_id) = test_database();
        for invalid in [
            NewOtherBbsEntry {
                name: String::new(),
                speed: "SSH".to_owned(),
                dial_string: "example".to_owned(),
            },
            NewOtherBbsEntry {
                name: "x".repeat(MAX_OTHER_BBS_NAME_BYTES + 1),
                speed: "SSH".to_owned(),
                dial_string: "example".to_owned(),
            },
            NewOtherBbsEntry {
                name: "Control\nName".to_owned(),
                speed: "SSH".to_owned(),
                dial_string: "example".to_owned(),
            },
        ] {
            assert!(matches!(
                database.add_other_bbs(PublicInformationActor::LocalOperator, invalid, 1),
                Err(DatabaseError::PublicInformation(
                    PublicInformationError::InvalidText { .. }
                ))
            ));
        }
        let transaction = database.connection.transaction().unwrap();
        for order in 1..=MAX_OTHER_BBS_ENTRIES {
            transaction.execute("INSERT INTO other_bbs_entries(bbs_name,speed_label,dial_string,display_order) VALUES(?1,'SSH',?2,?3)", params![format!("Board {order}"), format!("board-{order}.example"), order as i64]).unwrap();
        }
        transaction.commit().unwrap();
        assert!(matches!(
            database.add_other_bbs(
                PublicInformationActor::LocalOperator,
                NewOtherBbsEntry {
                    name: "Overflow".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "overflow.example".to_owned()
                },
                2
            ),
            Err(DatabaseError::PublicInformation(
                PublicInformationError::OtherBbsFull
            ))
        ));
        database.validate_current_snapshot().unwrap();
    }

    #[test]
    fn public_resource_generation_is_content_addressed_and_audited_once_per_change() {
        let (_temp, mut database, _caller_id) = test_database();
        let first_digest = "11".repeat(32);
        let second_digest = "22".repeat(32);
        let first = database
            .observe_public_resource("newsletter", &first_digest, 100)
            .unwrap();
        assert_eq!(first.generation, 1);
        assert_eq!(
            database
                .observe_public_resource("newsletter", &first_digest, 200)
                .unwrap(),
            first
        );
        let second = database
            .observe_public_resource("newsletter", &second_digest, 300)
            .unwrap();
        assert_eq!(second.generation, 2);
        assert_eq!(second.published_at, 300);
        let events: i64 = database.connection.query_row("SELECT COUNT(*) FROM public_information_events WHERE operation='resource-published' AND resource_kind='newsletter'", [], |row| row.get(0)).unwrap();
        assert_eq!(events, 2);
        let audited_digest: String = database.connection.query_row("SELECT resource_digest FROM public_information_events WHERE operation='resource-published' ORDER BY event_id DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        assert_eq!(audited_digest, second_digest);
        database.validate_current_snapshot().unwrap();
    }
}
