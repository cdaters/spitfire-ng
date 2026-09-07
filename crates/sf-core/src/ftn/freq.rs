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

//! Bounded attempts of one durable exact-name FREQ request.
use super::*;
pub const FREQ_MAX_ATTEMPTS: u32 = 3;
pub const FREQ_RETRY_DELAY: i64 = 900;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FreqState {
    Pending,
    Sent,
    Acknowledged,
    Complete,
    Exhausted,
    Held,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreqAttempt {
    pub number: u32,
    pub delivery: String,
    pub created_at: i64,
    pub offered: bool,
    pub acknowledged_at: Option<i64>,
    pub transport_attempts: u32,
    pub active: bool,
    pub held: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreqResponse {
    pub name: String,
    pub area: i64,
    pub file: Option<i64>,
    pub sha256: Option<String>,
    pub received_at: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreqRequest {
    pub request: String,
    pub link: String,
    pub created_at: i64,
    pub version: i64,
    pub state: FreqState,
    pub retry_after: Option<i64>,
    pub attempts: Vec<FreqAttempt>,
    pub responses: Vec<FreqResponse>,
}
fn status(c: &rusqlite::Connection, id: &str) -> Result<FreqRequest> {
    let (link,created_at,version,held):(String,i64,i64,bool)=c.query_row("SELECT d.link_id,d.created_at,r.version,r.held FROM ftn_freq_recovery r JOIN ftn_file_deliveries d ON d.delivery_id=r.request_id WHERE r.request_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let attempts=c.prepare("SELECT a.number,a.delivery_id,d.created_at,a.offered,d.accepted_at,d.attempts,d.session_id IS NOT NULL,d.held FROM ftn_freq_attempts a JOIN ftn_file_deliveries d USING(delivery_id) WHERE a.request_id=?1 ORDER BY a.number")?.query_map([id],|r|Ok(FreqAttempt{number:r.get(0)?,delivery:r.get(1)?,created_at:r.get(2)?,offered:r.get(3)?,acknowledged_at:r.get(4)?,transport_attempts:r.get(5)?,active:r.get(6)?,held:r.get(7)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
    let responses=c.prepare("SELECT name,area_id,file_id,sha256,received_at FROM ftn_freq_inbound WHERE request_id=?1 ORDER BY name")?.query_map([id],|r|Ok(FreqResponse{name:r.get(0)?,area:r.get(1)?,file:r.get(2)?,sha256:r.get(3)?,received_at:r.get(4)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
    let last = attempts.last().ok_or(FileNetworkError::Storage)?;
    if responses.is_empty() {
        return Err(FileNetworkError::Storage);
    }
    let state = if held {
        FreqState::Held
    } else if responses.iter().all(|r| r.file.is_some()) {
        FreqState::Complete
    } else if last.acknowledged_at.is_some() && last.number >= FREQ_MAX_ATTEMPTS
        || last.acknowledged_at.is_none() && last.transport_attempts >= 12 && !last.active
    {
        FreqState::Exhausted
    } else if last.held {
        FreqState::Held
    } else if last.acknowledged_at.is_some() {
        FreqState::Acknowledged
    } else if last.offered {
        FreqState::Sent
    } else {
        FreqState::Pending
    };
    let retry_after = if state == FreqState::Acknowledged {
        last.acknowledged_at
            .and_then(|t| t.checked_add(FREQ_RETRY_DELAY))
    } else {
        None
    };
    Ok(FreqRequest {
        request: id.into(),
        link,
        created_at,
        version,
        state,
        retry_after,
        attempts,
        responses,
    })
}
/// True for other file work; FREQ attempts require an unheld family and at least
/// one still-unanswered exact name present in this immutable attempt's request.
pub(super) fn sendable(c: &rusqlite::Connection, delivery: &str) -> Result<bool> {
    let row:Option<(String,bool,Vec<u8>)>=c.query_row("SELECT a.request_id,r.held,d.request FROM ftn_freq_attempts a JOIN ftn_freq_recovery r USING(request_id) JOIN ftn_file_deliveries d USING(delivery_id) WHERE a.delivery_id=?1",[delivery],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((request, held, bytes)) = row else {
        return Ok(true);
    };
    if held {
        return Ok(false);
    }
    for name in tic::parse_request(&bytes, 32)? {
        let pending:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_inbound WHERE request_id=?1 AND name=?2 AND file_id IS NULL)",params![request,name],|r|r.get(0))?;
        if pending {
            return Ok(true);
        }
    }
    Ok(false)
}
impl RuntimeDatabase {
    pub fn freq_requests(&self) -> Result<Vec<FreqRequest>> {
        self.connection
            .prepare("SELECT request_id FROM ftn_freq_recovery ORDER BY rowid DESC LIMIT 100")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|id| status(&self.connection, &id))
            .collect()
    }
    /// A manual CAS action, not a new request and not a scheduler.
    pub fn retry_ftn_request(
        &mut self,
        policy: &Policy,
        principal: &str,
        request: &str,
        expected: i64,
        now: i64,
    ) -> Result<()> {
        let limits = self.file_network_policy()?;
        if !limits.enabled || !limits.freq {
            return Err(FileNetworkError::Denied);
        }
        policy.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        no_active(&tx)?;
        super::super::bind(&tx, policy)?;
        let current = status(&tx, request)?;
        if current.version != expected {
            return Err(FileNetworkError::Conflict);
        }
        if current.state != FreqState::Acknowledged || current.retry_after.is_none_or(|at| now < at)
        {
            return Err(FileNetworkError::Denied);
        }
        let configured = policy.link(&current.link)?;
        if !configured.enabled || !configured.outbound || !policy.aka(&configured.aka)?.enabled {
            return Err(FileNetworkError::Denied);
        }
        let mut names = vec![];
        for response in &current.responses {
            if response.file.is_some() {
                continue;
            }
            let active: bool = tx.query_row(
                "SELECT active FROM file_areas WHERE area_id=?1",
                [response.area],
                |r| r.get(0),
            )?;
            if !active {
                return Err(FileNetworkError::Denied);
            }
            names.push(response.name.clone());
        }
        let bytes = (names.join("\r\n") + "\r\n").into_bytes();
        tic::parse_request(&bytes, limits.freq_files as usize)?;
        capacity(&tx, 1)?;
        let attempt = super::super::id();
        tx.execute("INSERT INTO ftn_file_deliveries(delivery_id,link_id,name,sha256,size,kind,request,tic_accepted,created_at) SELECT ?2,link_id,name,?3,?4,'freq-request',?5,1,?6 FROM ftn_file_deliveries WHERE delivery_id=?1",params![request,attempt,qwk::digest(&bytes),bytes.len() as i64,bytes,now])?;
        tx.execute(
            "INSERT INTO ftn_freq_attempts(request_id,number,delivery_id) VALUES(?1,?2,?3)",
            params![request, current.attempts.len() as u32 + 1, attempt],
        )?;
        tx.execute(
            "UPDATE ftn_freq_recovery SET version=version+1 WHERE request_id=?1 AND version=?2",
            params![request, expected],
        )?;
        activity(
            &tx,
            Some(&current.link),
            None,
            "freq-reissue-queued",
            0,
            0,
            now,
        )?;
        super::super::audit(&tx, principal, "ftn.freq-retry", now)?;
        tx.commit()?;
        Ok(())
    }
}
/// Stable identity/receipt history excludes mutable restore holds and leases.
/// An older snapshot can be unheld only against an identical surviving family.
pub(crate) fn freq_history(
    c: &rusqlite::Connection,
) -> std::result::Result<Vec<(String, String, bool)>, rusqlite::Error> {
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ftn_freq_recovery')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(vec![]);
    }
    let ids = c
        .prepare("SELECT request_id,held FROM ftn_freq_recovery ORDER BY request_id LIMIT 10001")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if ids.len() > 10000 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    ids.into_iter().map(|(id,held)| {
        let attempts=c.prepare("SELECT a.number,a.delivery_id,a.offered,d.link_id,d.name,d.sha256,d.created_at,d.attempts,d.accepted_at FROM ftn_freq_attempts a JOIN ftn_file_deliveries d USING(delivery_id) WHERE a.request_id=?1 ORDER BY a.number")?.query_map([&id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,bool>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,i64>(6)?,r.get::<_,i64>(7)?,r.get::<_,Option<i64>>(8)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let receipts=c.prepare("SELECT name,area_id,file_id,sha256,size,received_at FROM ftn_freq_inbound WHERE request_id=?1 ORDER BY name")?.query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,Option<i64>>(5)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok((id,serde_json::to_string(&(attempts,receipts)).map_err(|_|rusqlite::Error::InvalidQuery)?,held))
    }).collect()
}
