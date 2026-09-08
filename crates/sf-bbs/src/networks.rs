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

//! Privacy-safe operator projections; existing services remain authoritative.
use crate::{ApplicationError, BoardRuntime};
use serde::{Deserialize, Serialize};
use sf_core::{ftn, RuntimeDatabase};
pub const NETWORKS_MINOR: u16 = 12;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub events: Vec<sf_core::events::Status>,
    #[serde(default)]
    pub event_history: Vec<sf_core::events::History>,
    #[serde(default)]
    pub circuitnet: Vec<crate::circuitnet_live::Status>,
    pub page: ftn::NetworkPage,
    pub ftn: ftn::Policy,
    pub transport: ftn::BinkpPolicy,
    pub binkp: crate::binkp::Status,
    pub qwk: Vec<sf_core::qwk_network::LinkStatus>,
    pub now: i64,
    #[serde(default)]
    pub files: Option<ftn::files::FileStatus>,
}
pub(crate) fn snapshot(
    runtime: &BoardRuntime,
    query: &ftn::NetworkQuery,
) -> Result<Snapshot, ApplicationError> {
    let config = runtime.configuration.current()?;
    let mut db = RuntimeDatabase::open_read_only(runtime.database_path())?;
    db.bind_posting_identity_configuration(&config);
    let mut result = Snapshot {
        events: db.events()?,
        event_history: if query.section == ftn::NetworkSection::Events {
            db.events()?
                .get(query.offset as usize)
                .map(|e| db.event_history(&e.definition.id))
                .transpose()?
                .unwrap_or_default()
        } else {
            vec![]
        },
        circuitnet: crate::circuitnet_live::status(runtime, query.offset)?,
        page: db.network_page(query)?,
        ftn: config.ftn,
        transport: config.binkp,
        binkp: crate::binkp::status(runtime)?,
        qwk: db.qwk_network_status()?,
        now: chrono::Utc::now().timestamp(),
        files: Some(db.file_network_status()?),
    };
    if query.section == ftn::NetworkSection::Circuitnet {
        result.page.more = result.circuitnet.iter().any(|s| s.more);
    }
    if serde_json::to_vec(&result).map_or(true, |b| b.len() > 524288) {
        return Err(ftn::Error::Capacity.into());
    }
    Ok(result)
}
