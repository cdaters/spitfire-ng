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

//! Durable BinkP admission and queue leases; N3 retains routing and mail authority.
use super::*;
use sf_net::binkp as protocol;
pub(crate) const BINKP_MIGRATION: &str = include_str!("binkp.sql");
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BinkpPolicy {
    pub listener: Option<BinkpListener>,
    pub links: Vec<BinkpLink>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinkpListener {
    pub enabled: bool,
    pub bind: std::net::SocketAddr,
    pub akas: Vec<String>,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinkpAuth {
    #[default]
    RequireCram,
    AllowPlain,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinkpLink {
    pub link: String,
    pub enabled: bool,
    pub outbound: bool,
    pub inbound: bool,
    pub endpoint: Option<String>,
    #[serde(default = "binkp_port")]
    pub port: u16,
    #[serde(default)]
    pub directory: bool,
    pub akas: Vec<String>,
    #[serde(default)]
    pub remote_akas: Vec<Endpoint>,
    #[serde(default)]
    pub auth: BinkpAuth,
    #[serde(default)]
    pub allow_domainless: bool,
}
fn binkp_port() -> u16 {
    24554
}
impl BinkpPolicy {
    pub fn validate(&self, ftn: &Policy) -> Result<(), Error> {
        ftn.validate()?;
        if self.links.len() > 32 {
            return Err(Error::Capacity);
        }
        let mut seen = BTreeSet::new();
        for t in &self.links {
            let l = ftn.link(&t.link)?;
            if !seen.insert(&t.link)
                || t.port == 0
                || t.akas.is_empty()
                || t.akas.len() > 32
                || !t.akas.contains(&l.aka)
            {
                return Err(Error::Policy);
            }
            if t.remote_akas.len() > 31
                || t.remote_akas
                    .iter()
                    .any(|a| a.domain != l.remote.domain || ftn.local(a).is_some())
            {
                return Err(Error::Policy);
            }
            let mut remote = BTreeSet::new();
            if t.remote_akas
                .iter()
                .any(|a| !remote.insert(a) || *a == l.remote)
            {
                return Err(Error::Policy);
            }
            let mut akas = BTreeSet::new();
            for a in &t.akas {
                let a = ftn.aka(a)?;
                if !akas.insert(&a.id)
                    || a.endpoint.domain != l.remote.domain
                    || (t.enabled && !a.enabled)
                {
                    return Err(Error::Policy);
                }
            }
            if let Some(host) = &t.endpoint {
                if !host_valid(host) {
                    return Err(Error::Policy);
                }
            }
            if t.enabled && t.outbound && t.endpoint.is_none() && !t.directory {
                return Err(Error::Policy);
            }
        }
        if let Some(listener) = &self.listener {
            if listener.bind.port() == 0 || listener.akas.is_empty() || listener.akas.len() > 32 {
                return Err(Error::Policy);
            }
            let mut seen = BTreeSet::new();
            for a in &listener.akas {
                if !seen.insert(a) || !ftn.aka(a)?.enabled {
                    return Err(Error::Policy);
                }
            }
            if listener.enabled
                && self
                    .links
                    .iter()
                    .filter(|l| l.enabled && l.inbound)
                    .any(|l| l.akas.iter().any(|a| !listener.akas.contains(a)))
            {
                return Err(Error::Policy);
            }
        }
        Ok(())
    }
    pub fn link(&self, id: &str) -> Result<&BinkpLink, Error> {
        self.links
            .iter()
            .find(|l| l.link == id && l.enabled)
            .ok_or(Error::Denied)
    }
    pub fn digest(&self, ftn: &Policy) -> Result<String, Error> {
        Ok(sf_net::qwk::digest(
            &serde_json::to_vec(&(self, ftn)).map_err(|_| Error::Policy)?,
        ))
    }
}
pub fn host_valid(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && (host.parse::<std::net::IpAddr>().is_ok()
            || host.split('.').all(|l| {
                !l.is_empty()
                    && l.len() <= 63
                    && !l.starts_with('-')
                    && !l.ends_with('-')
                    && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            }))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinkpEndpoint {
    pub host: String,
    pub port: u16,
    pub source: String,
    pub generation: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinkpHealth {
    pub link: String,
    pub active: bool,
    pub last_attempt: Option<i64>,
    pub last_success: Option<i64>,
    pub last_error: Option<String>,
    pub latency_ms: Option<u32>,
    pub failures: u32,
    pub next_attempt: Option<i64>,
    pub held: bool,
    pub queued: u32,
    pub addresses: Vec<Endpoint>,
    pub capabilities: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct BinkpWork {
    pub queue: String,
    pub artifact: String,
    pub size: u64,
    pub time: u64,
    pub version: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinkpMode {
    Poll,
    Test,
}
impl RuntimeDatabase {
    pub fn resolve_binkp(
        &self,
        ftn: &Policy,
        policy: &BinkpPolicy,
        link: &str,
        now: i64,
    ) -> Result<BinkpEndpoint, Error> {
        policy.validate(ftn)?;
        let t = policy.link(link)?;
        if let Some(host) = &t.endpoint {
            return Ok(BinkpEndpoint {
                host: host.clone(),
                port: t.port,
                source: "override".into(),
                generation: None,
            });
        }
        if t.directory {
            if let Some(entry) = self.lookup_ftn_directory(ftn, &ftn.link(link)?.remote, now)? {
                let services: Vec<_> = entry
                    .services
                    .iter()
                    .filter(|s| s.protocol == "IBN")
                    .collect();
                let generic: Vec<_> = entry
                    .services
                    .iter()
                    .filter(|s| s.protocol == "INA" && s.host.is_some())
                    .collect();
                if services.len() == 1 {
                    let service = services[0];
                    let host = service.host.as_deref().or_else(|| {
                        if generic.len() == 1 {
                            generic[0].host.as_deref()
                        } else {
                            None
                        }
                    });
                    if let Some(host) = host {
                        let host = host
                            .strip_prefix('[')
                            .and_then(|h| h.strip_suffix(']'))
                            .unwrap_or(host);
                        if host_valid(host) {
                            return Ok(BinkpEndpoint {
                                host: host.into(),
                                port: service.port.unwrap_or(t.port),
                                source: entry.source,
                                generation: Some(entry.generation),
                            });
                        }
                    }
                }
            }
        }
        Err(Error::Routing)
    }
    /// One durable session per link, two across the daemon. Secrets are never arguments.
    pub fn begin_binkp(
        &mut self,
        ftn: &Policy,
        policy: &BinkpPolicy,
        link: &str,
        generation: &str,
        mode: BinkpMode,
        now: i64,
    ) -> Result<String, Error> {
        policy.validate(ftn)?;
        let t = policy.link(link)?;
        let l = ftn.link(link)?;
        if !l.enabled
            || !ftn.aka(&l.aka)?.enabled
            || (!t.inbound && !t.outbound)
            || generation.is_empty()
            || generation.len() > 64
        {
            return Err(Error::Denied);
        }
        if hub::downstream(&self.connection, link)?.is_some_and(|d| !d.enabled || d.held) {
            return Err(Error::Held);
        }
        let digest = policy.digest(ftn)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, ftn)?;
        tx.execute(
            "INSERT OR IGNORE INTO binkp_link_health(link_id,policy_digest) VALUES(?1,?2)",
            params![link, digest],
        )?;
        let (active,old,held,next,last_test):(Option<String>,String,bool,Option<i64>,Option<i64>)=tx.query_row("SELECT session_id,policy_digest,held,next_attempt,last_test FROM binkp_link_health WHERE link_id=?1",[link],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        let count: u32 = tx.query_row(
            "SELECT COUNT(*) FROM binkp_link_health WHERE session_id IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        if active.is_some() || count >= 2 {
            return Err(Error::Conflict);
        }
        if mode == BinkpMode::Test && last_test.is_some_and(|n| now < n + 60) {
            return Err(Error::Held);
        }
        if old == digest && mode == BinkpMode::Poll && (held || next.is_some_and(|n| now < n)) {
            return Err(Error::Held);
        }
        let session = id();
        tx.execute("UPDATE binkp_link_health SET authenticated=0,session_id=?2,daemon_generation=?3,last_attempt=?4,policy_digest=?5,last_test=CASE WHEN ?6 THEN ?4 ELSE last_test END,held=CASE WHEN policy_digest<>?5 THEN 0 ELSE held END,next_attempt=CASE WHEN policy_digest<>?5 THEN NULL ELSE next_attempt END WHERE link_id=?1",params![link,session,generation,now,digest,mode==BinkpMode::Test])?;
        event(&tx, "binkp-session-started", now)?;
        tx.commit()?;
        Ok(session)
    }
    pub fn claim_binkp(
        &mut self,
        ftn: &Policy,
        policy: &BinkpPolicy,
        store: &dyn NetworkArtifactStore,
        session: &str,
        now: i64,
    ) -> Result<Vec<BinkpWork>, Error> {
        let link: String = self.connection.query_row(
            "SELECT link_id FROM binkp_link_health WHERE session_id=?1",
            [session],
            |r| r.get(0),
        )?;
        let permitted = &policy.link(&link)?.akas;
        let ids=self.connection.prepare("SELECT q.queue_id,q.version FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE d.link_id=?1 AND q.state IN ('pending','ready','retry') AND (q.next_attempt IS NULL OR q.next_attempt<=?2) ORDER BY q.created_at,q.queue_id LIMIT 64")?.query_map(params![link,now],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
        for (queue, version) in &ids {
            match self.build_ftn(store, ftn, queue, *version, now) {
                Ok(_) => (),
                Err(Error::Held | Error::Conflict) => (),
                Err(e) => return Err(e),
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let digest = bind(&tx, ftn)?;
        let mut work = Vec::new();
        let mut bytes = 0;
        for (queue, _) in ids {
            let aka: String = tx.query_row(
                "SELECT aka FROM ftn_routing_decisions WHERE queue_id=?1",
                [&queue],
                |r| r.get(0),
            )?;
            if !permitted.contains(&aka) {
                continue;
            }
            let row:Option<(String,u32,i64,i64)>=tx.query_row("SELECT q.artifact_id,a.byte_length,q.created_at,q.version FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) JOIN network_artifacts a ON a.artifact_id=q.artifact_id JOIN messages m ON m.message_id=(SELECT message_id FROM ftn_messages WHERE publication_id=d.publication_id) WHERE q.queue_id=?1 AND q.state IN ('ready','retry') AND d.policy_digest=?2 AND m.lifecycle_state='active' AND m.state_version=d.message_version AND q.attempts<12",params![queue,digest],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
            if let Some((artifact, size, time, version)) = row {
                if bytes + u64::from(size) > protocol::MAX_SESSION_BYTES {
                    break;
                }
                bytes += u64::from(size);
                tx.execute("INSERT INTO binkp_queue_claims(queue_id,session_id,queue_version) VALUES(?1,?2,?3)",params![queue,session,version])?;
                work.push(BinkpWork {
                    queue,
                    artifact,
                    size: u64::from(size),
                    time: u64::try_from(time).map_err(|_| Error::Rejected)?,
                    version,
                });
            }
        }
        tx.commit()?;
        Ok(work)
    }
    pub fn offered_binkp(&mut self, session: &str, queue: &str) -> Result<(), Error> {
        super::mail::validate_export_identity(
            &self.connection,
            &self.identity_context,
            &self.identity_context.ftn,
            queue,
        )?;
        if self.connection.execute(
            "UPDATE binkp_queue_claims SET offered=1 WHERE session_id=?1 AND queue_id=?2",
            params![session, queue],
        )? != 1
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    pub fn accepted_binkp(&mut self, session: &str, queue: &str, now: i64) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version:i64=tx.query_row("SELECT queue_version FROM binkp_queue_claims WHERE session_id=?1 AND queue_id=?2 AND offered=1",params![session,queue],|r|r.get(0))?;
        // ACK remains true even if policy was restricted after bytes left the board.
        let attempts: u32 = tx.query_row(
            "SELECT attempts FROM network_outbound_queue WHERE queue_id=?1",
            [queue],
            |r| r.get(0),
        )?;
        if tx.execute("UPDATE network_outbound_queue SET state='accepted',attempts=attempts+1,next_attempt=NULL,reason='binkp-peer-accepted',version=version+1 WHERE queue_id=?1 AND version>=?2 AND state<>'accepted' AND attempts<12",params![queue,version])?!=1 {return Err(Error::Conflict);}
        tx.execute("INSERT INTO network_delivery_attempts(queue_id,occurred_at,outcome,attempt_number) VALUES(?1,?2,'accepted',?3)",params![queue,now,attempts+1])?;
        tx.execute("DELETE FROM binkp_queue_claims WHERE queue_id=?1", [queue])?;
        event(&tx, "binkp-outbound-accepted", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn observe_binkp(
        &mut self,
        session: &str,
        addresses: &[Endpoint],
        capabilities: &[String],
        latency: u64,
        now: i64,
    ) -> Result<(), Error> {
        if addresses.len() > 32
            || capabilities.len() > 32
            || capabilities.iter().any(|s| {
                s.is_empty()
                    || s.len() > 32
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
            })
        {
            return Err(Error::Capacity);
        }
        let tx = self.connection.transaction()?;
        let link: String = tx.query_row(
            "SELECT link_id FROM binkp_link_health WHERE session_id=?1",
            [session],
            |r| r.get(0),
        )?;
        tx.execute("DELETE FROM binkp_peer_addresses WHERE link_id=?1", [&link])?;
        tx.execute("DELETE FROM binkp_capabilities WHERE link_id=?1", [&link])?;
        for (n, a) in addresses.iter().enumerate() {
            let a = address_id(&tx, a)?;
            tx.execute(
                "INSERT INTO binkp_peer_addresses VALUES(?1,?2,?3)",
                params![link, n as u32, a],
            )?;
        }
        for (n, c) in capabilities.iter().enumerate() {
            tx.execute(
                "INSERT INTO binkp_capabilities VALUES(?1,?2,?3)",
                params![link, n as u32, c],
            )?;
        }
        tx.execute(
            "UPDATE binkp_link_health SET authenticated=1,latency_ms=?2 WHERE session_id=?1",
            params![session, latency.min(600000) as u32],
        )?;
        event(&tx, "binkp-authenticated-address-matched", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn receive_binkp(
        &mut self,
        store: &dyn NetworkArtifactStore,
        ftn: &Policy,
        policy: &BinkpPolicy,
        session: &str,
        bytes: &[u8],
        now: i64,
    ) -> Result<TossResult, Error> {
        self.receive_binkp_with_areafix(store, ftn, policy, session, bytes, now, None)
    }
    /// The credential verifier is supplied only by protected runtime credential custody.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_binkp_with_areafix(
        &mut self,
        store: &dyn NetworkArtifactStore,
        ftn: &Policy,
        policy: &BinkpPolicy,
        session: &str,
        bytes: &[u8],
        now: i64,
        verifier: Option<&AreaFixVerifier<'_>>,
    ) -> Result<TossResult, Error> {
        let authenticated: bool = self.connection.query_row(
            "SELECT authenticated FROM binkp_link_health WHERE session_id=?1",
            [session],
            |r| r.get(0),
        )?;
        let link: String = self.connection.query_row(
            "SELECT link_id FROM binkp_link_health WHERE session_id=?1",
            [session],
            |r| r.get(0),
        )?;
        let transport = policy.link(&link)?;
        let permitted = &transport.akas;
        let mut remote = vec![ftn.link(&link)?.remote.address];
        for alias in &transport.remote_akas {
            let admitted:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM binkp_peer_addresses p JOIN ftn_addresses a USING(address_id) WHERE p.link_id=?1 AND a.domain=?2 AND a.zone=?3 AND a.net=?4 AND a.node=?5 AND a.point=?6)",params![link,alias.domain.as_str(),alias.address.zone(),alias.address.net(),alias.address.node(),alias.address.point()],|r|r.get(0))?;
            if admitted {
                remote.push(alias.address);
            }
        }
        let result = self.toss_ftn_admitted(
            store,
            ftn,
            &link,
            bytes,
            now,
            Some((permitted, &remote)),
            if authenticated { verifier } else { None },
            authenticated.then_some(policy),
        )?;
        let digest = sf_net::qwk::digest(bytes);
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO binkp_custody_receipts VALUES(?1,?2,?3)",
            params![link, digest, now],
        )?;
        event(&tx, "binkp-inbound-received", now)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn finish_binkp(
        &mut self,
        session: &str,
        failure: Option<protocol::Error>,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        finish(&tx, session, failure, now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn recover_binkp(&mut self, now: i64) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sessions = tx
            .prepare("SELECT session_id FROM binkp_link_health WHERE session_id IS NOT NULL")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for session in sessions {
            finish(&tx, &session, Some(protocol::Error::Interrupted), now)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn binkp_health(&self) -> Result<Vec<BinkpHealth>, Error> {
        let mut rows=self.connection.prepare("SELECT link_id,session_id IS NOT NULL,last_attempt,last_success,last_error,latency_ms,failures,next_attempt,held,(SELECT COUNT(*) FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE d.link_id=h.link_id AND q.state NOT IN ('accepted','cancelled')) FROM binkp_link_health h ORDER BY link_id LIMIT 32")?.query_map([],|r|Ok(BinkpHealth{link:r.get(0)?,active:r.get(1)?,last_attempt:r.get(2)?,last_success:r.get(3)?,last_error:r.get(4)?,latency_ms:r.get(5)?,failures:r.get(6)?,next_attempt:r.get(7)?,held:r.get(8)?,queued:r.get(9)?,addresses:vec![],capabilities:vec![]}))?.collect::<Result<Vec<_>,_>>()?;
        for h in &mut rows {
            let ids = self
                .connection
                .prepare(
                    "SELECT address_id FROM binkp_peer_addresses WHERE link_id=?1 ORDER BY ordinal",
                )?
                .query_map([&h.link], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            h.addresses = ids
                .into_iter()
                .map(|id| endpoint(&self.connection, id))
                .collect::<Result<_, _>>()?;
            h.capabilities = self
                .connection
                .prepare(
                    "SELECT capability FROM binkp_capabilities WHERE link_id=?1 ORDER BY ordinal",
                )?
                .query_map([&h.link], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
        }
        Ok(rows)
    }
    /// Hold unclaimed work. Active custody cannot be revoked by an operator race.
    pub fn hold_binkp(
        &mut self,
        ftn: &Policy,
        principal: &str,
        queue: &str,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, ftn)?;
        if tx.execute("UPDATE network_outbound_queue SET state='held',next_attempt=NULL,reason='binkp-operator-held',version=version+1 WHERE queue_id=?1 AND version=?2 AND state IN ('pending','ready','retry','failed') AND EXISTS(SELECT 1 FROM ftn_routing_decisions WHERE queue_id=?1) AND NOT EXISTS(SELECT 1 FROM binkp_queue_claims WHERE queue_id=?1)",params![queue,expected])? != 1 { return Err(Error::Conflict); }
        audit(&tx, principal, "binkp-queue-hold", now)?;
        event(&tx, "binkp-queue-held", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn release_binkp(
        &mut self,
        ftn: &Policy,
        principal: &str,
        queue: &str,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let digest = bind(&tx, ftn)?;
        let (link,stored,state):(String,String,String)=tx.query_row("SELECT d.link_id,d.policy_digest,q.state FROM ftn_routing_decisions d JOIN network_outbound_queue q USING(queue_id) WHERE q.queue_id=?1 AND q.version=?2",params![queue,expected],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM binkp_queue_claims WHERE queue_id=?1)",
            [queue],
            |r| r.get(0),
        )?;
        if active
            || digest != stored
            || !matches!(
                state.as_str(),
                "pending" | "ready" | "held" | "retry" | "failed"
            )
        {
            return Err(Error::Held);
        }
        // Exhausted histories cannot reuse attempt ordinals. A new corrective decision is required.
        if tx.execute("UPDATE network_outbound_queue SET state=CASE WHEN artifact_id IS NULL THEN 'pending' ELSE 'ready' END,next_attempt=NULL,reason='binkp-reviewed',version=version+1 WHERE queue_id=?1 AND attempts<12",[queue])?!=1 {return Err(Error::Held);}
        tx.execute("UPDATE binkp_link_health SET held=0,next_attempt=NULL,failures=0 WHERE link_id=?1 AND session_id IS NULL",[link])?;
        audit(&tx, principal, "binkp-queue-release", now)?;
        tx.commit()?;
        Ok(())
    }
}
fn finish(
    tx: &Transaction<'_>,
    session: &str,
    failure: Option<protocol::Error>,
    now: i64,
) -> Result<(), Error> {
    let has_files: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ftn_file_deliveries')",
        [],
        |r| r.get(0),
    )?;
    if has_files {
        tx.execute(
            "UPDATE binkp_link_health SET freq_files=0,freq_bytes=0 WHERE session_id=?1",
            [session],
        )?;
        tx.execute("UPDATE ftn_file_deliveries SET session_id=NULL,payload_offered=0,tic_offered=0,last_error='transfer-incomplete',version=version+1 WHERE session_id=?1 AND accepted_at IS NULL", [session])?;
    }
    let has_auth:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('binkp_link_health') WHERE name='authenticated')",[],|r|r.get(0))?;
    if has_auth {
        tx.execute(
            "UPDATE binkp_link_health SET authenticated=0 WHERE session_id=?1",
            [session],
        )?;
    }
    let (link, failures): (String, u32) = tx.query_row(
        "SELECT link_id,failures FROM binkp_link_health WHERE session_id=?1",
        [session],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let claims=tx.prepare("SELECT q.queue_id,q.attempts,q.created_at,q.state FROM binkp_queue_claims c JOIN network_outbound_queue q USING(queue_id) WHERE c.session_id=?1")?.query_map([session],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?)))?.collect::<Result<Vec<_>,_>>()?;
    let failed = failure.or((!claims.is_empty()).then_some(protocol::Error::Interrupted));
    let held = matches!(
        failed,
        Some(
            protocol::Error::Authentication
                | protocol::Error::Address
                | protocol::Error::Malformed
                | protocol::Error::Limit
                | protocol::Error::Custody
        )
    );
    let count = if failed.is_some() {
        (failures + 1).min(12)
    } else {
        0
    };
    let delay = (300i64.saturating_mul(1i64 << count.saturating_sub(1).min(7))).min(21600)
        + i64::from(rand::random::<u8>() % 31);
    for (queue, attempts, created, state) in claims {
        let number = (attempts + 1).min(12);
        let outcome = if number >= 12 || now.saturating_sub(created) > 7 * 86400 {
            "failed"
        } else {
            "retry"
        };
        let next_state = if held || state == "held" {
            "held"
        } else {
            outcome
        };
        tx.execute("UPDATE network_outbound_queue SET state=?2,attempts=?3,next_attempt=?4,reason='binkp-unacknowledged',version=version+1 WHERE queue_id=?1",params![queue,next_state,number,now+delay])?;
        tx.execute("INSERT INTO network_delivery_attempts(queue_id,occurred_at,outcome,attempt_number) VALUES(?1,?2,?3,?4)",params![queue,now,outcome,number])?;
    }
    tx.execute(
        "DELETE FROM binkp_queue_claims WHERE session_id=?1",
        [session],
    )?;
    let reason = failed.map(|e| {
        serde_json::to_value(e)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "interrupted".into())
    });
    tx.execute("UPDATE binkp_link_health SET session_id=NULL,daemon_generation=NULL,last_success=CASE WHEN ?2 IS NULL THEN ?3 ELSE last_success END,last_error=?2,failures=?4,next_attempt=?5,held=?6 WHERE link_id=?1",params![link,reason,now,count,failed.map(|_|now+delay),held || count>=12])?;
    if let Some(e) = failed {
        event(
            tx,
            match e {
                protocol::Error::Authentication => "binkp-authentication-failed",
                protocol::Error::Address => "binkp-address-rejected",
                _ if held => "binkp-link-held",
                _ => "binkp-link-retry",
            },
            now,
        )?;
    }
    event(
        tx,
        if failed.is_some() {
            "binkp-session-failed"
        } else {
            "binkp-session-complete"
        },
        now,
    )?;
    Ok(())
}
