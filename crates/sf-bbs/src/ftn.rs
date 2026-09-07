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
    Files {
        request: FileAction,
    },
    Downstream {
        downstream: ftn::Downstream,
        expected: i64,
    },
    Subscription {
        subscription: ftn::Subscription,
        expected: i64,
    },
    AreaAccess {
        access: ftn::AreaAccess,
        expected: i64,
    },
    Rescan {
        link: String,
        areas: Vec<ftn::RescanArea>,
    },
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FileAction {
    Policy {
        value: ftn::files::FilePolicy,
    },
    Area {
        value: ftn::files::FileEchoArea,
    },
    Subscription {
        value: ftn::files::FileSubscription,
    },
    Grant {
        value: ftn::files::FreqGrant,
    },
    Preview {
        file: i64,
        domain: sf_net::ftn::Domain,
        tag: String,
    },
    Hatch {
        preview: Box<ftn::files::HatchPreview>,
    },
    Request {
        link: String,
        names: Vec<String>,
        native_area: i64,
    },
    RetryRequest {
        request: String,
        expected: i64,
    },
    Hold {
        delivery: String,
        expected: i64,
        held: bool,
    },
}
impl FileAction {
    fn capability(&self) -> Capability {
        match self {
            Self::Policy { .. }
            | Self::Area { .. }
            | Self::Subscription { .. }
            | Self::Grant { .. } => Capability::ChangeSensitiveConfiguration,
            Self::Preview { .. } => Capability::NetworkStatus,
            Self::Hold { .. } => Capability::NetworkQueue,
            _ => Capability::NetworkRun,
        }
    }
    fn operation(&self) -> &'static str {
        match self {
            Self::Policy { .. } => "ftn.file-policy",
            Self::Area { .. } => "ftn.file-area",
            Self::Subscription { .. } => "ftn.file-subscription",
            Self::Grant { .. } => "ftn.freq-grant",
            Self::Preview { .. } => "ftn.hatch-preview",
            Self::Hatch { .. } => "ftn.hatch",
            Self::Request { .. } => "ftn.freq-request",
            Self::RetryRequest { .. } => "ftn.freq-retry",
            Self::Hold { .. } => "ftn.file-hold",
        }
    }
}
impl Action {
    pub fn capability(&self) -> Capability {
        match self {
            Self::Files { request } => request.capability(),
            Self::Mapping { .. }
            | Self::Alias { .. }
            | Self::Downstream { .. }
            | Self::Subscription { .. }
            | Self::AreaAccess { .. } => Capability::ChangeSensitiveConfiguration,
            Self::DirectoryActivate { .. } => Capability::NetworkDirectoryActivate,
            _ => Capability::NetworkRun,
        }
    }
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Files { request } => request.operation(),
            Self::Downstream { .. } => "ftn.downstream",
            Self::Subscription { .. } => "ftn.subscription",
            Self::AreaAccess { .. } => "ftn.area-access",
            Self::Rescan { .. } => "ftn.rescan",
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
    HatchPreview {
        preview: Box<ftn::files::HatchPreview>,
    },
    Updated,
    Scanned {
        messages: u32,
    },
    Tossed {
        summary: ftn::TossResult,
    },
    Built {
        artifact: String,
    },
    Directory {
        generation: ftn::Generation,
    },
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
        Action::Files { request } => {
            let storage = sf_core::FileStorage::open_existing(&runtime.paths)?;
            match request {
                FileAction::Policy { value } => db.configure_file_network(principal, value, now)?,
                FileAction::Area { value } => {
                    db.configure_fileecho_area(&policy, principal, value, now)?
                }
                FileAction::Subscription { value } => {
                    db.configure_file_subscription(&policy, principal, value, now)?
                }
                FileAction::Grant { value } => {
                    db.configure_freq_grant(&policy, &storage, principal, value, now)?
                }
                FileAction::Preview { file, domain, tag } => {
                    return Ok(Result::HatchPreview {
                        preview: Box::new(
                            db.preview_file_hatch(&policy, &storage, *file, domain, tag, now)?,
                        ),
                    })
                }
                FileAction::Hatch { preview } => {
                    db.hatch_native_file(&policy, &storage, principal, preview, now)?;
                }
                FileAction::Request {
                    link,
                    names,
                    native_area,
                } => {
                    db.request_ftn_files(&policy, principal, link, names, *native_area, now)?;
                }
                FileAction::RetryRequest { request, expected } => {
                    db.retry_ftn_request(&policy, principal, request, *expected, now)?;
                }
                FileAction::Hold {
                    delivery,
                    expected,
                    held,
                } => db.hold_file_delivery(principal, delivery, *expected, *held, now)?,
            }
            Result::Updated
        }
        Action::Downstream {
            downstream,
            expected,
        } => {
            runtime
                .configuration
                .current()?
                .binkp
                .link(&downstream.link)?;
            db.configure_ftn_downstream(&policy, principal, downstream, *expected, now)?;
            Result::Updated
        }
        Action::Subscription {
            subscription,
            expected,
        } => {
            db.configure_ftn_subscription(&policy, principal, subscription, *expected, now)?;
            Result::Updated
        }
        Action::AreaAccess { access, expected } => {
            db.configure_ftn_area_access(&policy, principal, access, *expected, now)?;
            Result::Updated
        }
        Action::Rescan { link, areas } => {
            db.rescan_ftn(&policy, principal, link, areas, now)?;
            Result::Updated
        }
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

#[cfg(test)]
mod freq_tests {
    use super::*;
    #[test]
    fn freq_retry_action_requires_run_and_only_accepts_original_identity_version() {
        let action: FileAction = serde_json::from_str(
            r#"{"operation":"retry-request","request":"original","expected":2}"#,
        )
        .unwrap();
        assert_eq!(action.capability(), Capability::NetworkRun);
        assert_eq!(action.operation(), "ftn.freq-retry");
        for extra in [
            r#", "link":"other""#,
            r#", "names":["OTHER.TXT"]"#,
            r#", "attempts":0"#,
        ] {
            let json = format!(
                r#"{{"operation":"retry-request","request":"original","expected":2{extra}}}"#
            );
            assert!(serde_json::from_str::<FileAction>(&json).is_err());
        }
    }
}
