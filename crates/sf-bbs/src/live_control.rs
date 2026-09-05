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

use crate::{
    ApplicationError, BoardRuntime, MutationResult, OperatorControlError, OperatorFeature,
};
use serde::{Deserialize, Serialize};
use sf_core::{
    LocalOperatorCapability as Capability, NodeRuntimeState, OperatorChat, PageRequest, PageState,
    RuntimeDatabase, SessionId, SysopAvailability,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const LIVE_CONTROL_MINOR: u16 = 3;
pub const DISCONNECT_GRACE: Duration = Duration::from_secs(3);
const PREFLIGHT_TTL: Duration = Duration::from_secs(30);
const MAX_HANDOFFS: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LiveSessionTarget {
    pub daemon_generation: String,
    pub node_id: u32,
    pub session_id: u64,
    pub occupancy_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum LiveControlAction {
    PrepareGracefulShutdown {
        daemon_generation: String,
    },
    RequestGracefulShutdown {
        daemon_generation: String,
        preflight_token: String,
    },
    SetPageAvailability {
        available: bool,
    },
    AnswerCallerPage {
        target: LiveSessionTarget,
        interaction_id: u64,
    },
    DeclineCallerPage {
        target: LiveSessionTarget,
        interaction_id: u64,
    },
    InviteOperatorChat {
        target: LiveSessionTarget,
    },
    PrepareDisconnect {
        target: LiveSessionTarget,
        notice: bool,
    },
    DisconnectSession {
        target: LiveSessionTarget,
        notice: bool,
        preflight_token: String,
    },
}

impl LiveControlAction {
    pub fn feature(&self) -> OperatorFeature {
        match self {
            Self::PrepareGracefulShutdown { .. } | Self::RequestGracefulShutdown { .. } => {
                OperatorFeature::GracefulShutdown
            }
            Self::SetPageAvailability { .. } => OperatorFeature::PageAvailability,
            Self::AnswerCallerPage { .. } | Self::DeclineCallerPage { .. } => {
                OperatorFeature::CallerPages
            }
            Self::InviteOperatorChat { .. } => OperatorFeature::CallerChat,
            Self::PrepareDisconnect { .. } | Self::DisconnectSession { .. } => {
                OperatorFeature::SessionDisconnect
            }
        }
    }
    pub fn capability(&self) -> Capability {
        match self {
            Self::PrepareGracefulShutdown { .. } | Self::RequestGracefulShutdown { .. } => {
                Capability::RequestGracefulShutdown
            }
            Self::SetPageAvailability { .. } => Capability::ManagePageAvailability,
            Self::AnswerCallerPage { .. } | Self::DeclineCallerPage { .. } => {
                Capability::ManageCallerPages
            }
            Self::InviteOperatorChat { .. } => Capability::ChatWithCaller,
            Self::PrepareDisconnect { .. } | Self::DisconnectSession { .. } => {
                Capability::DisconnectSession
            }
        }
    }
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::PrepareGracefulShutdown { .. } => "prepare-graceful-shutdown",
            Self::RequestGracefulShutdown { .. } => "request-graceful-shutdown",
            Self::SetPageAvailability { .. } => "set-page-availability",
            Self::AnswerCallerPage { .. } => "answer-caller-page",
            Self::DeclineCallerPage { .. } => "decline-caller-page",
            Self::InviteOperatorChat { .. } => "invite-operator-chat",
            Self::PrepareDisconnect { .. } => "prepare-disconnect",
            Self::DisconnectSession { .. } => "disconnect-session",
        }
    }
    pub fn target(&self) -> Option<&LiveSessionTarget> {
        match self {
            Self::SetPageAvailability { .. }
            | Self::PrepareGracefulShutdown { .. }
            | Self::RequestGracefulShutdown { .. } => None,
            Self::AnswerCallerPage { target, .. }
            | Self::DeclineCallerPage { target, .. }
            | Self::InviteOperatorChat { target }
            | Self::PrepareDisconnect { target, .. }
            | Self::DisconnectSession { target, .. } => Some(target),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PendingCallerPage {
    pub target: LiveSessionTarget,
    pub interaction_id: u64,
    pub public_handle: String,
    pub state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InteractionSnapshot {
    pub available: bool,
    pub pages: Vec<PendingCallerPage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DisconnectPreflight {
    pub target: LiveSessionTarget,
    pub public_handle: String,
    pub online_seconds: u64,
    pub transfer_active: bool,
    pub interaction_active: bool,
    pub notice: bool,
    pub token: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LiveControlResult {
    ShutdownPreflight {
        impact: crate::ShutdownImpact,
    },
    DisconnectPreflight {
        impact: DisconnectPreflight,
    },
    ChatReady {
        join_token: String,
        target: LiveSessionTarget,
        invited: bool,
    },
    Pending {
        result_class: String,
    },
}

struct PreflightEntry {
    owner: String,
    command_id: String,
    created: Instant,
    impact: DisconnectPreflight,
    node_state: NodeRuntimeState,
    activity_file: Option<String>,
    interaction_id: Option<u64>,
}

pub(crate) struct ChatHandoff {
    pub owner: String,
    pub principal: String,
    pub command_id: String,
    pub target: LiveSessionTarget,
    pub invited: bool,
    pub chat: OperatorChat,
    pub created: Instant,
}

/// Ephemeral preflight and authenticated stream handoff resources only.
/// InteractionHub remains the interaction authority; no transcript lives here.
#[derive(Default)]
pub(crate) struct LiveControlResources {
    work: Arc<AtomicUsize>,
    pub evidence_failed: AtomicBool,
    preflights: Mutex<HashMap<String, PreflightEntry>>,
    chats: Mutex<HashMap<String, ChatHandoff>>,
}

impl LiveControlResources {
    pub fn track(&self) -> ControlWork {
        self.work.fetch_add(1, Ordering::AcqRel);
        ControlWork(self.work.clone())
    }
    pub fn pending_work(&self) -> bool {
        self.work.load(Ordering::Acquire) != 0
    }
    pub fn attachment_ended(&self, owner: &str) {
        if let Ok(mut values) = self.preflights.lock() {
            values.retain(|_, value| value.owner != owner);
        }
        if let Ok(mut values) = self.chats.lock() {
            values.retain(|_, value| value.owner != owner);
        }
    }

    pub fn take_chat(
        &self,
        token: &str,
        principal: &str,
    ) -> Result<ChatHandoff, OperatorControlError> {
        let mut values = self
            .chats
            .lock()
            .map_err(|_| OperatorControlError::AuthenticationFailed)?;
        if !values.get(token).is_some_and(|value| {
            value.principal == principal && value.created.elapsed() < PREFLIGHT_TTL
        }) {
            return Err(OperatorControlError::AuthenticationFailed);
        }
        values
            .remove(token)
            .ok_or(OperatorControlError::AuthenticationFailed)
    }
}

pub(crate) struct ControlWork(Arc<AtomicUsize>);
impl Drop for ControlWork {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) fn snapshot(runtime: &BoardRuntime) -> Result<InteractionSnapshot, ApplicationError> {
    let nodes = runtime.node_snapshots()?;
    let pages = runtime
        .interaction()
        .pages()?
        .into_iter()
        .filter_map(|page| {
            let node = nodes
                .iter()
                .find(|node| node.id == page.node_id && node.session_id == Some(page.session_id))?;
            Some(PendingCallerPage {
                target: LiveSessionTarget {
                    daemon_generation: runtime.daemon_generation().to_owned(),
                    node_id: node.id.get(),
                    session_id: page.session_id.get(),
                    occupancy_generation: node.occupancy_generation?,
                },
                interaction_id: page.interaction_id,
                public_handle: node.caller_name.clone().unwrap_or_default(),
                state: if page.state == PageState::Pending {
                    "pending"
                } else {
                    "chatting"
                }
                .to_owned(),
            })
        })
        .collect();
    Ok(InteractionSnapshot {
        available: runtime.interaction().availability()? == SysopAvailability::Available,
        pages,
    })
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn audit(
    runtime: &BoardRuntime,
    principal: &str,
    command_id: &str,
    operation: &str,
    target: Option<&LiveSessionTarget>,
    authorization: &str,
    outcome: &str,
    detail: &str,
) -> Result<(), ApplicationError> {
    let mut database = RuntimeDatabase::open(runtime.database_path())?;
    database.record_operator_control_audit(&sf_core::NewOperatorControlAudit {
        occurred_at: now(),
        operator_kind: "host-operator".to_owned(),
        operator_id: Some(principal.to_owned()),
        operation: operation.to_owned(),
        authorization_result: authorization.to_owned(),
        target_kind: target.map(|_| "session".to_owned()),
        target_id: target.map(|value| value.session_id.to_string()),
        command_id: (16..=64)
            .contains(&command_id.len())
            .then(|| command_id.to_owned()),
        correlation_id: None,
        outcome: outcome.to_owned(),
        detail_code: Some(detail.to_owned()),
    })?;
    Ok(())
}

pub(crate) fn event(runtime: &BoardRuntime, code: &str) -> Result<(), ApplicationError> {
    let mut database = RuntimeDatabase::open(runtime.database_path())?;
    // Only the safe transition code; no chat text, identities, or terminal bytes.
    let mut event = sf_core::NewOperationalEvent::new(
        now(),
        sf_core::EventCategory::Operator,
        sf_core::EventSeverity::Notice,
        format!("operator.{code}"),
        if code == "authorization-denied" {
            sf_core::EventOutcome::Denied
        } else if code.ends_with("-failed") {
            sf_core::EventOutcome::Failed
        } else {
            sf_core::EventOutcome::Succeeded
        },
    );
    event.attributes = sf_core::EventAttributes::Operator {
        action: code.to_owned(),
    };
    database.record_operational_event(&event)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch(
    runtime: Arc<BoardRuntime>,
    principal: String,
    owner: String,
    capabilities: &[Capability],
    authorize_chat: sf_core::ChatAuthorization,
    command_id: String,
    fingerprint: String,
    action: LiveControlAction,
) -> Result<MutationResult, ApplicationError> {
    let _work = runtime.live_controls.track();
    let name = action.name();
    if !capabilities.contains(&action.capability())
        || (matches!(action, LiveControlAction::AnswerCallerPage { .. })
            && !capabilities.contains(&Capability::ChatWithCaller))
    {
        audit(
            &runtime,
            &principal,
            &command_id,
            name,
            action.target(),
            "denied",
            "denied",
            "capability-denied",
        )?;
        return Err(OperatorControlError::AuthorizationDenied.into());
    }
    if !(16..=64).contains(&command_id.len()) || command_id.chars().any(char::is_control) {
        audit(
            &runtime,
            &principal,
            "",
            name,
            action.target(),
            "allowed",
            "rejected",
            "invalid-command-id",
        )?;
        return Err(OperatorControlError::InvalidCommand.into());
    }
    if action.target().is_some_and(|target| {
        target.node_id == 0 || target.session_id == 0 || target.occupancy_generation == 0
    }) {
        audit(
            &runtime,
            &principal,
            &command_id,
            name,
            action.target(),
            "allowed",
            "rejected",
            "invalid-target",
        )?;
        return Err(OperatorControlError::InvalidCommand.into());
    }
    let mut database = RuntimeDatabase::open(runtime.database_path())?;
    if matches!(
        action,
        LiveControlAction::PrepareGracefulShutdown { .. }
            | LiveControlAction::RequestGracefulShutdown { .. }
    ) {
        return crate::shutdown::dispatch(
            &runtime,
            &principal,
            &owner,
            command_id,
            fingerprint,
            action,
        );
    }
    if !matches!(action, LiveControlAction::PrepareDisconnect { .. }) {
        let receipt = sf_core::NewOperatorCommandReceipt {
            command_id: command_id.clone(),
            daemon_generation: runtime.daemon_generation().to_owned(),
            operator_id: principal.clone(),
            command_family: "operator-control".to_owned(),
            command_type: name.to_owned(),
            request_fingerprint: fingerprint,
            target_kind: action.target().map(|_| "session".to_owned()),
            target_id: action.target().map(|target| target.session_id.to_string()),
            target_generation: action.target().map(|target| {
                format!(
                    "node-{}-occupancy-{}",
                    target.node_id, target.occupancy_generation
                )
            }),
            received_at: now(),
        };
        match database.accept_operator_command(&receipt)? {
            sf_core::CommandReceiptResult::Accepted => {}
            sf_core::CommandReceiptResult::Replayed(receipt) => {
                audit(
                    &runtime,
                    &principal,
                    &command_id,
                    name,
                    action.target(),
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
                    &runtime,
                    &principal,
                    &command_id,
                    name,
                    action.target(),
                    "allowed",
                    "rejected",
                    "command-id-conflict",
                )?;
                return Err(OperatorControlError::Conflict.into());
            }
        }
    }
    let reject = |reason: &str| -> Result<MutationResult, ApplicationError> {
        let mut database = RuntimeDatabase::open(runtime.database_path())?;
        if !matches!(action, LiveControlAction::PrepareDisconnect { .. }) {
            database.reject_operator_command(&command_id, reason, now())?;
        }
        audit(
            &runtime,
            &principal,
            &command_id,
            name,
            action.target(),
            "allowed",
            "rejected",
            reason,
        )?;
        Ok(MutationResult::Rejected {
            command_id: command_id.clone(),
            reason: reason.to_owned(),
        })
    };
    let node = if let Some(target) = action.target() {
        if target.daemon_generation != runtime.daemon_generation() {
            return reject("stale-target");
        }
        let node = runtime.with_live_target(
            target.node_id,
            target.session_id,
            target.occupancy_generation,
            Clone::clone,
        )?;
        match node {
            Some(node)
                if node.caller_id.is_some() && node.state != NodeRuntimeState::Disconnecting =>
            {
                Some(node)
            }
            _ => return reject("stale-target"),
        }
    } else {
        None
    };
    if runtime.shutdown_in_progress()? {
        return reject("shutdown-already-requested");
    }
    if let LiveControlAction::PrepareDisconnect { target, notice } = &action {
        let node = node.as_ref().expect("target validated");
        let mut values = runtime
            .live_controls
            .preflights
            .lock()
            .map_err(|_| ApplicationError::Coordination("preflight lock poisoned"))?;
        values.retain(|_, value| value.created.elapsed() < PREFLIGHT_TTL);
        if values.len() >= 128 {
            return reject("control-busy");
        }
        let token = crate::operator_control::random_token();
        let interaction_id = runtime
            .interaction()
            .interaction_state(SessionId::new(target.session_id)?)?
            .map(|value| value.0);
        let impact = DisconnectPreflight {
            target: target.clone(),
            public_handle: node.caller_name.clone().unwrap_or_default(),
            online_seconds: now()
                .saturating_sub(node.connected_at.unwrap_or(now()))
                .max(0) as u64,
            transfer_active: matches!(
                node.state,
                NodeRuntimeState::Downloading | NodeRuntimeState::Uploading
            ),
            interaction_active: interaction_id.is_some(),
            notice: *notice,
            token: token.clone(),
        };
        values.insert(
            token,
            PreflightEntry {
                owner,
                command_id: command_id.clone(),
                created: Instant::now(),
                impact: impact.clone(),
                node_state: node.state,
                activity_file: node.activity_file.clone(),
                interaction_id,
            },
        );
        audit(
            &runtime,
            &principal,
            &command_id,
            name,
            Some(target),
            "allowed",
            "succeeded",
            "preflight-ready",
        )?;
        return Ok(MutationResult::LiveControl {
            command_id,
            value: LiveControlResult::DisconnectPreflight { impact },
        });
    }
    audit(
        &runtime,
        &principal,
        &command_id,
        name,
        action.target(),
        "allowed",
        "succeeded",
        "command-accepted",
    )?;
    let class = match &action {
        LiveControlAction::SetPageAvailability { available } => {
            runtime.interaction().set_availability(if *available {
                SysopAvailability::Available
            } else {
                SysopAvailability::Unavailable
            })?;
            "page-availability-set"
        }
        LiveControlAction::DeclineCallerPage {
            target,
            interaction_id,
        } => {
            let session = SessionId::new(target.session_id)?;
            let handled = runtime
                .with_live_target(
                    target.node_id,
                    target.session_id,
                    target.occupancy_generation,
                    |_| {
                        if runtime.interaction().interaction_state(session)?
                            != Some((*interaction_id, PageState::Pending))
                        {
                            return Ok(false);
                        }
                        runtime.interaction().decline(session)?;
                        Ok::<_, sf_core::InteractionError>(true)
                    },
                )?
                .transpose()?
                .unwrap_or(false);
            if !handled {
                return reject("page-already-handled");
            }
            "page-declined"
        }
        LiveControlAction::AnswerCallerPage { target, .. }
        | LiveControlAction::InviteOperatorChat { target } => {
            let session = SessionId::new(target.session_id)?;
            if !runtime.supports_live_input(session) {
                return reject("input-unavailable");
            }
            let invited = matches!(action, LiveControlAction::InviteOperatorChat { .. });
            let mut handoffs = runtime
                .live_controls
                .chats
                .lock()
                .map_err(|_| ApplicationError::Coordination("chat handoff lock poisoned"))?;
            handoffs.retain(|_, value| value.created.elapsed() < PREFLIGHT_TTL);
            if handoffs.len() >= MAX_HANDOFFS {
                return reject("chat-busy");
            }
            let chat = runtime.with_live_target(
                target.node_id,
                target.session_id,
                target.occupancy_generation,
                |node| {
                    if matches!(
                        node.state,
                        NodeRuntimeState::Downloading | NodeRuntimeState::Uploading
                    ) {
                        return Err(sf_core::InteractionError::NotPending(session.get()));
                    }
                    if let LiveControlAction::AnswerCallerPage { interaction_id, .. } = &action {
                        if runtime.interaction().interaction_state(session)?
                            != Some((*interaction_id, PageState::Pending))
                        {
                            return Err(sf_core::InteractionError::NotPending(session.get()));
                        }
                        runtime
                            .interaction()
                            .answer_owned(session, Some(owner.clone()))
                    } else {
                        runtime.interaction().invite(
                            PageRequest {
                                interaction_id: 0,
                                session_id: session,
                                node_id: node.id,
                                caller_id: node.caller_id.expect("authenticated node"),
                                caller_name: node.caller_name.clone().unwrap_or_default(),
                                requested_at: now(),
                                state: PageState::Invited,
                            },
                            owner.clone(),
                            authorize_chat,
                        )
                    }
                },
            )?;
            let chat = match chat {
                Some(Ok(chat)) => chat,
                Some(Err(_)) => return reject("chat-busy"),
                None => return reject("stale-target"),
            };
            let token = crate::operator_control::random_token();
            handoffs.insert(
                token.clone(),
                ChatHandoff {
                    owner,
                    principal: principal.clone(),
                    command_id: command_id.clone(),
                    target: target.clone(),
                    invited,
                    chat,
                    created: Instant::now(),
                },
            );
            let class = if invited {
                "chat-invited"
            } else {
                "page-answered"
            };
            database.complete_operator_command(&command_id, class, 1, now())?;
            audit(
                &runtime,
                &principal,
                &command_id,
                name,
                Some(target),
                "allowed",
                "succeeded",
                class,
            )?;
            event(&runtime, class)?;
            return Ok(MutationResult::LiveControl {
                command_id,
                value: LiveControlResult::ChatReady {
                    join_token: token,
                    target: target.clone(),
                    invited,
                },
            });
        }
        LiveControlAction::DisconnectSession {
            target,
            notice,
            preflight_token,
        } => {
            let session = SessionId::new(target.session_id)?;
            let preflight = runtime
                .live_controls
                .preflights
                .lock()
                .map_err(|_| ApplicationError::Coordination("preflight lock poisoned"))?
                .remove(preflight_token);
            let Some(preflight) = preflight else {
                return reject("preflight-required");
            };
            if preflight.owner != owner
                || preflight.command_id != command_id
                || preflight.created.elapsed() >= PREFLIGHT_TTL
                || preflight.impact.target != *target
                || preflight.impact.notice != *notice
            {
                return reject("preflight-required");
            }
            let ticket = runtime
                .with_live_target(
                    target.node_id,
                    target.session_id,
                    target.occupancy_generation,
                    |node| {
                        let current_interaction = runtime
                            .interaction()
                            .interaction_state(session)?
                            .map(|value| value.0);
                        if node.state != preflight.node_state
                            || node.activity_file != preflight.activity_file
                            || current_interaction != preflight.interaction_id
                        {
                            return Ok(None);
                        }
                        runtime
                            .interaction()
                            .request_disconnect_policy(session, *notice, true)
                            .map(Some)
                    },
                )?
                .transpose()?
                .flatten();
            let Some((ticket, first)) = ticket else {
                return reject("preflight-stale");
            };
            if !first {
                return reject("disconnect-already-requested");
            }
            audit(
                &runtime,
                &principal,
                &command_id,
                name,
                Some(target),
                "allowed",
                "succeeded",
                if *notice {
                    "disconnect-notice-requested"
                } else {
                    "disconnect-no-notice-requested"
                },
            )?;
            event(&runtime, "disconnect-requested")?;
            let runtime = runtime.clone();
            let principal = principal.clone();
            let command_id_copy = command_id.clone();
            let target = target.clone();
            let work = runtime.live_controls.track();
            std::thread::spawn(move || {
                let _work = work;
                let finish = || -> Result<(), ApplicationError> {
                    let started = Instant::now();
                    while !ticket.completed()
                        && !ticket.failed()
                        && started.elapsed() < DISCONNECT_GRACE
                    {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    let target_stale = target.daemon_generation != runtime.daemon_generation()
                        || runtime
                            .with_live_target(
                                target.node_id,
                                target.session_id,
                                target.occupancy_generation,
                                |_| (),
                            )?
                            .is_none();
                    if !ticket.completed() && !ticket.failed() && !target_stale {
                        match runtime.emergency_close_session(
                            target.node_id,
                            target.session_id,
                            target.occupancy_generation,
                        ) {
                            Ok(true) => {
                                ticket.mark_fallback();
                                audit(
                                    &runtime,
                                    &principal,
                                    &command_id_copy,
                                    name,
                                    Some(&target),
                                    "allowed",
                                    "succeeded",
                                    "emergency-transport-close",
                                )?;
                                event(&runtime, "emergency-transport-close")?;
                            }
                            Ok(false) => {}
                            Err(_) => {
                                audit(
                                    &runtime,
                                    &principal,
                                    &command_id_copy,
                                    name,
                                    Some(&target),
                                    "allowed",
                                    "failed",
                                    "emergency-close-failed",
                                )?;
                                // A failed owned close must still reach the bounded
                                // final receipt; it is not evidence of completion.
                            }
                        }
                    }
                    let final_wait = Instant::now();
                    while !ticket.completed()
                        && !ticket.failed()
                        && final_wait.elapsed() < Duration::from_secs(5)
                    {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    let class = if ticket.completed() {
                        if ticket.fallback_used() {
                            "session-disconnected-fallback"
                        } else {
                            "session-disconnected"
                        }
                    } else if target_stale {
                        "stale-target"
                    } else {
                        "disconnect-finalization-failed"
                    };
                    let mut database = RuntimeDatabase::open(runtime.database_path())?;
                    database.complete_operator_command(
                        &command_id_copy,
                        class,
                        target.occupancy_generation,
                        now(),
                    )?;
                    audit(
                        &runtime,
                        &principal,
                        &command_id_copy,
                        name,
                        Some(&target),
                        "allowed",
                        if ticket.completed() {
                            "succeeded"
                        } else {
                            "failed"
                        },
                        class,
                    )?;
                    event(&runtime, class)
                };
                if finish().is_err() {
                    runtime
                        .live_controls
                        .evidence_failed
                        .store(true, Ordering::Release);
                    tracing::error!(
                        "operator disconnect finalization evidence could not be persisted"
                    );
                }
            });
            return Ok(MutationResult::LiveControl {
                command_id,
                value: LiveControlResult::Pending {
                    result_class: "disconnect-requested".to_owned(),
                },
            });
        }
        LiveControlAction::PrepareDisconnect { .. }
        | LiveControlAction::PrepareGracefulShutdown { .. }
        | LiveControlAction::RequestGracefulShutdown { .. } => unreachable!(),
    };
    database.complete_operator_command(&command_id, class, 1, now())?;
    audit(
        &runtime,
        &principal,
        &command_id,
        name,
        action.target(),
        "allowed",
        "succeeded",
        class,
    )?;
    event(&runtime, class)?;
    Ok(MutationResult::Completed {
        command_id,
        result_class: class.to_owned(),
    })
}
