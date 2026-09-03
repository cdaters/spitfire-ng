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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use thiserror::Error;

use crate::{CallerId, NodeId, SessionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SysopAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageState {
    Pending,
    Chatting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRequest {
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
    caller_sender: Sender<CallerEvent>,
    operator_receiver: Option<Receiver<OperatorEvent>>,
}

struct InteractionInner {
    availability: SysopAvailability,
    slots: BTreeMap<SessionId, InteractionSlot>,
    disconnects: BTreeMap<SessionId, ()>,
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
                availability: SysopAvailability::Available,
                slots: BTreeMap::new(),
                disconnects: BTreeMap::new(),
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
                    let _ = slot.caller_sender.send(CallerEvent::Declined);
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
        let (caller_sender, caller_receiver) = mpsc::channel();
        let (operator_sender, operator_receiver) = mpsc::channel();
        inner.slots.insert(
            session_id,
            InteractionSlot {
                request: PageRequest {
                    session_id,
                    node_id,
                    caller_id,
                    caller_name: caller_name.to_owned(),
                    requested_at,
                    state: PageState::Pending,
                },
                caller_sender,
                operator_receiver: Some(operator_receiver),
            },
        );
        Ok(PageTicket {
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
        let _ = slot.caller_sender.send(CallerEvent::Declined);
        Ok(())
    }

    pub fn answer(&self, session_id: SessionId) -> Result<OperatorChat, InteractionError> {
        let mut inner = self.lock()?;
        let slot = inner
            .slots
            .get_mut(&session_id)
            .ok_or(InteractionError::UnknownSession(session_id.get()))?;
        if slot.request.state != PageState::Pending {
            return Err(InteractionError::NotPending(session_id.get()));
        }
        let receiver = slot
            .operator_receiver
            .take()
            .ok_or(InteractionError::NotPending(session_id.get()))?;
        slot.request.state = PageState::Chatting;
        slot.caller_sender
            .send(CallerEvent::Answered)
            .map_err(|_| InteractionError::CallerGone(session_id.get()))?;
        Ok(OperatorChat {
            hub: self.clone(),
            session_id,
            caller_sender: slot.caller_sender.clone(),
            operator_receiver: receiver,
            ended: false,
        })
    }

    pub fn request_disconnect(&self, session_id: SessionId) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        inner.disconnects.insert(session_id, ());
        if let Some(slot) = inner.slots.get(&session_id) {
            let _ = slot.caller_sender.send(CallerEvent::Ended);
        }
        Ok(())
    }

    pub fn take_disconnect(&self, session_id: SessionId) -> Result<bool, InteractionError> {
        Ok(self.lock()?.disconnects.remove(&session_id).is_some())
    }

    pub fn session_ended(&self, session_id: SessionId) -> Result<(), InteractionError> {
        let mut inner = self.lock()?;
        inner.disconnects.remove(&session_id);
        if let Some(slot) = inner.slots.remove(&session_id) {
            let _ = slot.caller_sender.send(CallerEvent::Ended);
        }
        Ok(())
    }

    fn remove_slot(&self, session_id: SessionId) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.slots.remove(&session_id);
        }
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
    hub: InteractionHub,
    session_id: SessionId,
    caller_receiver: Option<Receiver<CallerEvent>>,
    operator_sender: Sender<OperatorEvent>,
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
                self.hub.remove_slot(self.session_id);
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
            self.hub.remove_slot(self.session_id);
        }
    }
}

pub struct CallerChat {
    hub: InteractionHub,
    session_id: SessionId,
    caller_receiver: Receiver<CallerEvent>,
    operator_sender: Sender<OperatorEvent>,
    ended: bool,
}

impl CallerChat {
    pub fn send_line(&self, line: &str) -> Result<(), InteractionError> {
        self.operator_sender
            .send(OperatorEvent::CallerLine(line.to_owned()))
            .map_err(|_| InteractionError::OperatorGone(self.session_id.get()))
    }

    pub fn receive_line(&self, timeout: Duration) -> Result<Option<String>, InteractionError> {
        match self.caller_receiver.recv_timeout(timeout) {
            Ok(CallerEvent::OperatorLine(line)) => Ok(Some(line)),
            Ok(CallerEvent::Ended | CallerEvent::Declined) => Ok(None),
            Ok(CallerEvent::Answered) => Err(InteractionError::ProtocolState),
            Err(RecvTimeoutError::Timeout) => Err(InteractionError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn end(mut self) {
        let _ = self.operator_sender.send(OperatorEvent::CallerLeft);
        self.hub.remove_slot(self.session_id);
        self.ended = true;
    }
}

impl Drop for CallerChat {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.operator_sender.send(OperatorEvent::CallerLeft);
            self.hub.remove_slot(self.session_id);
        }
    }
}

pub struct OperatorChat {
    hub: InteractionHub,
    session_id: SessionId,
    caller_sender: Sender<CallerEvent>,
    operator_receiver: Receiver<OperatorEvent>,
    ended: bool,
}

impl OperatorChat {
    pub fn receive_line(&self, timeout: Duration) -> Result<Option<String>, InteractionError> {
        match self.operator_receiver.recv_timeout(timeout) {
            Ok(OperatorEvent::CallerLine(line)) => Ok(Some(line)),
            Ok(OperatorEvent::CallerLeft) => Ok(None),
            Err(RecvTimeoutError::Timeout) => Err(InteractionError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn send_line(&self, line: &str) -> Result<(), InteractionError> {
        self.caller_sender
            .send(CallerEvent::OperatorLine(line.to_owned()))
            .map_err(|_| InteractionError::CallerGone(self.session_id.get()))
    }

    pub fn end(mut self) {
        let _ = self.caller_sender.send(CallerEvent::Ended);
        self.hub.remove_slot(self.session_id);
        self.ended = true;
    }
}

impl Drop for OperatorChat {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.caller_sender.send(CallerEvent::Ended);
            self.hub.remove_slot(self.session_id);
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum InteractionError {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

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
