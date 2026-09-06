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

//! Verified, bounded recovery evidence from an exclusively owned surviving board.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OriginState {
    pub endpoint: Endpoint,
    pub next_serial: i64,
    pub held: bool,
}
/// Not deserializable: only a validated native database can supply this evidence.
/// The caller must own both boards exclusively and retire the source before apply.
pub struct RecoveryEvidence {
    origins: Vec<OriginState>,
    accepted: Vec<AcceptedEvidence>,
}
struct AcceptedEvidence {
    queue: String,
    artifact: String,
    publication: String,
    fingerprint: String,
    link: String,
    policy: String,
    occurred_at: i64,
    attempt: i64,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub origins: u32,
    pub accepted: u32,
    pub held: u32,
}
impl RuntimeDatabase {
    pub fn ftn_origin_states(&self) -> Result<Vec<OriginState>, Error> {
        let mut s=self.connection.prepare("SELECT a.domain,a.zone,a.net,a.node,a.point,s.next_serial,s.restored_hold FROM ftn_serials s JOIN ftn_addresses a USING(address_id) ORDER BY a.domain,a.zone,a.net,a.node,a.point LIMIT 1025")?;
        let rows = s
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u16>(1)?,
                    r.get::<_, u16>(2)?,
                    r.get::<_, u16>(3)?,
                    r.get::<_, u16>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, bool>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 1024 {
            return Err(Error::Capacity);
        }
        rows.into_iter()
            .map(|(d, z, n, node, p, next_serial, held)| {
                Ok(OriginState {
                    endpoint: format!("{z}:{n}/{node}.{p}@{d}").parse()?,
                    next_serial,
                    held,
                })
            })
            .collect()
    }
    pub fn ftn_recovery_evidence(&self) -> Result<RecoveryEvidence, Error> {
        let origins = self.ftn_origin_states()?;
        let mut s=self.connection.prepare("SELECT q.queue_id,q.artifact_id,d.publication_id,m.fingerprint,d.link_id,d.policy_digest,a.occurred_at,a.attempt_number FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) JOIN ftn_messages m USING(publication_id) JOIN network_delivery_attempts a USING(queue_id) WHERE q.state='accepted' AND a.outcome='accepted' ORDER BY q.queue_id LIMIT 10001")?;
        let accepted = s
            .query_map([], |r| {
                Ok(AcceptedEvidence {
                    queue: r.get(0)?,
                    artifact: r.get(1)?,
                    publication: r.get(2)?,
                    fingerprint: r.get(3)?,
                    link: r.get(4)?,
                    policy: r.get(5)?,
                    occurred_at: r.get(6)?,
                    attempt: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if accepted.len() > 10000 {
            return Err(Error::Capacity);
        }
        Ok(RecoveryEvidence { origins, accepted })
    }
    /// Monotonic reconciliation; no caller-supplied numeric floor or random reset.
    pub fn reconcile_ftn_recovery(
        &mut self,
        evidence: &RecoveryEvidence,
        principal: &str,
        now: i64,
    ) -> Result<RecoveryResult, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM binkp_link_health WHERE session_id IS NOT NULL)",
            [],
            |r| r.get(0),
        )?;
        if active {
            return Err(Error::Conflict);
        }
        let mut result = RecoveryResult::default();
        for origin in &evidence.origins {
            let aid = address_id(&tx, &origin.endpoint)?;
            tx.execute("INSERT INTO ftn_serials(address_id,next_serial,restored_hold) VALUES(?1,?2,?3) ON CONFLICT(address_id) DO UPDATE SET next_serial=MAX(next_serial,excluded.next_serial),restored_hold=CASE WHEN excluded.next_serial>=next_serial THEN excluded.restored_hold ELSE restored_hold END",params![aid,origin.next_serial,origin.held])?;
            result.origins += 1;
            result.held += u32::from(origin.held);
        }
        for a in &evidence.accepted {
            let matches:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) JOIN ftn_messages m USING(publication_id) WHERE q.queue_id=?1 AND q.artifact_id=?2 AND d.publication_id=?3 AND m.fingerprint=?4 AND d.link_id=?5 AND d.policy_digest=?6 AND q.state<>'accepted')",params![a.queue,a.artifact,a.publication,a.fingerprint,a.link,a.policy],|r|r.get(0))?;
            if !matches {
                continue;
            }
            let prior:Option<(String,i64)>=tx.query_row("SELECT outcome,occurred_at FROM network_delivery_attempts WHERE queue_id=?1 AND attempt_number=?2",params![a.queue,a.attempt],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if prior.is_some_and(|p| p != ("accepted".into(), a.occurred_at)) {
                return Err(Error::Conflict);
            }
            tx.execute("INSERT OR IGNORE INTO network_delivery_attempts(queue_id,occurred_at,outcome,attempt_number) VALUES(?1,?2,'accepted',?3)",params![a.queue,a.occurred_at,a.attempt])?;
            tx.execute("UPDATE network_outbound_queue SET state='accepted',attempts=MAX(attempts,?2),reason='recovered-peer-acceptance',next_attempt=NULL,version=version+1 WHERE queue_id=?1",params![a.queue,a.attempt])?;
            result.accepted += 1;
        }
        audit(&tx, principal, "ftn.recovery-reconciled", now)?;
        event(&tx, "recovery-reconciled", now)?;
        tx.commit()?;
        Ok(result)
    }
}
