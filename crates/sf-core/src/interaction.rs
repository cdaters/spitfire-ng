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

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::{CallerId, NodeId, SessionId};

pub const MAX_CHAT_LINE_BYTES: usize = 512;
pub const CHAT_QUEUE_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SysopAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageState {
    Pending,
    Invited,
    Chatting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRequest {
    pub interaction_id: u64,
    pub session_id: SessionId,
    pub node_id: NodeId,
    pub caller_id: CallerId,
    pub caller_name: String,
    pub requested_at: i64,
    pub state: PageState,
}

enum CallerEvent {
    Answered,
    Declined,
    OperatorLine(String),
    Ended,
}

enum OperatorEvent {
    CallerLine(String),
    CallerLeft,
}

struct InteractionSlot {
    request: PageRequest,
    caller_sender: SyncSender<CallerEvent>,
    operator_receiver: Option<Receiver<OperatorEvent>>,
    invitation: Option<InvitationChannels>,
    owner: Option<String>,
}

pub type ChatAuthorization = Arc<dyn Fn() -> bool + Send + Sync>;

struct InvitationChannels {
    caller_receiver: Receiver<CallerEvent>,
    operator_sender: SyncSender<OperatorEvent>,
    authorize: ChatAuthorization,
}

struct InteractionInner {
    next_interaction: u64,
    availability: SysopAvailability,
    slots: BTreeMap<SessionId, InteractionSlot>,
    disconnects: BTreeMap<SessionId, DisconnectRequest>,
    allowance_pauses: BTreeMap<SessionId, (Duration, Option<Instant>)>,
}

#[derive(Clone)]
pub struct DisconnectTicket {
    state: Arc<AtomicU8>,
    fallback: Arc<AtomicBool>,
}

impl DisconnectTicket {
    pub fn completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }
    pub fn failed(&self) -> bool {
        self.state.load(Ordering::Acquire) == 2
    }
    pub fn fallback_used(&self) -> bool {
        self.fallback.load(Ordering::Acquire)
    }
    pub fn mark_fallback(&self) {
        self.fallback.store(true, Ordering::Release);
    }
}

struct DisconnectRequest {
    notice: bool,
    board_shutdown: bool,
    consumed: bool,
    correlated: bool,
    ticket: DisconnectTicket,
}

/// In-process operator/caller coordination keyed by stable session identity.
/// Node numbers are presentation data and can be reused only after the owning
/// session is gone.
#[derive(Clone)]
pub struct InteractionHub {
    inner: Arc<Mutex<InteractionInner>>,
}

impl Default for InteractionHub {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractionHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InteractionInner {
                next_interaction: 1,
                availability: SysopAvailability::Available,
                slots: BTreeMap::new(),
                disconnects: BTreeMap::new(),
                allowance_pauses: BTreeMap::new(),
            })),
        }
    }

    pub fn availability(&self) -> Result<SysopAvailability, InteractionError> {
        Ok(self.lock()?.availability)
    }

    pub fn set_availability(
        &self,
        availability: SysopAvailability,
    ) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        inner.availability = availability;
        if availability == SysopAvailability::Unavailable {
            let pending: Vec<_> = inner
                .slots
                .iter()
                .filter(|(_, slot)| slot.request.state == PageState::Pending)
                .map(|(session, _)| *session)
                .collect();
            for session in pending {
                if let Some(slot) = inner.slots.remove(&session) {
                    let _ = slot.caller_sender.try_send(CallerEvent::Declined);
                }
            }
        }
        Ok(())
    }

    pub fn request_page(
        &self,
        session_id: SessionId,
        node_id: NodeId,
        caller_id: CallerId,
        caller_name: &str,
        requested_at: i64,
    ) -> Result<PageTicket, InteractionError> {
        let mut inner = self.lock()?;
        if inner.availability == SysopAvailability::Unavailable {
            return Err(InteractionError::SysopUnavailable);
        }
        if inner.slots.contains_key(&session_id) {
            return Err(InteractionError::AlreadyPaged(session_id.get()));
        }
        let interaction_id = inner.next_interaction;
        inner.next_interaction = interaction_id
            .checked_add(1)
            .ok_or(InteractionError::ProtocolState)?;
        let (caller_sender, caller_receiver) = mpsc::sync_channel(CHAT_QUEUE_CAPACITY);
        let (operator_sender, operator_receiver) = mpsc::sync_channel(CHAT_QUEUE_CAPACITY);
        inner.slots.insert(
            session_id,
            InteractionSlot {
                request: PageRequest {
                    interaction_id,
                    session_id,
                    node_id,
                    caller_id,
                    caller_name: caller_name.to_owned(),
                    requested_at,
                    state: PageState::Pending,
                },
                caller_sender,
                operator_receiver: Some(operator_receiver),
                invitation: None,
                owner: None,
            },
        );
        Ok(PageTicket {
            interaction_id,
            hub: self.clone(),
            session_id,
            caller_receiver: Some(caller_receiver),
            operator_sender,
            finished: false,
        })
    }

    pub fn pages(&self) -> Result<Vec<PageRequest>, InteractionError> {
        Ok(self
            .lock()?
            .slots
            .values()
            .filter(|slot| slot.request.state != PageState::Invited)
            .map(|slot| slot.request.clone())
            .collect())
    }

    pub fn decline(&self, session_id: SessionId) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        let slot = inner
            .slots
            .get(&session_id)
            .ok_or(InteractionError::UnknownSession(session_id.get()))?;
        if slot.request.state != PageState::Pending {
            return Err(InteractionError::NotPending(session_id.get()));
        }
        let slot = inner
            .slots
            .remove(&session_id)
            .expect("page was checked before removal");
        let _ = slot.caller_sender.try_send(CallerEvent::Declined);
        Ok(())
    }

    pub fn answer(&self, session_id: SessionId) -> Result<OperatorChat, InteractionError> {
        self.answer_owned(session_id, None)
    }

    pub fn answer_owned(
        &self,
        session_id: SessionId,
        owner: Option<String>,
    ) -> Result<OperatorChat, InteractionError> {
        let mut inner = self.lock()?;
        let slot = inner
            .slots
            .get_mut(&session_id)
            .ok_or(InteractionError::UnknownSession(session_id.get()))?;
        if slot.request.state != PageState::Pending {
            return Err(InteractionError::NotPending(session_id.get()));
        }
        slot.owner = owner;
        let receiver = slot
            .operator_receiver
            .take()
            .ok_or(InteractionError::NotPending(session_id.get()))?;
        slot.request.state = PageState::Chatting;
        slot.caller_sender
            .try_send(CallerEvent::Answered)
            .map_err(|_| InteractionError::CallerGone(session_id.get()))?;
        Ok(OperatorChat {
            interaction_id: slot.request.interaction_id,
            hub: self.clone(),
            session_id,
            caller_sender: slot.caller_sender.clone(),
            operator_receiver: Mutex::new(receiver),
            ended: false,
        })
    }

    /// An invitation uses the same slot and bounded channels as a caller page.
    /// Only the caller can accept it; ordinary page answer cannot force it.
    pub fn invite(
        &self,
        mut request: PageRequest,
        owner: String,
        authorize: ChatAuthorization,
    ) -> Result<OperatorChat, InteractionError> {
        let mut inner = self.lock()?;
        if inner.slots.contains_key(&request.session_id)
            || inner.disconnects.contains_key(&request.session_id)
        {
            return Err(InteractionError::AlreadyPaged(request.session_id.get()));
        }
        request.interaction_id = inner.next_interaction;
        inner.next_interaction = inner
            .next_interaction
            .checked_add(1)
            .ok_or(InteractionError::ProtocolState)?;
        request.state = PageState::Invited;
        let (caller_sender, caller_receiver) = mpsc::sync_channel(CHAT_QUEUE_CAPACITY);
        let (operator_sender, operator_receiver) = mpsc::sync_channel(CHAT_QUEUE_CAPACITY);
        let chat = OperatorChat {
            hub: self.clone(),
            session_id: request.session_id,
            interaction_id: request.interaction_id,
            caller_sender: caller_sender.clone(),
            operator_receiver: Mutex::new(operator_receiver),
            ended: false,
        };
        inner.slots.insert(
            request.session_id,
            InteractionSlot {
                request,
                caller_sender,
                operator_receiver: None,
                invitation: Some(InvitationChannels {
                    caller_receiver,
                    operator_sender,
                    authorize,
                }),
                owner: Some(owner),
            },
        );
        Ok(chat)
    }

    pub fn invitation_pending(&self, session_id: SessionId) -> Result<bool, InteractionError> {
        Ok(self
            .lock()?
            .slots
            .get(&session_id)
            .is_some_and(|slot| slot.request.state == PageState::Invited))
    }

    pub fn answer_invitation(
        &self,
        session_id: SessionId,
        accept: bool,
    ) -> Result<Option<CallerChat>, InteractionError> {
        let interaction_id = self
            .interaction_state(session_id)?
            .ok_or(InteractionError::UnknownSession(session_id.get()))?
            .0;
        self.answer_invitation_exact(session_id, interaction_id, accept)
    }

    pub fn answer_invitation_exact(
        &self,
        session_id: SessionId,
        interaction_id: u64,
        accept: bool,
    ) -> Result<Option<CallerChat>, InteractionError> {
        // Reauthorize immediately before the transition, without holding the
        // interaction mutex across application policy/database access.
        let authorize = {
            let inner = self.lock()?;
            let slot = inner
                .slots
                .get(&session_id)
                .filter(|slot| slot.request.interaction_id == interaction_id)
                .ok_or(InteractionError::UnknownSession(session_id.get()))?;
            let invitation = slot
                .invitation
                .as_ref()
                .ok_or(InteractionError::NotPending(session_id.get()))?;
            invitation.authorize.clone()
        };
        let allowed = accept && authorize();
        let mut inner = self.lock()?;
        let slot = inner
            .slots
            .get_mut(&session_id)
            .filter(|slot| {
                slot.request.interaction_id == interaction_id
                    && slot.request.state == PageState::Invited
            })
            .ok_or(InteractionError::NotPending(session_id.get()))?;
        if !allowed {
            inner.slots.remove(&session_id);
            return Ok(None);
        }
        let invitation = slot
            .invitation
            .take()
            .ok_or(InteractionError::NotPending(session_id.get()))?;
        slot.request.state = PageState::Chatting;
        Ok(Some(CallerChat {
            hub: self.clone(),
            session_id,
            interaction_id,
            caller_receiver: invitation.caller_receiver,
            operator_sender: invitation.operator_sender,
            ended: false,
        }))
    }

    pub fn end_operator_attachment(&self, owner: &str) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        inner
            .slots
            .retain(|_, slot| slot.owner.as_deref() != Some(owner));
        Ok(())
    }

    pub fn interaction_state(
        &self,
        session_id: SessionId,
    ) -> Result<Option<(u64, PageState)>, InteractionError> {
        Ok(self
            .lock()?
            .slots
            .get(&session_id)
            .map(|slot| (slot.request.interaction_id, slot.request.state)))
    }

    pub fn request_disconnect(&self, session_id: SessionId) -> Result<(), InteractionError> {
        self.request_disconnect_policy(session_id, true, false)
            .map(|_| ())
    }

    pub fn request_disconnect_policy(
        &self,
        session_id: SessionId,
        notice: bool,
        correlated: bool,
    ) -> Result<(DisconnectTicket, bool), InteractionError> {
        self.request_disconnect_mode(session_id, notice, correlated, false)
    }

    fn request_disconnect_mode(
        &self,
        session_id: SessionId,
        notice: bool,
        correlated: bool,
        board_shutdown: bool,
    ) -> Result<(DisconnectTicket, bool), InteractionError> {
        let mut inner = self.lock()?;
        if let Some(existing) = inner.disconnects.get(&session_id) {
            return Ok((existing.ticket.clone(), false));
        }
        let ticket = DisconnectTicket {
            state: Arc::new(AtomicU8::new(0)),
            fallback: Arc::new(AtomicBool::new(false)),
        };
        inner.disconnects.insert(
            session_id,
            DisconnectRequest {
                notice,
                board_shutdown,
                consumed: false,
                correlated,
                ticket: ticket.clone(),
            },
        );
        if let Some(slot) = inner.slots.remove(&session_id) {
            let _ = slot.caller_sender.try_send(CallerEvent::Ended);
        }
        Ok((ticket, true))
    }

    pub fn take_disconnect(&self, session_id: SessionId) -> Result<bool, InteractionError> {
        Ok(self.take_disconnect_notice(session_id)?.is_some())
    }

    /// Reuse the session-owned cancellation ticket, with a distinct board notice.
    pub fn request_board_shutdown(
        &self,
        session_id: SessionId,
    ) -> Result<DisconnectTicket, InteractionError> {
        self.request_disconnect_mode(session_id, true, true, true)
            .map(|(ticket, _)| ticket)
    }

    pub fn board_shutdown_pending(&self, session_id: SessionId) -> Result<bool, InteractionError> {
        Ok(self
            .lock()?
            .disconnects
            .get(&session_id)
            .is_some_and(|r| r.board_shutdown))
    }

    pub fn disconnect_pending(&self, session_id: SessionId) -> Result<bool, InteractionError> {
        Ok(self.lock()?.disconnects.contains_key(&session_id))
    }

    pub fn take_disconnect_notice(
        &self,
        session_id: SessionId,
    ) -> Result<Option<bool>, InteractionError> {
        let mut inner = self.lock()?;
        Ok(inner.disconnects.get_mut(&session_id).and_then(|request| {
            if request.consumed {
                None
            } else {
                request.consumed = true;
                Some(request.notice)
            }
        }))
    }

    pub fn disconnect_finalized(
        &self,
        session_id: SessionId,
        succeeded: bool,
    ) -> Result<(), InteractionError> {
        if let Some(request) = self.lock()?.disconnects.remove(&session_id) {
            request
                .ticket
                .state
                .store(if succeeded { 1 } else { 2 }, Ordering::Release);
        }
        Ok(())
    }

    pub fn session_ended(&self, session_id: SessionId) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        if inner
            .disconnects
            .get(&session_id)
            .is_some_and(|request| !request.correlated)
        {
            inner.disconnects.remove(&session_id);
        }
        inner.allowance_pauses.remove(&session_id);
        if let Some(slot) = inner.slots.remove(&session_id) {
            let _ = slot.caller_sender.try_send(CallerEvent::Ended);
        }
        Ok(())
    }

    pub fn pause_allowance(
        &self,
        session_id: SessionId,
    ) -> Result<AllowancePause, InteractionError> {
        let mut inner = self.lock()?;
        let pause = inner
            .allowance_pauses
            .entry(session_id)
            .or_insert((Duration::ZERO, None));
        if pause.1.is_some() {
            return Err(InteractionError::AlreadyCompleted);
        }
        pause.1 = Some(Instant::now());
        Ok(AllowancePause {
            hub: self.clone(),
            session_id,
        })
    }

    pub fn paused_allowance(&self, session_id: SessionId) -> Result<Duration, InteractionError> {
        Ok(self.lock()?.allowance_pauses.get(&session_id).map_or(
            Duration::ZERO,
            |(total, active)| {
                total.saturating_add(active.map_or(Duration::ZERO, |started| started.elapsed()))
            },
        ))
    }

    fn remove_slot(&self, session_id: SessionId, interaction_id: u64) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner
                .slots
                .get(&session_id)
                .is_some_and(|slot| slot.request.interaction_id == interaction_id)
            {
                inner.slots.remove(&session_id);
            }
        }
    }

    fn contains(
        &self,
        session_id: SessionId,
        interaction_id: u64,
    ) -> Result<bool, InteractionError> {
        Ok(self
            .lock()?
            .slots
            .get(&session_id)
            .is_some_and(|slot| slot.request.interaction_id == interaction_id))
    }

    fn lock(&self) -> Result<MutexGuard<'_, InteractionInner>, InteractionError> {
        self.inner
            .lock()
            .map_err(|_| InteractionError::CoordinationPoisoned)
    }
}

pub enum PageAnswer {
    Accepted(CallerChat),
    Declined,
    TimedOut,
}

pub struct PageTicket {
    interaction_id: u64,
    hub: InteractionHub,
    session_id: SessionId,
    caller_receiver: Option<Receiver<CallerEvent>>,
    operator_sender: SyncSender<OperatorEvent>,
    finished: bool,
}

impl PageTicket {
    pub fn wait(mut self, timeout: Duration) -> Result<PageAnswer, InteractionError> {
        let receiver = self
            .caller_receiver
            .take()
            .ok_or(InteractionError::AlreadyCompleted)?;
        match receiver.recv_timeout(timeout) {
            Ok(CallerEvent::Answered) => {
                self.finished = true;
                Ok(PageAnswer::Accepted(CallerChat {
                    interaction_id: self.interaction_id,
                    hub: self.hub.clone(),
                    session_id: self.session_id,
                    caller_receiver: receiver,
                    operator_sender: self.operator_sender.clone(),
                    ended: false,
                }))
            }
            Ok(CallerEvent::Declined | CallerEvent::Ended) => {
                self.finished = true;
                Ok(PageAnswer::Declined)
            }
            Ok(CallerEvent::OperatorLine(_)) => Err(InteractionError::ProtocolState),
            Err(RecvTimeoutError::Timeout) => {
                self.hub.remove_slot(self.session_id, self.interaction_id);
                self.finished = true;
                Ok(PageAnswer::TimedOut)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.finished = true;
                Ok(PageAnswer::Declined)
            }
        }
    }
}

impl Drop for PageTicket {
    fn drop(&mut self) {
        if !self.finished {
            self.hub.remove_slot(self.session_id, self.interaction_id);
        }
    }
}

pub struct CallerChat {
    interaction_id: u64,
    hub: InteractionHub,
    session_id: SessionId,
    caller_receiver: Receiver<CallerEvent>,
    operator_sender: SyncSender<OperatorEvent>,
    ended: bool,
}

impl CallerChat {
    pub fn send_line(&self, line: &str) -> Result<(), InteractionError> {
        if !self.hub.contains(self.session_id, self.interaction_id)? {
            return Err(InteractionError::OperatorGone(self.session_id.get()));
        }
        validate_chat_line(line)?;
        self.operator_sender
            .try_send(OperatorEvent::CallerLine(line.to_owned()))
            .map_err(|error| match error {
                TrySendError::Full(_) => InteractionError::Backpressure,
                TrySendError::Disconnected(_) => {
                    InteractionError::OperatorGone(self.session_id.get())
                }
            })
    }

    pub fn receive_line(&self, timeout: Duration) -> Result<Option<String>, InteractionError> {
        if !self.hub.contains(self.session_id, self.interaction_id)? {
            return Ok(None);
        }
        match self.caller_receiver.recv_timeout(timeout) {
            Ok(CallerEvent::OperatorLine(line)) => Ok(Some(line)),
            Ok(CallerEvent::Ended | CallerEvent::Declined) => Ok(None),
            Ok(CallerEvent::Answered) => Err(InteractionError::ProtocolState),
            Err(RecvTimeoutError::Timeout) => Err(InteractionError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn end(mut self) {
        let _ = self.operator_sender.try_send(OperatorEvent::CallerLeft);
        self.hub.remove_slot(self.session_id, self.interaction_id);
        self.ended = true;
    }
}

impl Drop for CallerChat {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.operator_sender.try_send(OperatorEvent::CallerLeft);
            self.hub.remove_slot(self.session_id, self.interaction_id);
        }
    }
}

pub struct OperatorChat {
    interaction_id: u64,
    hub: InteractionHub,
    session_id: SessionId,
    caller_sender: SyncSender<CallerEvent>,
    operator_receiver: Mutex<Receiver<OperatorEvent>>,
    ended: bool,
}

impl OperatorChat {
    pub fn state(&self) -> Result<Option<PageState>, InteractionError> {
        Ok(self
            .hub
            .interaction_state(self.session_id)?
            .filter(|(id, _)| *id == self.interaction_id)
            .map(|(_, state)| state))
    }
    pub fn receive_line(&self, timeout: Duration) -> Result<Option<String>, InteractionError> {
        if !self.hub.contains(self.session_id, self.interaction_id)? {
            return Ok(None);
        }
        match self
            .operator_receiver
            .lock()
            .map_err(|_| InteractionError::CoordinationPoisoned)?
            .recv_timeout(timeout)
        {
            Ok(OperatorEvent::CallerLine(line)) => Ok(Some(line)),
            Ok(OperatorEvent::CallerLeft) => Ok(None),
            Err(RecvTimeoutError::Timeout) => Err(InteractionError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn send_line(&self, line: &str) -> Result<(), InteractionError> {
        if !self.hub.contains(self.session_id, self.interaction_id)? {
            return Err(InteractionError::CallerGone(self.session_id.get()));
        }
        if self.hub.interaction_state(self.session_id)?
            != Some((self.interaction_id, PageState::Chatting))
        {
            return Err(InteractionError::NotPending(self.session_id.get()));
        }
        validate_chat_line(line)?;
        self.caller_sender
            .try_send(CallerEvent::OperatorLine(line.to_owned()))
            .map_err(|error| match error {
                TrySendError::Full(_) => InteractionError::Backpressure,
                TrySendError::Disconnected(_) => {
                    InteractionError::CallerGone(self.session_id.get())
                }
            })
    }

    pub fn end(mut self) {
        let _ = self.caller_sender.try_send(CallerEvent::Ended);
        self.hub.remove_slot(self.session_id, self.interaction_id);
        self.ended = true;
    }
}

/// Pauses allowance consumption, not factual connection/accounting duration.
pub struct AllowancePause {
    hub: InteractionHub,
    session_id: SessionId,
}

impl Drop for AllowancePause {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.hub.inner.lock() {
            if let Some((total, started)) = inner.allowance_pauses.get_mut(&self.session_id) {
                if let Some(started) = started.take() {
                    *total = total.saturating_add(started.elapsed());
                }
            }
        }
    }
}

impl Drop for OperatorChat {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.caller_sender.try_send(CallerEvent::Ended);
            self.hub.remove_slot(self.session_id, self.interaction_id);
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum InteractionError {
    #[error("chat line exceeds its bound or contains terminal control characters")]
    InvalidLine,
    #[error("chat peer must consume queued lines before another line is sent")]
    Backpressure,
    #[error("the Sysop page is unavailable")]
    SysopUnavailable,
    #[error("session {0} already has an outstanding page")]
    AlreadyPaged(u64),
    #[error("session {0} has no page or chat")]
    UnknownSession(u64),
    #[error("session {0} does not have a pending page")]
    NotPending(u64),
    #[error("caller session {0} disconnected")]
    CallerGone(u64),
    #[error("operator left caller session {0}")]
    OperatorGone(u64),
    #[error("page or chat timed out")]
    TimedOut,
    #[error("page ticket was already completed")]
    AlreadyCompleted,
    #[error("page/chat protocol entered an impossible state")]
    ProtocolState,
    #[error("page/chat coordination lock was poisoned")]
    CoordinationPoisoned,
}

fn validate_chat_line(line: &str) -> Result<(), InteractionError> {
    if line.len() > MAX_CHAT_LINE_BYTES || line.chars().any(char::is_control) {
        return Err(InteractionError::InvalidLine);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn invitations_require_caller_consent_current_authorization_and_one_owner() {
        let hub = InteractionHub::new();
        let session = SessionId::new(1).unwrap();
        let request = PageRequest {
            interaction_id: 0,
            session_id: session,
            node_id: NodeId::new(1).unwrap(),
            caller_id: CallerId::new(1).unwrap(),
            caller_name: "Public Caller".to_owned(),
            requested_at: 100,
            state: PageState::Invited,
        };
        let allowed = Arc::new(AtomicBool::new(true));
        let policy = allowed.clone();
        let operator = hub
            .invite(
                request.clone(),
                "attachment-a".to_owned(),
                Arc::new(move || policy.load(Ordering::Acquire)),
            )
            .unwrap();
        assert!(hub.invitation_pending(session).unwrap());
        assert!(hub.pages().unwrap().is_empty());
        assert!(
            hub.answer(session).is_err(),
            "page answer must not force an invitation"
        );
        assert!(operator.send_line("before consent").is_err());
        assert!(hub
            .invite(
                request.clone(),
                "attachment-b".to_owned(),
                Arc::new(|| true)
            )
            .is_err());
        allowed.store(false, Ordering::Release);
        assert!(hub.answer_invitation(session, true).unwrap().is_none());
        assert_eq!(operator.receive_line(Duration::ZERO).unwrap(), None);
        let replacement = hub
            .invite(request, "attachment-b".to_owned(), Arc::new(|| true))
            .unwrap();
        assert!(
            hub.answer_invitation_exact(session, operator.interaction_id, true)
                .is_err(),
            "a stale caller acceptance cannot accept a replacement invitation"
        );
        drop(operator);
        assert!(
            hub.invitation_pending(session).unwrap(),
            "old handles cannot erase a newer interaction"
        );
        let caller = hub.answer_invitation(session, true).unwrap().unwrap();
        replacement.send_line("consented message").unwrap();
        assert_eq!(
            caller.receive_line(Duration::ZERO).unwrap().as_deref(),
            Some("consented message")
        );
        hub.end_operator_attachment("attachment-b").unwrap();
        assert_eq!(caller.receive_line(Duration::ZERO).unwrap(), None);
        assert!(caller.send_line("after owner loss").is_err());
    }

    #[test]
    fn bounded_chat_rejects_controls_and_backpressures_without_blocking_end() {
        let hub = InteractionHub::new();
        let ticket = request(&hub, 1, 1);
        let operator = hub.answer(SessionId::new(1).unwrap()).unwrap();
        let PageAnswer::Accepted(caller) = ticket.wait(Duration::ZERO).unwrap() else {
            panic!("chat")
        };
        assert_eq!(
            operator.send_line(&"x".repeat(MAX_CHAT_LINE_BYTES + 1)),
            Err(InteractionError::InvalidLine)
        );
        assert_eq!(
            caller.send_line("\x1b[2J"),
            Err(InteractionError::InvalidLine)
        );
        for _ in 0..CHAT_QUEUE_CAPACITY {
            operator.send_line("bounded").unwrap();
        }
        assert_eq!(
            operator.send_line("overflow"),
            Err(InteractionError::Backpressure)
        );
        operator.end();
        assert_eq!(caller.receive_line(Duration::ZERO).unwrap(), None);
    }

    #[test]
    fn allowance_pause_is_single_owned_and_disconnect_cleanup_is_correlated() {
        let hub = InteractionHub::new();
        let session = SessionId::new(1).unwrap();
        let pause = hub.pause_allowance(session).unwrap();
        assert!(hub.pause_allowance(session).is_err());
        std::thread::sleep(Duration::from_millis(10));
        assert!(hub.paused_allowance(session).unwrap() >= Duration::from_millis(10));
        drop(pause);
        let paused = hub.paused_allowance(session).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(hub.paused_allowance(session).unwrap(), paused);
        let (ticket, first) = hub.request_disconnect_policy(session, false, true).unwrap();
        assert!(first);
        assert!(
            !hub.request_disconnect_policy(session, true, true)
                .unwrap()
                .1
        );
        assert_eq!(hub.take_disconnect_notice(session).unwrap(), Some(false));
        assert_eq!(hub.take_disconnect_notice(session).unwrap(), None);
        hub.session_ended(session).unwrap();
        assert!(
            !ticket.completed(),
            "interaction cleanup must not claim accounting finalized"
        );
        assert_eq!(hub.paused_allowance(session).unwrap(), Duration::ZERO);
        hub.disconnect_finalized(session, true).unwrap();
        assert!(ticket.completed());
        assert!(!hub.disconnect_pending(session).unwrap());
    }

    #[test]
    fn caller_loss_ends_only_its_current_chat() {
        let hub = InteractionHub::new();
        let ticket = request(&hub, 81, 1);
        let operator = hub.answer(SessionId::new(81).unwrap()).unwrap();
        let PageAnswer::Accepted(caller) = ticket.wait(Duration::ZERO).unwrap() else {
            panic!("chat expected")
        };
        drop(caller);
        assert_eq!(operator.state().unwrap(), None);
        assert_eq!(operator.receive_line(Duration::ZERO).unwrap(), None);
        assert!(operator.send_line("after caller loss").is_err());
        let replacement = request(&hub, 81, 1);
        drop(operator);
        assert_eq!(hub.pages().unwrap().len(), 1);
        drop(replacement);
    }

    fn request(hub: &InteractionHub, session: u64, node: u32) -> PageTicket {
        hub.request_page(
            SessionId::new(session).unwrap(),
            NodeId::new(node).unwrap(),
            CallerId::new(session as i64).unwrap(),
            &format!("Caller {session}"),
            100,
        )
        .unwrap()
    }

    #[test]
    fn unavailable_duplicate_decline_and_timeout_are_bounded() {
        let hub = InteractionHub::new();
        hub.set_availability(SysopAvailability::Unavailable)
            .unwrap();
        assert!(matches!(
            hub.request_page(
                SessionId::new(1).unwrap(),
                NodeId::new(1).unwrap(),
                CallerId::new(1).unwrap(),
                "Caller",
                0
            ),
            Err(InteractionError::SysopUnavailable)
        ));
        hub.set_availability(SysopAvailability::Available).unwrap();
        let ticket = request(&hub, 1, 1);
        assert!(matches!(
            hub.request_page(
                SessionId::new(1).unwrap(),
                NodeId::new(1).unwrap(),
                CallerId::new(1).unwrap(),
                "Caller",
                0
            ),
            Err(InteractionError::AlreadyPaged(1))
        ));
        hub.decline(SessionId::new(1).unwrap()).unwrap();
        assert!(matches!(
            ticket.wait(Duration::from_millis(50)).unwrap(),
            PageAnswer::Declined
        ));
        assert!(matches!(
            request(&hub, 2, 2).wait(Duration::from_millis(1)).unwrap(),
            PageAnswer::TimedOut
        ));
    }

    #[test]
    fn two_pages_chat_independently_and_stale_sessions_cannot_attach() {
        let hub = InteractionHub::new();
        let first = request(&hub, 11, 1);
        let second = request(&hub, 12, 2);
        assert_eq!(hub.pages().unwrap().len(), 2);
        let operator = hub.answer(SessionId::new(11).unwrap()).unwrap();
        let caller_thread = thread::spawn(move || {
            let PageAnswer::Accepted(chat) = first.wait(Duration::from_secs(1)).unwrap() else {
                panic!("page not accepted")
            };
            chat.send_line("Hello Sysop").unwrap();
            assert_eq!(
                chat.receive_line(Duration::from_secs(1))
                    .unwrap()
                    .as_deref(),
                Some("Hello Caller")
            );
            chat.end();
        });
        assert_eq!(
            operator
                .receive_line(Duration::from_secs(1))
                .unwrap()
                .as_deref(),
            Some("Hello Sysop")
        );
        operator.send_line("Hello Caller").unwrap();
        caller_thread.join().unwrap();
        assert!(matches!(
            second.wait(Duration::from_millis(1)).unwrap(),
            PageAnswer::TimedOut
        ));
        assert!(matches!(
            hub.answer(SessionId::new(11).unwrap()),
            Err(InteractionError::UnknownSession(11))
        ));

        let third = request(&hub, 13, 3);
        let operator = hub.answer(SessionId::new(13).unwrap()).unwrap();
        let PageAnswer::Accepted(caller) = third.wait(Duration::from_secs(1)).unwrap() else {
            panic!("third page was not accepted")
        };
        hub.session_ended(SessionId::new(13).unwrap()).unwrap();
        assert_eq!(caller.receive_line(Duration::from_secs(1)).unwrap(), None);
        drop(caller);
        assert_eq!(
            operator.receive_line(Duration::from_millis(1)).unwrap(),
            None
        );

        hub.request_disconnect(SessionId::new(14).unwrap()).unwrap();
        assert!(hub.take_disconnect(SessionId::new(14).unwrap()).unwrap());
        assert!(!hub.take_disconnect(SessionId::new(14).unwrap()).unwrap());
    }
}
