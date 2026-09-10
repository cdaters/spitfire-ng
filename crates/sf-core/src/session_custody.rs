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

//! Exclusive caller admission custody; recovered only by the board-lock owner.
use crate::{
    insert_operational_event_tx, CallerId, DatabaseError, EventCategory, EventOutcome,
    EventSeverity, NewOperationalEvent, RuntimeDatabase,
};
use rusqlite::{params, Transaction};

pub(crate) fn write_event(
    tx: &Transaction<'_>,
    caller: CallerId,
    now: i64,
    node: u32,
    session: u64,
    code: &str,
) -> Result<(), DatabaseError> {
    let caller_event = code.starts_with("caller.");
    let mut event = NewOperationalEvent::new(
        now,
        if caller_event {
            EventCategory::Caller
        } else {
            EventCategory::Session
        },
        EventSeverity::Notice,
        code,
        if code.ends_with("recovered") || code.ends_with("abandoned") {
            EventOutcome::Observed
        } else {
            EventOutcome::Denied
        },
    );
    if !caller_event {
        event.attributes = crate::EventAttributes::Session {
            public_handle: None,
            transport: None,
            duration_seconds: None,
            close_reason: None,
        };
    }
    event.caller_id = Some(caller);
    event.node_id = (node != 0).then_some(node);
    event.session_id = (session != 0).then_some(session);
    insert_operational_event_tx(tx, &event)?;
    Ok(())
}

pub(crate) fn refuse<T>(
    tx: Transaction<'_>,
    caller: CallerId,
    now: i64,
    node: u32,
    session: u64,
    code: &str,
    error: DatabaseError,
) -> Result<T, DatabaseError> {
    write_event(&tx, caller, now, node, session, code)?;
    tx.commit().map_err(DatabaseError::Sqlite)?;
    Err(error)
}

impl RuntimeDatabase {
    /// Host-only binding. All connections serving one board runtime use its
    /// generation. A connection opened by an unrelated operation has no claim.
    pub fn bind_session_generation(&mut self, generation: &str) {
        self.session_generation = generation.to_owned();
    }

    /// Call only while holding the exclusive board-operation lock, before any
    /// listener or caller starts. Never infer liveness from wall-clock age.
    pub fn recover_caller_sessions(&mut self, now: i64) -> Result<usize, DatabaseError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let rows = tx
            .prepare("SELECT caller_id,node_id,session_id FROM caller_sessions")
            .map_err(DatabaseError::Sqlite)?
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(DatabaseError::Sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        for (caller, node, session) in &rows {
            write_event(
                &tx,
                CallerId::new(*caller)?,
                now,
                *node,
                u64::try_from(*session).map_err(|_| DatabaseError::SessionOwnershipMismatch)?,
                "session.interrupted-recovered",
            )?;
        }
        tx.execute("DELETE FROM caller_sessions", [])
            .map_err(DatabaseError::Sqlite)?;
        tx.commit().map_err(DatabaseError::Sqlite)?;
        Ok(rows.len())
    }

    /// Release only this connection's exact runtime/node/session on unwind.
    /// Normal settlement has already removed its claim, making this a no-op.
    pub fn abandon_caller_session(
        &mut self,
        node: u32,
        session: u64,
        now: i64,
    ) -> Result<(), DatabaseError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let ids = tx.prepare("SELECT caller_id FROM caller_sessions WHERE runtime_generation=?1 AND node_id=?2 AND session_id=?3")
            .map_err(DatabaseError::Sqlite)?.query_map(params![self.session_generation, node, i64::try_from(session).map_err(|_| DatabaseError::SessionOwnershipMismatch)?], |r| r.get::<_,i64>(0))
            .map_err(DatabaseError::Sqlite)?.collect::<Result<Vec<_>,_>>().map_err(DatabaseError::Sqlite)?;
        for id in ids {
            write_event(
                &tx,
                CallerId::new(id)?,
                now,
                node,
                session,
                "session.abandoned",
            )?;
        }
        tx.execute("DELETE FROM caller_sessions WHERE runtime_generation=?1 AND node_id=?2 AND session_id=?3",
            params![self.session_generation, node, i64::try_from(session).map_err(|_| DatabaseError::SessionOwnershipMismatch)?]).map_err(DatabaseError::Sqlite)?;
        tx.commit().map_err(DatabaseError::Sqlite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallerAccessActor, CallerConfig, CallerPreferences, CallerProfilePolicy, CallerState,
        CredentialHasher, PasswordHashConfig, SecurityLevel,
    };

    fn board() -> (
        tempfile::TempDir,
        RuntimeDatabase,
        RuntimeDatabase,
        crate::Caller,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("board.sqlite3");
        let mut a = RuntimeDatabase::open(&path).unwrap();
        a.migrate().unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(&rand::random::<[u8; 32]>()).unwrap();
        let caller = a
            .create_caller(
                b"Session Fixture",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                true,
                1_700_000_000,
            )
            .unwrap();
        let b = RuntimeDatabase::open(&path).unwrap();
        (temp, a, b, caller)
    }
    fn policy() -> CallerConfig {
        CallerConfig {
            minutes_per_call: 1,
            minutes_per_day: 1,
            new_caller_first_day_minutes: 1,
            ..CallerConfig::default()
        }
    }
    fn admit(
        db: &mut RuntimeDatabase,
        c: &crate::Caller,
        node: u32,
    ) -> Result<crate::AuthenticatedCaller, DatabaseError> {
        db.begin_caller_session_observed(
            c,
            &policy(),
            1_700_000_001,
            chrono_tz::UTC,
            Some((node, u64::from(node), "telnet")),
        )
    }
    #[test]
    fn d2_atomic_exclusive_allowance_and_exact_settlement() {
        let (_temp, mut a, mut b, caller) = board();
        let first = admit(&mut a, &caller, 1).unwrap();
        assert_eq!(first.allowance.limit_seconds(), 60);
        assert!(matches!(
            admit(&mut b, &caller, 2),
            Err(DatabaseError::CallerAlreadyOnline)
        ));
        assert_eq!(a.caller_by_id(caller.id).unwrap().unwrap().call_count, 1);
        let day = i64::from(crate::board_local_day(1_700_000_001, chrono_tz::UTC).unwrap());
        assert!(matches!(
            b.finish_caller_session_observed(
                caller.id,
                10,
                10,
                day,
                Some((1, 1, "telnet", "goodbye")),
                1_700_000_011
            ),
            Err(DatabaseError::SessionOwnershipMismatch)
        ));
        b.abandon_caller_session(1, 1, 1_700_000_011).unwrap();
        assert!(matches!(
            admit(&mut b, &caller, 2),
            Err(DatabaseError::CallerAlreadyOnline)
        ));
        a.finish_caller_session_observed(
            caller.id,
            59,
            59,
            day,
            Some((1, 1, "telnet", "goodbye")),
            1_700_000_060,
        )
        .unwrap();
        assert!(matches!(
            a.finish_caller_session_observed(
                caller.id,
                59,
                59,
                day,
                Some((1, 1, "telnet", "goodbye")),
                1_700_000_060
            ),
            Err(DatabaseError::SessionOwnershipMismatch)
        ));
        let second = admit(&mut b, &caller, 2).unwrap();
        assert_eq!(second.allowance.limit_seconds(), 1);
        assert!(!second.first_session);
        assert_eq!(second.previous_call_at, Some(1_700_000_001));
        b.finish_caller_session_observed(
            caller.id,
            1,
            1,
            day,
            Some((2, 2, "telnet", "goodbye")),
            1_700_000_061,
        )
        .unwrap();
        assert!(matches!(
            admit(&mut a, &caller, 1),
            Err(DatabaseError::DailyTimeLimitReached)
        ));
        assert_eq!(a.caller_by_id(caller.id).unwrap().unwrap().call_count, 2);
    }
    #[test]
    fn d2_lifecycle_revalidated_after_authentication_snapshot() {
        let (_temp, mut a, mut b, caller) = board();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let password = rand::random::<[u8; 32]>();
        a.connection
            .execute(
                "UPDATE caller_credentials SET password_hash=?2 WHERE caller_id=?1",
                params![caller.id.get(), hasher.hash(&password).unwrap()],
            )
            .unwrap();
        let verified = match a
            .authenticate_login_identifier(caller.login_identifier.as_bytes(), &password, &hasher)
            .unwrap()
        {
            crate::AuthenticationResult::Valid(caller) => caller,
            _ => panic!("fixture authentication failed"),
        };
        let disabled = b
            .mutate_caller_lifecycle(
                caller.id,
                caller.state_version,
                CallerState::Disabled,
                CallerAccessActor::LocalOperator,
                &policy(),
                1_700_000_001,
            )
            .unwrap();
        assert!(matches!(
            admit(&mut a, &verified, 1),
            Err(DatabaseError::CallerUnavailable)
        ));
        assert_eq!(a.caller_by_id(caller.id).unwrap().unwrap().call_count, 0);
        assert_eq!(
            a.connection
                .query_row("SELECT count(*) FROM caller_sessions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        b.mutate_caller_lifecycle(
            caller.id,
            disabled.state_version,
            CallerState::Active,
            CallerAccessActor::LocalOperator,
            &policy(),
            1_700_000_002,
        )
        .unwrap();
        assert!(admit(&mut a, &verified, 1).is_ok());
    }
    #[test]
    fn d2_preference_and_contact_snapshots_conflict_without_lost_updates() {
        let (_temp, a, b, caller) = board();
        let mut first = caller.preferences;
        first.hot_keys = !first.hot_keys;
        let committed = a
            .update_caller_preferences(caller.id, caller.state_version, first)
            .unwrap();
        let mut stale = caller.preferences;
        stale.more_prompt = !stale.more_prompt;
        assert!(matches!(
            b.update_caller_preferences(caller.id, caller.state_version, stale),
            Err(DatabaseError::CallerStateConflict { .. })
        ));
        assert!(matches!(
            b.update_caller_preferences(
                caller.id,
                caller.state_version,
                CallerPreferences::default()
            ),
            Err(DatabaseError::CallerStateConflict { .. })
        ));
        assert_eq!(
            b.caller_by_id(caller.id).unwrap().unwrap().preferences,
            first
        );
        let mut profile = committed.profile.clone();
        profile.address.city = Some("Fixture City".into());
        let policy = CallerProfilePolicy {
            address: crate::ProfileFieldPolicy::Optional,
            ..CallerProfilePolicy::default()
        };
        let updated = a
            .update_caller_profile_versioned(
                caller.id,
                committed.state_version,
                profile.clone(),
                &policy,
                crate::identity::IdentityEditActor::Caller,
                1_700_000_003,
            )
            .unwrap();
        profile.address.city = Some("Other City".into());
        assert!(matches!(
            b.update_caller_profile_versioned(
                caller.id,
                committed.state_version,
                profile,
                &policy,
                crate::identity::IdentityEditActor::Caller,
                1_700_000_004
            ),
            Err(DatabaseError::CallerStateConflict { .. })
        ));
        assert!(updated.state_version > committed.state_version);
    }
    #[test]
    fn d2_independent_callers_and_restart_recovery() {
        let (_temp, mut a, mut b, caller) = board();
        let other = b
            .create_caller(
                b"Other Fixture",
                "synthetic-unusable-hash",
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                true,
                1_700_000_000,
            )
            .unwrap();
        admit(&mut a, &caller, 1).unwrap();
        admit(&mut b, &other, 2).unwrap();
        a.abandon_caller_session(1, 1, 1_700_000_002).unwrap();
        assert!(matches!(
            admit(&mut a, &other, 1),
            Err(DatabaseError::CallerAlreadyOnline)
        ));
        // The test now models the exclusive board-lock owner after both old
        // runtime workers have ended; no wall-clock expiry authorizes recovery.
        assert_eq!(a.recover_caller_sessions(1_700_000_003).unwrap(), 1);
        assert_eq!(a.recover_caller_sessions(1_700_000_004).unwrap(), 0);
        assert!(admit(&mut a, &other, 1).is_ok());
    }
}
