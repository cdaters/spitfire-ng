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

//! Downstream and subscription authority, shared by operators and AreaFix.
use super::*;
/// Runtime-owned write-only credential check; never serialized or logged.
pub type AreaFixVerifier<'a> = dyn Fn(&str, &str) -> bool + 'a;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Downstream {
    pub link: String,
    pub enabled: bool,
    pub held: bool,
    pub boss_aka: Option<String>,
    pub areafix: bool,
    pub rescan: bool,
    pub max_area: u32,
    pub max_total: u32,
    pub cooldown: u32,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub link: String,
    pub domain: Domain,
    pub area: String,
    pub subscribed: bool,
    pub source: SubscriptionSource,
    pub version: i64,
    pub changed_at: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionSource {
    Manual,
    Areafix,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaAccess {
    pub domain: Domain,
    pub area: String,
    pub remote_subscribe: bool,
    pub rescan: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RescanArea {
    pub area: String,
    pub count: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AreaFixActivity {
    pub link: String,
    pub authenticated: bool,
    pub commands: u32,
    pub changes: u32,
    pub rescans: u32,
    pub result: String,
    pub received_at: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RescanActivity {
    pub request: String,
    pub link: String,
    pub area: String,
    pub requested: u32,
    pub queued: u32,
    pub accepted: u32,
    pub created_at: i64,
}
pub(super) fn downstream(
    conn: &rusqlite::Connection,
    link: &str,
) -> Result<Option<Downstream>, Error> {
    Ok(conn.query_row("SELECT enabled,boss_aka,areafix,rescan,max_area,max_total,cooldown,version,held FROM ftn_downstreams WHERE link_id=?1",[link],|r|Ok(Downstream{link:link.into(),enabled:r.get(0)?,boss_aka:r.get(1)?,areafix:r.get(2)?,rescan:r.get(3)?,max_area:r.get(4)?,max_total:r.get(5)?,cooldown:r.get(6)?,version:r.get(7)?,held:r.get(8)?})).optional()?)
}
pub(super) fn downstream_valid(policy: &Policy, d: &Downstream) -> Result<(), Error> {
    let l = policy.link(&d.link)?;
    if d.max_area == 0
        || d.max_area > 500
        || d.max_total < d.max_area
        || d.max_total > 1000
        || !(60..=86400).contains(&d.cooldown)
    {
        return Err(Error::Policy);
    }
    if let Some(boss) = &d.boss_aka {
        let a = policy.aka(boss)?;
        if boss != &l.aka
            || a.endpoint.address.point() != 0
            || l.remote.address.point() == 0
            || a.endpoint.domain != l.remote.domain
            || a.endpoint.address != l.remote.address.boss()
        {
            return Err(Error::Policy);
        }
    } else if l.remote.address.point() != 0 {
        return Err(Error::Policy);
    }
    Ok(())
}
pub(super) fn subscribed(
    conn: &rusqlite::Connection,
    link: &str,
    domain: &Domain,
    area: &str,
) -> Result<bool, Error> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM ftn_subscriptions s JOIN ftn_downstreams d USING(link_id) WHERE s.link_id=?1 AND s.domain=?2 AND s.area=?3 AND s.subscribed=1 AND d.enabled=1)",params![link,domain.as_str(),area],|r|r.get(0))?)
}
pub(super) fn echo_links(conn: &rusqlite::Connection, m: &Mapping) -> Result<Vec<String>, Error> {
    let mut links = m.links.clone();
    links.extend(conn.prepare("SELECT s.link_id FROM ftn_subscriptions s JOIN ftn_downstreams d USING(link_id) WHERE s.domain=?1 AND s.area=?2 AND s.subscribed=1 AND d.enabled=1 ORDER BY s.link_id")?.query_map(params![m.domain.as_str(),m.area],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?);
    links.sort();
    links.dedup();
    Ok(links)
}
pub(super) fn access(
    conn: &rusqlite::Connection,
    domain: &Domain,
    area: &str,
) -> Result<AreaAccess, Error> {
    Ok(conn.query_row("SELECT remote_subscribe,rescan,version FROM ftn_area_access WHERE domain=?1 AND area=?2",params![domain.as_str(),area],|r|Ok(AreaAccess{domain:domain.clone(),area:area.into(),remote_subscribe:r.get(0)?,rescan:r.get(1)?,version:r.get(2)?})).optional()?.unwrap_or(AreaAccess{domain:domain.clone(),area:area.into(),remote_subscribe:false,rescan:false,version:0}))
}
fn no_claims(tx: &Transaction<'_>, link: &str) -> Result<(), Error> {
    // Once offered, peer acceptance may already be in flight. Do not race it.
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM binkp_link_health WHERE link_id=?1 AND session_id IS NOT NULL)",[link],|r|r.get(0))?;
    if active {
        return Err(Error::Conflict);
    }
    Ok(())
}
pub(super) fn set_subscription(
    tx: &Transaction<'_>,
    link: &str,
    domain: &Domain,
    area: &str,
    state: bool,
    source: SubscriptionSource,
    now: i64,
) -> Result<bool, Error> {
    let prior: Option<bool> = tx
        .query_row(
            "SELECT subscribed FROM ftn_subscriptions WHERE link_id=?1 AND domain=?2 AND area=?3",
            params![link, domain.as_str(), area],
            |r| r.get(0),
        )
        .optional()?;
    if prior == Some(state) {
        return Ok(false);
    }
    tx.execute("INSERT INTO ftn_subscriptions VALUES(?1,?2,?3,?4,?5,1,?6) ON CONFLICT(link_id,domain,area) DO UPDATE SET subscribed=excluded.subscribed,source=excluded.source,version=version+1,changed_at=excluded.changed_at",params![link,domain.as_str(),area,state,if source==SubscriptionSource::Manual{"manual"}else{"areafix"},now])?;
    if !state {
        tx.execute("UPDATE network_outbound_queue SET state='held',reason='ftn-unsubscribed',version=version+1 WHERE state IN('pending','ready','retry') AND queue_id IN(SELECT d.queue_id FROM ftn_routing_decisions d JOIN ftn_messages f USING(publication_id) WHERE d.link_id=?1 AND f.domain=?2 AND f.area=?3) AND queue_id NOT IN(SELECT queue_id FROM binkp_queue_claims)",params![link,domain.as_str(),area])?;
    }
    Ok(true)
}
impl RuntimeDatabase {
    pub fn configure_ftn_downstream(
        &mut self,
        policy: &Policy,
        principal: &str,
        d: &Downstream,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        downstream_valid(policy, d)?;
        if expected < 0 || expected.checked_add(1) != Some(d.version) {
            return Err(Error::Policy);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        no_claims(&tx, &d.link)?;
        let old = downstream(&tx, &d.link)?;
        if old.as_ref().map_or(0, |d| d.version) != expected {
            return Err(Error::Conflict);
        }
        if old.as_ref().is_some_and(|old| old.boss_aka != d.boss_aka) {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO ftn_downstreams VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(link_id) DO UPDATE SET enabled=excluded.enabled,areafix=excluded.areafix,rescan=excluded.rescan,max_area=excluded.max_area,max_total=excluded.max_total,cooldown=excluded.cooldown,version=excluded.version,held=excluded.held",params![d.link,d.enabled,d.boss_aka,d.areafix,d.rescan,d.max_area,d.max_total,d.cooldown,d.version,d.held])?;
        if old.as_ref().is_some_and(|old| old.held && !d.held) {
            tx.execute("UPDATE binkp_link_health SET held=0,next_attempt=NULL,failures=0 WHERE link_id=?1 AND session_id IS NULL",[&d.link])?;
        }
        // Transfer existing peer membership exactly once, with no competing authority.
        tx.execute("INSERT INTO ftn_subscriptions SELECT link_id,domain,area,1,'manual',1,?2 FROM ftn_area_links WHERE link_id=?1",params![d.link,now])?;
        tx.execute("DELETE FROM ftn_area_links WHERE link_id=?1", [&d.link])?;
        if !d.enabled {
            tx.execute("UPDATE network_outbound_queue SET state='held',reason='ftn-downstream-disabled',version=version+1 WHERE state IN('pending','ready','retry') AND queue_id IN(SELECT queue_id FROM ftn_routing_decisions WHERE link_id=?1)",[&d.link])?;
        }
        audit(&tx, principal, "ftn.downstream-changed", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn configure_ftn_subscription(
        &mut self,
        policy: &Policy,
        principal: &str,
        s: &Subscription,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        if expected < 0
            || expected.checked_add(1) != Some(s.version)
            || s.source != SubscriptionSource::Manual
        {
            return Err(Error::Policy);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        no_claims(&tx, &s.link)?;
        let d = downstream(&tx, &s.link)?.ok_or(Error::Denied)?;
        downstream_valid(policy, &d)?;
        let m = mail::mapping(&tx, &s.domain, &s.area)?;
        let l = policy.link(&s.link)?;
        if l.remote.domain != m.domain
            || l.remote.address.zone() != policy.aka(&m.aka)?.endpoint.address.zone()
        {
            return Err(Error::Policy);
        }
        let prior: i64 = tx
            .query_row(
                "SELECT version FROM ftn_subscriptions WHERE link_id=?1 AND domain=?2 AND area=?3",
                params![s.link, s.domain.as_str(), s.area],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if prior != expected {
            return Err(Error::Conflict);
        }
        set_subscription(
            &tx,
            &s.link,
            &s.domain,
            &s.area,
            s.subscribed,
            SubscriptionSource::Manual,
            now,
        )?;
        audit(&tx, principal, "ftn.subscription-changed", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn configure_ftn_area_access(
        &mut self,
        policy: &Policy,
        principal: &str,
        a: &AreaAccess,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        if expected < 0 || expected.checked_add(1) != Some(a.version) {
            return Err(Error::Policy);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        mail::mapping(&tx, &a.domain, &a.area)?;
        if access(&tx, &a.domain, &a.area)?.version != expected {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO ftn_area_access VALUES(?1,?2,?3,?4,?5) ON CONFLICT(domain,area) DO UPDATE SET remote_subscribe=excluded.remote_subscribe,rescan=excluded.rescan,version=excluded.version",params![a.domain.as_str(),a.area,a.remote_subscribe,a.rescan,a.version])?;
        audit(&tx, principal, "ftn.area-access-changed", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn ftn_downstreams(&self) -> Result<Vec<Downstream>, Error> {
        let ids = self
            .connection
            .prepare("SELECT link_id FROM ftn_downstreams ORDER BY link_id LIMIT 33")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| downstream(&self.connection, &id)?.ok_or(Error::Policy))
            .collect()
    }
    pub fn ftn_subscriptions(&self, offset: u32) -> Result<Vec<Subscription>, Error> {
        self.connection.prepare("SELECT link_id,domain,area,subscribed,source,version,changed_at FROM ftn_subscriptions ORDER BY link_id,domain,area LIMIT 100 OFFSET ?1")?.query_map([offset],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get(3)?,r.get::<_,String>(4)?,r.get(5)?,r.get(6)?)))?.collect::<Result<Vec<_>,_>>()?.into_iter().map(|(link,domain,area,subscribed,source,version,changed_at)|Ok(Subscription{link,domain:domain.parse()?,area,subscribed,source:if source=="manual"{SubscriptionSource::Manual}else{SubscriptionSource::Areafix},version,changed_at})).collect::<Result<_,Error>>()
    }
    pub fn ftn_area_access(&self, domain: &Domain, area: &str) -> Result<AreaAccess, Error> {
        access(&self.connection, domain, area)
    }
    pub fn ftn_areafix_activity(&self, offset: u32) -> Result<Vec<AreaFixActivity>, Error> {
        Ok(self.connection.prepare("SELECT link_id,authenticated,commands,changes,rescans,result,received_at FROM ftn_areafix_requests ORDER BY received_at DESC,request_id LIMIT 100 OFFSET ?1")?.query_map([offset],|r|Ok(AreaFixActivity{link:r.get(0)?,authenticated:r.get(1)?,commands:r.get(2)?,changes:r.get(3)?,rescans:r.get(4)?,result:r.get(5)?,received_at:r.get(6)?}))?.collect::<Result<_,_>>()?)
    }
    pub fn ftn_rescan_activity(&self, offset: u32) -> Result<Vec<RescanActivity>, Error> {
        Ok(self.connection.prepare("SELECT r.request_id,r.link_id,a.area,a.requested,a.queued,r.created_at,(SELECT COUNT(*) FROM ftn_routing_decisions d JOIN network_outbound_queue q USING(queue_id) JOIN ftn_messages m USING(publication_id) WHERE d.delivery_key=r.request_id AND m.domain=a.domain AND m.area=a.area AND q.state='accepted') FROM ftn_rescans r JOIN ftn_rescan_areas a USING(request_id) ORDER BY r.created_at DESC,r.request_id,a.area LIMIT 100 OFFSET ?1")?.query_map([offset],|r|Ok(RescanActivity{request:r.get(0)?,link:r.get(1)?,area:r.get(2)?,requested:r.get(3)?,queued:r.get(4)?,created_at:r.get(5)?,accepted:r.get(6)?}))?.collect::<Result<_,_>>()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AreaFixCommand {
    Help,
    List,
    Query,
    Subscribe(String),
    Unsubscribe(String),
    Rescan(RescanArea),
}
/// Parses the complete bounded request. No unrecognized line can partially apply.
pub fn parse_areafix(body: &str, default_count: u32) -> Result<Vec<AreaFixCommand>, Error> {
    if body.len() > 4096
        || body
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n'))
    {
        return Err(Error::Rejected);
    }
    let mut commands = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "---" || line.starts_with("--- ") {
            break;
        }
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        let head = fields[0].to_ascii_uppercase();
        let command = match head.as_str() {
            "%HELP" if fields.len() == 1 => AreaFixCommand::Help,
            "%LIST" if fields.len() == 1 => AreaFixCommand::List,
            "%QUERY" if fields.len() == 1 => AreaFixCommand::Query,
            "%RESCAN" if (2..=3).contains(&fields.len()) => {
                let area = fields[1].to_ascii_uppercase();
                if !wire::valid_area(&area) {
                    return Err(Error::Rejected);
                }
                let count = if fields.len() == 3 {
                    fields[2]
                        .to_ascii_uppercase()
                        .strip_prefix("R=")
                        .unwrap_or(fields[2])
                        .parse::<u32>()
                        .map_err(|_| Error::Rejected)?
                } else {
                    default_count
                };
                if count == 0 {
                    return Err(Error::Rejected);
                }
                AreaFixCommand::Rescan(RescanArea { area, count })
            }
            _ if fields.len() == 1 && (head.starts_with('+') || head.starts_with('-')) => {
                let area = &head[1..];
                if !wire::valid_area(area) {
                    return Err(Error::Rejected);
                }
                if head.starts_with('+') {
                    AreaFixCommand::Subscribe(area.into())
                } else {
                    AreaFixCommand::Unsubscribe(area.into())
                }
            }
            _ => return Err(Error::Rejected),
        };
        commands.push(command);
        if commands.len() > 32 {
            return Err(Error::Capacity);
        }
    }
    if commands.is_empty() {
        return Err(Error::Rejected);
    }
    Ok(commands)
}
