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

//! Durable signed network metadata and explicit local conference choices.
use super::*;
use sf_net::circuitnet::catalog::Error as CatalogError;
pub use sf_net::circuitnet::catalog::{Authority, Body, Entry, Intent, Lifecycle, Signed};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogStatus {
    pub pending_sync: bool,
    pub authority: Option<Authority>,
    pub revision: u64,
    pub hash: Option<String>,
    pub pending_mapping: u32,
    pub required_unmapped: u32,
    pub rejected: u32,
    pub last_error: Option<String>,
    pub last_success: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalEntry {
    pub entry: Entry,
    pub decision: String,
    pub conference: Option<i64>,
}
pub(super) fn authority(c: &Connection, n: &NetworkId) -> Result<Option<Authority>, Error> {
    c.query_row(
        "SELECT authority FROM circuitnet_catalog_authority WHERE network=?1",
        [n.as_str()],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| serde_json::from_str(&s).map_err(Error::from))
    .transpose()
}
fn current(c: &Connection, n: &NetworkId) -> Result<Option<Signed>, Error> {
    c.query_row("SELECT object FROM circuitnet_catalog_revisions WHERE network=?1 ORDER BY revision DESC LIMIT 1",[n.as_str()],|r|r.get::<_,String>(0)).optional()?.map(|s|Signed::decode(s.as_bytes()).map_err(Error::from)).transpose()
}
/// None is the explicitly ungoverned C2-C6 profile, never unknown under a pin.
pub(super) fn active(
    c: &Connection,
    n: &NetworkId,
    code: &Codename,
    subscription: bool,
) -> Result<Option<Entry>, Error> {
    if authority(c, n)?.is_none() {
        return Ok(None);
    }
    let s = current(c, n)?.ok_or(Error::Policy)?;
    s.body
        .entries
        .into_iter()
        .find(|e| {
            e.codename == *code
                && (e.status == Lifecycle::Active
                    || (!subscription && e.status == Lifecycle::Deprecated))
        })
        .map(Some)
        .ok_or(Error::Policy)
}
pub(super) fn message_allowed(c: &Connection, n: &NetworkId, m: &Message) -> Result<bool, Error> {
    match active(c, n, &m.codename, false) {
        Ok(Some(e)) => Ok(m.conference_identity.as_ref() == Some(&e.id)),
        Ok(None) => Ok(m.conference_identity.is_none()),
        Err(Error::Policy) => Ok(false),
        Err(e) => Err(e),
    }
}
pub(super) fn mapped(
    c: &Connection,
    n: &NetworkId,
    code: &Codename,
    conference: i64,
) -> Result<bool, Error> {
    let e = match active(c, n, code, false) {
        Ok(Some(e)) => e,
        Ok(None) => return Ok(true),
        Err(Error::Policy) => return Ok(false),
        Err(e) => return Err(e),
    };
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_catalog_choices WHERE network=?1 AND identity=?2 AND decision='mapped' AND conference_id=?3)",params![n.as_str(),e.id,conference],|r|r.get(0))?)
}
pub(super) fn bind_dossier(
    c: &Connection,
    n: &NetworkId,
    peer: &NodeId,
    code: &Codename,
) -> Result<(), Error> {
    if let Some(e) = active(c, n, code, true)? {
        c.execute(
            "INSERT OR IGNORE INTO circuitnet_catalog_dossiers VALUES(?1,?2,?3,?4)",
            params![n.as_str(), peer.as_str(), code.as_str(), e.id],
        )?;
    }
    Ok(())
}
pub(super) fn dossier_allowed(
    c: &Connection,
    n: &NetworkId,
    peer: &NodeId,
    code: &Codename,
) -> Result<bool, Error> {
    let e = match active(c, n, code, false) {
        Ok(Some(e)) => e,
        Ok(None) => return Ok(true),
        Err(Error::Policy) => return Ok(false),
        Err(e) => return Err(e),
    };
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_catalog_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3 AND identity=?4)",params![n.as_str(),peer.as_str(),code.as_str(),e.id],|r|r.get(0))?)
}
impl RuntimeDatabase {
    pub fn circuitnet_catalog_draft(
        &self,
        actor: &str,
        n: &NetworkId,
        reference: &str,
        rationale: &str,
        now: i64,
    ) -> Result<Body, Error> {
        let status = self.circuitnet_catalog_status(n)?;
        let a = status.authority.ok_or(Error::Policy)?;
        let p = profile(&self.connection, n)?.0;
        if p.local != a.publisher {
            return Err(Error::Policy);
        }
        let old = self.circuitnet_catalog_current(n)?;
        let body = Body {
            format: "circuitnet-ng-catalog".into(),
            schema: 1,
            network: n.clone(),
            catalog_id: a.catalog_id,
            revision: status.revision + 1,
            previous_revision: status.revision,
            previous_hash: status.hash,
            published_at: now,
            publisher: a.publisher,
            governance_reference: reference.into(),
            rationale: rationale.into(),
            intent: Intent::Ordinary,
            entries: old.map_or_else(Vec::new, |s| s.body.entries),
        };
        body.validate()?;
        audit(
            &self.connection,
            n,
            actor,
            &format!("catalog-draft:{}", body.revision),
            now,
        )?;
        Ok(body)
    }
    pub fn circuitnet_catalog_observe_parent(
        &mut self,
        n: &NetworkId,
        peer: &NodeId,
        revision: u64,
        hash: Option<&str>,
        now: i64,
    ) -> Result<(), Error> {
        let result = (|| {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let p = profile(&tx, n)?.0;
            if p.topology.node(&p.local)?.parent.as_ref() != Some(peer) {
                return Err(Error::Policy);
            }
            if authority(&tx, n)?.is_none() {
                return Ok(());
            }
            if revision > i64::MAX as u64
                || (revision == 0) != hash.is_none()
                || hash.is_some_and(|h| !wire::catalog::is_hex(h, 32))
            {
                return Err(CatalogError::Invalid.into());
            }
            let old = current(&tx, n)?;
            if old
                .as_ref()
                .is_some_and(|s| s.body.revision == revision && Some(s.hash.as_str()) != hash)
            {
                return Err(CatalogError::Fork.into());
            }
            let observed: i64 = tx.query_row(
                "SELECT upstream_revision FROM circuitnet_catalog_health WHERE network=?1",
                [n.as_str()],
                |r| r.get(0),
            )?;
            tx.execute(
                "UPDATE circuitnet_catalog_health SET upstream_revision=?2 WHERE network=?1",
                params![n.as_str(), revision as i64],
            )?;
            if observed != revision as i64 {
                tx.execute(
                    "UPDATE network_preparation SET generation=generation+1 WHERE singleton=1",
                    [],
                )?;
            }
            if old
                .as_ref()
                .is_some_and(|s| s.body.revision == revision && Some(s.hash.as_str()) == hash)
            {
                tx.execute("UPDATE circuitnet_catalog_health SET last_success=?2,last_error=NULL WHERE network=?1",params![n.as_str(),now])?;
            }
            tx.commit()?;
            Ok(())
        })();
        if let Err(Error::Catalog(e)) = &result {
            self.connection.execute("UPDATE circuitnet_catalog_health SET rejected=MIN(rejected+1,1000000),last_error=?2 WHERE network=?1",params![n.as_str(),e.to_string()])?;
            audit(
                &self.connection,
                n,
                peer.as_str(),
                &format!("catalog-head-rejected:{e}"),
                now,
            )?;
        }
        result
    }
    pub fn circuitnet_catalog_pending(
        &self,
        n: &NetworkId,
        node: Option<&NodeId>,
    ) -> Result<bool, Error> {
        let revision = current(&self.connection, n)?.map_or(0, |s| s.body.revision);
        let p = profile(&self.connection, n)?.0;
        if node.is_none_or(|id| {
            p.topology
                .node(&p.local)
                .is_ok_and(|v| v.parent.as_ref() == Some(id))
        }) {
            let upstream: i64 = self
                .connection
                .query_row(
                    "SELECT upstream_revision FROM circuitnet_catalog_health WHERE network=?1",
                    [n.as_str()],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or(0);
            if upstream > revision as i64 {
                return Ok(true);
            }
        }
        for child in
            p.topology.nodes.iter().filter(|v| {
                v.parent.as_ref() == Some(&p.local) && node.is_none_or(|id| id == &v.id)
            })
        {
            let observed:i64=self.connection.query_row("SELECT revision FROM circuitnet_catalog_peers WHERE network=?1 AND neighbor=?2",params![n.as_str(),child.id.as_str()],|r|r.get(0)).optional()?.unwrap_or(0);
            if observed < revision as i64 {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn circuitnet_catalog_peer(
        &mut self,
        n: &NetworkId,
        node: &NodeId,
        revision: u64,
        hash: Option<&str>,
        now: i64,
    ) -> Result<(), Error> {
        let p = profile(&self.connection, n)?.0;
        p.neighbor(node)?;
        if authority(&self.connection, n)?.is_none() {
            return Ok(());
        }
        if revision > 0
            && self
                .circuitnet_catalog_revision(n, revision)?
                .as_ref()
                .map(|s| s.hash.as_str())
                != hash
        {
            return Err(CatalogError::Fork.into());
        }
        self.connection.execute("INSERT INTO circuitnet_catalog_peers VALUES(?1,?2,?3,?4,?5) ON CONFLICT(network,neighbor) DO UPDATE SET revision=excluded.revision,hash=excluded.hash,observed_at=excluded.observed_at",params![n.as_str(),node.as_str(),i64::try_from(revision).map_err(|_|Error::Policy)?,hash,now])?;
        Ok(())
    }

    pub fn circuitnet_catalog_pin(
        &mut self,
        actor: &str,
        a: &Authority,
        now: i64,
    ) -> Result<(), Error> {
        a.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, &a.network)?;
        if p.topology.node(&a.publisher)?.role != Role::Root {
            return Err(Error::Policy);
        }
        if let Some(old) = authority(&tx, &a.network)? {
            return if old == *a {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        tx.execute(
            "INSERT INTO circuitnet_catalog_authority VALUES(?1,?2)",
            params![a.network.as_str(), serde_json::to_string(a)?],
        )?;
        tx.execute(
            "INSERT INTO circuitnet_catalog_health(network) VALUES(?1)",
            [a.network.as_str()],
        )?;
        audit(&tx, &a.network, actor, "catalog-pin", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_catalog_current(&self, n: &NetworkId) -> Result<Option<Signed>, Error> {
        current(&self.connection, n)
    }
    pub fn circuitnet_catalog_revision(
        &self,
        n: &NetworkId,
        revision: u64,
    ) -> Result<Option<Signed>, Error> {
        self.connection
            .query_row(
                "SELECT object FROM circuitnet_catalog_revisions WHERE network=?1 AND revision=?2",
                params![
                    n.as_str(),
                    i64::try_from(revision).map_err(|_| Error::Policy)?
                ],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| Signed::decode(s.as_bytes()).map_err(Error::from))
            .transpose()
    }
    pub fn circuitnet_catalog_receive(
        &mut self,
        actor: &str,
        n: &NetworkId,
        s: &Signed,
        now: i64,
    ) -> Result<bool, Error> {
        let result = (|| {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let a = authority(&tx, n)?.ok_or(Error::Policy)?;
            s.verify(&a)?;
            let old = current(&tx, n)?;
            if !s.follows(old.as_ref())? {
                return Ok(false);
            }
            capacity(&tx)?;
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM circuitnet_catalog_revisions WHERE network=?1",
                [n.as_str()],
                |r| r.get(0),
            )?;
            if count >= 4096 {
                return Err(Error::Capacity);
            }
            tx.execute(
                "INSERT INTO circuitnet_catalog_revisions VALUES(?1,?2,?3,?4,?5)",
                params![
                    n.as_str(),
                    s.body.revision as i64,
                    s.hash,
                    String::from_utf8(s.encode()?).map_err(|_| Error::Policy)?,
                    now
                ],
            )?;
            for entry in &s.body.entries {
                if entry.status == Lifecycle::Retired {
                    if !s.body.entries.iter().any(|e| {
                        e.codename == entry.codename
                            && matches!(e.status, Lifecycle::Active | Lifecycle::Deprecated)
                    }) {
                        tx.execute("UPDATE circuitnet_mappings SET send=0,receive=0,version=version+1 WHERE network=?1 AND codename=?2 AND (send=1 OR receive=1)",params![n.as_str(),entry.codename.as_str()])?;
                    }
                    tx.execute("UPDATE network_outbound_queue SET state='held',reason='catalog-retired',version=version+1 WHERE state NOT IN('accepted','cancelled','held') AND queue_id IN(SELECT d.queue_id FROM circuitnet_deliveries d JOIN circuitnet_messages m USING(network,identity) WHERE d.network=?1 AND m.conference_identity=?2)",params![n.as_str(),entry.id])?;
                }
            }
            tx.execute("UPDATE circuitnet_catalog_health SET last_success=?2,last_error=NULL WHERE network=?1",params![n.as_str(),now])?;
            audit(
                &tx,
                n,
                actor,
                &format!("catalog-verified:{}:{}", s.body.revision, s.hash),
                now,
            )?;
            tx.commit()?;
            Ok(true)
        })();
        if let Err(ref e) = result {
            let reason = match e {
                Error::Catalog(e) => e.to_string(),
                _ => "catalog-policy".into(),
            };
            self.connection.execute("UPDATE circuitnet_catalog_health SET rejected=MIN(rejected+1,1000000),last_error=?2 WHERE network=?1",params![n.as_str(),reason])?;
            audit(
                &self.connection,
                n,
                actor,
                &format!("catalog-rejected:{reason}"),
                now,
            )?;
        }
        result
    }
    /// Caller must hold local sensitive-configuration capability; key possession alone
    /// does not replace the pinned publisher or the signed human decision reference.
    pub fn circuitnet_catalog_publish(
        &mut self,
        actor: &str,
        body: Body,
        key: &[u8],
        now: i64,
    ) -> Result<Signed, Error> {
        let (p, _) = profile(&self.connection, &body.network)?;
        let a = authority(&self.connection, &body.network)?.ok_or(Error::Policy)?;
        if p.local != a.publisher
            || body.publisher != p.local
            || p.topology.node(&p.local)?.role != Role::Root
        {
            return Err(Error::Policy);
        }
        let signed = Signed::sign(body, key)?;
        self.circuitnet_catalog_receive(actor, &p.network, &signed, now)?;
        audit(
            &self.connection,
            &p.network,
            actor,
            &format!(
                "catalog-publish:{}:{:?}",
                signed.body.revision, signed.body.intent
            ),
            now,
        )?;
        Ok(signed)
    }
    pub fn circuitnet_catalog_entries(&self, n: &NetworkId) -> Result<Vec<LocalEntry>, Error> {
        let Some(s) = current(&self.connection, n)? else {
            return Ok(vec![]);
        };
        s.body.entries.into_iter().map(|entry|{
            let choice:Option<(String,Option<i64>)>=self.connection.query_row("SELECT decision,conference_id FROM circuitnet_catalog_choices WHERE network=?1 AND identity=?2",params![n.as_str(),entry.id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let (mut decision,conference)=choice.unwrap_or(("available".into(),None));
            if entry.status==Lifecycle::Retired {decision="retired".into()}
            else if entry.status==Lifecycle::Deprecated {decision="needs-attention".into()}
            Ok(LocalEntry{entry,decision,conference})
        }).collect()
    }
    pub fn circuitnet_catalog_status(&self, n: &NetworkId) -> Result<CatalogStatus, Error> {
        let authority = authority(&self.connection, n)?;
        let current = current(&self.connection, n)?;
        let entries = self.circuitnet_catalog_entries(n)?;
        let health:Option<(Option<i64>,u32,Option<String>)>=self.connection.query_row("SELECT last_success,rejected,last_error FROM circuitnet_catalog_health WHERE network=?1",[n.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (last_success, rejected, last_error) = health.unwrap_or_default();
        let upstream: i64 = self
            .connection
            .query_row(
                "SELECT upstream_revision FROM circuitnet_catalog_health WHERE network=?1",
                [n.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok(CatalogStatus {
            pending_sync: current.as_ref().map_or(0, |s| s.body.revision) < upstream as u64,
            authority,
            revision: current.as_ref().map_or(0, |s| s.body.revision),
            hash: current.map(|s| s.hash),
            pending_mapping: entries
                .iter()
                .filter(|e| e.decision == "available" || e.decision == "needs-attention")
                .count() as u32,
            required_unmapped: entries
                .iter()
                .filter(|e| {
                    e.entry.required
                        && e.entry.status != Lifecycle::Retired
                        && e.decision != "mapped"
                })
                .count() as u32,
            rejected,
            last_error,
            last_success,
        })
    }
    /// Explicit local selection, including local conference ID. No wire number exists.
    pub fn circuitnet_catalog_choose(
        &mut self,
        actor: &str,
        n: &NetworkId,
        id: &str,
        conference: Option<i64>,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let s = current(&tx, n)?.ok_or(Error::Policy)?;
        let e = s
            .body
            .entries
            .iter()
            .find(|e| e.id == id && matches!(e.status, Lifecycle::Active | Lifecycle::Deprecated))
            .ok_or(Error::Policy)?;
        if let Some(mid) = conference {
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM message_conferences WHERE conference_id=?1 AND active=1 AND public_only=1)",[mid],|r|r.get(0))?;
            if !valid {
                return Err(Error::Policy);
            }
            // Historical per-generation choices remain; only the live routing projection changes.
            tx.execute(
                "DELETE FROM circuitnet_mappings WHERE network=?1 AND codename=?2",
                params![n.as_str(), e.codename.as_str()],
            )?;
            tx.execute(
                "INSERT INTO circuitnet_mappings VALUES(?1,?2,?3,1,1,1)",
                params![n.as_str(), e.codename.as_str(), mid],
            )?;
        } else {
            tx.execute("UPDATE circuitnet_mappings SET send=0,receive=0,version=version+1 WHERE network=?1 AND codename=?2",params![n.as_str(),e.codename.as_str()])?;
        }
        tx.execute("INSERT INTO circuitnet_catalog_choices VALUES(?1,?2,?3,?4,(SELECT COALESCE(MAX(message_id),0) FROM messages)) ON CONFLICT(network,identity) DO UPDATE SET decision=excluded.decision,conference_id=excluded.conference_id,after_message=excluded.after_message",params![n.as_str(),id,if conference.is_some(){"mapped"}else{"ignored"},conference])?;
        audit(
            &tx,
            n,
            actor,
            &format!(
                "catalog-choice:{id}:{}",
                if conference.is_some() {
                    "mapped"
                } else {
                    "ignored"
                }
            ),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Preflight before native creation, then explicit mapping. A failed map leaves
    /// an ordinary local conference for operator recovery, never an auto-subscription.
    pub fn circuitnet_catalog_create_map(
        &mut self,
        actor: &str,
        n: &NetworkId,
        id: &str,
        definition: &crate::ConferenceDefinition,
        now: i64,
    ) -> Result<i64, Error> {
        if !self
            .circuitnet_catalog_entries(n)?
            .iter()
            .any(|e| e.entry.id == id && e.entry.status == Lifecycle::Active)
            || !definition.public_only
        {
            return Err(Error::Policy);
        }
        let c = self.create_conference(definition)?;
        self.circuitnet_catalog_choose(actor, n, id, Some(c.id.get()), now)?;
        Ok(c.id.get())
    }
    pub fn validate_catalog_authority(&self) -> Result<(), Error> {
        let networks: Vec<String> = self
            .connection
            .prepare("SELECT network FROM circuitnet_catalog_authority")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for n in networks {
            let n = NetworkId::new(&n)?;
            let a = authority(&self.connection, &n)?.ok_or(Error::Policy)?;
            a.validate()?;
            let p = profile(&self.connection, &n)?.0;
            if a.network != n || p.topology.node(&a.publisher)?.role != Role::Root {
                return Err(Error::Policy);
            }
            let mut previous = None;
            let objects:Vec<(i64,String,String)>=self.connection.prepare("SELECT revision,hash,object FROM circuitnet_catalog_revisions WHERE network=?1 ORDER BY revision")?.query_map([n.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<Result<_,_>>()?;
            for (revision, hash, object) in objects {
                let s = Signed::decode(object.as_bytes())?;
                s.verify(&a)?;
                s.follows(previous.as_ref())?;
                if s.body.revision != revision as u64 || s.hash != hash {
                    return Err(Error::Conflict);
                }
                previous = Some(s);
            }
            let choices:Vec<String>=self.connection.prepare("SELECT identity FROM circuitnet_catalog_choices WHERE network=?1 UNION SELECT identity FROM circuitnet_catalog_dossiers WHERE network=?1")?.query_map([n.as_str()],|r|r.get(0))?.collect::<Result<_,_>>()?;
            for id in choices {
                if !previous
                    .as_ref()
                    .is_some_and(|s| s.body.entries.iter().any(|e| e.id == id))
                {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }
    /// Reject restoring behind knowledge retained by the stopped destination board.
    pub fn circuitnet_catalog_check_restore(&self, older: &Self) -> Result<(), Error> {
        for n in self.circuitnet_profiles()? {
            let Some(a) = authority(&self.connection, &n)? else {
                continue;
            };
            if authority(&older.connection, &n)?.as_ref() != Some(&a) {
                return Err(CatalogError::Publisher.into());
            }
            if let Some(s) = current(&self.connection, &n)? {
                let restored = current(&older.connection, &n)?.ok_or(CatalogError::Rollback)?;
                if restored.body.revision < s.body.revision {
                    return Err(CatalogError::Rollback.into());
                }
                let same = older
                    .circuitnet_catalog_revision(&n, s.body.revision)?
                    .ok_or(CatalogError::Missing)?;
                if same.hash != s.hash {
                    return Err(CatalogError::Fork.into());
                }
            }
        }
        Ok(())
    }
}
