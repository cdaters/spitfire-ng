use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, SystemTime};

use sf_core::{
    NetworkTerminalDefaults, Terminal, TerminalError, TerminalInfo, TerminalSize, TransportKind,
};

use super::configured_capabilities;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const OPT_BINARY: u8 = 0;
const OPT_ECHO: u8 = 1;
const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
const OPT_TERMINAL_TYPE: u8 = 24;
const OPT_NAWS: u8 = 31;
const TERMINAL_TYPE_IS: u8 = 0;
const TERMINAL_TYPE_SEND: u8 = 1;
const MAX_SUBNEGOTIATION_BYTES: usize = 1024;

const INITIAL_NEGOTIATION: &[u8] = &[
    IAC,
    WILL,
    OPT_ECHO,
    IAC,
    WILL,
    OPT_SUPPRESS_GO_AHEAD,
    IAC,
    WILL,
    OPT_BINARY,
    IAC,
    DO,
    OPT_BINARY,
    IAC,
    DO,
    OPT_TERMINAL_TYPE,
    IAC,
    DO,
    OPT_NAWS,
];

pub struct TelnetTerminal {
    stream: TcpStream,
    protocol: TelnetProtocol,
    info: TerminalInfo,
    line: Vec<u8>,
    skip_lf: bool,
    idle_timeout: Duration,
}

impl TelnetTerminal {
    pub fn accept(
        mut stream: TcpStream,
        remote_address: SocketAddr,
        defaults: &NetworkTerminalDefaults,
    ) -> Result<Self, TerminalError> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(300)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        stream.set_nodelay(true)?;
        stream.write_all(INITIAL_NEGOTIATION)?;
        stream.flush()?;
        Ok(Self {
            stream,
            protocol: TelnetProtocol::new(),
            info: TerminalInfo {
                transport: TransportKind::Telnet,
                local: false,
                capabilities: configured_capabilities(defaults),
                remote_address: Some(remote_address),
                connected_at: SystemTime::now(),
                connection_speed: None,
                carrier: None,
                declared_identity: None,
            },
            line: Vec::new(),
            skip_lf: false,
            idle_timeout: Duration::from_secs(300),
        })
    }

    fn apply_negotiated_capabilities(&mut self) {
        if let Some(terminal_type) = self.protocol.terminal_type.clone() {
            let lowercase = terminal_type.to_ascii_lowercase();
            self.info.capabilities.ansi = self.info.capabilities.ansi
                || lowercase.contains("ansi")
                || lowercase.contains("xterm")
                || lowercase.contains("vt100");
            self.info.capabilities.terminal_type = Some(terminal_type);
        }
        if let Some(size) = self.protocol.size {
            self.info.capabilities.size = Some(size);
        }
    }

    fn read_application_byte(&mut self) -> Result<Option<u8>, TerminalError> {
        loop {
            if let Some(byte) = self.protocol.application.pop_front() {
                return Ok(Some(byte));
            }
            let mut input = [0_u8; 512];
            let count = loop {
                match self.stream.read(&mut input) {
                    Ok(count) => break count,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                    {
                        return Err(TerminalError::TimedOut)
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            if count == 0 {
                return Ok(None);
            }
            self.protocol.feed(&input[..count])?;
            if !self.protocol.outbound.is_empty() {
                self.stream.write_all(&self.protocol.outbound)?;
                self.stream.flush()?;
                self.protocol.outbound.clear();
            }
            self.apply_negotiated_capabilities();
        }
    }

    fn read_terminal_line(
        &mut self,
        maximum_bytes: usize,
        echo: bool,
    ) -> Result<Option<Vec<u8>>, TerminalError> {
        loop {
            let Some(byte) = self.read_application_byte()? else {
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
                    self.stream.write_all(b"\r\n")?;
                    self.stream.flush()?;
                    return Ok(Some(std::mem::take(&mut self.line)));
                }
                b'\n' => {
                    self.stream.write_all(b"\r\n")?;
                    self.stream.flush()?;
                    return Ok(Some(std::mem::take(&mut self.line)));
                }
                0x08 | 0x7F => {
                    if self.line.pop().is_some() && echo {
                        self.stream.write_all(b"\x08 \x08")?;
                        self.stream.flush()?;
                    }
                }
                value => {
                    if self.line.len() == maximum_bytes {
                        self.line.clear();
                        let actual =
                            self.drain_overlong_line(maximum_bytes.saturating_add(1), echo)?;
                        return Err(TerminalError::InputTooLong {
                            actual,
                            maximum: maximum_bytes,
                        });
                    }
                    self.line.push(value);
                    if echo {
                        self.stream.write_all(&[value])?;
                        self.stream.flush()?;
                    }
                }
            }
        }
    }

    fn drain_overlong_line(
        &mut self,
        mut actual: usize,
        echo: bool,
    ) -> Result<usize, TerminalError> {
        loop {
            let Some(byte) = self.read_application_byte()? else {
                return Ok(actual);
            };
            match byte {
                b'\r' => {
                    self.skip_lf = true;
                    if echo {
                        self.stream.write_all(b"\r\n")?;
                        self.stream.flush()?;
                    }
                    return Ok(actual);
                }
                b'\n' => {
                    if echo {
                        self.stream.write_all(b"\r\n")?;
                        self.stream.flush()?;
                    }
                    return Ok(actual);
                }
                _ => actual = actual.saturating_add(1),
            }
        }
    }
}

impl Terminal for TelnetTerminal {
    fn info(&self) -> TerminalInfo {
        self.info.clone()
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        for segment in bytes.split_inclusive(|byte| *byte == IAC) {
            self.stream.write_all(segment)?;
            if segment.last() == Some(&IAC) {
                self.stream.write_all(&[IAC])?;
            }
        }
        self.stream.flush()?;
        Ok(())
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.read_terminal_line(maximum_bytes, true)
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.stream.set_read_timeout(Some(timeout))?;
        self.idle_timeout = timeout;
        Ok(())
    }

    fn read_secret_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.read_terminal_line(maximum_bytes, false)
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        loop {
            match self.read_application_byte()? {
                Some(b'\r' | b'\n') => continue,
                value => return Ok(value),
            }
        }
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.skip_lf = false;
        if !self.protocol.local_binary || !self.protocol.remote_binary {
            return Err(TerminalError::BinaryUnsupported);
        }
        Ok(())
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, TerminalError> {
        self.stream.set_read_timeout(Some(timeout))?;
        let mut count = 0;
        while count < buffer.len() {
            match self.read_application_byte() {
                Ok(Some(byte)) => {
                    buffer[count] = byte;
                    count += 1;
                    if self.protocol.application.is_empty() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(TerminalError::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    if count == 0 {
                        return Err(TerminalError::TimedOut);
                    }
                    break;
                }
                Err(TerminalError::TimedOut) => {
                    if count == 0 {
                        return Err(TerminalError::TimedOut);
                    }
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(count)
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

#[derive(Clone, Debug)]
enum ParseState {
    Data,
    Iac,
    Negotiation(u8),
    Subnegotiation {
        option: Option<u8>,
        data: Vec<u8>,
        after_iac: bool,
    },
}

#[derive(Clone, Debug)]
struct TelnetProtocol {
    state: ParseState,
    application: VecDeque<u8>,
    outbound: Vec<u8>,
    terminal_type: Option<String>,
    size: Option<TerminalSize>,
    local_binary: bool,
    remote_binary: bool,
}

impl TelnetProtocol {
    fn new() -> Self {
        Self {
            state: ParseState::Data,
            application: VecDeque::new(),
            outbound: Vec::new(),
            terminal_type: None,
            size: None,
            local_binary: false,
            remote_binary: false,
        }
    }

    fn feed(&mut self, input: &[u8]) -> Result<(), TerminalError> {
        for &byte in input {
            let state = std::mem::replace(&mut self.state, ParseState::Data);
            self.state = match state {
                ParseState::Data if byte == IAC => ParseState::Iac,
                ParseState::Data => {
                    self.application.push_back(byte);
                    ParseState::Data
                }
                ParseState::Iac => match byte {
                    IAC => {
                        self.application.push_back(IAC);
                        ParseState::Data
                    }
                    DO | DONT | WILL | WONT => ParseState::Negotiation(byte),
                    SB => ParseState::Subnegotiation {
                        option: None,
                        data: Vec::new(),
                        after_iac: false,
                    },
                    _ => ParseState::Data,
                },
                ParseState::Negotiation(command) => {
                    self.handle_negotiation(command, byte);
                    ParseState::Data
                }
                ParseState::Subnegotiation {
                    mut option,
                    mut data,
                    mut after_iac,
                } => {
                    if option.is_none() {
                        option = Some(byte);
                    } else if after_iac {
                        after_iac = false;
                        if byte == SE {
                            self.finish_subnegotiation(option.unwrap_or_default(), &data)?;
                            continue;
                        } else if byte == IAC {
                            data.push(IAC);
                        } else {
                            return Err(TerminalError::MalformedProtocol(
                                "invalid Telnet subnegotiation escape",
                            ));
                        }
                    } else if byte == IAC {
                        after_iac = true;
                    } else {
                        if data.len() == MAX_SUBNEGOTIATION_BYTES {
                            return Err(TerminalError::MalformedProtocol(
                                "Telnet subnegotiation exceeds 1024 bytes",
                            ));
                        }
                        data.push(byte);
                    }
                    ParseState::Subnegotiation {
                        option,
                        data,
                        after_iac,
                    }
                }
            };
        }
        Ok(())
    }

    fn handle_negotiation(&mut self, command: u8, option: u8) {
        let supported_remote = matches!(option, OPT_BINARY | OPT_TERMINAL_TYPE | OPT_NAWS);
        let supported_local = matches!(option, OPT_BINARY | OPT_ECHO | OPT_SUPPRESS_GO_AHEAD);
        match (command, option) {
            (WILL, OPT_BINARY) => self.remote_binary = true,
            (WONT, OPT_BINARY) => self.remote_binary = false,
            (DO, OPT_BINARY) => self.local_binary = true,
            (DONT, OPT_BINARY) => self.local_binary = false,
            _ => {}
        }
        match (command, supported_remote, supported_local) {
            (WILL, true, _) if option == OPT_TERMINAL_TYPE => {
                self.outbound.extend_from_slice(&[
                    IAC,
                    SB,
                    OPT_TERMINAL_TYPE,
                    TERMINAL_TYPE_SEND,
                    IAC,
                    SE,
                ]);
            }
            (WILL, false, _) => self.outbound.extend_from_slice(&[IAC, DONT, option]),
            (DO, _, false) => self.outbound.extend_from_slice(&[IAC, WONT, option]),
            _ => {}
        }
    }

    fn finish_subnegotiation(&mut self, option: u8, data: &[u8]) -> Result<(), TerminalError> {
        match option {
            OPT_TERMINAL_TYPE if data.first() == Some(&TERMINAL_TYPE_IS) => {
                let terminal = &data[1..];
                if terminal.is_empty()
                    || terminal.len() > 64
                    || !terminal.iter().all(u8::is_ascii_graphic)
                {
                    return Err(TerminalError::MalformedProtocol(
                        "invalid Telnet terminal type",
                    ));
                }
                self.terminal_type = Some(String::from_utf8_lossy(terminal).into_owned());
            }
            OPT_NAWS if data.len() == 4 => {
                let width = u16::from_be_bytes([data[0], data[1]]);
                let height = u16::from_be_bytes([data[2], data[3]]);
                if width > 0 && height > 0 {
                    self.size = Some(TerminalSize { width, height });
                }
            }
            OPT_NAWS => {
                return Err(TerminalError::MalformedProtocol(
                    "Telnet NAWS payload must contain four bytes",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn parses_terminal_type_window_size_and_application_data() {
        let mut protocol = TelnetProtocol::new();
        protocol
            .feed(&[
                IAC,
                WILL,
                OPT_TERMINAL_TYPE,
                IAC,
                SB,
                OPT_TERMINAL_TYPE,
                TERMINAL_TYPE_IS,
                b'A',
                b'N',
                b'S',
                b'I',
                IAC,
                SE,
                IAC,
                SB,
                OPT_NAWS,
                0,
                100,
                0,
                40,
                IAC,
                SE,
                b'G',
                b'\r',
            ])
            .unwrap();
        assert_eq!(protocol.terminal_type.as_deref(), Some("ANSI"));
        assert_eq!(
            protocol.size,
            Some(TerminalSize {
                width: 100,
                height: 40
            })
        );
        assert_eq!(protocol.application.into_iter().collect::<Vec<_>>(), b"G\r");
        assert!(protocol
            .outbound
            .windows(6)
            .any(|bytes| { bytes == [IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND, IAC, SE,] }));
    }

    #[test]
    fn rejects_malformed_or_unbounded_subnegotiation() {
        let mut bad_naws = TelnetProtocol::new();
        assert!(bad_naws.feed(&[IAC, SB, OPT_NAWS, 1, 2, IAC, SE]).is_err());

        let mut unbounded = vec![IAC, SB, 99];
        unbounded.extend(std::iter::repeat_n(b'X', MAX_SUBNEGOTIATION_BYTES + 1));
        assert!(TelnetProtocol::new().feed(&unbounded).is_err());
    }

    #[test]
    fn telnet_hot_key_is_immediate_after_protocol_filtering() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut negotiation = vec![0_u8; INITIAL_NEGOTIATION.len()];
            stream.read_exact(&mut negotiation).unwrap();
            stream.write_all(&[IAC, WILL, OPT_BINARY, b'm']).unwrap();
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            TelnetTerminal::accept(stream, remote, &NetworkTerminalDefaults::default()).unwrap();
        assert_eq!(terminal.read_key().unwrap(), Some(b'm'));
        client.join().unwrap();
    }

    #[test]
    fn telnet_line_reader_drains_oversized_input_before_the_next_prompt() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut negotiation = vec![0_u8; INITIAL_NEGOTIATION.len()];
            stream.read_exact(&mut negotiation).unwrap();
            stream.write_all(b"12345678901\r\nG\r").unwrap();
            finished_rx.recv().unwrap();
        });
        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            TelnetTerminal::accept(stream, remote, &NetworkTerminalDefaults::default()).unwrap();

        assert!(matches!(
            terminal.read_line(8),
            Err(TerminalError::InputTooLong {
                actual: 11,
                maximum: 8
            })
        ));
        assert_eq!(terminal.read_line(8).unwrap(), Some(b"G".to_vec()));

        finished_tx.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn binary_mode_preserves_every_application_byte_and_escapes_iac() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let expected = (0_u8..=255).collect::<Vec<_>>();
        let client_expected = expected.clone();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut negotiation = vec![0_u8; INITIAL_NEGOTIATION.len()];
            stream.read_exact(&mut negotiation).unwrap();
            assert_eq!(negotiation, INITIAL_NEGOTIATION);
            let mut encoded = vec![IAC, WILL, OPT_BINARY, IAC, DO, OPT_BINARY];
            for byte in &client_expected {
                encoded.push(*byte);
                if *byte == IAC {
                    encoded.push(IAC);
                }
            }
            stream.write_all(&encoded).unwrap();
            let mut returned = vec![0_u8; client_expected.len() + 1];
            stream.read_exact(&mut returned).unwrap();
            let mut expected_wire = client_expected.clone();
            expected_wire.push(IAC);
            assert_eq!(returned, expected_wire);
        });

        let (stream, remote) = listener.accept().unwrap();
        let mut terminal =
            TelnetTerminal::accept(stream, remote, &NetworkTerminalDefaults::default()).unwrap();
        let mut received = Vec::new();
        while received.len() < expected.len() {
            let mut buffer = [0_u8; 256];
            let count = terminal
                .read_binary(&mut buffer, Duration::from_secs(1))
                .unwrap();
            received.extend_from_slice(&buffer[..count]);
        }
        terminal.begin_binary_mode().unwrap();
        assert_eq!(received, expected);
        terminal.write_binary(&expected).unwrap();
        client.join().unwrap();
    }
}
