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
    files: Vec<FileEvidence>,
    freq: Vec<(String, String, bool)>,
}
struct FileEvidence {
    delivery: String,
    publication: Option<String>,
    link: String,
    file: Option<i64>,
    name: String,
    sha: String,
    size: i64,
    kind: String,
    payload: bool,
    tic: bool,
    held: bool,
    attempts: i64,
    accepted_at: Option<i64>,
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
        let has_files: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ftn_file_deliveries')",
            [],
            |r| r.get(0),
        )?;
        let files = if has_files {
            self.connection.prepare("SELECT delivery_id,publication_id,link_id,file_id,name,sha256,size,kind,payload_accepted,tic_accepted,held,attempts,accepted_at FROM ftn_file_deliveries ORDER BY delivery_id LIMIT 10001")?.query_map([], |r| Ok(FileEvidence {
                delivery:r.get(0)?,publication:r.get(1)?,link:r.get(2)?,file:r.get(3)?,name:r.get(4)?,sha:r.get(5)?,size:r.get(6)?,kind:r.get(7)?,payload:r.get(8)?,tic:r.get(9)?,held:r.get(10)?,attempts:r.get(11)?,accepted_at:r.get(12)?
            }))?.collect::<Result<Vec<_>,_>>()?
        } else {
            vec![]
        };
        if files.len() > 10000 {
            return Err(Error::Capacity);
        }
        Ok(RecoveryEvidence {
            origins,
            accepted,
            files,
            freq: files::freq_history(&self.connection)?,
        })
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
        let freq_history = files::freq_history(&tx)?;
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
        for file in &evidence.files {
            let changed = tx.execute("UPDATE ftn_file_deliveries SET payload_accepted=MAX(payload_accepted,?9),tic_accepted=MAX(tic_accepted,?10),held=?11,attempts=MAX(attempts,?12),accepted_at=COALESCE(accepted_at,?13),last_error=NULL,session_id=NULL,payload_offered=0,tic_offered=0,version=version+1 WHERE delivery_id=?1 AND publication_id IS ?2 AND link_id=?3 AND file_id IS ?4 AND name=?5 AND sha256=?6 AND size=?7 AND kind=?8 AND session_id IS NULL", params![file.delivery,file.publication,file.link,file.file,file.name,file.sha,file.size,file.kind,file.payload,file.tic,file.held,file.attempts,file.accepted_at])?;
            if changed == 1 && file.payload && file.tic {
                result.accepted += 1;
            }
        }
        for (request, history, _) in freq_history {
            let held = evidence
                .freq
                .iter()
                .find(|(id, source, _)| id == &request && source == &history)
                .is_none_or(|(_, _, held)| *held);
            tx.execute(
                "UPDATE ftn_freq_recovery SET held=?2,version=version+1 WHERE request_id=?1",
                params![request, held],
            )?;
        }
        audit(&tx, principal, "ftn.recovery-reconciled", now)?;
        event(&tx, "recovery-reconciled", now)?;
        tx.commit()?;
        Ok(result)
    }
}
