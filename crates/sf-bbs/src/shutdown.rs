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

//! Ephemeral coordination for the existing daemon listener/session exit path.
use crate::live_control::{audit, event, now};
use crate::{
    ApplicationError, BoardRuntime, LiveControlAction, LiveControlResult, MutationResult,
    OperatorControlError,
};
use serde::{Deserialize, Serialize};
use sf_core::{
    CommandReceiptResult, NewOperatorCommandReceipt, NodeRuntimeState, PageState, RuntimeDatabase,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const SHUTDOWN_MINOR: u16 = 4;
const PREFLIGHT_TTL: Duration = Duration::from_secs(30);
// Covers the existing five-second complete chat-frame deadline plus scheduling.
const FINALIZE_GRACE: Duration = Duration::from_secs(6);
type Consequence = (
    u32,
    u64,
    u64,
    String,
    Option<String>,
    Option<(u64, PageState)>,
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShutdownPhase {
    #[default]
    Running,
    Requested,
    Draining,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ShutdownImpact {
    pub daemon_generation: String,
    pub active_callers: usize,
    pub active_transfers: usize,
    pub active_chats: usize,
    pub interactions: usize,
    pub phase: ShutdownPhase,
    pub token: String,
}

struct Preflight {
    owner: String,
    command_id: String,
    created: Instant,
    impact: ShutdownImpact,
    // Safe exact runtime identities, not private caller data.
    consequence: Vec<Consequence>,
}

#[derive(Default)]
pub(crate) struct ShutdownControl {
    pub phase: ShutdownPhase,
    pub origin: Option<(String, String)>,
    preflights: HashMap<String, Preflight>,
}

fn impact(
    runtime: &BoardRuntime,
    phase: ShutdownPhase,
) -> Result<(ShutdownImpact, Vec<sf_core::NodeSnapshot>), ApplicationError> {
    let nodes = runtime
        .node_snapshots()?
        .into_iter()
        .filter(|n| n.session_id.is_some())
        .collect::<Vec<_>>();
    let mut value = ShutdownImpact {
        daemon_generation: runtime.daemon_generation().to_owned(),
        active_callers: nodes.len(),
        active_transfers: 0,
        active_chats: 0,
        interactions: 0,
        phase,
        token: String::new(),
    };
    for node in &nodes {
        value.active_transfers += usize::from(matches!(
            node.state,
            NodeRuntimeState::Downloading | NodeRuntimeState::Uploading
        ));
        if let Some((_, state)) = runtime
            .interaction()
            .interaction_state(node.session_id.expect("active node"))?
        {
            value.interactions += 1;
            value.active_chats += usize::from(state == PageState::Chatting);
        }
    }
    Ok((value, nodes))
}

pub(crate) fn status(runtime: &BoardRuntime) -> Result<ShutdownImpact, ApplicationError> {
    let control = runtime
        .shutdown
        .lock()
        .map_err(|_| ApplicationError::Coordination("shutdown lock poisoned"))?;
    Ok(impact(runtime, control.phase)?.0)
}

pub(crate) fn dispatch(
    runtime: &BoardRuntime,
    principal: &str,
    owner: &str,
    command_id: String,
    fingerprint: String,
    action: LiveControlAction,
) -> Result<MutationResult, ApplicationError> {
    let name = action.name();
    let (generation, token) = match &action {
        LiveControlAction::PrepareGracefulShutdown { daemon_generation } => {
            (daemon_generation, None)
        }
        LiveControlAction::RequestGracefulShutdown {
            daemon_generation,
            preflight_token,
        } => (daemon_generation, Some(preflight_token)),
        _ => unreachable!(),
    };
    // Admission and shutdown commands share this short transition lock.
    let mut control = runtime
        .shutdown
        .lock()
        .map_err(|_| ApplicationError::Coordination("shutdown lock poisoned"))?;
    let mut database = RuntimeDatabase::open(runtime.database_path())?;
    if token.is_some() {
        match database.accept_operator_command(&NewOperatorCommandReceipt {
            command_id: command_id.clone(),
            daemon_generation: runtime.daemon_generation().to_owned(),
            operator_id: principal.to_owned(),
            command_family: "operator-control".to_owned(),
            command_type: name.to_owned(),
            request_fingerprint: fingerprint,
            target_kind: Some("daemon".to_owned()),
            target_id: Some(runtime.daemon_generation().to_owned()),
            target_generation: Some(generation.clone()),
            received_at: now(),
        })? {
            CommandReceiptResult::Accepted => {}
            CommandReceiptResult::Replayed(receipt) => {
                audit(
                    runtime,
                    principal,
                    &command_id,
                    name,
                    None,
                    "allowed",
                    "succeeded",
                    "command-replayed",
                )?;
                return Ok(MutationResult::Replayed {
                    command_id,
                    result_class: receipt.result_class,
                });
            }
            _ => {
                audit(
                    runtime,
                    principal,
                    &command_id,
                    name,
                    None,
                    "allowed",
                    "rejected",
                    "command-id-conflict",
                )?;
                return Err(OperatorControlError::Conflict.into());
            }
        }
    }
    let reject = |reason: &str| -> Result<MutationResult, ApplicationError> {
        if token.is_some() {
            RuntimeDatabase::open(runtime.database_path())?.reject_operator_command(
                &command_id,
                reason,
                now(),
            )?;
        }
        audit(
            runtime,
            principal,
            &command_id,
            name,
            None,
            "allowed",
            "rejected",
            reason,
        )?;
        Ok(MutationResult::Rejected {
            command_id: command_id.clone(),
            reason: reason.to_owned(),
        })
    };
    if generation != runtime.daemon_generation() {
        return reject("stale-target");
    }
    if control.phase != ShutdownPhase::Running {
        return reject("shutdown-already-requested");
    }
    let (mut current, nodes) = impact(runtime, control.phase)?;
    let consequence = nodes
        .iter()
        .map(|node| {
            Ok((
                node.id.get(),
                node.session_id.expect("active node").get(),
                node.occupancy_generation.unwrap_or(0),
                format!("{:?}", node.state),
                node.activity_file.clone(),
                runtime
                    .interaction()
                    .interaction_state(node.session_id.expect("active node"))?,
            ))
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    if let Some(token) = token {
        let Some(preflight) = control.preflights.remove(token) else {
            return reject("preflight-required");
        };
        current.token = token.clone();
        if preflight.owner != owner
            || preflight.command_id != command_id
            || preflight.created.elapsed() >= PREFLIGHT_TTL
            || preflight.impact != current
            || preflight.consequence != consequence
        {
            return reject("preflight-stale");
        }
        let detail = format!(
            "callers-{}-transfers-{}-chats-{}",
            current.active_callers, current.active_transfers, current.active_chats
        );
        audit(
            runtime,
            principal,
            &command_id,
            name,
            None,
            "allowed",
            "succeeded",
            &detail,
        )?;
        event(runtime, "shutdown-requested")?;
        // Receipt means accepted shutdown, never an impossible post-exit promise.
        database.complete_operator_command(&command_id, "shutdown-requested", 1, now())?;
        control.origin = Some((principal.to_owned(), command_id.clone()));
        control.phase = ShutdownPhase::Requested;
        control.preflights.clear();
        Ok(MutationResult::Completed {
            command_id,
            result_class: "shutdown-requested".to_owned(),
        })
    } else {
        control
            .preflights
            .retain(|_, value| value.created.elapsed() < PREFLIGHT_TTL);
        if control.preflights.len() >= 128 {
            return reject("control-busy");
        }
        current.token = crate::operator_control::random_token();
        control.preflights.insert(
            current.token.clone(),
            Preflight {
                owner: owner.to_owned(),
                command_id: command_id.clone(),
                created: Instant::now(),
                impact: current.clone(),
                consequence,
            },
        );
        audit(
            runtime,
            principal,
            &command_id,
            name,
            None,
            "allowed",
            "succeeded",
            "preflight-ready",
        )?;
        Ok(MutationResult::LiveControl {
            command_id,
            value: LiveControlResult::ShutdownPreflight { impact: current },
        })
    }
}

/// Executed by the daemon runner, never by an operator connection task.
pub(crate) fn drain(runtime: &BoardRuntime) -> Result<(), ApplicationError> {
    {
        let mut control = runtime
            .shutdown
            .lock()
            .map_err(|_| ApplicationError::Coordination("shutdown lock poisoned"))?;
        control.phase = ShutdownPhase::Draining;
    }
    event(runtime, "shutdown-draining")?;
    let mut tickets = Vec::new();
    for node in runtime.node_snapshots()? {
        if let (Some(session), Some(occupancy)) = (node.session_id, node.occupancy_generation) {
            if let Some(ticket) =
                runtime.with_live_target(node.id.get(), session.get(), occupancy, |_| {
                    runtime.interaction().request_board_shutdown(session)
                })?
            {
                tickets.push((node.id.get(), session.get(), occupancy, ticket?));
            }
        }
    }
    let started = Instant::now();
    while tickets
        .iter()
        .any(|(_, _, _, ticket)| !ticket.completed() && !ticket.failed())
        && started.elapsed() < crate::live_control::DISCONNECT_GRACE
    {
        std::thread::sleep(Duration::from_millis(25));
    }
    for (node, session, occupancy, ticket) in &tickets {
        if !ticket.completed() && !ticket.failed() {
            match runtime.emergency_close_session(*node, *session, *occupancy) {
                Ok(true) => {
                    ticket.mark_fallback();
                    event(runtime, "shutdown-emergency-close")?;
                }
                Ok(false) => {}
                Err(_) => event(runtime, "shutdown-close-failed")?,
            }
        }
    }
    let final_wait = Instant::now();
    while (tickets
        .iter()
        .any(|(_, _, _, ticket)| !ticket.completed() && !ticket.failed())
        || runtime.live_controls.pending_work())
        && final_wait.elapsed() < FINALIZE_GRACE
    {
        std::thread::sleep(Duration::from_millis(25));
    }
    let success = tickets.iter().all(|(_, _, _, ticket)| ticket.completed())
        && !runtime.live_controls.pending_work()
        && !runtime
            .live_controls
            .evidence_failed
            .load(std::sync::atomic::Ordering::Acquire)
        && runtime
            .node_snapshots()?
            .iter()
            .all(|n| n.session_id.is_none());
    let mut control = runtime
        .shutdown
        .lock()
        .map_err(|_| ApplicationError::Coordination("shutdown lock poisoned"))?;
    control.phase = if success {
        ShutdownPhase::Complete
    } else {
        ShutdownPhase::Failed
    };
    let class = if success {
        "shutdown-complete"
    } else {
        "shutdown-finalization-failed"
    };
    if let Some((principal, command_id)) = &control.origin {
        audit(
            runtime,
            principal,
            command_id,
            "request-graceful-shutdown",
            None,
            "allowed",
            if success { "succeeded" } else { "failed" },
            class,
        )?;
    }
    event(runtime, class)?;
    if success {
        Ok(())
    } else {
        Err(ApplicationError::Coordination(
            "shutdown finalization incomplete; daemon retained for data safety",
        ))
    }
}
