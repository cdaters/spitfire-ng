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

//! Native adapter authority for caller offline mail. Packet codecs cannot post.
use crate::{
    CallerId, Conference, ConferenceId, MessageActor, MessageBackend, MessageError, MessageId,
    MessageKind, MessageVisibility, NewMessage, NodeId, RuntimeDatabase, TransferId,
    TransferMethod,
};
use chrono::{TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, OptionalExtension, Transaction};
use sf_net::qwk::{self, Profile};
use std::collections::BTreeMap;
use thiserror::Error;

pub(crate) const MIGRATION: &str = r#"
ALTER TABLE caller_last_read ADD COLUMN pointer_version INTEGER NOT NULL DEFAULT 1 CHECK(pointer_version>0);
ALTER TABLE caller_last_read ADD COLUMN reset_version INTEGER NOT NULL DEFAULT 0 CHECK(reset_version>=0);
CREATE TRIGGER caller_pointer_version AFTER UPDATE OF last_message_number ON caller_last_read
WHEN NEW.pointer_version=OLD.pointer_version
BEGIN UPDATE caller_last_read SET pointer_version=pointer_version+1 WHERE caller_id=NEW.caller_id AND conference_id=NEW.conference_id; END;
CREATE TABLE network_artifacts (
 artifact_id TEXT PRIMARY KEY CHECK(length(artifact_id)=64),
 byte_length INTEGER NOT NULL CHECK(byte_length BETWEEN 0 AND 16777216),
 created_at INTEGER NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','complete'))
);
CREATE TABLE network_area_mappings (
 mapping_id INTEGER PRIMARY KEY,
 profile TEXT NOT NULL CHECK(profile='qwk-offline-cp437-v1'),
 wire_number INTEGER NOT NULL CHECK(wire_number BETWEEN 1 AND 65535),
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 state_version INTEGER NOT NULL DEFAULT 1 CHECK(state_version>0),
 UNIQUE(profile,wire_number), UNIQUE(profile,conference_id)
);
INSERT INTO network_area_mappings(profile,wire_number,conference_id)
 SELECT 'qwk-offline-cp437-v1',conference_number,conference_id FROM message_conferences;
CREATE TABLE qwk_requests (
 request_id TEXT PRIMARY KEY,
 caller_id INTEGER NOT NULL REFERENCES callers(caller_id),
 board_id TEXT NOT NULL CHECK(length(board_id) BETWEEN 1 AND 8),
 selection TEXT NOT NULL CHECK(selection IN ('new','to-you')),
 artifact_id TEXT REFERENCES network_artifacts(artifact_id),
 state TEXT NOT NULL CHECK(state IN ('prepared','delivered','confirmed','preview','failed','stale')),
 created_at INTEGER NOT NULL,
 transfer_id TEXT REFERENCES transfer_records(transfer_id)
);
CREATE TABLE qwk_manifest_conferences (
 request_id TEXT NOT NULL REFERENCES qwk_requests(request_id),
 mapping_id INTEGER NOT NULL REFERENCES network_area_mappings(mapping_id),
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 policy_digest TEXT NOT NULL,
 high_water INTEGER NOT NULL CHECK(high_water>=0),
 pointer_version INTEGER NOT NULL CHECK(pointer_version>=0),
 reset_version INTEGER NOT NULL CHECK(reset_version>=0),
 PRIMARY KEY(request_id,conference_id)
);
CREATE TABLE qwk_manifest_members (
 request_id TEXT NOT NULL REFERENCES qwk_requests(request_id),
 ordinal INTEGER NOT NULL,
 message_id INTEGER NOT NULL REFERENCES messages(message_id),
 state_version INTEGER NOT NULL,
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 wire_number INTEGER NOT NULL,
 message_number INTEGER NOT NULL,
 PRIMARY KEY(request_id,ordinal)
);
CREATE INDEX qwk_manifest_reply ON qwk_manifest_members(wire_number,message_number);
CREATE TABLE network_import_receipts (
 caller_id INTEGER NOT NULL REFERENCES callers(caller_id),
 profile TEXT NOT NULL CHECK(profile='qwk-offline-cp437-v1'),
 packet_digest TEXT NOT NULL CHECK(length(packet_digest)=64),
 ordinal INTEGER NOT NULL CHECK(ordinal>=0),
 member_digest TEXT NOT NULL CHECK(length(member_digest)=64),
 artifact_id TEXT NOT NULL REFERENCES network_artifacts(artifact_id),
 source_offset INTEGER NOT NULL CHECK(source_offset>=128),
 source_wall_time TEXT NOT NULL,
 received_at INTEGER NOT NULL,
 message_id INTEGER REFERENCES messages(message_id),
 outcome TEXT NOT NULL CHECK(outcome IN ('imported','rejected','control','possible-duplicate')),
 reason TEXT NOT NULL,
 PRIMARY KEY(caller_id,profile,packet_digest,ordinal)
);
CREATE INDEX network_import_content ON network_import_receipts(caller_id,member_digest);
CREATE TRIGGER network_receipt_no_update BEFORE UPDATE ON network_import_receipts BEGIN SELECT RAISE(ABORT,'network receipt is immutable'); END;
CREATE TRIGGER network_receipt_no_delete BEFORE DELETE ON network_import_receipts BEGIN SELECT RAISE(ABORT,'network receipt is retained'); END;
ALTER TABLE transfer_records ADD COLUMN purpose TEXT NOT NULL DEFAULT 'catalog-file' CHECK(purpose IN ('catalog-file','message-packet'));
ALTER TABLE transfer_records ADD COLUMN artifact_id TEXT REFERENCES network_artifacts(artifact_id);
"#;
/// Board-scoped admission shared by the daemon's existing session workers.
#[derive(Debug, Default)]
pub struct ImportCapacity(std::sync::atomic::AtomicUsize);
pub struct ImportPermit<'a>(&'a ImportCapacity);
impl ImportCapacity {
    pub fn acquire(&self) -> Result<ImportPermit<'_>, NetworkError> {
        self.0
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |n| (n < 2).then_some(n + 1),
            )
            .map_err(|_| NetworkError::Capacity)?;
        Ok(ImportPermit(self))
    }
}
impl Drop for ImportPermit<'_> {
    fn drop(&mut self) {
        self.0 .0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
const PROFILE: &str = NetworkKind::QwkOffline.profile();

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("QWK packet is unavailable")]
    Unavailable,
    #[error("QWK packet or pointer state changed; rebuild or confirm again")]
    Stale,
    #[error("no messages match this selection")]
    NoMessages,
    #[error("QWK capacity reached")]
    Capacity,
    #[error(transparent)]
    Codec(#[from] qwk::Error),
    #[error(transparent)]
    Message(#[from] MessageError),
    #[error("QWK database operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("QWK artifact operation failed")]
    Io(#[from] std::io::Error),
    #[error("QWK runtime operation failed")]
    Database(#[from] crate::DatabaseError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkKind {
    QwkOffline,
}
impl NetworkKind {
    pub const fn profile(self) -> &'static str {
        match self {
            Self::QwkOffline => "qwk-offline-cp437-v1",
        }
    }
}
#[derive(Clone, Debug)]
pub struct NetworkAreaMapping {
    pub id: i64,
    pub conference: ConferenceId,
    pub wire_number: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QwkSelection {
    New,
    ToYou,
}
#[derive(Debug)]
pub struct ExportReceipt {
    pub request_id: String,
    pub artifact_id: String,
    pub bytes: Vec<u8>,
    pub count: usize,
    pub board_id: String,
}
#[derive(Debug, Default, Eq, PartialEq)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub rejected: usize,
    pub controls: usize,
    pub possible_duplicates: usize,
}
#[derive(Clone, Debug)]
pub(crate) struct ImportReceipt {
    pub caller: CallerId,
    pub packet: String,
    pub ordinal: usize,
    pub member: String,
    pub artifact: String,
    pub offset: usize,
    pub wall: String,
    pub received: i64,
}
impl ImportReceipt {
    pub(crate) fn insert(
        &self,
        tx: &Transaction<'_>,
        message: Option<MessageId>,
        outcome: &str,
        reason: &str,
    ) -> rusqlite::Result<()> {
        tx.execute("INSERT INTO network_import_receipts(caller_id,profile,packet_digest,ordinal,member_digest,artifact_id,source_offset,source_wall_time,received_at,message_id,outcome,reason) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![self.caller.get(),PROFILE,self.packet,self.ordinal as i64,self.member,self.artifact,self.offset as i64,self.wall,self.received,message.map(MessageId::get),outcome,reason])?;
        Ok(())
    }
}

/// Storage interface implemented by the daemon. Codecs and native transactions
/// have no authority to choose host paths.
pub trait NetworkArtifactStore: Send + Sync {
    fn admit_import(&self) -> Result<ImportPermit<'_>, NetworkError>;
    fn preserve(&self, bytes: &[u8]) -> Result<String, NetworkError>;
    fn usage(&self) -> Result<(u64, usize), NetworkError>;
}
fn policy_digest(c: &Conference) -> String {
    qwk::digest(format!("{c:?}").as_bytes())
}
fn wire_text(text: &str) -> Result<Vec<u8>, NetworkError> {
    crate::encode_text(text, crate::TerminalTextEncoding::Cp437)
        .ok_or(qwk::Error::Unrepresentable.into())
}
fn request_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

impl RuntimeDatabase {
    pub fn offline_mappings(&mut self) -> Result<Vec<NetworkAreaMapping>, NetworkError> {
        // Seed only unmapped conferences. A renumber cannot repoint an issued wire key.
        self.connection.execute("INSERT OR IGNORE INTO network_area_mappings(profile,wire_number,conference_id) SELECT ?1,conference_number,conference_id FROM message_conferences",[PROFILE])?;
        let mut s=self.connection.prepare("SELECT mapping_id,conference_id,wire_number FROM network_area_mappings WHERE profile=?1 ORDER BY wire_number")?;
        let rows = s
            .query_map([PROFILE], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, u16>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, c, n)| {
                Ok(NetworkAreaMapping {
                    id,
                    conference: ConferenceId::new(c)?,
                    wire_number: n,
                })
            })
            .collect()
    }
    fn preserve_artifact(
        &mut self,
        store: &dyn NetworkArtifactStore,
        bytes: &[u8],
        now: i64,
    ) -> Result<String, NetworkError> {
        if bytes.len() > qwk::MAX_ARCHIVE {
            return Err(NetworkError::Capacity);
        }
        let id = qwk::digest(bytes);
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT state FROM network_artifacts WHERE artifact_id=?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?;
        match existing.as_deref() {
            Some("complete") => {
                tx.commit()?;
                store.preserve(bytes)?;
                return Ok(id);
            }
            Some(_) => return Err(NetworkError::Unavailable),
            None => {}
        }
        let (total, count) = store.usage()?;
        let reserved: i64 = tx.query_row(
            "SELECT COALESCE(SUM(byte_length),0) FROM network_artifacts WHERE state='pending'",
            [],
            |r| r.get(0),
        )?;
        if total
            .saturating_add(reserved as u64)
            .saturating_add(bytes.len() as u64)
            > 1024 * 1024 * 1024
            || count >= 10000
        {
            return Err(NetworkError::Capacity);
        }
        // Journal ownership before creating a host file. Recovery only removes
        // precisely these incomplete writes; unknown files are never collected.
        tx.execute("INSERT INTO network_artifacts(artifact_id,byte_length,created_at,state) VALUES(?1,?2,?3,'pending')",params![id,bytes.len() as i64,now])?;
        tx.commit()?;
        if store.preserve(bytes)? != id {
            return Err(NetworkError::Unavailable);
        }
        self.connection.execute("UPDATE network_artifacts SET state='complete' WHERE artifact_id=?1 AND state='pending'",[&id])?;
        Ok(id)
    }
    /// Bounded custody inventory for daemon recovery and cold backup validation.
    pub fn network_artifact_inventory(&self) -> Result<Vec<(String, u64, bool)>, NetworkError> {
        let mut statement = self.connection.prepare("SELECT artifact_id,byte_length,state='complete' FROM network_artifacts ORDER BY artifact_id LIMIT 10001")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    unsigned(r, 1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 10000
            || rows.iter().any(|(id, size, _)| {
                id.len() != 64
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    || *size > qwk::MAX_ARCHIVE as u64
            })
        {
            return Err(NetworkError::Capacity);
        }
        Ok(rows)
    }
    /// Called only by the daemon after removing an explicitly journaled pending
    /// file while holding exclusive board ownership. Foreign keys guard receipts.
    pub fn forget_incomplete_network_artifact(&mut self, id: &str) -> Result<(), NetworkError> {
        self.connection.execute(
            "DELETE FROM network_artifacts WHERE artifact_id=?1 AND state='pending'",
            [id],
        )?;
        Ok(())
    }
    pub fn recover_offline_requests(&mut self, now: i64) -> Result<(), NetworkError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute("UPDATE transfer_records SET state='failed',error_class='daemon-restart',state_version=state_version+1,updated_at=max(updated_at,?1),completed_at=max(updated_at,?1) WHERE purpose='message-packet' AND state='transferring'",[now])?;
        tx.execute(
            "UPDATE qwk_requests SET state='failed' WHERE state='prepared'",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn offline_pointer(
        &self,
        actor: MessageActor,
        c: ConferenceId,
    ) -> Result<(u64, u64, u64), NetworkError> {
        let pointer = self.last_read(actor, c)?;
        let versions=self.connection.query_row("SELECT pointer_version,reset_version FROM caller_last_read WHERE caller_id=?1 AND conference_id=?2",params![actor.caller_id().get(),c.get()],|r|Ok((unsigned(r,0)?,unsigned(r,1)?))).optional()?.unwrap_or((0,0));
        Ok((pointer, versions.0, versions.1))
    }
    pub fn reset_offline_pointer(
        &mut self,
        actor: MessageActor,
        c: ConferenceId,
        expected: u64,
        value: u64,
    ) -> Result<(), NetworkError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        crate::message::check_offline_conference(&tx, actor, c)?;
        let version:Option<i64>=tx.query_row("SELECT pointer_version FROM caller_last_read WHERE caller_id=?1 AND conference_id=?2",params![actor.caller_id().get(),c.get()],|r|r.get(0)).optional()?;
        if version.unwrap_or(0) != signed(expected)? {
            return Err(NetworkError::Stale);
        }
        let highest: i64 = tx.query_row(
            "SELECT COALESCE(MAX(message_number),0) FROM messages WHERE conference_id=?1",
            [c.get()],
            |r| r.get(0),
        )?;
        if signed(value)? > highest {
            return Err(NetworkError::Unavailable);
        }
        tx.execute("INSERT INTO caller_last_read(caller_id,conference_id,last_message_number,pointer_version,reset_version) VALUES(?1,?2,?3,1,1) ON CONFLICT(caller_id,conference_id) DO UPDATE SET last_message_number=excluded.last_message_number,pointer_version=pointer_version+1,reset_version=reset_version+1",params![actor.caller_id().get(),c.get(),signed(value)?])?;
        tx.commit()?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_offline_packet(
        &mut self,
        actor: MessageActor,
        board_id: &str,
        board_name: &str,
        selection: QwkSelection,
        scope: &[ConferenceId],
        timezone: Tz,
        store: &dyn NetworkArtifactStore,
        now: i64,
    ) -> Result<ExportReceipt, NetworkError> {
        let _permit = store.admit_import()?;
        if !qwk::valid_board_id(board_id) {
            return Err(NetworkError::Unavailable);
        }
        let mappings = self.offline_mappings()?;
        let snapshot = self.connection.unchecked_transaction()?;
        let caller = self
            .caller_by_id(actor.caller_id())?
            .ok_or(NetworkError::Unavailable)?;
        let caller_wire = wire_text(&caller.display_name)?;
        let conferences = self
            .conferences(actor)?
            .into_iter()
            .filter(|c| scope.contains(&c.id))
            .collect::<Vec<_>>();
        if conferences.is_empty() {
            return Err(NetworkError::Unavailable);
        }
        let mut messages = Vec::new();
        let mut native = Vec::new();
        let mut manifest = Vec::new();
        let mut labels = Vec::new();
        let mut examined = 0usize;
        for c in &conferences {
            let map = mappings
                .iter()
                .find(|m| m.conference == c.id)
                .ok_or(NetworkError::Unavailable)?;
            let pointer = self.offline_pointer(actor, c.id)?;
            labels.push((map.wire_number, wire_text(&c.name)?));
            let nums = {
                let mut s=self.connection.prepare("SELECT message_number FROM messages WHERE conference_id=?1 AND message_number>?2 AND lifecycle_state='active' ORDER BY message_number LIMIT 10001")?;
                let r = s
                    .query_map(
                        params![
                            c.id.get(),
                            if selection == QwkSelection::New {
                                signed(pointer.0)?
                            } else {
                                0
                            }
                        ],
                        |r| unsigned(r, 0),
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                r
            };
            examined += nums.len();
            if examined > 10000 {
                return Err(NetworkError::Capacity);
            }
            let mut high = pointer.0;
            for n in nums {
                let m = match self.message(actor, c.id, n) {
                    Ok(m) => m,
                    Err(MessageError::MessageAccessDenied) => continue,
                    Err(e) => return Err(e.into()),
                };
                if selection == QwkSelection::ToYou
                    && m.recipient_caller_id != Some(actor.caller_id())
                {
                    continue;
                }
                if messages.len() >= qwk::MAX_MESSAGES {
                    return Err(NetworkError::Capacity);
                }
                let timestamp = Utc
                    .timestamp_opt(m.created_at, 0)
                    .single()
                    .ok_or(qwk::Error::Date)?
                    .with_timezone(&timezone)
                    .naive_local();
                let from = wire_text(&m.author_name)?;
                let to = if m.recipient_caller_id.is_none() {
                    b"ALL".to_vec()
                } else {
                    wire_text(&m.recipient_name)?
                };
                let reference = if let Some(parent) = m.parent_message_id {
                    self.connection.query_row("SELECT message_number FROM messages WHERE message_id=?1 AND conference_id=?2",params![parent.get(),c.id.get()],|r|r.get::<_,u32>(0)).optional()?.unwrap_or(0)
                } else {
                    0
                };
                messages.push(qwk::Message {
                    number: u32::try_from(m.number).map_err(|_| qwk::Error::Unrepresentable)?,
                    conference: map.wire_number,
                    reference,
                    private: m.visibility == MessageVisibility::Private,
                    received: m.received,
                    to,
                    from,
                    subject: m.subject.clone(),
                    body: m.body.clone(),
                    wall_time: timestamp,
                });
                high = high.max(n);
                native.push(m);
            }
            manifest.push((map.id, c.id, policy_digest(c), high, pointer.1, pointer.2));
        }
        if messages.is_empty() {
            return Err(NetworkError::NoMessages);
        }
        let profile = Profile::ExtendedCp437;
        let (records, offsets) = qwk::encode_records(&messages, None, profile)?;
        let control = qwk::Control {
            board_id: board_id.to_owned(),
            board_name: wire_text(board_name)?,
            caller: caller_wire,
            created: Utc
                .timestamp_opt(now, 0)
                .single()
                .ok_or(qwk::Error::Date)?
                .with_timezone(&timezone)
                .naive_local(),
            conferences: labels,
        };
        let mut files=BTreeMap::from([("CONTROL.DAT".into(),qwk::control(&control,messages.len(),profile)?),("MESSAGES.DAT".into(),records),("DOOR.ID".into(),b"DOOR = SPITFIRE NG\r\nVERSION = 0.1.0\r\nSYSTEM = SPITFIRE NG\r\nCONTROLNAME = Lakota\r\nCONTROLTYPE = ADD\r\nCONTROLTYPE = DROP\r\nMIXEDCASE = YES\r\n".to_vec())]);
        let mut reader = Vec::new();
        reader.extend_from_slice(b"ALIAS ");
        reader.extend_from_slice(&control.caller);
        reader.extend_from_slice(b"\r\n");
        for ((number, _), c) in control.conferences.iter().zip(&conferences) {
            let privacy = if c.public_only { "O" } else { "X" };
            let mode = if selection == QwkSelection::ToYou {
                "p"
            } else {
                "a"
            };
            let posting = if c.allows_post(&caller, actor.sysop_security()) {
                ""
            } else {
                "R"
            };
            reader.extend_from_slice(
                format!("AREA {number} {mode}wLH{privacy}{posting}\r\n").as_bytes(),
            );
        }
        files.insert("TOREADER.EXT".into(), reader);
        let headers: Vec<_> = offsets
            .iter()
            .copied()
            .zip(messages.iter().map(|m| m.conference))
            .collect();
        for ((m, n), offset) in messages.iter().zip(&native).zip(offsets) {
            let index = qwk::index_record(offset, m.conference)?;
            files
                .entry(format!("{:03}.NDX", m.conference))
                .or_default()
                .extend(index);
            if n.recipient_caller_id == Some(actor.caller_id()) {
                files
                    .entry("PERSONAL.NDX".into())
                    .or_default()
                    .extend(index);
            }
        }
        qwk::validate_indexes(&files, &headers)?;
        let bytes = qwk::archive(&files)?;
        snapshot.commit()?;
        let artifact = self.preserve_artifact(store, &bytes, now)?;
        let request = request_id();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let requests: i64 = tx.query_row("SELECT COUNT(*) FROM qwk_requests", [], |r| r.get(0))?;
        let manifest_bytes:i64=tx.query_row("SELECT COALESCE(SUM(pgsize),0) FROM dbstat WHERE name LIKE 'qwk_%' OR name LIKE 'sqlite_autoindex_qwk_%'",[],|r|r.get(0))?;
        if requests >= 10000
            || manifest_bytes.saturating_add((native.len() + manifest.len() + 4) as i64 * 4096)
                > 256 * 1024 * 1024
        {
            return Err(NetworkError::Capacity);
        }
        tx.execute("INSERT INTO qwk_requests(request_id,caller_id,board_id,selection,artifact_id,state,created_at) VALUES(?1,?2,?3,?4,?5,'prepared',?6)",params![request,actor.caller_id().get(),board_id,if selection==QwkSelection::New{"new"}else{"to-you"},artifact,now])?;
        for (map, c, policy, high, pv, rv) in manifest {
            tx.execute(
                "INSERT INTO qwk_manifest_conferences VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    request,
                    map,
                    c.get(),
                    policy,
                    signed(high)?,
                    signed(pv)?,
                    signed(rv)?
                ],
            )?;
        }
        for (i, (m, wire)) in native.iter().zip(&messages).enumerate() {
            tx.execute(
                "INSERT INTO qwk_manifest_members VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    request,
                    i as i64,
                    m.id.get(),
                    signed(m.state_version)?,
                    m.conference_id.get(),
                    wire.conference,
                    signed(m.number)?
                ],
            )?;
        }
        tx.commit()?;
        self.revalidate_offline_packet(actor, &request)?;
        self.qwk_event(
            actor,
            "qwk.packet-generated",
            crate::EventOutcome::Succeeded,
            now,
        )?;
        Ok(ExportReceipt {
            request_id: request,
            artifact_id: artifact,
            bytes,
            count: messages.len(),
            board_id: board_id.to_owned(),
        })
    }
    pub fn revalidate_offline_packet(
        &self,
        actor: MessageActor,
        request: &str,
    ) -> Result<(), NetworkError> {
        let snapshot = self.connection.unchecked_transaction()?;
        check_manifest(&snapshot, actor, request)?;
        snapshot.commit()?;
        Ok(())
    }
    pub fn begin_packet_transfer(
        &mut self,
        actor: MessageActor,
        node: NodeId,
        session: i64,
        protocol: crate::TransferProtocol,
        download: Option<&ExportReceipt>,
        now: i64,
    ) -> Result<TransferId, NetworkError> {
        self.conferences(actor)?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(packet) = download {
            check_manifest(&tx, actor, &packet.request_id)?;
        }
        let id = TransferId::generated(session);
        tx.execute("INSERT INTO transfer_records(transfer_id,caller_id,node_id,direction,protocol,state,bytes_expected,started_at,updated_at,purpose,artifact_id) VALUES(?1,?2,?3,?4,?5,'transferring',?6,?7,?7,'message-packet',?8)",params![id.as_str(),actor.caller_id().get(),node.get(),if download.is_some(){"download"}else{"upload"},TransferMethod::Binary(protocol).database_value(),download.map_or(0,|p|p.bytes.len()as i64),now,download.map(|p|p.artifact_id.as_str())])?;
        if let Some(p) = download {
            tx.execute(
                "UPDATE qwk_requests SET transfer_id=?2 WHERE request_id=?1 AND state='prepared'",
                params![p.request_id, id.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }
    pub fn cancel_offline_packet(
        &mut self,
        actor: MessageActor,
        request: &str,
    ) -> Result<(), NetworkError> {
        self.connection.execute("UPDATE qwk_requests SET state='failed' WHERE request_id=?1 AND caller_id=?2 AND state='prepared'",params![request,actor.caller_id().get()])?;
        Ok(())
    }
    pub fn attach_reply_artifact(
        &mut self,
        actor: MessageActor,
        transfer: &TransferId,
        bytes: &[u8],
    ) -> Result<(), NetworkError> {
        self.connection.execute("UPDATE transfer_records SET artifact_id=?3 WHERE transfer_id=?1 AND caller_id=?2 AND purpose='message-packet' AND direction='upload' AND EXISTS(SELECT 1 FROM network_artifacts WHERE artifact_id=?3 AND state='complete')",params![transfer.as_str(),actor.caller_id().get(),qwk::digest(bytes)])?;
        Ok(())
    }
    pub fn finish_packet_transfer(
        &mut self,
        actor: MessageActor,
        id: &TransferId,
        success: bool,
        bytes: usize,
        now: i64,
    ) -> Result<(), NetworkError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let n=tx.execute("UPDATE transfer_records SET state=?3,bytes_transferred=?4,updated_at=?5,completed_at=?5,state_version=state_version+1 WHERE transfer_id=?1 AND caller_id=?2 AND purpose='message-packet' AND state='transferring'",params![id.as_str(),actor.caller_id().get(),if success{"completed"}else{"failed"},bytes as i64,now])?;
        if n != 1 {
            return Err(NetworkError::Stale);
        }
        tx.execute(
            "UPDATE qwk_requests SET state=?2 WHERE transfer_id=?1 AND state='prepared'",
            params![id.as_str(), if success { "delivered" } else { "failed" }],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn confirm_offline_delivery(
        &mut self,
        actor: MessageActor,
        request: &str,
        update: bool,
    ) -> Result<(), NetworkError> {
        self.revalidate_offline_packet(actor, request)?;
        let state: String = self.connection.query_row(
            "SELECT state FROM qwk_requests WHERE request_id=?1",
            [request],
            |r| r.get(0),
        )?;
        if state != "delivered" {
            return Err(NetworkError::Stale);
        }
        let rows = {
            let mut s=self.connection.prepare("SELECT conference_id,high_water,pointer_version,reset_version FROM qwk_manifest_conferences WHERE request_id=?1")?;
            let rows = s
                .query_map([request], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        unsigned(r, 1)?,
                        unsigned(r, 2)?,
                        unsigned(r, 3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        if update {
            for (c, _, _, rv) in &rows {
                let p = self.offline_pointer(actor, ConferenceId::new(*c)?)?;
                if p.2 != *rv {
                    self.connection.execute("UPDATE qwk_manifest_conferences SET pointer_version=?3,reset_version=?4 WHERE request_id=?1 AND conference_id=?2",params![request,c,signed(p.1)?,signed(p.2)?])?;
                    return Err(NetworkError::Stale);
                }
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        check_manifest(&tx, actor, request)?;
        if update {
            for (c, high, _, rv) in rows {
                let actual:i64=tx.query_row("SELECT COALESCE((SELECT reset_version FROM caller_last_read WHERE caller_id=?1 AND conference_id=?2),0)",params![actor.caller_id().get(),c],|r|r.get(0))?;
                if actual as u64 != rv {
                    return Err(NetworkError::Stale);
                }
                tx.execute("INSERT INTO caller_last_read(caller_id,conference_id,last_message_number) VALUES(?1,?2,?3) ON CONFLICT(caller_id,conference_id) DO UPDATE SET last_message_number=max(last_message_number,excluded.last_message_number)",params![actor.caller_id().get(),c,signed(high)?])?;
            }
            tx.execute("INSERT OR IGNORE INTO caller_message_receipts(caller_id,message_id) SELECT ?2,m.message_id FROM qwk_manifest_members m JOIN message_delivery_recipients r ON r.message_id=m.message_id WHERE m.request_id=?1 AND r.caller_id=?2",params![request,actor.caller_id().get()])?;
        }
        tx.execute(
            "UPDATE qwk_requests SET state=?2 WHERE request_id=?1 AND state='delivered'",
            params![request, if update { "confirmed" } else { "preview" }],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn qwk_event(
        &mut self,
        actor: MessageActor,
        code: &str,
        outcome: crate::EventOutcome,
        now: i64,
    ) -> Result<(), NetworkError> {
        let mut event = crate::NewOperationalEvent::new(
            now,
            crate::EventCategory::Message,
            crate::EventSeverity::Info,
            format!("message.{code}"),
            outcome,
        );
        event.caller_id = Some(actor.caller_id());
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        crate::insert_operational_event_tx(&tx, &event)?;
        tx.commit()?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SubmissionIntent {
    Retry,
    /// Caller explicitly confirms a new submission. The stable intent token must
    /// be reused for recovery; it is not supplied by the untrusted packet.
    New(String),
}
impl RuntimeDatabase {
    #[allow(clippy::too_many_arguments)]
    pub fn import_offline_replies(
        &mut self,
        actor: MessageActor,
        board_id: &str,
        bytes: &[u8],
        store: &dyn NetworkArtifactStore,
        intent: &SubmissionIntent,
        now: i64,
    ) -> Result<ImportSummary, NetworkError> {
        let _permit = store.admit_import()?;
        self.conferences(actor)?;
        let artifact = qwk::inspect(bytes)?;
        let name = format!("{board_id}.MSG");
        if artifact
            .members
            .keys()
            .any(|n| n != &name && n != &format!("{board_id}.LMR"))
        {
            return Err(qwk::Error::Unsupported.into());
        }
        // LMR remains evidence-qualified: retain the artifact, do not invent pointers.
        let members = qwk::decode_records(
            artifact.members.get(&name).ok_or(qwk::Error::Malformed)?,
            Some(board_id),
            Profile::ExtendedCp437,
        )?;
        let artifact_id = self.preserve_artifact(store, bytes, now)?;
        if artifact.members.contains_key(&format!("{board_id}.LMR")) {
            return Err(qwk::Error::Unsupported.into());
        }
        let receipt_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM network_import_receipts", [], |r| {
                    r.get(0)
                })?;
        let receipt_bytes: i64 = self.connection.query_row("SELECT COALESCE(SUM(pgsize),0) FROM dbstat WHERE name='network_import_receipts' OR name='network_import_content' OR name LIKE 'sqlite_autoindex_network_import_receipts_%'", [], |r|r.get(0))?;
        // Reserve conservative per-member space for the table and both indexes.
        if receipt_count + (qwk::MAX_MESSAGES * 2) as i64 > 2_000_000
            || receipt_bytes.saturating_add((qwk::MAX_MESSAGES * 2) as i64 * 4096)
                > 512 * 1024 * 1024
        {
            return Err(NetworkError::Capacity);
        }
        let packet = match intent {
            SubmissionIntent::Retry => artifact.digest,
            SubmissionIntent::New(token) => {
                if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(NetworkError::Unavailable);
                }
                qwk::digest(format!("{}:{token}", artifact.digest).as_bytes())
            }
        };
        let mappings = self.offline_mappings()?;
        let mut summary = ImportSummary::default();
        for member in members {
            let previous:Option<String>=self.connection.query_row("SELECT outcome FROM network_import_receipts WHERE caller_id=?1 AND profile=?2 AND packet_digest=?3 AND ordinal=?4",params![actor.caller_id().get(),PROFILE,packet,member.ordinal as i64],|r|r.get(0)).optional()?;
            if let Some(outcome) = previous {
                if outcome == "imported" || outcome == "control" {
                    summary.duplicates += 1
                } else {
                    summary.rejected += 1
                }
                continue;
            }
            let receipt = ImportReceipt {
                caller: actor.caller_id(),
                packet: packet.clone(),
                ordinal: member.ordinal,
                member: member.digest,
                artifact: artifact_id.clone(),
                offset: member.offset,
                wall: member.message.wall_time.to_string(),
                received: now,
            };
            let same:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM network_import_receipts WHERE caller_id=?1 AND member_digest=?2 AND packet_digest<>?3 AND outcome='imported')",params![actor.caller_id().get(),receipt.member,packet],|r|r.get(0))?;
            if same && matches!(intent, SubmissionIntent::Retry) {
                self.reject_offline_member(
                    &receipt,
                    "possible-duplicate",
                    "review-new-submission",
                )?;
                summary.possible_duplicates += 1;
                continue;
            }
            match self.import_offline_member(actor, &member.message, &mappings, board_id, &receipt)
            {
                Ok(true) => summary.imported += 1,
                Ok(false) => summary.controls += 1,
                Err(NetworkError::Message(MessageError::ImportAlreadyRecorded)) => {
                    summary.duplicates += 1
                }
                Err(NetworkError::Message(e)) => {
                    if matches!(
                        e,
                        MessageError::Sqlite(_)
                            | MessageError::Database(_)
                            | MessageError::MutationInvariant
                    ) {
                        return Err(e.into());
                    }
                    self.reject_offline_member(&receipt, "rejected", "native-policy")?;
                    summary.rejected += 1;
                }
                Err(NetworkError::Unavailable | NetworkError::Stale) => {
                    self.reject_offline_member(
                        &receipt,
                        "rejected",
                        "mapping-recipient-reference",
                    )?;
                    summary.rejected += 1;
                }
                Err(e) => return Err(e),
            }
        }
        self.qwk_event(
            actor,
            "qwk.replies-imported",
            crate::EventOutcome::Succeeded,
            now,
        )?;
        if summary.duplicates > 0 {
            self.qwk_event(
                actor,
                "qwk.duplicate-suppressed",
                crate::EventOutcome::Succeeded,
                now,
            )?;
        }
        Ok(summary)
    }
    fn reject_offline_member(
        &mut self,
        receipt: &ImportReceipt,
        outcome: &str,
        reason: &str,
    ) -> Result<(), NetworkError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM network_import_receipts WHERE caller_id=?1 AND packet_digest=?2 AND ordinal=?3)",params![receipt.caller.get(),receipt.packet,receipt.ordinal as i64],|r|r.get(0))?;
        if !exists {
            receipt.insert(&tx, None, outcome, reason)?;
        }
        tx.commit()?;
        Ok(())
    }
    fn import_offline_member(
        &mut self,
        actor: MessageActor,
        wire: &qwk::Message,
        mappings: &[NetworkAreaMapping],
        board: &str,
        receipt: &ImportReceipt,
    ) -> Result<bool, NetworkError> {
        let mapping = mappings
            .iter()
            .find(|m| m.wire_number == wire.conference)
            .ok_or(NetworkError::Unavailable)?;
        let c = self
            .conferences(actor)?
            .into_iter()
            .find(|c| c.id == mapping.conference)
            .ok_or(NetworkError::Unavailable)?;
        let recipient = crate::file_maintenance::decode_cp437(&wire.to);
        if recipient.eq_ignore_ascii_case("Lakota") && self.recipient(b"Lakota").is_err() {
            // A real local handle wins; never silently shadow that caller with a command.
            let body = wire.body.strip_suffix(b"\n").unwrap_or(&wire.body);
            let command = if wire.subject == b"ADD" || wire.subject == b"DROP" {
                wire.subject.as_slice()
            } else {
                body
            };
            if !matches!(command, b"ADD" | b"DROP") {
                return Err(NetworkError::Unavailable);
            }
            self.apply_offline_queue_control(actor, c.id, command == b"ADD", receipt)?;
            return Ok(false);
        }
        if wire.body.starts_with(b"->") {
            return Err(NetworkError::Unavailable);
        }
        let target = if recipient.eq_ignore_ascii_case("ALL")
            || recipient.eq_ignore_ascii_case("All Callers")
        {
            None
        } else {
            Some(self.recipient(recipient.as_bytes())?)
        };
        let mut parent = None;
        if wire.reference > 0 {
            let candidates = {
                let mut s=self.connection.prepare("SELECT DISTINCT m.message_id FROM qwk_manifest_members m JOIN qwk_requests r ON r.request_id=m.request_id WHERE r.caller_id=?1 AND r.board_id=?2 AND r.state IN ('delivered','confirmed','preview') AND m.wire_number=?3 AND m.message_number=?4 LIMIT 2")?;
                let v = s
                    .query_map(
                        params![
                            actor.caller_id().get(),
                            board,
                            wire.conference,
                            wire.reference
                        ],
                        |r| r.get::<_, i64>(0),
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                v
            };
            if candidates.len() > 1 {
                return Err(NetworkError::Stale);
            }
            if let Some(id) = candidates.first() {
                if let Ok(m) = self.message(actor, c.id, u64::from(wire.reference)) {
                    if m.id.get() == *id && m.lifecycle == crate::MessageLifecycle::Active {
                        parent = Some(m.id)
                    }
                }
            }
        }
        // Original wall time remains in provenance. Native creation/sort time is the
        // local receipt instant; a reader timestamp without offset never claims UTC.
        let message = NewMessage {
            conference_id: c.id,
            recipient_caller_id: target.as_ref().map(|r| r.caller_id),
            recipient_name: target.map_or_else(|| "All Callers".into(), |r| r.display_name),
            subject: wire.subject.clone(),
            body: wire.body.clone(),
            created_at: receipt.received,
            parent_message_id: parent,
            visibility: if wire.private {
                MessageVisibility::Private
            } else {
                MessageVisibility::Public
            },
            kind: MessageKind::Standard,
        };
        self.post_message_fanout(actor, message, &[], Some(receipt))?;
        Ok(true)
    }
}

fn signed(n: u64) -> Result<i64, NetworkError> {
    i64::try_from(n).map_err(|_| NetworkError::Capacity)
}
fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let n: i64 = row.get(index)?;
    u64::try_from(n).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, n))
}

fn check_manifest(
    conn: &rusqlite::Connection,
    actor: MessageActor,
    request: &str,
) -> Result<(), NetworkError> {
    let owner:Option<i64>=conn.query_row("SELECT caller_id FROM qwk_requests WHERE request_id=?1 AND state IN ('prepared','delivered')",[request],|r|r.get(0)).optional()?;
    if owner != Some(actor.caller_id().get()) {
        return Err(NetworkError::Stale);
    }
    let mut s = conn.prepare(
        "SELECT conference_id,policy_digest FROM qwk_manifest_conferences WHERE request_id=?1",
    )?;
    for row in s.query_map([request], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })? {
        let (c, d) = row?;
        let c = crate::message::check_offline_conference(conn, actor, ConferenceId::new(c)?)?;
        if policy_digest(&c) != d {
            return Err(NetworkError::Stale);
        }
    }
    let mut s=conn.prepare("SELECT message_id,state_version,conference_id,message_number FROM qwk_manifest_members WHERE request_id=?1")?;
    for row in s.query_map([request], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            unsigned(r, 1)?,
            r.get::<_, i64>(2)?,
            unsigned(r, 3)?,
        ))
    })? {
        let (id, v, c, n) = row?;
        crate::message::check_offline_message(
            conn,
            actor,
            ConferenceId::new(c)?,
            n,
            MessageId::new(id)?,
            v,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BoardIdentity, CallerState, ConferenceAccessMode, ConferenceDefinition, CredentialHasher,
        PasswordHashConfig, SecurityLevel,
    };
    use std::{io::Write, sync::Mutex};
    #[derive(Default)]
    struct MemoryArtifactStore(Mutex<BTreeMap<String, Vec<u8>>>, ImportCapacity);
    impl NetworkArtifactStore for MemoryArtifactStore {
        fn admit_import(&self) -> Result<ImportPermit<'_>, NetworkError> {
            self.1.acquire()
        }
        fn preserve(&self, b: &[u8]) -> Result<String, NetworkError> {
            let id = qwk::digest(b);
            self.0.lock().unwrap().insert(id.clone(), b.to_vec());
            Ok(id)
        }
        fn usage(&self) -> Result<(u64, usize), NetworkError> {
            let s = self.0.lock().unwrap();
            Ok((s.values().map(|b| b.len() as u64).sum(), s.len()))
        }
    }
    const NOW: i64 = 1_788_627_600;
    struct Fixture {
        _temp: tempfile::TempDir,
        db: RuntimeDatabase,
        store: MemoryArtifactStore,
        alice: MessageActor,
        bob: MessageActor,
        other: MessageActor,
        c: ConferenceId,
    }
    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let store = MemoryArtifactStore::default();
        let mut db = RuntimeDatabase::open(&temp.path().join("db.sqlite3")).unwrap();
        db.migrate().unwrap();
        db.ensure_board_identity(&BoardIdentity::new("Synthetic QWK Board", "Sysop").unwrap())
            .unwrap();
        let hash = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
        .hash(b"synthetic test password")
        .unwrap();
        let mut actors = Vec::new();
        for name in ["ALICE", "BOB", "OTHER"] {
            let c = db
                .create_caller(
                    name.as_bytes(),
                    &hash,
                    SecurityLevel::new(10).unwrap(),
                    CallerState::Active,
                    false,
                    NOW,
                )
                .unwrap();
            actors.push(MessageActor::new(c.id, SecurityLevel::new(100).unwrap()));
        }
        let c = db
            .ensure_conference(&ConferenceDefinition {
                number: 1,
                name: "General".into(),
                description: "Synthetic".into(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: false,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![],
            })
            .unwrap();
        Fixture {
            _temp: temp,
            db,
            store,
            alice: actors[0],
            bob: actors[1],
            other: actors[2],
            c: c.id,
        }
    }
    fn post(f: &mut Fixture, private: bool) -> crate::Message {
        f.db.post(
            f.bob,
            NewMessage {
                conference_id: f.c,
                recipient_caller_id: private.then_some(f.alice.caller_id()),
                recipient_name: if private { "ALICE" } else { "All Callers" }.into(),
                subject: b"Synthetic subject".to_vec(),
                body: b"Caf\x82 \xb3\r\n".to_vec(),
                created_at: NOW,
                parent_message_id: None,
                visibility: if private {
                    MessageVisibility::Private
                } else {
                    MessageVisibility::Public
                },
                kind: MessageKind::Standard,
            },
        )
        .unwrap()
    }
    fn packet(f: &mut Fixture, actor: MessageActor) -> ExportReceipt {
        f.db.prepare_offline_packet(
            actor,
            "TEST",
            "Synthetic",
            QwkSelection::New,
            &[f.c],
            chrono_tz::America::Phoenix,
            &f.store,
            NOW,
        )
        .unwrap()
    }
    fn deliver(f: &mut Fixture, actor: MessageActor, p: &ExportReceipt, success: bool) {
        let id =
            f.db.begin_packet_transfer(
                actor,
                NodeId::new(1).unwrap(),
                1,
                crate::TransferProtocol::YmodemBatch,
                Some(p),
                NOW,
            )
            .unwrap();
        f.db.finish_packet_transfer(
            actor,
            &id,
            success,
            if success { p.bytes.len() } else { 0 },
            NOW,
        )
        .unwrap();
    }
    fn reply(subject: &[u8]) -> qwk::Message {
        qwk::Message {
            number: 1,
            conference: 1,
            reference: 0,
            private: false,
            received: false,
            to: b"ALL".to_vec(),
            from: b"SYSOP SPOOF".to_vec(),
            subject: subject.to_vec(),
            body: b"Synthetic offline reply\n".to_vec(),
            wall_time: Utc.timestamp_opt(NOW - 3600, 0).unwrap().naive_utc(),
        }
    }
    fn rep(messages: &[qwk::Message]) -> Vec<u8> {
        let (b, _) = qwk::encode_records(messages, Some("TEST"), Profile::ExtendedCp437).unwrap();
        qwk::archive(&BTreeMap::from([("TEST.MSG".into(), b)])).unwrap()
    }
    #[test]
    fn export_privacy_delivery_confirmation_and_retry() {
        let mut f = fixture();
        let public = post(&mut f, false);
        let private = post(&mut f, true);
        let alice = f.alice;
        let other = f.other;
        let p = packet(&mut f, alice);
        assert_eq!(p.count, 2);
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), 0);
        assert!(!f.db.received(alice, f.c, private.number).unwrap());
        let outsider = packet(&mut f, other);
        assert_eq!(outsider.count, 1);
        deliver(&mut f, alice, &p, false);
        assert!(f
            .db
            .confirm_offline_delivery(alice, &p.request_id, true)
            .is_err());
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), 0);
        let retry = packet(&mut f, alice);
        deliver(&mut f, alice, &retry, true);
        f.db.confirm_offline_delivery(alice, &retry.request_id, true)
            .unwrap();
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), private.number);
        assert!(f.db.received(alice, f.c, private.number).unwrap());
        assert!(f.db.message(other, f.c, public.number).is_ok());
        assert!(matches!(
            f.db.prepare_offline_packet(
                alice,
                "TEST",
                "Synthetic",
                QwkSelection::New,
                &[f.c],
                chrono_tz::UTC,
                &f.store,
                NOW
            ),
            Err(NetworkError::NoMessages)
        ));
        assert!(f
            .db
            .prepare_offline_packet(
                alice,
                "TEST",
                "Synthetic",
                QwkSelection::ToYou,
                &[f.c],
                chrono_tz::UTC,
                &f.store,
                NOW
            )
            .is_ok());
    }
    #[test]
    fn replies_native_author_permissions_and_durable_replay() {
        let mut f = fixture();
        let bytes = rep(&[reply(b"One")]);
        let result =
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &bytes,
                &f.store,
                &SubmissionIntent::Retry,
                NOW,
            )
            .unwrap();
        assert_eq!(result.imported, 1);
        let m = f.db.message(f.alice, f.c, 1).unwrap();
        assert_eq!(m.author_caller_id, Some(f.alice.caller_id()));
        assert_eq!(m.author_name, "ALICE");
        assert_eq!(m.created_at, NOW);
        let path = f.db.path().to_path_buf();
        drop(f.db);
        f.db = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &bytes,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .duplicates,
            1
        );
        assert_eq!(f.db.messages(f.alice, f.c).unwrap().len(), 1);
        let mut bad = reply(b"Bad area");
        bad.conference = 65535;
        let mixed = rep(&[reply(b"Two"), bad]);
        let r =
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &mixed,
                &f.store,
                &SubmissionIntent::Retry,
                NOW,
            )
            .unwrap();
        assert_eq!((r.imported, r.rejected), (1, 1));
        let r =
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &mixed,
                &f.store,
                &SubmissionIntent::Retry,
                NOW,
            )
            .unwrap();
        assert_eq!((r.duplicates, r.rejected), (1, 1));
    }
    #[test]
    fn packet_recompression_and_possible_duplicate_review() {
        let mut f = fixture();
        let one = reply(b"Repeat");
        let a = rep(std::slice::from_ref(&one));
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &a,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .imported,
            1
        );
        let data = qwk::inspect(&a).unwrap();
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file(
            "test.msg",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        w.write_all(&data.members["TEST.MSG"]).unwrap();
        let b = w.finish().unwrap().into_inner();
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &b,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .duplicates,
            1
        );
        let c = rep(&[one, reply(b"Another")]);
        let r = f
            .db
            .import_offline_replies(f.alice, "TEST", &c, &f.store, &SubmissionIntent::Retry, NOW)
            .unwrap();
        assert_eq!((r.possible_duplicates, r.imported), (1, 1));
        let intent = SubmissionIntent::New("a".repeat(32));
        assert_eq!(
            f.db.import_offline_replies(f.alice, "TEST", &a, &f.store, &intent, NOW)
                .unwrap()
                .imported,
            1
        );
        assert_eq!(
            f.db.import_offline_replies(f.alice, "TEST", &a, &f.store, &intent, NOW)
                .unwrap()
                .duplicates,
            1
        );
    }
    #[test]
    fn restart_fails_unfinished_packet_transfers_without_pointer_movement() {
        let mut f = fixture();
        post(&mut f, false);
        let alice = f.alice;
        let packet = packet(&mut f, alice);
        let id =
            f.db.begin_packet_transfer(
                alice,
                NodeId::new(1).unwrap(),
                1,
                crate::TransferProtocol::YmodemBatch,
                Some(&packet),
                NOW,
            )
            .unwrap();
        assert!(!f.db.transfer_operations_ready_for_cold_backup().unwrap());
        f.db = RuntimeDatabase::open(&f._temp.path().join("db.sqlite3")).unwrap();
        f.db.recover_offline_requests(NOW + 1).unwrap();
        f.db.recover_offline_requests(NOW + 2).unwrap();
        assert!(f.db.transfer_operations_ready_for_cold_backup().unwrap());
        let state: String =
            f.db.connection
                .query_row(
                    "SELECT state FROM transfer_records WHERE transfer_id=?1",
                    [id.as_str()],
                    |r| r.get(0),
                )
                .unwrap();
        assert_eq!(state, "failed");
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), 0);
        assert!(f
            .db
            .confirm_offline_delivery(alice, &packet.request_id, true)
            .is_err());
    }
    #[test]
    fn exhausted_manifest_budget_holds_export_without_advancing() {
        let mut f = fixture();
        post(&mut f, false);
        f.db.connection.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO qwk_requests(request_id,caller_id,board_id,selection,state,created_at) SELECT CAST(x AS TEXT),?1,'TEST','new','failed',1 FROM n",[f.alice.caller_id().get()]).unwrap();
        assert!(matches!(
            f.db.prepare_offline_packet(
                f.alice,
                "TEST",
                "Synthetic",
                QwkSelection::New,
                &[f.c],
                chrono_tz::UTC,
                &f.store,
                NOW
            ),
            Err(NetworkError::Capacity)
        ));
        assert_eq!(f.db.last_read(f.alice, f.c).unwrap(), 0);
    }
    #[test]
    fn declared_queue_controls_are_receipted_without_posting() {
        let mut f = fixture();
        let mut control = reply(b"ADD");
        control.to = b"Lakota".to_vec();
        let packet = rep(std::slice::from_ref(&control));
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &packet,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .controls,
            1
        );
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &packet,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .duplicates,
            1
        );
        assert!(f.db.messages(f.alice, f.c).unwrap().is_empty());
        control.subject = b"DROP".to_vec();
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &rep(&[control]),
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .rejected,
            1
        );
        assert_eq!(f.db.queued_conferences(f.alice).unwrap()[0].number, 1);
    }
    #[test]
    fn mapping_keeps_issued_wire_number_and_unsupported_lmr_is_retained() {
        let mut f = fixture();
        let map = f.db.offline_mappings().unwrap()[0].clone();
        f.db.connection
            .execute(
                "UPDATE message_conferences SET conference_number=2 WHERE conference_id=?1",
                [f.c.get()],
            )
            .unwrap();
        let after = f.db.offline_mappings().unwrap();
        assert_eq!(after[0].id, map.id);
        assert_eq!(after[0].wire_number, 1);
        let (records, _) = qwk::encode_records(
            &[reply(b"Not posted")],
            Some("TEST"),
            Profile::ExtendedCp437,
        )
        .unwrap();
        let packet = qwk::archive(&BTreeMap::from([
            ("TEST.MSG".into(), records),
            ("TEST.LMR".into(), b"unknown bytes".to_vec()),
        ]))
        .unwrap();
        assert!(matches!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &packet,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            ),
            Err(NetworkError::Codec(qwk::Error::Unsupported))
        ));
        assert_eq!(f.db.network_artifact_inventory().unwrap().len(), 1);
        assert!(f.db.messages(f.alice, f.c).unwrap().is_empty());
    }
    #[test]
    fn imported_private_mail_and_recipient_authority() {
        let mut f = fixture();
        let mut m = reply(b"Private");
        m.private = true;
        m.to = b"BOB".to_vec();
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &rep(&[m]),
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .imported,
            1
        );
        assert!(f.db.message(f.bob, f.c, 1).is_ok());
        assert!(f.db.message(f.other, f.c, 1).is_err());
        let mut m = reply(b"Invalid private ALL");
        m.private = true;
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &rep(&[m]),
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .rejected,
            1
        );
    }
    #[test]
    fn pointer_reset_and_overlapping_packets_cannot_reverse_state() {
        let mut f = fixture();
        post(&mut f, false);
        let alice = f.alice;
        let p = packet(&mut f, alice);
        let second = packet(&mut f, alice);
        deliver(&mut f, alice, &p, true);
        deliver(&mut f, alice, &second, true);
        f.db.confirm_offline_delivery(alice, &p.request_id, true)
            .unwrap();
        let (_, version, _) = f.db.offline_pointer(alice, f.c).unwrap();
        f.db.reset_offline_pointer(alice, f.c, version, 0).unwrap();
        assert!(matches!(
            f.db.confirm_offline_delivery(alice, &second.request_id, true),
            Err(NetworkError::Stale)
        ));
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), 0);
        f.db.confirm_offline_delivery(alice, &second.request_id, true)
            .unwrap();
        assert_eq!(f.db.last_read(alice, f.c).unwrap(), 1);
        assert!(f.db.reset_offline_pointer(alice, f.c, version, 0).is_err());
    }
    #[test]
    fn deletion_and_policy_change_make_packet_stale() {
        let mut f = fixture();
        let m = post(&mut f, false);
        let alice = f.alice;
        let p = packet(&mut f, alice);
        f.db.delete_message(f.bob, f.c, m.number, m.state_version)
            .unwrap();
        assert!(f
            .db
            .revalidate_offline_packet(alice, &p.request_id)
            .is_err());
    }
    #[test]
    fn malformed_packet_has_no_mutations_and_receipt_failure_rolls_back() {
        let mut f = fixture();
        let bytes = rep(&[reply(b"Atomic")]);
        assert!(f
            .db
            .import_offline_replies(
                f.alice,
                "TEST",
                &bytes[..bytes.len() - 1],
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .is_err());
        f.db.connection.execute_batch("CREATE TRIGGER fail_receipt BEFORE INSERT ON network_import_receipts BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(f
            .db
            .import_offline_replies(
                f.alice,
                "TEST",
                &bytes,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .is_err());
        assert_eq!(f.db.messages(f.alice, f.c).unwrap().len(), 0);
        assert_eq!(
            f.db.caller_by_id(f.alice.caller_id())
                .unwrap()
                .unwrap()
                .messages_posted,
            0
        );
        f.db.connection
            .execute_batch("DROP TRIGGER fail_receipt")
            .unwrap();
        assert_eq!(
            f.db.import_offline_replies(
                f.alice,
                "TEST",
                &bytes,
                &f.store,
                &SubmissionIntent::Retry,
                NOW
            )
            .unwrap()
            .imported,
            1
        );
    }
    #[test]
    fn offline_events_are_content_free_and_backup_keeps_receipts() {
        let mut f = fixture();
        let b = rep(&[reply(b"PRIVATE SUBJECT SENTINEL")]);
        f.db.import_offline_replies(f.alice, "TEST", &b, &f.store, &SubmissionIntent::Retry, NOW)
            .unwrap();
        let snapshot = f._temp.path().join("snapshot.sqlite3");
        f.db.backup_to(&snapshot).unwrap();
        let mut restored = RuntimeDatabase::open(&snapshot).unwrap();
        assert_eq!(
            restored
                .import_offline_replies(
                    f.alice,
                    "TEST",
                    &b,
                    &f.store,
                    &SubmissionIntent::Retry,
                    NOW
                )
                .unwrap()
                .duplicates,
            1
        );
        let count: i64 = restored
            .connection
            .query_row(
                "SELECT COUNT(*) FROM operational_events WHERE event_code LIKE 'message.qwk.%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count > 0);
    }
}
