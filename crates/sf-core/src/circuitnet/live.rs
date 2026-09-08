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

//! Durable live-link policy; sockets and private keys belong to the host.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listener: Option<std::net::SocketAddr>,
    /// Public DER certificate. The host stores PKCS#8 separately under SYSTEM.
    pub certificate: Vec<u8>,
    pub peers: Vec<Peer>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {
    pub node: NodeId,
    pub host: String,
    pub port: u16,
    pub server_name: String,
    pub certificate: Vec<u8>,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub held: bool,
}
impl Config {
    pub fn validate(&self, p: &Profile) -> Result<(), Error> {
        p.validate()?;
        if self.peers.len() >= wire::MAX_NODES
            || self.certificate.len() > 16384
            || (self.listener.is_some() && self.certificate.is_empty())
            || self.listener.is_some_and(|a| a.port() == 0)
        {
            return Err(Error::Policy);
        }
        let mut nodes = BTreeSet::new();
        let mut certificates = BTreeSet::new();
        for peer in &self.peers {
            p.neighbor(&peer.node)?;
            if !nodes.insert(&peer.node)
                || !certificates.insert(&peer.certificate)
                || peer.certificate == self.certificate
                || peer.certificate.is_empty()
                || peer.certificate.len() > 16384
                || peer.port == 0
                || !hostname(&peer.host)
                || !hostname(&peer.server_name)
            {
                return Err(Error::Policy);
            }
        }
        Ok(())
    }
    pub fn peer(&self, node: &NodeId, inbound: bool) -> Result<&Peer, Error> {
        self.peers
            .iter()
            .find(|p| {
                &p.node == node
                    && p.enabled
                    && !p.held
                    && if inbound { p.inbound } else { p.outbound }
            })
            .ok_or(Error::Policy)
    }
}
fn hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub last_attempt: i64,
    pub last_success: Option<i64>,
    pub result: String,
    pub sent: u32,
    #[serde(default)]
    pub files_sent: u32,
    #[serde(default)]
    pub catalogs_sent: u32,
    #[serde(default)]
    pub catalogs_received: u32,
    #[serde(default)]
    pub files_received: u32,
    #[serde(default)]
    pub file_bytes: u64,
    pub accepted: u32,
    pub duplicates: u32,
    pub rejected: u32,
    pub bytes: u64,
    pub duration_ms: u64,
    pub retry: u32,
    pub protocol_minor: Option<u16>,
}
impl RuntimeDatabase {
    pub fn circuitnet_profiles(&self) -> Result<Vec<NetworkId>, Error> {
        self.connection
            .prepare("SELECT network FROM circuitnet_profiles ORDER BY network LIMIT 8")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(NetworkId::new(&r?)?))
            .collect()
    }
    pub fn circuitnet_live(&self, network: &NetworkId) -> Result<(Config, i64), Error> {
        let row: Option<(String, i64)> = self
            .connection
            .query_row(
                "SELECT configuration,version FROM circuitnet_live WHERE network=?1",
                [network.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (c, v) = match row {
            Some((j, v)) => (serde_json::from_str::<Config>(&j)?, v),
            None => (Config::default(), 0),
        };
        c.validate(&profile(&self.connection, network)?.0)?;
        Ok((c, v))
    }
    pub fn circuitnet_configure_live(
        &mut self,
        actor: &str,
        network: &NetworkId,
        config: &Config,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        config.validate(&profile(&tx, network)?.0)?;
        let old: Option<i64> = tx
            .query_row(
                "SELECT version FROM circuitnet_live WHERE network=?1",
                [network.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if old.unwrap_or(0) != expected {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO circuitnet_live VALUES(?1,?2,1) ON CONFLICT(network) DO UPDATE SET configuration=excluded.configuration,version=version+1",params![network.as_str(),serde_json::to_string(config)?])?;
        audit(&tx, network, actor, "live-config", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_link_health(
        &self,
        network: &NetworkId,
        node: &NodeId,
    ) -> Result<Option<Health>, Error> {
        self.connection
            .query_row(
                "SELECT observation FROM circuitnet_link_health WHERE network=?1 AND neighbor=?2",
                params![network.as_str(), node.as_str()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|j| Ok(serde_json::from_str(&j)?))
            .transpose()
    }
    pub fn circuitnet_record_link(
        &mut self,
        network: &NetworkId,
        node: &NodeId,
        health: &Health,
    ) -> Result<(), Error> {
        if !self
            .circuitnet_live(network)?
            .0
            .peers
            .iter()
            .any(|p| &p.node == node)
            || ![
                "ok",
                "connect",
                "tls",
                "timeout",
                "auth-failed",
                "unknown-node",
                "wrong-network",
                "topology-mismatch",
                "unsupported-version",
                "malformed-frame",
                "oversized",
                "conflicting-message",
                "unauthorized-codename",
                "custody",
                "held",
                "busy",
                "interrupted",
            ]
            .contains(&health.result.as_str())
            || health.retry > 3
        {
            return Err(Error::Policy);
        }
        self.connection.execute("INSERT INTO circuitnet_link_health VALUES(?1,?2,?3) ON CONFLICT(network,neighbor) DO UPDATE SET observation=excluded.observation",params![network.as_str(),node.as_str(),serde_json::to_string(health)?])?;
        Ok(())
    }
    pub fn circuitnet_neighbor_pending(
        &self,
        network: &NetworkId,
        node: &NodeId,
    ) -> Result<u32, Error> {
        Ok(self.connection.query_row("SELECT COUNT(*) FROM circuitnet_deliveries d JOIN network_outbound_queue q USING(queue_id) WHERE d.network=?1 AND d.neighbor=?2 AND q.state NOT IN('accepted','cancelled')",params![network.as_str(),node.as_str()],|r|r.get(0))?)
    }
}
