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

//! Durable direct-child subscription requests and deterministic directed intent.
use super::*;
pub use sf_net::circuitnet::control::{Operation, Outcome, Request, SubscriptionResult};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    #[default]
    RequireApproval,
    AutoApprove,
    Deny,
}
impl Policy {
    fn name(self) -> &'static str {
        match self {
            Self::RequireApproval => "require-approval",
            Self::AutoApprove => "auto-approve",
            Self::Deny => "deny",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub request: Request,
    pub result: SubscriptionResult,
    pub direction: String,
    pub authenticated_source: NodeId,
    pub settled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Route {
    pub network: NetworkId,
    pub local: NodeId,
    pub destination: NodeId,
    pub next_hop: Option<NodeId>,
    pub path: Vec<NodeId>,
}
fn policy(conn: &Connection, network: &NetworkId) -> Result<(Policy, i64), Error> {
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT policy,version FROM circuitnet_control_policy WHERE network=?1",
            [network.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        None => Ok((Policy::RequireApproval, 0)),
        Some((s, v)) => Ok((serde_json::from_str(&format!("\"{s}\""))?, v)),
    }
}
fn authorized(p: &Profile, peer: &NodeId, r: &Request) -> bool {
    p.enabled
        && r.network == p.network
        && r.target == p.local
        && r.requester == *peer
        && r.id.origin() == *peer
        && p.topology
            .node(peer)
            .is_ok_and(|n| n.parent.as_ref() == Some(&p.local))
        && p.topology
            .node(&p.local)
            .is_ok_and(|n| n.role.can_transit())
}
fn available(conn: &Connection, p: &Profile, c: &Codename) -> Result<bool, Error> {
    let Some(m) = area(conn, p, c)? else {
        return Ok(false);
    };
    Ok(m.send
        && m.receive
        && conn.query_row(
            "SELECT active=1 AND public_only=1 FROM message_conferences WHERE conference_id=?1",
            [m.conference],
            |r| r.get(0),
        )?)
}
fn apply(conn: &Connection, p: &Profile, r: &Request, now: i64) -> Result<Outcome, Error> {
    let c = r.codename.as_ref().ok_or(Error::Policy)?;
    if !available(conn, p, c)? {
        return Ok(Outcome::UnknownCodename);
    }
    let wanted = r.operation == Operation::Subscribe;
    if subscribed(conn, p, &r.requester, c)? == wanted {
        return Ok(if wanted {
            Outcome::AlreadySubscribed
        } else {
            Outcome::AlreadyUnsubscribed
        });
    }
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM circuitnet_dossiers WHERE network=?1",
        [p.network.as_str()],
        |r| r.get(0),
    )?;
    let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3)",params![p.network.as_str(),r.requester.as_str(),c.as_str()],|r|r.get(0))?;
    if count >= 4096 && !exists {
        return Err(Error::Capacity);
    }
    conn.execute("INSERT INTO circuitnet_dossiers VALUES(?1,?2,?3,?4,1) ON CONFLICT(network,neighbor,codename) DO UPDATE SET subscribed=excluded.subscribed,version=version+1",params![p.network.as_str(),r.requester.as_str(),c.as_str(),wanted])?;
    if !wanted {
        conn.execute("UPDATE network_outbound_queue SET state='held',reason='circuitnet-unsubscribed',version=version+1 WHERE state NOT IN('accepted','cancelled') AND queue_id IN(SELECT d.queue_id FROM circuitnet_deliveries d JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND d.neighbor=?2 AND m.codename=?3 AND m.destination IS NULL)",params![p.network.as_str(),r.requester.as_str(),c.as_str()])?;
    }
    audit(
        conn,
        &p.network,
        r.requester.as_str(),
        &format!(
            "control-mutation:{}:{}:{}",
            r.id,
            if wanted { "subscribe" } else { "unsubscribe" },
            c
        ),
        now,
    )?;
    Ok(Outcome::Applied)
}
fn record(
    conn: &Connection,
    r: &Request,
    result: &SubscriptionResult,
    peer: &NodeId,
    direction: &str,
    now: i64,
) -> Result<(), Error> {
    capacity(conn)?;
    conn.execute(
        "INSERT INTO circuitnet_controls VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
        params![
            r.network.as_str(),
            r.id.as_str(),
            direction,
            peer.as_str(),
            serde_json::to_string(r)?,
            r.fingerprint()?,
            serde_json::to_string(result)?,
            result.outcome.terminal(),
            now
        ],
    )?;
    Ok(())
}
impl RuntimeDatabase {
    pub fn circuitnet_control_counts(&self, network: &NetworkId) -> Result<(u32, u32, u32), Error> {
        Ok(self.connection.query_row("SELECT (SELECT COUNT(*) FROM circuitnet_controls WHERE network=?1 AND direction='incoming' AND settled=0), (SELECT COUNT(*) FROM circuitnet_deliveries d JOIN circuitnet_messages m USING(network,identity) JOIN network_outbound_queue q USING(queue_id) WHERE d.network=?1 AND m.destination IS NOT NULL AND q.state NOT IN('accepted','cancelled')), (SELECT COUNT(*) FROM circuitnet_changes WHERE network=?1 AND (operation LIKE 'unknown-destination:%' OR operation='directed-rejected'))",[network.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)
    }

    pub fn circuitnet_route(
        &self,
        network: &NetworkId,
        destination: &NodeId,
    ) -> Result<Route, Error> {
        let (p, _) = profile(&self.connection, network)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        let path = p.topology.path(&p.local, destination)?;
        Ok(Route {
            network: network.clone(),
            local: p.local,
            destination: destination.clone(),
            next_hop: path.get(1).cloned(),
            path,
        })
    }
    /// Operator-only selection before scanner publication. Never changes a published identity.
    pub fn circuitnet_direct(
        &mut self,
        network: &NetworkId,
        mid: i64,
        destination: &NodeId,
        now: i64,
    ) -> Result<(), Error> {
        let route_valid = self.circuitnet_route(network, destination).is_ok();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN message_conferences c USING(conference_id) JOIN circuitnet_mappings a USING(conference_id) WHERE m.message_id=?1 AND a.network=?2 AND a.send=1 AND c.active=1 AND c.public_only=1 AND m.origin_kind='native' AND m.visibility='public' AND m.audience_kind='all-callers' AND m.lifecycle_state='active' AND m.identity_proof IS NOT NULL AND NOT EXISTS(SELECT 1 FROM circuitnet_messages WHERE network=?2 AND message_id=?1))",params![mid,network.as_str()],|r|r.get(0))?;
        if !valid {
            return Err(Error::Policy);
        }
        tx.execute(
            "INSERT INTO circuitnet_destinations VALUES(?1,?2,?3,?4)",
            params![network.as_str(), mid, destination.as_str(), route_valid],
        )?;
        if !route_valid {
            audit(
                &tx,
                network,
                "operator",
                &format!("unknown-destination:{destination}"),
                now,
            )?;
            tx.commit()?;
            return Err(Error::Policy);
        }
        audit(
            &tx,
            network,
            "operator",
            &format!("directed-selected:{destination}"),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_control_policy(&self, network: &NetworkId) -> Result<(Policy, i64), Error> {
        profile(&self.connection, network)?;
        policy(&self.connection, network)
    }
    pub fn circuitnet_set_control_policy(
        &mut self,
        actor: &str,
        network: &NetworkId,
        value: Policy,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        profile(&tx, network)?;
        capacity(&tx)?;
        if policy(&tx, network)?.1 != expected {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO circuitnet_control_policy VALUES(?1,?2,1) ON CONFLICT(network) DO UPDATE SET policy=excluded.policy,version=version+1",params![network.as_str(),value.name()])?;
        audit(&tx, network, actor, value.name(), now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_request(
        &mut self,
        network: &NetworkId,
        operation: Operation,
        codename: Option<Codename>,
        now: i64,
    ) -> Result<Request, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        let target = p
            .topology
            .node(&p.local)?
            .parent
            .clone()
            .ok_or(Error::Policy)?;
        let r = Request {
            network: network.clone(),
            id: MessageId::new(&format!("{}:{:032x}", p.local, rand::random::<u128>()))?,
            requester: p.local,
            target: target.clone(),
            operation,
            codename,
        };
        r.validate()?;
        let result = SubscriptionResult::for_request(&r, Outcome::Accepted)?;
        record(&tx, &r, &result, &target, "outgoing", now)?;
        audit(
            &tx,
            network,
            "operator",
            &format!("control-created:{}", r.id),
            now,
        )?;
        tx.commit()?;
        Ok(r)
    }
    /// Host supplies a TLS-authenticated direct peer, never a caller or envelope claim.
    pub fn circuitnet_receive_control(
        &mut self,
        network: &NetworkId,
        peer: &NodeId,
        r: &Request,
        now: i64,
    ) -> Result<SubscriptionResult, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(peer)?;
        capacity(&tx)?;
        if !authorized(&p, peer, r) || r.validate().is_err() {
            audit(&tx, network, peer.as_str(), "control-unauthorized", now)?;
            let result = SubscriptionResult::for_request(r, Outcome::Unauthorized)?;
            tx.commit()?;
            return Ok(result);
        }
        let old:Option<(String,String)>=tx.query_row("SELECT fingerprint,result FROM circuitnet_controls WHERE network=?1 AND identity=?2 AND direction='incoming'",params![network.as_str(),r.id.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((hash, json)) = old {
            let result = if hash == r.fingerprint()? {
                serde_json::from_str(&json)?
            } else {
                audit(&tx, network, peer.as_str(), "control-replay-conflict", now)?;
                SubscriptionResult::for_request(r, Outcome::ReplayConflict)?
            };
            tx.commit()?;
            return Ok(result);
        }
        let outcome = if r.operation == Operation::QuerySubscriptions {
            Outcome::Applied
        } else if !available(&tx, &p, r.codename.as_ref().ok_or(Error::Policy)?)? {
            Outcome::UnknownCodename
        } else {
            match policy(&tx, network)?.0 {
                Policy::Deny => Outcome::Denied,
                Policy::RequireApproval => Outcome::PendingApproval,
                Policy::AutoApprove => apply(&tx, &p, r, now)?,
            }
        };
        let mut result = SubscriptionResult::for_request(r, outcome)?;
        if r.operation == Operation::QuerySubscriptions {
            let codes=tx.prepare("SELECT codename FROM circuitnet_dossiers WHERE network=?1 AND neighbor=?2 AND subscribed=1 ORDER BY codename LIMIT 4096")?.query_map(params![network.as_str(),peer.as_str()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            result.subscriptions = codes
                .into_iter()
                .map(|s| Codename::new(&s))
                .collect::<Result<_, _>>()?;
        }
        record(&tx, r, &result, peer, "incoming", now)?;
        audit(
            &tx,
            network,
            peer.as_str(),
            &format!("control-request:{}:{outcome:?}", r.id),
            now,
        )?;
        tx.commit()?;
        Ok(result)
    }
    pub fn circuitnet_decide_control(
        &mut self,
        actor: &str,
        network: &NetworkId,
        id: &MessageId,
        approve: bool,
        now: i64,
    ) -> Result<SubscriptionResult, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx)?;
        let (p, _) = profile(&tx, network)?;
        let(req,json,peer):(String,String,String)=tx.query_row("SELECT request,result,neighbor FROM circuitnet_controls WHERE network=?1 AND identity=?2 AND direction='incoming'",params![network.as_str(),id.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let r: Request = serde_json::from_str(&req)?;
        let mut result: SubscriptionResult = serde_json::from_str(&json)?;
        if result.outcome != Outcome::PendingApproval {
            return Err(Error::Conflict);
        }
        result.outcome = if !approve || policy(&tx, network)?.0 == Policy::Deny {
            Outcome::Denied
        } else if !authorized(&p, &NodeId::new(&peer)?, &r) {
            Outcome::Unauthorized
        } else {
            apply(&tx, &p, &r, now)?
        };
        tx.execute("UPDATE circuitnet_controls SET result=?3,settled=1,updated_at=?4 WHERE network=?1 AND identity=?2 AND direction='incoming'",params![network.as_str(),id.as_str(),serde_json::to_string(&result)?,now])?;
        audit(
            &tx,
            network,
            actor,
            &format!("control-decision:{id}:{:?}", result.outcome),
            now,
        )?;
        tx.commit()?;
        Ok(result)
    }
    pub fn circuitnet_controls(
        &self,
        network: &NetworkId,
        after: &str,
    ) -> Result<Vec<Entry>, Error> {
        self.connection.prepare("SELECT request,result,direction,neighbor,settled,created_at,updated_at FROM circuitnet_controls WHERE network=?1 AND identity>?2 ORDER BY identity,direction LIMIT 100")?.query_map(params![network.as_str(),after],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(r,s,d,n,settled,created_at,updated_at)|Ok(Entry {request:serde_json::from_str(&r)?,result:serde_json::from_str(&s)?,direction:d,authenticated_source:NodeId::new(&n)?,settled,created_at,updated_at})).collect()
    }
    pub fn circuitnet_controls_page(
        &self,
        network: &NetworkId,
        offset: u32,
    ) -> Result<Vec<Entry>, Error> {
        self.connection.prepare("SELECT request,result,direction,neighbor,settled,created_at,updated_at FROM circuitnet_controls WHERE network=?1 ORDER BY settled,updated_at DESC,identity,direction LIMIT 17 OFFSET ?2")?.query_map(params![network.as_str(),offset],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(r,s,d,n,settled,created_at,updated_at)|Ok(Entry {request:serde_json::from_str(&r)?,result:serde_json::from_str(&s)?,direction:d,authenticated_source:NodeId::new(&n)?,settled,created_at,updated_at})).collect()
    }
    pub fn circuitnet_pending_controls(
        &self,
        network: &NetworkId,
        peer: &NodeId,
    ) -> Result<Vec<Request>, Error> {
        let (p, _) = profile(&self.connection, network)?;
        p.neighbor(peer)?;
        self.connection.prepare("SELECT request FROM circuitnet_controls WHERE network=?1 AND neighbor=?2 AND direction='outgoing' AND settled=0 ORDER BY updated_at,identity LIMIT 16")?.query_map(params![network.as_str(),peer.as_str()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|s|Ok(serde_json::from_str(&s)?)).collect()
    }
    pub fn circuitnet_control_result(
        &mut self,
        network: &NetworkId,
        peer: &NodeId,
        result: &SubscriptionResult,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(peer)?;
        let(json,old):(String,String)=tx.query_row("SELECT request,result FROM circuitnet_controls WHERE network=?1 AND identity=?2 AND direction='outgoing' AND neighbor=?3",params![network.as_str(),result.id.as_str(),peer.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let r: Request = serde_json::from_str(&json)?;
        let previous: SubscriptionResult = serde_json::from_str(&old)?;
        if result.network != *network
            || result.requester != p.local
            || result.target != *peer
            || result.fingerprint != r.fingerprint()?
            || result.subscriptions.len() > 4096
            || (r.operation != Operation::QuerySubscriptions && !result.subscriptions.is_empty())
            || (previous.outcome.terminal() && previous != *result)
        {
            return Err(Error::Conflict);
        }
        tx.execute("UPDATE circuitnet_controls SET result=?3,settled=?4,updated_at=?5 WHERE network=?1 AND identity=?2 AND direction='outgoing'",params![network.as_str(),r.id.as_str(),serde_json::to_string(result)?,result.outcome.terminal(),now])?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_retry_control(
        &mut self,
        network: &NetworkId,
        id: &MessageId,
    ) -> Result<(), Error> {
        if self.connection.execute("UPDATE circuitnet_controls SET settled=0 WHERE network=?1 AND identity=?2 AND direction='outgoing'",params![network.as_str(),id.as_str()])?!=1 {return Err(Error::Policy)}
        Ok(())
    }
}

/// Called inside native posting authority, before the publication wakeup commits.
pub(crate) fn select_at_post(
    tx: &rusqlite::Transaction<'_>,
    network: &NetworkId,
    mid: i64,
    destination: &NodeId,
    now: i64,
) -> Result<(), Error> {
    let (p, _) = profile(tx, network)?;
    if !p.enabled {
        return Err(Error::Policy);
    }
    p.topology.path(&p.local, destination)?;
    let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN message_conferences c USING(conference_id) JOIN circuitnet_mappings a USING(conference_id) WHERE m.message_id=?1 AND a.network=?2 AND a.send=1 AND c.active=1 AND c.public_only=1 AND m.origin_kind='native' AND m.visibility='public' AND m.audience_kind='all-callers' AND m.identity_proof IS NOT NULL)",params![mid,network.as_str()],|r|r.get(0))?;
    if !valid {
        return Err(Error::Policy);
    }
    tx.execute(
        "INSERT INTO circuitnet_destinations VALUES(?1,?2,?3,1)",
        params![network.as_str(), mid, destination.as_str()],
    )?;
    audit(
        tx,
        network,
        "native-post",
        &format!("directed-selected:{destination}"),
        now,
    )?;
    Ok(())
}
