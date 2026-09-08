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

//! Durable native Event definitions, clock policy and execution claims.
use crate::RuntimeDatabase;
use chrono::{Datelike, LocalResult, TimeZone};
use chrono_tz::Tz;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Event definition")]
    Invalid,
    #[error("Event changed, running, or unavailable")]
    Conflict,
    #[error("Event storage failure")]
    Storage(#[from] rusqlite::Error),
    #[error("invalid stored Event")]
    Encoding(#[from] serde_json::Error),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    ConferenceHealth,
    Circuitnet {
        network: sf_net::circuitnet::NetworkId,
        node: Option<sf_net::circuitnet::NodeId>,
    },
    Binkp {
        link: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Schedule {
    Manual,
    Interval { seconds: u32 },
    Daily { hour: u8, minute: u8, days: Vec<u8> },
}
impl Schedule {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Manual => Ok(()),
            Self::Interval { seconds } if (5..=2_678_400).contains(seconds) => Ok(()),
            Self::Daily { hour, minute, days }
                if *hour < 24
                    && *minute < 60
                    && !days.is_empty()
                    && days.len() <= 7
                    && days.iter().all(|d| *d < 7)
                    && days.iter().collect::<std::collections::BTreeSet<_>>().len()
                        == days.len() =>
            {
                Ok(())
            }
            _ => Err(Error::Invalid),
        }
    }
    /// First slot strictly after UTC `after`. Sunday is zero. Ambiguous civil
    /// times use the first occurrence; nonexistent civil times are skipped.
    pub fn next_after(&self, after: i64, timezone: Tz) -> Result<Option<i64>, Error> {
        self.validate()?;
        match self {
            Self::Manual => Ok(None),
            Self::Interval { seconds } => Ok(Some(
                after
                    .checked_add(i64::from(*seconds))
                    .ok_or(Error::Invalid)?,
            )),
            Self::Daily { hour, minute, days } => {
                let time = chrono::DateTime::from_timestamp(after, 0)
                    .ok_or(Error::Invalid)?
                    .with_timezone(&timezone);
                let mut date = time.date_naive();
                for _ in 0..15 {
                    if days.contains(&(date.weekday().num_days_from_sunday() as u8)) {
                        let civil = date
                            .and_hms_opt(u32::from(*hour), u32::from(*minute), 0)
                            .ok_or(Error::Invalid)?;
                        let chosen = match timezone.from_local_datetime(&civil) {
                            LocalResult::Single(t) => Some(t),
                            LocalResult::Ambiguous(a, b) => Some(a.min(b)),
                            LocalResult::None => None,
                        };
                        if let Some(t) = chosen.filter(|t| t.timestamp() > after) {
                            return Ok(Some(t.timestamp()));
                        }
                    }
                    date = date.succ_opt().ok_or(Error::Invalid)?;
                }
                Err(Error::Invalid)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExchangePolicy {
    Immediate,
    Scheduled,
    Manual,
    Hybrid,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MissedPolicy {
    RunOnce,
    Skip,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub action: Action,
    pub schedule: Schedule,
    pub timezone: String,
    pub policy: ExchangePolicy,
    pub missed: MissedPolicy,
    pub minimum_spacing_seconds: u32,
}
impl Definition {
    pub fn validate(&self) -> Result<(), Error> {
        if !identifier(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(|c| {
                c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
            })
            || !(5..=86400).contains(&self.minimum_spacing_seconds)
        {
            return Err(Error::Invalid);
        }
        if let Action::Binkp { link } = &self.action {
            if !identifier(link) {
                return Err(Error::Invalid);
            }
        }
        if self.action == Action::ConferenceHealth
            && !matches!(
                self.policy,
                ExchangePolicy::Scheduled | ExchangePolicy::Manual
            )
        {
            return Err(Error::Invalid);
        }
        self.timezone.parse::<Tz>().map_err(|_| Error::Invalid)?;
        self.schedule.validate()?;
        if matches!(
            self.policy,
            ExchangePolicy::Scheduled | ExchangePolicy::Hybrid
        ) && self.schedule == Schedule::Manual
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn next(&self, after: i64) -> Result<Option<i64>, Error> {
        if !self.enabled
            || !matches!(
                self.policy,
                ExchangePolicy::Scheduled | ExchangePolicy::Hybrid
            )
        {
            return Ok(None);
        }
        self.schedule
            .next_after(after, self.timezone.parse().map_err(|_| Error::Invalid)?)
    }
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub definition: Definition,
    pub version: i64,
    pub next_due: Option<i64>,
    pub last_started: Option<i64>,
    pub last_completed: Option<i64>,
    pub last_result: Option<String>,
    pub failures: u32,
    pub running: bool,
    pub requested: bool,
    pub observed_generation: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub action: Action,
    pub run: String,
    pub event: String,
    pub trigger: String,
    pub started: i64,
    pub completed: Option<i64>,
    pub result: Option<String>,
    pub targets: u32,
}
pub struct Claim {
    pub run: String,
    pub definition: Definition,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Succeeded,
    Partial,
    Failed,
    Held,
    Busy,
    Interrupted,
}
impl Outcome {
    pub fn key(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Held => "held",
            Self::Busy => "busy",
            Self::Interrupted => "interrupted",
        }
    }
}
impl RuntimeDatabase {
    pub fn events(&self) -> Result<Vec<Status>, Error> {
        let mut s=self.connection.prepare("SELECT definition,version,next_due,last_started,last_completed,last_result,failures,running,requested,observed_generation FROM scheduled_events ORDER BY id LIMIT 128")?;
        let rows = s
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, bool>(8)?,
                    r.get(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    d,
                    version,
                    next_due,
                    last_started,
                    last_completed,
                    last_result,
                    failures,
                    running,
                    requested,
                    observed_generation,
                )| {
                    Ok(Status {
                        definition: serde_json::from_str(&d)?,
                        version,
                        next_due,
                        last_started,
                        last_completed,
                        last_result,
                        failures,
                        running: running.is_some(),
                        requested,
                        observed_generation,
                    })
                },
            )
            .collect()
    }
    pub fn event_save(&mut self, d: &Definition, expected: i64, now: i64) -> Result<(), Error> {
        d.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT version,running FROM scheduled_events WHERE id=?1",
                [&d.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if prior
            .as_ref()
            .is_some_and(|(v, r)| *v != expected || r.is_some())
            || (prior.is_none() && expected != 0)
        {
            return Err(Error::Conflict);
        }
        if prior.is_none()
            && tx.query_row("SELECT COUNT(*) FROM scheduled_events", [], |r| {
                r.get::<_, i64>(0)
            })? >= 128
        {
            return Err(Error::Conflict);
        }
        // One enabled Event owns a concrete target. This also rejects an all-peer
        // Event overlapping a single-peer Event in the same profile.
        let existing = tx
            .prepare("SELECT definition FROM scheduled_events WHERE id<>?1")?
            .query_map([&d.id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for json in existing {
            let other: Definition = serde_json::from_str(&json)?;
            let overlap = match (&d.action, &other.action) {
                (
                    Action::Circuitnet {
                        network: a,
                        node: x,
                    },
                    Action::Circuitnet {
                        network: b,
                        node: y,
                    },
                ) => a == b && (x.is_none() || y.is_none() || x == y),
                (Action::Binkp { link: a }, Action::Binkp { link: b }) => a == b,
                (Action::ConferenceHealth, Action::ConferenceHealth) => true,
                _ => false,
            };
            if d.enabled && other.enabled && overlap {
                return Err(Error::Conflict);
            }
        }
        let json = serde_json::to_string(d)?;
        let next = d.next(now)?;
        tx.execute("INSERT INTO scheduled_events(id,definition,version,next_due) VALUES(?1,?2,1,?3) ON CONFLICT(id) DO UPDATE SET definition=excluded.definition,version=version+1,next_due=excluded.next_due,requested=0",params![d.id,json,next])?;
        tx.commit()?;
        Ok(())
    }
    pub fn event_run_now(&mut self, id: &str, expected: i64) -> Result<(), Error> {
        let d = self
            .events()?
            .into_iter()
            .find(|s| s.definition.id == id)
            .ok_or(Error::Conflict)?;
        if !d.definition.enabled || d.version != expected {
            return Err(Error::Conflict);
        }
        // Coalescing includes an active run; repeatedly pressing Run Now never
        // queues a second copy behind it.
        if self.connection.execute("UPDATE scheduled_events SET requested=CASE WHEN running IS NULL THEN 1 ELSE 0 END WHERE id=?1 AND version=?2",params![id,expected])?!=1{return Err(Error::Conflict);}
        Ok(())
    }
    pub fn event_history(&self, id: &str) -> Result<Vec<History>, Error> {
        Ok(self.connection.prepare("SELECT run,event,trigger,started,completed,result,targets,action FROM scheduled_event_history WHERE event=?1 ORDER BY started DESC,rowid DESC LIMIT 50")?.query_map([id],|r|Ok(History{action:serde_json::from_str(&r.get::<_,String>(7)?).map_err(|e|rusqlite::Error::FromSqlConversionFailure(7,rusqlite::types::Type::Text,Box::new(e)))?,run:r.get(0)?,event:r.get(1)?,trigger:r.get(2)?,started:r.get(3)?,completed:r.get(4)?,result:r.get(5)?,targets:r.get(6)?}))?.collect::<Result<_,_>>()?)
    }
    pub fn event_recover(&mut self, now: i64) -> Result<(), Error> {
        let definitions = self.events()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE scheduled_event_history SET completed=?1,result='interrupted' WHERE completed IS NULL",[now])?;
        tx.execute("UPDATE scheduled_events SET running=NULL,last_completed=?1,last_result='interrupted',failures=failures+1 WHERE running IS NOT NULL",[now])?;
        for s in definitions {
            if s.definition.missed == MissedPolicy::Skip && s.next_due.is_some_and(|t| t < now) {
                tx.execute(
                    "UPDATE scheduled_events SET next_due=?2 WHERE id=?1",
                    params![s.definition.id, s.definition.next(now)?],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn event_claim(
        &mut self,
        id: &str,
        now: i64,
        generation: i64,
    ) -> Result<Option<Claim>, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        type ClaimRow = (String, Option<i64>, Option<i64>, bool, Option<String>, i64);
        let row:Option<ClaimRow>=tx.query_row("SELECT definition,next_due,last_started,requested,running,observed_generation FROM scheduled_events WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        let Some((json, next, last, requested, running, seen)) = row else {
            return Ok(None);
        };
        let d: Definition = serde_json::from_str(&json)?;
        d.validate()?;
        if !d.enabled
            || running.is_some()
            || last.is_some_and(|t| now < t.saturating_add(i64::from(d.minimum_spacing_seconds)))
        {
            return Ok(None);
        }
        let due = next.is_some_and(|t| t <= now)
            && matches!(d.policy, ExchangePolicy::Scheduled | ExchangePolicy::Hybrid);
        let prompt = generation > seen
            && matches!(d.policy, ExchangePolicy::Immediate | ExchangePolicy::Hybrid);
        if !requested && !due && !prompt {
            return Ok(None);
        }
        let trigger = if requested {
            "manual"
        } else if due {
            "scheduled"
        } else {
            "activity"
        };
        let run = format!("{:032x}", rand::random::<u128>());
        tx.execute("UPDATE scheduled_events SET running=?2,requested=0,last_started=?3,next_due=?4,observed_generation=?5 WHERE id=?1",params![id,run,now,if due{d.next(now)?}else{next},generation])?;
        tx.execute(
            "INSERT INTO scheduled_event_history(run,event,trigger,started,action) VALUES(?1,?2,?3,?4,?5)",
            params![run, id, trigger, now,serde_json::to_string(&d.action)?],
        )?;
        tx.execute("DELETE FROM scheduled_event_history WHERE rowid IN (SELECT rowid FROM scheduled_event_history WHERE completed IS NOT NULL ORDER BY rowid DESC LIMIT -1 OFFSET 4096)",[])?;
        tx.commit()?;
        Ok(Some(Claim { run, definition: d }))
    }
    pub fn event_complete(
        &mut self,
        run: &str,
        outcome: Outcome,
        targets: u32,
        now: i64,
    ) -> Result<(), Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE scheduled_events SET running=NULL,last_completed=MAX(last_started,?2),last_result=?3,failures=CASE WHEN ?3='succeeded' THEN 0 ELSE MIN(failures+1,1000000) END WHERE running=?1",params![run,now,outcome.key()])?!=1{return Err(Error::Conflict);}
        tx.execute("UPDATE scheduled_event_history SET completed=MAX(started,?2),result=?3,targets=?4 WHERE run=?1 AND completed IS NULL",params![run,now,outcome.key(),targets])?;
        tx.commit()?;
        Ok(())
    }
    /// A successful finite session may leave additional batches. Continue draining
    /// at the Event spacing, without inventing a protocol retry on failure.
    pub fn event_continue(&mut self, run: &str) -> Result<(), Error> {
        self.connection.execute("UPDATE scheduled_events SET observed_generation=-1 WHERE id=(SELECT event FROM scheduled_event_history WHERE run=?1 AND result='succeeded') AND running IS NULL",[run])?;
        Ok(())
    }
    pub fn event_has_outbound_work(&self, action: &Action, now: i64) -> Result<bool, Error> {
        if let Action::Circuitnet { network, node } = action {
            if self
                .circuitnet_catalog_pending(network, node.as_ref())
                .map_err(|_| Error::Invalid)?
            {
                return Ok(true);
            }
        }
        Ok(match action {
            Action::Circuitnet{network,node}=>self.connection.query_row("SELECT EXISTS(SELECT 1 FROM network_outbound_queue q JOIN circuitnet_deliveries d USING(queue_id) WHERE d.network=?1 AND (?2 IS NULL OR d.neighbor=?2) AND q.state IN ('pending','ready','retry') AND q.attempts<12 AND (q.next_attempt IS NULL OR q.next_attempt<=?3)) OR EXISTS(SELECT 1 FROM circuitnet_file_deliveries d JOIN circuitnet_file_publications p USING(network,identity) JOIN circuitnet_file_dossiers s ON s.network=d.network AND s.neighbor=d.neighbor AND s.codename=p.codename WHERE d.network=?1 AND (?2 IS NULL OR d.neighbor=?2) AND d.receipt IS NULL AND d.attempts<12 AND s.subscribed=1)",params![network.as_str(),node.as_ref().map(|n|n.as_str()),now],|r|r.get(0))?,
            Action::ConferenceHealth=>self.connection.query_row("SELECT seed_cursor<seed_until OR EXISTS(SELECT 1 FROM conference_health_work) OR EXISTS(SELECT 1 FROM conference_health_dirty) FROM conference_health_config",[],|r|r.get(0))?,
            Action::Binkp{link}=>self.connection.query_row("SELECT EXISTS(SELECT 1 FROM network_outbound_queue q JOIN ftn_routing_decisions d USING(queue_id) WHERE d.link_id=?1 AND q.state IN ('pending','ready','retry') AND (q.next_attempt IS NULL OR q.next_attempt<=?2)) OR EXISTS(SELECT 1 FROM ftn_file_deliveries WHERE link_id=?1 AND held=0 AND accepted_at IS NULL AND attempts<12 AND session_id IS NULL)",params![link,now],|r|r.get(0))?,
        })
    }
    pub fn event_has_activity(&self, action: &Action, now: i64) -> Result<bool, Error> {
        if self.event_has_outbound_work(action, now)? {
            return Ok(true);
        }
        if let Action::Circuitnet { network, node } = action {
            return Ok(self.connection.query_row("SELECT EXISTS(SELECT 1 FROM circuitnet_controls WHERE network=?1 AND direction='outgoing' AND settled=0 AND (?2 IS NULL OR neighbor=?2))",params![network.as_str(),node.as_ref().map(|n|n.as_str())],|r|r.get(0))?);
        }
        Ok(false)
    }
    /// Durable native-commit notification; preparation and exchange are distinct.
    pub fn network_preparation_state(&self) -> Result<(i64, i64), Error> {
        Ok(self.connection.query_row(
            "SELECT generation,prepared_generation FROM network_preparation WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }
    pub fn network_preparation_complete(&mut self, generation: i64) -> Result<(), Error> {
        self.connection.execute("UPDATE network_preparation SET prepared_generation=MAX(prepared_generation,?1) WHERE singleton=1",[generation])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn definition(policy: ExchangePolicy) -> Definition {
        Definition {
            id: "event-a".into(),
            name: "Mail Run".into(),
            enabled: true,
            action: Action::Circuitnet {
                network: sf_net::circuitnet::NetworkId::new("test").unwrap(),
                node: None,
            },
            schedule: Schedule::Interval { seconds: 60 },
            timezone: "America/Phoenix".into(),
            policy,
            missed: MissedPolicy::RunOnce,
            minimum_spacing_seconds: 10,
        }
    }
    fn db() -> (tempfile::TempDir, RuntimeDatabase) {
        let t = tempfile::tempdir().unwrap();
        let mut d = RuntimeDatabase::open(&t.path().join("native.sqlite3")).unwrap();
        d.migrate().unwrap();
        (t, d)
    }
    fn utc(s: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp()
    }
    #[test]
    fn daily_selected_days_and_dst_choose_one_civil_slot() {
        let daily = Schedule::Daily {
            hour: 2,
            minute: 30,
            days: (0..7).collect(),
        };
        let ny = "America/New_York".parse().unwrap();
        assert_eq!(
            daily.next_after(utc("2026-03-08T05:00:00Z"), ny).unwrap(),
            Some(utc("2026-03-09T06:30:00Z"))
        );
        let fall = Schedule::Daily {
            hour: 1,
            minute: 30,
            days: (0..7).collect(),
        };
        assert_eq!(
            fall.next_after(utc("2026-11-01T04:00:00Z"), ny).unwrap(),
            Some(utc("2026-11-01T05:30:00Z"))
        );
        assert_eq!(
            fall.next_after(utc("2026-11-01T05:30:00Z"), ny).unwrap(),
            Some(utc("2026-11-02T06:30:00Z"))
        );
        let weekdays = Schedule::Daily {
            hour: 3,
            minute: 30,
            days: vec![1, 2, 3, 4, 5],
        };
        assert_eq!(
            weekdays
                .next_after(utc("2026-09-05T00:00:00Z"), chrono_tz::UTC)
                .unwrap(),
            Some(utc("2026-09-07T03:30:00Z"))
        );
    }
    #[test]
    fn definitions_fail_closed_on_unknown_actions_schedules_and_unsafe_labels() {
        let mut d = definition(ExchangePolicy::Hybrid);
        d.validate().unwrap();
        d.name = "unsafe\nlabel".into();
        assert!(d.validate().is_err());
        d.name = "ok".into();
        d.timezone = "not-a-timezone".into();
        assert!(d.validate().is_err());
        assert!(
            serde_json::from_str::<Action>(r#"{"kind":"shell","command":"echo nope"}"#).is_err()
        );
        assert!(Schedule::Interval { seconds: 0 }.validate().is_err());
        assert!(Schedule::Daily {
            hour: 24,
            minute: 0,
            days: vec![0]
        }
        .validate()
        .is_err());
        assert!(Schedule::Daily {
            hour: 1,
            minute: 0,
            days: vec![0, 0]
        }
        .validate()
        .is_err());
        assert!(Schedule::Interval { seconds: 60 }
            .next_after(i64::MAX, chrono_tz::UTC)
            .is_err());
    }
    #[test]
    fn scheduled_queue_activity_does_not_run_before_due_and_rollback_cannot_repeat() {
        let (_t, mut d) = db();
        d.event_save(&definition(ExchangePolicy::Scheduled), 0, 100)
            .unwrap();
        assert!(d.event_claim("event-a", 159, 500).unwrap().is_none());
        let run = d.event_claim("event-a", 160, 500).unwrap().unwrap();
        assert!(d.event_claim("event-a", 170, 501).unwrap().is_none());
        d.event_complete(&run.run, Outcome::Succeeded, 1, 161)
            .unwrap();
        assert!(d.event_claim("event-a", 100, 600).unwrap().is_none());
        assert_eq!(d.events().unwrap()[0].next_due, Some(220));
        assert!(d.event_claim("event-a", 219, 600).unwrap().is_none());
        assert!(d.event_claim("event-a", 220, 600).unwrap().is_some());
    }
    #[test]
    fn run_now_coalesces_disabled_is_fenced_and_cas_rejects_stale_edits() {
        let (_t, mut d) = db();
        let mut event = definition(ExchangePolicy::Manual);
        d.event_save(&event, 0, 100).unwrap();
        assert!(d.event_claim("event-a", 1000, 500).unwrap().is_none());
        d.event_run_now("event-a", 1).unwrap();
        d.event_run_now("event-a", 1).unwrap();
        let run = d.event_claim("event-a", 1000, 500).unwrap().unwrap();
        d.event_run_now("event-a", 1).unwrap();
        assert!(d.event_save(&event, 1, 1000).is_err());
        d.event_complete(&run.run, Outcome::Succeeded, 1, 1001)
            .unwrap();
        assert!(d.event_claim("event-a", 1011, 900).unwrap().is_none());
        event.enabled = false;
        d.event_save(&event, 1, 1011).unwrap();
        assert!(d.event_save(&event, 1, 1012).is_err());
        assert!(d.event_run_now("event-a", 2).is_err());
    }
    #[test]
    fn immediate_burst_spacing_and_hybrid_retry_are_different_authorities() {
        let (_t, mut d) = db();
        d.event_save(&definition(ExchangePolicy::Immediate), 0, 100)
            .unwrap();
        let run = d.event_claim("event-a", 100, 50).unwrap().unwrap();
        d.event_complete(&run.run, Outcome::Failed, 1, 101).unwrap();
        assert!(d.event_claim("event-a", 109, 100).unwrap().is_none());
        let run = d.event_claim("event-a", 110, 100).unwrap().unwrap();
        d.event_complete(&run.run, Outcome::Succeeded, 1, 111)
            .unwrap();
        assert!(d.event_claim("event-a", 2000, 100).unwrap().is_none());
        d.event_save(&definition(ExchangePolicy::Hybrid), 1, 2000)
            .unwrap();
        assert!(d.event_claim("event-a", 2059, 100).unwrap().is_none());
        assert!(d.event_claim("event-a", 2060, 100).unwrap().is_some());
    }
    #[test]
    fn restart_catches_up_once_or_skips_and_never_resurrects_a_run() {
        let (t, mut d) = db();
        d.event_save(&definition(ExchangePolicy::Scheduled), 0, 100)
            .unwrap();
        let run = d.event_claim("event-a", 160, 1).unwrap().unwrap();
        drop(d);
        let mut d = RuntimeDatabase::open(&t.path().join("native.sqlite3")).unwrap();
        d.event_recover(1000).unwrap();
        assert!(!d.events().unwrap()[0].running);
        assert_eq!(
            d.event_history("event-a").unwrap()[0].result.as_deref(),
            Some("interrupted")
        );
        assert!(d
            .event_complete(&run.run, Outcome::Succeeded, 1, 1001)
            .is_err());
        let run = d.event_claim("event-a", 1000, 1).unwrap().unwrap();
        d.event_complete(&run.run, Outcome::Succeeded, 1, 1001)
            .unwrap();
        assert_eq!(d.events().unwrap()[0].next_due, Some(1060));
        let mut event = definition(ExchangePolicy::Scheduled);
        event.missed = MissedPolicy::Skip;
        d.event_save(&event, 1, 1100).unwrap();
        d.event_recover(3000).unwrap();
        assert!(d.event_claim("event-a", 3000, 1).unwrap().is_none());
        assert_eq!(d.events().unwrap()[0].next_due, Some(3060));
    }
    #[test]
    fn independent_database_connections_cannot_claim_the_same_event() {
        let (t, mut d) = db();
        d.event_save(&definition(ExchangePolicy::Scheduled), 0, 100)
            .unwrap();
        let mut other = RuntimeDatabase::open(&t.path().join("native.sqlite3")).unwrap();
        assert!(d.event_claim("event-a", 160, 1).unwrap().is_some());
        assert!(other.event_claim("event-a", 160, 1).unwrap().is_none());
    }
    #[test]
    fn history_contains_finite_outcomes_and_survives_database_copy() {
        let (t, mut d) = db();
        d.event_save(&definition(ExchangePolicy::Scheduled), 0, 100)
            .unwrap();
        let run = d.event_claim("event-a", 160, 1).unwrap().unwrap();
        d.event_complete(&run.run, Outcome::Held, 3, 170).unwrap();
        d.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(d);
        let restored = t.path().join("restored.sqlite3");
        std::fs::copy(t.path().join("native.sqlite3"), &restored).unwrap();
        let mut d = RuntimeDatabase::open(&restored).unwrap();
        d.event_recover(180).unwrap();
        let history = d.event_history("event-a").unwrap();
        assert_eq!(history[0].targets, 3);
        assert_eq!(history[0].result.as_deref(), Some("held"));
        let json = serde_json::to_string(&history).unwrap();
        for forbidden in ["password", "body", "certificate", "endpoint"] {
            assert!(!json.contains(forbidden));
        }
        assert_eq!(
            d.events().unwrap()[0].definition,
            definition(ExchangePolicy::Scheduled)
        );
    }
    #[test]
    fn overlapping_enabled_target_schedules_cannot_multiply_polls() {
        let (_t, mut d) = db();
        let original = definition(ExchangePolicy::Hybrid);
        d.event_save(&original, 0, 100).unwrap();
        let mut duplicate = original.clone();
        duplicate.id = "event-b".into();
        duplicate.action = Action::Circuitnet {
            network: sf_net::circuitnet::NetworkId::new("test").unwrap(),
            node: Some(sf_net::circuitnet::NodeId::new("END1").unwrap()),
        };
        assert!(d.event_save(&duplicate, 0, 100).is_err());
        duplicate.enabled = false;
        d.event_save(&duplicate, 0, 100).unwrap();
    }
    #[test]
    fn distribution_examples_are_proposals_without_native_numbers_or_privacy_claims() {
        // C7 promotes the joining example to an implementation-neutral seed;
        // the retained C5 conference proposal remains independently historical.
        let profile: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/circuitnet-ng/config/network-profile.example.json"
        ))
        .unwrap();
        assert_eq!(profile["format"], "circuitnet-ng-joining-seed");
        assert_eq!(profile["version"], 1);
        for field in ["node_id", "parent", "listener", "endpoint", "credentials"] {
            assert!(profile[field].is_null());
        }
        assert_eq!(
            profile["protocol"]["required_catalog_capability"],
            "catalog-sync"
        );
        let catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/circuitnet-ng/config/conferences.proposed.json"
        ))
        .unwrap();
        assert_eq!(catalog["network_policy_approved"], false);
        let mut codes = std::collections::BTreeSet::new();
        for area in catalog["conferences"].as_array().unwrap() {
            let code = area["codename"].as_str().unwrap();
            crate::circuitnet::Codename::new(code).unwrap();
            assert!(codes.insert(code));
            assert!(area.get("conference_id").is_none());
            assert!(area["status"].as_str().unwrap().starts_with("proposed"));
        }
        let security = include_str!("../../../docs/circuitnet-ng/SECURITY.md")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(security.contains("Public conference messages are not end-to-end encrypted"));
        assert!(security.contains("Directed routing does not make a message private"));
        assert!(include_str!("../../../docs/manual/events.md")
            .contains("QWK network build/ingest and custody handoff remain explicit"));
    }
}
