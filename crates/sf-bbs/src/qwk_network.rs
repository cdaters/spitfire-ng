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

//! On-demand, daemon-owned QWK network operations over protected operator IPC.
use crate::{ApplicationError, BoardRuntime, OperatorControlError};
use serde::{Deserialize, Serialize};
use sf_core::{
    qwk_network::{ImportResult, Link, LinkStatus, Mapping},
    LocalOperatorCapability as Capability, RuntimeDatabase,
};
pub const NETWORK_MINOR: u16 = 12;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NetworkAction {
    Circuitnet {
        request: crate::circuitnet_live::Action,
    },
    Hold {
        queue: String,
        expected: i64,
    },
    Binkp {
        request: crate::binkp::Action,
    },
    Ftn {
        request: crate::ftn::Action,
    },
    Configure {
        link: Link,
        mappings: Vec<Mapping>,
        expected: i64,
    },
    ConfigureMail {
        policy: sf_core::qwk_network::MailPolicy,
        expected: i64,
    },
    Build {
        link: String,
        expected: i64,
    },
    Ingest {
        link: String,
        expected: i64,
    },
    Handoff {
        link: String,
        expected: i64,
        artifact: String,
        accepted: bool,
    },
    Retry {
        queue: String,
        expected: i64,
    },
}
impl NetworkAction {
    pub fn feature(&self) -> crate::OperatorFeature {
        if matches!(self, Self::Circuitnet { .. }) {
            return crate::OperatorFeature::Circuitnet;
        }
        if matches!(
            self,
            Self::Ftn {
                request: crate::ftn::Action::Files { .. }
            } | Self::Binkp {
                request: crate::binkp::Action::TicCredential { .. }
                    | crate::binkp::Action::ClearTicCredential { .. }
            }
        ) {
            return crate::OperatorFeature::FtnFiles;
        }
        if matches!(
            self,
            Self::Ftn {
                request: crate::ftn::Action::Downstream { .. }
                    | crate::ftn::Action::Subscription { .. }
                    | crate::ftn::Action::AreaAccess { .. }
                    | crate::ftn::Action::Rescan { .. }
            } | Self::Binkp {
                request: crate::binkp::Action::AreaFixCredential { .. }
                    | crate::binkp::Action::ClearAreaFixCredential { .. }
            }
        ) {
            crate::OperatorFeature::FtnHub
        } else if matches!(self, Self::Hold { .. }) {
            crate::OperatorFeature::Networks
        } else if matches!(self, Self::Binkp { .. }) {
            crate::OperatorFeature::BinkpNetwork
        } else if matches!(self, Self::Ftn { .. }) {
            crate::OperatorFeature::FtnNetwork
        } else {
            crate::OperatorFeature::QwkNetwork
        }
    }
    pub fn capability(&self) -> Capability {
        match self {
            Self::Circuitnet { request } => request.capability(),
            Self::Binkp { request } => request.capability(),
            Self::Ftn { request } => request.capability(),
            Self::Configure { .. } | Self::ConfigureMail { .. } => {
                Capability::ChangeSensitiveConfiguration
            }
            Self::Hold { .. } | Self::Retry { .. } | Self::Handoff { .. } => {
                Capability::NetworkQueue
            }
            _ => Capability::NetworkRun,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "network-result", rename_all = "kebab-case")]
pub enum NetworkResult {
    Binkp { response: crate::binkp::Result },
    Ftn { response: crate::ftn::Result },
    Configured,
    Built { artifact: Option<String> },
    Imported { summary: ImportResult },
    Updated,
    Replayed { state: String },
    Rejected { reason: String },
}
pub(crate) fn status(runtime: &BoardRuntime) -> Result<Vec<LinkStatus>, ApplicationError> {
    Ok(RuntimeDatabase::open_read_only(runtime.database_path())?.qwk_network_status()?)
}
pub(crate) fn dispatch(
    runtime: &std::sync::Arc<BoardRuntime>,
    principal: &str,
    capabilities: &[Capability],
    command_id: &str,
    action: &NetworkAction,
) -> Result<NetworkResult, ApplicationError> {
    let _work = runtime.live_controls.track();
    let _guard = runtime
        .network_lock
        .lock()
        .map_err(|_| OperatorControlError::InvalidCommand)?;

    if command_id.len() < 16
        || command_id.len() > 64
        || !command_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(OperatorControlError::InvalidCommand.into());
    }
    if runtime.shutdown_in_progress()? {
        return Err(OperatorControlError::InvalidCommand.into());
    }
    let now = chrono::Utc::now().timestamp();
    let mut db = RuntimeDatabase::open(runtime.database_path())?;
    db.bind_posting_identity_configuration(&runtime.configuration.current()?);
    let operation = match action {
        NetworkAction::Circuitnet { request } => request.operation(),
        NetworkAction::Binkp { request } => request.operation(),
        NetworkAction::Ftn { request } => request.operation(),
        NetworkAction::Configure { .. } => "network.configure",
        NetworkAction::ConfigureMail { .. } => "network.mail-policy",
        NetworkAction::Build { .. } => "network.build",
        NetworkAction::Ingest { .. } => "network.ingest",
        NetworkAction::Handoff { .. } => "network.handoff",
        NetworkAction::Retry { .. } => "network.retry",
        NetworkAction::Hold { .. } => "network.hold",
    };
    let allowed = capabilities.contains(&action.capability());
    let audit = sf_core::NewOperatorControlAudit {
        occurred_at: now,
        operator_kind: "host-operator".into(),
        operator_id: Some(principal.into()),
        operation: operation.into(),
        authorization_result: if allowed { "allowed" } else { "denied" }.into(),
        target_kind: Some("message-network".into()),
        target_id: None,
        command_id: Some(command_id.into()),
        correlation_id: None,
        outcome: if allowed { "succeeded" } else { "denied" }.into(),
        detail_code: Some("network-request".into()),
    };
    db.record_operator_control_audit(&audit)?;
    if !allowed {
        return Err(OperatorControlError::AuthorizationDenied.into());
    }
    let fingerprint = sf_net::qwk::digest(
        &match action {
            NetworkAction::Binkp { request } => request.fingerprint(),
            _ => serde_json::to_vec(action),
        }
        .map_err(|_| OperatorControlError::InvalidCommand)?,
    );
    let receipt = sf_core::NewOperatorCommandReceipt {
        command_id: command_id.into(),
        daemon_generation: runtime.daemon_generation().into(),
        operator_id: principal.into(),
        command_family: "message-network".into(),
        command_type: "network-action".into(),
        request_fingerprint: fingerprint,
        target_kind: None,
        target_id: None,
        target_generation: None,
        received_at: now,
    };
    match db.accept_operator_command(&receipt)? {
        sf_core::CommandReceiptResult::Accepted => (),
        sf_core::CommandReceiptResult::Replayed(r) => {
            return Ok(NetworkResult::Replayed { state: r.state })
        }
        _ => return Err(OperatorControlError::Conflict.into()),
    }
    let result = (|| -> Result<NetworkResult, ApplicationError> {
        Ok(match action {
            NetworkAction::Circuitnet { request } => {
                crate::circuitnet_live::dispatch(runtime, principal, request)?
            }
            NetworkAction::Hold { queue, expected } => {
                db.hold_qwk_network(principal, queue, *expected, now)?;
                NetworkResult::Updated
            }
            NetworkAction::Binkp { request } => NetworkResult::Binkp {
                response: crate::binkp::dispatch(runtime, principal, request)?,
            },
            NetworkAction::Ftn { request } => NetworkResult::Ftn {
                response: crate::ftn::dispatch(runtime, &mut db, principal, request, now)?,
            },
            NetworkAction::Configure {
                link,
                mappings,
                expected,
            } => {
                db.configure_qwk_link(principal, link, mappings, *expected, now)?;
                runtime.network_artifacts.prepare_handoff(&link.id)?;
                NetworkResult::Configured
            }
            NetworkAction::ConfigureMail { policy, expected } => {
                db.configure_qwk_mail(principal, policy, *expected, now)?;
                NetworkResult::Configured
            }
            NetworkAction::Build { link, expected } => {
                let artifact =
                    db.build_qwk_network(&runtime.network_artifacts, link, *expected, now)?;
                NetworkResult::Built { artifact }
            }
            NetworkAction::Ingest { link, expected } => {
                db.validate_qwk_ingress(link, *expected)?;
                let bytes = runtime.network_artifacts.read_handoff(link)?;
                NetworkResult::Imported {
                    summary: db.ingest_qwk_network(
                        &runtime.network_artifacts,
                        link,
                        *expected,
                        &bytes,
                        now,
                    )?,
                }
            }
            NetworkAction::Handoff {
                link,
                expected,
                artifact,
                accepted,
            } => {
                db.finish_qwk_handoff(principal, link, *expected, artifact, *accepted, now)?;
                NetworkResult::Updated
            }
            NetworkAction::Retry { queue, expected } => {
                db.retry_qwk_network(principal, queue, *expected, now)?;
                NetworkResult::Updated
            }
        })
    })();
    match result {
        Ok(value) => {
            db.record_operator_control_audit(&sf_core::NewOperatorControlAudit {
                outcome: "succeeded".into(),
                detail_code: Some("network-completed".into()),
                ..audit
            })?;
            db.complete_operator_command(command_id, "network-completed", 1, now)?;
            Ok(value)
        }
        Err(error) => {
            db.record_operational_event(&sf_core::NewOperationalEvent::new(
                now,
                sf_core::EventCategory::Message,
                sf_core::EventSeverity::Warning,
                if matches!(action, NetworkAction::Circuitnet { .. }) {
                    "message.circuitnet.operation-failed"
                } else if matches!(action, NetworkAction::Ftn { .. }) {
                    "message.ftn.operation-failed"
                } else {
                    "message.qwk-network.exchange-failed"
                },
                sf_core::EventOutcome::Failed,
            ))?;
            db.record_operator_control_audit(&sf_core::NewOperatorControlAudit {
                outcome: "failed".into(),
                detail_code: Some("network-rejected".into()),
                ..audit
            })?;
            db.reject_operator_command(command_id, "network-rejected", now)?;
            Ok(NetworkResult::Rejected {
                reason: match error {
                    ApplicationError::Ftn(e) => match e {
                        sf_core::ftn::Error::Held => "ftn-held",
                        sf_core::ftn::Error::Conflict => "ftn-conflict",
                        sf_core::ftn::Error::Routing => "ftn-routing",
                        sf_core::ftn::Error::Policy => "ftn-policy",
                        sf_core::ftn::Error::Denied => "ftn-denied",
                        sf_core::ftn::Error::Capacity => "ftn-capacity",
                        _ => "ftn-rejected",
                    }
                    .into(),
                    ApplicationError::QwkNetwork(e) => e.to_string(),
                    _ => "network host operation failed".into(),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_network_capability_is_required_and_denial_is_audited() {
        let temp = tempfile::tempdir().unwrap();
        let board = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        let runtime = std::sync::Arc::new(BoardRuntime::load(&board.config_path).unwrap());
        let principal = crate::current_operator_identity().unwrap();
        let action = NetworkAction::Build {
            link: "unconfigured".into(),
            expected: 1,
        };
        assert!(matches!(
            dispatch(
                &runtime,
                &principal,
                &[Capability::NetworkStatus],
                &"a".repeat(32),
                &action
            ),
            Err(ApplicationError::OperatorControl(
                OperatorControlError::AuthorizationDenied
            ))
        ));
        let connection = rusqlite::Connection::open(runtime.database_path()).unwrap();
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM operator_control_audit WHERE operation='network.build' AND authorization_result='denied'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM operator_command_journal WHERE command_family='qwk-network'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]
    fn audit_failure_prevents_network_action_admission() {
        let temp = tempfile::tempdir().unwrap();
        let board = crate::initialize_fixture_board(&temp.path().join("board")).unwrap();
        let runtime = std::sync::Arc::new(BoardRuntime::load(&board.config_path).unwrap());
        let principal = crate::current_operator_identity().unwrap();
        let connection = rusqlite::Connection::open(runtime.database_path()).unwrap();
        connection.execute_batch("CREATE TRIGGER test_network_audit_failure BEFORE INSERT ON operator_control_audit BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(dispatch(
            &runtime,
            &principal,
            &[Capability::NetworkRun],
            &"b".repeat(32),
            &NetworkAction::Build {
                link: "unconfigured".into(),
                expected: 1
            }
        )
        .is_err());
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM operator_command_journal WHERE command_family='qwk-network'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
}
