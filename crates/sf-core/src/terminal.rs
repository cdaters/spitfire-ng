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

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

use thiserror::Error;

use crate::{CallerId, CallerPreferences};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TransportKind {
    InMemory,
    Telnet,
    RawTcp,
    Rlogin,
    UnixShell,
    Ssh,
    DirectSerial,
    HayesModem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportIdentity {
    pub name: String,
    /// Transport authentication never grants a SPITFIRE caller identity. This
    /// flag only records whether the transport itself authenticated the name.
    pub transport_authenticated: bool,
}

/// Optional credentials supplied by a transport compatibility convention.
/// They remain untrusted until the ordinary SPITFIRE credential verifier
/// accepts them. Debug output is deliberately redacted and the secret bytes
/// are cleared when dropped.
pub struct SuppliedCredentials {
    username: Vec<u8>,
    password: Vec<u8>,
}

/// One-use proof that a transport authenticated a caller through the native
/// credential authority. It contains no secret and never bypasses the stock
/// session's current lifecycle and policy reauthorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCallerGrant {
    pub caller_id: CallerId,
    pub authenticated_state_version: u64,
}

impl SuppliedCredentials {
    pub fn new(username: Vec<u8>, password: Vec<u8>) -> Self {
        Self { username, password }
    }

    pub fn username(&self) -> &[u8] {
        &self.username
    }

    pub fn password(&self) -> &[u8] {
        &self.password
    }
}

impl fmt::Debug for SuppliedCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SuppliedCredentials")
            .field("username", &String::from_utf8_lossy(&self.username))
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Drop for SuppliedCredentials {
    fn drop(&mut self) {
        self.password.fill(0);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCapabilities {
    pub terminal_type: Option<String>,
    pub ansi: bool,
    pub cp437: bool,
    pub size: Option<TerminalSize>,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            terminal_type: None,
            ansi: false,
            cp437: true,
            size: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalInfo {
    pub transport: TransportKind,
    pub local: bool,
    pub capabilities: TerminalCapabilities,
    pub remote_address: Option<SocketAddr>,
    pub connected_at: SystemTime,
    pub connection_speed: Option<u32>,
    pub carrier: Option<bool>,
    pub declared_identity: Option<TransportIdentity>,
}

impl TerminalInfo {
    pub fn in_memory() -> Self {
        Self {
            transport: TransportKind::InMemory,
            local: true,
            capabilities: TerminalCapabilities {
                terminal_type: Some("in-memory".to_owned()),
                ansi: true,
                cp437: true,
                size: Some(TerminalSize {
                    width: 80,
                    // The deterministic in-memory adapter does not have an
                    // interactive human to answer incidental MORE prompts.
                    // Paging tests supply an explicit realistic size.
                    height: 200,
                }),
            },
            remote_address: None,
            connected_at: SystemTime::now(),
            connection_speed: None,
            carrier: None,
            declared_identity: None,
        }
    }
}

/// Byte-oriented terminal boundary shared by local, network, serial, and test
/// adapters. Protocol parsing and capability negotiation remain outside the
/// SPITFIRE session engine.
pub trait Terminal: Send {
    fn info(&self) -> TerminalInfo;

    /// Returns transport-negotiated state before caller presentation
    /// preferences are applied. Wrappers forward this to their underlying
    /// adapter so asynchronous negotiation, such as an SSH window change,
    /// remains truthful in operator diagnostics.
    fn negotiated_info(&self) -> TerminalInfo {
        self.info()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError>;
    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError>;

    /// Configures the maximum interval without caller keyboard input. Network
    /// and serial adapters enforce it at their byte source; line-disciplined
    /// local shells may retain host-controlled blocking behavior.
    fn set_idle_timeout(&mut self, _timeout: Duration) -> Result<(), TerminalError> {
        Ok(())
    }

    /// Reads sensitive input. Network adapters which perform server-side echo
    /// must override this so the password is never echoed.
    fn read_secret_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.read_line(maximum_bytes)
    }

    /// Reads one stock menu command without requiring an Enter key. Terminal
    /// adapters which can observe application bytes immediately override this;
    /// line-disciplined hosts retain the safe line-input fallback.
    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        Ok(self
            .read_line(8)?
            .and_then(|line| line.into_iter().find(|byte| !byte.is_ascii_whitespace())))
    }

    fn take_supplied_credentials(&mut self) -> Option<SuppliedCredentials> {
        None
    }

    fn take_verified_caller_grant(&mut self) -> Option<VerifiedCallerGrant> {
        None
    }

    /// Transfers exclusive ownership of the application byte stream to a
    /// binary protocol engine. Adapters may negotiate protocol-specific
    /// binary transparency here; presentation wrappers must bypass paging,
    /// character translation, and line handling until `end_binary_mode`.
    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        Err(TerminalError::BinaryUnsupported)
    }

    /// Reads application bytes without line editing or presentation-layer
    /// interpretation. A zero-byte read means the peer disconnected.
    fn read_binary(
        &mut self,
        _buffer: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, TerminalError> {
        Err(TerminalError::BinaryUnsupported)
    }

    /// Writes application bytes without presentation-layer transformation.
    /// Transport framing such as Telnet IAC escaping still applies.
    fn write_binary(&mut self, _bytes: &[u8]) -> Result<(), TerminalError> {
        Err(TerminalError::BinaryUnsupported)
    }

    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        Ok(())
    }

    /// Marks the start of one independently abortable presentation unit.
    fn begin_output(&mut self) {}

    fn output_aborted(&self) -> bool {
        false
    }

    /// Enables or suppresses automatic MORE prompts for the current display
    /// resource. The next `begin_output` restores caller preferences.
    fn set_output_paging(&mut self, _enabled: bool) {}

    /// Enables or suppresses caller abort at MORE prompts for the current
    /// display resource. The next `begin_output` restores the default.
    fn set_output_abort(&mut self, _enabled: bool) {}

    /// Displays an explicit stock MORE prompt. This is used by the documented
    /// `^P`/`@PROMPT@` display control and re-enables automatic paging.
    fn prompt_more(&mut self) -> Result<(), TerminalError> {
        let bytes = crate::localized_bytes(
            &self.info(),
            "session-simple-more-prompt",
            &crate::LocalizationArgs::new(),
        );
        self.write_all(b"\r\n")?;
        self.write_all(&bytes)?;
        let _ = self.read_line(8)?;
        self.write_all(b"\r\n")
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        Ok(())
    }
}

/// Applies persistent caller display preferences over transport capabilities.
/// It keeps pagination out of message/file/resource storage and therefore
/// works identically for every terminal adapter.
pub struct PagingTerminal<'a> {
    inner: &'a mut dyn Terminal,
    info: TerminalInfo,
    preferences: CallerPreferences,
    lines: u16,
    aborted: bool,
    paging_override: Option<bool>,
    abort_enabled: bool,
}

impl<'a> PagingTerminal<'a> {
    pub fn new(inner: &'a mut dyn Terminal, preferences: CallerPreferences) -> Self {
        let mut info = inner.info();
        let negotiated_width = info.capabilities.size.map(|size| size.width);
        let negotiated_height = info.capabilities.size.map(|size| size.height);
        info.capabilities.ansi = preferences.graphics.allows_ansi(info.capabilities.ansi);
        info.capabilities.size = Some(TerminalSize {
            width: preferences.effective_width(negotiated_width),
            height: preferences.effective_page_length(negotiated_height),
        });
        Self {
            inner,
            info,
            preferences,
            lines: 0,
            aborted: false,
            paging_override: None,
            abort_enabled: true,
        }
    }

    pub fn set_preferences(&mut self, preferences: CallerPreferences) {
        let mut info = self.inner.info();
        let negotiated_width = info.capabilities.size.map(|size| size.width);
        let negotiated_height = info.capabilities.size.map(|size| size.height);
        info.capabilities.ansi = preferences.graphics.allows_ansi(info.capabilities.ansi);
        info.capabilities.size = Some(TerminalSize {
            width: preferences.effective_width(negotiated_width),
            height: preferences.effective_page_length(negotiated_height),
        });
        self.info = info;
        self.preferences = preferences;
        self.begin_output();
    }

    fn effective_info(&self) -> TerminalInfo {
        let mut info = self.inner.info();
        let negotiated_width = info.capabilities.size.map(|size| size.width);
        let negotiated_height = info.capabilities.size.map(|size| size.height);
        info.capabilities.ansi = self
            .preferences
            .graphics
            .allows_ansi(info.capabilities.ansi);
        info.capabilities.size = Some(TerminalSize {
            width: self.preferences.effective_width(negotiated_width),
            height: self.preferences.effective_page_length(negotiated_height),
        });
        info
    }

    fn stock_more_prompt(&mut self) -> Result<(), TerminalError> {
        let bytes = crate::localized_bytes(
            &self.info,
            "session-more-prompt",
            &crate::LocalizationArgs::new(),
        );
        self.inner.write_all(b"\r\n")?;
        self.inner.write_all(&bytes)?;
        let response = self.inner.read_line(8)?;
        let key = response.as_deref().and_then(|input| {
            input
                .iter()
                .copied()
                .find(|byte| !byte.is_ascii_whitespace())
        });
        let stop = key.is_some_and(|key| {
            key.eq_ignore_ascii_case(&b'S') || key.eq_ignore_ascii_case(&b'Q') || key == 0x1b
        });
        let nonstop = key.is_some_and(|key| key.eq_ignore_ascii_case(&b'N'));
        if stop && self.abort_enabled {
            self.aborted = true;
        } else {
            if nonstop {
                self.paging_override = Some(false);
            }
            if self.preferences.scroll_prompt && self.info.capabilities.ansi {
                self.inner.write_all(b"\r\x1B[2K")?;
            } else {
                self.inner.write_all(b"\r\n")?;
            }
        }
        self.lines = 0;
        Ok(())
    }
}

impl Terminal for PagingTerminal<'_> {
    fn info(&self) -> TerminalInfo {
        self.effective_info()
    }

    fn negotiated_info(&self) -> TerminalInfo {
        self.inner.negotiated_info()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        if self.aborted {
            return Ok(());
        }
        let mut start = 0;
        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte != b'\n' {
                continue;
            }
            self.inner.write_all(&bytes[start..=index])?;
            start = index + 1;
            self.lines = self.lines.saturating_add(1);
            let height = self
                .effective_info()
                .capabilities
                .size
                .map_or(24, |size| size.height);
            if self.paging_override.unwrap_or(self.preferences.more_prompt) && self.lines >= height
            {
                self.stock_more_prompt()?;
                if self.aborted {
                    return Ok(());
                }
            }
        }
        if start < bytes.len() {
            self.inner.write_all(&bytes[start..])?;
        }
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.inner.read_line(maximum_bytes)
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.inner.set_idle_timeout(timeout)
    }

    fn read_secret_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.inner.read_secret_line(maximum_bytes)
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        self.inner.read_key()
    }

    fn take_supplied_credentials(&mut self) -> Option<SuppliedCredentials> {
        self.inner.take_supplied_credentials()
    }

    fn take_verified_caller_grant(&mut self) -> Option<VerifiedCallerGrant> {
        self.inner.take_verified_caller_grant()
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.inner.begin_binary_mode()
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, TerminalError> {
        self.inner.read_binary(buffer, timeout)
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.inner.write_binary(bytes)
    }

    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.inner.end_binary_mode()
    }

    fn begin_output(&mut self) {
        self.lines = 0;
        self.aborted = false;
        self.paging_override = None;
        self.abort_enabled = true;
    }

    fn output_aborted(&self) -> bool {
        self.aborted
    }

    fn set_output_paging(&mut self, enabled: bool) {
        self.paging_override = Some(enabled);
    }

    fn set_output_abort(&mut self, enabled: bool) {
        self.abort_enabled = enabled;
    }

    fn prompt_more(&mut self) -> Result<(), TerminalError> {
        self.paging_override = Some(true);
        self.stock_more_prompt()
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        self.inner.disconnect()
    }
}

#[derive(Debug)]
pub struct InMemoryTerminal {
    input: VecDeque<Vec<u8>>,
    binary_input: VecDeque<u8>,
    output: Vec<u8>,
    info: TerminalInfo,
    disconnected: bool,
    supplied_credentials: Option<SuppliedCredentials>,
    verified_caller_grant: Option<VerifiedCallerGrant>,
    timeout_next_input: bool,
}

impl Default for InMemoryTerminal {
    fn default() -> Self {
        Self {
            input: VecDeque::new(),
            binary_input: VecDeque::new(),
            output: Vec::new(),
            info: TerminalInfo::in_memory(),
            disconnected: false,
            supplied_credentials: None,
            verified_caller_grant: None,
            timeout_next_input: false,
        }
    }
}

impl InMemoryTerminal {
    pub fn with_lines<I, B>(lines: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self {
            input: lines.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    pub fn with_info<I, B>(lines: I, info: TerminalInfo) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self {
            input: lines.into_iter().map(Into::into).collect(),
            info,
            ..Self::default()
        }
    }

    pub fn with_binary_input(bytes: impl IntoIterator<Item = u8>) -> Self {
        Self {
            binary_input: bytes.into_iter().collect(),
            ..Self::default()
        }
    }

    pub fn extend_binary_input(&mut self, bytes: impl IntoIterator<Item = u8>) {
        self.binary_input.extend(bytes);
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn output_text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.output)
    }

    pub const fn disconnected(&self) -> bool {
        self.disconnected
    }

    pub fn set_supplied_credentials(&mut self, credentials: SuppliedCredentials) {
        self.supplied_credentials = Some(credentials);
    }

    pub fn set_verified_caller_grant(&mut self, grant: VerifiedCallerGrant) {
        self.verified_caller_grant = Some(grant);
    }

    pub fn timeout_next_input(&mut self) {
        self.timeout_next_input = true;
    }
}

impl Terminal for InMemoryTerminal {
    fn info(&self) -> TerminalInfo {
        self.info.clone()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        if self.disconnected {
            return Err(TerminalError::Disconnected);
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        if std::mem::take(&mut self.timeout_next_input) {
            return Err(TerminalError::TimedOut);
        }
        if self.disconnected {
            return Ok(None);
        }
        let Some(line) = self.input.pop_front() else {
            return Ok(None);
        };
        if line.len() > maximum_bytes {
            return Err(TerminalError::InputTooLong {
                actual: line.len(),
                maximum: maximum_bytes,
            });
        }
        Ok(Some(line))
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        if std::mem::take(&mut self.timeout_next_input) {
            return Err(TerminalError::TimedOut);
        }
        if self.disconnected {
            return Ok(None);
        }
        Ok(self
            .input
            .pop_front()
            .and_then(|line| line.into_iter().find(|byte| !byte.is_ascii_whitespace())))
    }

    fn take_supplied_credentials(&mut self) -> Option<SuppliedCredentials> {
        self.supplied_credentials.take()
    }

    fn take_verified_caller_grant(&mut self) -> Option<VerifiedCallerGrant> {
        self.verified_caller_grant.take()
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        if self.disconnected {
            return Err(TerminalError::Disconnected);
        }
        Ok(())
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, TerminalError> {
        if self.disconnected {
            return Ok(0);
        }
        if self.binary_input.is_empty() {
            return Err(TerminalError::TimedOut);
        }
        let count = buffer.len().min(self.binary_input.len());
        for destination in &mut buffer[..count] {
            *destination = self.binary_input.pop_front().expect("count is bounded");
        }
        Ok(count)
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.write_all(bytes)
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        self.disconnected = true;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("terminal I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("terminal input is {actual} bytes; maximum is {maximum}")]
    InputTooLong { actual: usize, maximum: usize },
    #[error("terminal protocol data is malformed: {0}")]
    MalformedProtocol(&'static str),
    #[error("terminal binary mode is unsupported by this adapter")]
    BinaryUnsupported,
    #[error("terminal operation timed out")]
    TimedOut,
    #[error("terminal disconnected")]
    Disconnected,
}

#[cfg(test)]
mod paging_tests {
    use super::*;
    use crate::{CallerPreferences, GraphicsPreference};

    #[test]
    fn caller_preferences_override_negotiated_dimensions_and_ansi() {
        let mut inner = InMemoryTerminal::default();
        let terminal = PagingTerminal::new(
            &mut inner,
            CallerPreferences {
                graphics: GraphicsPreference::Text,
                screen_width: Some(100),
                page_length: Some(15),
                ..CallerPreferences::default()
            },
        );
        assert!(!terminal.info().capabilities.ansi);
        assert_eq!(
            terminal.info().capabilities.size,
            Some(TerminalSize {
                width: 100,
                height: 15
            })
        );
    }

    #[test]
    fn stock_more_prompt_continues_stops_or_runs_nonstop_per_output_unit() {
        let mut inner = InMemoryTerminal::with_lines([
            b"".to_vec(),
            b"S".to_vec(),
            b"G".to_vec(),
            b"N".to_vec(),
            b"".to_vec(),
        ]);
        let preferences = CallerPreferences {
            page_length: Some(10),
            ..CallerPreferences::default()
        };
        {
            let mut terminal = PagingTerminal::new(&mut inner, preferences);
            terminal.begin_output();
            terminal
                .write_all(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n")
                .unwrap();
            assert!(!terminal.output_aborted());
            terminal.begin_output();
            terminal
                .write_all(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\nignored\n")
                .unwrap();
            assert!(terminal.output_aborted());
            terminal.begin_output();
            assert_eq!(terminal.read_line(8).unwrap().unwrap(), b"G");
            terminal.begin_output();
            terminal
                .write_all(
                    b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n",
                )
                .unwrap();
            assert!(!terminal.output_aborted());
            terminal.begin_output();
            terminal
                .write_all(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n")
                .unwrap();
        }
        assert_eq!(
            inner
                .output()
                .windows(6)
                .filter(|part| *part == b"MORE: ")
                .count(),
            4
        );
        assert!(inner.output().windows(8).any(|part| part == b"<S>top, "));
        assert!(inner
            .output()
            .windows(11)
            .any(|part| part == b"<N>onstop, "));
        assert!(!inner.output().windows(7).any(|part| part == b"ignored"));
        assert!(inner.output().windows(3).any(|part| part == b"21\n"));
    }

    #[test]
    fn modern_quit_and_escape_aliases_stop_only_the_current_output_unit() {
        for key in [b"Q".to_vec(), vec![0x1b]] {
            let mut inner = InMemoryTerminal::with_lines([key, b"G".to_vec()]);
            let mut terminal = PagingTerminal::new(
                &mut inner,
                CallerPreferences {
                    page_length: Some(10),
                    ..CallerPreferences::default()
                },
            );
            terminal.begin_output();
            terminal
                .write_all(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\nignored\n")
                .unwrap();
            assert!(terminal.output_aborted());
            terminal.begin_output();
            assert_eq!(terminal.read_line(8).unwrap().unwrap(), b"G");
            assert!(!inner.output().windows(7).any(|part| part == b"ignored"));
        }
    }

    #[test]
    fn hot_key_reads_one_command_without_line_mode_or_password_side_effects() {
        let mut inner = InMemoryTerminal::with_lines([b"m".to_vec(), b"full line".to_vec()]);
        let mut terminal = PagingTerminal::new(
            &mut inner,
            CallerPreferences {
                hot_keys: true,
                ..CallerPreferences::default()
            },
        );
        assert_eq!(terminal.read_key().unwrap(), Some(b'm'));
        assert_eq!(terminal.read_line(32).unwrap(), Some(b"full line".to_vec()));
    }

    #[test]
    fn binary_mode_bypasses_paging_and_hot_key_interception() {
        let payload = (0_u8..=255).collect::<Vec<_>>();
        let mut inner = InMemoryTerminal::with_binary_input(payload.clone());
        let mut terminal = PagingTerminal::new(
            &mut inner,
            CallerPreferences {
                page_length: Some(10),
                hot_keys: true,
                ..CallerPreferences::default()
            },
        );
        terminal.begin_binary_mode().unwrap();
        let mut received = vec![0_u8; payload.len()];
        assert_eq!(
            terminal
                .read_binary(&mut received, Duration::from_secs(1))
                .unwrap(),
            payload.len()
        );
        terminal.write_binary(&payload).unwrap();
        terminal.end_binary_mode().unwrap();
        assert_eq!(received, payload);
        assert_eq!(inner.output(), payload);
        assert!(!inner.output().windows(5).any(|part| part == b"<More"));
    }
}
