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

//! Privacy-bounded operational observability authority.
//!
//! This module deliberately keeps operational events separate from security
//! audit, report rendering, publication artifacts, and host diagnostic logs.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use chrono::{Datelike, Days, Utc};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{board_local_day, CallerId, DatabaseError, RuntimeDatabase};

pub const DEFAULT_EVENT_PAGE_SIZE: usize = 100;
pub const MAX_EVENT_PAGE_SIZE: usize = 500;
pub const MAX_EVENT_QUERY_DAYS: i64 = 31;
pub const MAX_LIVE_EVENTS: usize = 2_048;
pub const MAX_LIVE_SUBSCRIBER_EVENTS: usize = 256;
pub const LIVE_EVENT_HORIZON_SECONDS: i64 = 15 * 60;
pub const RETENTION_CLEANUP_BATCH: usize = 500;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventId(u64);

impl EventId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NotificationId(u64);

impl NotificationId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

macro_rules! database_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }

            fn parse(value: &str) -> rusqlite::Result<Self> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(rusqlite::Error::InvalidQuery),
                }
            }
        }
    };
}

database_enum!(EventCategory {
    System => "system", Node => "node", Session => "session", Caller => "caller",
    Authentication => "authentication", Message => "message", File => "file",
    Transfer => "transfer", Storage => "storage", Backup => "backup",
    Operator => "operator", Error => "error"
});
database_enum!(EventSeverity {
    Info => "info", Notice => "notice", Warning => "warning", Error => "error",
    Critical => "critical"
});
database_enum!(EventOutcome {
    Succeeded => "succeeded", Failed => "failed", Cancelled => "cancelled",
    Denied => "denied", Unavailable => "unavailable", Observed => "observed"
});
database_enum!(RetentionClass {
    Operational => "operational", SummarySource => "summary-source"
});

impl EventCategory {
    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::System => "operator-event-category-system",
            Self::Node => "operator-event-category-node",
            Self::Session => "operator-event-category-session",
            Self::Caller => "operator-event-category-caller",
            Self::Authentication => "operator-event-category-authentication",
            Self::Message => "operator-event-category-message",
            Self::File => "operator-event-category-file",
            Self::Transfer => "operator-event-category-transfer",
            Self::Storage => "operator-event-category-storage",
            Self::Backup => "operator-event-category-backup",
            Self::Operator => "operator-event-category-operator",
            Self::Error => "operator-event-category-error",
        }
    }
}

impl EventSeverity {
    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::Info => "operator-event-severity-info",
            Self::Notice => "operator-event-severity-notice",
            Self::Warning => "operator-event-severity-warning",
            Self::Error => "operator-event-severity-error",
            Self::Critical => "operator-event-severity-critical",
        }
    }
}

impl EventOutcome {
    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::Succeeded => "operator-event-outcome-succeeded",
            Self::Failed => "operator-event-outcome-failed",
            Self::Cancelled => "operator-event-outcome-cancelled",
            Self::Denied => "operator-event-outcome-denied",
            Self::Unavailable => "operator-event-outcome-unavailable",
            Self::Observed => "operator-event-outcome-observed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventAttributes {
    None,
    Session {
        public_handle: Option<String>,
        transport: Option<String>,
        duration_seconds: Option<u64>,
        close_reason: Option<String>,
    },
    Transfer {
        protocol: Option<String>,
        direction: Option<String>,
        bytes: Option<u64>,
        files: Option<u64>,
    },
    Message {
        conference_id: u64,
        visibility: String,
        count: u64,
    },
    File {
        operation: String,
        bytes: Option<u64>,
    },
    Storage {
        state: String,
    },
    Backup {
        state: String,
        bytes: Option<u64>,
    },
    Error {
        subsystem: String,
        reason_key: String,
    },
    Operator {
        action: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOperationalEvent {
    pub occurred_at_utc: i64,
    pub category: EventCategory,
    pub severity: EventSeverity,
    pub event_code: String,
    pub outcome: EventOutcome,
    pub node_id: Option<u32>,
    pub session_id: Option<u64>,
    pub caller_id: Option<CallerId>,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub retention_class: RetentionClass,
    pub attributes: EventAttributes,
}

impl NewOperationalEvent {
    pub fn new(
        occurred_at_utc: i64,
        category: EventCategory,
        severity: EventSeverity,
        event_code: impl Into<String>,
        outcome: EventOutcome,
    ) -> Self {
        Self {
            occurred_at_utc,
            category,
            severity,
            event_code: event_code.into(),
            outcome,
            node_id: None,
            session_id: None,
            caller_id: None,
            correlation_id: None,
            idempotency_key: None,
            object_kind: None,
            object_id: None,
            retention_class: RetentionClass::Operational,
            attributes: EventAttributes::None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalEvent {
    pub id: EventId,
    pub occurred_at_utc: i64,
    pub board_day: i32,
    pub timezone_policy_version: u64,
    pub category: EventCategory,
    pub severity: EventSeverity,
    pub event_code: String,
    pub outcome: EventOutcome,
    pub node_id: Option<u32>,
    pub session_id: Option<u64>,
    pub caller_id: Option<CallerId>,
    pub correlation_id: Option<String>,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub retention_class: RetentionClass,
    pub attributes: EventAttributes,
}

#[derive(Serialize)]
struct OperationalEventJsonV1<'a> {
    schema: &'static str,
    event_id: u64,
    occurred_at_utc: i64,
    board_day: i32,
    timezone_policy_version: u64,
    category: &'static str,
    severity: &'static str,
    event_code: &'a str,
    outcome: &'static str,
    node_id: Option<u32>,
    session_id: Option<u64>,
    caller_id: Option<i64>,
    correlation_id: Option<&'a str>,
    object_kind: Option<&'a str>,
    object_id: Option<&'a str>,
    retention_class: &'static str,
    attribute_version: u8,
    attributes: &'a EventAttributes,
}

impl OperationalEvent {
    /// Encodes one bounded, versioned machine record. This does not write a
    /// file or create a report/publication artifact; B-022 owns those actions.
    pub fn to_json_line(&self) -> Result<String, DatabaseError> {
        let record = OperationalEventJsonV1 {
            schema: "spitfire-operational-event/v1",
            event_id: self.id.get(),
            occurred_at_utc: self.occurred_at_utc,
            board_day: self.board_day,
            timezone_policy_version: self.timezone_policy_version,
            category: self.category.as_str(),
            severity: self.severity.as_str(),
            event_code: &self.event_code,
            outcome: self.outcome.as_str(),
            node_id: self.node_id,
            session_id: self.session_id,
            caller_id: self.caller_id.map(CallerId::get),
            correlation_id: self.correlation_id.as_deref(),
            object_kind: self.object_kind.as_deref(),
            object_id: self.object_id.as_deref(),
            retention_class: self.retention_class.as_str(),
            attribute_version: 1,
            attributes: &self.attributes,
        };
        let mut line = serde_json::to_string(&record).map_err(|error| {
            DatabaseError::IntegrityCheck(format!(
                "operational event JSON encoding failed: {error}"
            ))
        })?;
        line.push('\n');
        Ok(line)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventCursor {
    pub occurred_at_utc: i64,
    pub event_id: EventId,
    pub snapshot_event_id: EventId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventQuery {
    pub from_utc: Option<i64>,
    pub through_utc: Option<i64>,
    pub category: Option<EventCategory>,
    pub minimum_severity: Option<EventSeverity>,
    pub outcome: Option<EventOutcome>,
    pub node_id: Option<u32>,
    pub caller_id: Option<CallerId>,
    pub cursor: Option<EventCursor>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventPage {
    pub events: Vec<OperationalEvent>,
    pub next_cursor: Option<EventCursor>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DailyOperationalSummary {
    pub board_day: i32,
    pub timezone_policy_version: u64,
    pub high_water_event_id: u64,
    pub calls_started: u64,
    pub calls_completed: u64,
    pub new_callers: u64,
    pub messages_posted: u64,
    pub successful_uploads: u64,
    pub upload_bytes: u64,
    pub successful_downloads: u64,
    pub download_bytes: u64,
    pub failed_transfers: u64,
    pub cancelled_transfers: u64,
    pub warning_events: u64,
    pub error_events: u64,
    pub critical_events: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    pub detail_days: u16,
    pub summary_days: u16,
    pub state_version: u64,
    pub activated_at: i64,
    pub last_cleanup_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationState {
    Open,
    Acknowledged,
    Resolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorNotification {
    pub id: NotificationId,
    pub source_event_id: EventId,
    pub created_at: i64,
    pub category: EventCategory,
    pub severity: EventSeverity,
    pub reason_key: String,
    pub remediation_key: Option<String>,
    pub state: NotificationState,
    pub state_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorPrincipalKind {
    HostOperator,
    NamedSysop,
    System,
}

impl OperatorPrincipalKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HostOperator => "host-operator",
            Self::NamedSysop => "named-sysop",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorPrincipal {
    pub kind: OperatorPrincipalKind,
    pub stable_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionCleanupResult {
    pub notification_rows_deleted: usize,
    pub event_rows_deleted: usize,
    pub summary_rows_deleted: usize,
    pub more_work: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionImpact {
    pub events_before_cutoff: u64,
    pub summaries_before_cutoff: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemStatistics {
    pub observability_activated_at: i64,
    pub today: DailyOperationalSummary,
    /// Existing caller authority counts accepted call starts, not inferred
    /// pre-schema-18 completion events.
    pub lifetime_calls: u64,
    pub lifetime_messages_posted: u64,
    pub lifetime_files_uploaded: u64,
    pub lifetime_upload_bytes: u64,
    pub lifetime_files_downloaded: u64,
    pub lifetime_download_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentCaller {
    pub event_id: EventId,
    pub public_handle: String,
    pub occurred_at_utc: i64,
    pub board_day: i32,
    pub transport: Option<String>,
    pub duration_seconds: u64,
    pub close_reason: Option<String>,
    pub node_id: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerActivity {
    pub caller_id: CallerId,
    pub public_handle: String,
    pub lifetime_calls: u64,
    pub lifetime_messages_posted: u64,
    pub lifetime_files_uploaded: u64,
    pub lifetime_upload_bytes: u64,
    pub lifetime_files_downloaded: u64,
    pub lifetime_download_bytes: u64,
    pub recent_events: EventPage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageActivity {
    pub conference_id: u64,
    pub visibility: String,
    pub messages_posted: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageActivityPage {
    pub rows: Vec<MessageActivity>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferActivity {
    pub protocol: Option<String>,
    pub direction: Option<String>,
    pub outcome: EventOutcome,
    pub transfers: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferActivityPage {
    pub rows: Vec<TransferActivity>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaintenanceStatus {
    pub open_notifications: u64,
    pub recent_warning_events: u64,
    pub recent_error_events: u64,
    pub unavailable_storage_roots: u64,
    pub pending_review_files: u64,
    pub nonterminal_transfers: u64,
    pub retention: RetentionPolicy,
}

impl RuntimeDatabase {
    pub fn record_authentication_failure(
        &mut self,
        occurred_at_utc: i64,
        node_id: u32,
        session_id: u64,
        transport: &str,
        reason_class: &str,
    ) -> Result<OperationalEvent, DatabaseError> {
        let mut event = NewOperationalEvent::new(
            occurred_at_utc,
            EventCategory::Authentication,
            EventSeverity::Notice,
            "authentication.failed",
            EventOutcome::Denied,
        );
        event.node_id = Some(node_id);
        event.session_id = Some(session_id);
        event.correlation_id = Some(format!("session-{session_id}"));
        event.attributes = EventAttributes::Session {
            public_handle: None,
            transport: Some(transport.to_owned()),
            duration_seconds: None,
            close_reason: Some(reason_class.to_owned()),
        };
        self.record_operational_event(&event)
    }

    pub fn record_operational_event(
        &mut self,
        event: &NewOperationalEvent,
    ) -> Result<OperationalEvent, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let stored = insert_operational_event_tx(&transaction, event)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(stored)
    }

    pub fn query_operational_events(&self, query: &EventQuery) -> Result<EventPage, DatabaseError> {
        validate_query(query)?;
        let limit = query
            .limit
            .unwrap_or(DEFAULT_EVENT_PAGE_SIZE)
            .min(MAX_EVENT_PAGE_SIZE);
        let from = query.from_utc.unwrap_or(0);
        let through = query.through_utc.unwrap_or(i64::MAX);
        let cursor_time = query
            .cursor
            .map(|item| item.occurred_at_utc)
            .unwrap_or(i64::MAX);
        let cursor_id = query
            .cursor
            .map(|item| item.event_id.get())
            .unwrap_or(i64::MAX as u64);
        let snapshot_event_id = match query.cursor {
            Some(cursor) => cursor.snapshot_event_id,
            None => EventId(
                nonnegative(
                    self.connection
                        .query_row(
                            "SELECT COALESCE(MAX(event_id),0) FROM operational_events",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(DatabaseError::Sqlite)?,
                )
                .map_err(DatabaseError::Sqlite)?,
            ),
        };
        let category = query.category.map(EventCategory::as_str);
        let severity_rank = query.minimum_severity.map(severity_rank).unwrap_or(0);
        let outcome = query.outcome.map(EventOutcome::as_str);
        let mut statement = self.connection.prepare(
            r#"
            SELECT event_id,occurred_at_utc,board_day,timezone_policy_version,
                   category,severity,event_code,outcome,node_id,session_id,caller_id,
                   correlation_id,object_kind,object_id,retention_class,
                   attribute_kind,text_value_1,text_value_2,text_value_3,number_value_1,number_value_2
              FROM operational_events
             WHERE occurred_at_utc BETWEEN ?1 AND ?2
               AND (?3 IS NULL OR category=?3)
               AND CASE severity WHEN 'info' THEN 1 WHEN 'notice' THEN 2
                   WHEN 'warning' THEN 3 WHEN 'error' THEN 4 ELSE 5 END >= ?4
               AND (?5 IS NULL OR outcome=?5)
               AND (?6 IS NULL OR node_id=?6)
               AND (?7 IS NULL OR caller_id=?7)
               AND (occurred_at_utc < ?8 OR (occurred_at_utc=?8 AND event_id<?9))
               AND event_id<=?10
             ORDER BY occurred_at_utc DESC,event_id DESC
             LIMIT ?11
            "#,
        ).map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map(
                params![
                    from,
                    through,
                    category,
                    severity_rank,
                    outcome,
                    query.node_id.map(i64::from),
                    query.caller_id.map(CallerId::get),
                    cursor_time,
                    sqlite_u64(cursor_id)?,
                    sqlite_u64(snapshot_event_id.get())?,
                    i64::try_from(limit + 1).unwrap_or(501)
                ],
                decode_event,
            )
            .map_err(DatabaseError::Sqlite)?;
        let mut events = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        let has_more = events.len() > limit;
        events.truncate(limit);
        let next_cursor = has_more
            .then(|| {
                events.last().map(|item| EventCursor {
                    occurred_at_utc: item.occurred_at_utc,
                    event_id: item.id,
                    snapshot_event_id,
                })
            })
            .flatten();
        Ok(EventPage {
            events,
            next_cursor,
        })
    }

    pub fn daily_operational_summary(
        &self,
        board_day: i32,
        timezone_policy_version: u64,
    ) -> Result<Option<DailyOperationalSummary>, DatabaseError> {
        self.connection.query_row(
            "SELECT board_day,timezone_policy_version,high_water_event_id,calls_started,calls_completed,new_callers,messages_posted,successful_uploads,upload_bytes,successful_downloads,download_bytes,failed_transfers,cancelled_transfers,warning_events,error_events,critical_events FROM operational_daily_summaries WHERE board_day=?1 AND timezone_policy_version=?2",
            params![board_day, sqlite_u64(timezone_policy_version)?],
            |row| Ok(DailyOperationalSummary {
                board_day: row.get(0)?, timezone_policy_version: nonnegative(row.get(1)?)?,
                high_water_event_id: nonnegative(row.get(2)?)?, calls_started: nonnegative(row.get(3)?)?,
                calls_completed: nonnegative(row.get(4)?)?, new_callers: nonnegative(row.get(5)?)?,
                messages_posted: nonnegative(row.get(6)?)?, successful_uploads: nonnegative(row.get(7)?)?,
                upload_bytes: nonnegative(row.get(8)?)?, successful_downloads: nonnegative(row.get(9)?)?,
                download_bytes: nonnegative(row.get(10)?)?, failed_transfers: nonnegative(row.get(11)?)?,
                cancelled_transfers: nonnegative(row.get(12)?)?, warning_events: nonnegative(row.get(13)?)?,
                error_events: nonnegative(row.get(14)?)?, critical_events: nonnegative(row.get(15)?)?,
            }),
        ).optional().map_err(DatabaseError::Sqlite)
    }

    pub fn retention_policy(&self) -> Result<RetentionPolicy, DatabaseError> {
        self.connection.query_row(
            "SELECT detail_days,summary_days,state_version,activated_at,last_cleanup_at FROM operational_retention_policy WHERE singleton=1",
            [],
            |row| Ok(RetentionPolicy { detail_days: row.get(0)?, summary_days: row.get(1)?, state_version: nonnegative(row.get(2)?)?, activated_at: row.get(3)?, last_cleanup_at: row.get(4)? }),
        ).map_err(DatabaseError::Sqlite)
    }

    pub fn cleanup_operational_retention(
        &mut self,
        now_utc: i64,
    ) -> Result<RetentionCleanupResult, DatabaseError> {
        let policy = self.retention_policy()?;
        let event_cutoff = now_utc.saturating_sub(i64::from(policy.detail_days) * 86_400);
        let summary_cutoff = self.summary_cutoff(now_utc, policy.summary_days)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let notification_rows_deleted = transaction.execute(
            "DELETE FROM operator_notifications WHERE notification_id IN (SELECT notification_id FROM operator_notifications WHERE state<>'open' AND created_at<?1 ORDER BY created_at,notification_id LIMIT ?2)",
            params![event_cutoff, RETENTION_CLEANUP_BATCH as i64],
        ).map_err(DatabaseError::Sqlite)?;
        let event_rows_deleted = transaction.execute(
            "DELETE FROM operational_events WHERE event_id IN (SELECT e.event_id FROM operational_events e LEFT JOIN operator_notifications n ON n.source_event_id=e.event_id WHERE e.occurred_at_utc<?1 AND n.notification_id IS NULL ORDER BY e.occurred_at_utc,e.event_id LIMIT ?2)",
            params![event_cutoff, RETENTION_CLEANUP_BATCH as i64],
        ).map_err(DatabaseError::Sqlite)?;
        let summary_rows_deleted = transaction.execute(
            "DELETE FROM operational_daily_summaries WHERE rowid IN (SELECT rowid FROM operational_daily_summaries WHERE board_day<?1 ORDER BY board_day,timezone_policy_version LIMIT ?2)",
            params![summary_cutoff, RETENTION_CLEANUP_BATCH as i64],
        ).map_err(DatabaseError::Sqlite)?;
        transaction
            .execute(
                "UPDATE operational_retention_policy SET last_cleanup_at=?1 WHERE singleton=1",
                params![now_utc],
            )
            .map_err(DatabaseError::Sqlite)?;
        transaction.execute(
            "INSERT INTO operator_observability_audit(occurred_at,action,actor_kind,target_kind,target_id,outcome) VALUES(?1,'retention-cleanup','system','retention-policy','1','succeeded')",
            params![now_utc],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(RetentionCleanupResult {
            notification_rows_deleted,
            event_rows_deleted,
            summary_rows_deleted,
            more_work: notification_rows_deleted == RETENTION_CLEANUP_BATCH
                || event_rows_deleted == RETENTION_CLEANUP_BATCH
                || summary_rows_deleted == RETENTION_CLEANUP_BATCH,
        })
    }

    pub fn retention_impact(
        &self,
        detail_days: u16,
        summary_days: u16,
        now_utc: i64,
    ) -> Result<RetentionImpact, DatabaseError> {
        validate_retention(detail_days, summary_days)?;
        let event_cutoff = now_utc.saturating_sub(i64::from(detail_days) * 86_400);
        let summary_cutoff = self.summary_cutoff(now_utc, summary_days)?;
        let counts: (i64, i64) = self
            .connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM operational_events e LEFT JOIN operator_notifications n ON n.source_event_id=e.event_id WHERE e.occurred_at_utc<?1 AND n.notification_id IS NULL),(SELECT COUNT(*) FROM operational_daily_summaries WHERE board_day<?2)",
                params![event_cutoff, summary_cutoff],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(DatabaseError::Sqlite)?;
        Ok(RetentionImpact {
            events_before_cutoff: nonnegative(counts.0).map_err(DatabaseError::Sqlite)?,
            summaries_before_cutoff: nonnegative(counts.1).map_err(DatabaseError::Sqlite)?,
        })
    }

    pub fn update_retention_policy(
        &mut self,
        expected_version: u64,
        detail_days: u16,
        summary_days: u16,
        confirmed_impact: Option<RetentionImpact>,
        actor: &OperatorPrincipal,
        now_utc: i64,
    ) -> Result<Option<RetentionPolicy>, DatabaseError> {
        validate_retention(detail_days, summary_days)?;
        validate_optional_text(actor.stable_id.as_deref(), 64, "operator stable ID")?;
        let current = self.retention_policy()?;
        if detail_days < current.detail_days || summary_days < current.summary_days {
            let actual = self.retention_impact(detail_days, summary_days, now_utc)?;
            if confirmed_impact != Some(actual) {
                return Err(DatabaseError::IntegrityCheck(
                    "shorter observability retention requires a current impact confirmation"
                        .to_owned(),
                ));
            }
        }
        let next = expected_version.saturating_add(1);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let changed = transaction.execute(
            "UPDATE operational_retention_policy SET detail_days=?2,summary_days=?3,state_version=?4 WHERE singleton=1 AND state_version=?1",
            params![sqlite_u64(expected_version)?,detail_days,summary_days,sqlite_u64(next)?],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.execute(
            "INSERT INTO operator_observability_audit(occurred_at,action,actor_kind,actor_id,target_kind,target_id,outcome,prior_version,next_version) VALUES(?1,'retention-policy-changed',?2,?3,'retention-policy','1',?4,?5,?6)",
            params![now_utc,actor.kind.as_str(),actor.stable_id,if changed==1 {"succeeded"} else {"conflict"},sqlite_u64(expected_version)?,if changed==1 {Some(sqlite_u64(next)?)} else {None}],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        if changed == 1 {
            self.retention_policy().map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn rebuild_daily_operational_summary(
        &mut self,
        board_day: i32,
        timezone_policy_version: u64,
        now_utc: i64,
    ) -> Result<DailyOperationalSummary, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        transaction.execute(
            r#"
            INSERT INTO operational_daily_summaries(
                board_day,timezone_policy_version,high_water_event_id,calls_started,calls_completed,
                new_callers,messages_posted,successful_uploads,upload_bytes,successful_downloads,
                download_bytes,failed_transfers,cancelled_transfers,warning_events,error_events,
                critical_events,updated_at
            )
            SELECT ?1,?2,COALESCE(MAX(event_id),0),
                   COALESCE(SUM(event_code='session.started'),0),
                   COALESCE(SUM(event_code='session.completed'),0),
                   COALESCE(SUM(event_code='caller.created'),0),
                   COALESCE(SUM(CASE WHEN event_code='message.posted' THEN COALESCE(number_value_1,1) ELSE 0 END),0),
                   COALESCE(SUM(event_code='transfer.upload.completed'),0),
                   COALESCE(SUM(CASE WHEN event_code='transfer.upload.completed' THEN COALESCE(number_value_1,0) ELSE 0 END),0),
                   COALESCE(SUM(event_code='transfer.download.completed'),0),
                   COALESCE(SUM(CASE WHEN event_code='transfer.download.completed' THEN COALESCE(number_value_1,0) ELSE 0 END),0),
                   COALESCE(SUM(event_code='transfer.failed'),0),
                   COALESCE(SUM(event_code='transfer.cancelled'),0),
                   COALESCE(SUM(severity='warning'),0),
                   COALESCE(SUM(severity='error'),0),
                   COALESCE(SUM(severity='critical'),0),?3
              FROM operational_events
             WHERE board_day=?1 AND timezone_policy_version=?2
            ON CONFLICT(board_day,timezone_policy_version) DO UPDATE SET
                high_water_event_id=excluded.high_water_event_id,
                calls_started=excluded.calls_started,calls_completed=excluded.calls_completed,
                new_callers=excluded.new_callers,messages_posted=excluded.messages_posted,
                successful_uploads=excluded.successful_uploads,upload_bytes=excluded.upload_bytes,
                successful_downloads=excluded.successful_downloads,download_bytes=excluded.download_bytes,
                failed_transfers=excluded.failed_transfers,cancelled_transfers=excluded.cancelled_transfers,
                warning_events=excluded.warning_events,error_events=excluded.error_events,
                critical_events=excluded.critical_events,state_version=state_version+1,
                updated_at=excluded.updated_at
            "#,
            params![board_day,sqlite_u64(timezone_policy_version)?,now_utc],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.daily_operational_summary(board_day, timezone_policy_version)?
            .ok_or_else(|| {
                DatabaseError::IntegrityCheck(
                    "daily observability summary was not created".to_owned(),
                )
            })
    }

    pub fn notifications(
        &self,
        include_closed: bool,
        limit: usize,
    ) -> Result<Vec<OperatorNotification>, DatabaseError> {
        let limit = limit.clamp(1, MAX_EVENT_PAGE_SIZE);
        let mut statement = self.connection.prepare(
            "SELECT notification_id,source_event_id,created_at,category,severity,reason_key,remediation_key,state,state_version FROM operator_notifications WHERE (?1=1 OR state='open') ORDER BY created_at DESC,notification_id DESC LIMIT ?2",
        ).map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map(params![include_closed, limit as i64], |row| {
                let state: String = row.get(7)?;
                Ok(OperatorNotification {
                    id: NotificationId(nonnegative(row.get(0)?)?),
                    source_event_id: EventId(nonnegative(row.get(1)?)?),
                    created_at: row.get(2)?,
                    category: EventCategory::parse(&row.get::<_, String>(3)?)?,
                    severity: EventSeverity::parse(&row.get::<_, String>(4)?)?,
                    reason_key: row.get(5)?,
                    remediation_key: row.get(6)?,
                    state: match state.as_str() {
                        "open" => NotificationState::Open,
                        "acknowledged" => NotificationState::Acknowledged,
                        "resolved" => NotificationState::Resolved,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    state_version: nonnegative(row.get(8)?)?,
                })
            })
            .map_err(DatabaseError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)
    }

    pub fn acknowledge_notification(
        &mut self,
        notification_id: NotificationId,
        expected_version: u64,
        actor: &OperatorPrincipal,
        now_utc: i64,
    ) -> Result<bool, DatabaseError> {
        validate_optional_text(actor.stable_id.as_deref(), 64, "operator stable ID")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let changed = transaction.execute(
            "UPDATE operator_notifications SET state='acknowledged',state_version=state_version+1,acknowledged_at=?3,acknowledged_by=?4 WHERE notification_id=?1 AND state='open' AND state_version=?2",
            params![sqlite_u64(notification_id.get())?,sqlite_u64(expected_version)?,now_utc,actor.stable_id],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.execute(
            "INSERT INTO operator_observability_audit(occurred_at,action,actor_kind,actor_id,target_kind,target_id,outcome,prior_version,next_version) VALUES(?1,'notification-acknowledged',?2,?3,'notification',?4,?5,?6,?7)",
            params![now_utc,actor.kind.as_str(),actor.stable_id,notification_id.get().to_string(),if changed==1 {"succeeded"} else {"conflict"},sqlite_u64(expected_version)?,if changed==1 { Some(sqlite_u64(expected_version.saturating_add(1))?) } else { None }],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(changed == 1)
    }

    pub fn resolve_notification(
        &mut self,
        notification_id: NotificationId,
        expected_version: u64,
        now_utc: i64,
    ) -> Result<bool, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let changed = transaction.execute(
            "UPDATE operator_notifications SET state='resolved',state_version=state_version+1,resolved_at=?3 WHERE notification_id=?1 AND state<>'resolved' AND state_version=?2",
            params![sqlite_u64(notification_id.get())?,sqlite_u64(expected_version)?,now_utc],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.execute(
            "INSERT INTO operator_observability_audit(occurred_at,action,actor_kind,target_kind,target_id,outcome,prior_version,next_version) VALUES(?1,'notification-resolved','system','notification',?2,?3,?4,?5)",
            params![now_utc,notification_id.get().to_string(),if changed==1 {"succeeded"} else {"conflict"},sqlite_u64(expected_version)?,if changed==1 {Some(sqlite_u64(expected_version.saturating_add(1))?)} else {None}],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(changed == 1)
    }

    pub fn system_statistics(&self, now_utc: i64) -> Result<SystemStatistics, DatabaseError> {
        let (timezone_name, timezone_version): (String, i64) = self
            .connection
            .query_row(
                "SELECT timezone_name,state_version FROM transfer_timezone_policy WHERE singleton=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(DatabaseError::Sqlite)?;
        let timezone = timezone_name.parse::<chrono_tz::Tz>().map_err(|_| {
            DatabaseError::IntegrityCheck("stored board timezone is invalid".to_owned())
        })?;
        let board_day = board_local_day(now_utc, timezone)?;
        let today = self
            .daily_operational_summary(
                board_day,
                nonnegative(timezone_version).map_err(DatabaseError::Sqlite)?,
            )?
            .unwrap_or(DailyOperationalSummary {
                board_day,
                timezone_policy_version: nonnegative(timezone_version)
                    .map_err(DatabaseError::Sqlite)?,
                ..DailyOperationalSummary::default()
            });
        let totals: (i64, i64, i64, i64, i64, i64) = self.connection.query_row(
            "SELECT COALESCE(SUM(call_count),0),COALESCE(SUM(messages_posted),0),COALESCE(SUM(files_uploaded),0),COALESCE(SUM(upload_bytes),0),COALESCE(SUM(files_downloaded),0),COALESCE(SUM(download_bytes),0) FROM callers WHERE account_state<>'deleted'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        ).map_err(DatabaseError::Sqlite)?;
        let activation = self.retention_policy()?.activated_at;
        Ok(SystemStatistics {
            observability_activated_at: activation,
            today,
            lifetime_calls: nonnegative(totals.0).map_err(DatabaseError::Sqlite)?,
            lifetime_messages_posted: nonnegative(totals.1).map_err(DatabaseError::Sqlite)?,
            lifetime_files_uploaded: nonnegative(totals.2).map_err(DatabaseError::Sqlite)?,
            lifetime_upload_bytes: nonnegative(totals.3).map_err(DatabaseError::Sqlite)?,
            lifetime_files_downloaded: nonnegative(totals.4).map_err(DatabaseError::Sqlite)?,
            lifetime_download_bytes: nonnegative(totals.5).map_err(DatabaseError::Sqlite)?,
        })
    }

    pub fn recent_callers(&self, limit: usize) -> Result<Vec<RecentCaller>, DatabaseError> {
        let limit = limit.clamp(1, MAX_EVENT_PAGE_SIZE);
        let page = self.query_operational_events(&EventQuery {
            category: Some(EventCategory::Session),
            limit: Some(MAX_EVENT_PAGE_SIZE),
            ..EventQuery::default()
        })?;
        Ok(page
            .events
            .into_iter()
            .filter_map(|event| {
                if event.event_code != "session.completed" {
                    return None;
                }
                let EventAttributes::Session {
                    public_handle: Some(public_handle),
                    transport,
                    duration_seconds,
                    close_reason,
                } = event.attributes
                else {
                    return None;
                };
                Some(RecentCaller {
                    event_id: event.id,
                    public_handle,
                    occurred_at_utc: event.occurred_at_utc,
                    board_day: event.board_day,
                    transport,
                    duration_seconds: duration_seconds.unwrap_or(0),
                    close_reason,
                    node_id: event.node_id,
                })
            })
            .take(limit)
            .collect())
    }

    pub fn caller_activity(
        &self,
        caller_id: CallerId,
        query: &EventQuery,
    ) -> Result<Option<CallerActivity>, DatabaseError> {
        let Some(caller) = self.caller_by_id(caller_id)? else {
            return Ok(None);
        };
        let mut query = query.clone();
        query.caller_id = Some(caller_id);
        Ok(Some(CallerActivity {
            caller_id,
            public_handle: caller.display_name,
            lifetime_calls: caller.call_count,
            lifetime_messages_posted: caller.messages_posted,
            lifetime_files_uploaded: caller.files_uploaded,
            lifetime_upload_bytes: caller.upload_bytes,
            lifetime_files_downloaded: caller.files_downloaded,
            lifetime_download_bytes: caller.download_bytes,
            recent_events: self.query_operational_events(&query)?,
        }))
    }

    pub fn message_activity(
        &self,
        from_utc: i64,
        through_utc: i64,
    ) -> Result<MessageActivityPage, DatabaseError> {
        validate_time_range(from_utc, through_utc)?;
        let mut statement = self.connection.prepare(
            "SELECT number_value_2,text_value_1,SUM(number_value_1) FROM operational_events WHERE event_code='message.posted' AND occurred_at_utc BETWEEN ?1 AND ?2 GROUP BY number_value_2,text_value_1 ORDER BY number_value_2,text_value_1 LIMIT 501",
        ).map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map(params![from_utc, through_utc], |row| {
                Ok(MessageActivity {
                    conference_id: nonnegative(row.get(0)?)?,
                    visibility: row.get(1)?,
                    messages_posted: nonnegative(row.get(2)?)?,
                })
            })
            .map_err(DatabaseError::Sqlite)?;
        let mut rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        let truncated = rows.len() > MAX_EVENT_PAGE_SIZE;
        rows.truncate(MAX_EVENT_PAGE_SIZE);
        Ok(MessageActivityPage { rows, truncated })
    }

    pub fn file_activity(&self, query: &EventQuery) -> Result<EventPage, DatabaseError> {
        let mut query = query.clone();
        query.category = Some(EventCategory::File);
        self.query_operational_events(&query)
    }

    pub fn transfer_activity(
        &self,
        from_utc: i64,
        through_utc: i64,
    ) -> Result<TransferActivityPage, DatabaseError> {
        validate_time_range(from_utc, through_utc)?;
        let mut statement = self.connection.prepare(
            "SELECT text_value_1,text_value_2,outcome,COUNT(*),SUM(COALESCE(number_value_1,0)) FROM operational_events WHERE category='transfer' AND attribute_kind='transfer' AND event_code<>'transfer.completed' AND occurred_at_utc BETWEEN ?1 AND ?2 GROUP BY text_value_1,text_value_2,outcome ORDER BY text_value_1,text_value_2,outcome LIMIT 501",
        ).map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map(params![from_utc, through_utc], |row| {
                Ok(TransferActivity {
                    protocol: row.get(0)?,
                    direction: row.get(1)?,
                    outcome: EventOutcome::parse(&row.get::<_, String>(2)?)?,
                    transfers: nonnegative(row.get(3)?)?,
                    bytes: nonnegative(row.get(4)?)?,
                })
            })
            .map_err(DatabaseError::Sqlite)?;
        let mut rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        let truncated = rows.len() > MAX_EVENT_PAGE_SIZE;
        rows.truncate(MAX_EVENT_PAGE_SIZE);
        Ok(TransferActivityPage { rows, truncated })
    }

    pub fn recent_errors(&self, query: &EventQuery) -> Result<EventPage, DatabaseError> {
        let mut query = query.clone();
        query.minimum_severity = Some(EventSeverity::Error);
        self.query_operational_events(&query)
    }

    pub fn maintenance_status(&self, now_utc: i64) -> Result<MaintenanceStatus, DatabaseError> {
        let since = now_utc.saturating_sub(86_400);
        let counts: (i64, i64, i64, i64, i64, i64) = self.connection.query_row(
            "SELECT (SELECT COUNT(*) FROM operator_notifications WHERE state='open'),(SELECT COUNT(*) FROM operational_events WHERE occurred_at_utc>=?1 AND severity='warning'),(SELECT COUNT(*) FROM operational_events WHERE occurred_at_utc>=?1 AND severity IN ('error','critical')),(SELECT COUNT(*) FROM file_storage_roots WHERE configured_state='enabled' AND availability='unavailable'),(SELECT COUNT(*) FROM files WHERE lifecycle='pending-review'),(SELECT COUNT(*) FROM transfer_records WHERE state NOT IN ('completed','cancelled','failed'))",
            params![since],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        ).map_err(DatabaseError::Sqlite)?;
        Ok(MaintenanceStatus {
            open_notifications: nonnegative(counts.0).map_err(DatabaseError::Sqlite)?,
            recent_warning_events: nonnegative(counts.1).map_err(DatabaseError::Sqlite)?,
            recent_error_events: nonnegative(counts.2).map_err(DatabaseError::Sqlite)?,
            unavailable_storage_roots: nonnegative(counts.3).map_err(DatabaseError::Sqlite)?,
            pending_review_files: nonnegative(counts.4).map_err(DatabaseError::Sqlite)?,
            nonterminal_transfers: nonnegative(counts.5).map_err(DatabaseError::Sqlite)?,
            retention: self.retention_policy()?,
        })
    }

    fn summary_cutoff(&self, now_utc: i64, summary_days: u16) -> Result<i32, DatabaseError> {
        let timezone_name: String = self
            .connection
            .query_row(
                "SELECT timezone_name FROM transfer_timezone_policy WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .map_err(DatabaseError::Sqlite)?;
        let timezone = timezone_name.parse::<chrono_tz::Tz>().map_err(|_| {
            DatabaseError::IntegrityCheck("stored board timezone is invalid".to_owned())
        })?;
        let now = chrono::DateTime::<Utc>::from_timestamp(now_utc, 0).ok_or_else(|| {
            DatabaseError::IntegrityCheck(
                "retention time is outside the supported range".to_owned(),
            )
        })?;
        let cutoff = now
            .with_timezone(&timezone)
            .date_naive()
            .checked_sub_days(Days::new(u64::from(summary_days)))
            .ok_or_else(|| {
                DatabaseError::IntegrityCheck(
                    "summary retention cutoff is outside the supported range".to_owned(),
                )
            })?;
        Ok(cutoff.year() * 10_000
            + i32::try_from(cutoff.month()).unwrap_or(0) * 100
            + i32::try_from(cutoff.day()).unwrap_or(0))
    }
}

pub(crate) fn insert_operational_event_tx(
    transaction: &Transaction<'_>,
    event: &NewOperationalEvent,
) -> Result<OperationalEvent, DatabaseError> {
    validate_new_event(event)?;
    let (timezone_name, timezone_version): (String, i64) = transaction
        .query_row(
            "SELECT timezone_name,state_version FROM transfer_timezone_policy WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let timezone = timezone_name.parse::<chrono_tz::Tz>().map_err(|_| {
        DatabaseError::IntegrityCheck("stored board timezone is invalid".to_owned())
    })?;
    let board_day = board_local_day(event.occurred_at_utc, timezone)?;
    let encoded = encode_attributes(&event.attributes)?;
    let inserted = transaction.execute(
        "INSERT OR IGNORE INTO operational_events(occurred_at_utc,board_day,timezone_policy_version,category,severity,event_code,outcome,node_id,session_id,caller_id,correlation_id,idempotency_key,object_kind,object_id,retention_class,attribute_kind,text_value_1,text_value_2,text_value_3,number_value_1,number_value_2) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
        params![event.occurred_at_utc,board_day,timezone_version,event.category.as_str(),event.severity.as_str(),event.event_code,event.outcome.as_str(),event.node_id.map(i64::from),event.session_id.map(sqlite_u64).transpose()?,event.caller_id.map(CallerId::get),event.correlation_id,event.idempotency_key,event.object_kind,event.object_id,event.retention_class.as_str(),encoded.kind,encoded.text_1,encoded.text_2,encoded.text_3,encoded.number_1.map(sqlite_u64).transpose()?,encoded.number_2.map(sqlite_u64).transpose()?],
    ).map_err(DatabaseError::Sqlite)?;
    if inserted == 0 {
        let key = event.idempotency_key.as_deref().ok_or_else(|| {
            DatabaseError::IntegrityCheck("operational event was rejected".to_owned())
        })?;
        return transaction
            .query_row(
                "SELECT event_id,occurred_at_utc,board_day,timezone_policy_version,category,severity,event_code,outcome,node_id,session_id,caller_id,correlation_id,object_kind,object_id,retention_class,attribute_kind,text_value_1,text_value_2,text_value_3,number_value_1,number_value_2 FROM operational_events WHERE idempotency_key=?1",
                params![key],
                decode_event,
            )
            .map_err(DatabaseError::Sqlite);
    }
    let event_id =
        EventId(nonnegative(transaction.last_insert_rowid()).map_err(DatabaseError::Sqlite)?);
    update_daily_summary(
        transaction,
        event_id,
        board_day,
        timezone_version,
        event,
        &encoded,
    )?;
    if notification_reason(event).is_some() {
        let (reason, remediation) = notification_reason(event).expect("checked above");
        transaction.execute(
            "INSERT OR IGNORE INTO operator_notifications(source_event_id,created_at,category,severity,reason_key,remediation_key) VALUES(?1,?2,?3,?4,?5,?6)",
            params![sqlite_u64(event_id.get())?,event.occurred_at_utc,event.category.as_str(),event.severity.as_str(),reason,remediation],
        ).map_err(DatabaseError::Sqlite)?;
    }
    Ok(OperationalEvent {
        id: event_id,
        occurred_at_utc: event.occurred_at_utc,
        board_day,
        timezone_policy_version: nonnegative(timezone_version).map_err(DatabaseError::Sqlite)?,
        category: event.category,
        severity: event.severity,
        event_code: event.event_code.clone(),
        outcome: event.outcome,
        node_id: event.node_id,
        session_id: event.session_id,
        caller_id: event.caller_id,
        correlation_id: event.correlation_id.clone(),
        object_kind: event.object_kind.clone(),
        object_id: event.object_id.clone(),
        retention_class: event.retention_class,
        attributes: event.attributes.clone(),
    })
}

#[derive(Clone)]
pub struct ObservabilityService {
    database_path: PathBuf,
    started_at: i64,
    ring: Arc<Mutex<LiveRing>>,
    subscribers: Arc<Mutex<Vec<Weak<Mutex<SubscriberQueue>>>>>,
}

#[derive(Default)]
struct LiveRing {
    latest_id: u64,
    gap_before_first: bool,
    events: VecDeque<OperationalEvent>,
}

#[derive(Default)]
struct SubscriberQueue {
    latest_id: u64,
    gap_before_first: bool,
    events: VecDeque<OperationalEvent>,
}

#[derive(Clone)]
pub struct LiveEventSubscription {
    queue: Arc<Mutex<SubscriberQueue>>,
}

impl LiveEventSubscription {
    pub fn drain(&self) -> Result<LiveEventBatch, DatabaseError> {
        let mut queue = self.queue.lock().map_err(|_| {
            DatabaseError::IntegrityCheck(
                "live observability subscription coordination failed".to_owned(),
            )
        })?;
        let events = queue.events.drain(..).collect();
        let gap_before_first = queue.gap_before_first;
        queue.gap_before_first = false;
        Ok(LiveEventBatch {
            events,
            gap_before_first,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveEventBatch {
    pub events: Vec<OperationalEvent>,
    /// True means a consumer must resume from durable history to see every
    /// event; no missing event is silently represented as a complete stream.
    pub gap_before_first: bool,
}

impl ObservabilityService {
    pub fn new(database_path: &Path, started_at: i64) -> Self {
        Self {
            database_path: database_path.to_path_buf(),
            started_at,
            ring: Arc::new(Mutex::new(LiveRing::default())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn subscribe_live(&self) -> Result<LiveEventSubscription, DatabaseError> {
        let queue = Arc::new(Mutex::new(SubscriberQueue::default()));
        self.subscribers
            .lock()
            .map_err(|_| {
                DatabaseError::IntegrityCheck(
                    "live observability subscriber coordination failed".to_owned(),
                )
            })?
            .push(Arc::downgrade(&queue));
        Ok(LiveEventSubscription { queue })
    }

    pub fn poll_live_subscription(
        &self,
        subscription: &LiveEventSubscription,
        now_utc: i64,
    ) -> Result<LiveEventBatch, DatabaseError> {
        self.refresh_live_batch(now_utc)?;
        subscription.drain()
    }

    pub fn record(&self, event: &NewOperationalEvent) -> Result<OperationalEvent, DatabaseError> {
        let mut database = RuntimeDatabase::open(&self.database_path)?;
        let stored = database.record_operational_event(event)?;
        self.push_live(stored.clone())?;
        Ok(stored)
    }

    pub fn refresh_live(&self, now_utc: i64) -> Result<Vec<OperationalEvent>, DatabaseError> {
        Ok(self.refresh_live_batch(now_utc)?.events)
    }

    pub fn refresh_live_batch(&self, now_utc: i64) -> Result<LiveEventBatch, DatabaseError> {
        let latest = self
            .ring
            .lock()
            .map_err(|_| {
                DatabaseError::IntegrityCheck(
                    "live observability ring coordination failed".to_owned(),
                )
            })?
            .latest_id;
        let database = RuntimeDatabase::open_read_only(&self.database_path)?;
        let mut statement = database.connection.prepare(
            "SELECT event_id,occurred_at_utc,board_day,timezone_policy_version,category,severity,event_code,outcome,node_id,session_id,caller_id,correlation_id,object_kind,object_id,retention_class,attribute_kind,text_value_1,text_value_2,text_value_3,number_value_1,number_value_2 FROM operational_events WHERE event_id>?1 AND occurred_at_utc>=?2 ORDER BY event_id LIMIT 2049",
        ).map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map(params![sqlite_u64(latest)?, self.started_at], decode_event)
            .map_err(DatabaseError::Sqlite)?;
        let events = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        drop(statement);
        if events.len() > MAX_LIVE_EVENTS {
            self.ring
                .lock()
                .map_err(|_| {
                    DatabaseError::IntegrityCheck(
                        "live observability ring coordination failed".to_owned(),
                    )
                })?
                .gap_before_first = true;
        }
        for event in events {
            self.push_live(event)?;
        }
        let cutoff = now_utc.saturating_sub(LIVE_EVENT_HORIZON_SECONDS);
        let mut ring = self.ring.lock().map_err(|_| {
            DatabaseError::IntegrityCheck("live observability ring coordination failed".to_owned())
        })?;
        while ring
            .events
            .front()
            .is_some_and(|event| event.occurred_at_utc < cutoff)
        {
            ring.events.pop_front();
            ring.gap_before_first = true;
        }
        Ok(LiveEventBatch {
            events: ring.events.iter().cloned().collect(),
            gap_before_first: ring.gap_before_first,
        })
    }

    fn push_live(&self, event: OperationalEvent) -> Result<(), DatabaseError> {
        {
            let mut ring = self.ring.lock().map_err(|_| {
                DatabaseError::IntegrityCheck(
                    "live observability ring coordination failed".to_owned(),
                )
            })?;
            if event.id.get() <= ring.latest_id {
                return Ok(());
            }
            if ring.latest_id != 0 && event.id.get() > ring.latest_id.saturating_add(1) {
                ring.gap_before_first = true;
            }
            ring.latest_id = event.id.get();
            ring.events.push_back(event.clone());
            while ring.events.len() > MAX_LIVE_EVENTS {
                ring.events.pop_front();
                ring.gap_before_first = true;
            }
        }
        let mut subscribers = self.subscribers.lock().map_err(|_| {
            DatabaseError::IntegrityCheck(
                "live observability subscriber coordination failed".to_owned(),
            )
        })?;
        subscribers.retain(|subscriber| {
            let Some(subscriber) = subscriber.upgrade() else {
                return false;
            };
            let Ok(mut queue) = subscriber.lock() else {
                return false;
            };
            if queue.latest_id != 0 && event.id.get() > queue.latest_id.saturating_add(1) {
                queue.gap_before_first = true;
            }
            queue.latest_id = event.id.get();
            queue.events.push_back(event.clone());
            while queue.events.len() > MAX_LIVE_SUBSCRIBER_EVENTS {
                queue.events.pop_front();
                queue.gap_before_first = true;
            }
            true
        });
        Ok(())
    }
}

struct EncodedAttributes {
    kind: &'static str,
    text_1: Option<String>,
    text_2: Option<String>,
    text_3: Option<String>,
    number_1: Option<u64>,
    number_2: Option<u64>,
}

fn encode_attributes(attributes: &EventAttributes) -> Result<EncodedAttributes, DatabaseError> {
    let encoded = match attributes {
        EventAttributes::None => EncodedAttributes {
            kind: "none",
            text_1: None,
            text_2: None,
            text_3: None,
            number_1: None,
            number_2: None,
        },
        EventAttributes::Session {
            public_handle,
            transport,
            duration_seconds,
            close_reason,
        } => EncodedAttributes {
            kind: "session",
            text_1: public_handle.clone(),
            text_2: transport.clone(),
            text_3: close_reason.clone(),
            number_1: *duration_seconds,
            number_2: None,
        },
        EventAttributes::Transfer {
            protocol,
            direction,
            bytes,
            files,
        } => EncodedAttributes {
            kind: "transfer",
            text_1: protocol.clone(),
            text_2: direction.clone(),
            text_3: None,
            number_1: *bytes,
            number_2: *files,
        },
        EventAttributes::Message {
            conference_id,
            visibility,
            count,
        } => EncodedAttributes {
            kind: "message",
            text_1: Some(visibility.clone()),
            text_2: None,
            text_3: None,
            number_1: Some(*count),
            number_2: Some(*conference_id),
        },
        EventAttributes::File { operation, bytes } => EncodedAttributes {
            kind: "file",
            text_1: Some(operation.clone()),
            text_2: None,
            text_3: None,
            number_1: *bytes,
            number_2: None,
        },
        EventAttributes::Storage { state } => EncodedAttributes {
            kind: "storage",
            text_1: Some(state.clone()),
            text_2: None,
            text_3: None,
            number_1: None,
            number_2: None,
        },
        EventAttributes::Backup { state, bytes } => EncodedAttributes {
            kind: "backup",
            text_1: Some(state.clone()),
            text_2: None,
            text_3: None,
            number_1: *bytes,
            number_2: None,
        },
        EventAttributes::Error {
            subsystem,
            reason_key,
        } => EncodedAttributes {
            kind: "error",
            text_1: Some(subsystem.clone()),
            text_2: Some(reason_key.clone()),
            text_3: None,
            number_1: None,
            number_2: None,
        },
        EventAttributes::Operator { action } => EncodedAttributes {
            kind: "operator",
            text_1: Some(action.clone()),
            text_2: None,
            text_3: None,
            number_1: None,
            number_2: None,
        },
    };
    validate_optional_text(encoded.text_1.as_deref(), 256, "event attribute")?;
    validate_optional_text(encoded.text_2.as_deref(), 256, "event attribute")?;
    validate_optional_text(encoded.text_3.as_deref(), 256, "event attribute")?;
    Ok(encoded)
}

fn decode_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationalEvent> {
    let category = EventCategory::parse(&row.get::<_, String>(4)?)?;
    let severity = EventSeverity::parse(&row.get::<_, String>(5)?)?;
    let outcome = EventOutcome::parse(&row.get::<_, String>(7)?)?;
    let retention_class = RetentionClass::parse(&row.get::<_, String>(14)?)?;
    let kind: String = row.get(15)?;
    let text_1: Option<String> = row.get(16)?;
    let text_2: Option<String> = row.get(17)?;
    let text_3: Option<String> = row.get(18)?;
    let number_1: Option<i64> = row.get(19)?;
    let number_2: Option<i64> = row.get(20)?;
    let n1 = number_1.map(nonnegative).transpose()?;
    let n2 = number_2.map(nonnegative).transpose()?;
    let attributes = match kind.as_str() {
        "none" => EventAttributes::None,
        "session" => EventAttributes::Session {
            public_handle: text_1,
            transport: text_2,
            duration_seconds: n1,
            close_reason: text_3,
        },
        "transfer" => EventAttributes::Transfer {
            protocol: text_1,
            direction: text_2,
            bytes: n1,
            files: n2,
        },
        "message" => EventAttributes::Message {
            conference_id: n2.ok_or(rusqlite::Error::InvalidQuery)?,
            visibility: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
            count: n1.ok_or(rusqlite::Error::InvalidQuery)?,
        },
        "file" => EventAttributes::File {
            operation: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
            bytes: n1,
        },
        "storage" => EventAttributes::Storage {
            state: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
        },
        "backup" => EventAttributes::Backup {
            state: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
            bytes: n1,
        },
        "error" => EventAttributes::Error {
            subsystem: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
            reason_key: text_2.ok_or(rusqlite::Error::InvalidQuery)?,
        },
        "operator" => EventAttributes::Operator {
            action: text_1.ok_or(rusqlite::Error::InvalidQuery)?,
        },
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(OperationalEvent {
        id: EventId(nonnegative(row.get(0)?)?),
        occurred_at_utc: row.get(1)?,
        board_day: row.get(2)?,
        timezone_policy_version: nonnegative(row.get(3)?)?,
        category,
        severity,
        event_code: row.get(6)?,
        outcome,
        node_id: row
            .get::<_, Option<i64>>(8)?
            .map(|value| u32::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery))
            .transpose()?,
        session_id: row.get::<_, Option<i64>>(9)?.map(nonnegative).transpose()?,
        caller_id: row
            .get::<_, Option<i64>>(10)?
            .map(|value| CallerId::new(value).map_err(|_| rusqlite::Error::InvalidQuery))
            .transpose()?,
        correlation_id: row.get(11)?,
        object_kind: row.get(12)?,
        object_id: row.get(13)?,
        retention_class,
        attributes,
    })
}

fn validate_new_event(event: &NewOperationalEvent) -> Result<(), DatabaseError> {
    if event.occurred_at_utc < 0
        || event.event_code.len() < 3
        || event.event_code.len() > 64
        || !event.event_code.bytes().all(|value| {
            value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, b'.' | b'-')
        })
    {
        return Err(DatabaseError::IntegrityCheck(
            "operational event has an invalid timestamp or code".to_owned(),
        ));
    }
    validate_optional_text(event.correlation_id.as_deref(), 64, "correlation ID")?;
    validate_optional_text(event.idempotency_key.as_deref(), 96, "idempotency key")?;
    validate_optional_text(event.object_kind.as_deref(), 32, "object kind")?;
    validate_optional_text(event.object_id.as_deref(), 64, "object ID")?;
    if event.event_code.split('.').next() != Some(event.category.as_str())
        || !matches!(
            (&event.category, &event.attributes),
            (
                EventCategory::System,
                EventAttributes::None | EventAttributes::Error { .. }
            ) | (
                EventCategory::Node,
                EventAttributes::None | EventAttributes::Error { .. }
            ) | (EventCategory::Session, EventAttributes::Session { .. })
                | (EventCategory::Caller, EventAttributes::None)
                | (
                    EventCategory::Authentication,
                    EventAttributes::Session { .. }
                )
                | (EventCategory::Message, EventAttributes::Message { .. })
                | (EventCategory::File, EventAttributes::File { .. })
                | (EventCategory::Transfer, EventAttributes::Transfer { .. })
                | (EventCategory::Storage, EventAttributes::Storage { .. })
                | (EventCategory::Backup, EventAttributes::Backup { .. })
                | (EventCategory::Operator, EventAttributes::Operator { .. })
                | (EventCategory::Error, EventAttributes::Error { .. })
        )
    {
        return Err(DatabaseError::IntegrityCheck(
            "operational event code, category, and attributes disagree".to_owned(),
        ));
    }
    let encoded = encode_attributes(&event.attributes)?;
    let bytes = encoded.text_1.as_ref().map_or(0, |v| v.len())
        + encoded.text_2.as_ref().map_or(0, |v| v.len())
        + encoded.text_3.as_ref().map_or(0, |v| v.len());
    if bytes > 2_048 {
        return Err(DatabaseError::IntegrityCheck(
            "operational event attributes exceed 2 KiB".to_owned(),
        ));
    }
    Ok(())
}

fn validate_query(query: &EventQuery) -> Result<(), DatabaseError> {
    if query
        .limit
        .is_some_and(|value| value == 0 || value > MAX_EVENT_PAGE_SIZE)
    {
        return Err(DatabaseError::IntegrityCheck(
            "operational event page limit is outside 1..=500".to_owned(),
        ));
    }
    if let (Some(from), Some(through)) = (query.from_utc, query.through_utc) {
        validate_time_range(from, through)?;
    }
    if query
        .cursor
        .is_some_and(|cursor| cursor.event_id > cursor.snapshot_event_id)
    {
        return Err(DatabaseError::IntegrityCheck(
            "operational event cursor is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_time_range(from: i64, through: i64) -> Result<(), DatabaseError> {
    if from < 0 || through < from || through.saturating_sub(from) > MAX_EVENT_QUERY_DAYS * 86_400 {
        return Err(DatabaseError::IntegrityCheck(
            "operational event query exceeds the 31-day detail window".to_owned(),
        ));
    }
    Ok(())
}

fn update_daily_summary(
    transaction: &Transaction<'_>,
    event_id: EventId,
    board_day: i32,
    timezone_version: i64,
    event: &NewOperationalEvent,
    attributes: &EncodedAttributes,
) -> Result<(), DatabaseError> {
    let mut delta = [0_i64; 13];
    match event.event_code.as_str() {
        "session.started" => delta[0] = 1,
        "session.completed" => delta[1] = 1,
        "caller.created" => delta[2] = 1,
        "message.posted" => delta[3] = sqlite_u64(attributes.number_1.unwrap_or(1))?,
        "transfer.upload.completed" => {
            delta[4] = 1;
            delta[5] = sqlite_u64(attributes.number_1.unwrap_or(0))?;
        }
        "transfer.download.completed" => {
            delta[6] = 1;
            delta[7] = sqlite_u64(attributes.number_1.unwrap_or(0))?;
        }
        "transfer.failed" => delta[8] = 1,
        "transfer.cancelled" => delta[9] = 1,
        _ => {}
    }
    match event.severity {
        EventSeverity::Warning => delta[10] = 1,
        EventSeverity::Error => delta[11] = 1,
        EventSeverity::Critical => delta[12] = 1,
        _ => {}
    }
    transaction.execute(
        "INSERT INTO operational_daily_summaries(board_day,timezone_policy_version,high_water_event_id,calls_started,calls_completed,new_callers,messages_posted,successful_uploads,upload_bytes,successful_downloads,download_bytes,failed_transfers,cancelled_transfers,warning_events,error_events,critical_events,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17) ON CONFLICT(board_day,timezone_policy_version) DO UPDATE SET high_water_event_id=MAX(high_water_event_id,excluded.high_water_event_id),calls_started=calls_started+excluded.calls_started,calls_completed=calls_completed+excluded.calls_completed,new_callers=new_callers+excluded.new_callers,messages_posted=messages_posted+excluded.messages_posted,successful_uploads=successful_uploads+excluded.successful_uploads,upload_bytes=upload_bytes+excluded.upload_bytes,successful_downloads=successful_downloads+excluded.successful_downloads,download_bytes=download_bytes+excluded.download_bytes,failed_transfers=failed_transfers+excluded.failed_transfers,cancelled_transfers=cancelled_transfers+excluded.cancelled_transfers,warning_events=warning_events+excluded.warning_events,error_events=error_events+excluded.error_events,critical_events=critical_events+excluded.critical_events,state_version=state_version+1,updated_at=excluded.updated_at",
        params![board_day,timezone_version,sqlite_u64(event_id.get())?,delta[0],delta[1],delta[2],delta[3],delta[4],delta[5],delta[6],delta[7],delta[8],delta[9],delta[10],delta[11],delta[12],event.occurred_at_utc],
    ).map_err(DatabaseError::Sqlite)?;
    Ok(())
}

fn notification_reason(
    event: &NewOperationalEvent,
) -> Option<(&'static str, Option<&'static str>)> {
    match event.event_code.as_str() {
        "backup.failed" => Some((
            "operator-notification-backup-failed",
            Some("operator-remediation-check-backup"),
        )),
        "storage.unavailable" => Some((
            "operator-notification-storage-unavailable",
            Some("operator-remediation-check-storage"),
        )),
        "node.fault" => Some((
            "operator-notification-node-fault",
            Some("operator-remediation-check-node"),
        )),
        _ if matches!(
            event.severity,
            EventSeverity::Error | EventSeverity::Critical
        ) =>
        {
            Some((
                "operator-notification-operational-error",
                Some("operator-remediation-review-event"),
            ))
        }
        _ => None,
    }
}

fn severity_rank(severity: EventSeverity) -> i64 {
    match severity {
        EventSeverity::Info => 1,
        EventSeverity::Notice => 2,
        EventSeverity::Warning => 3,
        EventSeverity::Error => 4,
        EventSeverity::Critical => 5,
    }
}

fn validate_optional_text(
    value: Option<&str>,
    maximum: usize,
    name: &str,
) -> Result<(), DatabaseError> {
    if value.is_some_and(|value| {
        value.is_empty() || value.len() > maximum || value.chars().any(char::is_control)
    }) {
        return Err(DatabaseError::IntegrityCheck(format!(
            "{name} is empty, overlong, or contains controls"
        )));
    }
    Ok(())
}

fn validate_retention(detail_days: u16, summary_days: u16) -> Result<(), DatabaseError> {
    if !(1..=365).contains(&detail_days) || !(31..=3_650).contains(&summary_days) {
        return Err(DatabaseError::IntegrityCheck(
            "observability retention is outside the accepted bounds".to_owned(),
        ));
    }
    Ok(())
}

fn sqlite_u64(value: u64) -> Result<i64, DatabaseError> {
    i64::try_from(value).map_err(|_| {
        DatabaseError::IntegrityCheck("schema-18 identifier exceeds SQLite range".to_owned())
    })
}

fn nonnegative(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

pub fn now_utc() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardIdentity, SCHEMA_VERSION};
    use std::sync::{Arc, Barrier};

    fn database() -> (tempfile::TempDir, RuntimeDatabase) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("board.sqlite3");
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        database
            .ensure_board_identity(&BoardIdentity::new("Observability Board", "Sysop").unwrap())
            .unwrap();
        assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
        (temp, database)
    }

    fn event(at: i64, node: u32, code: &str, severity: EventSeverity) -> NewOperationalEvent {
        let mut event = NewOperationalEvent::new(
            at,
            EventCategory::Session,
            severity,
            code,
            EventOutcome::Succeeded,
        );
        event.node_id = Some(node);
        event.session_id = Some(u64::from(node));
        event.correlation_id = Some(format!("session-{node}"));
        event.idempotency_key = Some(format!("{code}-{node}-{at}"));
        event.attributes = EventAttributes::Session {
            public_handle: Some(format!("Caller {node}")),
            transport: Some("ssh".to_owned()),
            duration_seconds: Some(60),
            close_reason: Some("goodbye".to_owned()),
        };
        event
    }

    #[test]
    fn schema_eighteen_starts_honestly_and_uses_explicit_retention_defaults() {
        let (_temp, database) = database();
        let policy = database.retention_policy().unwrap();
        assert_eq!(policy.detail_days, 30);
        assert_eq!(policy.summary_days, 400);
        assert_eq!(policy.state_version, 1);
        assert!(database
            .query_operational_events(&EventQuery::default())
            .unwrap()
            .events
            .is_empty());
        assert!(database.notifications(true, 100).unwrap().is_empty());
    }

    #[test]
    fn event_insertion_is_typed_ordered_idempotent_and_updates_daily_summary_once() {
        let (_temp, mut database) = database();
        let first = database
            .record_operational_event(&event(
                1_800_000_000,
                1,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        let replay = database
            .record_operational_event(&event(
                1_800_000_000,
                1,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert_eq!(first.id, replay.id);
        database
            .record_operational_event(&event(
                1_800_000_000,
                2,
                "session.completed",
                EventSeverity::Notice,
            ))
            .unwrap();
        let page = database
            .query_operational_events(&EventQuery {
                limit: Some(1),
                ..EventQuery::default()
            })
            .unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].node_id, Some(2));
        database
            .record_operational_event(&event(
                1_700_000_000,
                3,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        let next = database
            .query_operational_events(&EventQuery {
                cursor: page.next_cursor,
                limit: Some(1),
                ..EventQuery::default()
            })
            .unwrap();
        assert_eq!(next.events[0].node_id, Some(1));
        assert!(next.next_cursor.is_none());
        let summary = database
            .daily_operational_summary(first.board_day, first.timezone_policy_version)
            .unwrap()
            .unwrap();
        assert_eq!(summary.calls_started, 1);
        assert_eq!(summary.calls_completed, 1);
        let json = first.to_json_line().unwrap();
        assert!(json.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema"], "spitfire-operational-event/v1");
        assert_eq!(parsed["event_code"], "session.started");
        assert_eq!(parsed["attributes"]["kind"], "session");
    }

    #[test]
    fn event_bounds_and_query_window_fail_closed() {
        let (_temp, mut database) = database();
        let mut overlong = event(1_800_000_000, 1, "session.started", EventSeverity::Info);
        overlong.correlation_id = Some("x".repeat(65));
        assert!(database.record_operational_event(&overlong).is_err());

        let mut mismatched = event(1_800_000_001, 1, "file.added", EventSeverity::Info);
        mismatched.idempotency_key = Some("mismatched-event-contract".to_owned());
        assert!(database.record_operational_event(&mismatched).is_err());
        assert!(database
            .query_operational_events(&EventQuery {
                from_utc: Some(0),
                through_utc: Some((MAX_EVENT_QUERY_DAYS + 1) * 86_400),
                ..EventQuery::default()
            })
            .is_err());
    }

    #[test]
    fn error_notification_acknowledgement_is_stale_safe_and_audited() {
        let (_temp, mut database) = database();
        let mut warning = event(
            1_800_000_000,
            1,
            "storage.unavailable",
            EventSeverity::Warning,
        );
        warning.category = EventCategory::Storage;
        warning.outcome = EventOutcome::Unavailable;
        warning.attributes = EventAttributes::Storage {
            state: "unavailable".to_owned(),
        };
        database.record_operational_event(&warning).unwrap();
        let notification = database.notifications(false, 10).unwrap().remove(0);
        let actor = OperatorPrincipal {
            kind: OperatorPrincipalKind::HostOperator,
            stable_id: Some("local-operator".to_owned()),
        };
        assert!(database
            .acknowledge_notification(notification.id, 1, &actor, 1_800_000_001)
            .unwrap());
        assert!(!database
            .acknowledge_notification(notification.id, 1, &actor, 1_800_000_002)
            .unwrap());
        let acknowledged = database.notifications(true, 10).unwrap().remove(0);
        assert_eq!(acknowledged.state, NotificationState::Acknowledged);
        let audits: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM operator_observability_audit",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audits, 2);
    }

    #[test]
    fn simultaneous_notification_acknowledgements_commit_once() {
        let (temp, mut database) = database();
        let path = temp.path().join("board.sqlite3");
        let mut warning = event(
            1_800_000_000,
            1,
            "storage.unavailable",
            EventSeverity::Warning,
        );
        warning.category = EventCategory::Storage;
        warning.outcome = EventOutcome::Unavailable;
        warning.attributes = EventAttributes::Storage {
            state: "unavailable".to_owned(),
        };
        database.record_operational_event(&warning).unwrap();
        let notification = database.notifications(false, 10).unwrap().remove(0);
        let first = RuntimeDatabase::open(&path).unwrap();
        let second = RuntimeDatabase::open(&path).unwrap();
        let workers = [first, second]
            .into_iter()
            .enumerate()
            .map(|(worker, mut database)| {
                std::thread::spawn(move || {
                    database
                        .acknowledge_notification(
                            notification.id,
                            1,
                            &OperatorPrincipal {
                                kind: OperatorPrincipalKind::HostOperator,
                                stable_id: Some(format!("operator-{worker}")),
                            },
                            1_800_000_001 + worker as i64,
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let committed = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|committed| *committed)
            .count();
        assert_eq!(committed, 1);
        assert_eq!(
            database.notifications(true, 10).unwrap()[0].state,
            NotificationState::Acknowledged
        );
    }

    #[test]
    fn live_ring_is_ephemeral_bounded_and_restart_begins_fresh() {
        let (temp, _database) = database();
        let path = temp.path().join("board.sqlite3");
        let service = ObservabilityService::new(&path, 1_800_000_000);
        let subscription = service.subscribe_live().unwrap();
        let stored = service
            .record(&event(
                1_800_000_001,
                1,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert_eq!(service.refresh_live(1_800_000_002).unwrap().len(), 1);
        for id in 2..=2_050 {
            let mut item = stored.clone();
            item.id = EventId(id);
            item.occurred_at_utc = 1_800_000_001;
            service.push_live(item).unwrap();
        }
        let bounded = service.refresh_live_batch(1_800_000_002).unwrap();
        assert_eq!(bounded.events.len(), MAX_LIVE_EVENTS);
        assert!(bounded.gap_before_first);
        let subscriber = subscription.drain().unwrap();
        assert_eq!(subscriber.events.len(), MAX_LIVE_SUBSCRIBER_EVENTS);
        assert!(subscriber.gap_before_first);
        let restarted = ObservabilityService::new(&path, 1_800_000_003);
        assert!(restarted.refresh_live(1_800_000_003).unwrap().is_empty());
    }

    #[test]
    fn retention_cleanup_is_bounded_and_never_deletes_audit_or_open_notifications() {
        let (_temp, mut database) = database();
        for node in 1..=600 {
            let mut item = event(
                1_700_000_000 + i64::from(node),
                node,
                "session.started",
                EventSeverity::Info,
            );
            item.idempotency_key = Some(format!("old-{node}"));
            database.record_operational_event(&item).unwrap();
        }
        let mut warning = event(
            1_700_000_700,
            700,
            "storage.unavailable",
            EventSeverity::Warning,
        );
        warning.category = EventCategory::Storage;
        warning.outcome = EventOutcome::Unavailable;
        warning.attributes = EventAttributes::Storage {
            state: "unavailable".to_owned(),
        };
        database.record_operational_event(&warning).unwrap();
        let first = database
            .cleanup_operational_retention(1_800_000_000)
            .unwrap();
        assert_eq!(first.event_rows_deleted, RETENTION_CLEANUP_BATCH);
        assert!(first.more_work);
        assert_eq!(database.notifications(false, 10).unwrap().len(), 1);
        let audit_count: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM operator_observability_audit",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audit_count, 1);
        while database
            .cleanup_operational_retention(1_800_000_000)
            .unwrap()
            .more_work
        {}
        let after_cleanup = database
            .record_operational_event(&event(
                1_800_000_001,
                800,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert!(after_cleanup.id.get() > 601);
    }

    #[test]
    fn retention_cleanup_and_bounded_readers_can_run_concurrently() {
        let (temp, mut database) = database();
        let path = temp.path().join("board.sqlite3");
        for node in 1..=520 {
            let mut item = event(
                1_700_000_000 + i64::from(node),
                node,
                "session.started",
                EventSeverity::Info,
            );
            item.idempotency_key = Some(format!("concurrent-old-{node}"));
            database.record_operational_event(&item).unwrap();
        }
        let barrier = Arc::new(Barrier::new(2));
        let reader_barrier = Arc::clone(&barrier);
        let reader = std::thread::spawn(move || {
            let database = RuntimeDatabase::open_read_only(&path).unwrap();
            reader_barrier.wait();
            for _ in 0..8 {
                let page = database
                    .query_operational_events(&EventQuery {
                        limit: Some(MAX_EVENT_PAGE_SIZE),
                        ..EventQuery::default()
                    })
                    .unwrap();
                assert!(page.events.len() <= MAX_EVENT_PAGE_SIZE);
            }
        });
        barrier.wait();
        let cleanup = database
            .cleanup_operational_retention(1_800_000_000)
            .unwrap();
        assert_eq!(cleanup.event_rows_deleted, RETENTION_CLEANUP_BATCH);
        reader.join().unwrap();
    }

    #[test]
    fn summary_rebuild_and_retention_policy_are_versioned_and_idempotent() {
        let (_temp, mut database) = database();
        let stored = database
            .record_operational_event(&event(
                1_800_000_000,
                1,
                "session.completed",
                EventSeverity::Info,
            ))
            .unwrap();
        let first = database
            .rebuild_daily_operational_summary(
                stored.board_day,
                stored.timezone_policy_version,
                1_800_000_001,
            )
            .unwrap();
        let second = database
            .rebuild_daily_operational_summary(
                stored.board_day,
                stored.timezone_policy_version,
                1_800_000_002,
            )
            .unwrap();
        assert_eq!(first.calls_completed, 1);
        assert_eq!(second.calls_completed, 1);
        let actor = OperatorPrincipal {
            kind: OperatorPrincipalKind::HostOperator,
            stable_id: Some("retention-admin".to_owned()),
        };
        assert!(database
            .update_retention_policy(1, 45, 500, None, &actor, 1_800_000_003)
            .unwrap()
            .is_some());
        assert!(database
            .update_retention_policy(1, 60, 600, None, &actor, 1_800_000_004)
            .unwrap()
            .is_none());
        assert_eq!(database.retention_policy().unwrap().detail_days, 45);
        let impact = database.retention_impact(30, 400, 1_800_000_005).unwrap();
        assert!(database
            .update_retention_policy(2, 30, 400, None, &actor, 1_800_000_005)
            .is_err());
        assert!(database
            .update_retention_policy(2, 30, 400, Some(impact), &actor, 1_800_000_005,)
            .unwrap()
            .is_some());
    }

    #[test]
    fn timezone_versions_keep_daily_facts_separate_across_dst_and_policy_change() {
        use chrono::TimeZone;

        let (_temp, mut database) = database();
        database
            .connection
            .execute(
                "UPDATE transfer_timezone_policy SET timezone_name='America/New_York',state_version=2 WHERE singleton=1",
                [],
            )
            .unwrap();
        let timezone = chrono_tz::America::New_York;
        let spring_before = timezone
            .with_ymd_and_hms(2026, 3, 8, 1, 59, 0)
            .single()
            .unwrap()
            .timestamp();
        let spring_after = timezone
            .with_ymd_and_hms(2026, 3, 8, 3, 1, 0)
            .single()
            .unwrap()
            .timestamp();
        let before = database
            .record_operational_event(&event(
                spring_before,
                1,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        let after = database
            .record_operational_event(&event(
                spring_after,
                2,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert_eq!((before.board_day, after.board_day), (20260308, 20260308));
        let repeated = timezone.with_ymd_and_hms(2026, 11, 1, 1, 30, 0);
        let (first_hour, second_hour) = match repeated {
            chrono::LocalResult::Ambiguous(first, second) => {
                (first.timestamp(), second.timestamp())
            }
            _ => panic!("expected a repeated New York fall-back hour"),
        };
        let first = database
            .record_operational_event(&event(
                first_hour,
                3,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        let second = database
            .record_operational_event(&event(
                second_hour,
                4,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert_eq!((first.board_day, second.board_day), (20261101, 20261101));
        let midnight_before = timezone
            .with_ymd_and_hms(2026, 4, 5, 23, 59, 59)
            .single()
            .unwrap()
            .timestamp();
        let midnight_after = timezone
            .with_ymd_and_hms(2026, 4, 6, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(
            database
                .record_operational_event(&event(
                    midnight_before,
                    5,
                    "session.started",
                    EventSeverity::Info,
                ))
                .unwrap()
                .board_day,
            20260405
        );
        assert_eq!(
            database
                .record_operational_event(&event(
                    midnight_after,
                    6,
                    "session.started",
                    EventSeverity::Info,
                ))
                .unwrap()
                .board_day,
            20260406
        );
        assert_eq!(before.timezone_policy_version, 2);
        database
            .connection
            .execute(
                "UPDATE transfer_timezone_policy SET timezone_name='America/Phoenix',state_version=3 WHERE singleton=1",
                [],
            )
            .unwrap();
        let changed = database
            .record_operational_event(&event(
                spring_before,
                7,
                "session.started",
                EventSeverity::Info,
            ))
            .unwrap();
        assert_eq!(changed.timezone_policy_version, 3);
        assert_eq!(changed.board_day, 20260307);
    }

    #[test]
    fn concurrent_nodes_get_unique_event_ids_and_exact_totals() {
        let (temp, _database) = database();
        let path = temp.path().join("board.sqlite3");
        let mut workers = Vec::new();
        for node in 1..=2 {
            // Open both migrated connections before either worker starts. The
            // concurrency exercised here is event insertion, not two schema
            // migration coordinators racing during process startup.
            let mut database = RuntimeDatabase::open(&path).unwrap();
            workers.push(std::thread::spawn(move || {
                for offset in 0..25 {
                    database
                        .record_operational_event(&event(
                            1_800_000_000 + offset,
                            node,
                            "session.completed",
                            EventSeverity::Info,
                        ))
                        .unwrap();
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        let database = RuntimeDatabase::open_read_only(&path).unwrap();
        let page = database
            .query_operational_events(&EventQuery {
                limit: Some(100),
                ..EventQuery::default()
            })
            .unwrap();
        assert_eq!(page.events.len(), 50);
        let ids = page
            .events
            .iter()
            .map(|item| item.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 50);
        let sample = &page.events[0];
        assert_eq!(
            database
                .daily_operational_summary(sample.board_day, sample.timezone_policy_version)
                .unwrap()
                .unwrap()
                .calls_completed,
            50
        );
    }

    #[test]
    fn ordinary_event_storage_contains_no_authentication_secret_or_private_identity() {
        let (_temp, mut database) = database();
        database
            .record_authentication_failure(1_800_000_000, 1, 9, "ssh", "invalid-credentials")
            .unwrap();
        let serialized: String = database.connection.query_row(
            "SELECT COALESCE(event_code,'')||COALESCE(correlation_id,'')||COALESCE(text_value_1,'')||COALESCE(text_value_2,'')||COALESCE(text_value_3,'') FROM operational_events",
            [], |row| row.get(0),
        ).unwrap();
        for forbidden in [
            "secret-password",
            "private-login",
            "Private Real Name",
            "/Users/",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
