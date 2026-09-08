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

//! File Dossiers and delivery receipts reference approved native Files authority.
use super::{audit, profile, Codename, Error, NetworkId, NodeId, Profile, Role};
use crate::files::{AdmissionStatus, FilesError};
use crate::{FileAdminActor, FileAreaId, FileStorage, RuntimeDatabase};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sf_net::circuitnet::{
    files::{Outcome, Publication, Receipt, Want, MAX_FILE},
    MessageId,
};
use std::io::Read;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mapping {
    pub codename: Codename,
    pub area: i64,
    pub send: bool,
    pub receive: bool,
    pub maximum_bytes: u64,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileDossier {
    pub neighbor: NodeId,
    pub codename: Codename,
    pub subscribed: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delivery {
    pub publication: MessageId,
    pub neighbor: NodeId,
    pub attempts: u32,
    pub receipt: Option<Receipt>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub pending: u64,
    #[serde(default)]
    pub completed: u64,
    #[serde(default)]
    pub failed: u64,
    pub mappings: Vec<Mapping>,
    pub dossiers: Vec<FileDossier>,
    pub deliveries: Vec<Delivery>,
}
fn mapping(c: &Connection, n: &NetworkId, code: &Codename) -> Result<Mapping, Error> {
    c.query_row("SELECT area_id,send,receive,maximum_bytes,version FROM circuitnet_file_mappings WHERE network=?1 AND codename=?2",params![n.as_str(),code.as_str()],|r|Ok(Mapping{codename:code.clone(),area:r.get(0)?,send:r.get(1)?,receive:r.get(2)?,maximum_bytes:r.get::<_,i64>(3)? as u64,version:r.get(4)?})).optional()?.ok_or(Error::Policy)
}
fn subscribed(
    c: &Connection,
    p: &Profile,
    neighbor: &NodeId,
    code: &Codename,
) -> Result<bool, Error> {
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_file_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3 AND subscribed=1)",params![p.network.as_str(),neighbor.as_str(),code.as_str()],|r|r.get(0))?)
}
fn fanout(c: &Connection, p: &Profile, publication: &Publication, now: i64) -> Result<(), Error> {
    if p.topology.node(&p.local)?.role == Role::End && publication.origin != p.local {
        return Ok(());
    }
    let map = mapping(c, &p.network, &publication.codename)?;
    if !map.send || publication.size > map.maximum_bytes {
        return Ok(());
    }
    for neighbor in p.topology.neighbors(&p.local)? {
        if publication.path.contains(&neighbor)
            || !subscribed(c, p, &neighbor, &publication.codename)?
        {
            continue;
        }
        c.execute("INSERT INTO circuitnet_file_deliveries(network,identity,neighbor,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",params![p.network.as_str(),publication.id.as_str(),neighbor.as_str(),now])?;
    }
    Ok(())
}
impl RuntimeDatabase {
    pub fn circuitnet_file_map(
        &mut self,
        actor: &str,
        network: &NetworkId,
        value: &Mapping,
        now: i64,
    ) -> Result<(), Error> {
        if value.maximum_bytes == 0 || value.maximum_bytes > MAX_FILE {
            return Err(Error::Policy);
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        profile(&tx, network)?;
        let valid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_areas WHERE area_id=?1 AND active=1)",
            [value.area],
            |r| r.get(0),
        )?;
        if !valid {
            return Err(Error::Policy);
        }
        let old = mapping(&tx, network, &value.codename).ok();
        if old.as_ref().map_or(0, |m| m.version) != value.version
            || old.as_ref().is_some_and(|m| m.area != value.area)
        {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO circuitnet_file_mappings VALUES(?1,?2,?3,?4,?5,?6,1) ON CONFLICT(network,codename) DO UPDATE SET send=excluded.send,receive=excluded.receive,maximum_bytes=excluded.maximum_bytes,version=version+1",params![network.as_str(),value.codename.as_str(),value.area,value.send,value.receive,value.maximum_bytes as i64])?;
        audit(&tx, network, actor, "file-mapping", now)?;
        tx.execute(
            "UPDATE network_preparation SET generation=generation+1 WHERE singleton=1",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_file_subscribe(
        &mut self,
        actor: &str,
        network: &NetworkId,
        value: &FileDossier,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(&value.neighbor)?;
        mapping(&tx, network, &value.codename)?;
        let version:i64=tx.query_row("SELECT version FROM circuitnet_file_dossiers WHERE network=?1 AND neighbor=?2 AND codename=?3",params![network.as_str(),value.neighbor.as_str(),value.codename.as_str()],|r|r.get(0)).optional()?.unwrap_or(0);
        if value.version != version {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO circuitnet_file_dossiers VALUES(?1,?2,?3,?4,1) ON CONFLICT(network,neighbor,codename) DO UPDATE SET subscribed=excluded.subscribed,version=version+1",params![network.as_str(),value.neighbor.as_str(),value.codename.as_str(),value.subscribed])?;
        audit(
            &tx,
            network,
            actor,
            if value.subscribed {
                "file-subscribe"
            } else {
                "file-unsubscribe"
            },
            now,
        )?;
        tx.execute(
            "UPDATE network_preparation SET generation=generation+1 WHERE singleton=1",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_file_status(&self, network: &NetworkId) -> Result<Status, Error> {
        profile(&self.connection, network)?;
        let codes=self.connection.prepare("SELECT codename FROM circuitnet_file_mappings WHERE network=?1 ORDER BY codename LIMIT 128")?.query_map([network.as_str()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        let mappings = codes
            .into_iter()
            .map(|c| mapping(&self.connection, network, &Codename::new(&c)?))
            .collect::<Result<Vec<_>, Error>>()?;
        let rows=self.connection.prepare("SELECT neighbor,codename,subscribed,version FROM circuitnet_file_dossiers WHERE network=?1 ORDER BY neighbor,codename LIMIT 4096")?.query_map([network.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,bool>(2)?,r.get::<_,i64>(3)?)))?.collect::<Result<Vec<_>,_>>()?;
        let dossiers = rows
            .into_iter()
            .map(|(n, c, subscribed, version)| {
                Ok(FileDossier {
                    neighbor: NodeId::new(&n)?,
                    codename: Codename::new(&c)?,
                    subscribed,
                    version,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let rows=self.connection.prepare("SELECT identity,neighbor,attempts,receipt,error FROM circuitnet_file_deliveries WHERE network=?1 ORDER BY updated_at DESC LIMIT 100")?.query_map([network.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?)))?.collect::<Result<Vec<_>,_>>()?;
        let deliveries = rows
            .into_iter()
            .map(|(id, n, attempts, receipt, error)| {
                Ok(Delivery {
                    publication: MessageId::new(&id)?,
                    neighbor: NodeId::new(&n)?,
                    attempts,
                    receipt: receipt.map(|r| serde_json::from_str(&r)).transpose()?,
                    error,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let (pending,completed,failed):(i64,i64,i64)=self.connection.query_row("SELECT COUNT(*) FILTER(WHERE receipt IS NULL),COUNT(*) FILTER(WHERE receipt IS NOT NULL),COUNT(*) FILTER(WHERE receipt IS NULL AND attempts>=12) FROM circuitnet_file_deliveries WHERE network=?1",[network.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        Ok(Status {
            pending: pending as u64,
            completed: completed as u64,
            failed: failed as u64,
            mappings,
            dossiers,
            deliveries,
        })
    }
    /// Queue native approved work independently from Event exchange timing.
    pub fn circuitnet_files_prepare(
        &mut self,
        network: &NetworkId,
        now: i64,
    ) -> Result<usize, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        if !p.enabled {
            return Ok(0);
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM circuitnet_file_publications",
            [],
            |r| r.get(0),
        )?;
        if count >= 100000 {
            return Err(Error::Capacity);
        }
        let rows=tx.prepare("SELECT f.file_id,f.filename,f.description,f.sha256,f.size_bytes,f.uploaded_at,m.codename FROM files f JOIN file_validation v USING(file_id) JOIN file_areas a USING(area_id) JOIN circuitnet_file_mappings m USING(area_id) WHERE m.network=?1 AND m.send=1 AND a.active=1 AND f.lifecycle='active' AND v.status='published' AND v.source<>'circuitnet' AND f.size_bytes<=m.maximum_bytes AND NOT EXISTS(SELECT 1 FROM circuitnet_file_publications p WHERE p.network=?1 AND p.file_id=f.file_id AND p.codename=m.codename AND p.ingress IS NULL) ORDER BY f.file_id LIMIT 100")?.query_map([network.as_str()],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,String>(6)?)))?.collect::<Result<Vec<_>,_>>()?;
        let count = rows.len();
        for (file, filename, description, sha256, size, timestamp, codename) in rows {
            let publication = Publication {
                network: network.clone(),
                id: MessageId::new(&format!("{}:{:032x}", p.local, rand::random::<u128>()))?,
                origin: p.local.clone(),
                codename: Codename::new(&codename)?,
                sha256,
                size: size as u64,
                filename,
                description,
                timestamp,
                path: vec![p.local.clone()],
            };
            tx.execute(
                "INSERT INTO circuitnet_file_publications VALUES(?1,?2,?3,?4,?5,?6,NULL,NULL)",
                params![
                    network.as_str(),
                    publication.id.as_str(),
                    file,
                    codename,
                    serde_json::to_string(&publication)?,
                    publication.fingerprint()?
                ],
            )?;
        }
        // Re-evaluate approvals and subscription changes without re-originating imports.
        let rows=tx.prepare("SELECT p.envelope FROM circuitnet_file_publications p JOIN files f USING(file_id) JOIN file_validation v USING(file_id) JOIN file_areas a USING(area_id) WHERE p.network=?1 AND f.lifecycle='active' AND v.status='published' AND a.active=1 ORDER BY p.identity")?.query_map([network.as_str()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        for json in rows {
            fanout(&tx, &p, &serde_json::from_str(&json)?, now)?;
        }
        tx.commit()?;
        Ok(count)
    }
    pub fn circuitnet_file_offers(
        &mut self,
        network: &NetworkId,
        neighbor: &NodeId,
        now: i64,
    ) -> Result<Vec<Publication>, Error> {
        self.circuitnet_files_prepare(network, now)?;
        let (p, _) = profile(&self.connection, network)?;
        p.neighbor(neighbor)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        let rows=self.connection.prepare("SELECT p.envelope FROM circuitnet_file_deliveries d JOIN circuitnet_file_publications p USING(network,identity) JOIN files f USING(file_id) JOIN file_validation v USING(file_id) JOIN file_areas a USING(area_id) JOIN circuitnet_file_mappings m ON m.network=p.network AND m.codename=p.codename JOIN circuitnet_file_dossiers s ON s.network=d.network AND s.neighbor=d.neighbor AND s.codename=p.codename WHERE d.network=?1 AND d.neighbor=?2 AND d.receipt IS NULL AND d.attempts<12 AND f.lifecycle='active' AND v.status='published' AND a.active=1 AND m.send=1 AND m.maximum_bytes>=f.size_bytes AND s.subscribed=1 ORDER BY d.identity LIMIT 8")?.query_map(params![network.as_str(),neighbor.as_str()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        rows.into_iter()
            .map(|j| Ok(serde_json::from_str(&j)?))
            .collect()
    }
    /// Caller supplies the TLS-authenticated direct peer, never envelope identity alone.
    pub fn circuitnet_file_offer(
        &self,
        store: &FileStorage,
        network: &NetworkId,
        neighbor: &NodeId,
        publication: &Publication,
    ) -> Result<Want, Error> {
        publication.validate()?;
        let (p, _) = profile(&self.connection, network)?;
        p.neighbor(neighbor)?;
        if !p.enabled
            || publication.network != *network
            || publication.path.contains(&p.local)
            || p.topology.path(&publication.origin, neighbor)? != publication.path
        {
            return Err(Error::Policy);
        }
        let old:Option<(String,Option<String>)>=self.connection.query_row("SELECT fingerprint,receipt FROM circuitnet_file_publications WHERE network=?1 AND identity=?2",params![network.as_str(),publication.id.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((fingerprint, receipt)) = old {
            if fingerprint != publication.fingerprint()? {
                return Err(Error::Conflict);
            }
            if let Some(receipt) = receipt {
                return Ok(Want::Complete {
                    receipt: serde_json::from_str(&receipt)?,
                });
            }
        }
        let map = mapping(&self.connection, network, &publication.codename)?;
        let area = self
            .load_area_by_id(FileAreaId::new(map.area).map_err(FilesError::from)?)
            .map_err(FilesError::from)?
            .ok_or(Error::Policy)?;
        let policy = self.file_safety_policy(area.id)?;
        if !subscribed(&self.connection, &p, neighbor, &publication.codename)?
            || !map.receive
            || !area.active
            || publication.size
                > map
                    .maximum_bytes
                    .min(policy.archives.source_bytes)
                    .min(area.maximum_upload_bytes)
        {
            return Err(Error::Policy);
        }
        Ok(
            if store
                .open_content(&publication.sha256, publication.size)
                .is_ok()
            {
                Want::Have
            } else {
                Want::Send
            },
        )
    }
    pub fn circuitnet_file_receive(
        &mut self,
        store: &FileStorage,
        network: &NetworkId,
        neighbor: &NodeId,
        publication: &Publication,
        input: &mut dyn Read,
        now: i64,
    ) -> Result<Receipt, Error> {
        if let Want::Complete { receipt } =
            self.circuitnet_file_offer(store, network, neighbor, publication)?
        {
            return Ok(receipt);
        }
        let map = mapping(&self.connection, network, &publication.codename)?;
        let area = self
            .load_area_by_id(FileAreaId::new(map.area).map_err(FilesError::from)?)
            .map_err(FilesError::from)?
            .ok_or(Error::Policy)?;
        // The transport verifies before admission; this independent bounded check also protects direct adapters.
        let mut staged = tempfile::tempfile().map_err(FilesError::from)?;
        use sha2::{Digest, Sha256};
        use std::io::{Seek, Write};
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 65536];
        loop {
            let n = input.read(&mut buffer).map_err(FilesError::from)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > publication.size {
                return Err(Error::Conflict);
            }
            hash.update(&buffer[..n]);
            staged.write_all(&buffer[..n]).map_err(FilesError::from)?;
        }
        if size != publication.size || format!("{:x}", hash.finalize()) != publication.sha256 {
            return Err(Error::Conflict);
        }
        staged.rewind().map_err(FilesError::from)?;
        let imported = store.import_file(
            self,
            FileAdminActor::LocalOperator,
            &area,
            &publication.filename,
            &publication.description,
            &mut staged,
            "circuitnet",
            None,
        );
        let (file, outcome, reason) = match imported {
            Ok(imported) => {
                let admission = self
                    .file_admission(imported.file.id)?
                    .ok_or(Error::Policy)?;
                let outcome = match admission.status {
                    AdmissionStatus::Published => Outcome::Published,
                    AdmissionStatus::PendingApproval => Outcome::PendingApproval,
                    AdmissionStatus::Quarantined | AdmissionStatus::Inspecting => {
                        Outcome::Quarantined
                    }
                    AdmissionStatus::Rejected => Outcome::Rejected,
                };
                (Some(imported.file.id.get()), outcome, "local-policy")
            }
            Err(FilesError::Policy(_)) => (None, Outcome::Rejected, "native-admission-rejected"),
            Err(e) => return Err(e.into()),
        };
        let receipt = Receipt {
            publication: publication.id.clone(),
            sha256: publication.sha256.clone(),
            outcome,
            reason: reason.into(),
        };
        self.circuitnet_file_record(network, neighbor, publication, file, &receipt, now)?;
        Ok(receipt)
    }
    pub fn circuitnet_file_reject_hash(
        &mut self,
        store: &FileStorage,
        network: &NetworkId,
        neighbor: &NodeId,
        publication: &Publication,
        now: i64,
    ) -> Result<Receipt, Error> {
        if let Want::Complete { receipt } =
            self.circuitnet_file_offer(store, network, neighbor, publication)?
        {
            return Ok(receipt);
        }
        let receipt = Receipt {
            publication: publication.id.clone(),
            sha256: publication.sha256.clone(),
            outcome: Outcome::Rejected,
            reason: "hash-mismatch".into(),
        };
        self.circuitnet_file_record(network, neighbor, publication, None, &receipt, now)?;
        Ok(receipt)
    }
    fn circuitnet_file_record(
        &mut self,
        network: &NetworkId,
        neighbor: &NodeId,
        publication: &Publication,
        file: Option<i64>,
        receipt: &Receipt,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(neighbor)?;
        let mut forwarded = publication.clone();
        forwarded.path.push(p.local.clone());
        tx.execute("INSERT INTO circuitnet_file_publications VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(network,identity) DO NOTHING",params![network.as_str(),publication.id.as_str(),file,publication.codename.as_str(),serde_json::to_string(&forwarded)?,publication.fingerprint()?,neighbor.as_str(),serde_json::to_string(receipt)?])?;
        if receipt.outcome == Outcome::Published {
            fanout(&tx, &p, &forwarded, now)?;
        }
        audit(&tx, network, neighbor.as_str(), "file-receipt", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_file_retry(
        &mut self,
        actor: &str,
        network: &NetworkId,
        neighbor: &NodeId,
        id: &MessageId,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (p, _) = profile(&tx, network)?;
        p.neighbor(neighbor)?;
        if !p.enabled {
            return Err(Error::Policy);
        }
        let json: String = tx.query_row(
            "SELECT envelope FROM circuitnet_file_publications WHERE network=?1 AND identity=?2",
            params![network.as_str(), id.as_str()],
            |r| r.get(0),
        )?;
        let publication: Publication = serde_json::from_str(&json)?;
        if !subscribed(&tx, &p, neighbor, &publication.codename)?
            || !mapping(&tx, network, &publication.codename)?.send
        {
            return Err(Error::Policy);
        }
        if tx.execute("UPDATE circuitnet_file_deliveries SET attempts=0,error=NULL,updated_at=?4 WHERE network=?1 AND identity=?2 AND neighbor=?3 AND receipt IS NULL",params![network.as_str(),id.as_str(),neighbor.as_str(),now])?!=1 {return Err(Error::Conflict);}
        tx.execute(
            "UPDATE network_preparation SET generation=generation+1 WHERE singleton=1",
            [],
        )?;
        audit(&tx, network, actor, "file-retry", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn circuitnet_file_attempt(
        &mut self,
        network: &NetworkId,
        neighbor: &NodeId,
        id: &MessageId,
        receipt: Option<&Receipt>,
        now: i64,
    ) -> Result<(), Error> {
        if let Some(receipt) = receipt {
            let json:String=self.connection.query_row("SELECT envelope FROM circuitnet_file_publications WHERE network=?1 AND identity=?2",params![network.as_str(),id.as_str()],|r|r.get(0))?;
            let publication: Publication = serde_json::from_str(&json)?;
            if receipt.publication != *id
                || receipt.sha256 != publication.sha256
                || !matches!(
                    receipt.reason.as_str(),
                    "local-policy" | "native-admission-rejected" | "hash-mismatch"
                )
            {
                return Err(Error::Conflict);
            }
        }
        self.connection.execute("UPDATE circuitnet_file_deliveries SET attempts=attempts+CASE WHEN ?4 IS NULL THEN 1 ELSE 0 END,receipt=COALESCE(receipt,?4),error=CASE WHEN ?4 IS NULL THEN 'awaiting-receipt' ELSE NULL END,updated_at=?5 WHERE network=?1 AND identity=?2 AND neighbor=?3 AND receipt IS NULL",params![network.as_str(),id.as_str(),neighbor.as_str(),receipt.map(serde_json::to_string).transpose()?,now])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
