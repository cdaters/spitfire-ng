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

//! Native CircuitNET NG policy, publication, queue and offline receipt authority.
pub mod control;
pub mod live;
use crate::{network::NetworkArtifactStore, RuntimeDatabase};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sf_net::circuitnet::{self as wire, Batch, Message, MessageId, Receipt};
pub use sf_net::circuitnet::{Codename, NetworkId, Node, NodeId, Role, Topology};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("CircuitNET policy does not authorize this operation")]
    Policy,
    #[error("CircuitNET authority changed or identity conflicts")]
    Conflict,
    #[error("CircuitNET has no eligible pending deliveries")]
    Empty,
    #[error("CircuitNET capacity exhausted")]
    Capacity,
    #[error(transparent)]
    Codec(#[from] wire::Error),
    #[error("CircuitNET database operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("CircuitNET stored configuration is invalid")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Native(#[from] crate::MessageError),
    #[error(transparent)]
    Identity(#[from] crate::IdentityError),
    #[error(transparent)]
    Artifact(#[from] crate::network::NetworkError),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub network: NetworkId,
    pub display_name: String,
    pub local: NodeId,
    pub enabled: bool,
    pub trusted_offline: bool,
    pub topology: Topology,
}
impl Profile {
    pub fn validate(&self) -> Result<(), Error> {
        self.topology.validate()?;
        self.topology.node(&self.local)?;
        if self.display_name.is_empty()
            || self.display_name.len() > 60
            || self.display_name.chars().any(char::is_control)
        {
            return Err(Error::Policy);
        }
        Ok(())
    }
    fn neighbor(&self, id: &NodeId) -> Result<(), Error> {
        if !self.topology.neighbors(&self.local)?.contains(id) {
            return Err(Error::Policy);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub codename: Codename,
    pub conference: i64,
    pub send: bool,
    pub receive: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dossier {
    pub neighbor: NodeId,
    pub codename: Codename,
    pub subscribed: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueItem {
    pub destination: Option<NodeId>,
    pub reason: Option<String>,
    pub id: String,
    pub identity: MessageId,
    pub neighbor: NodeId,
    pub state: String,
    pub attempts: u32,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub profile: Profile,
    pub version: i64,
    pub mappings: Vec<(Mapping, i64)>,
    pub dossiers: Vec<Dossier>,
    pub pending: u32,
    pub accepted: u32,
    pub receipts: u32,
}
#[derive(Clone, Debug)]
pub struct Prepared {
    pub artifact: String,
    pub bytes: Vec<u8>,
    pub messages: usize,
}
#[derive(Clone, Debug)]
pub struct Imported {
    pub imported: u32,
    pub duplicates: u32,
    pub replay: bool,
    pub receipt: Vec<u8>,
}

fn profile(conn: &Connection, network: &NetworkId) -> Result<(Profile, i64), Error> {
    let (json, version): (String, i64) = conn.query_row(
        "SELECT configuration,version FROM circuitnet_profiles WHERE network=?1",
        [network.as_str()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let p: Profile = serde_json::from_str(&json)?;
    p.validate()?;
    Ok((p, version))
}
fn audit(
    conn: &Connection,
    network: &NetworkId,
    actor: &str,
    operation: &str,
    now: i64,
) -> Result<(), Error> {
    if actor.is_empty() || actor.len() > 128 || actor.chars().any(char::is_control) {
        return Err(Error::Policy);
    }
    conn.execute(
        "INSERT INTO circuitnet_changes(network,operation,actor,occurred_at) VALUES(?1,?2,?3,?4)",
        params![network.as_str(), operation, actor, now],
    )?;
    Ok(())
}
fn capacity(conn: &Connection) -> Result<(), Error> {
    let rows:i64=conn.query_row("SELECT (SELECT COUNT(*) FROM circuitnet_messages)+(SELECT COUNT(*) FROM circuitnet_receipts)+(SELECT COUNT(*) FROM circuitnet_changes)+(SELECT COUNT(*) FROM circuitnet_deliveries)+(SELECT COUNT(*) FROM circuitnet_imports)+(SELECT COUNT(*) FROM circuitnet_batches)+(SELECT COUNT(*) FROM circuitnet_controls)",[],|r|r.get(0))?;
    if rows >= 100_000 {
        return Err(Error::Capacity);
    }
    Ok(())
}
fn subscribed(
    conn: &Connection,
    p: &Profile,
    neighbor: &NodeId,
    code: &Codename,
) -> Result<bool, Error> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3 AND subscribed=1)",params![p.network.as_str(),neighbor.as_str(),code.as_str()],|r|r.get(0))?)
}
fn area(conn: &Connection, p: &Profile, code: &Codename) -> Result<Option<Mapping>, Error> {
    Ok(conn.query_row("SELECT conference_id,send,receive FROM circuitnet_mappings WHERE network=?1 AND codename=?2",params![p.network.as_str(),code.as_str()],|r|Ok(Mapping{codename:code.clone(),conference:r.get(0)?,send:r.get(1)?,receive:r.get(2)?})).optional()?)
}
fn retain(
    conn: &Connection,
    store: &dyn NetworkArtifactStore,
    bytes: &[u8],
    now: i64,
) -> Result<String, Error> {
    let (usage, count) = store.usage()?;
    if usage + bytes.len() as u64 > 512 * 1024 * 1024 || count >= 20_000 {
        return Err(Error::Capacity);
    }
    let id = wire::digest(bytes);
    if store.preserve(bytes)? != id {
        return Err(Error::Conflict);
    }
    conn.execute("INSERT OR IGNORE INTO network_artifacts(artifact_id,byte_length,created_at,state) VALUES(?1,?2,?3,'complete')",params![id,bytes.len() as i64,now])?;
    let complete: bool = conn.query_row(
        "SELECT state='complete' AND byte_length=?2 FROM network_artifacts WHERE artifact_id=?1",
        params![id, bytes.len() as i64],
        |r| r.get(0),
    )?;
    if !complete {
        return Err(Error::Conflict);
    }
    Ok(id)
}
impl RuntimeDatabase {
    pub fn circuitnet_configure(
        &mut self,
        actor: &str,
        value: &Profile,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        value.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let old = profile(&tx, &value.network);
        match old {
            Ok((old, version)) => {
                if version != expected {
                    return Err(Error::Conflict);
                }
                let retained: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM circuitnet_messages WHERE network=?1 UNION ALL SELECT 1 FROM circuitnet_controls WHERE network=?1 UNION ALL SELECT 1 FROM circuitnet_destinations WHERE network=?1)",
                    [value.network.as_str()],
                    |r| r.get(0),
                )?;
                if retained
                    && (old.local != value.local
                        || old.topology != value.topology
                        || old.trusted_offline != value.trusted_offline)
                {
                    return Err(Error::Conflict);
                }
                let neighbors = value.topology.neighbors(&value.local)?;
                let rows: Vec<String> = tx
                    .prepare("SELECT DISTINCT neighbor FROM circuitnet_dossiers WHERE network=?1")?
                    .query_map([value.network.as_str()], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                if rows
                    .iter()
                    .any(|n| !neighbors.iter().any(|id| id.as_str() == n))
                {
                    return Err(Error::Conflict);
                }
            }
            Err(Error::Sqlite(rusqlite::Error::QueryReturnedNoRows)) if expected == 0 => {
                let count: i64 =
                    tx.query_row("SELECT COUNT(*) FROM circuitnet_profiles", [], |r| r.get(0))?;
                if count >= 8 {
                    return Err(Error::Capacity);
                }
            }
            Err(e) => return Err(e),
        }
        tx.execute("INSERT INTO circuitnet_profiles VALUES(?1,?2,1) ON CONFLICT(network) DO UPDATE SET configuration=excluded.configuration,version=version+1",params![value.network.as_str(),serde_json::to_string(value)?])?;
        audit(&tx, &value.network, actor, "profile", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_map(
        &mut self,
        actor: &str,
        network: &NetworkId,
        value: &Mapping,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        profile(&tx, network)?;
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM message_conferences WHERE conference_id=?1 AND active=1 AND public_only=1)",[value.conference],|r|r.get(0))?;
        if !valid {
            return Err(Error::Policy);
        }
        let old:Option<(i64,i64)>=tx.query_row("SELECT conference_id,version FROM circuitnet_mappings WHERE network=?1 AND codename=?2",params![network.as_str(),value.codename.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if old.as_ref().map_or(0, |v| v.1) != expected {
            return Err(Error::Conflict);
        }
        if old.as_ref().is_some_and(|v| v.0 != value.conference) {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO circuitnet_mappings VALUES(?1,?2,?3,?4,?5,1) ON CONFLICT(network,codename) DO UPDATE SET send=excluded.send,receive=excluded.receive,version=version+1",params![network.as_str(),value.codename.as_str(),value.conference,value.send,value.receive])?;
        audit(&tx, network, actor, "mapping", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_subscribe(
        &mut self,
        actor: &str,
        network: &NetworkId,
        value: &Dossier,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(&value.neighbor)?;
        let version:i64=tx.query_row("SELECT version FROM circuitnet_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3",params![network.as_str(),value.neighbor.as_str(),value.codename.as_str()],|r|r.get(0)).optional()?.unwrap_or(0);
        if version != value.version {
            return Err(Error::Conflict);
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM circuitnet_dossiers WHERE network=?1",
            [network.as_str()],
            |r| r.get(0),
        )?;
        if count >= 4096 && version == 0 {
            return Err(Error::Capacity);
        }
        tx.execute("INSERT INTO circuitnet_dossiers VALUES(?1,?2,?3,?4,1) ON CONFLICT(network,neighbor,codename) DO UPDATE SET subscribed=excluded.subscribed,version=version+1",params![network.as_str(),value.neighbor.as_str(),value.codename.as_str(),value.subscribed])?;
        if !value.subscribed {
            tx.execute("UPDATE network_outbound_queue SET state='held',reason='circuitnet-unsubscribed',version=version+1 WHERE state NOT IN('accepted','cancelled') AND queue_id IN (SELECT d.queue_id FROM circuitnet_deliveries d JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND d.neighbor=?2 AND m.codename=?3 AND m.destination IS NULL)",params![network.as_str(),value.neighbor.as_str(),value.codename.as_str()])?;
        }
        audit(
            &tx,
            network,
            actor,
            if value.subscribed {
                "subscribe"
            } else {
                "unsubscribe"
            },
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_status(&self, network: &NetworkId) -> Result<Status, Error> {
        let (p, version) = profile(&self.connection, network)?;
        let mappings=self.connection.prepare("SELECT codename,conference_id,send,receive,version FROM circuitnet_mappings WHERE network=?1 ORDER BY codename")?.query_map([network.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(c,conference,send,receive,v)|Ok((Mapping{codename:Codename::new(&c)?,conference,send,receive},v))).collect::<Result<Vec<_>,Error>>()?;
        let dossiers=self.connection.prepare("SELECT neighbor,codename,subscribed,version FROM circuitnet_dossiers WHERE network=?1 ORDER BY neighbor,codename")?.query_map([network.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get(2)?,r.get(3)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(n,c,subscribed,version)|Ok(Dossier{neighbor:NodeId::new(&n)?,codename:Codename::new(&c)?,subscribed,version})).collect::<Result<Vec<_>,Error>>()?;
        let(pending,accepted):(u32,u32)=self.connection.query_row("SELECT COALESCE(SUM(q.state NOT IN('accepted','cancelled')),0),COALESCE(SUM(q.state='accepted'),0) FROM network_outbound_queue q JOIN circuitnet_deliveries d USING(queue_id) WHERE network=?1",[network.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let receipts = self.connection.query_row(
            "SELECT COUNT(*) FROM circuitnet_receipts WHERE network=?1",
            [network.as_str()],
            |r| r.get(0),
        )?;
        Ok(Status {
            profile: p,
            version,
            mappings,
            dossiers,
            pending,
            accepted,
            receipts,
        })
    }
    pub fn circuitnet_queue(
        &self,
        network: &NetworkId,
        after: &str,
    ) -> Result<Vec<QueueItem>, Error> {
        self.connection.prepare("SELECT q.queue_id,d.identity,d.neighbor,q.state,q.attempts,q.version,m.destination,q.reason FROM network_outbound_queue q JOIN circuitnet_deliveries d USING(queue_id) JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND q.queue_id>?2 ORDER BY q.queue_id LIMIT 100")?.query_map(params![network.as_str(),after],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get::<_,Option<String>>(6)?,r.get(7)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(id,i,n,state,attempts,version,destination,reason)|Ok(QueueItem{destination:destination.map(|s|NodeId::new(&s)).transpose()?,reason,id,identity:MessageId::new(&i)?,neighbor:NodeId::new(&n)?,state,attempts,version})).collect()
    }
}

struct StoredMessage {
    mid: i64,
    code: String,
    origin: String,
    reply: Option<String>,
    path: String,
    author: String,
    subject: Vec<u8>,
    body: Vec<u8>,
    encoding: String,
    timestamp: i64,
}
fn load_message(
    conn: &Connection,
    p: &Profile,
    identity: &MessageId,
) -> Result<(Message, i64), Error> {
    let StoredMessage{mid,code,origin,reply,path,author,subject,body,encoding,timestamp}=conn.query_row("SELECT c.message_id,c.codename,c.origin,c.reply,c.path,m.author_name,p.subject,p.body,p.encoding,m.created_at FROM circuitnet_messages c JOIN messages m USING(message_id) JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE c.network=?1 AND c.identity=?2",params![p.network.as_str(),identity.as_str()],|r|Ok(StoredMessage{mid:r.get(0)?,code:r.get(1)?,origin:r.get(2)?,reply:r.get(3)?,path:r.get(4)?,author:r.get(5)?,subject:r.get(6)?,body:r.get(7)?,encoding:r.get(8)?,timestamp:r.get(9)?}))?;
    let decode = |bytes: Vec<u8>| -> Result<String, Error> {
        match encoding.as_str() {
            "cp437" => Ok(crate::file_maintenance::decode_cp437(&bytes)),
            "utf8" => String::from_utf8(bytes).map_err(|_| Error::Policy),
            _ => Err(Error::Policy),
        }
    };
    Ok((
        Message {
            id: identity.clone(),
            origin: NodeId::new(&origin)?,
            codename: Codename::new(&code)?,
            author,
            subject: decode(subject)?,
            body: decode(body)?,
            timestamp,
            reply: reply.map(|r| MessageId::new(&r)).transpose()?,
            path: serde_json::from_str(&path)?,
            destination: conn
                .query_row(
                    "SELECT destination FROM circuitnet_messages WHERE network=?1 AND identity=?2",
                    params![p.network.as_str(), identity.as_str()],
                    |r| r.get::<_, Option<String>>(0),
                )?
                .map(|v| NodeId::new(&v))
                .transpose()?,
        },
        mid,
    ))
}
fn publish(
    conn: &Connection,
    p: &Profile,
    m: &Message,
    mid: i64,
    ingress: Option<&NodeId>,
    now: i64,
) -> Result<(), Error> {
    capacity(conn)?;
    let version: i64 = conn.query_row(
        "SELECT state_version FROM messages WHERE message_id=?1",
        [mid],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO circuitnet_messages VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            p.network.as_str(),
            m.id.as_str(),
            mid,
            m.codename.as_str(),
            m.origin.as_str(),
            ingress.map(NodeId::as_str),
            m.reply.as_ref().map(MessageId::as_str),
            serde_json::to_string(&m.path)?,
            m.fingerprint()?,
            version,
            now,
            m.destination.as_ref().map(NodeId::as_str)
        ],
    )?;
    let neighbors = if let Some(destination) = &m.destination {
        let path = p.topology.path(&p.local, destination)?;
        if ingress.is_some()
            && !p.topology.node(&p.local)?.role.can_transit()
            && destination != &p.local
        {
            return Err(Error::Policy);
        }
        audit(
            conn,
            &p.network,
            p.local.as_str(),
            &format!(
                "directed-route:{}:{}:{}",
                m.id,
                destination,
                path.get(1).map_or("local", NodeId::as_str)
            ),
            now,
        )?;
        path.get(1).cloned().into_iter().collect()
    } else {
        p.topology.neighbors(&p.local)?
    };
    for neighbor in neighbors {
        if m.path.contains(&neighbor)
            || ingress == Some(&neighbor)
            || (m.destination.is_none() && !subscribed(conn, p, &neighbor, &m.codename)?)
        {
            continue;
        }
        capacity(conn)?;
        let pending:i64=conn.query_row("SELECT COUNT(*) FROM circuitnet_deliveries d JOIN network_outbound_queue q USING(queue_id) WHERE network=?1 AND neighbor=?2 AND state NOT IN('accepted','cancelled')",params![p.network.as_str(),neighbor.as_str()],|r|r.get(0))?;
        if pending >= 1000 {
            return Err(Error::Capacity);
        }
        let q = format!("{:032x}", rand::random::<u128>());
        conn.execute(
            "INSERT INTO network_queue_work VALUES(?1,'circuitnet')",
            [&q],
        )?;
        conn.execute(
            "INSERT INTO circuitnet_deliveries VALUES(?1,?2,?3,?4,?5)",
            params![q, p.network.as_str(), m.id.as_str(), neighbor.as_str(), now],
        )?;
        conn.execute("INSERT INTO network_outbound_queue(queue_id,state,created_at,reserved_bytes) VALUES(?1,'pending',?2,?3)",params![q,now,m.body.len() as i64+4096])?;
        crate::identity::freeze_sender(conn, &q, mid, m.author.as_bytes(), wire::FORMAT)?;
    }
    Ok(())
}
fn eligible(conn: &Connection, p: &Profile, q: &str) -> Result<bool, Error> {
    let (identity,neighbor,active):(String,String,bool)=conn.query_row("SELECT d.identity,d.neighbor,m.lifecycle_state='active' AND m.state_version=c.message_version FROM circuitnet_deliveries d JOIN circuitnet_messages c USING(network,identity) JOIN messages m USING(message_id) WHERE queue_id=?1 AND d.network=?2",params![q,p.network.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if !p.enabled || !active {
        return Ok(false);
    }
    let n = NodeId::new(&neighbor)?;
    p.neighbor(&n)?;
    let (m, mid) = load_message(conn, p, &MessageId::new(&identity)?)?;
    if m.destination.is_none() && !subscribed(conn, p, &n, &m.codename)? {
        return Ok(false);
    }
    if let Some(destination) = &m.destination {
        if p.topology.path(&p.local, destination)?.get(1) != Some(&n) {
            return Ok(false);
        }
    }
    let transit = m.destination.is_some() && m.origin != p.local;
    if transit {
        if !p.topology.node(&p.local)?.role.can_transit() {
            return Ok(false);
        }
        if area(conn, p, &m.codename)?.is_some_and(|m| !m.send) {
            return Ok(false);
        }
    } else if let Some(mapping) = area(conn, p, &m.codename)? {
        if !mapping.send {
            return Ok(false);
        }
        let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN message_conferences c USING(conference_id) WHERE m.message_id=?1 AND c.conference_id=?2 AND c.active=1 AND c.public_only=1 AND m.visibility='public')",params![mid,mapping.conference],|r|r.get(0))?;
        if !valid {
            return Ok(false);
        }
    } else if !p.topology.node(&p.local)?.role.can_transit() {
        return Ok(false);
    }
    let fp: String = conn.query_row(
        "SELECT fingerprint FROM circuitnet_messages WHERE network=?1 AND identity=?2",
        params![p.network.as_str(), identity],
        |r| r.get(0),
    )?;
    if fp != m.fingerprint()? {
        return Ok(false);
    }
    crate::identity::freeze_sender(conn, q, mid, m.author.as_bytes(), wire::FORMAT)?;
    Ok(true)
}

impl RuntimeDatabase {
    /// Bounded explicit scanner; no daemon scheduler or cross-adapter bridge.
    pub fn circuitnet_scan(
        &mut self,
        network: &NetworkId,
        after: i64,
        now: i64,
    ) -> Result<(u32, i64), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        let rows=tx.prepare("SELECT m.message_id,a.codename,m.author_name,p.subject,p.body,p.encoding,m.created_at,m.parent_message_id FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) JOIN message_conferences c USING(conference_id) JOIN circuitnet_mappings a USING(conference_id) WHERE a.network=?1 AND a.send=1 AND c.active=1 AND c.public_only=1 AND m.origin_kind='native' AND m.visibility='public' AND m.audience_kind='all-callers' AND m.lifecycle_state='active' AND p.content_kind='standard' AND m.message_id>?2 AND NOT EXISTS(SELECT 1 FROM circuitnet_destinations dst WHERE dst.network=?1 AND dst.message_id=m.message_id AND dst.accepted=0) AND NOT EXISTS(SELECT 1 FROM circuitnet_messages n WHERE n.network=?1 AND n.message_id=m.message_id) ORDER BY m.message_id LIMIT 100")?.query_map(params![network.as_str(),after],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?,r.get::<_,Vec<u8>>(4)?,r.get::<_,String>(5)?,r.get::<_,i64>(6)?,r.get::<_,Option<i64>>(7)?)))?.collect::<Result<Vec<_>,_>>()?;
        let mut count = 0;
        let mut cursor = after;
        for (mid, code, author, subject, body, encoding, timestamp, parent) in rows {
            cursor = mid;
            let scope = format!("circuitnet:{network}:{code}");
            if crate::identity::validate_destination(
                &tx,
                &self.identity_context,
                mid,
                crate::PostingIdentityPolicy::HandleAllowed,
                &scope,
            )
            .is_err()
            {
                continue;
            }
            // No legacy-unclassified history is granted a newly enabled destination.
            let has_proof: bool = tx.query_row(
                "SELECT identity_proof IS NOT NULL FROM messages WHERE message_id=?1",
                [mid],
                |r| r.get(0),
            )?;
            if !has_proof {
                continue;
            }
            let decode = |bytes: Vec<u8>| -> Result<String, Error> {
                match encoding.as_str() {
                    "cp437" => Ok(crate::file_maintenance::decode_cp437(&bytes)),
                    "utf8" => String::from_utf8(bytes).map_err(|_| Error::Policy),
                    _ => Err(Error::Policy),
                }
            };
            let reply=parent.map(|mid|tx.query_row("SELECT identity FROM circuitnet_messages WHERE network=?1 AND message_id=?2 AND codename=?3",params![network.as_str(),mid,code],|r|r.get::<_,String>(0)).optional()).transpose()?.flatten().map(|id|MessageId::new(&id)).transpose()?;
            let m = Message {
                id: MessageId::new(&format!("{}:{:032x}", p.local, rand::random::<u128>()))?,
                origin: p.local.clone(),
                codename: Codename::new(&code)?,
                author,
                subject: decode(subject)?,
                body: decode(body)?,
                timestamp,
                reply,
                path: vec![p.local.clone()],
                destination: tx.query_row("SELECT destination FROM circuitnet_destinations WHERE network=?1 AND message_id=?2",params![network.as_str(),mid],|r|r.get::<_,String>(0)).optional()?.map(|s|NodeId::new(&s)).transpose()?,
            };
            m.validate()?;
            publish(&tx, &p, &m, mid, None, now)?;
            count += 1;
        }
        tx.commit()?;
        Ok((count, cursor))
    }
    pub fn circuitnet_prepare(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        neighbor: &NodeId,
        now: i64,
    ) -> Result<Prepared, Error> {
        if !profile(&self.connection, network)?.0.trusted_offline {
            return Err(Error::Policy);
        }
        self.circuitnet_prepare_neighbor(store, network, neighbor, now)
    }
    /// The host must establish authenticated direct-neighbor authority before calling.
    pub fn circuitnet_prepare_neighbor(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        neighbor: &NodeId,
        now: i64,
    ) -> Result<Prepared, Error> {
        self.circuitnet_prepare_capable_neighbor(store, network, neighbor, true, now)
    }
    pub fn circuitnet_prepare_capable_neighbor(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        neighbor: &NodeId,
        directed: bool,
        now: i64,
    ) -> Result<Prepared, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(neighbor)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        // Finish an existing immutable offer before adding newly posted traffic.
        let prior:Option<String>=tx.query_row("SELECT q.artifact_id FROM network_outbound_queue q JOIN circuitnet_deliveries d USING(queue_id) JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND d.neighbor=?2 AND q.state IN('ready','retry') AND q.artifact_id IS NOT NULL AND q.attempts<12 AND (?3 OR m.destination IS NULL) AND (?3 OR NOT EXISTS(SELECT 1 FROM circuitnet_batch_members bm JOIN circuitnet_deliveries bd USING(queue_id) JOIN circuitnet_messages cm USING(network,identity) WHERE bm.artifact=q.artifact_id AND cm.destination IS NOT NULL)) ORDER BY m.message_id LIMIT 1",params![network.as_str(),neighbor.as_str(),directed],|r|r.get(0)).optional()?;
        let candidates:Vec<(String,String)>=tx.prepare("SELECT q.queue_id,d.identity FROM network_outbound_queue q JOIN circuitnet_deliveries d USING(queue_id) JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND d.neighbor=?2 AND q.state IN('pending','ready','retry') AND q.attempts<12 AND (?3 IS NULL OR q.artifact_id=?3) AND (?4 OR m.destination IS NULL) AND (?4 OR q.artifact_id IS NULL OR NOT EXISTS(SELECT 1 FROM circuitnet_batch_members bm JOIN circuitnet_deliveries bd USING(queue_id) JOIN circuitnet_messages cm USING(network,identity) WHERE bm.artifact=q.artifact_id AND cm.destination IS NOT NULL)) ORDER BY m.message_id LIMIT 1000")?.query_map(params![network.as_str(),neighbor.as_str(),prior,directed],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?;
        let mut messages = vec![];
        let mut encoded_bytes = 1024;
        let mut queues = vec![];
        for (q, id) in candidates {
            if !eligible(&tx, &p, &q)? {
                tx.execute("UPDATE network_outbound_queue SET state='held',reason='circuitnet-policy',version=version+1 WHERE queue_id=?1",[&q])?;
                continue;
            }
            let (m, mid) = load_message(&tx, &p, &MessageId::new(&id)?)?;
            if crate::identity::validate_destination(
                &tx,
                &self.identity_context,
                mid,
                crate::PostingIdentityPolicy::HandleAllowed,
                &format!("circuitnet:{network}:{}", m.codename),
            )
            .is_err()
            {
                tx.execute("UPDATE network_outbound_queue SET state='held',reason='circuitnet-identity',version=version+1 WHERE queue_id=?1",[&q])?;
                continue;
            }
            let member_bytes = serde_json::to_vec(&m)?.len() + 1;
            if encoded_bytes + member_bytes > wire::MAX_ARTIFACT {
                break;
            }
            encoded_bytes += member_bytes;
            messages.push(m);
            queues.push(q);
            if messages.len() == wire::MAX_MESSAGES {
                break;
            }
        }
        if messages.is_empty() {
            tx.commit()?;
            return Err(Error::Empty);
        }
        let bytes = Batch {
            format: wire::FORMAT.into(),
            version: 1,
            network: network.clone(),
            sender: p.local.clone(),
            neighbor: neighbor.clone(),
            messages,
        }
        .encode()?;
        let artifact = retain(&tx, store, &bytes, now)?;
        tx.execute(
            "INSERT OR IGNORE INTO circuitnet_batches VALUES(?1,?2,?3,?4)",
            params![artifact, network.as_str(), neighbor.as_str(), now],
        )?;
        for (i, q) in queues.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO circuitnet_batch_members VALUES(?1,?2,?3)",
                params![artifact, q, i as i64],
            )?;
            tx.execute("UPDATE network_outbound_queue SET state='ready',artifact_id=?2,version=version+1 WHERE queue_id=?1",params![q,artifact])?;
        }
        tx.commit()?;
        Ok(Prepared {
            artifact,
            bytes,
            messages: queues.len(),
        })
    }
    /// Only an operator who has established offline custody supplies expected_neighbor.
    pub fn circuitnet_import(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        expected_neighbor: &NodeId,
        bytes: &[u8],
        now: i64,
    ) -> Result<Imported, Error> {
        if !profile(&self.connection, network)?.0.trusted_offline {
            return Err(Error::Policy);
        }
        self.circuitnet_import_neighbor(store, network, expected_neighbor, bytes, now)
    }
    /// The host must establish authenticated direct-neighbor authority before calling.
    pub fn circuitnet_import_neighbor(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        expected_neighbor: &NodeId,
        bytes: &[u8],
        now: i64,
    ) -> Result<Imported, Error> {
        let result = self.circuitnet_import_inner(store, network, expected_neighbor, bytes, now);
        if result.is_err()
            && Batch::decode(bytes)
                .is_ok_and(|b| b.messages.iter().any(|m| m.destination.is_some()))
        {
            audit(
                &self.connection,
                network,
                expected_neighbor.as_str(),
                "directed-rejected",
                now,
            )?;
        }
        result
    }
    fn circuitnet_import_inner(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        expected_neighbor: &NodeId,
        bytes: &[u8],
        now: i64,
    ) -> Result<Imported, Error> {
        let _permit = store.admit_import()?;
        let batch = Batch::decode(bytes)?;
        let artifact = wire::digest(bytes);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(expected_neighbor)?;
        if batch.network != *network
            || batch.sender != *expected_neighbor
            || batch.neighbor != p.local
        {
            return Err(Error::Policy);
        }
        let seen:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_imports WHERE network=?1 AND neighbor=?2 AND artifact=?3)",params![network.as_str(),expected_neighbor.as_str(),artifact],|r|r.get(0))?;
        let mut imported = 0;
        let mut duplicates = 0;
        if !seen {
            if !p.enabled {
                return Err(Error::Policy);
            }
            for m in &batch.messages {
                if p.topology.path(&m.origin, expected_neighbor)? != m.path {
                    return Err(Error::Policy);
                }
                let old:Option<String>=tx.query_row("SELECT fingerprint FROM circuitnet_messages WHERE network=?1 AND identity=?2",params![network.as_str(),m.id.as_str()],|r|r.get(0)).optional()?;
                if let Some(old) = old {
                    if old != m.fingerprint()? {
                        return Err(Error::Conflict);
                    }
                    duplicates += 1;
                    continue;
                }
                if let Some(destination) = &m.destination {
                    let route = p.topology.path(&m.origin, destination)?;
                    let mut arrived = m.path.clone();
                    arrived.push(p.local.clone());
                    if !route.starts_with(&arrived)
                        || (!p.topology.node(&p.local)?.role.can_transit()
                            && destination != &p.local)
                    {
                        return Err(Error::Policy);
                    }
                }
                if m.destination.is_none() && !subscribed(&tx, &p, expected_neighbor, &m.codename)?
                {
                    return Err(Error::Policy);
                }
                let mapping = area(&tx, &p, &m.codename)?;
                let transit = m.destination.as_ref().is_some_and(|d| d != &p.local);
                let conference = if transit {
                    if mapping.is_some_and(|a| !a.receive || !a.send) {
                        return Err(Error::Policy);
                    }
                    None
                } else {
                    match mapping {
                        Some(a) if a.receive => {
                            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM message_conferences WHERE conference_id=?1 AND public_only=1 AND active=1)",[a.conference],|r|r.get(0))?;
                            if !valid {
                                return Err(Error::Policy);
                            }
                            Some(a.conference)
                        }
                        Some(_) => return Err(Error::Policy),
                        None if m.destination.is_none()
                            && p.topology.node(&p.local)?.role.can_transit() =>
                        {
                            None
                        }
                        None => return Err(Error::Policy),
                    }
                };
                let mid = insert_native(&tx, m, conference, now)?;
                let mut forwarded = m.clone();
                forwarded.path.push(p.local.clone());
                publish(&tx, &p, &forwarded, mid, Some(expected_neighbor), now)?;
                imported += 1;
            }
            reconcile_threads(&tx, network)?;
            retain(&tx, store, bytes, now)?;
            tx.execute(
                "INSERT INTO circuitnet_imports VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    network.as_str(),
                    expected_neighbor.as_str(),
                    artifact,
                    imported,
                    duplicates,
                    now
                ],
            )?;
        } else {
            duplicates = batch.messages.len() as u32;
        }
        tx.execute("INSERT INTO circuitnet_receipts(network,neighbor,artifact,outcome,occurred_at) VALUES(?1,?2,?3,?4,?5)",params![network.as_str(),expected_neighbor.as_str(),artifact,if seen{"replayed"}else{"imported"},now])?;
        let receipt = Receipt {
            format: "circuitnet-ng-offline-receipt".into(),
            version: 1,
            network: network.clone(),
            sender: p.local,
            neighbor: expected_neighbor.clone(),
            artifact,
            accepted: batch.messages.into_iter().map(|m| m.id).collect(),
        }
        .encode()?;
        tx.commit()?;
        Ok(Imported {
            imported,
            duplicates,
            replay: seen,
            receipt,
        })
    }
    pub fn circuitnet_acknowledge(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        expected_neighbor: &NodeId,
        bytes: &[u8],
        now: i64,
    ) -> Result<u32, Error> {
        if !profile(&self.connection, network)?.0.trusted_offline {
            return Err(Error::Policy);
        }
        self.circuitnet_acknowledge_neighbor(store, network, expected_neighbor, bytes, now)
    }
    /// The host must establish authenticated direct-neighbor authority before calling.
    pub fn circuitnet_acknowledge_neighbor(
        &mut self,
        store: &dyn NetworkArtifactStore,
        network: &NetworkId,
        expected_neighbor: &NodeId,
        bytes: &[u8],
        now: i64,
    ) -> Result<u32, Error> {
        let receipt = Receipt::decode(bytes)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(expected_neighbor)?;
        if receipt.network != *network
            || receipt.sender != *expected_neighbor
            || receipt.neighbor != p.local
        {
            return Err(Error::Policy);
        }
        let rows:Vec<(String,String,String)>=tx.prepare("SELECT b.queue_id,d.identity,q.state FROM circuitnet_batch_members b JOIN circuitnet_batches a ON a.artifact=b.artifact JOIN circuitnet_deliveries d USING(queue_id) JOIN network_outbound_queue q USING(queue_id) WHERE a.artifact=?1 AND a.network=?2 AND a.neighbor=?3 ORDER BY b.ordinal")?.query_map(params![receipt.artifact,network.as_str(),expected_neighbor.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<Result<_,_>>()?;
        if rows.len() != receipt.accepted.len()
            || rows
                .iter()
                .zip(&receipt.accepted)
                .any(|(r, id)| r.1 != id.as_str())
        {
            return Err(Error::Conflict);
        }
        let mut completed = 0;
        for (q, _, state) in rows {
            if state == "accepted" {
                continue;
            }
            tx.execute("UPDATE network_outbound_queue SET state='accepted',reason='circuitnet-native-accepted',next_attempt=NULL,version=version+1 WHERE queue_id=?1",[&q])?;
            completed += 1;
        }
        let ack = retain(&tx, store, bytes, now)?;
        tx.execute("INSERT INTO circuitnet_receipts(network,neighbor,artifact,outcome,occurred_at) VALUES(?1,?2,?3,?4,?5)",params![network.as_str(),expected_neighbor.as_str(),ack,if completed==0{"ack-replayed"}else{"acknowledged"},now])?;
        tx.commit()?;
        Ok(completed)
    }
    /// Record a failed local transfer without altering other recipients.
    pub fn circuitnet_failed(
        &mut self,
        network: &NetworkId,
        artifact: &str,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let queues:Vec<(String,i64)>=tx.prepare("SELECT q.queue_id,q.attempts FROM network_outbound_queue q JOIN circuitnet_batch_members m USING(queue_id) JOIN circuitnet_batches b ON m.artifact=b.artifact WHERE b.network=?1 AND b.artifact=?2 AND q.state='ready' AND q.attempts<12")?.query_map(params![network.as_str(),artifact],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?;
        for (q, attempts) in queues {
            tx.execute("UPDATE network_outbound_queue SET state=CASE WHEN attempts=11 THEN 'failed' ELSE 'retry' END,attempts=attempts+1,reason='circuitnet-transfer-failed',version=version+1 WHERE queue_id=?1",[&q])?;
            tx.execute("INSERT INTO network_delivery_attempts(queue_id,occurred_at,outcome,attempt_number) VALUES(?1,?2,?3,?4)",params![q,now,if attempts==11{"failed"}else{"retry"},attempts+1])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_retry(
        &mut self,
        network: &NetworkId,
        queue: &str,
        expected: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        if !eligible(&tx, &p, queue)? {
            return Err(Error::Policy);
        }
        if tx.execute("UPDATE network_outbound_queue SET state='retry',reason='circuitnet-operator-retry',version=version+1 WHERE queue_id=?1 AND version=?2 AND attempts<12 AND state IN('held','retry')",params![queue,expected])?!=1{return Err(Error::Conflict);}
        tx.commit()?;
        Ok(())
    }
    pub fn hold_restored_circuitnet(&mut self) -> Result<(), Error> {
        self.connection.execute("UPDATE network_outbound_queue SET state='held',reason='circuitnet-restore-review',version=version+1 WHERE state NOT IN('accepted','cancelled','failed') AND queue_id IN(SELECT queue_id FROM circuitnet_deliveries)",[])?;
        Ok(())
    }
}
fn insert_native(
    conn: &rusqlite::Transaction<'_>,
    m: &Message,
    conference: Option<i64>,
    now: i64,
) -> Result<i64, Error> {
    let mid = crate::message::next_message_id(conn)?.get();
    let number = conference
        .map(|id| crate::message::next_message_number(conn, crate::ConferenceId::new(id)?))
        .transpose()?;
    conn.execute("INSERT INTO message_payloads(subject,body,content_kind,encoding) VALUES(?1,?2,'standard','utf8')",params![m.subject.as_bytes(),m.body.as_bytes()])?;
    let payload = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO message_fanouts(payload_id,created_at) VALUES(?1,?2)",
        params![payload, now],
    )?;
    let fanout = conn.last_insert_rowid();
    conn.execute("INSERT INTO messages(message_id,fanout_id,conference_id,message_number,author_name,created_at,placed_at,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal,origin_kind,container_kind,identity_mode) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'active','single',0,'external-network',?10,'external-asserted')",params![mid,fanout,conference,number.map(|n|n as i64),m.author,m.timestamp,now,if conference.is_some(){"all-callers"}else{"external-recipient"},if conference.is_some(){"public"}else{"private"},if conference.is_some(){"conference"}else{"network-transit"}])?;
    Ok(mid)
}
fn reconcile_threads(conn: &Connection, network: &NetworkId) -> Result<(), Error> {
    let rows:Vec<(i64,i64)>=conn.prepare("SELECT child.message_id,parent.message_id FROM circuitnet_messages child JOIN circuitnet_messages parent ON parent.network=child.network AND parent.identity=child.reply AND parent.codename=child.codename JOIN messages cm ON cm.message_id=child.message_id JOIN messages pm ON pm.message_id=parent.message_id WHERE child.network=?1 AND cm.parent_message_id IS NULL AND cm.conference_id=pm.conference_id LIMIT 1000")?.query_map([network.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?;
    for (child, parent) in rows {
        let cycle:bool=conn.query_row("WITH RECURSIVE ancestors(id) AS (SELECT ?1 UNION SELECT m.parent_message_id FROM messages m JOIN ancestors a ON m.message_id=a.id WHERE m.parent_message_id IS NOT NULL) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?2)",params![parent,child],|r|r.get(0))?;
        if cycle {
            return Err(Error::Conflict);
        }
        conn.execute("UPDATE messages SET parent_message_id=?2 WHERE message_id=?1 AND parent_message_id IS NULL",params![child,parent])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "circuitnet_tests.rs"]
mod tests;
