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

//! Durable explicit local signing-authority replacement.
use super::*;
use sf_net::circuitnet::catalog::keys::Transition;
use std::collections::BTreeSet;
fn exists(c: &Connection) -> Result<bool, Error> {
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='circuitnet_catalog_keys')",[],|r|r.get(0))?)
}
pub(super) fn authority_at(
    c: &Connection,
    n: &NetworkId,
    revision: u64,
) -> Result<Option<Authority>, Error> {
    if !exists(c)? {
        return authority(c, n);
    }
    c.query_row("SELECT authority FROM circuitnet_catalog_keys WHERE network=?1 AND first_revision<=?2 ORDER BY first_revision DESC,epoch DESC LIMIT 1",params![n.as_str(),i64::try_from(revision).map_err(|_|Error::Policy)?],|r|r.get::<_,String>(0)).optional()?.map(|s|serde_json::from_str(&s).map_err(Error::from)).transpose()
}
pub(super) fn validate(c: &Connection, n: &NetworkId, latest: &Authority) -> Result<(), Error> {
    if !exists(c)? {
        return Ok(());
    }
    let rows:Vec<(i64,i64,String,Option<String>)>=c.prepare("SELECT epoch,first_revision,authority,transition_object FROM circuitnet_catalog_keys WHERE network=?1 ORDER BY epoch")?.query_map([n.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?.collect::<Result<_,_>>()?;
    if rows.len() > 64 {
        return Err(Error::Capacity);
    }
    let mut previous_first = 1;
    let mut previous = None;
    let mut used = BTreeSet::new();
    for (index, (epoch, first, json, transition)) in rows.into_iter().enumerate() {
        if epoch != index as i64 + 1 || first < previous_first {
            return Err(Error::Policy);
        }
        previous_first = first;
        let a: Authority = serde_json::from_str(&json)?;
        a.validate()?;
        if a.network != *n || !used.insert(a.public_key.clone()) {
            return Err(Error::Policy);
        }
        if let Some(old) = previous {
            let t: Transition = serde_json::from_str(transition.as_deref().ok_or(Error::Policy)?)?;
            t.verify()?;
            let hash: String = c.query_row(
                "SELECT hash FROM circuitnet_catalog_revisions WHERE network=?1 AND revision=?2",
                params![n.as_str(), t.change.revision as i64],
                |r| r.get(0),
            )?;
            if t.change.previous != old
                || t.change.replacement != a
                || first != (t.change.revision + 1) as i64
                || hash != t.change.catalog_hash
            {
                return Err(Error::Policy);
            }
        } else if first != 1 || transition.is_some() {
            return Err(Error::Policy);
        }
        previous = Some(a);
    }
    if previous.as_ref() != Some(latest) {
        return Err(Error::Policy);
    }
    Ok(())
}
impl RuntimeDatabase {
    /// A local sensitive-configuration caller must supply the independently confirmed
    /// NEW fingerprint. Emergency mode never pretends to have old-key authorization.
    pub fn circuitnet_catalog_replace_key(
        &mut self,
        actor: &str,
        n: &NetworkId,
        t: &Transition,
        confirmed_fingerprint: &str,
        now: i64,
    ) -> Result<bool, Error> {
        let result = (|| {
            t.verify()?;
            if t.change.replacement.fingerprint()? != confirmed_fingerprint {
                return Err(CatalogError::Publisher.into());
            }
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let a = authority(&tx, n)?.ok_or(Error::Policy)?;
            if t.change.replacement.network != *n {
                return Err(CatalogError::Publisher.into());
            }
            let encoded = serde_json::to_string(t)?;
            if a == t.change.replacement {
                let stored:Option<String>=tx.query_row("SELECT transition_object FROM circuitnet_catalog_keys WHERE network=?1 AND transition_object=?2",params![n.as_str(),encoded],|r|r.get(0)).optional()?.flatten();
                return if stored.as_deref() == Some(&encoded) {
                    Ok(false)
                } else {
                    Err(Error::Conflict)
                };
            }
            if a != t.change.previous {
                return Err(CatalogError::Publisher.into());
            }
            let s = current(&tx, n)?.ok_or(Error::Policy)?;
            if s.body.revision != t.change.revision || s.hash != t.change.catalog_hash {
                return Err(CatalogError::Fork.into());
            }
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM circuitnet_catalog_keys WHERE network=?1",
                [n.as_str()],
                |r| r.get(0),
            )?;
            if count >= 64 {
                return Err(Error::Capacity);
            }
            let keys: Vec<String> = tx
                .prepare("SELECT authority FROM circuitnet_catalog_keys WHERE network=?1")?
                .query_map([n.as_str()], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            for value in keys {
                if serde_json::from_str::<Authority>(&value)?.public_key
                    == t.change.replacement.public_key
                {
                    return Err(Error::Conflict);
                }
            }
            tx.execute(
                "INSERT INTO circuitnet_catalog_keys VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    n.as_str(),
                    count + 1,
                    (t.change.revision + 1) as i64,
                    serde_json::to_string(&t.change.replacement)?,
                    encoded,
                    now
                ],
            )?;
            tx.execute(
                "UPDATE circuitnet_catalog_authority SET authority=?2 WHERE network=?1",
                params![n.as_str(), serde_json::to_string(&t.change.replacement)?],
            )?;
            audit(
                &tx,
                n,
                actor,
                &format!(
                    "catalog-key-replaced:{:?}:{}:{}",
                    t.change.mode,
                    t.change.revision,
                    t.change.replacement.fingerprint()?
                ),
                now,
            )?;
            tx.commit()?;
            Ok(true)
        })();
        if result.is_err() {
            audit(
                &self.connection,
                n,
                actor,
                "catalog-key-replacement-rejected",
                now,
            )?;
        }
        result
    }
}
