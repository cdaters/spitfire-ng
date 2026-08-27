use std::env;
use std::io::{BufReader, IsTerminal, Read, Stdin, Stdout, Write};
use std::time::{Duration, SystemTime};

use sf_core::{
    Terminal, TerminalCapabilities, TerminalError, TerminalInfo, TerminalSize, TransportKind,
};

use super::read_bounded_line;

pub struct StdioTerminal {
    input: BufReader<Stdin>,
    output: Stdout,
    info: TerminalInfo,
    skip_lf: bool,
}

impl StdioTerminal {
    pub fn open() -> Self {
        let input = std::io::stdin();
        let output = std::io::stdout();
        let terminal_type = env::var("TERM").ok();
        let ansi = output.is_terminal() && terminal_type.as_deref() != Some("dumb");
        let size = terminal_size::terminal_size().map(|(width, height)| TerminalSize {
            width: width.0,
            height: height.0,
        });
        Self {
            input: BufReader::new(input),
            output,
            info: TerminalInfo {
                transport: TransportKind::UnixShell,
                local: true,
                capabilities: TerminalCapabilities {
                    terminal_type,
                    ansi,
                    cp437: false,
                    size,
                },
                remote_address: None,
                connected_at: SystemTime::now(),
                connection_speed: None,
                carrier: None,
                declared_identity: None,
            },
            skip_lf: false,
        }
    }
}

impl Terminal for StdioTerminal {
    fn info(&self) -> TerminalInfo {
        self.info.clone()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.output.write_all(bytes)?;
        self.output.flush()?;
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        read_bounded_line(&mut self.input, maximum_bytes, &mut self.skip_lf)
    }

    fn read_secret_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        if self.input.get_ref().is_terminal() && self.output.is_terminal() {
            let password = rpassword::read_password()?;
            if password.len() > maximum_bytes {
                return Err(TerminalError::InputTooLong {
                    actual: password.len(),
                    maximum: maximum_bytes,
                });
            }
            return Ok(Some(password.into_bytes()));
        }
        self.read_line(maximum_bytes)
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.skip_lf = false;
        Ok(())
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, TerminalError> {
        self.input.read(buffer).map_err(Into::into)
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.write_all(bytes)
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        self.output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_is_local_but_does_not_grant_an_identity() {
        let terminal = StdioTerminal::open();
        let info = terminal.info();
        assert_eq!(info.transport, TransportKind::UnixShell);
        assert!(info.local);
        assert!(info.declared_identity.is_none());
        assert!(info.remote_address.is_none());
    }
}
