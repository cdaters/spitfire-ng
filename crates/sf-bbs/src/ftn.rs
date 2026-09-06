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

//! Manual FTN operations; protected operator context replaces transport in N3.
use crate::{ApplicationError, BoardRuntime};
use serde::{Deserialize, Serialize};
use sf_core::{ftn, LocalOperatorCapability as Capability, RuntimeDatabase};
pub const FTN_MINOR: u16 = 7;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    Mapping {
        mapping: ftn::Mapping,
        expected: i64,
    },
    Alias {
        alias: ftn::MailboxAlias,
    },
    Prepare {
        link: String,
    },
    PrepareDirectory {
        source: String,
    },
    Toss {
        link: String,
    },
    Scan,
    Build {
        queue: String,
        expected: i64,
    },
    DirectoryIngest {
        source: String,
        date: String,
    },
    DirectoryActivate {
        generation: String,
        expected: i64,
    },
}
impl Action {
    pub fn capability(&self) -> Capability {
        match self {
            Self::Mapping { .. } | Self::Alias { .. } => Capability::ChangeSensitiveConfiguration,
            Self::DirectoryActivate { .. } => Capability::NetworkDirectoryActivate,
            _ => Capability::NetworkRun,
        }
    }
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Mapping { .. } => "ftn.mapping",
            Self::Alias { .. } => "ftn.alias",
            Self::Prepare { .. } => "ftn.prepare",
            Self::PrepareDirectory { .. } => "ftn.prepare-directory",
            Self::Toss { .. } => "ftn.toss",
            Self::Scan => "ftn.scan",
            Self::Build { .. } => "ftn.build",
            Self::DirectoryIngest { .. } => "ftn.directory-ingest",
            Self::DirectoryActivate { .. } => "ftn.directory-activate",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum Result {
    Updated,
    Scanned { messages: u32 },
    Tossed { summary: ftn::TossResult },
    Built { artifact: String },
    Directory { generation: ftn::Generation },
}
pub(crate) fn dispatch(
    runtime: &BoardRuntime,
    db: &mut RuntimeDatabase,
    principal: &str,
    action: &Action,
    now: i64,
) -> std::result::Result<Result, ApplicationError> {
    let policy = runtime.configuration.current()?.ftn;
    Ok(match action {
        Action::Mapping { mapping, expected } => {
            db.configure_ftn_mapping(&policy, principal, mapping, *expected, now)?;
            Result::Updated
        }
        Action::Alias { alias } => {
            db.configure_ftn_alias(&policy, principal, alias, now)?;
            Result::Updated
        }
        Action::Prepare { link } => {
            policy.link(link)?;
            runtime.network_artifacts.prepare_ftn_handoff(link)?;
            Result::Updated
        }
        Action::PrepareDirectory { source } => {
            if !policy.sources.iter().any(|s| s.id == *source && s.enabled) {
                return Err(ftn::Error::Policy.into());
            }
            runtime.network_artifacts.prepare_ftn_handoff(source)?;
            Result::Updated
        }
        Action::Toss { link } => {
            let l = policy.link(link)?;
            if !policy.enabled || !l.enabled || !l.inbound {
                return Err(ftn::Error::Denied.into());
            }
            let bytes = runtime.network_artifacts.read_ftn_handoff(link)?;
            Result::Tossed {
                summary: db.toss_ftn(&runtime.network_artifacts, &policy, link, &bytes, now)?,
            }
        }
        Action::Scan => Result::Scanned {
            messages: db.scan_ftn(&policy, now)?,
        },
        Action::Build { queue, expected } => Result::Built {
            artifact: db.build_ftn(&runtime.network_artifacts, &policy, queue, *expected, now)?,
        },
        Action::DirectoryIngest { source, date } => {
            if !policy.sources.iter().any(|s| s.id == *source && s.enabled) {
                return Err(ftn::Error::Policy.into());
            }
            let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| ftn::Error::Policy)?;
            let bytes = runtime.network_artifacts.read_ftn_handoff(source)?;
            Result::Directory {
                generation: db.ingest_ftn_directory(
                    &runtime.network_artifacts,
                    &policy,
                    source,
                    date,
                    &bytes,
                    now,
                )?,
            }
        }
        Action::DirectoryActivate {
            generation,
            expected,
        } => {
            db.activate_ftn_directory(&policy, principal, generation, *expected, now)?;
            Result::Updated
        }
    })
}
