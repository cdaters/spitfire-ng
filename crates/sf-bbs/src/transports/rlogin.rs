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

use sf_core::{
    NetworkTerminalDefaults, SuppliedCredentials, Terminal, TerminalError, TerminalInfo,
    TransportIdentity, TransportKind,
};

use super::{configured_capabilities, read_bounded_line, read_nul_field};

pub struct RloginTerminal {
    stream: TcpStream,
    info: TerminalInfo,
    skip_lf: bool,
    supplied_credentials: Option<SuppliedCredentials>,
    idle_timeout: Duration,
}

impl RloginTerminal {
    pub fn accept(
        mut stream: TcpStream,
        remote_address: SocketAddr,
        defaults: &NetworkTerminalDefaults,
        auto_login: bool,
    ) -> Result<Self, TerminalError> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let mut initial = [0_u8; 1];
        stream.read_exact(&mut initial)?;
        if initial[0] != 0 {
            return Err(TerminalError::MalformedProtocol(
                "RLogin handshake must begin with NUL",
            ));
        }
        let client_user = read_nul_field(&mut stream)?;
        let server_user = read_nul_field(&mut stream)?;
        let terminal_speed = read_nul_field(&mut stream)?;
        stream.write_all(&[0])?;
        stream.flush()?;
        stream.set_read_timeout(Some(Duration::from_secs(300)))?;
        stream.set_nodelay(true)?;

        let terminal_speed = String::from_utf8_lossy(&terminal_speed);
        let (terminal_type, speed) = terminal_speed
            .split_once('/')
            .map(|(terminal, speed)| (terminal, speed.parse::<u32>().ok()))
            .unwrap_or((&terminal_speed, None));
        let mut capabilities = configured_capabilities(defaults);
        if !terminal_type.is_empty() {
            capabilities.terminal_type = Some(terminal_type.to_owned());
            capabilities.ansi = terminal_type.to_ascii_lowercase().contains("ansi")
                || terminal_type.to_ascii_lowercase().contains("xterm")
                || defaults.ansi;
        }
        // Keep only the requested server/BBS username as identity metadata.
        // SyncTERM places a password in the nominal client-user field, even
        // when this listener has auto-login disabled, so retaining that field
        // in TerminalInfo could accidentally preserve or expose a secret.
        let declared_identity = (!server_user.is_empty()).then(|| TransportIdentity {
            name: String::from_utf8_lossy(&server_user).into_owned(),
            transport_authenticated: false,
        });
        let supplied_credentials =
            (auto_login && !server_user.is_empty() && !client_user.is_empty())
                .then(|| SuppliedCredentials::new(server_user, client_user));

        Ok(Self {
            stream,
            info: TerminalInfo {
                transport: TransportKind::Rlogin,
                local: false,
                capabilities,
                remote_address: Some(remote_address),
                connected_at: SystemTime::now(),
                connection_speed: speed,
                carrier: None,
                declared_identity,
            },
            skip_lf: false,
            supplied_credentials,
            idle_timeout: Duration::from_secs(300),
        })
    }
}

impl Terminal for RloginTerminal {
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

    fn take_supplied_credentials(&mut self) -> Option<SuppliedCredentials> {
        self.supplied_credentials.take()
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
    fn parses_framing_but_marks_rlogin_identity_untrusted() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(b"\0remote-name\0local-name\0xterm/14400\0")
                .unwrap();
            let mut acknowledgement = [1_u8; 1];
            stream.read_exact(&mut acknowledgement).unwrap();
            assert_eq!(acknowledgement, [0]);
        });
        let (stream, remote) = listener.accept().unwrap();
        let terminal =
            RloginTerminal::accept(stream, remote, &NetworkTerminalDefaults::default(), false)
                .unwrap();
        let info = terminal.info();
        assert_eq!(info.connection_speed, Some(14_400));
        assert_eq!(info.capabilities.terminal_type.as_deref(), Some("xterm"));
        assert_eq!(
            info.declared_identity,
            Some(TransportIdentity {
                name: "local-name".to_owned(),
                transport_authenticated: false,
            })
        );
        client.join().unwrap();
    }

    #[test]
    fn syncterm_credentials_are_only_exposed_when_explicitly_enabled() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(b"\0test-only-password\0Alex Caller\0ansi/38400\0")
                .unwrap();
            let mut acknowledgement = [1_u8; 1];
            stream.read_exact(&mut acknowledgement).unwrap();
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            RloginTerminal::accept(stream, remote, &NetworkTerminalDefaults::default(), true)
                .unwrap();
        assert_eq!(
            terminal.info().declared_identity.unwrap().name,
            "Alex Caller"
        );
        let credential = terminal.take_supplied_credentials().unwrap();
        assert_eq!(credential.username(), b"Alex Caller");
        assert_eq!(credential.password(), b"test-only-password");
        assert!(!format!("{credential:?}").contains("test-only-password"));
        client.join().unwrap();
    }

    #[test]
    fn post_handshake_binary_stream_is_byte_transparent() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let expected = (0_u8..=255).collect::<Vec<_>>();
        let client_expected = expected.clone();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(b"\0\0caller\0ansi/38400\0").unwrap();
            let mut acknowledgement = [1_u8; 1];
            stream.read_exact(&mut acknowledgement).unwrap();
            stream.write_all(&client_expected).unwrap();
            let mut returned = vec![0_u8; client_expected.len()];
            stream.read_exact(&mut returned).unwrap();
            assert_eq!(returned, client_expected);
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            RloginTerminal::accept(stream, remote, &NetworkTerminalDefaults::default(), false)
                .unwrap();
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
}
