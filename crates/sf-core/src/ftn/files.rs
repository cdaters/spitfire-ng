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

//! FTN file adapters around native file authority. No host paths in services.
use super::*;
use crate::{FileAdminActor, FileAreaId, FileId, FileIntegrity, FileLifecycle, FileStorage};
use sf_net::{qwk, tic};
use std::io::Read;
#[path = "file_admission.rs"]
mod admission;
pub use admission::FileReceiveContext;
use admission::{StoredTic, TicReceipt};
#[path = "freq.rs"]
mod freq;
pub use freq::*;

#[derive(Debug, thiserror::Error)]
pub enum FileNetworkError {
    #[error("file-network policy or authentication denied")]
    Denied,
    #[error("file-network state changed; refresh and retry")]
    Conflict,
    #[error("file-network capacity reached")]
    Capacity,
    #[error("native file storage unavailable or requires recovery")]
    Storage,
    #[error("file-network database operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("file-network metadata rejected")]
    Codec(#[from] tic::Error),
    #[error("FTN authority rejected file work")]
    Ftn(#[from] super::Error),
}
type Result<T> = std::result::Result<T, FileNetworkError>;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePolicy {
    pub enabled: bool,
    pub freq: bool,
    pub max_payload: u64,
    pub staging_bytes: u64,
    pub staging_count: u32,
    pub staging_age: u32,
    pub freq_files: u32,
    pub freq_bytes: u64,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEchoArea {
    pub domain: Domain,
    pub tag: String,
    pub native_area: i64,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub description: String,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSubscription {
    pub link: String,
    pub domain: Domain,
    pub tag: String,
    pub inbound: bool,
    pub subscribed: bool,
    pub held: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreqGrant {
    pub link: String,
    pub name: String,
    pub file: i64,
    pub enabled: bool,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileQueueRow {
    pub delivery: String,
    pub link: String,
    pub filename: String,
    pub kind: String,
    pub area: Option<String>,
    pub origin: Option<String>,
    pub provenance: Option<String>,
    pub size: u64,
    pub sha256: String,
    pub payload_accepted: bool,
    pub tic_accepted: bool,
    pub held: bool,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileActivity {
    pub link: Option<String>,
    pub filename: Option<String>,
    pub result: String,
    pub files: u32,
    pub bytes: u64,
    pub time: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileStatus {
    #[serde(default)]
    pub requests: Vec<FreqRequest>,
    pub native_files: Vec<NativeFileChoice>,
    pub policy: FilePolicy,
    pub areas: Vec<FileEchoArea>,
    pub subscriptions: Vec<FileSubscription>,
    pub grants: Vec<FreqGrant>,
    pub queue: Vec<FileQueueRow>,
    pub activity: Vec<FileActivity>,
    pub staged: u32,
    pub staged_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeFileChoice {
    pub id: i64,
    pub area: i64,
    pub area_name: String,
    pub filename: String,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HatchPreview {
    pub file: i64,
    pub file_version: u64,
    pub native_area: i64,
    pub filename: String,
    pub transfer_name: String,
    pub description: String,
    pub size: u64,
    pub sha256: String,
    pub crc: u32,
    pub recipients: Vec<String>,
    pub area: FileEchoArea,
}
#[derive(Clone, Debug)]
pub struct FileWork {
    pub key: String,
    pub name: String,
    pub size: u64,
    pub time: u64,
}

fn metadata(json: &str) -> Result<tic::Metadata> {
    serde_json::from_str(json).map_err(|_| FileNetworkError::Storage)
}
fn no_active(conn: &rusqlite::Connection) -> Result<()> {
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM binkp_link_health WHERE session_id IS NOT NULL)",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(FileNetworkError::Conflict);
    }
    Ok(())
}
fn session_link(conn: &rusqlite::Connection, session: &str) -> Result<String> {
    conn.query_row(
        "SELECT link_id FROM binkp_link_health WHERE session_id=?1 AND authenticated=1",
        [session],
        |r| r.get(0),
    )
    .optional()?
    .ok_or(FileNetworkError::Denied)
}
fn activity(
    tx: &Transaction<'_>,
    link: Option<&str>,
    name: Option<&str>,
    code: &str,
    files: u32,
    bytes: u64,
    now: i64,
) -> Result<()> {
    tx.execute("INSERT INTO ftn_file_activity(link_id,name,result,files,bytes,occurred_at) VALUES(?1,?2,?3,?4,?5,?6)",params![link,name,code,files,bytes as i64,now])?;
    tx.execute("DELETE FROM ftn_file_activity WHERE activity_id NOT IN (SELECT activity_id FROM ftn_file_activity ORDER BY activity_id DESC LIMIT 256)",[])?;
    super::event(tx, code, now)?;
    Ok(())
}
fn capacity(conn: &rusqlite::Connection, additional: usize) -> Result<()> {
    let count:i64=conn.query_row("SELECT (SELECT COUNT(*) FROM ftn_file_deliveries)+(SELECT COUNT(*) FROM ftn_file_publications)",[],|r|r.get(0))?;
    if count + additional as i64 > 10000 {
        return Err(FileNetworkError::Capacity);
    }
    Ok(())
}
fn recipients(
    conn: &rusqlite::Connection,
    policy: &Policy,
    map: &FileEchoArea,
    meta: &tic::Metadata,
    ingress: Option<&str>,
) -> Result<Vec<String>> {
    if !map.enabled || !map.outbound {
        return Ok(vec![]);
    }
    let candidates=conn.prepare("SELECT link_id FROM ftn_file_subscriptions WHERE domain=?1 AND tag=?2 AND subscribed=1 ORDER BY link_id")?
        .query_map(params![map.domain.as_str(),map.tag],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
    Ok(candidates
        .into_iter()
        .filter(|id| {
            policy.link(id).is_ok_and(|l| {
                l.outbound
                    && Some(id.as_str()) != ingress
                    && l.remote != meta.origin
                    && !meta.seen.contains(&l.remote)
                    && !meta.path.iter().any(|p| p.address == l.remote)
            })
        })
        .collect())
}
#[allow(clippy::too_many_arguments)]
fn publish(
    tx: &Transaction<'_>,
    policy: &Policy,
    map: &FileEchoArea,
    meta: &tic::Metadata,
    file: FileId,
    sha: &str,
    ingress: Option<&str>,
    received_tic: Option<&TicReceipt>,
    now: i64,
) -> Result<String> {
    let current: i64 = tx.query_row(
        "SELECT version FROM ftn_file_areas WHERE domain=?1 AND tag=?2 AND enabled=1",
        params![map.domain.as_str(), map.tag],
        |r| r.get(0),
    )?;
    if current != map.version {
        return Err(FileNetworkError::Conflict);
    }
    let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM files f JOIN file_areas a USING(area_id) WHERE file_id=?1 AND area_id=?2 AND sha256=?3 AND size_bytes=?4 AND lifecycle='active' AND a.active=1)",params![file.get(),map.native_area,sha,meta.size as i64],|r|r.get(0))?;
    if !valid {
        return Err(FileNetworkError::Conflict);
    }
    let targets = recipients(tx, policy, map, meta, ingress)?;
    capacity(tx, targets.len() + 1)?;
    let publication = super::id();
    let mut json = serde_json::to_value(meta).map_err(|_| FileNetworkError::Storage)?;
    if let Some(received) = received_tic {
        json["received_tic"] =
            serde_json::to_value(received).map_err(|_| FileNetworkError::Storage)?;
    }
    let json = json.to_string();
    tx.execute(
        "INSERT INTO ftn_file_publications VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            publication,
            map.domain.as_str(),
            map.tag,
            file.get(),
            sha,
            meta.size as i64,
            json,
            ingress,
            if ingress.is_some() {
                "inbound"
            } else {
                "hatch"
            },
            now
        ],
    )?;
    for link in &targets {
        tx.execute("INSERT INTO ftn_file_deliveries(delivery_id,publication_id,link_id,file_id,name,sha256,size,kind,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'fileecho',?8)",params![super::id(),publication,link,file.get(),meta.file,sha,meta.size as i64,now])?;
    }
    activity(
        tx,
        ingress,
        Some(&meta.file),
        if ingress.is_some() {
            "fileecho-imported"
        } else {
            "file-hatched"
        },
        targets.len() as u32,
        meta.size,
        now,
    )?;
    Ok(publication)
}
impl RuntimeDatabase {
    pub fn file_network_policy(&self) -> Result<FilePolicy> {
        Ok(self.connection.query_row("SELECT enabled,freq,max_payload,staging_bytes,staging_count,staging_age,freq_files,freq_bytes,version FROM ftn_file_policy WHERE singleton=1",[],|r|Ok(FilePolicy{enabled:r.get(0)?,freq:r.get(1)?,max_payload:r.get::<_,u32>(2)?.into(),staging_bytes:r.get::<_,u32>(3)?.into(),staging_count:r.get(4)?,staging_age:r.get(5)?,freq_files:r.get(6)?,freq_bytes:r.get::<_,u32>(7)?.into(),version:r.get(8)?}))?)
    }
    pub fn configure_file_network(
        &mut self,
        principal: &str,
        value: &FilePolicy,
        now: i64,
    ) -> Result<()> {
        if value.max_payload == 0
            || value.max_payload > tic::MAX_PAYLOAD
            || !(32768..=134217728).contains(&value.staging_bytes)
            || !(1..=67108864).contains(&value.freq_bytes)
        {
            return Err(FileNetworkError::Denied);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        no_active(&tx)?;
        if tx.execute("UPDATE ftn_file_policy SET enabled=?1,freq=?2,max_payload=?3,staging_bytes=?4,staging_count=?5,staging_age=?6,freq_files=?7,freq_bytes=?8,version=version+1 WHERE singleton=1 AND version=?9",params![value.enabled,value.freq,value.max_payload as i64,value.staging_bytes as i64,value.staging_count,value.staging_age,value.freq_files,value.freq_bytes as i64,value.version])?!=1 { return Err(FileNetworkError::Conflict); }
        super::audit(&tx, principal, "ftn-file-policy", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn configure_fileecho_area(
        &mut self,
        policy: &Policy,
        principal: &str,
        value: &FileEchoArea,
        now: i64,
    ) -> Result<()> {
        if tic::area(&value.tag)? != value.tag
            || value.description.len() > 160
            || value.description.chars().any(char::is_control)
            || !policy
                .akas
                .iter()
                .any(|a| a.enabled && a.endpoint.domain == value.domain)
        {
            return Err(FileNetworkError::Denied);
        }
        let area = self
            .load_area_by_id(
                FileAreaId::new(value.native_area).map_err(|_| FileNetworkError::Denied)?,
            )
            .map_err(|_| FileNetworkError::Storage)?
            .ok_or(FileNetworkError::Denied)?;
        if !area.active {
            return Err(FileNetworkError::Denied);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::bind(&tx, policy)?;
        no_active(&tx)?;
        let old: Option<(i64, i64)> = tx
            .query_row(
                "SELECT version,area_id FROM ftn_file_areas WHERE domain=?1 AND tag=?2",
                params![value.domain.as_str(), value.tag],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if old.map_or(value.version != 0, |(v, a)| {
            v != value.version || a != value.native_area
        }) {
            return Err(FileNetworkError::Conflict);
        }
        if old.is_none()
            && tx.query_row("SELECT COUNT(*) FROM ftn_file_areas", [], |r| {
                r.get::<_, u32>(0)
            })? >= 256
        {
            return Err(FileNetworkError::Capacity);
        }
        tx.execute("INSERT INTO ftn_file_areas VALUES(?1,?2,?3,?4,?5,?6,?7,1) ON CONFLICT(domain,tag) DO UPDATE SET enabled=excluded.enabled,inbound=excluded.inbound,outbound=excluded.outbound,description=excluded.description,version=ftn_file_areas.version+1",params![value.domain.as_str(),value.tag,value.native_area,value.enabled,value.inbound,value.outbound,value.description])?;
        super::audit(&tx, principal, "ftn-file-area", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn configure_file_subscription(
        &mut self,
        policy: &Policy,
        principal: &str,
        value: &FileSubscription,
        now: i64,
    ) -> Result<()> {
        if policy.link(&value.link)?.remote.domain != value.domain {
            return Err(FileNetworkError::Denied);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::bind(&tx, policy)?;
        no_active(&tx)?;
        let old:Option<i64>=tx.query_row("SELECT version FROM ftn_file_subscriptions WHERE link_id=?1 AND domain=?2 AND tag=?3",params![value.link,value.domain.as_str(),value.tag],|r|r.get(0)).optional()?;
        if old.unwrap_or(0) != value.version {
            return Err(FileNetworkError::Conflict);
        }
        if old.is_none()
            && tx.query_row("SELECT COUNT(*) FROM ftn_file_subscriptions", [], |r| {
                r.get::<_, u32>(0)
            })? >= 1024
        {
            return Err(FileNetworkError::Capacity);
        }
        tx.execute("INSERT INTO ftn_file_subscriptions VALUES(?1,?2,?3,?4,?5,?6,1) ON CONFLICT(link_id,domain,tag) DO UPDATE SET inbound=excluded.inbound,subscribed=excluded.subscribed,held=excluded.held,version=ftn_file_subscriptions.version+1",params![value.link,value.domain.as_str(),value.tag,value.inbound,value.subscribed,value.held])?;
        if !value.subscribed {
            tx.execute("UPDATE ftn_file_deliveries SET held=1,last_error='unsubscribed',version=version+1 WHERE link_id=?1 AND accepted_at IS NULL AND publication_id IN (SELECT publication_id FROM ftn_file_publications WHERE domain=?2 AND tag=?3)",params![value.link,value.domain.as_str(),value.tag])?;
        }
        if !value.held {
            tx.execute(
                "UPDATE binkp_link_health SET held=0,next_attempt=NULL WHERE link_id=?1",
                [&value.link],
            )?;
        }
        super::audit(&tx, principal, "ftn-file-subscription", now)?;
        tx.commit()?;
        Ok(())
    }
    fn fileecho_area(&self, domain: &Domain, tag: &str) -> Result<FileEchoArea> {
        self.connection.query_row("SELECT area_id,enabled,inbound,outbound,description,version FROM ftn_file_areas WHERE domain=?1 AND tag=?2",params![domain.as_str(),tag],|r|Ok(FileEchoArea{domain:domain.clone(),tag:tag.into(),native_area:r.get(0)?,enabled:r.get(1)?,inbound:r.get(2)?,outbound:r.get(3)?,description:r.get(4)?,version:r.get(5)?})).optional()?.ok_or(FileNetworkError::Denied)
    }
    fn file_network_bytes(
        &self,
        storage: &FileStorage,
        id: i64,
        max: u64,
        public: bool,
    ) -> Result<(crate::FileEntry, Vec<u8>)> {
        let file = self
            .load_file_by_id(FileId::new(id).map_err(|_| FileNetworkError::Denied)?)
            .map_err(|_| FileNetworkError::Storage)?
            .ok_or(FileNetworkError::Denied)?;
        let area = self
            .load_area_by_id(file.area_id)
            .map_err(|_| FileNetworkError::Storage)?
            .ok_or(FileNetworkError::Denied)?;
        // Public FREQ uses a deliberately explicit zero-threshold public area.
        if !area.active
            || file.lifecycle != FileLifecycle::Active
            || !matches!(
                file.integrity,
                FileIntegrity::Present | FileIntegrity::Unknown
            )
            || file.size_bytes == 0
            || file.size_bytes > max
            || (public
                && (area.read_security.get() != 0
                    || area.access_mode != crate::FileAccessMode::AtLeast))
        {
            return Err(FileNetworkError::Denied);
        }
        let (root, locator) = self
            .resolve_file_storage(file.id)
            .map_err(|_| FileNetworkError::Storage)?;
        if root.kind != crate::StorageRootKind::Managed || locator.relative_path != file.filename {
            return Err(FileNetworkError::Denied);
        }
        let managed:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM file_storage_locators l JOIN file_storage_roots r USING(storage_root_id) WHERE l.file_id=?1 AND r.area_id=?2 AND r.priority=0)",params![id,area.id.get()],|r|r.get(0))?;
        if !managed {
            return Err(FileNetworkError::Denied);
        }
        let mut bytes = vec![];
        storage
            .open_download(&area, &file)
            .map_err(|_| FileNetworkError::Storage)?
            .take(max + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FileNetworkError::Storage)?;
        if bytes.len() as u64 != file.size_bytes || qwk::digest(&bytes) != file.sha256 {
            return Err(FileNetworkError::Storage);
        }
        Ok((file, bytes))
    }
    pub fn preview_file_hatch(
        &self,
        policy: &Policy,
        storage: &FileStorage,
        file_id: i64,
        domain: &Domain,
        tag: &str,
        now: i64,
    ) -> Result<HatchPreview> {
        let limits = self.file_network_policy()?;
        if !limits.enabled {
            return Err(FileNetworkError::Denied);
        }
        let map = self.fileecho_area(domain, tag)?;
        let (file, bytes) = self.file_network_bytes(storage, file_id, limits.max_payload, false)?;
        if file.area_id.get() != map.native_area || !map.enabled || !map.outbound {
            return Err(FileNetworkError::Denied);
        }
        let meta = hatch_metadata(policy, &map, &file, &bytes, now)?;
        Ok(HatchPreview {
            file: file_id,
            file_version: file.state_version,
            native_area: map.native_area,
            filename: file.filename,
            transfer_name: meta.file.clone(),
            description: meta.description(),
            size: meta.size,
            sha256: file.sha256,
            crc: meta.crc,
            recipients: recipients(&self.connection, policy, &map, &meta, None)?,
            area: map,
        })
    }
    pub fn hatch_native_file(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        principal: &str,
        preview: &HatchPreview,
        now: i64,
    ) -> Result<String> {
        let current = self.preview_file_hatch(
            policy,
            storage,
            preview.file,
            &preview.area.domain,
            &preview.area.tag,
            now,
        )?;
        if current.file_version != preview.file_version
            || current.area.version != preview.area.version
            || current.recipients != preview.recipients
            || current.sha256 != preview.sha256
        {
            return Err(FileNetworkError::Conflict);
        }
        let (file, bytes) = self.file_network_bytes(
            storage,
            preview.file,
            self.file_network_policy()?.max_payload,
            false,
        )?;
        let meta = hatch_metadata(policy, &current.area, &file, &bytes, now)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::bind(&tx, policy)?;
        no_active(&tx)?;
        let version: i64 = tx.query_row(
            "SELECT state_version FROM files WHERE file_id=?1",
            [file.id.get()],
            |r| r.get(0),
        )?;
        if version as u64 != preview.file_version {
            return Err(FileNetworkError::Conflict);
        }
        let publication = publish(
            &tx,
            policy,
            &current.area,
            &meta,
            file.id,
            &file.sha256,
            None,
            None,
            now,
        )?;
        super::audit(&tx, principal, "ftn-file-hatch", now)?;
        tx.commit()?;
        Ok(publication)
    }
    pub fn configure_freq_grant(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        principal: &str,
        grant: &FreqGrant,
        now: i64,
    ) -> Result<()> {
        if tic::filename(&grant.name)? != grant.name {
            return Err(FileNetworkError::Denied);
        }
        policy.link(&grant.link)?;
        if grant.enabled {
            let (file, _) = self.file_network_bytes(
                storage,
                grant.file,
                self.file_network_policy()?.max_payload,
                true,
            )?;
            if tic::transfer_name(&file.filename, &file.sha256)? != grant.name {
                return Err(FileNetworkError::Denied);
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::bind(&tx, policy)?;
        no_active(&tx)?;
        let old: Option<i64> = tx
            .query_row(
                "SELECT version FROM ftn_freq_grants WHERE link_id=?1 AND name=?2",
                params![grant.link, grant.name],
                |r| r.get(0),
            )
            .optional()?;
        if old.unwrap_or(0) != grant.version {
            return Err(FileNetworkError::Conflict);
        }
        if old.is_none()
            && tx.query_row("SELECT COUNT(*) FROM ftn_freq_grants", [], |r| {
                r.get::<_, u32>(0)
            })? >= 256
        {
            return Err(FileNetworkError::Capacity);
        }
        tx.execute("INSERT INTO ftn_freq_grants VALUES(?1,?2,?3,?4,1) ON CONFLICT(link_id,name) DO UPDATE SET file_id=excluded.file_id,enabled=excluded.enabled,version=ftn_freq_grants.version+1",params![grant.link,grant.name,grant.file,grant.enabled])?;
        super::audit(&tx, principal, "ftn-freq-grant", now)?;
        tx.commit()?;
        Ok(())
    }
}
fn hatch_metadata(
    policy: &Policy,
    map: &FileEchoArea,
    file: &crate::FileEntry,
    bytes: &[u8],
    now: i64,
) -> Result<tic::Metadata> {
    let aka = policy
        .akas
        .iter()
        .find(|a| a.enabled && a.primary && a.endpoint.domain == map.domain)
        .ok_or(FileNetworkError::Denied)?;
    let wire_name = tic::transfer_name(&file.filename, &file.sha256)?;
    if [".TIC", ".REQ", ".PKT"]
        .iter()
        .any(|ext| wire_name.ends_with(ext))
    {
        return Err(FileNetworkError::Denied);
    }
    let meta = tic::Metadata {
        area: map.tag.clone(),
        file: wire_name.clone(),
        long_name: (wire_name != file.filename).then(|| file.filename.clone()),
        origin: aka.endpoint.clone(),
        from: aka.endpoint.clone(),
        to: None,
        size: file.size_bytes,
        crc: tic::crc32(bytes),
        descriptions: vec![],
        long_descriptions: file.description.lines().map(str::to_owned).collect(),
        path: vec![tic::Hop {
            address: aka.endpoint.clone(),
            time: now.try_into().map_err(|_| FileNetworkError::Denied)?,
            detail: String::new(),
        }],
        seen: vec![aka.endpoint.clone()],
        opaque: vec![],
    };
    meta.encode("VALIDATE")?;
    Ok(meta)
}

impl RuntimeDatabase {
    /// Preserve the sender's offer identity for request replay suppression.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_file_offer(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        session: &str,
        offer: &sf_net::binkp::Offer,
        bytes: &[u8],
        now: i64,
        verify: &dyn Fn(&str, &str) -> bool,
    ) -> Result<()> {
        let name = tic::filename(&offer.name)?;
        if name.ends_with(".REQ") {
            let link = session_link(&self.connection, session)?;
            if !self.file_network_policy()?.enabled {
                return Err(FileNetworkError::Denied);
            }
            return self.serve_freq(session, storage, &link, &name, bytes, offer.time, now);
        }
        self.receive_file_artifact(policy, storage, session, &name, bytes, now, verify)
    }
    /// Receive with immutable raw-TIC custody and authenticated transport authority.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_file_offer_with_context(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        context: &FileReceiveContext<'_>,
        session: &str,
        offer: &sf_net::binkp::Offer,
        bytes: &[u8],
        now: i64,
        verify: &dyn Fn(&str, &str) -> bool,
    ) -> Result<()> {
        if tic::filename(&offer.name)?.ends_with(".REQ") {
            return self.receive_file_offer(policy, storage, session, offer, bytes, now, verify);
        }
        self.receive_file_artifact_admitted(
            policy,
            storage,
            Some(context),
            session,
            &offer.name,
            bytes,
            now,
            verify,
        )
    }
    /// Rotation invalidates previously authenticated incomplete controls.
    pub fn invalidate_staged_tics(&mut self, link: &str, now: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        no_active(&tx)?;
        tx.execute(
            "DELETE FROM ftn_file_staging WHERE link_id=?1 AND kind='tic'",
            [link],
        )?;
        activity(
            &tx,
            Some(link),
            None,
            "file-tic-credential-invalidated",
            0,
            0,
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Custody is admitted only from the current authenticated transport lease.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_file_artifact(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        session: &str,
        wire_name: &str,
        bytes: &[u8],
        now: i64,
        verify: &dyn Fn(&str, &str) -> bool,
    ) -> Result<()> {
        self.receive_file_artifact_admitted(
            policy, storage, None, session, wire_name, bytes, now, verify,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn receive_file_artifact_admitted(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        context: Option<&FileReceiveContext<'_>>,
        session: &str,
        wire_name: &str,
        bytes: &[u8],
        now: i64,
        verify: &dyn Fn(&str, &str) -> bool,
    ) -> Result<()> {
        let link = session_link(&self.connection, session)?;
        let limits = self.file_network_policy()?;
        if !limits.enabled {
            return Err(FileNetworkError::Denied);
        }
        let name = tic::filename(wire_name)?;
        if name.ends_with(".REQ") {
            return self.serve_freq(session, storage, &link, &name, bytes, now as u64, now);
        }
        if bytes.is_empty() || bytes.len() as u64 > limits.max_payload {
            return Err(FileNetworkError::Capacity);
        }
        let configured = policy.link(&link)?;
        let remote = &configured.remote;
        let local = &policy.aka(&configured.aka)?.endpoint;
        if let Some(context) = context {
            context.transport.validate(policy)?;
            context.transport.link(&link)?;
            let current: bool = self.connection.query_row(
                "SELECT policy_digest=?2 FROM binkp_link_health WHERE session_id=?1 AND authenticated=1",
                params![session, context.transport.digest(policy)?], |r| r.get(0),
            )?;
            let observed: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM binkp_peer_addresses p JOIN ftn_addresses a USING(address_id) WHERE p.link_id=?1 AND a.domain=?2 AND a.zone=?3 AND a.net=?4 AND a.node=?5 AND a.point=?6)",
                params![link, remote.domain.as_str(), remote.address.zone(), remote.address.net(), remote.address.node(), remote.address.point()], |r| r.get(0),
            )?;
            if !current || !observed || !configured.enabled || policy.local(local).is_none() {
                return Err(FileNetworkError::Denied);
            }
        }
        let (pair, kind, content, digest) = if name.ends_with(".TIC") {
            let parsed = tic::parse(bytes, &remote.domain).or_else(|error| {
                let Some(context) = context else {
                    return Err(error);
                };
                let envelope = tic::parse_direct_hatch(bytes, remote, local)?;
                if !admission::unambiguous_hatch(&envelope.metadata, policy, context.transport) {
                    return Err(tic::Error::Malformed);
                }
                Ok(envelope)
            });
            let envelope = match parsed {
                Ok(tic) => tic,
                Err(error) => {
                    self.record_file_rejection(
                        &link,
                        Some(&name),
                        match error {
                            tic::Error::Filename => "file-unsafe-name",
                            _ => "file-malformed-tic",
                        },
                        now,
                    )?;
                    return Ok(());
                }
            };
            if !envelope.authenticates(|supplied| verify(&link, supplied))
                || envelope.metadata.from != *remote
                || envelope
                    .metadata
                    .to
                    .as_ref()
                    .is_some_and(|to| !policy.akas.iter().any(|a| a.enabled && a.endpoint == *to))
            {
                self.record_file_rejection(
                    &link,
                    Some(&name),
                    "file-tic-authentication-rejected",
                    now,
                )?;
                return Ok(());
            }
            let meta = envelope.metadata;
            let map = match self.fileecho_area(&remote.domain, &meta.area) {
                Ok(map) => map,
                Err(_) => {
                    self.record_file_rejection(&link, Some(&name), "file-area-rejected", now)?;
                    return Ok(());
                }
            };
            let permitted:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_file_subscriptions WHERE link_id=?1 AND domain=?2 AND tag=?3 AND inbound=1)",params![link,remote.domain.as_str(),meta.area],|r|r.get(0))?;
            if !map.enabled
                || !map.inbound
                || !permitted
                || meta.size > limits.max_payload
                || meta.file.ends_with(".TIC")
                || meta.file.ends_with(".REQ")
                || meta.file.ends_with(".PKT")
                || meta.path.iter().any(|p| {
                    policy
                        .akas
                        .iter()
                        .any(|a| a.enabled && a.endpoint == p.address)
                })
            {
                self.record_file_rejection(&link, Some(&name), "file-routing-rejected", now)?;
                return Ok(());
            }
            // Replay identity excludes receive-time provenance; retries retain the
            // first admitted receipt instead of conflicting across sessions.
            let digest =
                qwk::digest(&serde_json::to_vec(&meta).map_err(|_| FileNetworkError::Storage)?);
            let received_tic = if let Some(context) = context {
                let _permit = context
                    .artifacts
                    .admit_import()
                    .map_err(|_| FileNetworkError::Storage)?;
                Some(TicReceipt {
                    artifact: self
                        .preserve_artifact(context.artifacts, bytes, now)
                        .map_err(|_| FileNetworkError::Storage)?,
                    peer: remote.clone(),
                    session: session.to_owned(),
                    received_at: now,
                    direct_hatch: envelope.direct_hatch,
                })
            } else {
                None
            };
            let pair = meta.file.clone();
            let content = serde_json::to_vec(&StoredTic {
                metadata: meta,
                received_tic,
            })
            .map_err(|_| FileNetworkError::Storage)?;
            (pair, "tic", content, digest)
        } else {
            (name, "payload", bytes.to_vec(), qwk::digest(bytes))
        };
        if kind == "payload" {
            self.receive_freq_payload(storage, &link, &pair, &content, now)?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Expiration is a custody cleanup, never a native-file deletion.
        let expired = tx.execute(
            "DELETE FROM ftn_file_staging WHERE received_at<?1",
            [now - i64::from(limits.staging_age)],
        )?;
        if expired > 0 {
            activity(
                &tx,
                None,
                None,
                "file-incomplete-pair-expired",
                expired as u32,
                0,
                now,
            )?;
        }
        let previous: Option<String> = tx
            .query_row(
                "SELECT digest FROM ftn_file_staging WHERE link_id=?1 AND name=?2 AND kind=?3",
                params![link, pair, kind],
                |r| r.get(0),
            )
            .optional()?;
        if previous.as_ref().is_some_and(|old| old != &digest) {
            activity(
                &tx,
                Some(&link),
                Some(&pair),
                "file-pair-conflict",
                0,
                0,
                now,
            )?;
            tx.commit()?;
            return Ok(());
        }
        if previous.is_none() {
            let (count, total): (u32, i64) = tx.query_row(
                "SELECT COUNT(*),COALESCE(SUM(length(content)),0) FROM ftn_file_staging",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if count >= limits.staging_count
                || total as u64 + content.len() as u64 > limits.staging_bytes
            {
                return Err(FileNetworkError::Capacity);
            }
            tx.execute(
                "INSERT INTO ftn_file_staging VALUES(?1,?2,?3,?4,?5,?6)",
                params![link, pair, kind, content, digest, now],
            )?;
        }
        tx.commit()?;
        self.complete_file_pair(policy, storage, context, &link, &pair, now)
    }
    fn record_file_rejection(
        &mut self,
        link: &str,
        name: Option<&str>,
        reason: &str,
        now: i64,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        activity(&tx, Some(link), name, reason, 0, 0, now)?;
        tx.commit()?;
        Ok(())
    }
    fn complete_file_pair(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        context: Option<&FileReceiveContext<'_>>,
        link: &str,
        name: &str,
        now: i64,
    ) -> Result<()> {
        let row:Option<(Vec<u8>,Vec<u8>)>=self.connection.query_row("SELECT t.content,p.content FROM ftn_file_staging t JOIN ftn_file_staging p ON p.link_id=t.link_id AND p.name=t.name AND p.kind='payload' WHERE t.link_id=?1 AND t.name=?2 AND t.kind='tic'",params![link,name],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((json, bytes)) = row else {
            return Ok(());
        };
        let stored: StoredTic =
            serde_json::from_slice(&json).map_err(|_| FileNetworkError::Storage)?;
        let meta = stored.metadata;
        if let Some(receipt) = stored
            .received_tic
            .as_ref()
            .filter(|r| r.direct_hatch.is_some())
        {
            let Some(context) = context else {
                return Err(FileNetworkError::Denied);
            };
            let configured = policy.link(link)?;
            if receipt.peer != configured.remote
                || meta.origin != configured.remote
                || meta.to.as_ref() != Some(&policy.aka(&configured.aka)?.endpoint)
                || !admission::unambiguous_hatch(&meta, policy, context.transport)
            {
                return Err(FileNetworkError::Denied);
            }
        }
        if let Err(error) = meta.validate_payload(&bytes) {
            self.record_file_rejection(
                link,
                Some(name),
                if error == tic::Error::Size {
                    "file-size-mismatch"
                } else {
                    "file-checksum-mismatch"
                },
                now,
            )?;
            return Ok(());
        }
        let map = self.fileecho_area(&meta.from.domain, &meta.area)?;
        let permitted:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_file_subscriptions WHERE link_id=?1 AND domain=?2 AND tag=?3 AND inbound=1)",params![link,map.domain.as_str(),map.tag],|r|r.get(0))?;
        if !map.enabled || !map.inbound || !permitted || policy.link(link)?.remote != meta.from {
            return Err(FileNetworkError::Denied);
        }
        let sha = qwk::digest(&bytes);
        let exists:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_file_publications WHERE domain=?1 AND tag=?2 AND sha256=?3 AND size=?4)",params![map.domain.as_str(),map.tag,sha,bytes.len() as i64],|r|r.get(0))?;
        if exists {
            let tx = self.connection.transaction()?;
            tx.execute(
                "DELETE FROM ftn_file_staging WHERE link_id=?1 AND name=?2",
                params![link, name],
            )?;
            activity(
                &tx,
                Some(link),
                Some(name),
                "file-duplicate-suppressed",
                0,
                0,
                now,
            )?;
            tx.commit()?;
            return Ok(());
        }
        capacity(&self.connection, 34)?;
        let native:Option<i64>=self.connection.query_row("SELECT file_id FROM files WHERE area_id=?1 AND sha256=?2 AND size_bytes=?3 ORDER BY file_id LIMIT 1",params![map.native_area,sha,bytes.len() as i64],|r|r.get(0)).optional()?;
        let commit = |tx: &Transaction<'_>, id: FileId| -> rusqlite::Result<()> {
            publish(
                tx,
                policy,
                &map,
                &meta,
                id,
                &sha,
                Some(link),
                stored.received_tic.as_ref(),
                now,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
            tx.execute(
                "DELETE FROM ftn_file_staging WHERE link_id=?1 AND name=?2",
                params![link, name],
            )?;
            Ok(())
        };
        if let Some(id) = native {
            self.file_network_bytes(storage, id, self.file_network_policy()?.max_payload, false)?;
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            commit(&tx, FileId::new(id).map_err(|_| FileNetworkError::Storage)?)?;
            tx.commit()?;
        } else {
            let area = self
                .load_area_by_id(
                    FileAreaId::new(map.native_area).map_err(|_| FileNetworkError::Storage)?,
                )
                .map_err(|_| FileNetworkError::Storage)?
                .ok_or(FileNetworkError::Denied)?;
            if !area.active {
                return Err(FileNetworkError::Denied);
            }
            let collision: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM files WHERE area_id=?1 AND normalized_filename=?2)",
                params![map.native_area, meta.file],
                |r| r.get(0),
            )?;
            let native_name = if collision {
                format!("{}-{}", &sha[..16], meta.file)
            } else {
                meta.file.clone()
            };
            self.add_managed_file_committing(
                storage,
                FileAdminActor::LocalOperator,
                area.id,
                area.state_version,
                &native_name,
                &if meta.description().trim().is_empty() {
                    meta.file.clone()
                } else {
                    meta.description()
                },
                &bytes,
                commit,
            )
            .map_err(|_| FileNetworkError::Storage)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn serve_freq(
        &mut self,
        session: &str,
        storage: &FileStorage,
        link: &str,
        name: &str,
        bytes: &[u8],
        wire_time: u64,
        now: i64,
    ) -> Result<()> {
        let limits = self.file_network_policy()?;
        let names = if limits.freq {
            tic::parse_request(bytes, limits.freq_files as usize).ok()
        } else {
            None
        };
        let Some(names) = names else {
            self.record_file_rejection(link, Some(name), "freq-request-denied", now)?;
            return Ok(());
        };
        let mut selected = vec![];
        let mut total = 0u64;
        for name in names {
            let id:Option<i64>=self.connection.query_row("SELECT file_id FROM ftn_freq_grants WHERE link_id=?1 AND name=?2 AND enabled=1",params![link,name],|r|r.get(0)).optional()?;
            let result = id.and_then(|id| {
                self.file_network_bytes(storage, id, limits.max_payload, true)
                    .ok()
            });
            let Some((file, _)) = result else {
                self.record_file_rejection(link, None, "freq-request-denied", now)?;
                return Ok(());
            };
            total += file.size_bytes;
            if total > limits.freq_bytes {
                self.record_file_rejection(link, None, "freq-bound-exceeded", now)?;
                return Ok(());
            }
            selected.push((name, file));
        }
        let request_key = qwk::digest(
            &[
                link.as_bytes(),
                name.as_bytes(),
                &wire_time.to_be_bytes(),
                bytes,
            ]
            .concat(),
        );
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        capacity(&tx, selected.len())?;
        if tx.execute("UPDATE binkp_link_health SET freq_files=freq_files+?2,freq_bytes=freq_bytes+?3 WHERE session_id=?1 AND authenticated=1 AND freq_files+?2<=?4 AND freq_bytes+?3<=?5",params![session,selected.len() as i64,total as i64,limits.freq_files,limits.freq_bytes as i64])?!=1 {
            activity(&tx,Some(link),None,"freq-session-bound-exceeded",0,0,now)?;tx.commit()?;return Ok(());
        }
        for (name, file) in &selected {
            tx.execute("INSERT OR IGNORE INTO ftn_file_deliveries(delivery_id,link_id,file_id,name,sha256,size,kind,request_key,tic_accepted,created_at) VALUES(?1,?2,?3,?4,?5,?6,'freq-response',?7,1,?8)",params![super::id(),link,file.id.get(),name,file.sha256,file.size_bytes as i64,request_key,now])?;
        }
        activity(
            &tx,
            Some(link),
            None,
            "freq-response-queued",
            selected.len() as u32,
            total,
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn request_ftn_files(
        &mut self,
        policy: &Policy,
        principal: &str,
        link: &str,
        names: &[String],
        native_area: i64,
        now: i64,
    ) -> Result<String> {
        let limits = self.file_network_policy()?;
        if !limits.enabled || !policy.link(link)?.outbound {
            return Err(FileNetworkError::Denied);
        }
        let area = self
            .load_area_by_id(FileAreaId::new(native_area).map_err(|_| FileNetworkError::Denied)?)
            .map_err(|_| FileNetworkError::Storage)?
            .ok_or(FileNetworkError::Denied)?;
        if !area.active {
            return Err(FileNetworkError::Denied);
        }
        let bytes = (names.join("\r\n") + "\r\n").into_bytes();
        let requested = tic::parse_request(&bytes, limits.freq_files as usize)?;
        if requested.iter().any(|name| {
            [".TIC", ".REQ", ".PKT"]
                .iter()
                .any(|ext| name.ends_with(ext))
        }) {
            return Err(FileNetworkError::Denied);
        }
        let delivery = super::id();
        let remote = &policy.link(link)?.remote.address;
        let name = format!("{:04X}{:04X}.REQ", remote.net(), remote.node());
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::bind(&tx, policy)?;
        capacity(&tx, 1)?;
        for name in &requested {
            let pending: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_inbound WHERE link_id=?1 AND name=?2 AND file_id IS NULL)", params![link,name], |r| r.get(0))?;
            if pending {
                return Err(FileNetworkError::Conflict);
            }
        }
        tx.execute("INSERT INTO ftn_file_deliveries(delivery_id,link_id,name,sha256,size,kind,request,tic_accepted,created_at) VALUES(?1,?2,?3,?4,?5,'freq-request',?6,1,?7)",params![delivery,link,name,qwk::digest(&bytes),bytes.len() as i64,bytes,now])?;
        tx.execute(
            "INSERT INTO ftn_freq_recovery(request_id) VALUES(?1)",
            [&delivery],
        )?;
        tx.execute(
            "INSERT INTO ftn_freq_attempts(request_id,number,delivery_id) VALUES(?1,1,?1)",
            [&delivery],
        )?;
        for name in requested {
            tx.execute(
                "INSERT INTO ftn_freq_inbound(request_id,link_id,name,area_id) VALUES(?1,?2,?3,?4)",
                params![delivery, link, name, native_area],
            )?;
        }
        super::audit(&tx, principal, "ftn-freq-request", now)?;
        tx.commit()?;
        Ok(delivery)
    }
    pub fn file_network_status(&self) -> Result<FileStatus> {
        let areas = self
            .connection
            .prepare("SELECT domain,tag FROM ftn_file_areas ORDER BY domain,tag LIMIT 256")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(d, t)| self.fileecho_area(&d.parse().map_err(super::Error::from)?, &t))
            .collect::<Result<Vec<_>>>()?;
        let subscriptions=self.connection.prepare("SELECT link_id,domain,tag,inbound,subscribed,held,version FROM ftn_file_subscriptions ORDER BY link_id,domain,tag LIMIT 1024")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,bool>(3)?,r.get::<_,bool>(4)?,r.get::<_,bool>(5)?,r.get::<_,i64>(6)?)))?.collect::<std::result::Result<Vec<_>,_>>()?.into_iter().map(|(link,d,tag,inbound,subscribed,held,version)|Ok(FileSubscription{link,domain:d.parse().map_err(super::Error::from)?,tag,inbound,subscribed,held,version})).collect::<Result<Vec<_>>>()?;
        let grants=self.connection.prepare("SELECT link_id,name,file_id,enabled,version FROM ftn_freq_grants ORDER BY link_id,name LIMIT 256")?.query_map([],|r|Ok(FreqGrant{link:r.get(0)?,name:r.get(1)?,file:r.get(2)?,enabled:r.get(3)?,version:r.get(4)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let queue=self.connection.prepare("SELECT d.delivery_id,d.link_id,d.name,d.kind,d.size,d.sha256,d.payload_accepted,d.tic_accepted,d.held,d.attempts,d.last_error,d.version,p.tag,p.metadata,p.source FROM ftn_file_deliveries d LEFT JOIN ftn_file_publications p USING(publication_id) ORDER BY d.created_at DESC,d.delivery_id LIMIT 100")?.query_map([],|r|Ok(FileQueueRow{delivery:r.get(0)?,link:r.get(1)?,filename:r.get(2)?,kind:r.get(3)?,size:r.get::<_,u32>(4)?.into(),sha256:r.get(5)?,payload_accepted:r.get(6)?,tic_accepted:r.get(7)?,held:r.get(8)?,attempts:r.get(9)?,last_error:r.get(10)?,version:r.get(11)?,area:r.get(12)?,origin:r.get::<_,Option<String>>(13)?.and_then(|json|metadata(&json).ok()).map(|m|m.origin.to_string()),provenance:r.get(14)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let activity=self.connection.prepare("SELECT link_id,name,result,files,bytes,occurred_at FROM ftn_file_activity ORDER BY activity_id DESC LIMIT 100")?.query_map([],|r|Ok(FileActivity{link:r.get(0)?,filename:r.get(1)?,result:r.get(2)?,files:r.get(3)?,bytes:r.get::<_,u32>(4)?.into(),time:r.get(5)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let (staged, staged_bytes): (u32, u32) = self.connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(length(content)),0) FROM ftn_file_staging",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let native_files = self.connection.prepare("SELECT file_id,area_id,a.name,f.filename,size_bytes FROM files f JOIN file_areas a USING(area_id) WHERE a.active=1 AND f.lifecycle='active' ORDER BY area_id,file_id LIMIT 256")?.query_map([],|r|Ok(NativeFileChoice{id:r.get(0)?,area:r.get(1)?,area_name:r.get(2)?,filename:r.get(3)?,size:r.get::<_,i64>(4)? as u64}))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(FileStatus {
            requests: self.freq_requests()?,
            native_files,
            policy: self.file_network_policy()?,
            areas,
            subscriptions,
            grants,
            queue,
            activity,
            staged,
            staged_bytes: staged_bytes.into(),
        })
    }
    pub fn hold_file_delivery(
        &mut self,
        principal: &str,
        delivery: &str,
        expected: i64,
        held: bool,
        now: i64,
    ) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let freq_blocked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_attempts a JOIN ftn_freq_recovery r USING(request_id) JOIN ftn_file_deliveries d ON d.delivery_id=a.delivery_id WHERE d.delivery_id=?1 AND (r.held=1 OR d.attempts>=12))", [delivery], |r| r.get(0))?;
        if !held && freq_blocked {
            return Err(FileNetworkError::Denied);
        }
        if tx.execute("UPDATE ftn_file_deliveries SET held=?1,last_error=NULL,attempts=CASE WHEN ?1=0 AND kind<>'freq-request' THEN 0 ELSE attempts END,version=version+1 WHERE delivery_id=?2 AND version=?3 AND session_id IS NULL AND accepted_at IS NULL",params![held,delivery,expected])?!=1 {return Err(FileNetworkError::Conflict)}
        if !held {
            tx.execute("UPDATE binkp_link_health SET held=0,next_attempt=NULL WHERE link_id=(SELECT link_id FROM ftn_file_deliveries WHERE delivery_id=?1)",[delivery])?;
        }
        super::audit(
            &tx,
            principal,
            if held {
                "ftn-file-held"
            } else {
                "ftn-file-released"
            },
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
}

impl RuntimeDatabase {
    /// Claim finite immutable in-memory transfer snapshots. Native integrity and
    /// current FREQ grants are checked again before every offer, including retry.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub fn claim_file_work(
        &mut self,
        policy: &Policy,
        storage: &FileStorage,
        session: &str,
        credential: &dyn Fn(&str) -> Option<String>,
        max_files: usize,
        max_bytes: u64,
    ) -> Result<Vec<(FileWork, Vec<u8>)>> {
        let link = session_link(&self.connection, session)?;
        let limits = self.file_network_policy()?;
        if !limits.enabled {
            return Ok(vec![]);
        }
        self.connection.execute("UPDATE ftn_file_deliveries SET held=1,last_error='file-attempt-limit',version=version+1 WHERE link_id=?1 AND accepted_at IS NULL AND session_id IS NULL AND held=0 AND attempts>=12",[&link])?;
        let ids=self.connection.prepare("SELECT delivery_id FROM ftn_file_deliveries WHERE link_id=?1 AND accepted_at IS NULL AND held=0 AND session_id IS NULL AND attempts<12 ORDER BY created_at,delivery_id LIMIT 32")?.query_map([&link],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let mut prepared = vec![];
        let mut total = 0u64;
        let mut freq_count = 0u32;
        let mut freq_bytes = 0u64;
        for id in ids {
            let (publication,file,name,sha,size,kind,request,pa,ta,time):(Option<String>,Option<i64>,String,String,u32,String,Option<Vec<u8>>,bool,bool,i64)=self.connection.query_row("SELECT publication_id,file_id,name,sha256,size,kind,request,payload_accepted,tic_accepted,created_at FROM ftn_file_deliveries WHERE delivery_id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?)))?;
            if kind == "freq-response"
                && (freq_count >= limits.freq_files
                    || freq_bytes + u64::from(size) > limits.freq_bytes)
            {
                continue;
            }
            if kind == "freq-request" && !freq::sendable(&self.connection, &id)? {
                continue;
            }
            let result = (|| -> Result<Vec<(FileWork, Vec<u8>)>> {
                let mut items = vec![];
                let mut meta = None;
                if let Some(publication) = &publication {
                    let json: String = self.connection.query_row(
                        "SELECT metadata FROM ftn_file_publications WHERE publication_id=?1",
                        [publication],
                        |r| r.get(0),
                    )?;
                    let stored: StoredTic =
                        serde_json::from_str(&json).map_err(|_| FileNetworkError::Storage)?;
                    let mut value = stored.metadata;
                    if let Some(receipt) = stored.received_tic.filter(|r| r.direct_hatch.is_some())
                    {
                        // This is witnessed ingress, separate from received Seenby.
                        // The untimed remote PATH stays in private provenance.
                        value.seen.push(receipt.peer);
                    }
                    let map = self.fileecho_area(&value.from.domain, &value.area)?;
                    let permits:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_file_subscriptions WHERE link_id=?1 AND domain=?2 AND tag=?3 AND subscribed=1 AND held=0)",params![link,map.domain.as_str(),map.tag],|r|r.get(0))?;
                    if !map.enabled || !map.outbound || !permits {
                        return Ok(vec![]);
                    }
                    let local = policy.aka(&policy.link(&link)?.aka)?.endpoint.clone();
                    value.from = local.clone();
                    value.to = Some(policy.link(&link)?.remote.clone());
                    if value.path.last().is_none_or(|h| h.address != local) {
                        value.path.push(tic::Hop {
                            address: local.clone(),
                            time: time.try_into().map_err(|_| FileNetworkError::Storage)?,
                            detail: String::new(),
                        });
                    }
                    value.seen.push(local);
                    let targets = self
                        .connection
                        .prepare("SELECT link_id FROM ftn_file_deliveries WHERE publication_id=?1")?
                        .query_map([publication], |r| r.get::<_, String>(0))?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for target in targets {
                        value.seen.push(policy.link(&target)?.remote.clone());
                    }
                    value.seen.sort();
                    value.seen.dedup();
                    meta = Some(value);
                }
                if kind == "freq-response" {
                    let permitted:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_grants WHERE link_id=?1 AND file_id=?2 AND name=?3 AND enabled=1)",params![link,file,name],|r|r.get(0))?;
                    if !limits.freq || !permitted {
                        return Err(FileNetworkError::Denied);
                    }
                }
                if !pa {
                    let bytes = if kind == "freq-request" {
                        request.clone().ok_or(FileNetworkError::Storage)?
                    } else {
                        self.file_network_bytes(
                            storage,
                            file.ok_or(FileNetworkError::Storage)?,
                            limits.max_payload,
                            kind == "freq-response",
                        )?
                        .1
                    };
                    if bytes.len() != size as usize || qwk::digest(&bytes) != sha {
                        return Err(FileNetworkError::Storage);
                    }
                    items.push((
                        FileWork {
                            key: format!("{id}:p"),
                            name: name.clone(),
                            size: bytes.len() as u64,
                            time: time.try_into().map_err(|_| FileNetworkError::Storage)?,
                        },
                        bytes,
                    ));
                }
                if !ta {
                    let bytes = meta
                        .ok_or(FileNetworkError::Storage)?
                        .encode(&credential(&link).ok_or(FileNetworkError::Denied)?)?;
                    items.push((
                        FileWork {
                            key: format!("{id}:t"),
                            name: format!("{}.TIC", id[..8].to_ascii_uppercase()),
                            size: bytes.len() as u64,
                            time: time.try_into().map_err(|_| FileNetworkError::Storage)?,
                        },
                        bytes,
                    ));
                }
                Ok(items)
            })();
            let items = match result {
                Ok(items) => items,
                Err(_) => {
                    self.connection.execute("UPDATE ftn_file_deliveries SET held=1,last_error='file-unavailable-or-policy',version=version+1 WHERE delivery_id=?1",[&id])?;
                    continue;
                }
            };
            let size = items.iter().map(|(w, _)| w.size).sum::<u64>();
            if prepared.len() + items.len() > max_files || total + size > max_bytes {
                break;
            }
            if items.is_empty() {
                continue;
            }
            if self.connection.execute("UPDATE ftn_file_deliveries SET session_id=?2,payload_offered=0,tic_offered=0,attempts=attempts+1,version=version+1 WHERE delivery_id=?1 AND session_id IS NULL AND accepted_at IS NULL",params![id,session])?!=1 {return Err(FileNetworkError::Conflict)}
            if kind == "freq-response" {
                freq_count += 1;
                freq_bytes += size;
            }
            prepared.extend(items);
            total += size;
        }
        Ok(prepared)
    }
    pub fn file_work_offered(&mut self, session: &str, key: &str) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        session_link(&tx, session)?;
        let (id, part) = key.split_once(':').ok_or(FileNetworkError::Denied)?;
        if !freq::sendable(&tx, id)? {
            return Err(FileNetworkError::Denied);
        }
        let sql=match part {"p"=>"UPDATE ftn_file_deliveries SET payload_offered=1 WHERE delivery_id=?1 AND session_id=?2 AND payload_accepted=0", "t"=>"UPDATE ftn_file_deliveries SET tic_offered=1 WHERE delivery_id=?1 AND session_id=?2 AND tic_accepted=0",_=>return Err(FileNetworkError::Denied)};
        if tx.execute(sql, params![id, session])? != 1 {
            return Err(FileNetworkError::Conflict);
        }
        tx.execute(
            "UPDATE ftn_freq_attempts SET offered=1 WHERE delivery_id=?1",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn file_work_accepted(&mut self, session: &str, key: &str, now: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let link = session_link(&tx, session)?;
        let (id, part) = key.split_once(':').ok_or(FileNetworkError::Denied)?;
        let sql=match part {"p"=>"UPDATE ftn_file_deliveries SET payload_accepted=1,version=version+1 WHERE delivery_id=?1 AND session_id=?2 AND payload_offered=1 AND payload_accepted=0", "t"=>"UPDATE ftn_file_deliveries SET tic_accepted=1,version=version+1 WHERE delivery_id=?1 AND session_id=?2 AND tic_offered=1 AND tic_accepted=0",_=>return Err(FileNetworkError::Denied)};
        if tx.execute(sql, params![id, session])? != 1 {
            return Err(FileNetworkError::Conflict);
        }
        if tx.execute("UPDATE ftn_file_deliveries SET accepted_at=?2,last_error=NULL,session_id=NULL WHERE delivery_id=?1 AND payload_accepted=1 AND tic_accepted=1",params![id,now])?==1 {
            activity(&tx,Some(&link),None,"file-delivery-accepted",1,0,now)?;
            tx.execute("UPDATE ftn_freq_recovery SET version=version+1 WHERE request_id IN (SELECT request_id FROM ftn_freq_attempts WHERE delivery_id=?1)", [id])?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;

impl RuntimeDatabase {
    fn receive_freq_payload(
        &mut self,
        storage: &FileStorage,
        link: &str,
        name: &str,
        bytes: &[u8],
        now: i64,
    ) -> Result<bool> {
        let pending:Option<(String,i64)>=self.connection.query_row("SELECT request_id,area_id FROM ftn_freq_inbound WHERE link_id=?1 AND name=?2 AND file_id IS NULL",params![link,name],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let sha = qwk::digest(bytes);
        let Some((request, area_id)) = pending else {
            return Ok(self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_inbound WHERE link_id=?1 AND name=?2 AND sha256=?3 AND size=?4 AND file_id IS NOT NULL)",params![link,name,sha,bytes.len() as i64],|r|r.get(0))?);
        };
        let allowed: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM ftn_freq_recovery r JOIN ftn_freq_attempts a USING(request_id) WHERE r.request_id=?1 AND r.held=0 AND a.offered=1)", [&request], |r| r.get(0))?;
        if !allowed {
            return Err(FileNetworkError::Denied);
        }
        let limits = self.file_network_policy()?;
        let total: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(size),0) FROM ftn_freq_inbound WHERE request_id=?1",
            [&request],
            |r| r.get(0),
        )?;
        if bytes.is_empty()
            || bytes.len() as u64 > limits.max_payload
            || total as u64 + bytes.len() as u64 > limits.freq_bytes
        {
            return Err(FileNetworkError::Capacity);
        }
        let area = self
            .load_area_by_id(FileAreaId::new(area_id).map_err(|_| FileNetworkError::Storage)?)
            .map_err(|_| FileNetworkError::Storage)?
            .ok_or(FileNetworkError::Denied)?;
        if !area.active {
            return Err(FileNetworkError::Denied);
        }
        let existing:Option<i64>=self.connection.query_row("SELECT file_id FROM files WHERE area_id=?1 AND sha256=?2 AND size_bytes=?3 ORDER BY file_id LIMIT 1",params![area_id,sha,bytes.len() as i64],|r|r.get(0)).optional()?;
        let commit = |tx: &Transaction<'_>, file: FileId| -> rusqlite::Result<()> {
            if tx.execute("UPDATE ftn_freq_inbound SET file_id=?3,sha256=?4,size=?5,received_at=?6 WHERE request_id=?1 AND name=?2 AND file_id IS NULL",params![request,name,file.get(),sha,bytes.len() as i64,now])?!=1 {return Err(rusqlite::Error::InvalidQuery);}
            tx.execute(
                "UPDATE ftn_freq_recovery SET version=version+1 WHERE request_id=?1",
                [&request],
            )?;
            activity(
                tx,
                Some(link),
                Some(name),
                "freq-file-received",
                1,
                bytes.len() as u64,
                now,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(())
        };
        if let Some(id) = existing {
            self.file_network_bytes(storage, id, limits.max_payload, false)?;
            let tx = self.connection.transaction()?;
            commit(&tx, FileId::new(id).map_err(|_| FileNetworkError::Storage)?)?;
            tx.commit()?;
        } else {
            let collision: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM files WHERE area_id=?1 AND normalized_filename=?2)",
                params![area_id, name],
                |r| r.get(0),
            )?;
            let native = if collision {
                format!("{}-{name}", &sha[..16])
            } else {
                name.to_owned()
            };
            self.add_managed_file_committing(
                storage,
                FileAdminActor::LocalOperator,
                area.id,
                area.state_version,
                &native,
                "Requested through authenticated FTN link",
                bytes,
                commit,
            )
            .map_err(|_| FileNetworkError::Storage)?;
        }
        Ok(true)
    }
}
