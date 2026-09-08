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

//! Bounded projections of native public conference activity. Never message authority.
use crate::{MessageActor, MessageBackend, RuntimeDatabase};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const DAY: i64 = 86400;
pub const MESSAGE_BATCH: usize = 1000;
pub const SNAPSHOT_BATCH: usize = 32;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Conference Health settings or clock")]
    Invalid,
    #[error("Conference Health storage failure")]
    Storage(#[from] rusqlite::Error),
    #[error("invalid Conference Health snapshot")]
    Encoding(#[from] serde_json::Error),
    #[error("Conference Health access denied")]
    Access,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub retention_days: u16,
    pub dormant_days: u16,
    pub bulletin: bool,
    pub bulletin_limit: u8,
}
impl Settings {
    pub fn validate(&self) -> Result<(), Error> {
        if !(180..=730).contains(&self.retention_days)
            || ![7, 30, 90].contains(&self.dormant_days)
            || !(1..=20).contains(&self.bulletin_limit)
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub days: u16,
    pub local_posts: u64,
    pub inbound_posts: u64,
    pub replies: u64,
    pub local_posters: u64,
    pub readers: u64,
    pub progress: u64,
    pub active_threads: u64,
    pub thread_starts: u64,
    pub last_message: Option<i64>,
    pub last_local_post: Option<i64>,
    pub last_read: Option<i64>,
    pub incomplete_threads: u64,
}
impl Metrics {
    pub fn posts(&self) -> u64 {
        self.local_posts + self.inbound_posts
    }
    pub fn reads_per_post(&self) -> Option<f64> {
        (self.posts() > 0).then(|| self.progress as f64 / self.posts() as f64)
    }
    pub fn replies_per_active_thread(&self) -> Option<f64> {
        (self.active_threads > 0).then(|| self.replies as f64 / self.active_threads as f64)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    New,
    Healthy,
    Quiet,
    LocallyUnread,
    Dormant,
    Disabled,
    CatchingUp,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trend {
    Rising,
    Stable,
    Falling,
    InsufficientHistory,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Detail {
    pub conference_id: i64,
    pub as_of: i64,
    pub monitoring_since: i64,
    pub windows: Vec<Metrics>,
    pub previous_30: Metrics,
    pub retained_posts: u64,
    pub last_message: Option<i64>,
    pub last_local_post: Option<i64>,
    pub last_read: Option<i64>,
    pub status: Status,
    pub trend: Trend,
    pub reasons: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub adapter: String,
    pub network: String,
    pub area: String,
    pub retired: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub conference_id: i64,
    pub number: u16,
    pub name: String,
    pub retired: bool,
    pub contexts: Vec<Context>,
    pub detail: Option<Detail>,
    pub pending: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub monitoring_since: i64,
    pub now: i64,
    pub pending_messages: u64,
    pub historical_scan_pending: bool,
    pub pending_conferences: u64,
    pub rows: Vec<Row>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Query {
    pub offset: usize,
    pub filter: Filter,
    pub include_retired: bool,
    pub sort_posts: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Filter {
    #[default]
    All,
    Local,
    Ftn,
    Qwk,
    Circuitnet,
    Dormant,
    LocallyUnread,
    Rising,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub snapshot: Snapshot,
    pub total: usize,
    pub offset: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rollup {
    pub messages: usize,
    pub conferences: usize,
    pub pending: bool,
}
fn token(tx: &Transaction<'_>, caller: Option<i64>) -> Result<Option<String>, Error> {
    let Some(caller) = caller else {
        return Ok(None);
    };
    tx.execute("INSERT OR IGNORE INTO conference_health_tokens(caller_id) SELECT caller_id FROM callers WHERE caller_id=?1 AND account_state<>'deleted'",[caller])?;
    Ok(tx
        .query_row(
            "SELECT token FROM conference_health_tokens WHERE caller_id=?1",
            [caller],
            |r| r.get(0),
        )
        .optional()?)
}
fn root(tx: &Transaction<'_>, id: i64, conference: i64) -> Result<(i64, bool), Error> {
    let mut current = id;
    let mut seen = BTreeSet::new();
    for _ in 0..64 {
        if !seen.insert(current) {
            return Ok((id, true));
        }
        let parent: Option<(Option<i64>, Option<i64>)> = tx
            .query_row(
                "SELECT parent_message_id,conference_id FROM messages WHERE message_id=?1",
                [current],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match parent {
            Some((None, Some(c))) if c == conference => return Ok((current, false)),
            Some((Some(p), Some(c))) if c == conference => current = p,
            _ => return Ok((id, true)),
        }
    }
    Ok((id, true))
}
fn project(tx: &Transaction<'_>, id: i64, now: i64, retention: u16) -> Result<(), Error> {
    let old: Option<(i64, i64, Option<String>)> = tx
        .query_row(
            "SELECT conference_id,thread,poster FROM conference_health_messages WHERE message_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((c, _, _)) = &old {
        tx.execute(
            "INSERT OR IGNORE INTO conference_health_dirty VALUES(?1)",
            [c],
        )?;
    }
    tx.execute(
        "DELETE FROM conference_health_messages WHERE message_id=?1",
        [id],
    )?;
    let message:Option<(i64,i64,bool,bool,Option<i64>)>=tx.query_row("SELECT conference_id,placed_at,origin_kind='native',parent_message_id IS NOT NULL,author_caller_id FROM messages WHERE message_id=?1 AND conference_id IS NOT NULL AND visibility='public' AND audience_kind='all-callers' AND lifecycle_state='active'",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    if let Some((c, at, local, reply, author)) = message {
        if at >= now - i64::from(retention) * DAY {
            let (thread, incomplete) = root(tx, id, c)?;
            if old
                .as_ref()
                .is_some_and(|(_, previous, _)| *previous != thread)
            {
                tx.execute("INSERT OR IGNORE INTO conference_health_work SELECT message_id FROM messages WHERE parent_message_id=?1",[id])?;
            }
            let poster = if local {
                token(tx, author)?.or_else(|| old.as_ref().and_then(|(_, _, p)| p.clone()))
            } else {
                None
            };
            tx.execute(
                "INSERT INTO conference_health_messages VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![id, c, at, local, reply, thread, poster, incomplete],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO conference_health_dirty VALUES(?1)",
                [c],
            )?;
        }
    }
    tx.execute(
        "DELETE FROM conference_health_work WHERE message_id=?1",
        [id],
    )?;
    Ok(())
}
fn count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(e),
        )
    })
}
fn metrics(tx: &Transaction<'_>, c: i64, from: i64, to: i64, days: u16) -> Result<Metrics, Error> {
    let mut m=tx.query_row("SELECT COALESCE(SUM(local),0),COALESCE(SUM(1-local),0),COALESCE(SUM(reply),0),COUNT(DISTINCT poster),COUNT(DISTINCT thread),COALESCE(SUM(reply=0),0),MAX(placed_at),MAX(CASE WHEN local=1 THEN placed_at END),COALESCE(SUM(thread_incomplete),0) FROM conference_health_messages WHERE conference_id=?1 AND placed_at>=?2 AND placed_at<?3",params![c,from,to],|r|Ok(Metrics{days,local_posts:count(r,0)?,inbound_posts:count(r,1)?,replies:count(r,2)?,local_posters:count(r,3)?,active_threads:count(r,4)?,thread_starts:count(r,5)?,last_message:r.get(6)?,last_local_post:r.get(7)?,incomplete_threads:count(r,8)?,..Metrics::default()}))?;
    let (readers,progress,last):(u64,u64,Option<i64>)=tx.query_row("SELECT COUNT(DISTINCT token),COALESCE(SUM(progress),0),MAX(last_at) FROM conference_health_reads WHERE conference_id=?1 AND day>=?2 AND day<?3 AND last_at<?4",params![c,from/DAY,(to+DAY-1)/DAY,to],|r|Ok((count(r,0)?,count(r,1)?,r.get(2)?)))?;
    m.readers = readers;
    m.progress = progress;
    m.last_read = last;
    Ok(m)
}
pub fn classify(
    current: &Metrics,
    previous: &Metrics,
    observed_days: i64,
    dormant: &Metrics,
) -> (Status, Trend, Vec<String>) {
    let trend = if observed_days < 60 || current.readers + previous.readers < 5 {
        Trend::InsufficientHistory
    } else if current.readers >= previous.readers.saturating_add(3)
        && current.readers * 4 >= previous.readers * 5
    {
        Trend::Rising
    } else if previous.readers >= current.readers.saturating_add(3)
        && current.readers * 5 <= previous.readers * 4
    {
        Trend::Falling
    } else {
        Trend::Stable
    };
    let status = if current.local_posts > 0 && current.readers > 0 {
        Status::Healthy
    } else if observed_days < i64::from(dormant.days) {
        Status::New
    } else if dormant.inbound_posts > 0 && dormant.progress == 0 {
        Status::LocallyUnread
    } else if dormant.posts() == 0 && dormant.progress == 0 {
        Status::Dormant
    } else {
        Status::Quiet
    };
    (status,trend,vec![format!("{} local posts; {} inbound; {} reader-progress events; {} unique readers in {} days",current.local_posts,current.inbound_posts,current.progress,current.readers,current.days),format!("{} days of observable readership; previous 30-day readers {}",observed_days.max(0),previous.readers),format!("Dormancy window {} days: {} posts, {} reader-progress events",dormant.days,dormant.posts(),dormant.progress)])
}
fn load_settings(connection: &rusqlite::Connection) -> Result<Settings, Error> {
    Ok(connection.query_row("SELECT enabled,retention_days,dormant_days,bulletin,bulletin_limit FROM conference_health_config WHERE singleton=1",[],|r|Ok(Settings{enabled:r.get(0)?,retention_days:r.get(1)?,dormant_days:r.get(2)?,bulletin:r.get(3)?,bulletin_limit:r.get(4)?}))?)
}
impl RuntimeDatabase {
    pub fn conference_health_settings(&self) -> Result<Settings, Error> {
        load_settings(&self.connection)
    }
    pub fn conference_health_configure(
        &mut self,
        settings: &Settings,
        now: i64,
    ) -> Result<(), Error> {
        settings.validate()?;
        if now < 0 {
            return Err(Error::Invalid);
        }
        let tx = self.connection.transaction()?;
        tx.execute("UPDATE conference_health_config SET monitoring_since=CASE WHEN enabled=0 AND ?1=1 THEN ?6 ELSE monitoring_since END,seed_cursor=CASE WHEN enabled=0 AND ?1=1 THEN 0 ELSE seed_cursor END,seed_until=MAX(seed_until,COALESCE((SELECT MAX(message_id) FROM messages),0)),enabled=?1,retention_days=?2,dormant_days=?3,bulletin=?4,bulletin_limit=?5",params![settings.enabled,settings.retention_days,settings.dormant_days,settings.bulletin,settings.bulletin_limit,now])?;
        tx.execute("INSERT OR IGNORE INTO conference_health_dirty SELECT conference_id FROM message_conferences",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn conference_health_rollup(&mut self, now: i64) -> Result<Rollup, Error> {
        if now < 0 {
            return Err(Error::Invalid);
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let settings = load_settings(&tx)?;
        if !settings.enabled {
            return Ok(Rollup {
                messages: 0,
                conferences: 0,
                pending: false,
            });
        }
        let (cursor,until,since,last):(i64,i64,i64,Option<i64>)=tx.query_row("SELECT seed_cursor,seed_until,monitoring_since,last_rollup FROM conference_health_config",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        // Rollback never manufactures an event in the future: invalidate projections
        // and omit future native placement until the clock catches up.
        if last.is_some_and(|n| n > now) {
            tx.execute("DELETE FROM conference_health_snapshots", [])?;
            tx.execute("INSERT OR IGNORE INTO conference_health_dirty SELECT conference_id FROM message_conferences",[])?;
        }
        let ids=tx.prepare("SELECT message_id FROM messages WHERE message_id>?1 AND message_id<=?2 ORDER BY message_id LIMIT ?3")?.query_map(params![cursor,until,MESSAGE_BATCH as i64],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
        let mut messages = ids.len();
        for id in &ids {
            project(&tx, *id, now, settings.retention_days)?;
        }
        let next = if ids.len() < MESSAGE_BATCH {
            until
        } else {
            *ids.last().unwrap_or(&until)
        };
        tx.execute("UPDATE conference_health_config SET seed_cursor=?1", [next])?;
        let work = tx
            .prepare("SELECT message_id FROM conference_health_work ORDER BY message_id LIMIT ?1")?
            .query_map([(MESSAGE_BATCH - messages) as i64], |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        messages += work.len();
        for id in work {
            project(&tx, id, now, settings.retention_days)?;
        }
        let cutoff = now - i64::from(settings.retention_days) * DAY;
        tx.execute("INSERT OR IGNORE INTO conference_health_dirty SELECT DISTINCT conference_id FROM conference_health_messages WHERE message_id IN(SELECT message_id FROM conference_health_messages WHERE placed_at<?1 LIMIT 1000)",[cutoff])?;
        tx.execute("DELETE FROM conference_health_messages WHERE message_id IN(SELECT message_id FROM conference_health_messages WHERE placed_at<?1 LIMIT 1000)",[cutoff])?;
        tx.execute("DELETE FROM conference_health_reads WHERE rowid IN(SELECT rowid FROM conference_health_reads WHERE day<?1 LIMIT 1000)",[cutoff/DAY])?;
        if last.is_none_or(|t| t / 3600 != now / 3600) {
            tx.execute("INSERT OR IGNORE INTO conference_health_dirty SELECT conference_id FROM message_conferences",[])?;
        }
        let dirty=tx.prepare("SELECT d.conference_id,MAX(?1,COALESCE(unixepoch(c.created_at),?1)) FROM conference_health_dirty d JOIN message_conferences c USING(conference_id) ORDER BY COALESCE((SELECT as_of FROM conference_health_snapshots s WHERE s.conference_id=d.conference_id),0),d.conference_id LIMIT ?2")?.query_map(params![since,SNAPSHOT_BATCH as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
        let endday = (now / DAY + 1) * DAY;
        for (c, start) in &dirty {
            let mut windows = Vec::new();
            for days in [7, 30, 90] {
                windows.push(metrics(
                    &tx,
                    *c,
                    endday - i64::from(days) * DAY,
                    now + 1,
                    days,
                )?);
            }
            let previous = metrics(&tx, *c, endday - 60 * DAY, endday - 30 * DAY, 30)?;
            let dormant = &windows[match settings.dormant_days {
                7 => 0,
                90 => 2,
                _ => 1,
            }];
            let (status, trend, reasons) =
                classify(&windows[1], &previous, (now - start) / DAY, dormant);
            let all = metrics(&tx, *c, cutoff, now + 1, settings.retention_days)?;
            let detail = Detail {
                conference_id: *c,
                as_of: now,
                monitoring_since: *start,
                retained_posts: all.posts(),
                last_message: all.last_message,
                last_local_post: all.last_local_post,
                last_read: all.last_read,
                windows,
                previous_30: previous,
                status,
                trend,
                reasons,
            };
            tx.execute("INSERT INTO conference_health_snapshots VALUES(?1,?2,?3) ON CONFLICT(conference_id) DO UPDATE SET as_of=excluded.as_of,snapshot=excluded.snapshot",params![c,now,serde_json::to_string(&detail)?])?;
            tx.execute(
                "DELETE FROM conference_health_dirty WHERE conference_id=?1",
                [c],
            )?;
        }
        tx.execute("UPDATE conference_health_config SET last_rollup=?1", [now])?;
        let pending:bool=tx.query_row("SELECT seed_cursor<seed_until OR EXISTS(SELECT 1 FROM conference_health_work) OR EXISTS(SELECT 1 FROM conference_health_dirty) FROM conference_health_config",[],|r|r.get(0))?;
        tx.commit()?;
        Ok(Rollup {
            messages,
            conferences: dirty.len(),
            pending,
        })
    }
    pub fn conference_health_snapshot(&self, now: i64) -> Result<Snapshot, Error> {
        let settings = self.conference_health_settings()?;
        let (since,seed,pending,dirty):(i64,bool,u64,u64)=self.connection.query_row("SELECT monitoring_since,seed_cursor<seed_until,(SELECT COUNT(*) FROM conference_health_work),(SELECT COUNT(*) FROM conference_health_dirty) FROM conference_health_config",[],|r|Ok((r.get(0)?,r.get(1)?,count(r,2)?,count(r,3)?)))?;
        let mut rows = Vec::new();
        let raw=self.connection.prepare("SELECT c.conference_id,c.conference_number,c.name,c.active,s.snapshot,EXISTS(SELECT 1 FROM conference_health_dirty d WHERE d.conference_id=c.conference_id) FROM message_conferences c LEFT JOIN conference_health_snapshots s USING(conference_id) ORDER BY c.conference_number")?.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,u16>(1)?,r.get::<_,String>(2)?,r.get::<_,bool>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,bool>(5)?)))?.collect::<Result<Vec<_>,_>>()?;
        for (id, number, name, active, s, changed) in raw {
            let mut detail: Option<Detail> = s.map(|s| serde_json::from_str(&s)).transpose()?;
            let pending = seed || pending > 0 || changed;
            if let Some(d) = &mut detail {
                if d.as_of > now {
                    detail = None;
                } else if !settings.enabled {
                    d.status = Status::Disabled;
                } else if pending {
                    d.status = Status::CatchingUp;
                }
            }
            let contexts = self.health_contexts(id)?;
            let retired = !active || (!contexts.is_empty() && contexts.iter().all(|c| c.retired));
            rows.push(Row {
                conference_id: id,
                number,
                name,
                retired,
                contexts,
                detail,
                pending,
            });
        }
        Ok(Snapshot {
            settings,
            monitoring_since: since,
            now,
            pending_messages: pending,
            historical_scan_pending: seed,
            pending_conferences: dirty,
            rows,
        })
    }
    fn health_contexts(&self, conference: i64) -> Result<Vec<Context>, Error> {
        let mut contexts = Vec::new();
        for (sql,adapter) in [
            ("SELECT domain,area,(receive=0 AND send=0) FROM ftn_area_mappings WHERE conference_id=?1","ftn"),
            ("SELECT DISTINCT l.network,m.area,(m.enabled=0 OR l.enabled=0) FROM qwk_link_mappings m JOIN qwk_links l USING(link_id) WHERE m.conference_id=?1","qwk"),
            ("SELECT m.network,m.codename,COALESCE((SELECT json_extract(e.value,'$.status')='retired' FROM circuitnet_catalog_choices c JOIN circuitnet_catalog_revisions r ON r.network=c.network,json_each(r.object,'$.body.entries') e WHERE c.conference_id=m.conference_id AND c.network=m.network AND c.identity=json_extract(e.value,'$.id') AND json_extract(e.value,'$.codename')=m.codename AND r.revision=(SELECT MAX(revision) FROM circuitnet_catalog_revisions WHERE network=m.network)),0) FROM circuitnet_mappings m WHERE m.conference_id=?1","circuitnet")
        ] {
            contexts.extend(self.connection.prepare(sql)?.query_map([conference],|r|Ok(Context{adapter:adapter.into(),network:r.get(0)?,area:r.get(1)?,retired:r.get(2)?}))?.collect::<Result<Vec<_>,_>>()?);
        }
        Ok(contexts)
    }
    pub fn conference_health_page(&self, query: &Query, now: i64) -> Result<Page, Error> {
        if query.offset > 784 {
            return Err(Error::Invalid);
        }
        let mut snapshot = self.conference_health_snapshot(now)?;
        snapshot.rows.retain(|r| {
            (query.include_retired || !r.retired)
                && match query.filter {
                    Filter::All => true,
                    Filter::Local => r.contexts.is_empty(),
                    Filter::Ftn => r.contexts.iter().any(|c| c.adapter == "ftn"),
                    Filter::Qwk => r.contexts.iter().any(|c| c.adapter == "qwk"),
                    Filter::Circuitnet => r.contexts.iter().any(|c| c.adapter == "circuitnet"),
                    Filter::Dormant => r
                        .detail
                        .as_ref()
                        .is_some_and(|d| d.status == Status::Dormant),
                    Filter::LocallyUnread => r
                        .detail
                        .as_ref()
                        .is_some_and(|d| d.status == Status::LocallyUnread),
                    Filter::Rising => r.detail.as_ref().is_some_and(|d| d.trend == Trend::Rising),
                }
        });
        snapshot.rows.sort_by_key(|r| {
            (
                std::cmp::Reverse(
                    r.detail
                        .as_ref()
                        .map(|d| {
                            if query.sort_posts {
                                d.windows[1].posts()
                            } else {
                                d.windows[1].readers
                            }
                        })
                        .unwrap_or(0),
                ),
                r.number,
            )
        });
        let total = snapshot.rows.len();
        snapshot.rows = snapshot
            .rows
            .into_iter()
            .skip(query.offset)
            .take(32)
            .collect();
        Ok(Page {
            snapshot,
            total,
            offset: query.offset,
        })
    }
    pub fn validate_conference_health(&self) -> Result<(), Error> {
        self.conference_health_settings()?.validate()?;
        let bad:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM conference_health_reads WHERE progress<1 OR day<0 OR last_at<0 OR length(token)<>32) OR EXISTS(SELECT 1 FROM conference_health_messages WHERE placed_at<0 OR local NOT IN(0,1) OR reply NOT IN(0,1))",[],|r|r.get(0))?;
        if bad {
            return Err(Error::Invalid);
        }
        for s in self
            .connection
            .prepare("SELECT conference_id,as_of,snapshot FROM conference_health_snapshots")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
        {
            let (id, as_of, json) = s?;
            let d: Detail = serde_json::from_str(&json)?;
            if d.conference_id != id
                || d.as_of != as_of
                || as_of < 0
                || d.monitoring_since < 0
                || d.previous_30.days != 30
                || d.windows.len() != 3
                || d.windows.iter().map(|m| m.days).collect::<Vec<_>>() != [7, 30, 90]
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    /// Caller-specific projection: authorization is reapplied on every rendering.
    pub fn conference_health_hot(&self, actor: MessageActor, now: i64) -> Result<Vec<Row>, Error> {
        let snapshot = self.conference_health_snapshot(now)?;
        if !snapshot.settings.enabled || !snapshot.settings.bulletin {
            return Ok(Vec::new());
        }
        let allowed = self
            .conferences(actor)
            .map_err(|_| Error::Access)?
            .into_iter()
            .filter(|c| c.public_only)
            .map(|c| c.id.get())
            .collect::<BTreeSet<_>>();
        let mut rows = snapshot
            .rows
            .into_iter()
            .filter(|r| {
                allowed.contains(&r.conference_id) && !r.retired && !r.pending && r.detail.is_some()
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|r| {
            std::cmp::Reverse(r.detail.as_ref().map(|d| d.windows[1].readers).unwrap_or(0))
        });
        rows.truncate(snapshot.settings.bulletin_limit as usize);
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "conference_health_tests.rs"]
mod tests;
