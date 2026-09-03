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

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use sf_core::{NetworkTerminalDefaults, Terminal, TerminalError, TerminalInfo, TransportKind};

use super::SerialTerminal;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HayesModemState {
    Initializing,
    WaitingForRing,
    Answering,
    Connected { speed: u32 },
    CarrierLost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HayesAction {
    None,
    Send(Vec<u8>),
    Connected(u32),
    CarrierLost,
}

#[derive(Clone, Debug)]
pub struct HayesModemStateMachine {
    state: HayesModemState,
    initialization: Vec<u8>,
    answer: Vec<u8>,
}

impl HayesModemStateMachine {
    pub fn new(initialization: &str, answer: &str) -> Self {
        Self {
            state: HayesModemState::Initializing,
            initialization: command_bytes(initialization),
            answer: command_bytes(answer),
        }
    }

    #[cfg(test)]
    pub const fn state(&self) -> HayesModemState {
        self.state
    }

    pub fn start(&self) -> HayesAction {
        HayesAction::Send(self.initialization.clone())
    }

    pub fn receive_line(&mut self, line: &[u8]) -> HayesAction {
        let normalized = String::from_utf8_lossy(line).trim().to_ascii_uppercase();
        if normalized == "NO CARRIER" {
            self.state = HayesModemState::CarrierLost;
            return HayesAction::CarrierLost;
        }
        match self.state {
            HayesModemState::Initializing if normalized == "OK" => {
                self.state = HayesModemState::WaitingForRing;
                HayesAction::None
            }
            HayesModemState::WaitingForRing if normalized == "RING" => {
                self.state = HayesModemState::Answering;
                HayesAction::Send(self.answer.clone())
            }
            HayesModemState::Answering if normalized.starts_with("CONNECT") => {
                let speed = normalized
                    .split_ascii_whitespace()
                    .nth(1)
                    .and_then(|value| value.split('/').next())
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(300);
                self.state = HayesModemState::Connected { speed };
                HayesAction::Connected(speed)
            }
            _ => HayesAction::None,
        }
    }
}

fn command_bytes(command: &str) -> Vec<u8> {
    let mut bytes = command.as_bytes().to_vec();
    bytes.push(b'\r');
    bytes
}

pub struct ModemTerminal {
    serial: SerialTerminal,
    state: HayesModemStateMachine,
}

impl ModemTerminal {
    pub fn answer(
        device: &str,
        dte_baud: u32,
        initialization: &str,
        answer: &str,
        defaults: &NetworkTerminalDefaults,
    ) -> Result<Self, TerminalError> {
        let port = serialport::new(device, dte_baud)
            .timeout(Duration::from_millis(250))
            .open()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Self::answer_port(
            port,
            initialization,
            answer,
            defaults,
            Duration::from_secs(300),
        )
    }

    pub(crate) fn answer_port(
        mut port: Box<dyn serialport::SerialPort>,
        initialization: &str,
        answer: &str,
        defaults: &NetworkTerminalDefaults,
        wait_limit: Duration,
    ) -> Result<Self, TerminalError> {
        let mut state = HayesModemStateMachine::new(initialization, answer);
        if let HayesAction::Send(command) = state.start() {
            port.write_all(&command)?;
            port.flush()?;
        }
        let started = Instant::now();
        let connect_speed = loop {
            if started.elapsed() > wait_limit {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "modem did not establish inbound carrier within five minutes",
                )
                .into());
            }
            let Some(line) = read_modem_line(&mut port)? else {
                continue;
            };
            match state.receive_line(&line) {
                HayesAction::Send(command) => {
                    port.write_all(&command)?;
                    port.flush()?;
                }
                HayesAction::Connected(speed) => break speed,
                HayesAction::CarrierLost => {
                    return Err(TerminalError::Disconnected);
                }
                HayesAction::None => {}
            }
        };
        let pending = read_optional_line_feed(&mut port)?;
        let mut serial = SerialTerminal::from_port(
            port,
            TransportKind::HayesModem,
            connect_speed,
            Some(true),
            defaults,
        );
        if let Some(byte) = pending {
            serial.push_pending(byte);
        }
        Ok(Self { serial, state })
    }
}

impl Terminal for ModemTerminal {
    fn info(&self) -> TerminalInfo {
        self.serial.info()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.serial.write_all(bytes)
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        let line = self.serial.read_line(maximum_bytes)?;
        if line.as_deref().is_some_and(|line| {
            String::from_utf8_lossy(line)
                .trim()
                .eq_ignore_ascii_case("NO CARRIER")
        }) {
            let _ = self.state.receive_line(b"NO CARRIER");
            self.serial.set_carrier(false);
            return Ok(None);
        }
        Ok(line)
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.serial.set_idle_timeout(timeout)
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        self.serial.read_key()
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.serial.begin_binary_mode()
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, TerminalError> {
        self.serial.read_binary(buffer, timeout)
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.serial.write_binary(bytes)
    }

    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.serial.end_binary_mode()
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        self.serial.disconnect()
    }
}

fn read_modem_line(
    port: &mut Box<dyn serialport::SerialPort>,
) -> Result<Option<Vec<u8>>, TerminalError> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        match port.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) if matches!(byte[0], b'\r' | b'\n') => {
                if !line.is_empty() {
                    return Ok(Some(line));
                }
            }
            Ok(_) => {
                if line.len() == 256 {
                    return Err(TerminalError::MalformedProtocol(
                        "modem result line exceeds 256 bytes",
                    ));
                }
                line.push(byte[0]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
}

fn read_optional_line_feed(
    port: &mut Box<dyn serialport::SerialPort>,
) -> Result<Option<u8>, TerminalError> {
    let mut byte = [0_u8; 1];
    match port.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) if byte[0] == b'\n' => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use serialport::SerialPort;
    #[cfg(unix)]
    use std::thread;

    #[test]
    fn simulates_inbound_ring_answer_connect_and_carrier_loss() {
        let mut modem = HayesModemStateMachine::new("AT&F", "ATA");
        assert_eq!(modem.start(), HayesAction::Send(b"AT&F\r".to_vec()));
        assert_eq!(modem.receive_line(b"OK"), HayesAction::None);
        assert_eq!(modem.state(), HayesModemState::WaitingForRing);
        assert_eq!(
            modem.receive_line(b"RING"),
            HayesAction::Send(b"ATA\r".to_vec())
        );
        assert_eq!(
            modem.receive_line(b"CONNECT 14400/ARQ"),
            HayesAction::Connected(14400)
        );
        assert_eq!(modem.state(), HayesModemState::Connected { speed: 14400 });
        assert_eq!(modem.receive_line(b"NO CARRIER"), HayesAction::CarrierLost);
        assert_eq!(modem.state(), HayesModemState::CarrierLost);
    }

    #[cfg(unix)]
    #[test]
    fn simulated_connect_exposes_a_binary_clean_terminal_stream() {
        let (mut master, slave) = serialport::TTYPort::pair().unwrap();
        master.set_timeout(Duration::from_secs(1)).unwrap();
        // Keep this PTY test focused on the Hayes-to-binary-stream handoff;
        // exhaustive byte transparency is covered deterministically by RAW
        // and Telnet because host PTY line disciplines reserve controls.
        let expected = b"\x01MODEM-BINARY\xff".to_vec();
        let peer_expected = expected.clone();
        let peer = thread::spawn(move || {
            let mut command = Vec::new();
            loop {
                let mut byte = [0_u8; 1];
                master.read_exact(&mut byte).unwrap();
                if byte[0] == b'\r' {
                    break;
                }
                command.push(byte[0]);
            }
            assert_eq!(command, b"AT&F");
            master.write_all(b"OK\r\nRING\r\n").unwrap();
            master.flush().unwrap();
            command.clear();
            loop {
                let mut byte = [0_u8; 1];
                master.read_exact(&mut byte).unwrap();
                if byte[0] == b'\r' {
                    break;
                }
                command.push(byte[0]);
            }
            assert_eq!(command, b"ATA");
            master.write_all(b"CONNECT 14400\r\n").unwrap();
            master.write_all(&peer_expected).unwrap();
            master.flush().unwrap();
            let mut returned = vec![0_u8; peer_expected.len()];
            master.read_exact(&mut returned).unwrap();
            assert_eq!(returned, peer_expected);
        });
        let mut terminal = ModemTerminal::answer_port(
            Box::new(slave),
            "AT&F",
            "ATA",
            &NetworkTerminalDefaults::default(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(terminal.info().connection_speed, Some(14_400));
        terminal.begin_binary_mode().unwrap();
        let mut received = vec![0_u8; expected.len()];
        let mut offset = 0;
        while offset < received.len() {
            offset += terminal
                .read_binary(&mut received[offset..], Duration::from_secs(1))
                .unwrap();
        }
        assert_eq!(received, expected);
        terminal.write_binary(&expected).unwrap();
        peer.join().unwrap();
    }
}
