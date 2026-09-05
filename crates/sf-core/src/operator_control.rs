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

//! Durable, bounded command receipts and semantic operator-control audit.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{DatabaseError, RuntimeDatabase};

pub const OPERATOR_COMMAND_RETENTION_DAYS: i64 = 30;
pub const OPERATOR_COMMAND_CLEANUP_BATCH: usize = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOperatorCommandReceipt {
    pub command_id: String,
    pub daemon_generation: String,
    pub operator_id: String,
    pub command_family: String,
    pub command_type: String,
    pub request_fingerprint: String,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub target_generation: Option<String>,
    pub received_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorCommandReceipt {
    pub command_id: String,
    pub request_fingerprint: String,
    pub state: String,
    pub result_class: Option<String>,
    pub result_version: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandReceiptResult {
    Accepted,
    Replayed(OperatorCommandReceipt),
    FingerprintConflict,
    PrincipalConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOperatorControlAudit {
    pub occurred_at: i64,
    pub operator_kind: String,
    pub operator_id: Option<String>,
    pub operation: String,
    pub authorization_result: String,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub command_id: Option<String>,
    pub correlation_id: Option<String>,
    pub outcome: String,
    pub detail_code: Option<String>,
}

impl RuntimeDatabase {
    pub fn accept_operator_command(
        &mut self,
        receipt: &NewOperatorCommandReceipt,
    ) -> Result<CommandReceiptResult, DatabaseError> {
        validate_receipt(receipt)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let existing = transaction.query_row(
            "SELECT command_id,request_fingerprint,state,result_class,result_version,operator_id,daemon_generation FROM operator_command_journal WHERE command_id=?1",
            [&receipt.command_id],
            |row| Ok((OperatorCommandReceipt { command_id:row.get(0)?,request_fingerprint:row.get(1)?,state:row.get(2)?,result_class:row.get(3)?,result_version:row.get::<_,Option<i64>>(4)?.and_then(|value| u64::try_from(value).ok()) }, row.get::<_,String>(5)?, row.get::<_,String>(6)?)),
        ).optional().map_err(DatabaseError::Sqlite)?;
        if let Some((existing, operator_id, daemon_generation)) = existing {
            return Ok(
                if operator_id != receipt.operator_id
                    || daemon_generation != receipt.daemon_generation
                {
                    CommandReceiptResult::PrincipalConflict
                } else if existing.request_fingerprint == receipt.request_fingerprint {
                    CommandReceiptResult::Replayed(existing)
                } else {
                    CommandReceiptResult::FingerprintConflict
                },
            );
        }
        let expires_at = receipt
            .received_at
            .checked_add(OPERATOR_COMMAND_RETENTION_DAYS * 86_400)
            .ok_or_else(|| {
                DatabaseError::IntegrityCheck("operator command expiry overflowed".to_owned())
            })?;
        transaction.execute(
            "INSERT INTO operator_command_journal(command_id,daemon_generation,operator_kind,operator_id,command_family,command_type,request_fingerprint,target_kind,target_id,target_generation,state,received_at,expires_at) VALUES(?1,?2,'host-operator',?3,?4,?5,?6,?7,?8,?9,'accepted',?10,?11)",
            params![receipt.command_id,receipt.daemon_generation,receipt.operator_id,receipt.command_family,receipt.command_type,receipt.request_fingerprint,receipt.target_kind,receipt.target_id,receipt.target_generation,receipt.received_at,expires_at],
        ).map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        Ok(CommandReceiptResult::Accepted)
    }

    pub fn operator_command_receipt(
        &self,
        command_id: &str,
        operator_id: &str,
        daemon_generation: &str,
    ) -> Result<Option<OperatorCommandReceipt>, DatabaseError> {
        self.connection
            .query_row(
                "SELECT command_id,request_fingerprint,state,result_class,result_version FROM operator_command_journal WHERE command_id=?1 AND operator_id=?2 AND daemon_generation=?3",
                params![command_id, operator_id, daemon_generation],
                |row| Ok(OperatorCommandReceipt { command_id: row.get(0)?, request_fingerprint: row.get(1)?, state: row.get(2)?, result_class: row.get(3)?, result_version: row.get::<_, Option<i64>>(4)?.and_then(|v| u64::try_from(v).ok()) }),
            )
            .optional()
            .map_err(DatabaseError::Sqlite)
    }

    pub fn reject_operator_command(
        &mut self,
        command_id: &str,
        result_class: &str,
        completed_at: i64,
    ) -> Result<bool, DatabaseError> {
        self.connection
            .execute("UPDATE operator_command_journal SET state='rejected',result_class=?2,completed_at=?3 WHERE command_id=?1 AND state='accepted'", params![command_id, result_class, completed_at])
            .map(|count| count == 1)
            .map_err(DatabaseError::Sqlite)
    }

    pub fn complete_operator_command(
        &mut self,
        command_id: &str,
        result_class: &str,
        result_version: u64,
        completed_at: i64,
    ) -> Result<bool, DatabaseError> {
        if result_class.len() < 3
            || result_class.len() > 64
            || result_class.chars().any(char::is_control)
        {
            return Err(DatabaseError::IntegrityCheck(
                "invalid operator result class".to_owned(),
            ));
        }
        let result_version = i64::try_from(result_version)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                DatabaseError::IntegrityCheck("invalid operator result version".to_owned())
            })?;
        self.connection.execute("UPDATE operator_command_journal SET state='completed',result_class=?2,result_version=?3,completed_at=?4 WHERE command_id=?1 AND state='accepted'", params![command_id,result_class,result_version,completed_at]).map(|count| count == 1).map_err(DatabaseError::Sqlite)
    }

    pub fn cleanup_operator_command_journal(
        &mut self,
        now_utc: i64,
    ) -> Result<usize, DatabaseError> {
        self.connection.execute("DELETE FROM operator_command_journal WHERE command_id IN (SELECT command_id FROM operator_command_journal WHERE expires_at<=?1 ORDER BY expires_at,command_id LIMIT ?2)", params![now_utc, i64::try_from(OPERATOR_COMMAND_CLEANUP_BATCH).unwrap_or(500)]).map_err(DatabaseError::Sqlite)
    }

    pub fn record_operator_control_audit(
        &mut self,
        audit: &NewOperatorControlAudit,
    ) -> Result<u64, DatabaseError> {
        if audit.occurred_at < 0 || audit.operation.len() < 3 || audit.operation.len() > 64 {
            return Err(DatabaseError::IntegrityCheck(
                "invalid operator control audit".to_owned(),
            ));
        }
        self.connection.execute("INSERT INTO operator_control_audit(occurred_at,operator_kind,operator_id,operation,authorization_result,target_kind,target_id,command_id,correlation_id,outcome,detail_code) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![audit.occurred_at,audit.operator_kind,audit.operator_id,audit.operation,audit.authorization_result,audit.target_kind,audit.target_id,audit.command_id,audit.correlation_id,audit.outcome,audit.detail_code]).map_err(DatabaseError::Sqlite)?;
        Ok(self.connection.last_insert_rowid() as u64)
    }
}

fn validate_receipt(value: &NewOperatorCommandReceipt) -> Result<(), DatabaseError> {
    let bounded = |text: &str, minimum, maximum| {
        text.len() >= minimum && text.len() <= maximum && !text.chars().any(char::is_control)
    };
    if !bounded(&value.command_id, 16, 64)
        || value.daemon_generation.len() != 32
        || value.request_fingerprint.len() != 64
        || !bounded(&value.operator_id, 1, 64)
        || !bounded(&value.command_family, 3, 48)
        || !bounded(&value.command_type, 3, 64)
        || value.received_at < 0
    {
        return Err(DatabaseError::IntegrityCheck(
            "invalid bounded operator command receipt".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoardIdentity;

    #[test]
    fn command_receipts_are_idempotent_bounded_and_separate_from_audit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("control.sqlite3");
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        database
            .ensure_board_identity(&BoardIdentity::new("Control", "Sysop").unwrap())
            .unwrap();
        let receipt = NewOperatorCommandReceipt {
            command_id: "command-00000001".to_owned(),
            daemon_generation: "1".repeat(32),
            operator_id: "unix-uid:501".to_owned(),
            command_family: "node".to_owned(),
            command_type: "node.disconnect".to_owned(),
            request_fingerprint: "a".repeat(64),
            target_kind: Some("session".to_owned()),
            target_id: Some("22".to_owned()),
            target_generation: Some("7".to_owned()),
            received_at: 1_700_000_000,
        };
        assert_eq!(
            database.accept_operator_command(&receipt).unwrap(),
            CommandReceiptResult::Accepted
        );
        assert!(matches!(
            database.accept_operator_command(&receipt).unwrap(),
            CommandReceiptResult::Replayed(_)
        ));
        let mut conflict = receipt.clone();
        conflict.request_fingerprint = "b".repeat(64);
        assert_eq!(
            database.accept_operator_command(&conflict).unwrap(),
            CommandReceiptResult::FingerprintConflict
        );
        let mut principal = receipt.clone();
        principal.operator_id = "unix-uid:502".to_owned();
        assert_eq!(
            database.accept_operator_command(&principal).unwrap(),
            CommandReceiptResult::PrincipalConflict
        );
        assert!(database
            .complete_operator_command(&receipt.command_id, "succeeded", 1, 1_700_000_001)
            .unwrap());
        assert_eq!(
            database
                .cleanup_operator_command_journal(1_700_000_000 + 31 * 86_400)
                .unwrap(),
            1
        );
    }
}
