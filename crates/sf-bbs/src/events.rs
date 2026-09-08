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

//! One daemon-owned scheduler. Native services keep queue and session authority.
use crate::{ApplicationError, BoardRuntime};
use serde::{Deserialize, Serialize};
use sf_core::{
    events::{Action, Definition, Outcome},
    RuntimeDatabase,
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct Wake {
    changed: Mutex<bool>,
    signal: Condvar,
}
impl Wake {
    pub(crate) fn notify(&self) {
        if let Ok(mut c) = self.changed.lock() {
            *c = true;
            self.signal.notify_one();
        }
    }
    fn wait(&self) {
        if let Ok(mut c) = self.changed.lock() {
            if !*c {
                if let Ok((next, _)) = self.signal.wait_timeout(c, Duration::from_secs(5)) {
                    c = next;
                } else {
                    return;
                }
            }
            *c = false;
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Save {
        definition: Definition,
        expected: i64,
    },
    Run {
        id: String,
        expected: i64,
    },
}
impl Command {
    pub fn capability(&self) -> sf_core::LocalOperatorCapability {
        match self {
            Self::Save { .. } => sf_core::LocalOperatorCapability::ChangeSensitiveConfiguration,
            Self::Run { .. } => sf_core::LocalOperatorCapability::NetworkRun,
        }
    }
}
pub(crate) fn dispatch(runtime: &BoardRuntime, command: &Command) -> Result<(), ApplicationError> {
    let mut db = RuntimeDatabase::open(runtime.database_path())?;
    match command {
        Command::Save {
            definition,
            expected,
        } => {
            match &definition.action {
                Action::Circuitnet { network, node } => {
                    let profile = db.circuitnet_status(network)?.profile;
                    let (config, _) = db.circuitnet_live(network)?;
                    if let Some(node) = node {
                        if profile
                            .topology
                            .path(&profile.local, node)
                            .map_err(sf_core::circuitnet::Error::from)?
                            .len()
                            != 2
                        {
                            return Err(sf_core::events::Error::Invalid.into());
                        }
                        if !config.peers.iter().any(|p| &p.node == node) {
                            return Err(sf_core::events::Error::Invalid.into());
                        }
                    }
                }
                Action::Binkp { link } => {
                    let c = runtime.configuration.current()?;
                    c.ftn.link(link)?;
                    c.binkp.link(link)?;
                }
            }
            db.event_save(definition, *expected, chrono::Utc::now().timestamp())?;
        }
        Command::Run { id, expected } => db.event_run_now(id, *expected)?,
    }
    runtime.events.notify();
    Ok(())
}
fn prepare(runtime: &BoardRuntime) -> Result<i64, ApplicationError> {
    let _guard = runtime
        .network_lock
        .lock()
        .map_err(|_| ApplicationError::Coordination("network lock poisoned"))?;
    let c = runtime.configuration.current()?;
    let mut db = RuntimeDatabase::open(runtime.database_path())?;
    db.bind_posting_identity_configuration(&c);
    let (generation, prepared) = db.network_preparation_state()?;
    if generation == prepared {
        return Ok(generation);
    }
    let now = chrono::Utc::now().timestamp();
    let mut complete = true;
    for network in db.circuitnet_profiles()? {
        let profile = db.circuitnet_status(&network)?.profile;
        if !profile.enabled {
            continue;
        }
        while db.circuitnet_files_prepare(&network, now)? == 100 {
            if runtime.shutdown_in_progress()? {
                complete = false;
                break;
            }
        }
        let mut cursor = 0;
        loop {
            // Each native scan is bounded. Continue past ineligible historical
            // rows rather than silently completing only a fixed prefix.
            if runtime.shutdown_in_progress()? {
                complete = false;
                break;
            }
            match db.circuitnet_scan(&profile.network, cursor, now) {
                Ok((_, next)) if next == cursor => break,
                Ok((_, next)) => cursor = next,
                Err(_) => {
                    complete = false;
                    break;
                }
            }
        }
    }
    if db.scan_ftn(&c.ftn, now).is_err() {
        complete = false;
    }
    // QWK has no live connection boundary. Its existing build remains manual;
    // packet custody/handoff must never be inferred from an Event firing.
    if complete {
        db.network_preparation_complete(generation)?;
    }
    Ok(generation)
}
fn execute(runtime: &Arc<BoardRuntime>, action: &Action) -> (Outcome, u32) {
    match action {
        Action::Binkp { link } => (crate::binkp::event_poll(runtime, link), 1),
        Action::Circuitnet { network, node } => {
            let Ok(db) = RuntimeDatabase::open_read_only(runtime.database_path()) else {
                return (Outcome::Failed, 0);
            };
            let Ok((c, _)) = db.circuitnet_live(network) else {
                return (Outcome::Failed, 0);
            };
            let peers: Vec<_> = c
                .peers
                .iter()
                .filter(|p| node.as_ref().is_none_or(|n| n == &p.node) && p.enabled && p.outbound)
                .collect();
            if peers.is_empty() {
                return (Outcome::Held, 0);
            }
            let mut successes = 0;
            let mut worst = Outcome::Held;
            let mut count = 0;
            for peer in peers {
                if runtime.shutdown_in_progress().unwrap_or(true) {
                    return (Outcome::Interrupted, count);
                }
                let last = crate::circuitnet_live::event_poll(runtime, network, &peer.node);
                if matches!(last, Outcome::Failed | Outcome::Interrupted)
                    || (last == Outcome::Busy && worst == Outcome::Held)
                {
                    worst = last;
                }
                count += 1;
                if last == Outcome::Succeeded {
                    successes += 1;
                }
            }
            (
                if successes == count {
                    Outcome::Succeeded
                } else if successes > 0 {
                    Outcome::Partial
                } else {
                    worst
                },
                count,
            )
        }
    }
}
pub(crate) fn start(
    runtime: Arc<BoardRuntime>,
) -> Result<Vec<std::thread::JoinHandle<()>>, ApplicationError> {
    RuntimeDatabase::open(runtime.database_path())?
        .event_recover(chrono::Utc::now().timestamp())?;
    let preparation = runtime.clone();
    let preparer = std::thread::spawn(move || {
        while !preparation.shutdown_in_progress().unwrap_or(true) {
            if prepare(&preparation).is_err() {
                tracing::warn!(
                    result = "storage-or-policy",
                    "Network preparation incomplete"
                );
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    let scheduler = std::thread::spawn(move || loop {
        if runtime.shutdown_in_progress().unwrap_or(true) {
            break;
        }
        let result = (|| -> Result<(), ApplicationError> {
            let mut db = RuntimeDatabase::open(runtime.database_path())?;
            let (generation, _) = db.network_preparation_state()?;
            for event in db.events()? {
                let admission = runtime
                    .shutdown
                    .lock()
                    .map_err(|_| ApplicationError::Coordination("shutdown lock poisoned"))?;
                if admission.phase != crate::ShutdownPhase::Running {
                    break;
                }
                let activity = if db
                    .event_has_activity(&event.definition.action, chrono::Utc::now().timestamp())?
                {
                    generation
                } else {
                    event.observed_generation
                };
                if let Some(claim) = db.event_claim(
                    &event.definition.id,
                    chrono::Utc::now().timestamp(),
                    activity,
                )? {
                    let _work = runtime.live_controls.track();
                    drop(admission);
                    let (outcome, count) = execute(&runtime, &claim.definition.action);
                    db.event_complete(&claim.run, outcome, count, chrono::Utc::now().timestamp())?;
                    if outcome == Outcome::Succeeded
                        && db.event_has_outbound_work(
                            &claim.definition.action,
                            chrono::Utc::now().timestamp(),
                        )?
                    {
                        db.event_continue(&claim.run)?;
                    }
                    tracing::info!(event=%claim.definition.id,result=outcome.key(),targets=count,"Event completed");
                }
            }
            Ok(())
        })();
        if result.is_err() {
            tracing::warn!(
                result = "storage-or-policy",
                "Event scheduler pass incomplete"
            );
        }
        runtime.events.wait();
    });
    Ok(vec![preparer, scheduler])
}

/// Bounded expert Events surface. All live reads and changes use operator IPC.
pub fn run(
    config: &std::path::Path,
    args: &[std::ffi::OsString],
) -> Result<String, ApplicationError> {
    let usage = || ApplicationError::Usage(crate::op("events-usage"));
    let args = args
        .iter()
        .map(|s| s.to_str().ok_or_else(usage))
        .collect::<Result<Vec<_>, _>>()?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| usage())?;
    rt.block_on(async {
        let mut client = crate::OperatorClient::connect(config).await?;
        client.describe_operator_controls().await?;
        let mut snapshot = client
            .networks(sf_core::ftn::NetworkQuery {
                section: sf_core::ftn::NetworkSection::Events,
                offset: 0,
            })
            .await?;
        let command = match args.as_slice() {
            ["list"] => return serde_json::to_string_pretty(&snapshot.events).map_err(|_| usage()),
            ["history", id] => {
                let index = snapshot
                    .events
                    .iter()
                    .position(|e| e.definition.id == *id)
                    .ok_or_else(usage)?;
                snapshot = client
                    .networks(sf_core::ftn::NetworkQuery {
                        section: sf_core::ftn::NetworkSection::Events,
                        offset: index as u32,
                    })
                    .await?;
                return serde_json::to_string_pretty(&snapshot.event_history).map_err(|_| usage());
            }
            ["run", id] => {
                let e = snapshot
                    .events
                    .iter()
                    .find(|e| e.definition.id == *id)
                    .ok_or_else(usage)?;
                Command::Run {
                    id: (*id).into(),
                    expected: e.version,
                }
            }
            ["enable" | "disable", id] => {
                let e = snapshot
                    .events
                    .iter()
                    .find(|e| e.definition.id == *id)
                    .ok_or_else(usage)?;
                let mut d = e.definition.clone();
                d.enabled = args[0] == "enable";
                Command::Save {
                    definition: d,
                    expected: e.version,
                }
            }
            ["save", path] => {
                use std::io::Read;
                let meta = std::fs::symlink_metadata(path).map_err(|_| usage())?;
                if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 16384 {
                    return Err(usage());
                }
                let mut bytes = vec![];
                std::fs::File::open(path)
                    .map_err(|_| usage())?
                    .take(16385)
                    .read_to_end(&mut bytes)
                    .map_err(|_| usage())?;
                if bytes.len() > 16384 {
                    return Err(usage());
                }
                let definition: Definition = serde_json::from_slice(&bytes).map_err(|_| usage())?;
                definition.validate()?;
                let expected = snapshot
                    .events
                    .iter()
                    .find(|e| e.definition.id == definition.id)
                    .map_or(0, |e| e.version);
                Command::Save {
                    definition,
                    expected,
                }
            }
            _ => return Err(usage()),
        };
        let result = client
            .qwk_network_action(
                crate::operator_control::random_token(),
                crate::NetworkAction::Event { request: command },
            )
            .await?;
        if matches!(result, crate::NetworkResult::Rejected { .. }) {
            return Err(usage());
        }
        serde_json::to_string_pretty(&result).map_err(|_| usage())
    })
}
