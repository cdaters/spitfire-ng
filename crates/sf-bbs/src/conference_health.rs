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

//! Local operator transport for native Conference Health, independent of adapters.
use crate::{ApplicationError, BoardRuntime};
use serde::{Deserialize, Serialize};
use sf_core::{
    conference_health::{Page, Query, Settings},
    RuntimeDatabase,
};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Configure { settings: Settings },
    Rollup,
    Schedule,
}
impl Command {
    pub fn capability(&self) -> sf_core::LocalOperatorCapability {
        match self {
            Self::Rollup => sf_core::LocalOperatorCapability::ChangeOnlineConfiguration,
            _ => sf_core::LocalOperatorCapability::ChangeSensitiveConfiguration,
        }
    }
}
pub(crate) fn snapshot(runtime: &BoardRuntime, query: &Query) -> Result<Page, ApplicationError> {
    Ok(RuntimeDatabase::open_read_only(runtime.database_path())?
        .conference_health_page(query, chrono::Utc::now().timestamp())?)
}
pub(crate) fn rollup(
    runtime: &BoardRuntime,
) -> Result<sf_core::conference_health::Rollup, ApplicationError> {
    let mut db = RuntimeDatabase::open(runtime.database_path())?;
    let started = std::time::Instant::now();
    let mut total = sf_core::conference_health::Rollup {
        messages: 0,
        conferences: 0,
        pending: false,
    };
    for _ in 0..16 {
        if runtime.shutdown_in_progress()? {
            total.pending = true;
            break;
        }
        let result = db.conference_health_rollup(chrono::Utc::now().timestamp())?;
        total.messages += result.messages;
        total.conferences += result.conferences;
        total.pending = result.pending;
        if !result.pending || started.elapsed().as_secs() >= 10 {
            break;
        }
    }
    Ok(total)
}
pub(crate) fn dispatch(runtime: &BoardRuntime, command: &Command) -> Result<(), ApplicationError> {
    let now = chrono::Utc::now().timestamp();
    match command {
        Command::Configure { settings } => RuntimeDatabase::open(runtime.database_path())?
            .conference_health_configure(settings, now)?,
        Command::Rollup => {
            rollup(runtime)?;
        }
        Command::Schedule => {
            use sf_core::events::*;
            let mut db = RuntimeDatabase::open(runtime.database_path())?;
            let definition = Definition {
                id: "conference-health".into(),
                name: "Conference Health".into(),
                enabled: true,
                action: Action::ConferenceHealth,
                schedule: Schedule::Interval { seconds: 300 },
                timezone: "UTC".into(),
                policy: ExchangePolicy::Scheduled,
                missed: MissedPolicy::RunOnce,
                minimum_spacing_seconds: 5,
            };
            // Never silently overwrite an operator's existing Event with this ID.
            db.event_save(&definition, 0, now)?;
        }
    }
    runtime.events.notify();
    Ok(())
}
pub fn run(
    config: &std::path::Path,
    args: &[std::ffi::OsString],
) -> Result<String, ApplicationError> {
    let usage = || {
        ApplicationError::Usage(sf_core::text(
            "health-usage",
            &sf_core::LocalizationArgs::new(),
        ))
    };
    let args = args
        .iter()
        .map(|a| a.to_str().ok_or_else(usage))
        .collect::<Result<Vec<_>, _>>()?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| usage())?;
    rt.block_on(async {
        let mut client = crate::OperatorClient::connect(config).await?;
        client.describe_operator_controls().await?;
        if let ["status"] = args.as_slice() {
            return serde_json::to_string_pretty(
                &client.conference_health(Query::default()).await?,
            )
            .map_err(|_| usage());
        }
        if let ["page", offset] = args.as_slice() {
            return serde_json::to_string_pretty(
                &client
                    .conference_health(Query {
                        offset: offset.parse().map_err(|_| usage())?,
                        include_retired: true,
                        ..Query::default()
                    })
                    .await?,
            )
            .map_err(|_| usage());
        }
        let command = match args.as_slice() {
            ["rollup"] => Command::Rollup,
            ["schedule"] => Command::Schedule,
            ["configure", enabled, retention, dormant, bulletin, limit] => {
                let boolean = |s: &str| match s {
                    "on" => Ok(true),
                    "off" => Ok(false),
                    _ => Err(usage()),
                };
                let settings = Settings {
                    enabled: boolean(enabled)?,
                    retention_days: retention.parse().map_err(|_| usage())?,
                    dormant_days: dormant.parse().map_err(|_| usage())?,
                    bulletin: boolean(bulletin)?,
                    bulletin_limit: limit.parse().map_err(|_| usage())?,
                };
                settings.validate()?;
                Command::Configure { settings }
            }
            _ => return Err(usage()),
        };
        let result = client
            .qwk_network_action(
                crate::operator_control::random_token(),
                crate::NetworkAction::ConferenceHealth { request: command },
            )
            .await?;
        if matches!(result, crate::NetworkResult::Rejected { .. }) {
            return Err(usage());
        }
        serde_json::to_string_pretty(&result).map_err(|_| usage())
    })
}
