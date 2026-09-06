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

//! Native FTN message, directory and queue authority. Transport remains N4.
pub mod files;
pub(crate) const FILE_MIGRATION: &str = include_str!("files.sql");
mod binkp;
pub(crate) use binkp::BINKP_MIGRATION;
pub use binkp::*;
mod operations;
pub use operations::*;
mod recovery;
pub use recovery::*;
mod directory;
mod hub;
mod mail;
pub use hub::*;
pub(crate) const HUB_MIGRATION: &str = include_str!("hub.sql");
mod policy;
use crate::{network::NetworkArtifactStore, RuntimeDatabase};
pub use directory::*;
pub use mail::*;
pub use policy::*;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sf_net::ftn::{
    self as wire, Address, Charset, Domain, Endpoint, PackedMessage, Packet, PacketHeader,
    PacketProfile, Text,
};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const MIGRATION: &str = include_str!("schema.sql");
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid FTN configuration")]
    Policy,
    #[error("FTN configuration or work version changed")]
    Conflict,
    #[error("FTN admission or message access denied")]
    Denied,
    #[error("FTN resource capacity reached")]
    Capacity,
    #[error("FTN routing failed or requires review")]
    Routing,
    #[error("FTN work is held")]
    Held,
    #[error("FTN message rejected")]
    Rejected,
    #[error("FTN directory conflict")]
    DirectoryConflict,
    #[error("FTN codec rejected input")]
    Codec(#[from] wire::Error),
    #[error("FTN database operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("FTN artifact operation failed")]
    Artifact(#[from] crate::network::NetworkError),
    #[error("native message operation failed")]
    Message(#[from] crate::MessageError),
    #[error("FTN event operation failed")]
    Database(#[from] crate::DatabaseError),
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b))
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
            if code.contains("rejected") || code.contains("failure") || code.contains("failed") {
                crate::EventSeverity::Warning
            } else {
                crate::EventSeverity::Info
            },
            format!("message.ftn.{code}"),
            if code.contains("rejected") || code.contains("failure") || code.contains("failed") {
                crate::EventOutcome::Failed
            } else {
                crate::EventOutcome::Succeeded
            },
        ),
    )?;
    Ok(())
}
fn audit(tx: &Transaction<'_>, principal: &str, code: &str, now: i64) -> Result<(), Error> {
    if principal.is_empty() || principal.len() > 64 || principal.chars().any(char::is_control) {
        return Err(Error::Denied);
    }
    tx.execute("INSERT INTO operator_control_audit(occurred_at,operator_kind,operator_id,operation,authorization_result,target_kind,outcome) VALUES(?1,'host-operator',?2,?3,'allowed','ftn','succeeded')",params![now,principal,code])?;
    Ok(())
}
fn address_id(tx: &rusqlite::Connection, e: &Endpoint) -> Result<i64, Error> {
    let a = e.address;
    tx.execute(
        "INSERT OR IGNORE INTO ftn_addresses(domain,zone,net,node,point) VALUES(?1,?2,?3,?4,?5)",
        params![e.domain.as_str(), a.zone(), a.net(), a.node(), a.point()],
    )?;
    Ok(tx.query_row("SELECT address_id FROM ftn_addresses WHERE domain=?1 AND zone=?2 AND net=?3 AND node=?4 AND point=?5",params![e.domain.as_str(),a.zone(),a.net(),a.node(),a.point()],|r|r.get(0))?)
}
fn endpoint(conn: &rusqlite::Connection, id: i64) -> Result<Endpoint, Error> {
    let (d, z, n, f, p): (String, u16, u16, u16, u16) = conn.query_row(
        "SELECT domain,zone,net,node,point FROM ftn_addresses WHERE address_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    Ok(Endpoint {
        domain: d.parse()?,
        address: Address::new(z, n, f, p)?,
    })
}
fn bind(tx: &Transaction<'_>, policy: &Policy) -> Result<String, Error> {
    policy.validate()?;
    if !policy.enabled {
        return Err(Error::Denied);
    }
    let digest = policy.digest()?;
    for a in &policy.akas {
        let addr = address_id(tx, &a.endpoint)?;
        tx.execute("INSERT OR IGNORE INTO ftn_serials(address_id,next_serial,restored_hold) VALUES(?1,1,0)",[addr])?;
    }
    for l in &policy.links {
        let remote = address_id(tx, &l.remote)?;
        let local = address_id(tx, &policy.aka(&l.aka)?.endpoint)?;
        let prior: Option<(i64, i64)> = tx
            .query_row(
                "SELECT remote_address,local_address FROM ftn_link_bindings WHERE link_id=?1",
                [&l.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if prior.is_some_and(|p| p != (remote, local)) {
            return Err(Error::Conflict);
        }
        tx.execute(
            "INSERT OR IGNORE INTO ftn_link_bindings VALUES(?1,?2,?3)",
            params![l.id, remote, local],
        )?;
    }
    tx.execute("UPDATE network_outbound_queue SET state='held',reason='ftn-policy-changed',version=version+1 WHERE state IN ('pending','ready','retry') AND queue_id IN (SELECT queue_id FROM ftn_routing_decisions WHERE policy_digest<>?1)",[&digest])?;
    Ok(digest)
}
fn quarantine(
    tx: &Transaction<'_>,
    link: Option<&str>,
    artifact: &str,
    reason: &str,
    now: i64,
) -> Result<(), Error> {
    let(count,bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(a.byte_length),0) FROM network_quarantine q LEFT JOIN network_artifacts a USING(artifact_id)",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let size: i64 = tx.query_row(
        "SELECT byte_length FROM network_artifacts WHERE artifact_id=?1",
        [artifact],
        |r| r.get(0),
    )?;
    if count >= 128 || bytes + size > 128 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    tx.execute("INSERT OR IGNORE INTO network_quarantine(artifact_id,reason,received_at,ftn_link) VALUES(?1,?2,?3,?4)",params![artifact,reason,now,link])?;
    event(tx, "quarantine-created", now)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    pub domain: Domain,
    pub area: String,
    pub conference_id: i64,
    pub aka: String,
    pub receive: bool,
    pub send: bool,
    pub origin: String,
    pub links: Vec<String>,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MailboxAlias {
    pub aka: String,
    pub alias: String,
    pub caller_id: i64,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub queue: BTreeMap<String, i64>,
    pub quarantine: i64,
    pub active_generations: i64,
    pub last_result: Option<String>,
}
impl RuntimeDatabase {
    pub fn configure_ftn_mapping(
        &mut self,
        policy: &Policy,
        principal: &str,
        m: &Mapping,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        if expected.checked_add(1) != Some(m.version)
            || expected < 0
            || !wire::valid_area(&m.area)
            || m.links.len() > 32
            || m.origin.is_empty()
            || m.origin.len() > 50
            || !m.origin.is_ascii()
            || m.origin.chars().any(char::is_control)
            || m.origin.contains(['(', ')'])
        {
            return Err(Error::Policy);
        }
        let a = policy.aka(&m.aka)?;
        if a.endpoint.domain != m.domain {
            return Err(Error::Policy);
        }
        let mut links = BTreeSet::new();
        for l in &m.links {
            let l = policy.link(l)?;
            if l.remote.domain != m.domain
                || l.remote.address.zone() != a.endpoint.address.zone()
                || !links.insert(&l.id)
            {
                return Err(Error::Policy);
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        for link in &m.links {
            if hub::downstream(&tx, link)?.is_some() {
                return Err(Error::Policy);
            }
        }
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM message_conferences WHERE conference_id=?1 AND active=1)",
            [m.conference_id],
            |r| r.get(0),
        )?;
        if !active {
            return Err(Error::Policy);
        }
        let prior: i64 = tx
            .query_row(
                "SELECT version FROM ftn_area_mappings WHERE domain=?1 AND area=?2",
                params![m.domain.as_str(), m.area],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if prior != expected {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO ftn_area_mappings VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(domain,area) DO UPDATE SET conference_id=excluded.conference_id,aka=excluded.aka,receive=excluded.receive,send=excluded.send,origin=excluded.origin,version=excluded.version",params![m.domain.as_str(),m.area,m.conference_id,m.aka,m.receive,m.send,m.origin,m.version])?;
        tx.execute(
            "DELETE FROM ftn_area_links WHERE domain=?1 AND area=?2",
            params![m.domain.as_str(), m.area],
        )?;
        for l in &m.links {
            tx.execute(
                "INSERT INTO ftn_area_links VALUES(?1,?2,?3)",
                params![m.domain.as_str(), m.area, l],
            )?;
        }
        tx.execute("UPDATE network_outbound_queue SET state='held',reason='ftn-area-changed',version=version+1 WHERE state IN ('pending','ready','retry') AND queue_id IN (SELECT d.queue_id FROM ftn_routing_decisions d JOIN ftn_messages f USING(publication_id) WHERE f.domain=?1 AND f.area=?2)",params![m.domain.as_str(),m.area])?;
        audit(&tx, principal, "ftn.area-changed", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn configure_ftn_alias(
        &mut self,
        policy: &Policy,
        principal: &str,
        a: &MailboxAlias,
        now: i64,
    ) -> Result<(), Error> {
        let aka = policy.aka(&a.aka)?;
        if a.alias.is_empty()
            || a.alias.len() > 35
            || a.alias.trim() != a.alias
            || !a
                .alias
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" .-_".contains(&b))
        {
            return Err(Error::Policy);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        let aid = address_id(&tx, &aka.endpoint)?;
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM callers WHERE caller_id=?1 AND account_state='active')",
            [a.caller_id],
            |r| r.get(0),
        )?;
        if !active {
            return Err(Error::Denied);
        }
        tx.execute("INSERT INTO ftn_mailbox_aliases VALUES(?1,?2,?3) ON CONFLICT(address_id,alias) DO UPDATE SET caller_id=excluded.caller_id",params![aid,a.alias,a.caller_id])?;
        audit(&tx, principal, "ftn.alias-changed", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn ftn_status(&self) -> Result<Status, Error> {
        let queue=self.connection.prepare("SELECT q.state,COUNT(*) FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) GROUP BY q.state")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?;
        Ok(Status {
            queue,
            quarantine: self.connection.query_row(
                "SELECT COUNT(*) FROM network_quarantine WHERE reason LIKE 'ftn-%'",
                [],
                |r| r.get(0),
            )?,
            active_generations: self.connection.query_row(
                "SELECT COUNT(*) FROM ftn_directory_active",
                [],
                |r| r.get(0),
            )?,
            last_result: self
                .connection
                .query_row(
                    "SELECT outcome FROM ftn_import_receipts ORDER BY received_at DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?,
        })
    }
    pub fn hold_restored_ftn(&mut self) -> Result<(), Error> {
        let tx = self.connection.transaction()?;
        tx.execute("UPDATE ftn_serials SET restored_hold=1", [])?;
        tx.execute("UPDATE network_outbound_queue SET state='held',reason='restored-ftn',version=version+1 WHERE state IN ('pending','ready','retry') AND queue_id IN (SELECT queue_id FROM ftn_routing_decisions)",[])?;
        let has_files: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ftn_file_deliveries')",
            [],
            |r| r.get(0),
        )?;
        if has_files {
            tx.execute("UPDATE ftn_file_deliveries SET held=1,last_error='restored-file-review',session_id=NULL,payload_offered=0,tic_offered=0,version=version+1 WHERE accepted_at IS NULL",[])?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
