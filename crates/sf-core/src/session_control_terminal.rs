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

use std::time::{Duration, Instant};

use crate::{
    CallerChat, InteractionError, InteractionHub, LocalizationArgs, SessionId,
    SessionStatusObserver, Terminal, TerminalError, TerminalInfo, MAX_CHAT_LINE_BYTES,
};

const CONTROL_POLL: Duration = Duration::from_millis(100);
pub const CHAT_INVITATION_TIMEOUT: Duration = Duration::from_secs(30);
const CHAT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Session-owned cooperative input wrapper. It does not inspect or forward
/// terminal bytes to an operator: only explicitly accepted chat lines leave
/// the caller through InteractionHub. Partial menu input stays on this stack.
pub struct SessionControlTerminal<'a> {
    inner: &'a mut dyn Terminal,
    hub: &'a InteractionHub,
    status: &'a dyn SessionStatusObserver,
    session: SessionId,
    idle: Duration,
    invitation_context: bool,
    skip_lf: bool,
    skip_decision_cr: bool,
    binary: bool,
    cancel_sent: bool,
}

impl<'a> SessionControlTerminal<'a> {
    pub fn new(
        inner: &'a mut dyn Terminal,
        hub: &'a InteractionHub,
        status: &'a dyn SessionStatusObserver,
        session: SessionId,
    ) -> Self {
        Self {
            inner,
            hub,
            status,
            session,
            idle: Duration::from_secs(300),
            invitation_context: false,
            skip_lf: false,
            skip_decision_cr: false,
            binary: false,
            cancel_sent: false,
        }
    }

    fn cancelled(&mut self) -> Result<(), TerminalError> {
        if self
            .hub
            .disconnect_pending(self.session)
            .map_err(interaction_error)?
        {
            if self.binary && !self.cancel_sent {
                self.cancel_sent = true;
                // Standard bounded cancellation sequence understood by the
                // existing X/Y/ZMODEM engines; never presentation text in binary mode.
                self.inner.write_binary(&[0x18; 8])?;
            }
            return Err(TerminalError::OperatorCancelled);
        }
        Ok(())
    }

    fn invitation(&mut self) -> Result<bool, TerminalError> {
        if !self.invitation_context {
            return Ok(false);
        }
        let Some((interaction_id, crate::PageState::Invited)) = self
            .hub
            .interaction_state(self.session)
            .map_err(interaction_error)?
        else {
            return Ok(false);
        };
        write_text(
            self.inner,
            "caller-operator-chat-invitation",
            &LocalizationArgs::new(),
        )?;
        let started = Instant::now();
        let (accepted, decision_key) = loop {
            self.cancelled()?;
            if started.elapsed() >= CHAT_INVITATION_TIMEOUT
                || self
                    .hub
                    .interaction_state(self.session)
                    .map_err(interaction_error)?
                    != Some((interaction_id, crate::PageState::Invited))
            {
                break (false, false);
            }
            match self.inner.read_input_byte(CONTROL_POLL) {
                Ok(Some(b'Y' | b'y')) => break (true, true),
                Ok(Some(b'N' | b'n' | 0x1b)) => break (false, true),
                Ok(Some(_)) | Err(TerminalError::TimedOut) => {}
                Ok(None) => return Err(TerminalError::Disconnected),
                Err(error) => return Err(error),
            }
        };
        let chat = match self
            .hub
            .answer_invitation_exact(self.session, interaction_id, accepted)
        {
            Ok(chat) => chat,
            Err(InteractionError::UnknownSession(_) | InteractionError::NotPending(_)) => None,
            Err(error) => return Err(interaction_error(error)),
        };
        if let Some(chat) = chat {
            self.status
                .chat_started()
                .map_err(|_| TerminalError::MalformedProtocol("session status unavailable"))?;
            let pause = self
                .hub
                .pause_allowance(self.session)
                .map_err(interaction_error)?;
            let result = run_attached_caller_chat(self.inner, self.hub, self.session, chat);
            drop(pause);
            self.status
                .interaction_finished()
                .map_err(|_| TerminalError::MalformedProtocol("session status unavailable"))?;
            result?;
        } else {
            self.skip_decision_cr = decision_key;
            write_text(
                self.inner,
                "caller-operator-chat-declined",
                &LocalizationArgs::new(),
            )?;
        }
        self.skip_lf = true;
        write_text(
            self.inner,
            "caller-operator-chat-return",
            &LocalizationArgs::new(),
        )?;
        Ok(true)
    }

    fn line(&mut self, maximum: usize, secret: bool) -> Result<Option<Vec<u8>>, TerminalError> {
        let mut line = Vec::new();
        let mut actual = 0usize;
        let mut overlong = false;
        let mut last_input = Instant::now();
        loop {
            self.cancelled()?;
            if !secret && self.invitation()? {
                last_input = Instant::now();
                if self.inner.echoes_input() {
                    self.inner.write_all(&line)?;
                }
            }
            let byte = match self.inner.read_input_byte(CONTROL_POLL) {
                Ok(Some(byte)) => byte,
                Ok(None) => return Ok((!line.is_empty()).then_some(line)),
                Err(TerminalError::InputPollingUnsupported) => {
                    return if secret {
                        self.inner.read_secret_line(maximum)
                    } else {
                        self.inner.read_line(maximum)
                    }
                }
                Err(TerminalError::TimedOut) if last_input.elapsed() < self.idle => continue,
                Err(error) => return Err(error),
            };
            last_input = Instant::now();
            if std::mem::take(&mut self.skip_decision_cr) && byte == b'\r' {
                self.skip_lf = true;
                continue;
            }
            if self.skip_lf && byte == b'\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = false;
            match byte {
                b'\r' | b'\n' => {
                    self.skip_lf = byte == b'\r';
                    if self.inner.echoes_input() {
                        self.inner.write_all(b"\r\n")?;
                    }
                    return if overlong {
                        Err(TerminalError::InputTooLong { actual, maximum })
                    } else {
                        Ok(Some(line))
                    };
                }
                8 | 127 if !overlong => {
                    if line.pop().is_some() && !secret && self.inner.echoes_input() {
                        self.inner.write_all(b"\x08 \x08")?;
                    }
                }
                _ => {
                    actual = actual.saturating_add(1);
                    if line.len() >= maximum {
                        overlong = true;
                    }
                    if !overlong {
                        line.push(byte);
                    }
                    if !secret && self.inner.echoes_input() {
                        self.inner.write_all(&[byte])?;
                    }
                }
            }
        }
    }
}

impl Terminal for SessionControlTerminal<'_> {
    fn supports_input_polling(&self) -> bool {
        self.inner.supports_input_polling()
    }
    fn info(&self) -> TerminalInfo {
        self.inner.info()
    }
    fn negotiated_info(&self) -> TerminalInfo {
        self.inner.negotiated_info()
    }
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.inner.write_all(bytes)
    }
    fn read_line(&mut self, maximum: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.line(maximum, false)
    }
    fn read_secret_line(&mut self, maximum: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.line(maximum, true)
    }
    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.idle = timeout;
        self.inner.set_idle_timeout(timeout)
    }
    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        let mut started = Instant::now();
        loop {
            self.cancelled()?;
            if self.invitation()? {
                started = Instant::now();
            }
            match self.inner.read_input_byte(CONTROL_POLL) {
                Ok(Some(b'\r' | b'\n')) => {}
                Ok(value) => return Ok(value),
                Err(TerminalError::InputPollingUnsupported) => return self.inner.read_key(),
                Err(TerminalError::TimedOut) if started.elapsed() < self.idle => {}
                Err(error) => return Err(error),
            }
        }
    }
    fn set_operator_invitation_context(&mut self, enabled: bool) {
        self.invitation_context = enabled;
    }
    fn read_input_byte(&mut self, timeout: Duration) -> Result<Option<u8>, TerminalError> {
        self.cancelled()?;
        self.inner.read_input_byte(timeout)
    }
    fn echoes_input(&self) -> bool {
        self.inner.echoes_input()
    }
    fn emergency_close_handle(&self) -> Result<Option<crate::EmergencyCloseHandle>, TerminalError> {
        self.inner.emergency_close_handle()
    }
    fn take_supplied_credentials(&mut self) -> Option<crate::SuppliedCredentials> {
        self.inner.take_supplied_credentials()
    }
    fn take_verified_caller_grant(&mut self) -> Option<crate::VerifiedCallerGrant> {
        self.inner.take_verified_caller_grant()
    }
    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.cancelled()?;
        self.inner.begin_binary_mode()?;
        self.binary = true;
        Ok(())
    }
    fn read_binary(&mut self, bytes: &mut [u8], timeout: Duration) -> Result<usize, TerminalError> {
        let started = Instant::now();
        loop {
            self.cancelled()?;
            match self.inner.read_binary(
                bytes,
                CONTROL_POLL.min(timeout.saturating_sub(started.elapsed())),
            ) {
                Err(TerminalError::TimedOut) if started.elapsed() < timeout => {}
                result => return result,
            }
        }
    }
    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.cancelled()?;
        self.inner.write_binary(bytes)
    }
    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.binary = false;
        self.inner.end_binary_mode()
    }
    fn disconnect(&mut self) -> Result<(), TerminalError> {
        self.inner.disconnect()
    }
}

/// Full-duplex bounded caller chat; its only text sink is the active terminal
/// and the live InteractionHub channel. Caller-page chat does not pause time.
pub fn run_attached_caller_chat(
    terminal: &mut dyn Terminal,
    hub: &InteractionHub,
    session: SessionId,
    chat: CallerChat,
) -> Result<(), TerminalError> {
    write_text(
        terminal,
        "caller-operator-chat-started",
        &LocalizationArgs::new(),
    )?;
    let mut line = Vec::new();
    let mut active = Instant::now();
    let mut skip_newline = true;
    loop {
        if hub.disconnect_pending(session).map_err(interaction_error)? {
            return Err(TerminalError::OperatorCancelled);
        }
        match chat.receive_line(Duration::ZERO) {
            Ok(Some(reply)) => {
                write_text(
                    terminal,
                    "caller-chat-sysop-line",
                    &LocalizationArgs::new().with("reply", reply),
                )?;
                active = Instant::now();
            }
            Ok(None) => break,
            Err(InteractionError::TimedOut) => {}
            Err(error) => return Err(interaction_error(error)),
        }
        if active.elapsed() >= CHAT_IDLE_TIMEOUT {
            break;
        }
        match terminal.read_input_byte(CONTROL_POLL) {
            Ok(Some(byte)) => {
                active = Instant::now();
                if skip_newline && matches!(byte, b'\r' | b'\n') {
                    continue;
                }
                skip_newline = false;
                match byte {
                    0x1b => break,
                    b'\r' | b'\n' => {
                        skip_newline = true;
                        let decoded = if terminal.info().capabilities.cp437 {
                            Some(crate::file_maintenance::decode_cp437(&line))
                        } else {
                            String::from_utf8(line.clone()).ok()
                        };
                        let Some(text) = decoded.filter(|text| text.len() <= MAX_CHAT_LINE_BYTES)
                        else {
                            write_text(
                                terminal,
                                "caller-operator-chat-invalid-line",
                                &LocalizationArgs::new(),
                            )?;
                            continue;
                        };
                        let text = text.trim();
                        if text.eq_ignore_ascii_case("/Q") {
                            break;
                        }
                        match chat.send_line(text) {
                            Ok(()) => {}
                            Err(InteractionError::Backpressure) => {
                                write_text(
                                    terminal,
                                    "caller-operator-chat-busy",
                                    &LocalizationArgs::new(),
                                )?;
                                continue;
                            }
                            Err(error) => return Err(interaction_error(error)),
                        }
                        line.clear();
                        terminal.write_all(b"\r\n")?;
                    }
                    8 | 127 => {
                        if line.pop().is_some() && terminal.echoes_input() {
                            terminal.write_all(b"\x08 \x08")?;
                        }
                    }
                    byte if byte >= 32 && line.len() < MAX_CHAT_LINE_BYTES => {
                        line.push(byte);
                        if terminal.echoes_input() {
                            terminal.write_all(&[byte])?;
                        }
                    }
                    _ => {}
                }
            }
            Ok(None) => return Err(TerminalError::Disconnected),
            Err(TerminalError::TimedOut) => {}
            Err(error) => return Err(error),
        }
    }
    chat.end();
    write_text(
        terminal,
        "caller-operator-chat-ended",
        &LocalizationArgs::new(),
    )
}

fn interaction_error(_: InteractionError) -> TerminalError {
    TerminalError::MalformedProtocol("operator interaction ended")
}
fn write_text(
    terminal: &mut dyn Terminal,
    key: &str,
    args: &LocalizationArgs,
) -> Result<(), TerminalError> {
    let bytes = crate::localized_bytes(&terminal.info(), key, args);
    terminal.write_all(b"\r\n")?;
    terminal.write_all(&bytes)?;
    terminal.write_all(b"\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Arc};

    struct InvitingTerminal {
        hub: InteractionHub,
        request: Option<crate::PageRequest>,
        operator: Option<crate::OperatorChat>,
        input: VecDeque<u8>,
    }
    impl Terminal for InvitingTerminal {
        fn info(&self) -> TerminalInfo {
            TerminalInfo::in_memory()
        }
        fn write_all(&mut self, _: &[u8]) -> Result<(), TerminalError> {
            Ok(())
        }
        fn read_line(&mut self, _: usize) -> Result<Option<Vec<u8>>, TerminalError> {
            unreachable!()
        }
        fn supports_input_polling(&self) -> bool {
            true
        }
        fn read_input_byte(&mut self, _: Duration) -> Result<Option<u8>, TerminalError> {
            if let Some(request) = self.request.take() {
                self.operator = Some(
                    self.hub
                        .invite(request, "operator".to_owned(), Arc::new(|| true))
                        .unwrap(),
                );
            }
            Ok(self.input.pop_front())
        }
    }

    #[test]
    fn declining_invitation_does_not_execute_partial_menu_input_with_its_newline() {
        let hub = InteractionHub::new();
        let node = crate::NodeId::new(1).unwrap();
        let session = SessionId::new(1).unwrap();
        let nodes = crate::NodeManager::new(vec![crate::NodeDefinition {
            id: node,
            enabled: true,
            description: None,
        }])
        .unwrap();
        let lease = nodes
            .acquire(session, crate::TransportKind::InMemory, 1)
            .unwrap();
        let mut terminal = InvitingTerminal {
            hub: hub.clone(),
            operator: None,
            request: Some(crate::PageRequest {
                interaction_id: 0,
                session_id: session,
                node_id: node,
                caller_id: crate::CallerId::new(1).unwrap(),
                caller_name: "Public Caller".to_owned(),
                requested_at: 1,
                state: crate::PageState::Invited,
            }),
            input: b"XN\r\nY\r".to_vec().into(),
        };
        let mut controlled = SessionControlTerminal::new(&mut terminal, &hub, &lease, session);
        controlled.set_operator_invitation_context(true);
        assert_eq!(controlled.read_line(8).unwrap(), Some(b"XY".to_vec()));
        assert_eq!(hub.paused_allowance(session).unwrap(), Duration::ZERO);
        assert!(hub.interaction_state(session).unwrap().is_none());
    }
}
