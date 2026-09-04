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

mod modem;
mod raw;
mod rlogin;
mod serial;
mod ssh;
#[cfg(unix)]
mod stdio;
mod telnet;

pub use modem::ModemTerminal;
pub use raw::RawTcpTerminal;
pub use rlogin::RloginTerminal;
pub use serial::SerialTerminal;
pub(crate) use ssh::{
    host_key_fingerprint, load_or_generate_host_key, serve_ssh_listener, SshListenerOptions,
};
#[cfg(unix)]
pub use stdio::StdioTerminal;
pub use telnet::TelnetTerminal;

use std::io::{self, Read};

use sf_core::{NetworkTerminalDefaults, TerminalCapabilities, TerminalError, TerminalSize};

const MAX_PROTOCOL_FIELD: usize = 256;

fn configured_capabilities(defaults: &NetworkTerminalDefaults) -> TerminalCapabilities {
    TerminalCapabilities {
        terminal_type: None,
        ansi: defaults.ansi,
        cp437: defaults.cp437,
        size: Some(TerminalSize {
            width: defaults.width,
            height: defaults.height,
        }),
    }
}

fn read_bounded_line<R: Read>(
    reader: &mut R,
    maximum: usize,
    skip_lf: &mut bool,
) -> Result<Option<Vec<u8>>, TerminalError> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        match reader.read(&mut byte) {
            Ok(0) => return Ok((!line.is_empty()).then_some(line)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(TerminalError::TimedOut)
            }
            Err(error) => return Err(error.into()),
        }
        if *skip_lf && byte[0] == b'\n' {
            *skip_lf = false;
            continue;
        }
        *skip_lf = false;
        match byte[0] {
            b'\r' => {
                *skip_lf = true;
                return Ok(Some(line));
            }
            b'\n' => return Ok(Some(line)),
            0x08 | 0x7F => {
                line.pop();
            }
            value => {
                if line.len() == maximum {
                    let actual = drain_overlong_line(reader, maximum.saturating_add(1), skip_lf)?;
                    return Err(TerminalError::InputTooLong { actual, maximum });
                }
                line.push(value);
            }
        }
    }
}

fn drain_overlong_line<R: Read>(
    reader: &mut R,
    mut actual: usize,
    skip_lf: &mut bool,
) -> Result<usize, TerminalError> {
    loop {
        let mut byte = [0_u8; 1];
        match reader.read(&mut byte) {
            Ok(0) => return Ok(actual),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(TerminalError::TimedOut)
            }
            Err(error) => return Err(error.into()),
        }
        match byte[0] {
            b'\r' => {
                *skip_lf = true;
                return Ok(actual);
            }
            b'\n' => return Ok(actual),
            _ => actual = actual.saturating_add(1),
        }
    }
}

fn read_nul_field<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut field = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        reader.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return Ok(field);
        }
        if field.len() == MAX_PROTOCOL_FIELD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "RLogin field exceeds 256 bytes",
            ));
        }
        field.push(byte[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_line_drains_oversized_input_before_the_next_prompt() {
        let mut input = io::Cursor::new(b"12345678901\r\nG\r".to_vec());
        let mut skip_lf = false;
        assert!(matches!(
            read_bounded_line(&mut input, 8, &mut skip_lf),
            Err(TerminalError::InputTooLong {
                actual: 11,
                maximum: 8
            })
        ));
        assert_eq!(
            read_bounded_line(&mut input, 8, &mut skip_lf).unwrap(),
            Some(b"G".to_vec())
        );
    }
}
