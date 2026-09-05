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

//! Native QWK link, publication, routing, receipt and queue authority (N2).
mod private;
pub(crate) use private::MIGRATION as PRIVATE_MIGRATION;
pub use private::{MailPolicy, MailboxAlias, NativeNetworkMail, NewNetworkMail};

use crate::{
    network::{NetworkArtifactStore, NetworkError},
    RuntimeDatabase,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sf_net::{qwk, qwk_network as wire};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MIGRATION: &str = r#"
ALTER TABLE messages ADD COLUMN origin_kind TEXT NOT NULL DEFAULT 'native' CHECK(origin_kind IN ('native','external-network'));
ALTER TABLE message_payloads ADD COLUMN encoding TEXT NOT NULL DEFAULT 'cp437' CHECK(encoding IN ('cp437','utf8'));
CREATE TABLE qwk_links (
 link_id TEXT PRIMARY KEY CHECK(length(link_id) BETWEEN 1 AND 32),
 network TEXT NOT NULL CHECK(length(network) BETWEEN 1 AND 32),
 local_id TEXT NOT NULL CHECK(length(local_id) BETWEEN 2 AND 8),
 remote_id TEXT NOT NULL CHECK(length(remote_id) BETWEEN 2 AND 8),
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 60),
 profile TEXT NOT NULL CHECK(profile IN ('qwk-headers','dove-headers')),
 role TEXT NOT NULL CHECK(role IN ('hub','node')),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 inbound INTEGER NOT NULL CHECK(inbound IN (0,1)),
 outbound INTEGER NOT NULL CHECK(outbound IN (0,1)),
 version INTEGER NOT NULL CHECK(version>0),
 UNIQUE(network,remote_id), CHECK(local_id<>remote_id)
);
CREATE TABLE qwk_link_state (
 link_id TEXT PRIMARY KEY REFERENCES qwk_links(link_id),
 last_contact INTEGER, last_result TEXT NOT NULL DEFAULT 'never'
);
CREATE TABLE qwk_link_mappings (
 link_id TEXT NOT NULL REFERENCES qwk_links(link_id),
 wire_conference INTEGER NOT NULL CHECK(wire_conference BETWEEN 1 AND 65535),
 area TEXT NOT NULL CHECK(length(area) BETWEEN 1 AND 64),
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 inbound INTEGER NOT NULL CHECK(inbound IN (0,1)),
 outbound INTEGER NOT NULL CHECK(outbound IN (0,1)),
 version INTEGER NOT NULL CHECK(version>0),
 PRIMARY KEY(link_id,wire_conference), UNIQUE(link_id,conference_id), UNIQUE(link_id,area)
);
CREATE TABLE network_publications (
 publication_id TEXT PRIMARY KEY,
 message_id INTEGER NOT NULL REFERENCES messages(message_id),
 network TEXT NOT NULL, area TEXT NOT NULL, wire_id TEXT NOT NULL,
 origin TEXT NOT NULL, ingress_link TEXT REFERENCES qwk_links(link_id),
 reply_id TEXT, source_written TEXT, source_wall_time TEXT, recipient BLOB, content_digest TEXT, created_at INTEGER NOT NULL,
 UNIQUE(network,area,wire_id), UNIQUE(message_id,network,area)
);
CREATE TABLE network_publication_path (
 publication_id TEXT NOT NULL REFERENCES network_publications(publication_id),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 31), system_id TEXT NOT NULL,
 PRIMARY KEY(publication_id,ordinal), UNIQUE(publication_id,system_id)
);
CREATE TABLE network_routing_decisions (
 decision_id TEXT PRIMARY KEY, publication_id TEXT NOT NULL REFERENCES network_publications(publication_id),
 link_id TEXT NOT NULL REFERENCES qwk_links(link_id), destination TEXT NOT NULL,
 link_version INTEGER NOT NULL, mapping_version INTEGER NOT NULL, wire_conference INTEGER NOT NULL,
 message_version INTEGER NOT NULL, policy_digest TEXT NOT NULL, created_at INTEGER NOT NULL,
 UNIQUE(publication_id,destination)
);
CREATE TABLE network_outbound_queue (
 queue_id TEXT PRIMARY KEY REFERENCES network_routing_decisions(decision_id),
 state TEXT NOT NULL CHECK(state IN ('pending','ready','held','retry','accepted','failed','quarantined','cancelled')),
 artifact_id TEXT REFERENCES network_artifacts(artifact_id),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 12),
 next_attempt INTEGER, reason TEXT NOT NULL DEFAULT 'prepared', created_at INTEGER NOT NULL, reserved_bytes INTEGER NOT NULL CHECK(reserved_bytes BETWEEN 0 AND 1048576)
);
CREATE TABLE network_delivery_attempts (
 attempt_id INTEGER PRIMARY KEY, queue_id TEXT NOT NULL REFERENCES network_outbound_queue(queue_id),
 occurred_at INTEGER NOT NULL, outcome TEXT NOT NULL CHECK(outcome IN ('accepted','retry','failed')),
 attempt_number INTEGER NOT NULL CHECK(attempt_number BETWEEN 1 AND 12), UNIQUE(queue_id,attempt_number)
);
CREATE TABLE network_history_capacity (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), receipt_rows INTEGER NOT NULL CHECK(receipt_rows BETWEEN 0 AND 2000000),
 reserved_bytes INTEGER NOT NULL CHECK(reserved_bytes BETWEEN 0 AND 536870912)
);
INSERT INTO network_history_capacity VALUES(1,0,0);
CREATE TABLE qwk_network_import_receipts (
 link_id TEXT NOT NULL REFERENCES qwk_links(link_id), packet_digest TEXT NOT NULL,
 ordinal INTEGER NOT NULL, artifact_id TEXT NOT NULL REFERENCES network_artifacts(artifact_id),
 message_id INTEGER REFERENCES messages(message_id), publication_id TEXT REFERENCES network_publications(publication_id),
 outcome TEXT NOT NULL CHECK(outcome IN ('imported','duplicate','loop','quarantined')),
 reason TEXT NOT NULL, received_at INTEGER NOT NULL,
 PRIMARY KEY(link_id,packet_digest,ordinal)
);
CREATE TABLE network_quarantine (
 quarantine_id INTEGER PRIMARY KEY, link_id TEXT REFERENCES qwk_links(link_id),
 artifact_id TEXT REFERENCES network_artifacts(artifact_id), reason TEXT NOT NULL,
 received_at INTEGER NOT NULL, UNIQUE(link_id,artifact_id,reason)
);
CREATE TRIGGER network_history_receipt_budget AFTER INSERT ON qwk_network_import_receipts BEGIN
 UPDATE network_history_capacity SET receipt_rows=receipt_rows+1,reserved_bytes=reserved_bytes+1024 WHERE singleton=1;
END;
CREATE TRIGGER network_history_publication_budget AFTER INSERT ON network_publications BEGIN
 UPDATE network_history_capacity SET reserved_bytes=reserved_bytes+16384 WHERE singleton=1;
END;
CREATE TRIGGER qwk_network_receipt_immutable BEFORE UPDATE ON qwk_network_import_receipts BEGIN SELECT RAISE(ABORT,'immutable network receipt'); END;
CREATE TRIGGER qwk_network_receipt_retained BEFORE DELETE ON qwk_network_import_receipts BEGIN SELECT RAISE(ABORT,'retained network receipt'); END;
CREATE TRIGGER network_publication_immutable BEFORE UPDATE ON network_publications BEGIN SELECT RAISE(ABORT,'immutable publication'); END;
CREATE TRIGGER network_publication_retained BEFORE DELETE ON network_publications BEGIN SELECT RAISE(ABORT,'retained publication'); END;
CREATE TRIGGER network_route_immutable BEFORE UPDATE ON network_routing_decisions BEGIN SELECT RAISE(ABORT,'immutable route'); END;
CREATE TRIGGER network_route_retained BEFORE DELETE ON network_routing_decisions BEGIN SELECT RAISE(ABORT,'retained route'); END;
CREATE TRIGGER network_path_immutable BEFORE UPDATE ON network_publication_path BEGIN SELECT RAISE(ABORT,'immutable path'); END;
CREATE TRIGGER network_path_retained BEFORE DELETE ON network_publication_path BEGIN SELECT RAISE(ABORT,'retained path'); END;
CREATE TRIGGER network_external_author_update BEFORE UPDATE OF origin_kind,author_caller_id ON messages WHEN NEW.origin_kind='external-network' AND NEW.author_caller_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'network author is not a caller'); END;
CREATE TRIGGER network_external_author BEFORE INSERT ON messages WHEN NEW.origin_kind='external-network' AND NEW.author_caller_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'network author is not a caller'); END;
"#;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid network partner or identity")]
    InvalidPartner,
    #[error("invalid or inaccessible network mapping")]
    InvalidMapping,
    #[error("network policy or expected version changed")]
    Conflict,
    #[error("network link is disabled or direction is denied")]
    Disabled,
    #[error("network resource capacity reached")]
    Capacity,
    #[error("network work is held or unavailable")]
    Held,
    #[error("network artifact rejected")]
    Rejected,
    #[error("network database operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("network artifact operation failed")]
    Artifact(#[from] NetworkError),
    #[error("network codec rejected input")]
    Codec(#[from] qwk::Error),
    #[error("native message operation failed")]
    Message(#[from] crate::MessageError),
    #[error("network event could not be recorded")]
    Database(#[from] crate::DatabaseError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    QwkHeaders,
    DoveHeaders,
}
impl Profile {
    fn key(self) -> &'static str {
        match self {
            Self::QwkHeaders => "qwk-headers",
            Self::DoveHeaders => "dove-headers",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartnerRole {
    Hub,
    Node,
}
impl PartnerRole {
    fn key(self) -> &'static str {
        match self {
            Self::Hub => "hub",
            Self::Node => "node",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub id: String,
    pub network: String,
    pub local_id: String,
    pub remote_id: String,
    pub name: String,
    pub profile: Profile,
    pub role: PartnerRole,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub wire_conference: u16,
    pub area: String,
    pub conference_id: i64,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub link: String,
    pub state: String,
    pub artifact: Option<String>,
    pub version: i64,
    pub attempts: u32,
    pub next_attempt: Option<i64>,
    pub reason: String,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: u32,
    pub duplicates: u32,
    pub loops: u32,
    pub quarantined: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LinkStatus {
    pub link: Link,
    pub mappings: Vec<Mapping>,
    /// First queue page; use qwk_network_queue for subsequent stable-ID pages.
    pub queue: Vec<QueueItem>,
    pub queue_counts: BTreeMap<String, i64>,
    pub queue_next: Option<String>,
    pub last_contact: Option<i64>,
    pub last_result: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueuePage {
    pub items: Vec<QueueItem>,
    pub next: Option<String>,
}
fn token(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.len() <= max
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.')
        })
}
fn id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
fn event(tx: &Transaction<'_>, code: &str, now: i64) -> Result<(), Error> {
    crate::insert_operational_event_tx(
        tx,
        &crate::NewOperationalEvent::new(
            now,
            crate::EventCategory::Message,
            crate::EventSeverity::Info,
            format!("message.qwk-network.{code}"),
            crate::EventOutcome::Succeeded,
        ),
    )?;
    Ok(())
}
fn audit(
    tx: &Transaction<'_>,
    principal: &str,
    operation: &str,
    target: &str,
    now: i64,
) -> Result<(), Error> {
    if principal.is_empty() || principal.len() > 64 || principal.chars().any(char::is_control) {
        return Err(Error::InvalidPartner);
    }
    tx.execute("INSERT INTO operator_control_audit(occurred_at,operator_kind,operator_id,operation,authorization_result,target_kind,target_id,outcome) VALUES(?1,'host-operator',?2,?3,'allowed','qwk-network',?4,'succeeded')",params![now,principal,operation,target])?;
    Ok(())
}
fn mapping_policy(conn: &rusqlite::Connection, mapping: &Mapping) -> Result<String, Error> {
    let policy: Option<(bool,bool,i64,i64,String)>=conn.query_row("SELECT active,public_only,read_security,post_security,access_mode FROM message_conferences WHERE conference_id=?1",[mapping.conference_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    match policy {
        Some((true, public_only, r, p, a)) => Ok(qwk::digest(
            format!("{}:{public_only}:{r}:{p}:{a}", mapping.conference_id).as_bytes(),
        )),
        _ => Err(Error::InvalidMapping),
    }
}
fn load_link(conn: &rusqlite::Connection, link: &str) -> Result<Link, Error> {
    conn.query_row("SELECT link_id,network,local_id,remote_id,name,profile,role,enabled,inbound,outbound,version FROM qwk_links WHERE link_id=?1",[link],|r|Ok(Link{id:r.get(0)?,network:r.get(1)?,local_id:r.get(2)?,remote_id:r.get(3)?,name:r.get(4)?,profile:if r.get::<_,String>(5)?=="dove-headers"{Profile::DoveHeaders}else{Profile::QwkHeaders},role:if r.get::<_,String>(6)?=="hub"{PartnerRole::Hub}else{PartnerRole::Node},enabled:r.get(7)?,inbound:r.get(8)?,outbound:r.get(9)?,version:r.get(10)?})).optional()?.ok_or(Error::InvalidPartner)
}
fn load_mappings(conn: &rusqlite::Connection, link: &str) -> Result<Vec<Mapping>, Error> {
    Ok(conn.prepare("SELECT wire_conference,area,conference_id,enabled,inbound,outbound,version FROM qwk_link_mappings WHERE link_id=?1 ORDER BY wire_conference LIMIT 785")?.query_map([link],|r|Ok(Mapping{wire_conference:r.get(0)?,area:r.get(1)?,conference_id:r.get(2)?,enabled:r.get(3)?,inbound:r.get(4)?,outbound:r.get(5)?,version:r.get(6)?}))?.collect::<Result<_,_>>()?)
}

impl RuntimeDatabase {
    /// Typed relational configuration. Host layer must authorize configuration capability.
    pub fn configure_qwk_link(
        &mut self,
        principal: &str,
        link: &Link,
        mappings: &[Mapping],
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        if expected < 0
            || !token(&link.id, 32)
            || !token(&link.network, 32)
            || !wire::valid_system_id(&link.local_id)
            || !wire::valid_system_id(&link.remote_id)
            || link.local_id == link.remote_id
            || link.name.is_empty()
            || link.name.len() > 60
            || link.name.chars().any(char::is_control)
            || link.version != expected.checked_add(1).ok_or(Error::Conflict)?
        {
            return Err(Error::InvalidPartner);
        }
        if mappings.is_empty() || mappings.len() > 64 {
            return Err(Error::InvalidMapping);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = match load_link(&tx, &link.id) {
            Ok(link) => Some(link),
            Err(Error::InvalidPartner) => None,
            Err(e) => return Err(e),
        };
        if existing.as_ref().map_or(0, |l| l.version) != expected {
            return Err(Error::Conflict);
        }
        if existing.as_ref().is_some_and(|l| {
            l.network != link.network
                || l.local_id != link.local_id
                || l.remote_id != link.remote_id
                || l.role != link.role
                || l.profile != link.profile
        }) {
            return Err(Error::Conflict);
        }
        let conflict: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM qwk_links WHERE network=?1 AND local_id<>?2)",
            params![link.network, link.local_id],
            |r| r.get(0),
        )?;
        if conflict {
            return Err(Error::InvalidPartner);
        }
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM qwk_links", [], |r| r.get(0))?;
        if existing.is_none() && count >= 32 {
            return Err(Error::Capacity);
        }
        let other_mappings: i64 = tx.query_row(
            "SELECT COUNT(*) FROM qwk_link_mappings WHERE link_id<>?1",
            [&link.id],
            |r| r.get(0),
        )?;
        if other_mappings + mappings.len() as i64 > 512 {
            return Err(Error::Capacity);
        }
        let mut wires = BTreeSet::new();
        let mut native = BTreeSet::new();
        let mut areas = BTreeSet::new();
        for m in mappings {
            if m.wire_conference == 0
                || !token(&m.area, 64)
                || !wires.insert(m.wire_conference)
                || !native.insert(m.conference_id)
                || !areas.insert(&m.area)
                || m.version != link.version
            {
                return Err(Error::InvalidMapping);
            }
            mapping_policy(&tx, m)?;
            if link.profile == Profile::DoveHeaders
                && matches!(m.wire_conference, 2008 | 2010 | 2013 | 2030)
            {
                return Err(Error::InvalidMapping);
            }
            let mismatch:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM qwk_link_mappings m JOIN qwk_links l USING(link_id) WHERE l.network=?1 AND l.link_id<>?2 AND ((m.area=?3 AND m.conference_id<>?4) OR (m.conference_id=?4 AND m.area<>?3)))",params![link.network,link.id,m.area,m.conference_id],|r|r.get(0))?;
            if mismatch {
                return Err(Error::InvalidMapping);
            }
        }
        tx.execute("INSERT INTO qwk_links VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(link_id) DO UPDATE SET name=excluded.name,enabled=excluded.enabled,inbound=excluded.inbound,outbound=excluded.outbound,version=excluded.version",params![link.id,link.network,link.local_id,link.remote_id,link.name,link.profile.key(),link.role.key(),link.enabled,link.inbound,link.outbound,link.version])?;
        tx.execute(
            "INSERT OR IGNORE INTO qwk_link_state(link_id) VALUES(?1)",
            [&link.id],
        )?;
        tx.execute("DELETE FROM qwk_link_mappings WHERE link_id=?1", [&link.id])?;
        for m in mappings {
            tx.execute(
                "INSERT INTO qwk_link_mappings VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    link.id,
                    m.wire_conference,
                    m.area,
                    m.conference_id,
                    m.enabled,
                    m.inbound,
                    m.outbound,
                    m.version
                ],
            )?;
        }
        tx.execute("UPDATE network_outbound_queue SET state='held',reason='policy-changed',version=version+1 WHERE state IN ('pending','ready','retry') AND queue_id IN (SELECT decision_id FROM network_routing_decisions WHERE link_id=?1)",[&link.id])?;
        audit(&tx, principal, "network.configure", &link.id, now)?;
        event(&tx, "configured", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn validate_qwk_ingress(&self, id: &str, expected: i64) -> Result<(), Error> {
        let link = load_link(&self.connection, id)?;
        if link.version != expected {
            return Err(Error::Conflict);
        }
        if !link.enabled || !link.inbound {
            return Err(Error::Disabled);
        }
        Ok(())
    }
    pub fn qwk_network_queue(&self, link: &str, after: Option<&str>) -> Result<QueuePage, Error> {
        load_link(&self.connection, link)?;
        if after.is_some_and(|s| s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(Error::Rejected);
        }
        let mut items=self.connection.prepare("SELECT q.queue_id,d.link_id,q.state,q.artifact_id,q.version,q.attempts,q.next_attempt,q.reason FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE d.link_id=?1 AND q.queue_id>?2 ORDER BY q.queue_id LIMIT 21")?.query_map(params![link,after.unwrap_or("")],|r|Ok(QueueItem{id:r.get(0)?,link:r.get(1)?,state:r.get(2)?,artifact:r.get(3)?,version:r.get(4)?,attempts:r.get(5)?,next_attempt:r.get(6)?,reason:r.get(7)?}))?.collect::<Result<Vec<_>,_>>()?;
        let next = if items.len() > 20 {
            items.pop();
            items.last().map(|q| q.id.clone())
        } else {
            None
        };
        Ok(QueuePage { items, next })
    }
    pub fn qwk_network_status(&self) -> Result<Vec<LinkStatus>, Error> {
        let ids = self
            .connection
            .prepare("SELECT link_id FROM qwk_links ORDER BY link_id LIMIT 33")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = Vec::new();
        for id in ids {
            let page = self.qwk_network_queue(&id, None)?;
            let queue_counts=self.connection.prepare("SELECT q.state,COUNT(*) FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE d.link_id=?1 GROUP BY q.state")?.query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<BTreeMap<_,_>,_>>()?;
            let (last_contact, last_result) = self.connection.query_row(
                "SELECT last_contact,last_result FROM qwk_link_state WHERE link_id=?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            result.push(LinkStatus {
                link: load_link(&self.connection, &id)?,
                mappings: load_mappings(&self.connection, &id)?,
                queue: page.items,
                queue_counts,
                queue_next: page.next,
                last_contact,
                last_result,
            });
        }
        Ok(result)
    }
}

fn native_text(bytes: &[u8], utf8: bool) -> Result<String, Error> {
    if utf8 {
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::Rejected)
    } else {
        Ok(crate::file_maintenance::decode_cp437(bytes))
    }
}
fn add_quarantine(
    tx: &Transaction<'_>,
    link: &str,
    artifact: &str,
    reason: &str,
    now: i64,
) -> Result<(), Error> {
    let (count,size):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(a.byte_length),0) FROM network_quarantine q JOIN network_artifacts a USING(artifact_id)",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let existing:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM network_quarantine WHERE link_id=?1 AND artifact_id=?2 AND reason=?3)",params![link,artifact,reason],|r|r.get(0))?;
    if existing {
        return Ok(());
    }
    let incoming: i64 = tx.query_row(
        "SELECT byte_length FROM network_artifacts WHERE artifact_id=?1",
        [artifact],
        |r| r.get(0),
    )?;
    if count >= 128 || size + incoming > 128 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    tx.execute("INSERT OR IGNORE INTO network_quarantine(link_id,artifact_id,reason,received_at) VALUES(?1,?2,?3,?4)",params![link,artifact,reason,now])?;
    event(tx, "quarantined", now)
}
impl RuntimeDatabase {
    /// Completed artifact ownership and the explicit link supply ingress authority.
    /// Wire author fields remain external attribution and cannot select local callers.
    pub fn ingest_qwk_network(
        &mut self,
        store: &dyn NetworkArtifactStore,
        link_id: &str,
        expected: i64,
        bytes: &[u8],
        now: i64,
    ) -> Result<ImportResult, Error> {
        let _permit = store.admit_import()?;
        let link = load_link(&self.connection, link_id)?;
        if link.version != expected {
            return Err(Error::Conflict);
        }
        if !link.enabled || !link.inbound {
            return Err(Error::Disabled);
        }
        let artifact = self.preserve_artifact(store, bytes, now)?;
        let decoded = (|| -> Result<_, Error> {
            let packet = qwk::inspect(bytes)?;
            if link.role == PartnerRole::Hub {
                let control = packet.members.get("CONTROL.DAT").ok_or(Error::Rejected)?;
                let lines: Vec<_> = control.split(|b| *b == b'\n').collect();
                let board = lines.get(4).ok_or(Error::Rejected)?.trim_ascii();
                if board.split(|b| *b == b',').next_back() != Some(link.remote_id.as_bytes()) {
                    return Err(Error::Rejected);
                }
            }
            let messages = wire::decode(
                &packet,
                if link.role == PartnerRole::Node {
                    Some(&link.local_id)
                } else {
                    None
                },
            )?;
            Ok((packet, messages))
        })();
        let (packet, messages) = match decoded {
            Ok(v) => v,
            Err(_) => {
                let tx = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                add_quarantine(&tx, link_id, &artifact, "invalid-packet", now)?;
                tx.execute(
                    "UPDATE qwk_link_state SET last_result='quarantined' WHERE link_id=?1",
                    [link_id],
                )?;
                tx.commit()?;
                return Ok(ImportResult {
                    quarantined: 1,
                    ..Default::default()
                });
            }
        };
        let mut result = ImportResult::default();
        for (ordinal, member) in messages.iter().enumerate() {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = load_link(&tx, link_id)?;
            if !current.enabled || !current.inbound || current.version != expected {
                return Err(Error::Conflict);
            }
            let prior:Option<String>=tx.query_row("SELECT outcome FROM qwk_network_import_receipts WHERE link_id=?1 AND packet_digest=?2 AND ordinal=?3",params![link_id,packet.digest,ordinal as i64],|r|r.get(0)).optional()?;
            if prior.is_some() {
                result.duplicates += 1;
                event(&tx, "duplicate", now)?;
                tx.commit()?;
                continue;
            }
            let (count,reserved):(i64,i64)=tx.query_row("SELECT receipt_rows,reserved_bytes FROM network_history_capacity WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
            if count >= 2_000_000 || reserved > 512 * 1024 * 1024 - 17408 {
                return Err(Error::Capacity);
            }
            let mappings = load_mappings(&tx, link_id)?;
            let mapping = mappings
                .iter()
                .find(|m| m.wire_conference == member.message.conference && m.enabled && m.inbound);
            let mut outcome = "imported";
            let mut reason = "native-import";
            let mut native = None;
            let mut publication = None;
            let mut path = member.metadata.path.clone();
            if path.contains(&link.local_id) {
                outcome = "loop";
                reason = "local-path";
            } else if path.contains(&link.remote_id) || path.len() >= wire::MAX_PATH {
                outcome = "quarantined";
                reason = "invalid-path";
            } else if !member.message.private && member.message.conference != 0 && mapping.is_none()
            {
                outcome = "quarantined";
                reason = "invalid-mapping";
            }
            path.insert(0, link.remote_id.clone());
            let private_member = member.message.private || member.message.conference == 0;
            if private_member && outcome == "imported" {
                let imported = private::ingest(&tx, &link, member, path.clone(), now)?;
                outcome = imported.outcome;
                reason = imported.reason;
                native = imported.native;
                publication = imported.publication;
            }
            if let Some(mapping) = mapping.filter(|_| outcome == "imported" && !private_member) {
                if mapping_policy(&tx, mapping).is_err() {
                    outcome = "quarantined";
                    reason = "mapping-policy";
                }
                let origin = path.last().ok_or(Error::Rejected)?;
                let previous:Option<(String,i64,String,Option<String>)>=tx.query_row("SELECT publication_id,message_id,origin,content_digest FROM network_publications WHERE network=?1 AND area=?2 AND wire_id=?3",params![link.network,mapping.area,member.metadata.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
                if let Some((p, m, o, digest)) = previous {
                    native = Some(m);
                    publication = Some(p);
                    if o != *origin || digest.is_some_and(|d| d != wire::content_digest(member)) {
                        outcome = "quarantined";
                        reason = "identity-collision";
                    } else {
                        outcome = "duplicate";
                        reason = "retained-identity";
                    }
                } else if member
                    .metadata
                    .id
                    .to_ascii_lowercase()
                    .ends_with(&format!("@{}.qwk>", link.local_id.to_ascii_lowercase()))
                {
                    outcome = "quarantined";
                    reason = "local-origin-spoof";
                }
                if outcome == "imported" {
                    let m = &member.message;
                    let author = native_text(&m.from, member.metadata.utf8)?;
                    if author.is_empty()
                        || author.chars().count() > 60
                        || author.chars().any(char::is_control)
                        || m.subject.is_empty()
                        || m.body.is_empty()
                        || !wire::valid_native_text(m, member.metadata.utf8)
                    {
                        outcome = "quarantined";
                        reason = "native-bounds";
                    } else {
                        let parent = if let Some(reply) = &member.metadata.reply {
                            tx.query_row("SELECT p.message_id FROM network_publications p JOIN messages m USING(message_id) WHERE p.network=?1 AND p.area=?2 AND p.wire_id=?3 AND m.conference_id=?4 AND m.visibility='public' AND m.lifecycle_state='active'",params![link.network,mapping.area,reply,mapping.conference_id],|r|r.get::<_,i64>(0)).optional()?
                        } else {
                            None
                        };
                        let conf = crate::ConferenceId::new(mapping.conference_id)?;
                        let number = crate::message::next_message_number(&tx, conf)?;
                        let message_id = crate::message::next_message_id(&tx)?;
                        tx.execute("INSERT INTO message_payloads(subject,body,content_kind,encoding) VALUES(?1,?2,'standard',?3)",params![m.subject,m.body,if member.metadata.utf8{"utf8"}else{"cp437"}])?;
                        let payload = tx.last_insert_rowid();
                        tx.execute("INSERT INTO message_fanouts(payload_id,created_by_caller_id,created_at) VALUES(?1,NULL,?2)",params![payload,now])?;
                        let fanout = tx.last_insert_rowid();
                        tx.execute("INSERT INTO messages(message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal,origin_kind) VALUES(?1,?2,?3,?4,NULL,?5,?6,?8,?7,'all-callers','public','active','single',0,'external-network')",params![message_id.get(),fanout,mapping.conference_id,i64::try_from(number).map_err(|_|Error::Capacity)?,author,member.metadata.written.as_deref().and_then(wire::written_timestamp).unwrap_or(now),parent,now])?;
                        let p = id();
                        tx.execute("INSERT INTO network_publications VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![p,message_id.get(),link.network,mapping.area,member.metadata.id,origin,link_id,member.metadata.reply,member.metadata.written,m.wall_time.format("%Y-%m-%dT%H:%M:%S").to_string(),m.to,wire::content_digest(member),now])?;
                        for (n, system) in path.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO network_publication_path VALUES(?1,?2,?3)",
                                params![p, n as i64, system],
                            )?;
                        }
                        native = Some(message_id.get());
                        publication = Some(p);
                    }
                }
            }
            tx.execute(
                "INSERT INTO qwk_network_import_receipts VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    link_id,
                    packet.digest,
                    ordinal as i64,
                    artifact,
                    native,
                    publication,
                    outcome,
                    reason,
                    now
                ],
            )?;
            match outcome {
                "imported" => result.imported += 1,
                "duplicate" => result.duplicates += 1,
                "loop" => result.loops += 1,
                _ => {
                    result.quarantined += 1;
                    add_quarantine(&tx, link_id, &artifact, reason, now)?;
                }
            }
            event(&tx, outcome, now)?;
            tx.commit()?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE qwk_link_state SET last_contact=?2,last_result='received' WHERE link_id=?1",
            params![link_id, now],
        )?;
        event(&tx, "received", now)?;
        tx.commit()?;
        Ok(result)
    }
}

type PublicationFields = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Vec<u8>>,
);

struct ExportMember {
    queue: String,
    publication: String,
    mapping: Mapping,
    message_version: i64,
    policy: String,
    wire: wire::NetworkMessage,
}
fn publication_path(conn: &rusqlite::Connection, p: &str) -> Result<Vec<String>, Error> {
    Ok(conn.prepare("SELECT system_id FROM network_publication_path WHERE publication_id=?1 ORDER BY ordinal")?.query_map([p],|r|r.get(0))?.collect::<Result<_,_>>()?)
}
fn export_member(
    conn: &rusqlite::Connection,
    queue: &str,
    link: &Link,
    mappings: &[Mapping],
) -> Result<ExportMember, Error> {
    let (publication,conference,area,version,policy,mid):(String,u16,String,i64,String,i64)=conn.query_row("SELECT p.publication_id,d.wire_conference,p.area,d.message_version,d.policy_digest,p.message_id FROM network_routing_decisions d JOIN network_publications p USING(publication_id) WHERE d.decision_id=?1 AND d.link_id=?2",params![queue,link.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    if conference == 0 {
        return private::export(conn, queue, link);
    }
    let mapping = mappings
        .iter()
        .find(|m| m.wire_conference == conference && m.area == area && m.enabled && m.outbound)
        .ok_or(Error::Held)?
        .clone();
    if mapping_policy(conn, &mapping)? != policy {
        return Err(Error::Held);
    }
    let (number,author,subject,body,encoding,created,parent):(u32,String,Vec<u8>,Vec<u8>,String,i64,Option<i64>)=conn.query_row("SELECT m.message_number,m.author_name,p.subject,p.body,p.encoding,m.created_at,m.parent_message_id FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE m.message_id=?1 AND m.conference_id=?2 AND m.visibility='public' AND m.lifecycle_state='active' AND m.state_version=?3",params![mid,mapping.conference_id,version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional()?.ok_or(Error::Held)?;
    let utf8 = encoding == "utf8";
    let from = if utf8 {
        author.into_bytes()
    } else {
        crate::encode_text(&author, crate::TerminalTextEncoding::Cp437).ok_or(Error::Held)?
    };
    let (wire_id, reply, written, source_wall, recipient): PublicationFields = conn.query_row(
        "SELECT wire_id,reply_id,source_written,source_wall_time,recipient FROM network_publications WHERE publication_id=?1",
        [&publication],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?,r.get(3)?,r.get(4)?)),
    )?;
    let external_time = source_wall.is_some();
    let reply=match (reply,parent){(Some(r),_)=>Some(r),(None,Some(p))=>conn.query_row("SELECT wire_id FROM network_publications WHERE message_id=?1 AND network=?2 AND area=?3",params![p,link.network,area],|r|r.get(0)).optional()?,_=>None};
    let path = publication_path(conn, &publication)?;
    if path.contains(&link.local_id) || path.contains(&link.remote_id) {
        return Err(Error::Held);
    }
    let wall_time = match source_wall {
        Some(t) => t
            .parse::<chrono::NaiveDateTime>()
            .map_err(|_| Error::Held)?,
        None => chrono::DateTime::from_timestamp(created, 0)
            .ok_or(Error::Held)?
            .naive_utc(),
    };
    let wire = wire::NetworkMessage {
        message: qwk::Message {
            number,
            conference,
            reference: 0,
            private: false,
            received: false,
            to: recipient.unwrap_or_else(|| b"All".to_vec()),
            from,
            subject,
            body,
            wall_time,
        },
        metadata: wire::Metadata {
            id: wire_id,
            reply,
            path,
            utf8,
            written: written.or_else(|| {
                (!external_time).then(|| format!("{} 0000", wall_time.format("%Y%m%dT%H%M%SZ")))
            }),
            fields: Vec::new(),
        },
        offset: 0,
        digest: String::new(),
    };
    Ok(ExportMember {
        queue: queue.into(),
        publication,
        mapping,
        message_version: version,
        policy,
        wire,
    })
}
impl RuntimeDatabase {
    /// Routing decisions are independent of immutable packet preparation and delivery attempts.
    pub fn build_qwk_network(
        &mut self,
        store: &dyn NetworkArtifactStore,
        link_id: &str,
        expected: i64,
        now: i64,
    ) -> Result<Option<String>, Error> {
        let link = load_link(&self.connection, link_id)?;
        if !link.enabled || !link.outbound {
            return Err(Error::Disabled);
        }
        if link.version != expected {
            return Err(Error::Conflict);
        }
        let ready:Option<String>=self.connection.query_row("SELECT DISTINCT q.artifact_id FROM network_outbound_queue q JOIN network_routing_decisions d ON q.queue_id=d.decision_id WHERE d.link_id=?1 AND q.state IN ('ready','retry') AND q.artifact_id IS NOT NULL LIMIT 1",[link_id],|r|r.get(0)).optional()?;
        let mappings = load_mappings(&self.connection, link_id)?;
        if let Some(artifact) = &ready {
            let queues=self.connection.prepare("SELECT q.queue_id,q.state FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE d.link_id=?1 AND q.artifact_id=?2")?.query_map(params![link_id,artifact],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            if queues.iter().any(|(queue, state)| {
                !matches!(state.as_str(), "ready" | "retry")
                    || export_member(&self.connection, queue, &link, &mappings).is_err()
            }) {
                // One immutable packet contains every member: holding only the
                // stale member must never make the old bytes available again.
                let tx = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute("UPDATE network_outbound_queue SET state='held',reason='packet-member-changed',version=version+1 WHERE artifact_id=?1 AND state IN ('ready','retry') AND queue_id IN (SELECT decision_id FROM network_routing_decisions WHERE link_id=?2)",params![artifact,link_id])?;
                event(&tx, "held", now)?;
                tx.commit()?;
                return Err(Error::Held);
            }
            return Ok(ready);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let total:i64=tx.query_row("SELECT COUNT(*) FROM network_outbound_queue WHERE state NOT IN ('accepted','cancelled')",[],|r|r.get(0))?;
        let link_total:i64=tx.query_row("SELECT COUNT(*) FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE d.link_id=?1 AND q.state NOT IN ('accepted','cancelled')",[link_id],|r|r.get(0))?;
        if total >= 10_000 || link_total >= 1000 {
            return Err(Error::Capacity);
        }
        let mut room = (10_000 - total).min(1000 - link_total) as usize;
        let (mut total_bytes,mut link_bytes):(i64,i64)=tx.query_row("SELECT COALESCE(SUM(q.reserved_bytes),0),COALESCE(SUM(CASE WHEN d.link_id=?1 THEN q.reserved_bytes ELSE 0 END),0) FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE q.state NOT IN ('accepted','cancelled')",[link_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        for mapping in mappings.iter().filter(|m| m.enabled && m.outbound) {
            if room == 0 {
                break;
            }
            let policy = mapping_policy(&tx, mapping)?;
            let candidates=tx.prepare("SELECT m.message_id,m.state_version FROM messages m WHERE m.conference_id=?1 AND m.visibility='public' AND m.audience_kind='all-callers' AND m.lifecycle_state='active' AND (m.author_caller_id IS NOT NULL OR (m.origin_kind='external-network' AND EXISTS(SELECT 1 FROM network_publications ep WHERE ep.message_id=m.message_id AND ep.network=?2 AND ep.area=?3 AND ep.ingress_link<>?5 AND NOT EXISTS(SELECT 1 FROM network_publication_path pp WHERE pp.publication_id=ep.publication_id AND pp.system_id IN (?4,?6))))) AND NOT EXISTS(SELECT 1 FROM network_publications p JOIN network_routing_decisions d USING(publication_id) WHERE p.message_id=m.message_id AND p.network=?2 AND p.area=?3 AND d.destination=?4) ORDER BY m.message_id LIMIT 1000")?.query_map(params![mapping.conference_id,link.network,mapping.area,link.remote_id,link_id,link.local_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            for (mid, version) in candidates {
                if room == 0 {
                    break;
                }
                let bytes:i64=tx.query_row("SELECT length(p.subject)+length(p.body)+16896 FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE m.message_id=?1",[mid],|r|r.get(0))?;
                if total_bytes + bytes > 256 * 1024 * 1024 || link_bytes + bytes > 64 * 1024 * 1024
                {
                    room = 0;
                    break;
                }
                let external: bool = tx.query_row(
                    "SELECT origin_kind='external-network' FROM messages WHERE message_id=?1",
                    [mid],
                    |r| r.get(0),
                )?;
                let found:Option<(String,Option<String>)>=tx.query_row("SELECT publication_id,ingress_link FROM network_publications WHERE message_id=?1 AND network=?2 AND area=?3",params![mid,link.network,mapping.area],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                if external && found.is_none() {
                    continue;
                }
                let publication = if let Some((p, ingress)) = found {
                    let path = publication_path(&tx, &p)?;
                    if ingress.as_deref() == Some(link_id)
                        || path.contains(&link.remote_id)
                        || path.contains(&link.local_id)
                    {
                        continue;
                    }
                    p
                } else {
                    let p = id();
                    let wire_id = format!("<{}@{}.qwk>", id(), link.local_id);
                    tx.execute("INSERT INTO network_publications(publication_id,message_id,network,area,wire_id,origin,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![p,mid,link.network,mapping.area,wire_id,link.local_id,now])?;
                    p
                };
                let queue = id();
                tx.execute("INSERT OR IGNORE INTO network_routing_decisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![queue,publication,link_id,link.remote_id,expected,mapping.version,mapping.wire_conference,version,policy,now])?;
                tx.execute("INSERT OR IGNORE INTO network_outbound_queue(queue_id,state,created_at,reserved_bytes) SELECT decision_id,'pending',?2,?3 FROM network_routing_decisions WHERE decision_id=?1",params![queue,now,bytes])?;
                total_bytes += bytes;
                link_bytes += bytes;
                room -= 1;
            }
        }
        tx.commit()?;
        let pending=self.connection.prepare("SELECT q.queue_id FROM network_outbound_queue q JOIN network_routing_decisions d ON q.queue_id=d.decision_id WHERE d.link_id=?1 AND q.state='pending' ORDER BY q.created_at,q.queue_id LIMIT 1000")?.query_map([link_id],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        if pending.is_empty() {
            return Ok(None);
        }
        let mut exports = Vec::new();
        let mut packet_bytes = 0;
        for queue in pending {
            match export_member(&self.connection, &queue, &link, &mappings) {
                Ok(m) => {
                    let reserved = m.wire.message.body.len() + 16896;
                    if packet_bytes + reserved > 8 * 1024 * 1024 {
                        break;
                    }
                    packet_bytes += reserved;
                    exports.push(m);
                }
                Err(_) => {
                    self.connection.execute("UPDATE network_outbound_queue SET state='held',reason='message-or-policy-changed',version=version+1 WHERE queue_id=?1",[queue])?;
                }
            }
        }
        if exports.is_empty() {
            return Err(Error::Held);
        }
        let messages: Vec<_> = exports.iter().map(|m| m.wire.clone()).collect();
        let encoded = wire::encode(
            &messages,
            if link.role == PartnerRole::Hub {
                Some(&link.remote_id)
            } else {
                None
            },
        );
        let (records, headers) = match encoded {
            Ok(v) => v,
            Err(e) => {
                for m in &exports {
                    self.connection.execute("UPDATE network_outbound_queue SET state='held',reason='unrepresentable',version=version+1 WHERE queue_id=?1",[&m.queue])?;
                }
                return Err(e.into());
            }
        };
        let name = if link.role == PartnerRole::Hub {
            format!("{}.MSG", link.remote_id)
        } else {
            "MESSAGES.DAT".into()
        };
        let mut files = BTreeMap::from([(name, records), ("HEADERS.DAT".into(), headers)]);
        if link.role == PartnerRole::Node {
            let created = chrono::DateTime::from_timestamp(now, 0)
                .ok_or(Error::Held)?
                .naive_utc();
            let conferences = mappings
                .iter()
                .filter(|m| m.enabled && m.outbound)
                .map(|m| (m.wire_conference, m.area.as_bytes().to_vec()))
                .collect();
            files.insert(
                "CONTROL.DAT".into(),
                qwk::control(
                    &qwk::Control {
                        board_id: link.local_id.clone(),
                        board_name: b"SPITFIRE NG".to_vec(),
                        caller: link.remote_id.as_bytes().to_vec(),
                        created,
                        conferences,
                    },
                    messages.len(),
                    qwk::Profile::ClassicCp437,
                )?,
            );
        }
        let bytes = qwk::archive(&files)?;
        let artifact = self.preserve_artifact(store, &bytes, now)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_link(&tx, link_id)?;
        if current.version != expected || !current.enabled || !current.outbound {
            return Err(Error::Conflict);
        }
        for m in &exports {
            let rechecked = export_member(&tx, &m.queue, &link, &load_mappings(&tx, link_id)?)?;
            if rechecked.publication != m.publication
                || rechecked.message_version != m.message_version
                || rechecked.policy != m.policy
                || rechecked.mapping != m.mapping
            {
                return Err(Error::Conflict);
            }
            let changed=tx.execute("UPDATE network_outbound_queue SET state='ready',artifact_id=?2,version=version+1 WHERE queue_id=?1 AND state='pending'",params![m.queue,artifact])?;
            if changed != 1 {
                return Err(Error::Conflict);
            }
        }
        event(&tx, "packet-built", now)?;
        tx.commit()?;
        Ok(Some(artifact))
    }
    /// Acknowledges a named immutable artifact, never merely successful packing.
    pub fn finish_qwk_handoff(
        &mut self,
        principal: &str,
        link_id: &str,
        expected: i64,
        artifact: &str,
        accepted: bool,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let link = load_link(&tx, link_id)?;
        if link.version != expected {
            return Err(Error::Conflict);
        }
        if !link.enabled || !link.outbound {
            return Err(Error::Disabled);
        }
        let mappings = load_mappings(&tx, link_id)?;
        let rows=tx.prepare("SELECT q.queue_id,q.state,q.attempts,q.created_at,q.next_attempt FROM network_outbound_queue q JOIN network_routing_decisions d ON q.queue_id=d.decision_id WHERE d.link_id=?1 AND q.artifact_id=?2 ORDER BY q.queue_id")?.query_map(params![link_id,artifact],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<i64>>(4)?)))?.collect::<Result<Vec<_>,_>>()?;
        if rows.is_empty() {
            return Err(Error::Held);
        }
        if rows.iter().all(|(_, state, _, _, _)| state == "accepted") {
            tx.commit()?;
            return Ok(());
        }
        for (queue, state, attempts, created, next) in rows {
            if state == "accepted" {
                continue;
            }
            if !matches!(state.as_str(), "ready" | "retry") || next.is_some_and(|n| n > now) {
                return Err(Error::Held);
            }
            export_member(&tx, &queue, &link, &mappings)?;
            let attempts = attempts + 1;
            let outcome = if accepted {
                "accepted"
            } else if attempts >= 12 || now.saturating_sub(created) >= 7 * 86400 {
                "failed"
            } else {
                "retry"
            };
            let next = if outcome == "retry" {
                Some(
                    now.saturating_add(
                        ((300i64 * (1i64 << (attempts - 1).min(7)))
                            + i64::from(queue.bytes().fold(0u8, u8::wrapping_add) % 31))
                        .min(21600),
                    ),
                )
            } else {
                None
            };
            tx.execute("INSERT INTO network_delivery_attempts(queue_id,occurred_at,outcome,attempt_number) VALUES(?1,?2,?3,?4)",params![queue,now,outcome,attempts])?;
            tx.execute("UPDATE network_outbound_queue SET state=?2,attempts=?3,next_attempt=?4,reason=?2,version=version+1 WHERE queue_id=?1",params![queue,outcome,attempts,next])?;
        }
        tx.execute(
            "UPDATE qwk_link_state SET last_contact=?2,last_result=?3 WHERE link_id=?1",
            params![link_id, now, if accepted { "accepted" } else { "retry" }],
        )?;
        audit(&tx, principal, "network.handoff", link_id, now)?;
        event(&tx, if accepted { "accepted" } else { "retry" }, now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn retry_qwk_network(
        &mut self,
        principal: &str,
        queue: &str,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (link_id,version,state,attempts):(String,i64,String,u32)=tx.query_row("SELECT d.link_id,q.version,q.state,q.attempts FROM network_outbound_queue q JOIN network_routing_decisions d ON q.queue_id=d.decision_id WHERE q.queue_id=?1",[queue],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        if expected != version {
            return Err(Error::Conflict);
        }
        if !matches!(state.as_str(), "retry" | "held") || attempts >= 12 {
            return Err(Error::Held);
        }
        let link = load_link(&tx, &link_id)?;
        if !link.enabled || !link.outbound {
            return Err(Error::Disabled);
        }
        export_member(&tx, queue, &link, &load_mappings(&tx, &link_id)?)?;
        tx.execute("UPDATE network_outbound_queue SET state=CASE WHEN artifact_id IS NULL THEN 'pending' ELSE 'ready' END,next_attempt=NULL,reason='manual-retry',version=version+1 WHERE queue_id=?1",[queue])?;
        audit(&tx, principal, "network.retry", queue, now)?;
        event(&tx, "retry-released", now)?;
        tx.commit()?;
        Ok(())
    }
}

impl RuntimeDatabase {
    /// A cold restore cannot know about acceptance after its snapshot. Preserve
    /// identities and artifacts, but require explicit revalidation of unsent work.
    pub fn hold_restored_qwk_network(&mut self) -> Result<(), Error> {
        self.connection.execute("UPDATE network_outbound_queue SET state='held',reason='restored-review',next_attempt=NULL,version=version+1 WHERE state IN ('pending','ready','retry')",[])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{ImportCapacity, ImportPermit};
    use crate::{
        BoardIdentity, CallerState, ConferenceAccessMode, ConferenceDefinition, CredentialHasher,
        MessageActor, MessageBackend, MessageKind, MessageVisibility, NewMessage,
        PasswordHashConfig, SecurityLevel,
    };
    use std::sync::Mutex;
    const NOW: i64 = 1_788_627_600;
    #[derive(Default)]
    struct Store(Mutex<BTreeMap<String, Vec<u8>>>, ImportCapacity);
    impl NetworkArtifactStore for Store {
        fn admit_import(&self) -> Result<ImportPermit<'_>, NetworkError> {
            self.1.acquire()
        }
        fn preserve(&self, b: &[u8]) -> Result<String, NetworkError> {
            let id = qwk::digest(b);
            self.0.lock().unwrap().insert(id.clone(), b.to_vec());
            Ok(id)
        }
        fn usage(&self) -> Result<(u64, usize), NetworkError> {
            let m = self.0.lock().unwrap();
            Ok((m.values().map(|b| b.len() as u64).sum(), m.len()))
        }
    }
    struct Fixture {
        temp: tempfile::TempDir,
        db: RuntimeDatabase,
        store: Store,
        actor: MessageActor,
        link: Link,
        mappings: Vec<Mapping>,
    }
    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let mut db = RuntimeDatabase::open(&temp.path().join("board.db")).unwrap();
        db.migrate().unwrap();
        db.ensure_board_identity(&BoardIdentity::new("Synthetic Network Board", "Sysop").unwrap())
            .unwrap();
        let hash = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
        .hash(b"synthetic-password")
        .unwrap();
        let caller = db
            .create_caller(
                b"Local Author",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                NOW,
            )
            .unwrap();
        let mut mappings = Vec::new();
        for number in [1, 2] {
            let c = db
                .ensure_conference(&ConferenceDefinition {
                    number,
                    name: format!("Area {number}"),
                    description: "Synthetic test".into(),
                    access_mode: ConferenceAccessMode::AtLeast,
                    read_security: SecurityLevel::new(5).unwrap(),
                    post_security: SecurityLevel::new(5).unwrap(),
                    public_only: true,
                    caller_deletion_enabled: true,
                    maximum_lines: 99,
                    privileged_security_levels: vec![],
                })
                .unwrap();
            mappings.push(Mapping {
                wire_conference: 2000 + number,
                area: format!("area-{number}"),
                conference_id: c.id.get(),
                enabled: true,
                inbound: true,
                outbound: true,
                version: 1,
            });
        }
        let link = Link {
            id: "first".into(),
            network: "testnet".into(),
            local_id: "LOCAL".into(),
            remote_id: "FIRST".into(),
            name: "Test Partner A".into(),
            profile: Profile::DoveHeaders,
            role: PartnerRole::Hub,
            enabled: true,
            inbound: true,
            outbound: true,
            version: 1,
        };
        db.configure_qwk_link("synthetic-operator", &link, &mappings, 0, NOW)
            .unwrap();
        Fixture {
            temp,
            db,
            store: Store::default(),
            actor: MessageActor::new(caller.id, SecurityLevel::new(100).unwrap()),
            link,
            mappings,
        }
    }
    fn post(f: &mut Fixture, area: usize) -> crate::Message {
        f.db.post(
            f.actor,
            NewMessage {
                conference_id: crate::ConferenceId::new(f.mappings[area].conference_id).unwrap(),
                recipient_caller_id: None,
                recipient_name: "All Callers".into(),
                subject: b"Native network message".to_vec(),
                body: b"Caf\x82 \xdb native bytes\r\n".to_vec(),
                created_at: NOW,
                parent_message_id: None,
                visibility: MessageVisibility::Public,
                kind: MessageKind::Standard,
            },
        )
        .unwrap()
    }
    fn incoming(id: &str, path: Vec<String>, conference: u16) -> Vec<u8> {
        let written = chrono::DateTime::from_timestamp(NOW, 0)
            .unwrap()
            .naive_utc();
        let m = wire::NetworkMessage {
            message: qwk::Message {
                number: 7,
                conference,
                reference: 0,
                private: false,
                received: false,
                to: b"All".to_vec(),
                from: b"Remote Author".to_vec(),
                subject: b"Peer subject".to_vec(),
                body: b"Peer caf\x82\n".to_vec(),
                wall_time: written,
            },
            metadata: wire::Metadata {
                id: id.into(),
                reply: None,
                path,
                utf8: false,
                written: None,
                fields: Vec::new(),
            },
            offset: 0,
            digest: String::new(),
        };
        let (records, headers) = wire::encode(&[m], None).unwrap();
        let control = qwk::control(
            &qwk::Control {
                board_id: "FIRST".into(),
                board_name: b"Independent test identity".to_vec(),
                caller: b"LOCAL".to_vec(),
                created: written,
                conferences: vec![(conference, b"Mapped".to_vec())],
            },
            1,
            qwk::Profile::ClassicCp437,
        )
        .unwrap();
        qwk::archive(&BTreeMap::from([
            ("MESSAGES.DAT".into(), records),
            ("HEADERS.DAT".into(), headers),
            ("CONTROL.DAT".into(), control),
        ]))
        .unwrap()
    }
    #[test]
    fn native_export_mapping_queue_retry_restart_and_idempotence() {
        let mut f = fixture();
        post(&mut f, 0);
        post(&mut f, 1);
        let a =
            f.db.build_qwk_network(&f.store, "first", 1, NOW)
                .unwrap()
                .unwrap();
        assert_eq!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW).unwrap(),
            Some(a.clone())
        );
        let bytes = f.store.0.lock().unwrap()[&a].clone();
        let packet = qwk::inspect(&bytes).unwrap();
        let decoded = wire::decode(&packet, Some("FIRST")).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(
            decoded
                .iter()
                .map(|m| m.message.conference)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([2001, 2002])
        );
        assert!(decoded
            .iter()
            .all(|m| m.message.from == b"Local Author" && m.metadata.path.is_empty()));
        f.db.finish_qwk_handoff("synthetic-operator", "first", 1, &a, false, NOW)
            .unwrap();
        assert!(f
            .db
            .finish_qwk_handoff("synthetic-operator", "first", 1, &a, true, NOW + 1)
            .is_err());
        drop(f.db);
        f.db = RuntimeDatabase::open(&f.temp.path().join("board.db")).unwrap();
        f.db.migrate().unwrap();
        let status = f.db.qwk_network_status().unwrap();
        assert_eq!(status[0].queue.len(), 2);
        assert!(status[0].queue.iter().all(|q| q.state == "retry"));
        for q in &status[0].queue {
            f.db.retry_qwk_network("synthetic-operator", &q.id, q.version, NOW + 1)
                .unwrap();
        }
        f.db.finish_qwk_handoff("synthetic-operator", "first", 1, &a, true, NOW + 2)
            .unwrap();
        f.db.finish_qwk_handoff("synthetic-operator", "first", 1, &a, true, NOW + 3)
            .unwrap();
        assert_eq!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW + 4)
                .unwrap(),
            None
        );
        f.db.validate_current_snapshot().unwrap();
    }
    #[test]
    fn import_native_authority_replay_and_legitimate_forwarding() {
        let mut f = fixture();
        let bytes = incoming("<peer-message@FIRST.qwk>", vec![], 2001);
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &bytes, NOW)
                .unwrap()
                .imported,
            1
        );
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &bytes, NOW + 1)
                .unwrap()
                .duplicates,
            1
        );
        assert_eq!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW).unwrap(),
            None
        );
        let mut second = f.link.clone();
        second.id = "second".into();
        second.remote_id = "SECOND".into();
        second.profile = Profile::QwkHeaders;
        f.db.configure_qwk_link("synthetic-operator", &second, &f.mappings, 0, NOW)
            .unwrap();
        let artifact =
            f.db.build_qwk_network(&f.store, "second", 1, NOW)
                .unwrap()
                .unwrap();
        let bytes = f.store.0.lock().unwrap()[&artifact].clone();
        let decoded = wire::decode(&qwk::inspect(&bytes).unwrap(), Some("SECOND")).unwrap();
        assert_eq!(decoded[0].metadata.id, "<peer-message@FIRST.qwk>");
        assert_eq!(decoded[0].metadata.path, vec!["FIRST"]);
        assert_eq!(decoded[0].message.from, b"Remote Author");
        let looped = incoming(
            "<peer-message@FIRST.qwk>",
            vec!["SECOND".into(), "LOCAL".into()],
            2001,
        );
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &looped, NOW)
                .unwrap()
                .loops,
            1
        );
        let (count,local_authors):(i64,i64)=f.db.connection.query_row("SELECT COUNT(*),COUNT(author_caller_id) FROM messages WHERE origin_kind='external-network'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!((count, local_authors), (1, 0));
        f.db.validate_current_snapshot().unwrap();
    }
    #[test]
    fn disabled_invalid_mapping_spoof_and_malformed_are_safe() {
        let mut f = fixture();
        let mut duplicates = f.mappings.clone();
        duplicates.push(duplicates[0].clone());
        assert!(f
            .db
            .configure_qwk_link("synthetic-operator", &f.link, &duplicates, 0, NOW)
            .is_err());
        for bytes in [
            incoming("<bad@FIRST.qwk>", vec![], 999),
            incoming("<forged@LOCAL.qwk>", vec![], 2001),
            b"not a packet".to_vec(),
        ] {
            assert_eq!(
                f.db.ingest_qwk_network(&f.store, "first", 1, &bytes, NOW)
                    .unwrap()
                    .quarantined,
                1
            );
        }
        assert!(f
            .db
            .ingest_qwk_network(&f.store, "unknown", 1, b"x", NOW)
            .is_err());
        assert!(f
            .db
            .ingest_qwk_network(&f.store, "first", 1, &vec![0; qwk::MAX_ARCHIVE + 1], NOW)
            .is_err());
        post(&mut f, 0);
        f.db.build_qwk_network(&f.store, "first", 1, NOW).unwrap();
        f.link.enabled = false;
        f.link.version = 2;
        for m in &mut f.mappings {
            m.version = 2;
        }
        f.db.configure_qwk_link("synthetic-operator", &f.link, &f.mappings, 1, NOW)
            .unwrap();
        assert!(f.db.build_qwk_network(&f.store, "first", 2, NOW).is_err());
        assert!(f
            .db
            .ingest_qwk_network(&f.store, "first", 2, b"x", NOW)
            .is_err());
        assert!(f.db.qwk_network_status().unwrap()[0]
            .queue
            .iter()
            .all(|q| q.state == "held"));
        let leaked:bool=f.db.connection.query_row("SELECT EXISTS(SELECT 1 FROM operational_events WHERE text_value_1 LIKE '%Remote Author%' OR text_value_1 LIKE '%Peer subject%')",[],|r|r.get(0)).unwrap();
        assert!(!leaked);
    }
    #[test]
    fn receipt_failure_rolls_back_native_import() {
        let mut f = fixture();
        f.db.connection.execute_batch("CREATE TRIGGER synthetic_receipt_failure BEFORE INSERT ON qwk_network_import_receipts BEGIN SELECT RAISE(ABORT,'synthetic'); END;").unwrap();
        assert!(f
            .db
            .ingest_qwk_network(
                &f.store,
                "first",
                1,
                &incoming("<unique@FIRST.qwk>", vec![], 2001),
                NOW
            )
            .is_err());
        assert_eq!(
            f.db.connection
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            f.db.connection
                .query_row("SELECT COUNT(*) FROM network_publications", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    fn alter_packet(bytes: &[u8], edit: impl FnOnce(&mut wire::NetworkMessage)) -> Vec<u8> {
        let mut p = qwk::inspect(bytes).unwrap();
        let mut m = wire::decode(&p, None).unwrap();
        edit(&mut m[0]);
        let (records, headers) = wire::encode(&m, None).unwrap();
        p.members.insert("MESSAGES.DAT".into(), records);
        p.members.insert("HEADERS.DAT".into(), headers);
        qwk::archive(&p.members).unwrap()
    }
    #[test]
    fn network_identity_collision_recompression_and_cross_namespace_isolation() {
        let mut f = fixture();
        let b = incoming("<stable@FIRST.qwk>", vec![], 2001);
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &b, NOW)
                .unwrap()
                .imported,
            1
        );
        let mut p = qwk::inspect(&b).unwrap();
        p.members
            .insert("DOOR.ID".into(), b"Synthetic repack".to_vec());
        assert_eq!(
            f.db.ingest_qwk_network(
                &f.store,
                "first",
                1,
                &qwk::archive(&p.members).unwrap(),
                NOW
            )
            .unwrap()
            .duplicates,
            1
        );
        let changed = alter_packet(&b, |m| m.message.body = b"Conflicting content\n".to_vec());
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &changed, NOW)
                .unwrap()
                .quarantined,
            1
        );
        let mut second = f.link.clone();
        second.id = "other".into();
        second.network = "othernet".into();
        second.remote_id = "OTHER".into();
        f.db.configure_qwk_link("operator", &second, &f.mappings, 0, NOW)
            .unwrap();
        assert_eq!(
            f.db.build_qwk_network(&f.store, "other", 1, NOW).unwrap(),
            None
        );
        assert!(f
            .db
            .connection
            .execute("UPDATE network_publications SET origin='FORGED'", [])
            .is_err());
        assert!(f
            .db
            .connection
            .execute("DELETE FROM network_publication_path", [])
            .is_err());
    }
    #[test]
    fn strict_unicode_native_read_and_forwarding_preserve_source_time_and_recipient() {
        let mut f = fixture();
        let b = alter_packet(&incoming("<unicode@FIRST.qwk>", vec![], 2001), |m| {
            m.metadata.utf8 = true;
            m.message.subject = "Unicode 界".as_bytes().to_vec();
            m.message.body = "Peer café 界\n".as_bytes().to_vec();
            m.message.to = b"Named External Recipient".to_vec();
            m.metadata.written = Some("20260905170500Z 0000".into());
        });
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &b, NOW + 3600)
                .unwrap()
                .imported,
            1
        );
        let c = crate::ConferenceId::new(f.mappings[0].conference_id).unwrap();
        let m = f.db.message(f.actor, c, 1).unwrap();
        assert_eq!(m.encoding, crate::message::MessageEncoding::Utf8);
        assert_eq!(m.origin, crate::message::MessageOrigin::ExternalNetwork);
        assert_eq!(m.created_at, NOW + 300);
        assert_eq!(m.body, "Peer café 界\n".as_bytes());
        assert!(m.encoding.cp437(&m.body).is_none());
        assert!(String::from_utf8_lossy(&m.encoding.display_cp437(&m.body)).contains("[U+754C]"));
        let mut second = f.link.clone();
        second.id = "second".into();
        second.remote_id = "SECOND".into();
        f.db.configure_qwk_link("operator", &second, &f.mappings, 0, NOW)
            .unwrap();
        let artifact =
            f.db.build_qwk_network(&f.store, "second", 1, NOW + 7200)
                .unwrap()
                .unwrap();
        let bytes = f.store.0.lock().unwrap()[&artifact].clone();
        let out = wire::decode(&qwk::inspect(&bytes).unwrap(), Some("SECOND")).unwrap();
        assert_eq!(out[0].message.to, b"Named External Recipient");
        assert_eq!(out[0].message.body, m.body);
        assert_eq!(
            out[0].metadata.written.as_deref(),
            Some("20260905170500Z 0000")
        );
    }
    #[test]
    fn unique_reply_identity_binds_native_parent_without_numeric_guessing() {
        let mut f = fixture();
        let parent = incoming("<parent@FIRST.qwk>", vec![], 2001);
        f.db.ingest_qwk_network(&f.store, "first", 1, &parent, NOW)
            .unwrap();
        for (id, reply) in [
            ("<reply@FIRST.qwk>", "<parent@FIRST.qwk>"),
            ("<orphan@FIRST.qwk>", "<unknown@FIRST.qwk>"),
        ] {
            let b = alter_packet(&incoming(id, vec![], 2001), |m| {
                m.metadata.reply = Some(reply.into());
                m.message.reference = 1;
            });
            f.db.ingest_qwk_network(&f.store, "first", 1, &b, NOW)
                .unwrap();
        }
        let c = crate::ConferenceId::new(f.mappings[0].conference_id).unwrap();
        let parent = f.db.message(f.actor, c, 1).unwrap();
        assert_eq!(
            f.db.message(f.actor, c, 2).unwrap().parent_message_id,
            Some(parent.id)
        );
        assert_eq!(f.db.message(f.actor, c, 3).unwrap().parent_message_id, None);
    }
    #[test]
    fn private_mail_and_conflicting_partner_display_cannot_be_publicized() {
        let mut f = fixture();
        let b = alter_packet(&incoming("<private@FIRST.qwk>", vec![], 2001), |m| {
            m.message.private = true
        });
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, "first", 1, &b, NOW)
                .unwrap()
                .quarantined,
            1
        );
        let mut p = qwk::inspect(&incoming("<spoof@FIRST.qwk>", vec![], 2001)).unwrap();
        let c = p.members.get_mut("CONTROL.DAT").unwrap();
        let t = String::from_utf8(c.clone())
            .unwrap()
            .replace("FIRST", "SPOOF");
        *c = t.into_bytes();
        assert_eq!(
            f.db.ingest_qwk_network(
                &f.store,
                "first",
                1,
                &qwk::archive(&p.members).unwrap(),
                NOW
            )
            .unwrap()
            .quarantined,
            1
        );
        assert_eq!(
            f.db.connection
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn bounded_queue_pages_retry_exhaustion_and_policy_revalidation() {
        let mut f = fixture();
        for _ in 0..23 {
            post(&mut f, 0);
        }
        let artifact =
            f.db.build_qwk_network(&f.store, "first", 1, NOW)
                .unwrap()
                .unwrap();
        let p = f.db.qwk_network_queue("first", None).unwrap();
        assert_eq!(p.items.len(), 20);
        let next = f.db.qwk_network_queue("first", p.next.as_deref()).unwrap();
        assert_eq!(next.items.len(), 3);
        assert!(next.next.is_none());
        assert_eq!(
            f.db.qwk_network_status().unwrap()[0].queue_counts["ready"],
            23
        );
        for attempt in 0..12 {
            f.db.finish_qwk_handoff(
                "operator",
                "first",
                1,
                &artifact,
                false,
                NOW + attempt * 22000,
            )
            .unwrap();
            if attempt < 11 {
                for q in p.items.iter().chain(next.items.iter()) {
                    let version =
                        f.db.connection
                            .query_row(
                                "SELECT version FROM network_outbound_queue WHERE queue_id=?1",
                                [&q.id],
                                |r| r.get(0),
                            )
                            .unwrap();
                    f.db.retry_qwk_network("operator", &q.id, version, NOW + attempt * 22000)
                        .unwrap();
                }
            }
        }
        assert_eq!(
            f.db.qwk_network_status().unwrap()[0].queue_counts["failed"],
            23
        );
        assert!(f
            .db
            .connection
            .execute(
                "UPDATE network_routing_decisions SET destination='WRONG'",
                []
            )
            .is_err());
    }
    #[test]
    fn dove_profile_is_per_link_and_mapping_versions_hold_materialized_packets() {
        let mut f = fixture();
        let mut second = f.link.clone();
        second.id = "second".into();
        second.remote_id = "SECOND".into();
        let mut maps = f.mappings.clone();
        maps[0].wire_conference = 2030;
        assert!(matches!(
            f.db.configure_qwk_link("operator", &second, &maps, 0, NOW),
            Err(Error::InvalidMapping)
        ));
        second.profile = Profile::QwkHeaders;
        f.db.configure_qwk_link("operator", &second, &maps, 0, NOW)
            .unwrap();
        post(&mut f, 0);
        let artifact =
            f.db.build_qwk_network(&f.store, "second", 1, NOW)
                .unwrap()
                .unwrap();
        second.version = 2;
        for m in &mut maps {
            m.version = 2;
        }
        maps[0].enabled = false;
        f.db.configure_qwk_link("operator", &second, &maps, 1, NOW)
            .unwrap();
        let q =
            f.db.qwk_network_queue("second", None)
                .unwrap()
                .items
                .remove(0);
        assert_eq!(q.state, "held");
        assert!(f
            .db
            .finish_qwk_handoff("operator", "second", 2, &artifact, true, NOW)
            .is_err());
        assert!(f
            .db
            .retry_qwk_network("operator", &q.id, q.version, NOW)
            .is_err());
    }
    #[test]
    fn history_and_quarantine_pressure_never_creates_partial_native_messages() {
        let mut f = fixture();
        f.db.connection
            .execute(
                "UPDATE network_history_capacity SET reserved_bytes=536870000",
                [],
            )
            .unwrap();
        assert!(matches!(
            f.db.ingest_qwk_network(
                &f.store,
                "first",
                1,
                &incoming("<pressure@FIRST.qwk>", vec![], 2001),
                NOW
            ),
            Err(Error::Capacity)
        ));
        assert_eq!(
            f.db.connection
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn one_stale_member_holds_the_whole_immutable_packet_on_every_build() {
        let mut f = fixture();
        post(&mut f, 0);
        post(&mut f, 1);
        let artifact =
            f.db.build_qwk_network(&f.store, "first", 1, NOW)
                .unwrap()
                .unwrap();
        f.db.connection
            .execute(
                "UPDATE message_conferences SET active=0 WHERE conference_id=?1",
                [f.mappings[0].conference_id],
            )
            .unwrap();
        assert!(matches!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW),
            Err(Error::Held)
        ));
        assert_eq!(
            f.db.qwk_network_status().unwrap()[0].queue_counts["held"],
            2
        );
        assert!(!matches!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW),
            Ok(Some(_))
        ));
        assert!(f
            .db
            .finish_qwk_handoff("operator", "first", 1, &artifact, true, NOW)
            .is_err());
        let q =
            f.db.qwk_network_queue("first", None)
                .unwrap()
                .items
                .remove(0);
        // Releasing only an individually valid member cannot release its stale sibling.
        let _ = f.db.retry_qwk_network("operator", &q.id, q.version, NOW);
        assert!(!matches!(
            f.db.build_qwk_network(&f.store, "first", 1, NOW),
            Ok(Some(_))
        ));
    }
    fn mail_policy(f: &Fixture, link: &Link) -> MailPolicy {
        MailPolicy {
            link: link.id.clone(),
            enabled: true,
            inbound: true,
            outbound: true,
            transit: true,
            version: 1,
            aliases: vec![MailboxAlias {
                alias: "Local Author".into(),
                caller_id: f.actor.caller_id().get(),
            }],
            destinations: vec![link.remote_id.clone()],
        }
    }
    fn mail() -> NewNetworkMail {
        NewNetworkMail {
            network: "testnet".into(),
            destination: "FIRST".into(),
            recipient: "Remote Author".into(),
            subject: b"Private subject".to_vec(),
            body: b"Private sentinel caf\x82\r\n".to_vec(),
            reply_to: None,
        }
    }
    fn private_packet(
        id: &str,
        recipient: &str,
        destination: Option<&str>,
        reply: Option<String>,
    ) -> Vec<u8> {
        let public = qwk::inspect(&incoming(id, vec![], 2001)).unwrap();
        let mut member = wire::decode(&public, None).unwrap().remove(0);
        member.message.private = true;
        member.message.conference = 0;
        member.message.to = recipient.as_bytes().to_vec();
        member.message.body = b"Private inbound sentinel caf\x82\n".to_vec();
        member.metadata.reply = reply;
        member.metadata.fields = destination
            .map(|s| vec![("recipientnetaddr".into(), s.as_bytes().to_vec())])
            .unwrap_or_default();
        let (records, headers) = wire::encode(&[member], None).unwrap();
        qwk::archive(&BTreeMap::from([
            ("MESSAGES.DAT".into(), records),
            ("HEADERS.DAT".into(), headers),
            ("CONTROL.DAT".into(), public.members["CONTROL.DAT"].clone()),
        ]))
        .unwrap()
    }
    #[test]
    fn private_native_queue_recipient_access_reply_and_repacked_identity() {
        let mut f = fixture();
        let policy = mail_policy(&f, &f.link);
        f.db.configure_qwk_mail("operator", &policy, 0, NOW)
            .unwrap();
        let sent = f.db.send_qwk_mail(f.actor, &mail(), NOW).unwrap();
        assert_eq!(
            f.db.qwk_network_queue(&f.link.id, None).unwrap().items[0].state,
            "pending"
        );
        assert_eq!(f.db.read_qwk_mail(f.actor, sent).unwrap().body, mail().body);
        assert!(f
            .db
            .messages(
                f.actor,
                crate::ConferenceId::new(f.mappings[0].conference_id).unwrap()
            )
            .unwrap()
            .is_empty());
        let artifact =
            f.db.build_qwk_network(&f.store, &f.link.id, 1, NOW)
                .unwrap()
                .unwrap();
        let packet = qwk::inspect(&f.store.0.lock().unwrap()[&artifact]).unwrap();
        let out = wire::decode(&packet, Some("FIRST")).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].message.private);
        assert_eq!(out[0].message.conference, 0);
        let inbound = private_packet(
            "<private@FIRST.test>",
            "Local Author",
            Some("LOCAL"),
            Some(out[0].metadata.id.clone()),
        );
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &inbound, NOW)
                .unwrap()
                .imported,
            1
        );
        let ids = f.db.qwk_mailbox(f.actor, None).unwrap();
        assert_eq!(ids.len(), 2);
        let received = *ids.last().unwrap();
        let read = f.db.read_qwk_mail(f.actor, received).unwrap();
        assert_eq!(read.parent, Some(sent));
        assert_eq!(read.origin, "FIRST");
        assert_eq!(read.author, "Remote Author");
        let unauthorized = MessageActor::new(
            crate::CallerId::new(999).unwrap(),
            SecurityLevel::new(999).unwrap(),
        );
        assert!(f.db.read_qwk_mail(unauthorized, received).is_err());
        assert!(f
            .db
            .messages(
                f.actor,
                crate::ConferenceId::new(f.mappings[0].conference_id).unwrap()
            )
            .unwrap()
            .is_empty());
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &inbound, NOW)
                .unwrap()
                .duplicates,
            1
        );
        let mut repacked = qwk::inspect(&inbound).unwrap().members;
        repacked.insert("DOOR.ID".into(), b"DOOR = synthetic\r\n".to_vec());
        assert_eq!(
            f.db.ingest_qwk_network(
                &f.store,
                &f.link.id,
                1,
                &qwk::archive(&repacked).unwrap(),
                NOW
            )
            .unwrap()
            .duplicates,
            1
        );
        f.db.finish_qwk_handoff("operator", &f.link.id, 1, &artifact, true, NOW)
            .unwrap();
        assert!(f
            .db
            .build_qwk_network(&f.store, &f.link.id, 1, NOW)
            .unwrap()
            .is_none());
        let mut reply = mail();
        reply.reply_to = Some(received);
        f.db.send_qwk_mail(f.actor, &reply, NOW).unwrap();
        let count: i64 =
            f.db.connection
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE conference_id IS NOT NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn private_transit_explicit_next_hop_no_broadcast_no_return_and_restart() {
        let mut f = fixture();
        let a = mail_policy(&f, &f.link);
        f.db.configure_qwk_mail("operator", &a, 0, NOW).unwrap();
        let mut second = f.link.clone();
        second.id = "second".into();
        second.remote_id = "SECOND".into();
        second.profile = Profile::QwkHeaders;
        f.db.configure_qwk_link("operator", &second, &f.mappings, 0, NOW)
            .unwrap();
        let b = mail_policy(&f, &second);
        f.db.configure_qwk_mail("operator", &b, 0, NOW).unwrap();
        let input = private_packet(
            "<transit@FIRST.test>",
            "Transit Recipient",
            Some("SECOND"),
            None,
        );
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &input, NOW)
                .unwrap()
                .imported,
            1
        );
        assert!(f.db.qwk_mailbox(f.actor, None).unwrap().is_empty());
        assert!(f
            .db
            .qwk_network_queue(&f.link.id, None)
            .unwrap()
            .items
            .is_empty());
        assert_eq!(
            f.db.qwk_network_queue("second", None).unwrap().items.len(),
            1
        );
        let mid =
            f.db.connection
                .query_row("SELECT message_id FROM messages", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap();
        assert!(f
            .db
            .read_qwk_mail(f.actor, crate::MessageId::new(mid).unwrap())
            .is_err());
        let artifact =
            f.db.build_qwk_network(&f.store, "second", 1, NOW)
                .unwrap()
                .unwrap();
        let packet = qwk::inspect(&f.store.0.lock().unwrap()[&artifact]).unwrap();
        let out = wire::decode(&packet, Some("SECOND")).unwrap();
        assert_eq!(out[0].metadata.path, vec!["FIRST"]);
        assert!(out[0].message.private);
        assert_eq!(out[0].message.to, b"Transit Recipient");
        f.db.finish_qwk_handoff("operator", "second", 1, &artifact, false, NOW)
            .unwrap();
        drop(f.db);
        f.db = RuntimeDatabase::open(&f.temp.path().join("board.db")).unwrap();
        let row =
            f.db.qwk_network_queue("second", None)
                .unwrap()
                .items
                .remove(0);
        assert_eq!(row.state, "retry");
        f.db.retry_qwk_network("operator", &row.id, row.version, NOW)
            .unwrap();
        assert_eq!(
            f.db.build_qwk_network(&f.store, "second", 1, NOW).unwrap(),
            Some(artifact.clone())
        );
        f.db.finish_qwk_handoff("operator", "second", 1, &artifact, true, NOW)
            .unwrap();
        f.db.finish_qwk_handoff("operator", "second", 1, &artifact, true, NOW)
            .unwrap();
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &input, NOW)
                .unwrap()
                .duplicates,
            1
        );
        let looped = private_packet("<return@FIRST.test>", "Remote Author", Some("FIRST"), None);
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &looped, NOW)
                .unwrap()
                .loops,
            1
        );
        assert_eq!(
            f.db.connection
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn private_policy_unknown_recipient_spoof_and_atomic_receipt_failure() {
        let mut f = fixture();
        let p = mail_policy(&f, &f.link);
        assert!(matches!(
            f.db.configure_qwk_mail("operator", &p, i64::MAX, NOW),
            Err(Error::InvalidMapping)
        ));
        f.db.configure_qwk_mail("operator", &p, 0, NOW).unwrap();
        for (id, to, destination) in [
            ("<bad@FIRST.test>", "Unknown Recipient", None),
            ("<spoof@LOCAL.qwk>", "Local Author", None),
            ("<route@FIRST.test>", "Local Author", Some("UNKNOWN")),
        ] {
            let packet = private_packet(id, to, destination, None);
            assert_eq!(
                f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &packet, NOW)
                    .unwrap()
                    .quarantined,
                1
            );
        }
        let mut invalid = p.clone();
        invalid.version = 2;
        invalid.aliases.push(invalid.aliases[0].clone());
        assert!(f
            .db
            .configure_qwk_mail("operator", &invalid, 1, NOW)
            .is_err());
        assert_eq!(f.db.qwk_mail_policy(&f.link.id).unwrap(), p);
        f.db.connection.execute_batch("CREATE TRIGGER private_receipt_failure BEFORE INSERT ON qwk_network_import_receipts WHEN NEW.outcome='imported' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        let packet = private_packet("<rollback@FIRST.test>", "Local Author", None, None);
        assert!(f
            .db
            .ingest_qwk_network(&f.store, &f.link.id, 1, &packet, NOW)
            .is_err());
        assert!(f.db.qwk_mailbox(f.actor, None).unwrap().is_empty());
        f.db.connection
            .execute_batch("DROP TRIGGER private_receipt_failure;")
            .unwrap();
        assert_eq!(
            f.db.ingest_qwk_network(&f.store, &f.link.id, 1, &packet, NOW)
                .unwrap()
                .imported,
            1
        );
        let mut disabled = p.clone();
        disabled.version = 2;
        disabled.enabled = false;
        f.db.configure_qwk_mail("operator", &disabled, 1, NOW)
            .unwrap();
        assert!(f.db.send_qwk_mail(f.actor, &mail(), NOW).is_err());
        let summaries = format!("{:?}", f.db.qwk_network_status().unwrap());
        assert!(!summaries.contains("sentinel"));
        assert!(!summaries.contains("Private subject"));
        let public = post(&mut f, 0);
        assert_eq!(
            f.db.messages(f.actor, public.conference_id).unwrap().len(),
            1
        );
        assert!(f
            .db
            .build_qwk_network(&f.store, &f.link.id, 1, NOW)
            .unwrap()
            .is_some());
    }
}
