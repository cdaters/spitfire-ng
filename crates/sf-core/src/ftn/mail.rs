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

//! Native NetMail/EchoMail transactions and immutable per-target work.
use super::*;
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TossResult {
    pub imported: u32,
    pub duplicates: u32,
    pub loops: u32,
    pub quarantined: u32,
}
/// Authenticated caller input; never a serialized operator impersonation request.
pub struct NewNetMail {
    pub aka: String,
    pub destination: Endpoint,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub reply_to: Option<crate::MessageId>,
}
pub struct NativeNetMail {
    pub id: crate::MessageId,
    pub author: String,
    pub source: Endpoint,
    pub destination: Endpoint,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub parent: Option<crate::MessageId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub link: String,
    pub final_destination: Endpoint,
    pub next_hop: Endpoint,
    pub aka: String,
    pub state: String,
    pub version: i64,
    pub artifact: Option<String>,
    pub reason: String,
}
struct Envelope {
    source: Endpoint,
    destination: Endpoint,
    from: String,
    to: String,
    subject: String,
    text: Text,
    date: [u8; 20],
    attributes: u16,
    cost: u16,
}
impl Envelope {
    fn scope(&self) -> String {
        self.text.area.clone().unwrap_or_else(|| {
            format!(
                "@netmail/{}/{}",
                self.destination.address,
                self.to.to_ascii_lowercase()
            )
        })
    }
    fn fingerprint(&self) -> String {
        let destination = if self.text.area.is_some() {
            String::new()
        } else {
            self.destination.to_string()
        };
        sf_net::qwk::digest(
            &serde_json::to_vec(&(
                self.source.to_string(),
                destination,
                &self.from,
                &self.to,
                &self.subject,
                self.text.body.trim_end_matches('\n'),
                self.date.as_slice(),
                self.text.offset_minutes,
                self.attributes,
                &self.text.reply,
            ))
            .expect("strings serialize"),
        )
    }
    fn validate(&self) -> Result<(), Error> {
        if self.subject.is_empty()
            || self.subject.len() > 72
            || self.text.body.is_empty()
            || self.text.body.len() > 65536
            || self.from.is_empty()
            || self.from.len() > 60
            || self.to.is_empty()
            || self.to.len() > 60
            || [&self.from, &self.to, &self.subject]
                .iter()
                .any(|s| s.chars().any(char::is_control))
        {
            return Err(Error::Rejected);
        }
        // File/request/receipt/audit and crash/direct/hold behavior need separate policy owners.
        // Local status bits are retained as evidence but never grant authority.
        if self.attributes
            & (0x0002 | 0x0010 | 0x0200 | 0x0400 | 0x0800 | 0x1000 | 0x2000 | 0x4000 | 0x8000)
            != 0
            || self.text.flags.iter().any(|f| f != "PVT")
        {
            return Err(Error::Rejected);
        }
        if self.text.area.is_some() && self.attributes & 1 != 0 {
            return Err(Error::Rejected);
        }
        wire::parse_date(&self.date)?;
        Ok(())
    }
}
fn mapping(conn: &rusqlite::Connection, domain: &Domain, area: &str) -> Result<Mapping, Error> {
    let mut m=conn.query_row("SELECT conference_id,aka,receive,send,origin,version FROM ftn_area_mappings WHERE domain=?1 AND area=?2",params![domain.as_str(),area],|r|Ok(Mapping{domain:domain.clone(),area:area.into(),conference_id:r.get(0)?,aka:r.get(1)?,receive:r.get(2)?,send:r.get(3)?,origin:r.get(4)?,version:r.get(5)?,links:vec![]})).optional()?.ok_or(Error::Policy)?;
    m.links = conn
        .prepare("SELECT link_id FROM ftn_area_links WHERE domain=?1 AND area=?2 ORDER BY link_id")?
        .query_map(params![domain.as_str(), area], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(m)
}
fn new_text(
    body: &str,
    charset: Charset,
    source: &Endpoint,
    serial: u32,
    reply: Option<&str>,
    destination: Option<&Endpoint>,
) -> Result<Text, Error> {
    let mut t = Text::plain(body, charset)?;
    t.msgid = Some(format!("{} {serial:08x}", source.address));
    t.reply = reply.map(String::from);
    t.offset_minutes = Some(0);
    t.add_control(&format!(
        "MSGID: {}",
        t.msgid.as_deref().ok_or(Error::Rejected)?
    ));
    if let Some(reply) = reply {
        t.add_control(&format!("REPLY: {reply}"));
    }
    t.add_control(&format!(
        "CHRS: {} {}",
        charset.identifier(),
        if charset == Charset::Utf8 {
            4
        } else if charset == Charset::Ascii {
            1
        } else {
            2
        }
    ));
    t.add_control("TZUTC: 0000");
    t.add_control("PID: SPITFIRE-NG 0.1");
    if let Some(d) = destination {
        t.intl = Some((d.address.boss(), source.address.boss()));
        t.fmpt = source.address.point();
        t.topt = d.address.point();
        t.add_control(&format!(
            "INTL {} {}",
            d.address.boss(),
            source.address.boss()
        ));
        if t.fmpt != 0 {
            t.add_control(&format!("FMPT {}", t.fmpt));
        }
        if t.topt != 0 {
            t.add_control(&format!("TOPT {}", t.topt));
        }
    }
    Ok(t)
}
fn serial(tx: &Transaction<'_>, source: &Endpoint) -> Result<u32, Error> {
    let aid = address_id(tx, source)?;
    let (n, held): (i64, bool) = tx.query_row(
        "SELECT next_serial,restored_hold FROM ftn_serials WHERE address_id=?1",
        [aid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if held {
        return Err(Error::Held);
    }
    let result = u32::try_from(n).map_err(|_| Error::Capacity)?;
    tx.execute(
        "UPDATE ftn_serials SET next_serial=next_serial+1 WHERE address_id=?1",
        [aid],
    )?;
    Ok(result)
}
fn insert_native(
    tx: &Transaction<'_>,
    e: &Envelope,
    author: Option<i64>,
    recipient: Option<(i64, String)>,
    conference: Option<i64>,
    parent: Option<i64>,
    now: i64,
) -> Result<i64, Error> {
    e.validate()?;
    let mid = crate::message::next_message_id(tx)?.get();
    let number = conference
        .map(|c| {
            crate::ConferenceId::new(c).and_then(|c| crate::message::next_message_number(tx, c))
        })
        .transpose()?;
    tx.execute("INSERT INTO message_payloads(subject,body,content_kind,encoding) VALUES(?1,?2,'standard','utf8')",params![e.subject.as_bytes(),e.text.body.as_bytes()])?;
    let payload = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO message_fanouts(payload_id,created_by_caller_id,created_at) VALUES(?1,?2,?3)",
        params![payload, author, now],
    )?;
    let fanout = tx.last_insert_rowid();
    let written = e
        .text
        .offset_minutes
        .and_then(|offset| {
            wire::parse_date(&e.date)
                .ok()?
                .and_utc()
                .timestamp()
                .checked_sub(i64::from(offset) * 60)
        })
        .unwrap_or(now);
    let container = if conference.is_some() {
        "conference"
    } else if recipient.is_some() || author.is_some() {
        "local-network-mailbox"
    } else {
        "network-transit"
    };
    tx.execute("INSERT INTO messages(message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,parent_message_id,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal,origin_kind,container_kind) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'active','single',0,?12,?13)",params![mid,fanout,conference,number.map(|n|n as i64),author,e.from,written,now,parent,if conference.is_some(){"all-callers"}else if recipient.is_some(){"local-recipient"}else{"external-recipient"},if conference.is_some(){"public"}else{"private"},if author.is_some(){"native"}else{"external-network"},container])?;
    if let Some((caller, name)) = recipient {
        tx.execute(
            "INSERT INTO message_delivery_recipients VALUES(?1,?2,?3,?4,?5)",
            params![mid, fanout, caller, name, now],
        )?;
    }
    Ok(mid)
}
fn publish(
    tx: &Transaction<'_>,
    e: &Envelope,
    mid: i64,
    ingress: Option<&str>,
    now: i64,
) -> Result<String, Error> {
    let pid = id();
    let source = address_id(tx, &e.source)?;
    let dest = address_id(tx, &e.destination)?;
    let fingerprint = e.fingerprint();
    let identity = e
        .text
        .msgid
        .clone()
        .unwrap_or_else(|| format!("weak:{fingerprint}"));
    tx.execute("INSERT INTO ftn_messages VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",params![pid,mid,e.source.domain.as_str(),e.scope(),source,dest,e.to,e.text.msgid,e.text.reply,fingerprint,identity,ingress,e.text.charset.identifier(),e.date.as_slice(),e.text.offset_minutes,e.attributes,e.cost,e.text.origin,e.text.tear,now])?;
    for (n, c) in e.text.controls.iter().enumerate() {
        tx.execute(
            "INSERT INTO ftn_controls VALUES(?1,?2,?3,?4)",
            params![pid, n as i64, c.paragraph as i64, c.raw],
        )?;
    }
    for (kind, entries) in [
        ("seen", e.text.seen_by.iter().copied().collect::<Vec<_>>()),
        ("path", e.text.path.clone()),
    ] {
        for (i, (net, node)) in entries.iter().enumerate() {
            tx.execute(
                "INSERT INTO ftn_echo_history VALUES(?1,?2,?3,?4,?5)",
                params![pid, kind, i as i64, net, node],
            )?;
        }
    }
    Ok(pid)
}
fn queue(
    tx: &Transaction<'_>,
    policy: &Policy,
    pid: &str,
    e: &Envelope,
    mid: i64,
    ingress: Option<&str>,
    now: i64,
) -> Result<(), Error> {
    let digest = policy.digest()?;
    let mut targets = vec![];
    if let Some(area) = &e.text.area {
        let m = mapping(tx, &e.source.domain, area)?;
        if !m.send {
            return Ok(());
        }
        let aka = policy.aka(&m.aka)?;
        if !aka.enabled {
            return Err(Error::Policy);
        }
        for lid in &m.links {
            let l = policy.link(lid)?;
            let a = l.remote.address;
            if !l.enabled
                || !l.outbound
                || Some(lid.as_str()) == ingress
                || (a.point() == 0 && e.text.seen_by.contains(&a.two_d()))
                || l.remote == e.source
            {
                continue;
            }
            if a.zone() != aka.endpoint.address.zone() || e.source.address.zone() != a.zone() {
                return Err(Error::Routing);
            }
            targets.push((
                RouteDecision {
                    final_destination: l.remote.clone(),
                    next_hop: l.remote.clone(),
                    link: l.id.clone(),
                    aka: m.aka.clone(),
                    reason: "echo-subscription".into(),
                },
                m.version,
            ));
        }
    } else if policy.local(&e.destination).is_none() {
        let d = policy.route(&e.destination)?;
        if Some(d.link.as_str()) == ingress {
            return Err(Error::Routing);
        }
        targets.push((d, 0));
    }
    for (d, mv) in targets {
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ftn_routing_decisions WHERE publication_id=?1 AND link_id=?2)",params![pid,d.link],|r|r.get(0))?;
        if exists {
            continue;
        }
        let(count,bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(reserved_bytes),0) FROM network_outbound_queue WHERE state NOT IN('accepted','cancelled')",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let(lcount,lbytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(q.reserved_bytes),0) FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE d.link_id=?1 AND q.state NOT IN('accepted','cancelled')",[&d.link],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let reserve = (e.text.body.len() + 32768) as i64;
        if count >= 10000
            || bytes + reserve > 256 * 1024 * 1024
            || lcount >= 1000
            || lbytes + reserve > 64 * 1024 * 1024
        {
            return Err(Error::Capacity);
        }
        let q = id();
        let local = address_id(tx, &policy.aka(&d.aka)?.endpoint)?;
        let next = address_id(tx, &d.next_hop)?;
        let final_address = address_id(tx, &d.final_destination)?;
        let version: i64 = tx.query_row(
            "SELECT state_version FROM messages WHERE message_id=?1",
            [mid],
            |r| r.get(0),
        )?;
        tx.execute("INSERT INTO network_queue_work VALUES(?1,'ftn')", [&q])?;
        tx.execute(
            "INSERT INTO ftn_routing_decisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                q,
                pid,
                d.link,
                final_address,
                next,
                local,
                d.aka,
                digest,
                mv,
                version,
                d.reason,
                now
            ],
        )?;
        tx.execute("INSERT INTO network_outbound_queue(queue_id,state,created_at,reserved_bytes) VALUES(?1,'pending',?2,?3)",params![q,now,reserve])?;
    }
    Ok(())
}
fn load(conn: &rusqlite::Connection, pid: &str) -> Result<(Envelope, i64, Option<String>), Error> {
    struct StoredEnvelope {
        source: i64,
        dest: i64,
        from: String,
        to: String,
        subject: Vec<u8>,
        body: Vec<u8>,
        encoding: String,
        date: Vec<u8>,
        attrs: u16,
        cost: u16,
        charset: String,
        area: String,
        msgid: Option<String>,
        reply: Option<String>,
        offset: Option<i32>,
        origin: Option<String>,
        tear: Option<String>,
        mid: i64,
        ingress: Option<String>,
    }
    let StoredEnvelope { source, dest, from, to, subject, body, encoding, date, attrs, cost, charset, area, msgid, reply, offset, origin, tear, mid, ingress } = conn.query_row("SELECT f.origin_address,f.destination_address,m.author_name,f.recipient,p.subject,p.body,p.encoding,f.original_date,f.attributes,f.cost,f.charset,f.area,f.msgid,f.reply,f.utc_offset,f.origin_line,f.tear_line,f.message_id,f.ingress_link FROM ftn_messages f JOIN messages m USING(message_id) JOIN message_fanouts mf USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE f.publication_id=?1",[pid], |r| Ok(StoredEnvelope {
        source: r.get(0)?,
        dest: r.get(1)?,
        from: r.get(2)?,
        to: r.get(3)?,
        subject: r.get(4)?,
        body: r.get(5)?,
        encoding: r.get(6)?,
        date: r.get(7)?,
        attrs: r.get(8)?,
        cost: r.get(9)?,
        charset: r.get(10)?,
        area: r.get(11)?,
        msgid: r.get(12)?,
        reply: r.get(13)?,
        offset: r.get(14)?,
        origin: r.get(15)?,
        tear: r.get(16)?,
        mid: r.get(17)?,
        ingress: r.get(18)?,
    }))?;
    let native_charset = if encoding == "utf8" {
        Charset::Utf8
    } else {
        Charset::Cp437
    };
    let body = native_charset
        .decode(&body)?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let charset = Charset::parse(&charset)?;
    let mut text = Text::plain(&body, charset)?;
    text.area = (!area.starts_with('@')).then_some(area);
    text.msgid = msgid;
    text.reply = reply;
    text.offset_minutes = offset;
    text.origin = origin;
    text.tear = tear;
    text.controls = conn
        .prepare("SELECT paragraph,raw FROM ftn_controls WHERE publication_id=?1 ORDER BY ordinal")?
        .query_map([pid], |r| {
            Ok(wire::Control {
                paragraph: r.get::<_, u32>(0)? as usize,
                raw: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let entries=conn.prepare("SELECT kind,net,node FROM ftn_echo_history WHERE publication_id=?1 ORDER BY kind,ordinal")?.query_map([pid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u16>(1)?,r.get::<_,u16>(2)?)))?.collect::<Result<Vec<_>,_>>()?;
    for (kind, n, f) in entries {
        if kind == "seen" {
            text.seen_by.insert((n, f));
        } else {
            text.path.push((n, f));
        }
    }
    Ok((
        Envelope {
            source: endpoint(conn, source)?,
            destination: endpoint(conn, dest)?,
            from,
            to,
            subject: native_charset.decode(&subject)?,
            text,
            date: date.try_into().map_err(|_| Error::Rejected)?,
            attributes: attrs,
            cost,
        },
        mid,
        ingress,
    ))
}
impl RuntimeDatabase {
    pub fn send_ftn_mail(
        &mut self,
        actor: crate::MessageActor,
        policy: &Policy,
        mail: &NewNetMail,
        now: i64,
    ) -> Result<crate::MessageId, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        let aka = policy.aka(&mail.aka)?;
        if !aka.enabled || aka.endpoint.domain != mail.destination.domain {
            return Err(Error::Policy);
        }
        let source = address_id(&tx, &aka.endpoint)?;
        let author:String=tx.query_row("SELECT a.alias FROM ftn_mailbox_aliases a JOIN callers c USING(caller_id) WHERE a.address_id=?1 AND a.caller_id=?2 AND c.account_state='active'",params![source,actor.caller_id().get()],|r|r.get(0)).optional()?.ok_or(Error::Denied)?;
        let reply = if let Some(parent) = mail.reply_to {
            tx.query_row("SELECT f.msgid FROM ftn_messages f JOIN messages m USING(message_id) JOIN message_delivery_recipients r USING(message_id) WHERE m.message_id=?1 AND r.caller_id=?2 AND m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND f.domain=?3 AND f.origin_address=?4 AND m.author_name=?5 COLLATE NOCASE",params![parent.get(),actor.caller_id().get(),mail.destination.domain.as_str(),address_id(&tx,&mail.destination)?,mail.recipient],|r|r.get::<_,Option<String>>(0)).optional()?.ok_or(Error::Denied)?
        } else {
            None
        };
        let charset = if policy.local(&mail.destination).is_some() {
            Charset::Utf8
        } else {
            policy.link(&policy.route(&mail.destination)?.link)?.charset
        };
        let text = new_text(
            &mail.body,
            charset,
            &aka.endpoint,
            serial(&tx, &aka.endpoint)?,
            reply.as_deref(),
            Some(&mail.destination),
        )?;
        let e = Envelope {
            source: aka.endpoint.clone(),
            destination: mail.destination.clone(),
            from: author,
            to: mail.recipient.clone(),
            subject: mail.subject.clone(),
            text,
            date: wire::format_date(
                chrono::DateTime::from_timestamp(now, 0)
                    .ok_or(Error::Rejected)?
                    .naive_utc(),
            )?,
            attributes: 1,
            cost: 0,
        };
        let local = if policy.local(&mail.destination).is_some() {
            Some(resolve_recipient(&tx, &mail.destination, &mail.recipient)?)
        } else {
            None
        };
        let mid = insert_native(
            &tx,
            &e,
            Some(actor.caller_id().get()),
            local,
            None,
            mail.reply_to.map(|m| m.get()),
            now,
        )?;
        let pid = publish(&tx, &e, mid, None, now)?;
        queue(&tx, policy, &pid, &e, mid, None, now)?;
        event(&tx, "netmail-created", now)?;
        tx.commit()?;
        Ok(crate::MessageId::new(mid)?)
    }
    pub fn read_ftn_mail(
        &self,
        actor: crate::MessageActor,
        mid: crate::MessageId,
    ) -> Result<NativeNetMail, Error> {
        let pid:Option<String>=self.connection.query_row("SELECT f.publication_id FROM ftn_messages f JOIN messages m USING(message_id) JOIN callers c ON c.caller_id=?2 LEFT JOIN message_delivery_recipients r USING(message_id) WHERE m.message_id=?1 AND m.container_kind='local-network-mailbox' AND m.lifecycle_state='active' AND c.account_state='active' AND (m.author_caller_id=?2 OR r.caller_id=?2)",params![mid.get(),actor.caller_id().get()],|r|r.get(0)).optional()?;
        let (e, _, _) = load(&self.connection, &pid.ok_or(Error::Denied)?)?;
        let parent = self.connection.query_row(
            "SELECT parent_message_id FROM messages WHERE message_id=?1",
            [mid.get()],
            |r| r.get::<_, Option<i64>>(0),
        )?;
        Ok(NativeNetMail {
            id: mid,
            author: e.from,
            source: e.source,
            destination: e.destination,
            recipient: e.to,
            subject: e.subject,
            body: e.text.body,
            parent: parent.map(crate::MessageId::new).transpose()?,
        })
    }
    pub fn scan_ftn(&mut self, policy: &Policy, now: i64) -> Result<u32, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        let candidates=tx.prepare("SELECT m.message_id,f.domain,f.area,m.author_name,p.subject,p.body,p.encoding,m.created_at FROM ftn_area_mappings f JOIN message_conferences c ON c.conference_id=f.conference_id JOIN messages m ON m.conference_id=c.conference_id JOIN message_fanouts mf USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE c.active=1 AND f.send=1 AND m.origin_kind='native' AND m.author_caller_id IS NOT NULL AND m.lifecycle_state='active' AND m.visibility='public' AND m.audience_kind='all-callers' AND NOT EXISTS(SELECT 1 FROM ftn_messages fm WHERE fm.message_id=m.message_id AND fm.domain=f.domain) ORDER BY m.message_id LIMIT 1000")?.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Vec<u8>>(4)?,r.get::<_,Vec<u8>>(5)?,r.get::<_,String>(6)?,r.get::<_,i64>(7)?)))?.collect::<Result<Vec<_>,_>>()?;
        let mut count = 0;
        for (mid, domain, area, from, subject, body, encoding, created) in candidates {
            let domain: Domain = domain.parse()?;
            let m = mapping(&tx, &domain, &area)?;
            let aka = policy.aka(&m.aka)?;
            if !aka.enabled {
                continue;
            }
            let c = if encoding == "utf8" {
                Charset::Utf8
            } else {
                Charset::Cp437
            };
            let body = c.decode(&body)?.replace("\r\n", "\n").replace('\r', "\n");
            let reply: Option<String> = tx.query_row(
                "SELECT f.msgid FROM messages m JOIN ftn_messages f ON f.message_id=m.parent_message_id WHERE m.message_id=?1 AND f.domain=?2 AND f.area=?3",
                params![mid,domain.as_str(),area], |r| r.get(0)).optional()?.flatten();
            let mut text = new_text(
                &body,
                c,
                &aka.endpoint,
                serial(&tx, &aka.endpoint)?,
                reply.as_deref(),
                None,
            )?;
            text.area = Some(area);
            text.origin = Some(format!(
                " * Origin: {} ({})",
                m.origin, aka.endpoint.address
            ));
            text.tear = Some("--- SPITFIRE-NG 0.1".into());
            let e = Envelope {
                source: aka.endpoint.clone(),
                destination: aka.endpoint.clone(),
                from,
                to: "All".into(),
                subject: c.decode(&subject)?,
                text,
                date: wire::format_date(
                    chrono::DateTime::from_timestamp(created, 0)
                        .ok_or(Error::Rejected)?
                        .naive_utc(),
                )?,
                attributes: 0,
                cost: 0,
            };
            e.validate()?;
            let pid = publish(&tx, &e, mid, None, now)?;
            queue(&tx, policy, &pid, &e, mid, None, now)?;
            count += 1;
        }
        event(&tx, "scan-completed", now)?;
        tx.commit()?;
        Ok(count)
    }
    pub fn toss_ftn(
        &mut self,
        store: &dyn NetworkArtifactStore,
        policy: &Policy,
        link_id: &str,
        bytes: &[u8],
        now: i64,
    ) -> Result<TossResult, Error> {
        policy.validate()?;
        let link = policy.link(link_id)?;
        let aka = policy.aka(&link.aka)?;
        if !policy.enabled || !link.enabled || !link.inbound || !aka.enabled {
            return Err(Error::Denied);
        }
        let _permit = store.admit_import()?;
        if bytes.len() > wire::MAX_PACKET {
            return Err(Error::Capacity);
        }
        {
            let tx = self.connection.transaction()?;
            bind(&tx, policy)?;
            tx.commit()?;
        }
        let artifact = self.preserve_artifact(store, bytes, now)?;
        let packet = Packet::decode(
            bytes,
            (link.remote.address.zone(), aka.endpoint.address.zone()),
        );
        let packet = match packet {
            Ok(p)
                if p.header.origin == link.remote.address
                    && p.header.destination == aka.endpoint.address
                    && p.header.password == [0; 8] =>
            {
                p
            }
            _ => {
                let tx = self.connection.transaction()?;
                let prior:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ftn_import_receipts WHERE link_id=?1 AND artifact_id=?2 AND ordinal=-1)",params![link_id,artifact],|r|r.get(0))?;
                if !prior {
                    quarantine(&tx, Some(link_id), &artifact, "ftn-packet-rejected", now)?;
                    receipt(
                        &tx,
                        link_id,
                        &artifact,
                        -1,
                        None,
                        "quarantined",
                        "ftn-packet-rejected",
                        now,
                    )?;
                    event(&tx, "packet-rejected", now)?;
                }
                tx.commit()?;
                return Ok(TossResult {
                    quarantined: 1,
                    ..Default::default()
                });
            }
        };
        let mut result = TossResult::default();
        for (n, m) in packet.messages.iter().enumerate() {
            let prior:Option<String>=self.connection.query_row("SELECT outcome FROM ftn_import_receipts WHERE link_id=?1 AND artifact_id=?2 AND ordinal=?3",params![link_id,artifact,n as i64],|r|r.get(0)).optional()?;
            if let Some(p) = prior {
                count_result(&mut result, if p == "imported" { "duplicate" } else { &p });
                continue;
            }
            let outcome = (|| -> Result<(String, Option<i64>), Error> {
                let text = Text::parse(&m.text, link.charset)?;
                let (o, d) = text.addresses(m, &packet.header)?;
                if let Some(asserted) = text
                    .origin
                    .as_deref()
                    .and_then(|s| s.rsplit_once('('))
                    .and_then(|(_, s)| s.strip_suffix(')'))
                    .and_then(|s| s.split_once('@'))
                    .map(|(_, d)| d)
                {
                    if asserted.parse::<Domain>()? != link.remote.domain {
                        return Err(Error::Denied);
                    }
                }
                let e = Envelope {
                    source: Endpoint {
                        domain: link.remote.domain.clone(),
                        address: o,
                    },
                    destination: Endpoint {
                        domain: link.remote.domain.clone(),
                        address: d,
                    },
                    from: text.charset.decode(&m.from)?,
                    to: text.charset.decode(&m.to)?,
                    subject: text.charset.decode(&m.subject)?,
                    text,
                    date: m.date,
                    attributes: m.attributes,
                    cost: m.cost,
                };
                e.validate()?;
                let tx = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                bind(&tx, policy)?;
                let identity = e
                    .text
                    .msgid
                    .clone()
                    .unwrap_or_else(|| format!("weak:{}", e.fingerprint()));
                let prior:Option<(String,i64)>=tx.query_row("SELECT fingerprint,message_id FROM ftn_messages WHERE domain=?1 AND area=?2 AND identity_key=?3",params![e.source.domain.as_str(),e.scope(),identity],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                if let Some((hash, mid)) = prior {
                    if hash != e.fingerprint() {
                        return Err(Error::Conflict);
                    }
                    let outcome = if e.text.msgid.is_some() {
                        "duplicate"
                    } else {
                        return Err(Error::Conflict);
                    };
                    receipt(
                        &tx,
                        link_id,
                        &artifact,
                        n as i64,
                        Some(mid),
                        outcome,
                        "ftn-duplicate",
                        now,
                    )?;
                    event(&tx, "duplicate-suppressed", now)?;
                    tx.commit()?;
                    return Ok((outcome.into(), Some(mid)));
                }
                let (conference, recipient) = if let Some(area) = &e.text.area {
                    let map = mapping(&tx, &e.source.domain, area)?;
                    if !map.receive || !map.links.iter().any(|l| l == link_id) {
                        return Err(Error::Denied);
                    }
                    let local = policy.aka(&map.aka)?;
                    if local.endpoint.domain != e.source.domain
                        || local.endpoint.address.zone() != link.remote.address.zone()
                        || e.source.address.zone() != local.endpoint.address.zone()
                    {
                        return Err(Error::Routing);
                    }
                    let active: bool = tx.query_row(
                        "SELECT active FROM message_conferences WHERE conference_id=?1",
                        [map.conference_id],
                        |r| r.get(0),
                    )?;
                    if !active {
                        return Err(Error::Denied);
                    }
                    if policy.local(&e.source).is_some()
                        || (local.endpoint.address.point() == 0
                            && e.text.path.contains(&local.endpoint.address.two_d()))
                    {
                        receipt(
                            &tx, link_id, &artifact, n as i64, None, "loop", "ftn-loop", now,
                        )?;
                        event(&tx, "loop-suppressed", now)?;
                        tx.commit()?;
                        return Ok(("loop".into(), None));
                    }
                    (Some(map.conference_id), None)
                } else if policy.local(&e.destination).is_some() {
                    (None, Some(resolve_recipient(&tx, &e.destination, &e.to)?))
                } else {
                    if !link.transit
                        || e.text.via.len() >= wire::MAX_HOPS
                        || e.text.via.iter().any(|v| {
                            policy.akas.iter().any(|a| {
                                v.split_ascii_whitespace().next().is_some_and(|s| {
                                    s == a.endpoint.to_string()
                                        || s == a.endpoint.address.to_string()
                                })
                            })
                        })
                    {
                        return Err(Error::Routing);
                    }
                    (None, None)
                };
                let parent = resolve_reply(&tx, &e, conference, recipient.as_ref().map(|r| r.0))?;
                let mid = insert_native(&tx, &e, None, recipient, conference, parent, now)?;
                let pid = publish(&tx, &e, mid, Some(link_id), now)?;
                queue(&tx, policy, &pid, &e, mid, Some(link_id), now)?;
                receipt(
                    &tx,
                    link_id,
                    &artifact,
                    n as i64,
                    Some(mid),
                    "imported",
                    "ftn-native-import",
                    now,
                )?;
                event(
                    &tx,
                    if conference.is_some() {
                        "echomail-imported"
                    } else if policy.local(&e.destination).is_some() {
                        "netmail-delivered"
                    } else {
                        "netmail-routed"
                    },
                    now,
                )?;
                tx.commit()?;
                Ok(("imported".into(), Some(mid)))
            })();
            match outcome {
                Ok((outcome, _)) => count_result(&mut result, &outcome),
                Err(
                    reason @ (Error::Codec(_)
                    | Error::Rejected
                    | Error::Denied
                    | Error::Policy
                    | Error::Routing
                    | Error::Conflict),
                ) => {
                    let tx = self.connection.transaction()?;
                    let code = match reason {
                        Error::Routing => "ftn-routing-failure",
                        Error::Denied => "ftn-admission-denied",
                        Error::Policy => "ftn-area-unmapped",
                        Error::Conflict => "ftn-identity-conflict",
                        Error::Codec(_) => "ftn-control-invalid",
                        _ => "ftn-member-rejected",
                    };
                    quarantine(&tx, Some(link_id), &artifact, code, now)?;
                    if matches!(reason, Error::Routing) {
                        event(&tx, "routing-failure", now)?;
                    }
                    receipt(
                        &tx,
                        link_id,
                        &artifact,
                        n as i64,
                        None,
                        "quarantined",
                        code,
                        now,
                    )?;
                    tx.commit()?;
                    result.quarantined += 1;
                }
                Err(e) => return Err(e),
            }
        }
        let tx = self.connection.transaction()?;
        event(&tx, "packet-ingested", now)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn ftn_queue(&self, after: Option<&str>) -> Result<Vec<QueueItem>, Error> {
        let rows=self.connection.prepare("SELECT q.queue_id,d.link_id,d.final_address,d.next_address,d.aka,q.state,q.version,q.artifact_id,q.reason FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE q.queue_id>?1 ORDER BY q.queue_id LIMIT 100")?.query_map([after.unwrap_or("")],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,i64>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,String>(8)?)))?.collect::<Result<Vec<_>,_>>()?;
        rows.into_iter()
            .map(
                |(id, link, final_id, next, aka, state, version, artifact, reason)| {
                    Ok(QueueItem {
                        id,
                        link,
                        final_destination: endpoint(&self.connection, final_id)?,
                        next_hop: endpoint(&self.connection, next)?,
                        aka,
                        state,
                        version,
                        artifact,
                        reason,
                    })
                },
            )
            .collect()
    }
    pub fn build_ftn(
        &mut self,
        store: &dyn NetworkArtifactStore,
        policy: &Policy,
        queue_id: &str,
        expected: i64,
        now: i64,
    ) -> Result<String, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let digest = bind(&tx, policy)?;
        let(pid,lid,aka,stored_digest,mv,version,state,artifact,qv):(String,String,String,String,i64,i64,String,Option<String>,i64)=tx.query_row("SELECT d.publication_id,d.link_id,d.aka,d.policy_digest,d.mapping_version,d.message_version,q.state,q.artifact_id,q.version FROM ftn_routing_decisions d JOIN network_outbound_queue q USING(queue_id) WHERE d.queue_id=?1",[queue_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)))?;
        if qv != expected {
            tx.commit()?;
            return Err(Error::Conflict);
        }
        let (e, mid, _) = load(&tx, &pid)?;
        let link = policy.link(&lid)?;
        let aka = policy.aka(&aka)?;
        let valid:bool=tx.query_row("SELECT lifecycle_state='active' AND state_version=?2 AND CASE WHEN container_kind='conference' THEN visibility='public' AND audience_kind='all-callers' ELSE visibility='private' END FROM messages WHERE message_id=?1",params![mid,version],|r|r.get(0))?;
        let map_ok = if let Some(area) = &e.text.area {
            let m = mapping(&tx, &e.source.domain, area)?;
            m.version == mv && m.send && m.links.contains(&lid)
        } else {
            true
        };
        if !valid
            || !map_ok
            || digest != stored_digest
            || !link.enabled
            || !link.outbound
            || !aka.enabled
            || !matches!(state.as_str(), "pending" | "ready" | "retry")
        {
            tx.execute("UPDATE network_outbound_queue SET state='held',reason='ftn-revalidation',version=version+1 WHERE queue_id=?1 AND state<>'accepted'",[queue_id])?;
            tx.commit()?;
            return Err(Error::Held);
        }
        if let Some(artifact) = artifact {
            tx.commit()?;
            return Ok(artifact);
        }
        let mut e = e;
        if e.text.area.is_some() {
            if aka.endpoint.address.point() == 0 {
                e.text.seen_by.insert(aka.endpoint.address.two_d());
                if e.text.path.last() != Some(&aka.endpoint.address.two_d()) {
                    e.text.path.push(aka.endpoint.address.two_d());
                }
            }
            let targets = tx
                .prepare("SELECT next_address FROM ftn_routing_decisions WHERE publication_id=?1")?
                .query_map([&pid], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for target in targets {
                let a = endpoint(&tx, target)?.address;
                if a.point() == 0 {
                    e.text.seen_by.insert(a.two_d());
                }
            }
            // Points cannot be represented in 2D history; their boss is the distribution anchor,
            // never a substitute for a native point delivery receipt.
            if e.text.seen_by.is_empty() {
                e.text.seen_by.insert(aka.endpoint.address.two_d());
            }
            e.destination = link.remote.clone();
        } else {
            let stamp = chrono::DateTime::from_timestamp(now, 0)
                .ok_or(Error::Rejected)?
                .format("%Y%m%d.%H%M%S")
                .to_string();
            e.text.add_control(&format!(
                "Via {} @{stamp}.UTC SPITFIRE-NG 0.1",
                aka.endpoint
            ));
        }
        let m = PackedMessage {
            origin: e.source.address.two_d(),
            destination: e.destination.address.two_d(),
            attributes: if e.text.area.is_some() { 0 } else { 1 },
            cost: e.cost,
            date: e.date,
            to: e.text.charset.encode(&e.to)?,
            from: e.text.charset.encode(&e.from)?,
            subject: e.text.charset.encode(&e.subject)?,
            text: e.text.encode()?,
        };
        let packet = Packet {
            header: PacketHeader {
                profile: link.profile,
                origin: aka.endpoint.address,
                destination: link.remote.address,
                created: chrono::DateTime::from_timestamp(now, 0)
                    .ok_or(Error::Rejected)?
                    .naive_utc(),
                password: [0; 8],
                product: [0xfe, 0, 0, 1],
                product_data: [0; 4],
                baud: 0,
                spare: [0; 20],
            },
            messages: vec![m],
        };
        let bytes = packet.encode()?;
        tx.commit()?;
        let artifact = self.preserve_artifact(store, &bytes, now)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE network_outbound_queue SET artifact_id=?1,state='ready',version=version+1 WHERE queue_id=?2 AND version=?3 AND state IN ('pending','retry') AND EXISTS(SELECT 1 FROM messages WHERE message_id=?4 AND lifecycle_state='active' AND state_version=?5)",params![artifact,queue_id,expected,mid,version])?!=1{return Err(Error::Conflict)}
        event(
            &tx,
            if e.text.area.is_some() {
                "echomail-exported"
            } else {
                "outbound-packet-built"
            },
            now,
        )?;
        tx.commit()?;
        Ok(artifact)
    }
}
fn resolve_recipient(
    tx: &Transaction<'_>,
    destination: &Endpoint,
    to: &str,
) -> Result<(i64, String), Error> {
    let aid = address_id(tx, destination)?;
    tx.query_row("SELECT a.caller_id,a.alias FROM ftn_mailbox_aliases a JOIN callers c USING(caller_id) WHERE a.address_id=?1 AND a.alias=?2 COLLATE NOCASE AND c.account_state='active'",params![aid,to],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or(Error::Denied)
}
fn resolve_reply(
    tx: &Transaction<'_>,
    e: &Envelope,
    conference: Option<i64>,
    recipient: Option<i64>,
) -> Result<Option<i64>, Error> {
    let Some(reply) = &e.text.reply else {
        return Ok(None);
    };
    let ids=tx.prepare("SELECT f.message_id FROM ftn_messages f JOIN messages m USING(message_id) WHERE f.domain=?1 AND f.msgid=?2 AND m.lifecycle_state='active' AND ((?3 IS NOT NULL AND m.conference_id=?3 AND m.visibility='public' AND f.area=?5) OR (?4 IS NOT NULL AND m.author_caller_id=?4 AND m.container_kind='local-network-mailbox' AND f.destination_address=?6 AND f.recipient=?7 COLLATE NOCASE)) LIMIT 2")?.query_map(params![e.source.domain.as_str(),reply,conference,recipient,e.scope(),address_id(tx,&e.source)?,e.from],|r|r.get(0))?.collect::<Result<Vec<i64>,_>>()?;
    Ok(if ids.len() == 1 {
        ids.first().copied()
    } else {
        None
    })
}
#[allow(clippy::too_many_arguments)]
fn receipt(
    tx: &Transaction<'_>,
    link: &str,
    artifact: &str,
    n: i64,
    mid: Option<i64>,
    outcome: &str,
    reason: &str,
    now: i64,
) -> Result<(), Error> {
    tx.execute(
        "INSERT INTO ftn_import_receipts VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![link, artifact, n, mid, outcome, reason, now],
    )?;
    Ok(())
}
fn count_result(r: &mut TossResult, s: &str) {
    match s {
        "imported" => r.imported += 1,
        "duplicate" => r.duplicates += 1,
        "loop" => r.loops += 1,
        _ => r.quarantined += 1,
    }
}
