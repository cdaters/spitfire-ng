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

//! Immutable directory observations and atomic active-generation selection.
use super::*;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    pub source: String,
    pub state: String,
    pub date: String,
    pub records: u32,
    pub issues: u32,
    pub artifact: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectoryLookup {
    pub endpoint: Endpoint,
    pub source: String,
    pub generation: String,
    pub date: String,
    pub stale: bool,
    pub keyword: String,
    pub system: String,
    pub location: String,
    pub sysop: String,
    pub phone: String,
    pub flags: Vec<String>,
    pub services: Vec<wire::directory::InternetService>,
    pub shadowed_sources: Vec<String>,
}
impl RuntimeDatabase {
    pub fn ingest_ftn_directory(
        &mut self,
        store: &dyn NetworkArtifactStore,
        policy: &Policy,
        source: &str,
        date: chrono::NaiveDate,
        bytes: &[u8],
        now: i64,
    ) -> Result<Generation, Error> {
        policy.validate()?;
        if !policy.enabled {
            return Err(Error::Denied);
        }
        let s = policy
            .sources
            .iter()
            .find(|s| s.id == source && s.enabled)
            .ok_or(Error::Policy)?;
        if bytes.len() > wire::MAX_PACKET {
            return Err(Error::Capacity);
        }
        let _permit = store.admit_import()?;
        let artifact = self.preserve_artifact(store, bytes, now)?;
        let profile = wire::directory::DirectoryProfile {
            format: s.format,
            charset: s.charset,
            date,
            default_zone: s.default_zone,
            require_crc: s.require_crc,
        };
        let candidate = wire::directory::parse(bytes, &profile);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let digest = sf_net::qwk::digest(&serde_json::to_vec(s).map_err(|_| Error::Policy)?);
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT domain,profile_digest FROM ftn_directory_sources WHERE source_id=?1",
                [source],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if prior.is_some_and(|(d, p)| d != s.domain.as_str() || p != digest) {
            return Err(Error::Conflict);
        }
        tx.execute(
            "INSERT OR IGNORE INTO ftn_directory_sources VALUES(?1,?2,?3)",
            params![source, s.domain.as_str(), digest],
        )?;
        let old:Option<(String,String,u32)>=tx.query_row("SELECT generation_id,state,record_count FROM ftn_directory_generations WHERE source_id=?1 AND artifact_id=?2 AND effective_date=?3",params![source,artifact,date.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((id, state, records)) = old {
            let issues = tx.query_row(
                "SELECT COUNT(*) FROM ftn_directory_issues WHERE generation_id=?1",
                [&id],
                |r| r.get(0),
            )?;
            tx.commit()?;
            return Ok(Generation {
                id,
                source: source.into(),
                state,
                date: date.to_string(),
                records,
                issues,
                artifact,
            });
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM ftn_directory_generations WHERE source_id=?1",
            [source],
            |r| r.get(0),
        )?;
        if count >= 8 {
            return Err(Error::Capacity);
        }
        let (candidate, parse_failed) = match candidate {
            Ok(c) => (c, false),
            Err(_) => (
                wire::directory::DirectoryCandidate {
                    entries: vec![],
                    issues: vec![wire::directory::DirectoryIssue {
                        line: 0,
                        reason: "parse-or-checksum-failed".into(),
                    }],
                    checksum: None,
                },
                true,
            ),
        };
        let valid = !parse_failed && candidate.issues.is_empty();
        let generation = id();
        let state = if valid { "validated" } else { "rejected" };
        tx.execute(
            "INSERT INTO ftn_directory_generations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                generation,
                source,
                artifact,
                date.to_string(),
                now,
                format!("{:?}", s.format),
                s.charset.identifier(),
                candidate.checksum,
                state,
                s.priority,
                s.cadence_days,
                candidate.entries.len() as i64
            ],
        )?;
        for e in &candidate.entries {
            let aid = address_id(
                &tx,
                &Endpoint {
                    domain: s.domain.clone(),
                    address: e.address,
                },
            )?;
            tx.execute(
                "INSERT INTO ftn_directory_records VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    generation, aid, e.keyword, e.system, e.location, e.sysop, e.phone, e.speed,
                    e.line, e.raw
                ],
            )?;
            for (n, f) in e.flags.iter().enumerate() {
                tx.execute(
                    "INSERT INTO ftn_directory_flags VALUES(?1,?2,?3,?4)",
                    params![generation, aid, n as i64, f],
                )?;
            }
            for (n, f) in e.services.iter().enumerate() {
                tx.execute(
                    "INSERT INTO ftn_directory_services VALUES(?1,?2,?3,?4,?5,?6)",
                    params![generation, aid, n as i64, f.protocol, f.host, f.port],
                )?;
            }
        }
        for (n, issue) in candidate.issues.iter().enumerate() {
            tx.execute(
                "INSERT INTO ftn_directory_issues VALUES(?1,?2,?3,?4)",
                params![generation, n as i64, issue.line, issue.reason],
            )?;
        }
        if !valid {
            quarantine(&tx, None, &artifact, "ftn-directory-rejected", now)?;
            event(&tx, "directory-import-failed", now)?;
        }
        tx.commit()?;
        Ok(Generation {
            id: generation,
            source: source.into(),
            state: state.into(),
            date: date.to_string(),
            records: candidate.entries.len() as u32,
            issues: candidate.issues.len() as u32,
            artifact,
        })
    }
    pub fn activate_ftn_directory(
        &mut self,
        policy: &Policy,
        principal: &str,
        generation: &str,
        expected: i64,
        now: i64,
    ) -> Result<(), Error> {
        policy.validate()?;
        if !policy.enabled || expected < 0 {
            return Err(Error::Policy);
        }
        let next = expected.checked_add(1).ok_or(Error::Capacity)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (source, state): (String, String) = tx.query_row(
            "SELECT source_id,state FROM ftn_directory_generations WHERE generation_id=?1",
            [generation],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if state != "validated" || !policy.sources.iter().any(|s| s.id == source && s.enabled) {
            return Err(Error::Rejected);
        }
        let source_policy = policy
            .sources
            .iter()
            .find(|s| s.id == source && s.enabled)
            .ok_or(Error::Policy)?;
        let profile_digest: String = tx.query_row(
            "SELECT profile_digest FROM ftn_directory_sources WHERE source_id=?1",
            [&source],
            |r| r.get(0),
        )?;
        if profile_digest
            != sf_net::qwk::digest(&serde_json::to_vec(source_policy).map_err(|_| Error::Policy)?)
        {
            return Err(Error::Conflict);
        }
        let enabled: BTreeMap<String, String> = policy
            .sources
            .iter()
            .filter(|s| s.enabled)
            .map(|s| {
                Ok((
                    s.id.clone(),
                    sf_net::qwk::digest(&serde_json::to_vec(s).map_err(|_| Error::Policy)?),
                ))
            })
            .collect::<Result<_, Error>>()?;
        let conflicts:Vec<(String,String)>=tx.prepare("SELECT DISTINCT a.source_id,s.profile_digest FROM ftn_directory_records candidate JOIN ftn_directory_generations cg ON cg.generation_id=candidate.generation_id JOIN ftn_directory_records old ON old.address_id=candidate.address_id JOIN ftn_directory_active a ON a.generation_id=old.generation_id JOIN ftn_directory_generations og ON og.generation_id=a.generation_id JOIN ftn_directory_sources s ON s.source_id=a.source_id WHERE candidate.generation_id=?1 AND a.source_id<>?2 AND cg.priority=og.priority AND candidate.raw<>old.raw")?.query_map(params![generation,source],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<_,_>>()?;
        if conflicts
            .iter()
            .any(|(id, digest)| enabled.get(id) == Some(digest))
        {
            let artifact: String = tx.query_row(
                "SELECT artifact_id FROM ftn_directory_generations WHERE generation_id=?1",
                [generation],
                |r| r.get(0),
            )?;
            quarantine(&tx, None, &artifact, "ftn-directory-conflict", now)?;
            audit(&tx, principal, "ftn.directory-conflict", now)?;
            tx.commit()?;
            return Err(Error::DirectoryConflict);
        }
        let prior: i64 = tx
            .query_row(
                "SELECT version FROM ftn_directory_active WHERE source_id=?1",
                [&source],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if prior != expected {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO ftn_directory_active VALUES(?1,?2,?3,?4) ON CONFLICT(source_id) DO UPDATE SET generation_id=excluded.generation_id,version=excluded.version,activated_at=excluded.activated_at",params![source,generation,next,now])?;
        audit(&tx, principal, "ftn.directory-activated", now)?;
        event(&tx, "directory-activated", now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn lookup_ftn_directory(
        &self,
        policy: &Policy,
        e: &Endpoint,
        now: i64,
    ) -> Result<Option<DirectoryLookup>, Error> {
        policy.validate()?;
        let a = e.address;
        let aid:Option<i64>=self.connection.query_row("SELECT address_id FROM ftn_addresses WHERE domain=?1 AND zone=?2 AND net=?3 AND node=?4 AND point=?5",params![e.domain.as_str(),a.zone(),a.net(),a.node(),a.point()],|r|r.get(0)).optional()?;
        let Some(aid) = aid else { return Ok(None) };
        let rows=self.connection.prepare("SELECT s.source_id,g.generation_id,g.effective_date,g.priority,g.cadence_days,r.keyword,r.system,r.location,r.sysop,r.phone,r.raw FROM ftn_directory_active a JOIN ftn_directory_sources s USING(source_id) JOIN ftn_directory_generations g USING(generation_id) JOIN ftn_directory_records r USING(generation_id) WHERE r.address_id=?1 ORDER BY g.priority,s.source_id")?.query_map([aid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u16>(3)?,r.get::<_,u16>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,Vec<u8>>(10)?)))?.collect::<Result<Vec<_>,_>>()?;
        let mut usable = vec![];
        for row in rows {
            let Some(source) = policy
                .sources
                .iter()
                .find(|s| s.id == row.0 && s.enabled && s.domain == e.domain)
            else {
                continue;
            };
            let digest: String = self.connection.query_row(
                "SELECT profile_digest FROM ftn_directory_sources WHERE source_id=?1",
                [&row.0],
                |r| r.get(0),
            )?;
            if digest
                != sf_net::qwk::digest(&serde_json::to_vec(source).map_err(|_| Error::Policy)?)
            {
                continue;
            }
            let date = chrono::NaiveDate::parse_from_str(&row.2, "%Y-%m-%d")
                .map_err(|_| Error::Rejected)?;
            let age = now.saturating_sub(
                date.and_hms_opt(0, 0, 0)
                    .ok_or(Error::Rejected)?
                    .and_utc()
                    .timestamp(),
            );
            if age < 0 || age > i64::from(row.4) * 86400 * 4 {
                continue;
            }
            usable.push((row, age));
        }
        let Some((first, age)) = usable.first() else {
            return Ok(None);
        };
        if usable
            .iter()
            .skip(1)
            .any(|(r, _)| r.3 == first.3 && r.10 != first.10)
        {
            return Err(Error::DirectoryConflict);
        }
        let flags=self.connection.prepare("SELECT flag FROM ftn_directory_flags WHERE generation_id=?1 AND address_id=?2 ORDER BY ordinal")?.query_map(params![first.1,aid],|r|r.get(0))?.collect::<Result<_,_>>()?;
        let services=self.connection.prepare("SELECT protocol,host,port FROM ftn_directory_services WHERE generation_id=?1 AND address_id=?2 ORDER BY ordinal")?.query_map(params![first.1,aid],|r|Ok(wire::directory::InternetService{protocol:r.get(0)?,host:r.get(1)?,port:r.get(2)?}))?.collect::<Result<_,_>>()?;
        Ok(Some(DirectoryLookup {
            endpoint: e.clone(),
            source: first.0.clone(),
            generation: first.1.clone(),
            date: first.2.clone(),
            stale: *age > i64::from(first.4) * 86400 * 2,
            keyword: first.5.clone(),
            system: first.6.clone(),
            location: first.7.clone(),
            sysop: first.8.clone(),
            phone: first.9.clone(),
            flags,
            services,
            shadowed_sources: usable.iter().skip(1).map(|(r, _)| r.0.clone()).collect(),
        }))
    }
}
