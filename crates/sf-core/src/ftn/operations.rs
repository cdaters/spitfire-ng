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

//! Bounded read projections over existing network authority; no message content.
use super::*;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkSection {
    #[default]
    Overview,
    Links,
    Queues,
    Areas,
    Directory,
    Quarantine,
    Recovery,
}
impl NetworkSection {
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Links,
        Self::Queues,
        Self::Areas,
        Self::Directory,
        Self::Quarantine,
        Self::Recovery,
    ];
    pub const fn key(self) -> &'static str {
        match self {
            Self::Overview => "networks-overview",
            Self::Links => "networks-links",
            Self::Queues => "networks-queues",
            Self::Areas => "networks-areas",
            Self::Directory => "networks-directory",
            Self::Quarantine => "networks-quarantine",
            Self::Recovery => "networks-recovery",
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetworkQuery {
    pub section: NetworkSection,
    pub offset: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkQueueDetail {
    pub id: String,
    pub adapter: String,
    pub link: String,
    pub network: String,
    pub state: String,
    pub version: i64,
    pub attempts: u32,
    pub created_at: i64,
    pub next_attempt: Option<i64>,
    pub reason: String,
    pub artifact: Option<String>,
    pub bytes: Option<i64>,
    pub qwk_destination: Option<String>,
    pub final_destination: Option<Endpoint>,
    pub next_hop: Option<Endpoint>,
    pub aka: Option<String>,
    pub route_reason: String,
    pub kind: String,
    pub publication: String,
    pub last_attempt: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryStatus {
    pub generation: String,
    pub source: String,
    pub domain: String,
    pub date: String,
    pub imported: i64,
    pub records: u32,
    pub issues: u32,
    pub state: String,
    pub priority: u16,
    pub cadence_days: u16,
    pub active: bool,
    pub version: i64,
    pub activated_at: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuarantineStatus {
    pub id: i64,
    pub link: Option<String>,
    pub reason: String,
    pub received: i64,
    pub bytes: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkConference {
    pub id: i64,
    pub number: u16,
    pub name: String,
    pub active: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NetworkPage {
    pub conferences: Vec<NetworkConference>,
    pub query: NetworkQuery,
    pub more: bool,
    pub queues: Vec<NetworkQueueDetail>,
    pub areas: Vec<Mapping>,
    pub directory: Vec<DirectoryStatus>,
    pub quarantine: Vec<QuarantineStatus>,
    pub origins: Vec<OriginState>,
    pub queue_counts: BTreeMap<String, i64>,
    pub ftn_queue_counts: BTreeMap<String, i64>,
    pub counters: BTreeMap<String, i64>,
}
impl RuntimeDatabase {
    /// Static edits cannot orphan relational maps or reinterpret a retained identity.
    pub fn validate_ftn_policy_references(&self, policy: &Policy) -> Result<(), Error> {
        policy.validate()?;
        let rows = self
            .connection
            .prepare("SELECT domain,area,aka FROM ftn_area_mappings")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (domain, area, aka) in rows {
            if policy.aka(&aka)?.endpoint.domain.as_str() != domain {
                return Err(Error::Conflict);
            }
            let links = self
                .connection
                .prepare("SELECT link_id FROM ftn_area_links WHERE domain=?1 AND area=?2")?
                .query_map(params![domain, area], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for link in links {
                if policy.link(&link)?.remote.domain.as_str() != domain {
                    return Err(Error::Conflict);
                }
            }
        }
        for l in &policy.links {
            let prior: Option<(i64, i64)> = self
                .connection
                .query_row(
                    "SELECT remote_address,local_address FROM ftn_link_bindings WHERE link_id=?1",
                    [&l.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((remote, local)) = prior {
                if endpoint(&self.connection, remote)? != l.remote
                    || endpoint(&self.connection, local)? != policy.aka(&l.aka)?.endpoint
                {
                    return Err(Error::Conflict);
                }
            }
        }
        for source in &policy.sources {
            let prior: Option<String> = self
                .connection
                .query_row(
                    "SELECT profile_digest FROM ftn_directory_sources WHERE source_id=?1",
                    [&source.id],
                    |r| r.get(0),
                )
                .optional()?;
            let mut enabled = source.clone();
            enabled.enabled = true;
            if prior.is_some_and(|d| {
                d != sf_net::qwk::digest(
                    &serde_json::to_vec(&enabled).expect("typed source serializes"),
                )
            }) {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }

    pub fn network_page(&self, query: &NetworkQuery) -> Result<NetworkPage, Error> {
        if query.offset > 2_000_000 {
            return Err(Error::Capacity);
        }
        let mut page = NetworkPage {
            query: query.clone(),
            ..Default::default()
        };
        page.queue_counts = self
            .connection
            .prepare("SELECT state,COUNT(*) FROM network_outbound_queue GROUP BY state")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        page.ftn_queue_counts = self.connection.prepare("SELECT d.link_id,COUNT(*) FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE q.state NOT IN ('accepted','cancelled') GROUP BY d.link_id LIMIT 513")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        if page.ftn_queue_counts.len() > 512 {
            return Err(Error::Capacity);
        }
        for (key, sql) in [
            ("quarantine", "SELECT COUNT(*) FROM network_quarantine"),
            ("directory", "SELECT COUNT(*) FROM ftn_directory_active"),
            (
                "ftn-imported",
                "SELECT COUNT(*) FROM ftn_import_receipts WHERE outcome='imported'",
            ),
            (
                "ftn-duplicate",
                "SELECT COUNT(*) FROM ftn_import_receipts WHERE outcome='duplicate'",
            ),
            (
                "ftn-loop",
                "SELECT COUNT(*) FROM ftn_import_receipts WHERE outcome='loop'",
            ),
            (
                "qwk-imported",
                "SELECT COUNT(*) FROM qwk_network_import_receipts WHERE outcome='imported'",
            ),
            (
                "qwk-duplicate",
                "SELECT COUNT(*) FROM qwk_network_import_receipts WHERE outcome='duplicate'",
            ),
            ("received", "SELECT COUNT(*) FROM binkp_custody_receipts"),
        ] {
            page.counters.insert(
                key.into(),
                self.connection.query_row(sql, [], |r| r.get(0))?,
            );
        }
        match query.section {
            NetworkSection::Queues => {
                let mut stmt=self.connection.prepare("SELECT q.queue_id,w.adapter,COALESCE(f.link_id,d.link_id),COALESCE(m.domain,l.network),q.state,q.version,q.attempts,q.created_at,q.next_attempt,q.reason,q.artifact_id,a.byte_length,f.final_address,f.next_address,f.aka,COALESCE(f.reason,'configured-qwk'),CASE WHEN f.queue_id IS NULL THEN 'qwk' WHEN m.area LIKE '@netmail/%' THEN 'netmail' ELSE 'echomail' END,COALESCE(f.publication_id,d.publication_id),(SELECT MAX(occurred_at) FROM network_delivery_attempts t WHERE t.queue_id=q.queue_id),d.destination FROM network_outbound_queue q JOIN network_queue_work w USING(queue_id) LEFT JOIN ftn_routing_decisions f USING(queue_id) LEFT JOIN ftn_messages m ON m.publication_id=f.publication_id LEFT JOIN network_routing_decisions d ON d.decision_id=q.queue_id LEFT JOIN qwk_links l ON l.link_id=d.link_id LEFT JOIN network_artifacts a USING(artifact_id) ORDER BY q.queue_id LIMIT 101 OFFSET ?1")?;
                let rows = stmt
                    .query_map([query.offset], |r| {
                        Ok((
                            NetworkQueueDetail {
                                id: r.get(0)?,
                                adapter: r.get(1)?,
                                link: r.get(2)?,
                                network: r.get(3)?,
                                state: r.get(4)?,
                                version: r.get(5)?,
                                attempts: r.get(6)?,
                                created_at: r.get(7)?,
                                next_attempt: r.get(8)?,
                                reason: r.get(9)?,
                                artifact: r.get(10)?,
                                bytes: r.get(11)?,
                                qwk_destination: r.get(19)?,
                                final_destination: None,
                                next_hop: None,
                                aka: r.get(14)?,
                                route_reason: r.get(15)?,
                                kind: r.get(16)?,
                                publication: r.get(17)?,
                                last_attempt: r.get(18)?,
                            },
                            r.get::<_, Option<i64>>(12)?,
                            r.get::<_, Option<i64>>(13)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                for (mut q, d, n) in rows {
                    q.final_destination = d.map(|id| endpoint(&self.connection, id)).transpose()?;
                    q.next_hop = n.map(|id| endpoint(&self.connection, id)).transpose()?;
                    page.queues.push(q)
                }
                page.more = page.queues.len() > 100;
                page.queues.truncate(100);
            }
            NetworkSection::Areas => {
                page.conferences=self.connection.prepare("SELECT conference_id,conference_number,name,active FROM message_conferences ORDER BY conference_number LIMIT 784")?.query_map([],|r|Ok(NetworkConference{id:r.get(0)?,number:r.get(1)?,name:r.get(2)?,active:r.get(3)?}))?.collect::<Result<_,_>>()?;
                let keys=self.connection.prepare("SELECT domain,area FROM ftn_area_mappings ORDER BY domain,area LIMIT 101 OFFSET ?1")?.query_map([query.offset],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
                page.more = keys.len() > 100;
                for (d, a) in keys.into_iter().take(100) {
                    page.areas
                        .push(super::mail::mapping(&self.connection, &d.parse()?, &a)?)
                }
            }
            NetworkSection::Directory => {
                page.directory=self.connection.prepare("SELECT g.generation_id,g.source_id,s.domain,g.effective_date,g.received_at,g.record_count,(SELECT COUNT(*) FROM ftn_directory_issues i WHERE i.generation_id=g.generation_id),g.state,g.priority,g.cadence_days,COALESCE(a.generation_id=g.generation_id,0),COALESCE(a.version,0),CASE WHEN a.generation_id=g.generation_id THEN a.activated_at END FROM ftn_directory_generations g JOIN ftn_directory_sources s USING(source_id) LEFT JOIN ftn_directory_active a USING(source_id) ORDER BY g.source_id,g.effective_date DESC,g.generation_id LIMIT 101 OFFSET ?1")?.query_map([query.offset],|r|Ok(DirectoryStatus{generation:r.get(0)?,source:r.get(1)?,domain:r.get(2)?,date:r.get(3)?,imported:r.get(4)?,records:r.get(5)?,issues:r.get(6)?,state:r.get(7)?,priority:r.get(8)?,cadence_days:r.get(9)?,active:r.get(10)?,version:r.get(11)?,activated_at:r.get(12)?}))?.collect::<Result<_,_>>()?;
                page.more = page.directory.len() > 100;
                page.directory.truncate(100);
            }
            NetworkSection::Quarantine => {
                page.quarantine=self.connection.prepare("SELECT q.quarantine_id,COALESCE(q.ftn_link,q.link_id),q.reason,q.received_at,a.byte_length FROM network_quarantine q LEFT JOIN network_artifacts a USING(artifact_id) ORDER BY q.quarantine_id DESC LIMIT 101 OFFSET ?1")?.query_map([query.offset],|r|Ok(QuarantineStatus{id:r.get(0)?,link:r.get(1)?,reason:r.get(2)?,received:r.get(3)?,bytes:r.get(4)?}))?.collect::<Result<_,_>>()?;
                page.more = page.quarantine.len() > 100;
                page.quarantine.truncate(100);
            }
            NetworkSection::Recovery => {
                page.origins = self.ftn_origin_states()?;
            }
            _ => {}
        }
        Ok(page)
    }
}
