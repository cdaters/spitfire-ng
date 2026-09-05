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

//! Native private QWK mailbox and transit delivery authority.
use super::*;

pub(crate) const MIGRATION: &str = r#"
CREATE TABLE messages_n22 (
            message_id INTEGER PRIMARY KEY,
            fanout_id INTEGER NOT NULL REFERENCES message_fanouts(fanout_id) ON DELETE RESTRICT,
            conference_id INTEGER REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
            message_number INTEGER CHECK (message_number > 0),
            author_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            author_name TEXT NOT NULL CHECK (length(author_name) BETWEEN 1 AND 60),
            created_at INTEGER NOT NULL,
            placed_at INTEGER NOT NULL,
            parent_message_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
            audience_kind TEXT NOT NULL CHECK (audience_kind IN ('all-callers', 'local-recipient', 'external-recipient')),
            visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
            lifecycle_state TEXT NOT NULL CHECK (lifecycle_state IN ('active', 'deleted')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            delivery_role TEXT NOT NULL CHECK (delivery_role IN ('single', 'primary', 'cc')),
            delivery_ordinal INTEGER NOT NULL CHECK (delivery_ordinal BETWEEN 0 AND 9),
            primary_delivery_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
 origin_kind TEXT NOT NULL DEFAULT 'native' CHECK(origin_kind IN ('native','external-network')),
 container_kind TEXT NOT NULL DEFAULT 'conference' CHECK(container_kind IN ('conference','local-network-mailbox','network-transit')),
            UNIQUE (conference_id, message_number),
            UNIQUE (fanout_id, delivery_ordinal),
            UNIQUE (message_id, fanout_id),
            CHECK (
                (delivery_role = 'single' AND delivery_ordinal = 0 AND primary_delivery_id IS NULL)
                OR (delivery_role = 'primary' AND delivery_ordinal = 0 AND primary_delivery_id = message_id)
                OR (delivery_role = 'cc' AND delivery_ordinal BETWEEN 1 AND 9 AND primary_delivery_id IS NOT NULL)
            ),
            CHECK (visibility = 'public' OR audience_kind IN ('local-recipient','external-recipient')),

 CHECK ((container_kind='conference' AND conference_id IS NOT NULL AND message_number IS NOT NULL AND audience_kind<>'external-recipient') OR (container_kind='local-network-mailbox' AND conference_id IS NULL AND message_number IS NULL AND visibility='private' AND audience_kind IN ('local-recipient','external-recipient')) OR (container_kind='network-transit' AND conference_id IS NULL AND message_number IS NULL AND visibility='private' AND audience_kind='external-recipient' AND author_caller_id IS NULL AND origin_kind='external-network'))
        );
INSERT INTO messages_n22(message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,state_version,delivery_role,delivery_ordinal,primary_delivery_id,origin_kind) SELECT message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,state_version,delivery_role,delivery_ordinal,primary_delivery_id,origin_kind FROM messages;
DROP TABLE messages;
ALTER TABLE messages_n22 RENAME TO messages;
CREATE INDEX messages_conference_scan ON messages(conference_id,message_number,lifecycle_state);
CREATE INDEX messages_author_scan ON messages(author_caller_id,visibility,lifecycle_state);
CREATE TRIGGER network_external_author_update BEFORE UPDATE OF origin_kind,author_caller_id ON messages WHEN NEW.origin_kind='external-network' AND NEW.author_caller_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'network author is not a caller'); END;
CREATE TRIGGER network_external_author BEFORE INSERT ON messages WHEN NEW.origin_kind='external-network' AND NEW.author_caller_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'network author is not a caller'); END;
CREATE TABLE qwk_private_policy (
 link_id TEXT PRIMARY KEY REFERENCES qwk_links(link_id), enabled INTEGER NOT NULL CHECK(enabled IN(0,1)), inbound INTEGER NOT NULL CHECK(inbound IN(0,1)), outbound INTEGER NOT NULL CHECK(outbound IN(0,1)), transit INTEGER NOT NULL CHECK(transit IN(0,1)), version INTEGER NOT NULL CHECK(version>0)
);
CREATE TABLE qwk_mailbox_aliases (
 link_id TEXT NOT NULL REFERENCES qwk_private_policy(link_id), alias TEXT NOT NULL COLLATE NOCASE CHECK(length(alias) BETWEEN 1 AND 40), caller_id INTEGER NOT NULL REFERENCES callers(caller_id), PRIMARY KEY(link_id,alias), UNIQUE(link_id,caller_id)
);
CREATE TABLE qwk_private_routes (
 network TEXT NOT NULL, destination TEXT NOT NULL, link_id TEXT NOT NULL REFERENCES qwk_private_policy(link_id), PRIMARY KEY(network,destination)
);
CREATE TABLE network_private_envelopes (
 message_id INTEGER PRIMARY KEY REFERENCES messages(message_id), network TEXT NOT NULL, origin_system TEXT NOT NULL, destination_system TEXT NOT NULL, recipient TEXT NOT NULL COLLATE NOCASE, next_link TEXT REFERENCES qwk_links(link_id), policy_version INTEGER, ingress_link TEXT REFERENCES qwk_links(link_id), CHECK((next_link IS NULL)=(policy_version IS NULL))
);
CREATE TRIGGER network_private_envelope_immutable BEFORE UPDATE ON network_private_envelopes BEGIN SELECT RAISE(ABORT,'immutable private envelope'); END;
CREATE TRIGGER network_private_envelope_retained BEFORE DELETE ON network_private_envelopes BEGIN SELECT RAISE(ABORT,'retained private envelope'); END;
"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxAlias {
    pub alias: String,
    pub caller_id: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailPolicy {
    pub link: String,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub transit: bool,
    pub version: i64,
    pub aliases: Vec<MailboxAlias>,
    pub destinations: Vec<String>,
}
/// Caller-authenticated native input, never an operator request impersonating a caller.
pub struct NewNetworkMail {
    pub network: String,
    pub destination: String,
    pub recipient: String,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    pub reply_to: Option<crate::MessageId>,
}
/// Returned only by the explicitly authorized mailbox path. No diagnostic serialization.
pub struct NativeNetworkMail {
    pub id: crate::MessageId,
    pub author: String,
    pub origin: String,
    pub destination: String,
    pub recipient: String,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    pub encoding: crate::message::MessageEncoding,
    pub parent: Option<crate::MessageId>,
}
fn alias_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.trim() == s
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" .-_".contains(&b))
        && s.bytes().any(|b| b.is_ascii_alphabetic())
        && !matches!(
            s.to_ascii_uppercase().as_str(),
            "SYSOP" | "SBBS" | "NETMAIL" | "ALL"
        )
}
fn policy(conn: &rusqlite::Connection, link: &str, inbound: bool) -> Result<(i64, bool), Error> {
    conn.query_row("SELECT version,transit FROM qwk_private_policy WHERE link_id=?1 AND enabled=1 AND CASE WHEN ?2 THEN inbound ELSE outbound END=1", params![link,inbound], |r| Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or(Error::Disabled)
}
fn next_hop(
    conn: &rusqlite::Connection,
    network: &str,
    destination: &str,
) -> Result<(Link, i64, bool), Error> {
    let next: String = conn
        .query_row(
            "SELECT link_id FROM qwk_private_routes WHERE network=?1 AND destination=?2",
            params![network, destination],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(Error::InvalidMapping)?;
    let link = load_link(conn, &next)?;
    let (version, transit) = policy(conn, &next, false)?;
    if !link.enabled || !link.outbound {
        return Err(Error::Disabled);
    }
    Ok((link, version, transit))
}
fn private_area(destination: &str, recipient: &str) -> String {
    // Disjoint from configured public area tokens; the full private audience is identity.
    format!("@private/{destination}/{}", recipient.to_ascii_lowercase())
}
impl RuntimeDatabase {
    /// Typed operator policy, CAS protected and audited without names or content.
    pub fn configure_qwk_mail(
        &mut self,
        principal: &str,
        config: &MailPolicy,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        if expected.checked_add(1) != Some(config.version)
            || expected < 0
            || config.aliases.len() > 256
            || config.destinations.len() > 64
        {
            return Err(Error::InvalidMapping);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let link = load_link(&tx, &config.link)?;
        let prior: i64 = tx
            .query_row(
                "SELECT version FROM qwk_private_policy WHERE link_id=?1",
                [&link.id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if prior != expected {
            return Err(Error::Conflict);
        }
        let mut aliases = BTreeSet::new();
        let mut callers = BTreeSet::new();
        let mut destinations = BTreeSet::new();
        for alias in &config.aliases {
            if !alias_valid(&alias.alias)
                || !aliases.insert(alias.alias.to_ascii_lowercase())
                || !callers.insert(alias.caller_id)
            {
                return Err(Error::InvalidMapping);
            }
            let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM callers WHERE caller_id=?1 AND account_state='active')",[alias.caller_id],|r|r.get(0))?;
            if !active {
                return Err(Error::InvalidMapping);
            }
        }
        for destination in &config.destinations {
            if !wire::valid_system_id(destination)
                || destination == &link.local_id
                || !destinations.insert(destination)
            {
                return Err(Error::InvalidMapping);
            }
            let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM qwk_private_routes WHERE network=?1 AND destination=?2 AND link_id<>?3)",params![link.network,destination,link.id],|r|r.get(0))?;
            if collision {
                return Err(Error::InvalidMapping);
            }
        }
        tx.execute("INSERT INTO qwk_private_policy VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(link_id) DO UPDATE SET enabled=excluded.enabled,inbound=excluded.inbound,outbound=excluded.outbound,transit=excluded.transit,version=excluded.version",params![config.link,config.enabled,config.inbound,config.outbound,config.transit,config.version])?;
        tx.execute(
            "DELETE FROM qwk_mailbox_aliases WHERE link_id=?1",
            [&link.id],
        )?;
        tx.execute(
            "DELETE FROM qwk_private_routes WHERE link_id=?1",
            [&link.id],
        )?;
        for a in &config.aliases {
            tx.execute(
                "INSERT INTO qwk_mailbox_aliases VALUES(?1,?2,?3)",
                params![link.id, a.alias, a.caller_id],
            )?;
        }
        for d in &config.destinations {
            tx.execute(
                "INSERT INTO qwk_private_routes VALUES(?1,?2,?3)",
                params![link.network, d, link.id],
            )?;
        }
        tx.execute("UPDATE network_outbound_queue SET state='held',reason='private-policy-changed',version=version+1 WHERE state IN ('pending','ready','retry') AND queue_id IN (SELECT decision_id FROM network_routing_decisions WHERE link_id=?1 AND wire_conference=0)",[&link.id])?;
        audit(&tx, principal, "network.mail-policy", &link.id, now)?;
        event(&tx, "configured", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn qwk_mail_policy(&self, link: &str) -> Result<MailPolicy, Error> {
        let mut p=self.connection.query_row("SELECT enabled,inbound,outbound,transit,version FROM qwk_private_policy WHERE link_id=?1",[link],|r|Ok(MailPolicy{link:link.into(),enabled:r.get(0)?,inbound:r.get(1)?,outbound:r.get(2)?,transit:r.get(3)?,version:r.get(4)?,aliases:Vec::new(),destinations:Vec::new()}))?;
        p.aliases = self
            .connection
            .prepare(
                "SELECT alias,caller_id FROM qwk_mailbox_aliases WHERE link_id=?1 ORDER BY alias",
            )?
            .query_map([link], |r| {
                Ok(MailboxAlias {
                    alias: r.get(0)?,
                    caller_id: r.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        p.destinations = self
            .connection
            .prepare(
                "SELECT destination FROM qwk_private_routes WHERE link_id=?1 ORDER BY destination",
            )?
            .query_map([link], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(p)
    }
    /// The existing native payload/fanout/delivery tables own private content.
    /// Publication and durable pending queue intent commit atomically with the delivery.
    pub fn send_qwk_mail(
        &mut self,
        actor: crate::MessageActor,
        mail: &NewNetworkMail,
        now: i64,
    ) -> Result<crate::MessageId, Error> {
        let caller = self
            .caller_by_id(actor.caller_id())?
            .filter(|c| c.state == crate::CallerState::Active)
            .ok_or(Error::Rejected)?;
        if !alias_valid(&mail.recipient)
            || !token(&mail.network, 32)
            || !wire::valid_system_id(&mail.destination)
        {
            return Err(Error::InvalidMapping);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (link, version, _) = next_hop(&tx, &mail.network, &mail.destination)?;
        let author: String = tx
            .query_row(
                "SELECT a.alias FROM qwk_mailbox_aliases a JOIN callers c USING(caller_id) WHERE a.link_id=?1 AND a.caller_id=?2 AND c.account_state='active'",
                params![link.id, caller.id.get()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Rejected)?;
        let wall_time = chrono::DateTime::from_timestamp(now, 0)
            .ok_or(Error::Rejected)?
            .naive_utc();
        let mut member = wire::NetworkMessage {
            message: qwk::Message {
                number: 1,
                conference: 0,
                reference: 0,
                private: true,
                received: false,
                to: mail.recipient.as_bytes().to_vec(),
                from: author.as_bytes().to_vec(),
                subject: mail.subject.clone(),
                body: mail.body.clone(),
                wall_time,
            },
            metadata: wire::Metadata {
                id: format!("<{}@{}.qwk>", id(), link.local_id),
                reply: None,
                path: Vec::new(),
                utf8: false,
                written: Some(format!("{} 0000", wall_time.format("%Y%m%dT%H%M%SZ"))),
                fields: Vec::new(),
            },
            offset: 0,
            digest: String::new(),
        };
        validate_member(&member)?;
        let parent = if let Some(mid) = mail.reply_to {
            let parent = self_authorized_parent(
                &tx,
                caller.id.get(),
                mid.get(),
                &mail.network,
                &mail.destination,
                &mail.recipient,
            )?;
            member.metadata.reply = Some(parent);
            Some(mid.get())
        } else {
            None
        };
        let mid = insert_native(
            &tx,
            &member,
            Some(caller.id.get()),
            None,
            false,
            parent,
            now,
        )?;
        tx.execute(
            "INSERT INTO network_private_envelopes VALUES(?1,?2,?3,?4,?5,?6,?7,NULL)",
            params![
                mid,
                link.network,
                link.local_id,
                mail.destination,
                mail.recipient,
                link.id,
                version
            ],
        )?;
        let publication = publish(
            &tx,
            &link,
            &member,
            mid,
            &mail.destination,
            &mail.recipient,
            &link.local_id,
            None,
            now,
        )?;
        queue(&tx, &link, version, &publication, mid, now)?;
        event(&tx, "private-created", now)?;
        tx.commit()?;
        crate::MessageId::new(mid).map_err(Error::Message)
    }
    /// Neither Sysop threshold nor packet display names confer mailbox access.
    pub fn read_qwk_mail(
        &self,
        actor: crate::MessageActor,
        id: crate::MessageId,
    ) -> Result<NativeNetworkMail, Error> {
        let caller = self
            .caller_by_id(actor.caller_id())?
            .filter(|c| c.state == crate::CallerState::Active)
            .ok_or(Error::Rejected)?;
        let allowed:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM messages m LEFT JOIN message_delivery_recipients r USING(message_id) WHERE m.message_id=?1 AND m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND (m.author_caller_id=?2 OR r.caller_id=?2))",params![id.get(),caller.id.get()],|r|r.get(0))?;
        if !allowed {
            return Err(Error::Rejected);
        }
        Ok(self.connection.query_row("SELECT m.author_name,e.origin_system,e.destination_system,e.recipient,p.subject,p.body,p.encoding,m.parent_message_id FROM messages m JOIN network_private_envelopes e USING(message_id) JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE m.message_id=?1",[id.get()],|r|Ok(NativeNetworkMail{id,author:r.get(0)?,origin:r.get(1)?,destination:r.get(2)?,recipient:r.get(3)?,subject:r.get(4)?,body:r.get(5)?,encoding:if r.get::<_,String>(6)?=="utf8"{crate::message::MessageEncoding::Utf8}else{crate::message::MessageEncoding::LegacyCp437},parent:r.get::<_,Option<i64>>(7)?.and_then(|v|crate::MessageId::new(v).ok())}))?)
    }
    pub fn qwk_mailbox(
        &self,
        actor: crate::MessageActor,
        after: Option<crate::MessageId>,
    ) -> Result<Vec<crate::MessageId>, Error> {
        let caller = self
            .caller_by_id(actor.caller_id())?
            .filter(|c| c.state == crate::CallerState::Active)
            .ok_or(Error::Rejected)?;
        let ids=self.connection.prepare("SELECT m.message_id FROM messages m LEFT JOIN message_delivery_recipients r USING(message_id) WHERE m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND (m.author_caller_id=?1 OR r.caller_id=?1) AND m.message_id>?2 ORDER BY m.message_id LIMIT 20")?.query_map(params![caller.id.get(),after.map_or(0,|m|m.get())],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
        ids.into_iter()
            .map(|v| crate::MessageId::new(v).map_err(Error::Message))
            .collect()
    }
}
fn self_authorized_parent(
    conn: &rusqlite::Connection,
    caller: i64,
    mid: i64,
    network: &str,
    destination: &str,
    recipient: &str,
) -> Result<String, Error> {
    conn.query_row("SELECT p.wire_id FROM messages m JOIN message_delivery_recipients r USING(message_id) JOIN network_private_envelopes e USING(message_id) JOIN network_publications p USING(message_id) WHERE m.message_id=?1 AND r.caller_id=?2 AND m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND e.network=?3 AND e.origin_system=?4 AND m.author_name=?5 COLLATE NOCASE",params![mid,caller,network,destination,recipient],|r|r.get(0)).optional()?.ok_or(Error::Rejected)
}
fn validate_member(member: &wire::NetworkMessage) -> Result<(), Error> {
    let m = &member.message;
    let author = native_text(&m.from, member.metadata.utf8)?;
    if m.subject.is_empty()
        || m.subject.len() > 72
        || m.body.is_empty()
        || m.body.len() > 65536
        || author.is_empty()
        || author.chars().count() > 60
        || !wire::valid_native_text(m, member.metadata.utf8)
    {
        return Err(Error::Rejected);
    }
    Ok(())
}
fn insert_native(
    tx: &Transaction<'_>,
    member: &wire::NetworkMessage,
    author: Option<i64>,
    recipient: Option<(i64, String)>,
    transit: bool,
    parent: Option<i64>,
    now: i64,
) -> Result<i64, Error> {
    let mid = crate::message::next_message_id(tx)?.get();
    let m = &member.message;
    tx.execute("INSERT INTO message_payloads(subject,body,content_kind,encoding) VALUES(?1,?2,'standard',?3)",params![m.subject,m.body,if member.metadata.utf8{"utf8"}else{"cp437"}])?;
    let payload = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO message_fanouts(payload_id,created_by_caller_id,created_at) VALUES(?1,?2,?3)",
        params![payload, author, now],
    )?;
    let fanout = tx.last_insert_rowid();
    tx.execute("INSERT INTO messages(message_id,fanout_id,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal,origin_kind,container_kind) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'private','active','single',0,?9,?10)",params![mid,fanout,author,native_text(&m.from,member.metadata.utf8)?,member.metadata.written.as_deref().and_then(wire::written_timestamp).unwrap_or(now),now,parent,if recipient.is_some(){"local-recipient"}else{"external-recipient"},if author.is_some(){"native"}else{"external-network"},if transit{"network-transit"}else{"local-network-mailbox"}])?;
    if let Some((caller, name)) = recipient {
        tx.execute(
            "INSERT INTO message_delivery_recipients VALUES(?1,?2,?3,?4,?5)",
            params![mid, fanout, caller, name, now],
        )?;
    }
    Ok(mid)
}
#[allow(clippy::too_many_arguments)]
fn publish(
    tx: &Transaction<'_>,
    link: &Link,
    member: &wire::NetworkMessage,
    mid: i64,
    destination: &str,
    recipient: &str,
    origin: &str,
    ingress: Option<&str>,
    now: i64,
) -> Result<String, Error> {
    let p = id();
    let m = &member.message;
    tx.execute(
        "INSERT INTO network_publications VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            p,
            mid,
            link.network,
            private_area(destination, recipient),
            member.metadata.id,
            origin,
            ingress,
            member.metadata.reply,
            member.metadata.written,
            m.wall_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            recipient.as_bytes(),
            wire::content_digest(member),
            now
        ],
    )?;
    for (n, system) in member.metadata.path.iter().enumerate() {
        tx.execute(
            "INSERT INTO network_publication_path VALUES(?1,?2,?3)",
            params![p, n as i64, system],
        )?;
    }
    Ok(p)
}
fn queue(
    tx: &Transaction<'_>,
    link: &Link,
    version: i64,
    publication: &str,
    mid: i64,
    now: i64,
) -> Result<(), Error> {
    let (total,count,bytes,linkbytes):(i64,i64,i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(d.link_id=?1),0),COALESCE(SUM(q.reserved_bytes),0),COALESCE(SUM(CASE WHEN d.link_id=?1 THEN q.reserved_bytes ELSE 0 END),0) FROM network_outbound_queue q JOIN network_routing_decisions d ON d.decision_id=q.queue_id WHERE q.state NOT IN ('accepted','cancelled')",[&link.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let reserve:i64=tx.query_row("SELECT length(p.subject)+length(p.body)+16896 FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE m.message_id=?1",[mid],|r|r.get(0))?;
    if total >= 10000
        || count >= 1000
        || bytes + reserve > 256 * 1024 * 1024
        || linkbytes + reserve > 64 * 1024 * 1024
    {
        return Err(Error::Capacity);
    }
    let q = id();
    tx.execute(
        "INSERT INTO network_routing_decisions VALUES(?1,?2,?3,?4,?5,?6,0,1,?7,?8)",
        params![
            q,
            publication,
            link.id,
            link.remote_id,
            link.version,
            version,
            format!("private:{version}"),
            now
        ],
    )?;
    tx.execute("INSERT INTO network_outbound_queue(queue_id,state,created_at,reserved_bytes) VALUES(?1,'pending',?2,?3)",params![q,now,reserve])?;
    Ok(())
}

pub(super) struct Imported {
    pub outcome: &'static str,
    pub reason: &'static str,
    pub native: Option<i64>,
    pub publication: Option<String>,
}
fn rejected(reason: &'static str) -> Imported {
    Imported {
        outcome: "quarantined",
        reason,
        native: None,
        publication: None,
    }
}
pub(super) fn ingest(
    tx: &Transaction<'_>,
    link: &Link,
    member: &wire::NetworkMessage,
    path: Vec<String>,
    now: i64,
) -> Result<Imported, Error> {
    if member.message.conference != 0 || !member.message.private {
        return Ok(rejected("private-framing"));
    }
    let Ok((_, allow_transit)) = policy(tx, &link.id, true) else {
        return Ok(rejected("private-policy"));
    };
    if validate_member(member).is_err() {
        return Ok(rejected("native-bounds"));
    }
    let recipient = native_text(&member.message.to, member.metadata.utf8)?;
    if !alias_valid(&recipient) {
        return Ok(rejected("invalid-recipient"));
    }
    let mut destination_path = member.metadata.destination_path()?;
    if destination_path.first() == Some(&link.local_id) {
        destination_path.remove(0);
    }
    let destination = destination_path
        .last()
        .cloned()
        .unwrap_or_else(|| link.local_id.clone());
    let origin = path.last().ok_or(Error::Rejected)?.clone();
    let mut next = None;
    let mut local = None;
    if destination == link.local_id {
        if !destination_path.is_empty() {
            return Ok(rejected("destination-mismatch"));
        }
        local=tx.query_row("SELECT a.caller_id,a.alias FROM qwk_mailbox_aliases a JOIN callers c USING(caller_id) WHERE a.link_id=?1 AND a.alias=?2 COLLATE NOCASE AND c.account_state='active'",params![link.id,recipient],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).optional()?;
        if local.is_none() {
            return Ok(rejected("unresolved-recipient"));
        }
    } else {
        let Ok((hop, version, transit)) = next_hop(tx, &link.network, &destination) else {
            return Ok(rejected("unresolved-private-route"));
        };
        if !allow_transit || !transit {
            return Ok(rejected("transit-denied"));
        }
        if hop.id == link.id
            || path.contains(&hop.remote_id)
            || destination_path
                .iter()
                .any(|s| path.contains(s) || s == &link.local_id)
        {
            return Ok(Imported {
                outcome: "loop",
                reason: "private-return-route",
                native: None,
                publication: None,
            });
        }
        // Incoming route is evidence: it cannot select an arbitrary next hop.
        if destination_path.len() > 2
            || (destination_path.len() == 2 && destination_path[0] != hop.remote_id)
        {
            return Ok(rejected("destination-mismatch"));
        }
        next = Some((hop, version));
    }
    let area = private_area(&destination, &recipient);
    let previous:Option<(String,i64,String,Option<String>)>=tx.query_row("SELECT publication_id,message_id,origin,content_digest FROM network_publications WHERE network=?1 AND area=?2 AND wire_id=?3",params![link.network,area,member.metadata.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    if let Some((p, mid, old_origin, digest)) = previous {
        if old_origin != origin || digest.as_deref() != Some(&wire::content_digest(member)) {
            return Ok(rejected("identity-collision"));
        }
        return Ok(Imported {
            outcome: "duplicate",
            reason: "retained-private-identity",
            native: Some(mid),
            publication: Some(p),
        });
    }
    if member
        .metadata
        .id
        .to_ascii_lowercase()
        .ends_with(&format!("@{}.qwk>", link.local_id.to_ascii_lowercase()))
    {
        return Ok(rejected("local-origin-spoof"));
    }
    let parent = if let (Some(reply), Some((caller, _))) = (&member.metadata.reply, &local) {
        tx.query_row("SELECT p.message_id FROM network_publications p JOIN messages m USING(message_id) JOIN network_private_envelopes e USING(message_id) WHERE p.network=?1 AND p.wire_id=?2 AND m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND m.author_caller_id=?3 AND e.destination_system=?4 AND e.recipient=?5 COLLATE NOCASE GROUP BY p.network,p.wire_id HAVING COUNT(DISTINCT p.message_id)=1",params![link.network,reply,caller,origin,native_text(&member.message.from,member.metadata.utf8)?],|r|r.get(0)).optional()?
    } else {
        None
    };
    let mid = insert_native(tx, member, None, local, next.is_some(), parent, now)?;
    tx.execute(
        "INSERT INTO network_private_envelopes VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            mid,
            link.network,
            origin,
            destination,
            recipient,
            next.as_ref().map(|v| &v.0.id),
            next.as_ref().map(|v| v.1),
            link.id
        ],
    )?;
    let mut retained = member.clone();
    retained.metadata.path = path;
    let p = publish(
        tx,
        link,
        &retained,
        mid,
        &destination,
        &recipient,
        &origin,
        Some(&link.id),
        now,
    )?;
    if let Some((hop, version)) = next {
        queue(tx, &hop, version, &p, mid, now)?;
    }
    Ok(Imported {
        outcome: "imported",
        reason: "native-private-import",
        native: Some(mid),
        publication: Some(p),
    })
}

pub(super) fn export(
    conn: &rusqlite::Connection,
    queue: &str,
    link: &Link,
) -> Result<ExportMember, Error> {
    let (publication,mid,version,policy_version,link_version):(String,i64,i64,i64,i64)=conn.query_row("SELECT d.publication_id,p.message_id,d.message_version,d.mapping_version,d.link_version FROM network_routing_decisions d JOIN network_publications p USING(publication_id) WHERE d.decision_id=?1 AND d.link_id=?2 AND d.wire_conference=0",params![queue,link.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    let (destination,recipient,ingress):(String,String,Option<String>)=conn.query_row("SELECT e.destination_system,e.recipient,e.ingress_link FROM network_private_envelopes e JOIN messages m USING(message_id) WHERE m.message_id=?1 AND e.next_link=?2 AND e.policy_version=?3 AND e.network=?4 AND m.state_version=?5 AND m.visibility='private' AND m.lifecycle_state='active' AND m.container_kind IN ('local-network-mailbox','network-transit')",params![mid,link.id,policy_version,link.network,version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(Error::Held)?;
    let (hop, current, transit) = next_hop(conn, &link.network, &destination)?;
    if hop.id != link.id
        || current != policy_version
        || link.version != link_version
        || ingress.as_deref() == Some(&link.id)
    {
        return Err(Error::Held);
    }
    if let Some(ingress) = ingress {
        let source = load_link(conn, &ingress)?;
        let (_, source_transit) = policy(conn, &ingress, true)?;
        if !transit || !source_transit || !source.enabled || !source.inbound {
            return Err(Error::Held);
        }
    } else {
        let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN callers c ON c.caller_id=m.author_caller_id JOIN qwk_mailbox_aliases a ON a.caller_id=c.caller_id WHERE m.message_id=?1 AND a.link_id=?2 AND a.alias=m.author_name COLLATE NOCASE AND c.account_state='active')",params![mid,link.id],|r|r.get(0))?;
        if !valid {
            return Err(Error::Held);
        }
    }
    let path = publication_path(conn, &publication)?;
    if path.contains(&link.remote_id)
        || path.contains(&link.local_id)
        || path.contains(&destination)
    {
        return Err(Error::Held);
    }
    let (author,subject,body,encoding):(String,Vec<u8>,Vec<u8>,String)=conn.query_row("SELECT m.author_name,p.subject,p.body,p.encoding FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE m.message_id=?1",[mid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let (wire_id,reply,written,wall):(String,Option<String>,Option<String>,String)=conn.query_row("SELECT wire_id,reply_id,source_written,source_wall_time FROM network_publications WHERE publication_id=?1",[&publication],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let utf8 = encoding == "utf8";
    let from = if utf8 {
        author.into_bytes()
    } else {
        crate::encode_text(&author, crate::TerminalTextEncoding::Cp437).ok_or(Error::Held)?
    };
    let fields = if destination == link.remote_id {
        Vec::new()
    } else {
        vec![("recipientnetaddr".into(), destination.into_bytes())]
    };
    Ok(ExportMember {
        queue: queue.into(),
        publication,
        mapping: Mapping {
            wire_conference: 0,
            area: "@private".into(),
            conference_id: 0,
            enabled: true,
            inbound: false,
            outbound: true,
            version: current,
        },
        message_version: version,
        policy: format!("private:{current}"),
        wire: wire::NetworkMessage {
            message: qwk::Message {
                number: 1,
                conference: 0,
                reference: 0,
                private: true,
                received: false,
                to: recipient.into_bytes(),
                from,
                subject,
                body,
                wall_time: wall.parse().map_err(|_| Error::Held)?,
            },
            metadata: wire::Metadata {
                id: wire_id,
                reply,
                path,
                utf8,
                written,
                fields,
            },
            offset: 0,
            digest: String::new(),
        },
    })
}
