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
use std::io::{Read, Write};
use std::time::{Duration, Instant, SystemTime};

use sf_core::{NetworkTerminalDefaults, Terminal, TerminalError, TerminalInfo, TransportKind};

use super::configured_capabilities;

pub struct SerialTerminal {
    port: Box<dyn serialport::SerialPort>,
    info: TerminalInfo,
    line: Vec<u8>,
    pending: VecDeque<u8>,
    skip_lf: bool,
    idle_timeout: Duration,
}

impl SerialTerminal {
    pub fn open(
        device: &str,
        baud: u32,
        defaults: &NetworkTerminalDefaults,
    ) -> Result<Self, TerminalError> {
        let mut port = serialport::new(device, baud)
            .timeout(Duration::from_millis(250))
            .open()
            .map_err(serial_error)?;
        let carrier = port.read_carrier_detect().ok();
        Ok(Self::from_port(
            port,
            TransportKind::DirectSerial,
            baud,
            carrier,
            defaults,
        ))
    }

    pub(crate) fn from_port(
        port: Box<dyn serialport::SerialPort>,
        transport: TransportKind,
        speed: u32,
        carrier: Option<bool>,
        defaults: &NetworkTerminalDefaults,
    ) -> Self {
        Self {
            port,
            info: TerminalInfo {
                transport,
                local: false,
                capabilities: configured_capabilities(defaults),
                remote_address: None,
                connected_at: SystemTime::now(),
                connection_speed: Some(speed),
                carrier,
                declared_identity: None,
            },
            line: Vec::new(),
            pending: VecDeque::new(),
            skip_lf: false,
            idle_timeout: Duration::from_secs(300),
        }
    }

    pub(crate) fn set_carrier(&mut self, carrier: bool) {
        self.info.carrier = Some(carrier);
    }

    pub(crate) fn push_pending(&mut self, byte: u8) {
        self.pending.push_back(byte);
    }

    fn read_byte(&mut self) -> Result<Option<u8>, TerminalError> {
        if let Some(byte) = self.pending.pop_front() {
            return Ok(Some(byte));
        }
        let mut byte = [0_u8; 1];
        let started = Instant::now();
        loop {
            match self.port.read(&mut byte) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(byte[0])),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                    if started.elapsed() >= self.idle_timeout {
                        return Err(TerminalError::TimedOut);
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Terminal for SerialTerminal {
    fn info(&self) -> TerminalInfo {
        self.info.clone()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.port.write_all(bytes)?;
        self.port.flush()?;
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        loop {
            let Some(byte) = self.read_byte()? else {
                return Ok((!self.line.is_empty()).then(|| std::mem::take(&mut self.line)));
            };
            if self.skip_lf && byte == b'\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = false;
            match byte {
                b'\r' => {
                    self.skip_lf = true;
                    return Ok(Some(std::mem::take(&mut self.line)));
                }
                b'\n' => return Ok(Some(std::mem::take(&mut self.line))),
                0x08 | 0x7F => {
                    self.line.pop();
                }
                value => {
                    if self.line.len() == maximum_bytes {
                        self.line.clear();
                        let actual = self.drain_overlong_line(maximum_bytes.saturating_add(1))?;
                        return Err(TerminalError::InputTooLong {
                            actual,
                            maximum: maximum_bytes,
                        });
                    }
                    self.line.push(value);
                }
            }
        }
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.idle_timeout = timeout;
        Ok(())
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        loop {
            match self.read_byte()? {
                Some(b'\r' | b'\n') => continue,
                value => return Ok(value),
            }
        }
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.skip_lf = false;
        Ok(())
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, TerminalError> {
        if !self.pending.is_empty() {
            let count = buffer.len().min(self.pending.len());
            for byte in &mut buffer[..count] {
                let Some(value) = self.pending.pop_front() else {
                    return Err(TerminalError::MalformedProtocol(
                        "serial pending-byte queue changed unexpectedly",
                    ));
                };
                *byte = value;
            }
            return Ok(count);
        }
        self.port.set_timeout(timeout).map_err(serial_error)?;
        match self.port.read(buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                Err(TerminalError::TimedOut)
            }
            result => result.map_err(Into::into),
        }
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.write_all(bytes)
    }

    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.port
            .set_timeout(Duration::from_millis(250))
            .map_err(serial_error)
    }
}

impl SerialTerminal {
    fn drain_overlong_line(&mut self, mut actual: usize) -> Result<usize, TerminalError> {
        loop {
            let Some(byte) = self.read_byte()? else {
                return Ok(actual);
            };
            match byte {
                b'\r' => {
                    self.skip_lf = true;
                    return Ok(actual);
                }
                b'\n' => return Ok(actual),
                _ => actual = actual.saturating_add(1),
            }
        }
    }
}

fn serial_error(error: serialport::Error) -> TerminalError {
    std::io::Error::other(error.to_string()).into()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serialport::SerialPort;

    #[test]
    fn pseudo_terminal_exercises_direct_serial_adapter() {
        let (mut master, slave) = serialport::TTYPort::pair().unwrap();
        master.set_timeout(Duration::from_secs(1)).unwrap();
        let mut terminal = SerialTerminal::from_port(
            Box::new(slave),
            TransportKind::DirectSerial,
            38_400,
            None,
            &NetworkTerminalDefaults::default(),
        );
        master.write_all(b"G\r").unwrap();
        master.flush().unwrap();
        assert_eq!(terminal.read_line(8).unwrap(), Some(b"G".to_vec()));
        master.write_all(b"12345678901\r\nG\r").unwrap();
        master.flush().unwrap();
        assert!(matches!(
            terminal.read_line(8),
            Err(TerminalError::InputTooLong {
                actual: 11,
                maximum: 8
            })
        ));
        assert_eq!(terminal.read_line(8).unwrap(), Some(b"G".to_vec()));
        assert_eq!(terminal.info().connection_speed, Some(38_400));
        drop(terminal);
        drop(master);

        let (mut master, slave) = serialport::TTYPort::pair().unwrap();
        master.set_timeout(Duration::from_secs(1)).unwrap();
        let mut terminal = SerialTerminal::from_port(
            Box::new(slave),
            TransportKind::DirectSerial,
            38_400,
            None,
            &NetworkTerminalDefaults::default(),
        );

        // The PTY provides deterministic adapter coverage; exhaustive byte
        // transparency belongs to RAW/Telnet tests because host PTY line
        // disciplines can assign meaning to arbitrary control values.
        let expected = b"\x01BINARY\xff".to_vec();
        master.write_all(&expected).unwrap();
        master.flush().unwrap();
        terminal.begin_binary_mode().unwrap();
        let mut received = vec![0_u8; expected.len()];
        let mut offset = 0;
        while offset < received.len() {
            offset += terminal
                .read_binary(&mut received[offset..], Duration::from_secs(1))
                .unwrap();
        }
        assert_eq!(received, expected);
    }
}
