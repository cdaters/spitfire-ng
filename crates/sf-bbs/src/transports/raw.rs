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
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, SystemTime};

use sf_core::{NetworkTerminalDefaults, Terminal, TerminalError, TerminalInfo, TransportKind};

use super::{configured_capabilities, read_bounded_line};

pub struct RawTcpTerminal {
    stream: TcpStream,
    info: TerminalInfo,
    skip_lf: bool,
    idle_timeout: Duration,
}

impl RawTcpTerminal {
    pub fn new(
        stream: TcpStream,
        remote_address: SocketAddr,
        defaults: &NetworkTerminalDefaults,
    ) -> Result<Self, TerminalError> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(300)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            info: TerminalInfo {
                transport: TransportKind::RawTcp,
                local: false,
                capabilities: configured_capabilities(defaults),
                remote_address: Some(remote_address),
                connected_at: SystemTime::now(),
                connection_speed: None,
                carrier: None,
                declared_identity: None,
            },
            skip_lf: false,
            idle_timeout: Duration::from_secs(300),
        })
    }
}

impl Terminal for RawTcpTerminal {
    fn supports_input_polling(&self) -> bool {
        true
    }
    fn read_input_byte(&mut self, timeout: Duration) -> Result<Option<u8>, TerminalError> {
        let mut byte = [0_u8; 1];
        let result = self
            .read_binary(&mut byte, timeout)
            .map(|count| (count != 0).then_some(byte[0]));
        self.end_binary_mode()?;
        result
    }
    fn emergency_close_handle(
        &self,
    ) -> Result<Option<sf_core::EmergencyCloseHandle>, TerminalError> {
        let stream = self.stream.try_clone()?;
        Ok(Some(std::sync::Arc::new(move || {
            match stream.shutdown(Shutdown::Both) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotConnected => Ok(()),
                Err(error) => Err(error.into()),
            }
        })))
    }
    fn info(&self) -> TerminalInfo {
        self.info.clone()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        read_bounded_line(&mut self.stream, maximum_bytes, &mut self.skip_lf)
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.stream.set_read_timeout(Some(timeout))?;
        self.idle_timeout = timeout;
        Ok(())
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        loop {
            let mut byte = [0_u8; 1];
            match self.stream.read(&mut byte).map_err(map_input_error)? {
                0 => return Ok(None),
                _ if matches!(byte[0], b'\r' | b'\n') => continue,
                _ => return Ok(Some(byte[0])),
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
        self.stream.set_read_timeout(Some(timeout))?;
        match self.stream.read(buffer) {
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                Err(TerminalError::TimedOut)
            }
            result => result.map_err(Into::into),
        }
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.write_all(bytes)
    }

    fn end_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.stream.set_read_timeout(Some(self.idle_timeout))?;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        match self.stream.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn map_input_error(error: std::io::Error) -> TerminalError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        TerminalError::TimedOut
    } else {
        error.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn raw_binary_mode_is_byte_transparent() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let expected = (0_u8..=255).collect::<Vec<_>>();
        let client_expected = expected.clone();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(&client_expected).unwrap();
            let mut returned = vec![0_u8; client_expected.len()];
            stream.read_exact(&mut returned).unwrap();
            assert_eq!(returned, client_expected);
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            RawTcpTerminal::new(stream, remote, &NetworkTerminalDefaults::default()).unwrap();
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
        client.join().unwrap();
    }

    #[test]
    fn raw_hot_key_does_not_require_a_line_terminator() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            TcpStream::connect(address)
                .unwrap()
                .write_all(b"m")
                .unwrap();
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            RawTcpTerminal::new(stream, remote, &NetworkTerminalDefaults::default()).unwrap();
        assert_eq!(terminal.read_key().unwrap(), Some(b'm'));
        client.join().unwrap();
    }
}
